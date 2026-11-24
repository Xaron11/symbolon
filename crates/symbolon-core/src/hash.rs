//! Content hashing: algorithms, full digests and partial digests.
//!
//! A [`Digest`] is a hash of a file's whole content and is the only proof that two files are
//! identical. A [`PartialDigest`] covers the size plus the first and last [`PARTIAL_BLOCK`]
//! bytes; it can only rule matches out, so it is a distinct type that never compares equal to,
//! or converts into, a [`Digest`].
//!
//! Digests remember their algorithm, and digests of different algorithms are never equal: two
//! locations hashed with different algorithms cannot be mistaken for matching.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::str::FromStr;

use sha2::Digest as _;

/// Bytes hashed from each end of a file for a [`PartialDigest`].
pub const PARTIAL_BLOCK: u64 = 64 * 1024;

const MAX_DIGEST_LEN: usize = 32;
const READ_BUFFER: usize = 256 * 1024;

/// A content hash algorithm. Stored per location; see the book chapter *Concepts*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HashAlgo {
    /// BLAKE3, 256-bit. The default: fast, SIMD-accelerated and cryptographically strong.
    #[default]
    Blake3,
    /// XXH3, 128-bit. Faster than BLAKE3 but not collision-resistant against adversaries.
    Xxh3,
    /// SHA-256. Slowest; matches what Android's `sha256sum` produces.
    Sha256,
}

impl HashAlgo {
    /// Every algorithm, in a stable order.
    pub const ALL: [Self; 3] = [Self::Blake3, Self::Xxh3, Self::Sha256];

    /// The stable name used in the catalog and on the command line.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Blake3 => "blake3",
            Self::Xxh3 => "xxh3",
            Self::Sha256 => "sha256",
        }
    }

    /// Length of this algorithm's digest in bytes.
    pub const fn digest_len(self) -> usize {
        match self {
            Self::Blake3 | Self::Sha256 => 32,
            Self::Xxh3 => 16,
        }
    }
}

impl fmt::Display for HashAlgo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for HashAlgo {
    type Err = HashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|algo| algo.name() == s)
            .ok_or_else(|| HashError::UnknownAlgo(s.to_owned()))
    }
}

/// Errors from hashing or from decoding stored digests.
#[derive(Debug, thiserror::Error)]
pub enum HashError {
    /// Reading the content failed.
    #[error("read failed: {0}")]
    Io(#[from] io::Error),
    /// The content was shorter or longer than its recorded size: the file changed, or the
    /// storage returned a short read. The partial result is discarded.
    #[error("expected {expected} bytes but read {actual}")]
    SizeMismatch {
        /// Size the caller expected.
        expected: u64,
        /// Bytes actually read.
        actual: u64,
    },
    /// A stored digest has the wrong length for its algorithm.
    #[error("{algo} digest must be {expected} bytes, got {actual}")]
    InvalidLength {
        /// The digest's algorithm.
        algo: HashAlgo,
        /// Required length.
        expected: usize,
        /// Length found.
        actual: usize,
    },
    /// An algorithm name that is not one of [`HashAlgo::ALL`].
    #[error("unknown hash algorithm {0:?} (expected blake3, xxh3 or sha256)")]
    UnknownAlgo(String),
}

/// Hash of a file's whole content, tagged with its algorithm.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest {
    algo: HashAlgo,
    bytes: [u8; MAX_DIGEST_LEN],
}

impl Digest {
    /// Rebuilds a digest from stored bytes.
    ///
    /// # Errors
    ///
    /// [`HashError::InvalidLength`] if `bytes` does not have the algorithm's digest length.
    pub fn from_bytes(algo: HashAlgo, bytes: &[u8]) -> Result<Self, HashError> {
        if bytes.len() != algo.digest_len() {
            return Err(HashError::InvalidLength {
                algo,
                expected: algo.digest_len(),
                actual: bytes.len(),
            });
        }
        let mut buf = [0; MAX_DIGEST_LEN];
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(Self { algo, bytes: buf })
    }

    /// The algorithm that produced this digest.
    pub const fn algo(&self) -> HashAlgo {
        self.algo
    }

