//! Lossless relative paths and the normalized keys used to match them (ADR-0004).
//!
//! A [`RelPath`] is a path relative to a location's root, stored exactly as the filesystem
//! reports it: raw bytes on Unix-like systems ([`PathEncoding::Posix`]), and WTF-8 encoded
//! UTF-16 on Windows ([`PathEncoding::Wtf8`]). Components are joined with `/`. Converting
//! never loses information.
//!
//! A [`PathKey`] is the matching form: valid Unicode stays as is, and every byte that is not
//! (invalid UTF-8 on Unix, unpaired surrogates on Windows) plus `%` itself is written as `%XX`.
//! The mapping is injective, and a name that is valid Unicode gets the same key on every
//! platform. [`KeyOptions`] adds Unicode NFC normalization and case-insensitive matching on top,
//! per comparison.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use unicode_normalization::UnicodeNormalization as _;

/// How a [`RelPath`]'s bytes encode the filesystem's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathEncoding {
    /// Raw bytes, as on Linux, Android and other Unix-like systems.
    Posix,
    /// UTF-16 code units (possibly with unpaired surrogates) encoded as WTF-8, as on Windows.
    Wtf8,
}

impl PathEncoding {
    /// The encoding used by paths on the platform this code runs on.
    pub const fn native() -> Self {
        if cfg!(windows) {
            Self::Wtf8
        } else {
            Self::Posix
        }
    }

    /// The stable name stored in the catalog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Posix => "posix",
            Self::Wtf8 => "wtf8",
        }
    }
}

impl fmt::Display for PathEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for PathEncoding {
    type Err = PathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "posix" => Ok(Self::Posix),
            "wtf8" => Ok(Self::Wtf8),
            _ => Err(PathError::UnknownEncoding(s.to_owned())),
        }
    }
}

/// Errors from building or converting relative paths.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// The path has a root or prefix; only paths relative to a location root are allowed.
    #[error("path is absolute; expected a path relative to the location root")]
    Absolute,
    /// The path contains `..`, which could escape the location root.
    #[error("path contains `..`")]
    ParentComponent,
    /// Raw bytes that are not a valid relative path in their encoding (empty, `.` or `..`
    /// components, leading or trailing `/`, NUL, or invalid WTF-8).
    #[error("invalid {encoding} relative path")]
    Malformed {
        /// The encoding the bytes claimed to be in.
        encoding: PathEncoding,
    },
    /// A path in one platform's encoding cannot be turned into a native path on another.
    #[error("a {encoding} path cannot be used as a native path on this platform")]
    ForeignEncoding {
        /// The path's encoding.
        encoding: PathEncoding,
    },
    /// An encoding name that is not `posix` or `wtf8`.
    #[error("unknown path encoding {0:?} (expected posix or wtf8)")]
    UnknownEncoding(String),
}

/// A path relative to a location root, stored losslessly.
///
/// The empty path is the root itself. Ordering compares raw bytes, which groups a directory's
/// descendants together.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelPath {
    encoding: PathEncoding,
    bytes: Vec<u8>,
}

impl RelPath {
    /// The location root itself.
    pub const fn root(encoding: PathEncoding) -> Self {
        Self {
            encoding,
            bytes: Vec::new(),
        }
    }

    /// Converts a relative native path, in this platform's [`PathEncoding::native`].
    ///
    /// `.` components are dropped.
    ///
    /// # Errors
    ///
    /// [`PathError::Absolute`] for paths with a root or prefix, [`PathError::ParentComponent`]
    /// for paths containing `..`.
    ///
    /// # Examples
    ///
    /// ```
    /// use symbolon_core::pathkey::RelPath;
    ///
    /// let path = RelPath::from_path("photos/./2024/a.jpg".as_ref())?;
    /// assert_eq!(path.key().as_str(), "photos/2024/a.jpg");
    /// assert!(RelPath::from_path("../etc/passwd".as_ref()).is_err());
    /// # Ok::<(), symbolon_core::pathkey::PathError>(())
    /// ```
    pub fn from_path(path: &Path) -> Result<Self, PathError> {
        let mut out = Self::root(PathEncoding::native());
        for component in path.components() {
            match component {
                Component::Normal(name) => {
                    if !out.bytes.is_empty() {
                        out.bytes.push(b'/');
                    }
                    push_native(&mut out.bytes, name);
                }
                Component::CurDir => {}
                Component::ParentDir => return Err(PathError::ParentComponent),
                Component::RootDir | Component::Prefix(_) => return Err(PathError::Absolute),
            }
        }
        Ok(out)
    }

