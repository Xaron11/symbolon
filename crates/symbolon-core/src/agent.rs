//! The protocol between the host and the on-device agent (ADR-0017): one JSON object per
//! line. Both sides use these types, so the host reads exactly what the agent writes.
//!
//! Every run starts with [`Line::Hello`] and ends with [`Line::Done`], or [`Line::Error`] when
//! the run could not do its work at all. Paths are relative to the root, written as POSIX path
//! keys (`%XX` for `%` and bytes that are not UTF-8), which is lossless.

use serde::{Deserialize, Serialize};

use crate::hash::{Digest, HashAlgo, PartialDigest};
use crate::pathkey::{self, PathEncoding, RelPath};
use crate::source::{EntryKind, HashReply, HashRequest, SourceEntry, SourceWarning, WarningKind};

/// The protocol version this build speaks. The host refuses an agent that speaks another.
pub const PROTOCOL: u32 = 1;

/// One line of agent output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Line {
    /// First line of every run.
    Hello {
        /// [`PROTOCOL`] of the agent.
        protocol: u32,
        /// The agent's version.
        version: String,
        /// The device's CPU architecture, as Rust names it (`aarch64`).
        arch: String,
        /// CPUs available to the agent.
        cpus: usize,
    },
    /// An entry found by `walk`.
    Entry {
        /// Path key relative to the root.
        path: String,
        /// `file`, `dir`, `symlink` or `other`.
        kind: String,
        /// Size in bytes.
        size: u64,
        /// Modification time in nanoseconds since the Unix epoch.
        mtime_ns: Option<i64>,
        /// A symlink's target, escaped like a path; absent when unknown (ADR-0019).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    /// A problem with one entry or request.
    Warning {
        /// For `hash`: the request it answers.
        index: Option<usize>,
        /// The entry, if known.
        path: Option<String>,
        /// [`WarningKind`] name.
        kind: String,
        /// What happened.
        message: String,
    },
    /// A digest answering request `index` of `hash`.
    Digest {
        /// The request it answers.
        index: usize,
        /// A partial digest rather than a full one.
        partial: bool,
        /// Lowercase hex.
        hex: String,
    },
    /// The run finished; `count` entries or requests were handled.
    Done {
        /// Entries listed or requests answered.
        count: u64,
    },
    /// The run could not do its work (for example the root is unreadable).
    Error {
        /// What went wrong.
        message: String,
    },
}

/// One request in the file `hash` reads, one per line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Path key relative to the root.
    pub path: String,
    /// Size when listed.
    pub size: u64,
    /// Modification time when listed.
    pub mtime_ns: Option<i64>,
    /// A partial digest instead of the full content.
    pub partial: bool,
}

impl Line {
    /// The `hello` line of this build.
    pub fn hello() -> Self {
        Self::Hello {
            protocol: PROTOCOL,
            version: crate::VERSION.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            cpus: std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        }
    }

    /// The line describing a walked entry.
    pub fn entry(entry: &SourceEntry) -> Self {
        Self::Entry {
            path: path_text(&entry.path),
            kind: entry.kind.name().to_owned(),
            size: entry.size,
            mtime_ns: entry.mtime_ns,
            target: entry.link_target.as_deref().map(pathkey::escape_raw),
        }
    }

    /// The line describing a warning, optionally answering request `index`.
    pub fn warning(index: Option<usize>, warning: &SourceWarning) -> Self {
        Self::Warning {
            index,
            path: warning.path.as_ref().map(path_text),
            kind: warning.kind.name().to_owned(),
            message: warning.message.clone(),
        }
    }

    /// The line answering request `index` with `reply`.
    pub fn reply(index: usize, reply: &HashReply) -> Self {
        match reply {
            HashReply::Full(digest) => Self::Digest {
                index,
                partial: false,
                hex: digest.to_hex(),
            },
            HashReply::Partial(digest) => Self::Digest {
                index,
                partial: true,
                hex: digest.to_hex(),
            },
            HashReply::Warning(warning) => Self::warning(Some(index), warning),
        }
    }

    /// The entry this line describes, if it is a valid `entry` line.
    pub fn to_entry(&self) -> Option<SourceEntry> {
        let Self::Entry {
            path,
            kind,
            size,
            mtime_ns,
            target,
        } = self
        else {
            return None;
        };
        let kind = kind.parse().ok()?;
        let link_target = match target {
            Some(text) if kind == EntryKind::Symlink => Some(
                pathkey::raw_from_escaped(text)
                    .filter(|raw| pathkey::valid_link_target(PathEncoding::Posix, raw))?,
            ),
            _ => None,
        };
        Some(SourceEntry {
            path: path_from_text(path)?,
            kind,
            size: *size,
            mtime_ns: *mtime_ns,
            link_target,
        })
    }

    /// The warning this line describes, if it is a `warning` line.
    pub fn to_warning(&self) -> Option<SourceWarning> {
        let Self::Warning {
            path,
            kind,
            message,
            ..
        } = self
        else {
            return None;
        };
        Some(SourceWarning {
            path: path.as_deref().and_then(path_from_text),
            kind: kind.parse().unwrap_or(WarningKind::Io),
            message: message.clone(),
        })
    }