    /// The digest bytes, [`HashAlgo::digest_len`] long.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.algo.digest_len()]
    }

    /// Lowercase hexadecimal form, as printed by `b3sum` or `sha256sum`.
    pub fn to_hex(&self) -> String {
        use fmt::Write as _;
        self.as_bytes()
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                // Writing to a String cannot fail.
                let _ = write!(hex, "{byte:02x}");
                hex
            })
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algo, self.to_hex())
    }
}

/// Hash of a file's size and its first and last [`PARTIAL_BLOCK`] bytes.
///
/// Different partial digests prove two files differ; equal ones prove nothing. Files of at most
/// `2 * PARTIAL_BLOCK` bytes are hashed whole, but the result is still only a partial digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartialDigest(Digest);

impl PartialDigest {
    /// Rebuilds a partial digest from stored bytes.
    ///
    /// # Errors
    ///
    /// [`HashError::InvalidLength`] if `bytes` does not have the algorithm's digest length.
    pub fn from_bytes(algo: HashAlgo, bytes: &[u8]) -> Result<Self, HashError> {
        Digest::from_bytes(algo, bytes).map(Self)
    }

    /// The algorithm that produced this digest.
    pub const fn algo(&self) -> HashAlgo {
        self.0.algo
    }

    /// The digest bytes, [`HashAlgo::digest_len`] long.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Lowercase hexadecimal form.
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl fmt::Debug for PartialDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "partial {:?}", self.0)
    }
}

enum Hasher {
    Blake3(Box<blake3::Hasher>),
    Xxh3(Box<xxhash_rust::xxh3::Xxh3>),
    Sha256(sha2::Sha256),
}

impl Hasher {
    fn new(algo: HashAlgo) -> Self {
        match algo {
            HashAlgo::Blake3 => Self::Blake3(Box::new(blake3::Hasher::new())),
            HashAlgo::Xxh3 => Self::Xxh3(Box::new(xxhash_rust::xxh3::Xxh3::new())),
            HashAlgo::Sha256 => Self::Sha256(sha2::Sha256::new()),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Blake3(h) => {
                h.update(data);
            }
            Self::Xxh3(h) => h.update(data),
            Self::Sha256(h) => h.update(data),
        }
    }

    fn finalize(self) -> Digest {
        let (algo, out) = match self {
            Self::Blake3(h) => (HashAlgo::Blake3, *h.finalize().as_bytes()),
            Self::Xxh3(h) => {
                let mut out = [0; MAX_DIGEST_LEN];
                out[..16].copy_from_slice(&h.digest128().to_be_bytes());
                (HashAlgo::Xxh3, out)
            }
            Self::Sha256(h) => (HashAlgo::Sha256, h.finalize().into()),
        };
        Digest { algo, bytes: out }
    }
}

/// Hashes an in-memory buffer.
pub fn hash_bytes(algo: HashAlgo, data: &[u8]) -> Digest {
    let mut hasher = Hasher::new(algo);
    hasher.update(data);
    hasher.finalize()
}

/// Hashes everything `reader` yields, which must be exactly `expected_len` bytes.
///
/// `expected_len` is the size recorded when the file was listed. Requiring it catches files that
/// grew or shrank while being read and storage that silently returns short reads, so a digest
/// is only returned for content of the recorded size.
///
/// # Errors
///
/// - [`HashError::Io`] if reading fails.
/// - [`HashError::SizeMismatch`] if the reader yields more or fewer than `expected_len` bytes.
///
/// # Examples
///
/// ```
/// use symbolon_core::hash::{hash_reader, HashAlgo};
///
/// let digest = hash_reader(HashAlgo::Blake3, &b"abc"[..], 3)?;
/// assert_eq!(
///     digest.to_hex(),
///     "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
/// );
/// # Ok::<(), symbolon_core::hash::HashError>(())
/// ```
pub fn hash_reader(
    algo: HashAlgo,
    mut reader: impl Read,
    expected_len: u64,
) -> Result<Digest, HashError> {
    let mut hasher = Hasher::new(algo);
    let mut buf = vec![0; READ_BUFFER];
    let mut total: u64 = 0;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        hasher.update(&buf[..n]);
        total += n as u64;
        if total > expected_len {
            break;
        }
    }
    if total != expected_len {
        return Err(HashError::SizeMismatch {
            expected: expected_len,
            actual: total,
        });
    }
    Ok(hasher.finalize())
}