    /// Rebuilds a path from stored bytes, validating them.
    ///
    /// # Errors
    ///
    /// [`PathError::Malformed`] if the bytes are not a valid relative path in `encoding`.
    pub fn from_raw(encoding: PathEncoding, bytes: Vec<u8>) -> Result<Self, PathError> {
        let malformed = PathError::Malformed { encoding };
        if !bytes.is_empty()
            && bytes
                .split(|&b| b == b'/')
                .any(|c| c.is_empty() || c == b"." || c == b".." || c.contains(&0))
        {
            return Err(malformed);
        }
        if encoding == PathEncoding::Wtf8 && wtf8_units(&bytes).is_none() {
            return Err(malformed);
        }
        Ok(Self { encoding, bytes })
    }

    /// The raw bytes, as stored in the catalog.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The encoding of [`RelPath::as_bytes`].
    pub const fn encoding(&self) -> PathEncoding {
        self.encoding
    }

    /// Whether this is the location root.
    pub const fn is_root(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The child `name` of this path, in this platform's native encoding.
    ///
    /// # Errors
    ///
    /// [`PathError::Malformed`] if `name` is empty, `.`, `..` or contains a separator, or
    /// [`PathError::ForeignEncoding`] if this path is not in the native encoding.
    pub fn join(&self, name: &OsStr) -> Result<Self, PathError> {
        if self.encoding != PathEncoding::native() {
            return Err(PathError::ForeignEncoding {
                encoding: self.encoding,
            });
        }
        let mut child = Vec::new();
        push_native(&mut child, name);
        if child.is_empty()
            || child == b"."
            || child == b".."
            || child.contains(&b'/')
            || (cfg!(windows) && child.contains(&b'\\'))
        {
            return Err(PathError::Malformed {
                encoding: self.encoding,
            });
        }
        let mut bytes = self.bytes.clone();
        if !bytes.is_empty() {
            bytes.push(b'/');
        }
        bytes.extend_from_slice(&child);
        Ok(Self {
            encoding: self.encoding,
            bytes,
        })
    }

    /// This path under `root` as a native path.
    ///
    /// # Errors
    ///
    /// [`PathError::ForeignEncoding`] if the path was recorded on a platform with a different
    /// encoding (for example a Windows path on Linux).
    pub fn to_native(&self, root: &Path) -> Result<PathBuf, PathError> {
        let foreign = PathError::ForeignEncoding {
            encoding: self.encoding,
        };
        if self.encoding != PathEncoding::native() {
            return Err(foreign);
        }
        if self.bytes.is_empty() {
            return Ok(root.to_path_buf());
        }
        native_from_bytes(&self.bytes)
            .map(|rel| root.join(rel))
            .ok_or(foreign)
    }

    /// The matching key; see [`PathKey`].
    pub fn key(&self) -> PathKey {
        let mut key = String::with_capacity(self.bytes.len());
        match self.encoding {
            PathEncoding::Posix => {
                for chunk in self.bytes.utf8_chunks() {
                    push_escaped_text(&mut key, chunk.valid());
                    for &byte in chunk.invalid() {
                        push_escaped_byte(&mut key, byte);
                    }
                }
            }
            PathEncoding::Wtf8 => {
                for unit in wtf8_units(&self.bytes).unwrap_or_default() {
                    match unit {
                        Wtf8Unit::Char(c) => {
                            push_escaped_text(&mut key, c.encode_utf8(&mut [0; 4]));
                        }
                        Wtf8Unit::Surrogate(s) => {
                            for byte in encode_surrogate(s) {
                                push_escaped_byte(&mut key, byte);
                            }
                        }
                    }
                }
            }
        }
        PathKey(key)
    }
}

impl fmt::Debug for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:?}", self.encoding, self.key().as_str())
    }
}