    /// For a `digest` or `warning` line answering a request: its index and the reply, read as
    /// `algo`. A digest that does not decode is reported as a warning.
    pub fn to_reply(&self, algo: HashAlgo) -> Option<(usize, HashReply)> {
        match self {
            Self::Digest {
                index,
                partial,
                hex,
            } => {
                let digest = decode_hex(hex).and_then(|bytes| {
                    if *partial {
                        PartialDigest::from_bytes(algo, &bytes)
                            .ok()
                            .map(HashReply::Partial)
                    } else {
                        Digest::from_bytes(algo, &bytes).ok().map(HashReply::Full)
                    }
                });
                Some((
                    *index,
                    digest.unwrap_or_else(|| {
                        HashReply::Warning(SourceWarning {
                            path: None,
                            kind: WarningKind::Io,
                            message: format!("the agent sent an invalid {algo} digest"),
                        })
                    }),
                ))
            }
            Self::Warning {
                index: Some(index), ..
            } => Some((*index, HashReply::Warning(self.to_warning()?))),
            _ => None,
        }
    }
}

impl Request {
    /// The request for `request`.
    pub fn new(request: &HashRequest) -> Self {
        Self {
            path: path_text(&request.path),
            size: request.size,
            mtime_ns: request.mtime_ns,
            partial: request.partial,
        }
    }

    /// The hashing request this line asks for, if its path is valid.
    pub fn to_hash_request(&self) -> Option<HashRequest> {
        Some(HashRequest {
            path: path_from_text(&self.path)?,
            size: self.size,
            mtime_ns: self.mtime_ns,
            partial: self.partial,
        })
    }
}

/// A relative path as the protocol writes it: its POSIX path key.
pub fn path_text(path: &RelPath) -> String {
    path.key().as_str().to_owned()
}

/// Reads a path written by [`path_text`]; `None` if it is not a valid relative POSIX path.
pub fn path_from_text(text: &str) -> Option<RelPath> {
    RelPath::from_raw(PathEncoding::Posix, pathkey::raw_from_escaped(text)?).ok()
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_reader;
    use crate::source::EntryKind;

    fn posix(bytes: &[u8]) -> RelPath {
        RelPath::from_raw(PathEncoding::Posix, bytes.to_vec()).unwrap()
    }

    #[test]
    fn paths_survive_the_protocol_byte_for_byte() {
        for raw in [
            &b"a/b c.jpg"[..],
            b"100%/x",
            b"bad\xff/\xfe",
            b"\xe2\x82\xac",
        ] {
            let text = path_text(&posix(raw));
            assert_eq!(path_from_text(&text).unwrap().as_bytes(), raw, "{text}");
        }
        assert_eq!(path_text(&posix(b"100%/x")), "100%25/x");
        for bad in ["../x", "/abs", "a//b", "%zz", "%4"] {
            assert!(path_from_text(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn lines_round_trip_through_json() {
        let entry = SourceEntry {
            path: posix(b"DCIM/a\xff.jpg"),
            kind: EntryKind::File,
            size: 4,
            mtime_ns: Some(-5),
            link_target: None,
        };
        let line = Line::entry(&entry);
        let json = serde_json::to_string(&line).unwrap();
        assert!(
            json.starts_with(r#"{"event":"entry","path":"DCIM/a%FF.jpg""#),
            "{json}"
        );
        let back: Line = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_entry(), Some(entry.clone()));

        let symlink = SourceEntry {
            path: posix(b"DCIM/latest"),
            kind: EntryKind::Symlink,
            size: 9,
            mtime_ns: Some(1),
            link_target: Some(b"../a\xff%b".to_vec()),
        };
        let json = serde_json::to_string(&Line::entry(&symlink)).unwrap();
        assert!(json.contains(r#""target":"../a%FF%25b""#), "{json}");
        let back: Line = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_entry(), Some(symlink));
        assert!(
            !serde_json::to_string(&Line::entry(&entry))
                .unwrap()
                .contains("target"),
            "no target field without a target"
        );
        let old: Line = serde_json::from_str(
            r#"{"event":"entry","path":"l","kind":"symlink","size":1,"mtime_ns":null}"#,
        )
        .unwrap();
        assert_eq!(old.to_entry().unwrap().link_target, None);
        let file: Line = serde_json::from_str(
            r#"{"event":"entry","path":"f","kind":"file","size":1,"mtime_ns":null,"target":"x"}"#,
        )
        .unwrap();
        assert_eq!(
            file.to_entry().unwrap().link_target,
            None,
            "only links have targets"
        );
        for bad in ["", "a%00b", "a%Z"] {
            let line: Line = serde_json::from_str(&format!(
                r#"{{"event":"entry","path":"l","kind":"symlink","size":1,"mtime_ns":null,"target":"{bad}"}}"#
            ))
            .unwrap();
            assert_eq!(line.to_entry(), None, "{bad:?}");
        }

        let digest = hash_reader(HashAlgo::Blake3, &b"aaaa"[..], 4).unwrap();
        let line = Line::reply(3, &HashReply::Full(digest));
        let back: Line = serde_json::from_str(&serde_json::to_string(&line).unwrap()).unwrap();
        assert_eq!(
            back.to_reply(HashAlgo::Blake3),
            Some((3, HashReply::Full(digest)))
        );
        assert!(
            matches!(
                back.to_reply(HashAlgo::Xxh3),
                Some((3, HashReply::Warning(_)))
            ),
            "a digest of the wrong length is not accepted"
        );

        let warning = SourceWarning {
            path: Some(posix(b"x")),
            kind: WarningKind::Permission,
            message: "denied".into(),
        };
        let line = Line::reply(1, &HashReply::Warning(warning.clone()));
        assert_eq!(
            line.to_reply(HashAlgo::Blake3),
            Some((1, HashReply::Warning(warning)))
        );
        let hello: Line =
            serde_json::from_str(&serde_json::to_string(&Line::hello()).unwrap()).unwrap();
        assert!(matches!(
            hello,
            Line::Hello {
                protocol: PROTOCOL,
                ..
            }
        ));
    }
}