/// Computes the [`PartialDigest`] of a file of `size` bytes.
///
/// Hashes `size` (little-endian `u64`) followed by the first and last [`PARTIAL_BLOCK`] bytes,
/// or by the whole content when it is at most `2 * PARTIAL_BLOCK` bytes. Reads at most
/// `2 * PARTIAL_BLOCK` bytes regardless of the file size.
///
/// # Errors
///
/// - [`HashError::Io`] if reading or seeking fails.
/// - [`HashError::SizeMismatch`] if the content ends before `size` bytes.
pub fn partial_hash(
    algo: HashAlgo,
    mut reader: impl Read + Seek,
    size: u64,
) -> Result<PartialDigest, HashError> {
    let mut hasher = Hasher::new(algo);
    hasher.update(&size.to_le_bytes());
    if size <= 2 * PARTIAL_BLOCK {
        read_exact_into(&mut hasher, &mut reader, size, size)?;
    } else {
        read_exact_into(&mut hasher, &mut reader, PARTIAL_BLOCK, size)?;
        reader.seek(SeekFrom::Start(size - PARTIAL_BLOCK))?;
        read_exact_into(&mut hasher, &mut reader, PARTIAL_BLOCK, size)?;
    }
    Ok(PartialDigest(hasher.finalize()))
}