impl fmt::Display for RelPath {
    /// Human-readable form; non-Unicode bytes appear as `%XX`, like in the key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key().as_str())
    }
}

/// The matching key of a [`RelPath`]: always valid UTF-8, `/`-separated, injective.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PathKey(String);

impl PathKey {
    /// Wraps a key read from the catalog. It is not re-validated.
    pub fn from_stored(key: String) -> Self {
        Self(key)
    }

    /// The key as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key of the parent directory; empty for entries directly under the root.
    pub fn parent(&self) -> &str {
        self.0.rsplit_once('/').map_or("", |(parent, _)| parent)
    }

    /// The last component.
    pub fn file_name(&self) -> &str {
        self.0.rsplit_once('/').map_or(&self.0, |(_, name)| name)
    }

    /// The key transformed by `options`, for matching within one comparison.
    ///
    /// Escapes (`%XX`) are kept verbatim; only the text between them is normalized.
    ///
    /// # Examples
    ///
    /// ```
    /// use symbolon_core::pathkey::{KeyOptions, PathKey};
    ///
    /// let nfd = PathKey::from_stored("Cafe\u{301}/README.md".into());
    /// let options = KeyOptions { nfc: true, case_insensitive: true };
    /// assert_eq!(nfd.normalized(options), "café/readme.md");
    /// ```
    pub fn normalized(&self, options: KeyOptions) -> String {
        if !options.nfc && !options.case_insensitive {
            return self.0.clone();
        }
        let mut out = String::with_capacity(self.0.len());
        let mut rest = self.0.as_str();
        while !rest.is_empty() {
            let text_end = rest.find('%').unwrap_or(rest.len());
            let (text, tail) = rest.split_at(text_end);
            push_normalized(&mut out, text, options);
            // An escape is always `%` and two hex digits.
            let escape_end = tail.len().min(3);
            out.push_str(&tail[..escape_end]);
            rest = &tail[escape_end..];
        }
        out
    }
}

impl fmt::Display for PathKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How path keys are compared in one comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyOptions {
    /// Treat canonically equivalent Unicode names (NFC and NFD, as written by macOS) as equal.
    pub nfc: bool,
    /// Ignore letter case, using simple one-to-one case folding as case-insensitive filesystems
    /// do (so `Straße` and `STRASSE` stay different).
    pub case_insensitive: bool,
}

/// Groups `paths` whose keys become equal under `options`, keeping only groups of two or more.
///
/// Such paths cannot be told apart in a comparison with these options and are reported as a
/// collision instead of being merged.
pub fn collisions<'a>(
    paths: impl IntoIterator<Item = &'a RelPath>,
    options: KeyOptions,
) -> Vec<Vec<&'a RelPath>> {
    let mut groups: BTreeMap<String, Vec<&RelPath>> = BTreeMap::new();
    for path in paths {
        groups
            .entry(path.key().normalized(options))
            .or_default()
            .push(path);
    }
    groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

fn push_normalized(out: &mut String, text: &str, options: KeyOptions) {
    let fold = |c: char| {
        if options.case_insensitive {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(single), None) => single,
                _ => c,
            }
        } else {
            c
        }
    };
    if options.nfc {
        out.extend(text.nfc().map(fold));
    } else {
        out.extend(text.chars().map(fold));
    }
}

fn push_escaped_text(key: &mut String, text: &str) {
    for c in text.chars() {
        if c == '%' {
            key.push_str("%25");
        } else {
            key.push(c);
        }
    }
}

fn push_escaped_byte(key: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    key.push('%');
    key.push(char::from(HEX[usize::from(byte >> 4)]));
    key.push(char::from(HEX[usize::from(byte & 0xF)]));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wtf8Unit {
    Char(char),
    Surrogate(u16),
}

fn encode_surrogate(s: u16) -> [u8; 3] {
    // Generalized UTF-8 for a code point in U+D800..=U+DFFF; the casts keep the low bits.
    [
        0xE0 | (s >> 12) as u8,
        0x80 | ((s >> 6) & 0x3F) as u8,
        0x80 | (s & 0x3F) as u8,
    ]
}

/// Encodes UTF-16 code units as WTF-8.
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "used on Windows and in tests")
)]
fn wtf8_encode(units: impl IntoIterator<Item = u16>, out: &mut Vec<u8>) {
    for decoded in char::decode_utf16(units) {
        match decoded {
            Ok(c) => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            Err(e) => out.extend_from_slice(&encode_surrogate(e.unpaired_surrogate())),
        }
    }
}

/// Decodes WTF-8, or `None` if the bytes are not well-formed WTF-8 (including a surrogate pair
/// written as two separate surrogates, which WTF-8 forbids).
fn wtf8_units(bytes: &[u8]) -> Option<Vec<Wtf8Unit>> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while !rest.is_empty() {
        if rest[0] == 0xED && rest.len() >= 3 && (0xA0..=0xBF).contains(&rest[1]) {
            if !(0x80..=0xBF).contains(&rest[2]) {
                return None;
            }
            let s = 0xD000 | (u16::from(rest[1] & 0x3F) << 6) | u16::from(rest[2] & 0x3F);
            let follows_lead = matches!(units.last(), Some(Wtf8Unit::Surrogate(prev)) if (0xD800..0xDC00).contains(prev));
            if (0xDC00..0xE000).contains(&s) && follows_lead {
                return None;
            }
            units.push(Wtf8Unit::Surrogate(s));
            rest = &rest[3..];
            continue;
        }
        let len = match rest[0] {
            0x00..=0x7F => 1,
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => return None,
        };
        let c = rest
            .get(..len)
            .and_then(|seq| std::str::from_utf8(seq).ok())
            .and_then(|s| s.chars().next())?;
        units.push(Wtf8Unit::Char(c));
        rest = &rest[len..];
    }
    Some(units)
}

#[cfg(unix)]
fn push_native(out: &mut Vec<u8>, name: &OsStr) {
    use std::os::unix::ffi::OsStrExt as _;
    out.extend_from_slice(name.as_bytes());
}

#[cfg(windows)]
fn push_native(out: &mut Vec<u8>, name: &OsStr) {
    use std::os::windows::ffi::OsStrExt as _;
    wtf8_encode(name.encode_wide(), out);
}

#[cfg(not(any(unix, windows)))]
fn push_native(out: &mut Vec<u8>, name: &OsStr) {
    out.extend_from_slice(name.to_string_lossy().as_bytes());
}

#[cfg(unix)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "same signature as on Windows, where decoding can fail"
)]
fn native_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    Some(PathBuf::from(OsStr::from_bytes(bytes)))
}