fn read_exact_into(
    hasher: &mut Hasher,
    reader: &mut impl Read,
    len: u64,
    size: u64,
) -> Result<(), HashError> {
    // At most 2 * PARTIAL_BLOCK, so the conversion cannot fail.
    let len = usize::try_from(len).map_err(|_| HashError::SizeMismatch {
        expected: size,
        actual: 0,
    })?;
    let mut buf = vec![0; len];
    reader.read_exact(&mut buf).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            HashError::SizeMismatch {
                expected: size,
                actual: 0,
            }
        } else {
            HashError::Io(e)
        }
    })?;
    hasher.update(&buf);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use proptest::prelude::*;

    use super::*;

    const BLOCK: usize = 64 * 1024;

    fn data(len: usize) -> Vec<u8> {
        (0..=250u8).cycle().step_by(31).take(len).collect()
    }

    fn reference(algo: HashAlgo, data: &[u8]) -> Vec<u8> {
        match algo {
            HashAlgo::Blake3 => blake3::hash(data).as_bytes().to_vec(),
            HashAlgo::Xxh3 => xxhash_rust::xxh3::xxh3_128(data).to_be_bytes().to_vec(),
            HashAlgo::Sha256 => sha2::Sha256::digest(data).to_vec(),
        }
    }

    #[test]
    fn known_answers_for_abc() {
        let expected = [
            (
                HashAlgo::Blake3,
                "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
            ),
            (HashAlgo::Xxh3, "06b05ab6733a618578af5f94892f3950"),
            (
                HashAlgo::Sha256,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
        ];
        for (algo, hex) in expected {
            assert_eq!(hash_bytes(algo, b"abc").to_hex(), hex, "{algo}");
            assert_eq!(hash_reader(algo, &b"abc"[..], 3).unwrap().to_hex(), hex);
        }
    }

    #[test]
    fn streaming_matches_one_shot_across_buffer_boundaries() {
        for algo in HashAlgo::ALL {
            for len in [
                0,
                1,
                READ_BUFFER - 1,
                READ_BUFFER,
                READ_BUFFER + 1,
                3 * READ_BUFFER + 7,
            ] {
                let input = data(len);
                let digest = hash_reader(algo, input.as_slice(), len as u64).unwrap();
                assert_eq!(
                    digest.as_bytes(),
                    reference(algo, &input),
                    "{algo} len {len}"
                );
                assert_eq!(digest.algo(), algo);
            }
        }
    }

    #[test]
    fn short_and_long_content_is_an_error_not_a_digest() {
        let input = data(1000);
        assert!(matches!(
            hash_reader(HashAlgo::Blake3, input.as_slice(), 1001),
            Err(HashError::SizeMismatch {
                expected: 1001,
                actual: 1000
            })
        ));
        assert!(matches!(
            hash_reader(HashAlgo::Blake3, input.as_slice(), 999),
            Err(HashError::SizeMismatch { expected: 999, .. })
        ));
    }

    struct FailingReader {
        remaining: usize,
    }

    impl Read for FailingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("device vanished"));
            }
            let n = buf.len().min(self.remaining);
            buf[..n].fill(7);
            self.remaining -= n;
            Ok(n)
        }
    }

    #[test]
    fn read_error_midway_is_returned() {
        let result = hash_reader(HashAlgo::Sha256, FailingReader { remaining: 5000 }, 10_000);
        assert!(matches!(result, Err(HashError::Io(e)) if e.to_string() == "device vanished"));
    }

    #[test]
    fn digests_of_different_algorithms_never_compare_equal() {
        let bytes = [9; 32];
        let a = Digest::from_bytes(HashAlgo::Blake3, &bytes).unwrap();
        let b = Digest::from_bytes(HashAlgo::Sha256, &bytes).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert_ne!(a, b);
    }

    #[test]
    fn from_bytes_round_trips_and_checks_length() {
        for algo in HashAlgo::ALL {
            let digest = hash_bytes(algo, b"round trip");
            assert_eq!(Digest::from_bytes(algo, digest.as_bytes()).unwrap(), digest);
        }
        assert!(matches!(
            Digest::from_bytes(HashAlgo::Xxh3, &[0; 32]),
            Err(HashError::InvalidLength {
                expected: 16,
                actual: 32,
                ..
            })
        ));
    }

    #[test]
    fn algo_names_round_trip() {
        for algo in HashAlgo::ALL {
            assert_eq!(algo.name().parse::<HashAlgo>().unwrap(), algo);
            assert_eq!(algo.to_string(), algo.name());
        }
        assert!(
            matches!("md5".parse::<HashAlgo>(), Err(HashError::UnknownAlgo(name)) if name == "md5")
        );
        assert_eq!(HashAlgo::default(), HashAlgo::Blake3);
    }

    #[test]
    fn block_constant_matches() {
        assert_eq!(BLOCK as u64, PARTIAL_BLOCK);
    }

    #[test]
    fn partial_hash_of_small_file_covers_whole_content() {
        let len = 2 * BLOCK;
        let mut a = data(len);
        let pa = partial_hash(HashAlgo::Blake3, Cursor::new(&a), len as u64).unwrap();
        a[len / 2] ^= 1;
        let pb = partial_hash(HashAlgo::Blake3, Cursor::new(&a), len as u64).unwrap();
        assert_ne!(pa, pb);
    }

    #[test]
    fn partial_hash_of_large_file_ignores_the_middle_only() {
        let len = 2 * BLOCK + 1000;
        let base = data(len);
        let partial = |bytes: &[u8]| {
            partial_hash(HashAlgo::Xxh3, Cursor::new(bytes), bytes.len() as u64).unwrap()
        };

        let mut middle = base.clone();
        middle[BLOCK + 500] ^= 1;
        assert_eq!(partial(&base), partial(&middle));

        let mut head = base.clone();
        head[0] ^= 1;
        assert_ne!(partial(&base), partial(&head));

        let mut tail = base.clone();
        tail[len - 1] ^= 1;
        assert_ne!(partial(&base), partial(&tail));
    }

    #[test]
    fn partial_hash_includes_the_size() {
        let a = data(10);
        let pa = partial_hash(HashAlgo::Blake3, Cursor::new(&a), 10).unwrap();
        let mut b = a.clone();
        b.push(0);
        let pb = partial_hash(HashAlgo::Blake3, Cursor::new(&b), 11).unwrap();
        assert_ne!(pa, pb);
    }

    #[test]
    fn partial_hash_of_truncated_content_is_an_error() {
        let a = data(100);
        assert!(matches!(
            partial_hash(HashAlgo::Blake3, Cursor::new(&a), 200),
            Err(HashError::SizeMismatch { expected: 200, .. })
        ));
    }

    proptest! {
        #[test]
        fn chunking_does_not_change_the_digest(
            input in proptest::collection::vec(any::<u8>(), 0..20_000),
            chunk in 1usize..5000,
        ) {
            struct Chunked<'a> { data: &'a [u8], chunk: usize }
            impl Read for Chunked<'_> {
                fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                    let n = self.chunk.min(buf.len()).min(self.data.len());
                    buf[..n].copy_from_slice(&self.data[..n]);
                    self.data = &self.data[n..];
                    Ok(n)
                }
            }
            for algo in HashAlgo::ALL {
                let streamed = hash_reader(algo, Chunked { data: &input, chunk }, input.len() as u64).unwrap();
                prop_assert_eq!(streamed, hash_bytes(algo, &input));
            }
        }
    }
}