#[cfg(windows)]
fn native_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt as _;
    let mut wide = Vec::with_capacity(bytes.len());
    for unit in wtf8_units(bytes)? {
        match unit {
            Wtf8Unit::Char('/') => wide.push(u16::from(b'\\')),
            Wtf8Unit::Char(c) => wide.extend_from_slice(c.encode_utf16(&mut [0; 2])),
            Wtf8Unit::Surrogate(s) => wide.push(s),
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

#[cfg(not(any(unix, windows)))]
fn native_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    std::str::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn posix(bytes: &[u8]) -> RelPath {
        RelPath::from_raw(PathEncoding::Posix, bytes.to_vec()).unwrap()
    }

    fn wtf8(components: &[&[u16]]) -> RelPath {
        let mut bytes = Vec::new();
        for (i, units) in components.iter().enumerate() {
            if i > 0 {
                bytes.push(b'/');
            }
            wtf8_encode(units.iter().copied(), &mut bytes);
        }
        RelPath::from_raw(PathEncoding::Wtf8, bytes).unwrap()
    }

    /// Inverse of `RelPath::key` for posix paths; proves the key is injective.
    fn decode_key(key: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chars = key.char_indices();
        while let Some((i, c)) = chars.next() {
            if c == '%' {
                out.push(u8::from_str_radix(&key[i + 1..i + 3], 16).unwrap());
                chars.next();
                chars.next();
            } else {
                out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
            }
        }
        out
    }

    #[test]
    fn unicode_names_keep_their_text() {
        assert_eq!(
            posix("zdjęcia/żółw.jpg".as_bytes()).key().as_str(),
            "zdjęcia/żółw.jpg"
        );
    }

    #[test]
    fn invalid_bytes_and_percent_are_escaped() {
        assert_eq!(posix(b"a\xffb/100%").key().as_str(), "a%FFb/100%25");
        assert_eq!(posix(b"%FF").key().as_str(), "%25FF");
        assert_ne!(posix(b"\xff").key(), posix(b"%FF").key());
    }

    #[test]
    fn same_unicode_name_has_same_key_on_both_platforms() {
        let name = "Café/naïve ✓.txt";
        let units: Vec<Vec<u16>> = name
            .split('/')
            .map(|c| c.encode_utf16().collect())
            .collect();
        let parts: Vec<&[u16]> = units.iter().map(Vec::as_slice).collect();
        assert_eq!(wtf8(&parts).key(), posix(name.as_bytes()).key());
        assert_eq!(wtf8(&parts).as_bytes(), name.as_bytes());
    }

    #[test]
    fn unpaired_surrogates_are_escaped() {
        let key = wtf8(&[&[u16::from(b'a'), 0xD800, u16::from(b'b')]]).key();
        assert_eq!(key.as_str(), "a%ED%A0%80b");
    }

    #[test]
    fn wtf8_rejects_split_surrogate_pairs_and_garbage() {
        // U+1F600 written as two 3-byte surrogates instead of one 4-byte sequence.
        let split = vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80];
        assert!(RelPath::from_raw(PathEncoding::Wtf8, split).is_err());
        assert!(RelPath::from_raw(PathEncoding::Wtf8, vec![0xFF]).is_err());
        assert!(RelPath::from_raw(PathEncoding::Wtf8, "😀".as_bytes().to_vec()).is_ok());
    }

    #[test]
    fn from_path_rejects_escapes_and_absolute_paths() {
        assert_eq!(
            RelPath::from_path("a/../b".as_ref()),
            Err(PathError::ParentComponent)
        );
        assert_eq!(
            RelPath::from_path("/etc".as_ref()),
            Err(PathError::Absolute)
        );
        assert!(RelPath::from_path("".as_ref()).unwrap().is_root());
        assert_eq!(
            RelPath::from_path("./a/./b".as_ref())
                .unwrap()
                .key()
                .as_str(),
            "a/b"
        );
    }

    #[test]
    fn from_raw_rejects_malformed_paths() {
        for bad in [&b"/a"[..], b"a/", b"a//b", b"./a", b"a/../b", b"a\0b"] {
            assert!(
                RelPath::from_raw(PathEncoding::Posix, bad.to_vec()).is_err(),
                "{bad:?}"
            );
        }
        assert!(
            RelPath::from_raw(PathEncoding::Posix, Vec::new())
                .unwrap()
                .is_root()
        );
    }

    #[test]
    fn join_builds_children_and_rejects_bad_names() {
        let root = RelPath::root(PathEncoding::native());
        let child = root
            .join("a".as_ref())
            .unwrap()
            .join("b c".as_ref())
            .unwrap();
        assert_eq!(child.key().as_str(), "a/b c");
        for bad in ["", ".", "..", "x/y"] {
            assert!(root.join(bad.as_ref()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn to_native_round_trips_and_refuses_foreign_encodings() {
        let rel = RelPath::from_path("dir/file.txt".as_ref()).unwrap();
        let native = rel.to_native("/root".as_ref()).unwrap();
        assert_eq!(native, Path::new("/root").join("dir").join("file.txt"));
        let foreign = match PathEncoding::native() {
            PathEncoding::Posix => PathEncoding::Wtf8,
            PathEncoding::Wtf8 => PathEncoding::Posix,
        };
        let other = RelPath::from_raw(foreign, b"dir".to_vec()).unwrap();
        assert!(matches!(
            other.to_native("/root".as_ref()),
            Err(PathError::ForeignEncoding { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_unix_names_round_trip_through_native_paths() {
        use std::os::unix::ffi::OsStrExt as _;
        let name = OsStr::from_bytes(b"bad\xffname");
        let rel = RelPath::from_path(Path::new(name)).unwrap();
        assert_eq!(rel.as_bytes(), b"bad\xffname");
        assert_eq!(rel.to_native("".as_ref()).unwrap().as_os_str(), name);
    }

    #[test]
    fn parent_and_file_name() {
        let key = posix(b"a/b/c.txt").key();
        assert_eq!(key.parent(), "a/b");
        assert_eq!(key.file_name(), "c.txt");
        let top = posix(b"top").key();
        assert_eq!(top.parent(), "");
        assert_eq!(top.file_name(), "top");
    }

    #[test]
    fn normalization_options() {
        let nfd = posix("Cafe\u{301}.TXT".as_bytes()).key();
        let nfc = posix("Café.txt".as_bytes()).key();
        assert_ne!(
            nfd.normalized(KeyOptions::default()),
            nfc.normalized(KeyOptions::default())
        );
        let both = KeyOptions {
            nfc: true,
            case_insensitive: true,
        };
        assert_eq!(nfd.normalized(both), nfc.normalized(both));
        // Simple folding: ß has no single-character lowercase-equivalent for "SS".
        let ci = KeyOptions {
            case_insensitive: true,
            ..KeyOptions::default()
        };
        assert_ne!(
            posix("Straße".as_bytes()).key().normalized(ci),
            posix(b"STRASSE").key().normalized(ci)
        );
        assert_eq!(posix(b"README").key().normalized(ci), "readme");
    }

    #[test]
    fn normalization_keeps_escapes_verbatim() {
        let ci = KeyOptions {
            case_insensitive: true,
            ..KeyOptions::default()
        };
        assert_eq!(posix(b"A\xffB").key().normalized(ci), "a%FFb");
    }

    #[test]
    fn collisions_are_grouped() {
        let paths = [posix(b"Readme.md"), posix(b"README.md"), posix(b"other")];
        let ci = KeyOptions {
            case_insensitive: true,
            ..KeyOptions::default()
        };
        let groups = collisions(&paths, ci);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(
            collisions(&paths, KeyOptions::default()),
            Vec::<Vec<&RelPath>>::new()
        );
    }

    #[test]
    fn encoding_names_round_trip() {
        for encoding in [PathEncoding::Posix, PathEncoding::Wtf8] {
            assert_eq!(encoding.name().parse::<PathEncoding>().unwrap(), encoding);
        }
        assert!("utf16".parse::<PathEncoding>().is_err());
    }

    fn component() -> impl Strategy<Value = Vec<u8>> {
        proptest::collection::vec(
            any::<u8>().prop_filter("no separator or NUL", |b| *b != b'/' && *b != 0),
            1..12,
        )
        .prop_filter("not . or ..", |c| c != b"." && c != b"..")
    }

    fn wide_component() -> impl Strategy<Value = Vec<u16>> {
        proptest::collection::vec(
            any::<u16>().prop_filter("no separator or NUL", |u| *u != u16::from(b'/') && *u != 0),
            1..12,
        )
        .prop_filter("not . or ..", |c| c != &[46] && c != &[46, 46])
    }

    proptest! {
        #[test]
        fn posix_keys_are_injective(parts in proptest::collection::vec(component(), 0..5)) {
            let raw = parts.join(&b'/');
            let path = RelPath::from_raw(PathEncoding::Posix, raw.clone()).unwrap();
            prop_assert_eq!(decode_key(path.key().as_str()), raw);
        }

        #[test]
        fn wtf8_round_trips_utf16(units in wide_component()) {
            let mut bytes = Vec::new();
            wtf8_encode(units.iter().copied(), &mut bytes);
            let path = RelPath::from_raw(PathEncoding::Wtf8, bytes).unwrap();
            let decoded: Vec<u16> = wtf8_units(path.as_bytes()).unwrap().into_iter().flat_map(|unit| match unit {
                Wtf8Unit::Char(c) => c.encode_utf16(&mut [0; 2]).to_vec(),
                Wtf8Unit::Surrogate(s) => vec![s],
            }).collect();
            prop_assert_eq!(decoded, units);
        }

        #[test]
        fn valid_unicode_keys_match_across_encodings(name in "[^/\u{0}]{1,16}") {
            prop_assume!(name != "." && name != "..");
            let p = RelPath::from_raw(PathEncoding::Posix, name.as_bytes().to_vec()).unwrap();
            let w = wtf8(&[&name.encode_utf16().collect::<Vec<_>>()]);
            prop_assert_eq!(p.key(), w.key());
        }
    }
}
