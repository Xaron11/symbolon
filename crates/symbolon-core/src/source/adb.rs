//! [`AdbSource`]: a directory on an Android phone, listed and hashed on the phone (ADR-0017).
//!
//! The agent is pushed once per source and removed when the source is dropped. When it cannot
//! run, the source falls back to toybox commands: whole-second times, SHA-256 only, no partial
//! digests; [`AdbSource::mode`] says which path is in use and why.

use std::collections::BTreeMap;
use std::io::{self, BufRead as _, BufReader, Write as _};
use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use super::{
    EntryKind, HashReply, HashRequest, ReadSeek, RemoteHasher, Source, SourceEntry, SourceError,
    SourceWarning, WalkItem, WalkOutcome, WarningKind,
};
use crate::agent::{Line, PROTOCOL, Request};
use crate::hash::{Digest, HashAlgo};
use crate::pathkey::{PathEncoding, RelPath};
use crate::transport::android::{AndroidTransport, shell_quote};

/// How an [`AdbSource`] reaches its phone.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AdbOptions {
    /// Directory on the phone the agent and request files are pushed to.
    pub remote_dir: String,
    /// Hashing threads on the phone; more than two were slower on phone storage (ADR-0003).
    pub threads: NonZeroUsize,
}

impl Default for AdbOptions {
    fn default() -> Self {
        Self {
            remote_dir: "/data/local/tmp".to_owned(),
            threads: NonZeroUsize::new(2).unwrap_or(NonZeroUsize::MIN),
        }
    }
}

/// Which path an [`AdbSource`] uses.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdbMode {
    /// The agent runs on the phone: any algorithm, partial digests.
    Agent,
    /// Toybox commands: SHA-256 only, full digests only, whole-second times.
    Fallback {
        /// Why the agent is not used.
        reason: String,
    },
}

/// A directory on an Android phone.
pub struct AdbSource {
    transport: Arc<dyn AndroidTransport>,
    serial: String,
    root: String,
    ignore_patterns: Vec<String>,
    ignore: Gitignore,
    agent: Option<PathBuf>,
    options: AdbOptions,
    session: Mutex<Session>,
}

#[derive(Default)]
struct Session {
    mode: Option<Mode>,
    /// Files on the phone to remove when the source is dropped.
    pushed: Vec<String>,
    requests: u32,
}

#[derive(Clone)]
enum Mode {
    Agent { remote: String },
    Fallback { reason: String },
}

impl std::fmt::Debug for AdbSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdbSource")
            .field("serial", &self.serial)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl AdbSource {
    /// The directory `root` (absolute, on the phone) of device `serial`. `agent` is the
    /// host's copy of the agent binary; without one, the fallback is used.
    ///
    /// # Errors
    ///
    /// [`SourceError::InvalidPattern`] for an ignore pattern that is not `.gitignore` syntax.
    pub fn new(
        transport: Arc<dyn AndroidTransport>,
        serial: impl Into<String>,
        root: impl Into<String>,
        ignore_patterns: &[String],
        agent: Option<PathBuf>,
        options: AdbOptions,
    ) -> Result<Self, SourceError> {
        let mut builder = GitignoreBuilder::new("/");
        for pattern in ignore_patterns {
            builder
                .add_line(None, pattern)
                .map_err(|e| SourceError::InvalidPattern {
                    pattern: pattern.clone(),
                    message: e.to_string(),
                })?;
        }
        let ignore = builder.build().map_err(|e| SourceError::InvalidPattern {
            pattern: ignore_patterns.join("\n"),
            message: e.to_string(),
        })?;
        Ok(Self {
            transport,
            serial: serial.into(),
            root: root.into(),
            ignore_patterns: ignore_patterns.to_vec(),
            ignore,
            agent,
            options,
            session: Mutex::new(Session::default()),
        })
    }

    /// Whether the agent or the fallback is used, deciding on first use: the agent needs a
    /// host copy, an arm64 phone, and a `remote_dir` it can be pushed to and run from.
    pub fn mode(&self) -> AdbMode {
        match self.ensure_mode() {
            Mode::Agent { .. } => AdbMode::Agent,
            Mode::Fallback { reason } => AdbMode::Fallback { reason },
        }
    }

    fn session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn ensure_mode(&self) -> Mode {
        if let Some(mode) = &self.session().mode {
            return mode.clone();
        }
        let mode = self
            .start_agent()
            .unwrap_or_else(|reason| Mode::Fallback { reason });
        self.session().mode = Some(mode.clone());
        mode
    }

    fn start_agent(&self) -> Result<Mode, String> {
        let agent = self
            .agent
            .as_ref()
            .ok_or("no copy of the agent was found on this computer")?;
        if let Ok(abi) = self
            .transport
            .shell(&self.serial, "getprop ro.product.cpu.abi")
        {
            let abi = String::from_utf8_lossy(&abi.stdout).trim().to_owned();
            if abi.starts_with("x86") || abi.starts_with("armeabi") {
                return Err(format!("the agent runs on arm64 phones; this one is {abi}"));
            }
        }
        let remote = format!(
            "{}/symbolon-agent-{}",
            self.options.remote_dir,
            crate::VERSION
        );
        self.transport
            .push(&self.serial, agent, &remote)
            .map_err(|e| format!("cannot copy the agent to the phone: {e}"))?;
        self.session().pushed.push(remote.clone());
        let check = format!("chmod 755 {0} && {0} --version", shell_quote(&remote));
        let output = self
            .transport
            .shell(&self.serial, &check)
            .map_err(|e| format!("cannot start the agent: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if output.status != 0 || !stdout.starts_with("symbolon-agent") {
            let why = [output.stderr.trim(), stdout.trim()]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("no output")
                .to_owned();
            return Err(format!("the agent cannot run on this phone ({why})"));
        }
        Ok(Mode::Agent { remote })
    }

    /// Runs the agent and hands each line after `hello` to `line`, until `done`.
    fn run_agent(
        &self,
        argv: &[String],
        line: &mut dyn FnMut(Line) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        let remote_error = |e: &dyn std::fmt::Display| SourceError::Remote(e.to_string());
        let stream = self
            .transport
            .exec(&self.serial, argv)
            .map_err(|e| remote_error(&e))?;
        let mut reader = BufReader::new(stream);
        let mut text = String::new();
        let mut first = true;
        loop {
            text.clear();
            if reader.read_line(&mut text).map_err(|e| remote_error(&e))? == 0 {
                return Err(SourceError::Remote(
                    "the agent stopped before finishing (was the phone disconnected?)".into(),
                ));
            }
            let parsed: Line = serde_json::from_str(text.trim_end()).map_err(|_| {
                SourceError::Remote(format!(
                    "the agent sent something unexpected: {}",
                    text.trim()
                ))
            })?;
            if first {
                first = false;
                match parsed {
                    Line::Hello { protocol, .. } if protocol == PROTOCOL => continue,
                    Line::Hello { protocol, .. } => {
                        return Err(SourceError::Remote(format!(
                            "the agent on the phone speaks protocol {protocol}; this Symbolon \
                             speaks {PROTOCOL}"
                        )));
                    }
                    _ => {
                        return Err(SourceError::Remote(
                            "the agent did not introduce itself".into(),
                        ));
                    }
                }
            }
            match parsed {
                Line::Done { .. } => break,
                Line::Error { message } => {
                    return Err(SourceError::Remote(format!("on the phone: {message}")));
                }
                other => {
                    if line(other).is_break() {
                        return Ok(WalkOutcome::Stopped);
                    }
                }
            }
        }
        reader.into_inner().finish().map_err(|e| remote_error(&e))?;
        Ok(WalkOutcome::Complete)
    }

    fn walk_agent(
        &self,
        remote: &str,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        let mut argv = vec![remote.to_owned(), "walk".to_owned(), self.root.clone()];
        for pattern in &self.ignore_patterns {
            argv.extend(["--ignore".to_owned(), pattern.clone()]);
        }
        self.run_agent(&argv, &mut |line| {
            if let Some(entry) = line.to_entry() {
                visit(WalkItem::Entry(entry))
            } else if let Some(warning) = line.to_warning() {
                visit(WalkItem::Warning(warning))
            } else {
                ControlFlow::Continue(())
            }
        })
    }

    /// Lists with `find` and `stat`, then reports in the order [`Source::walk`] promises.
    fn walk_fallback(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        let command = format!(
            "cd {} && find . -exec stat -c '%F|%s|%Y|%n' {{}} +",
            shell_quote(&self.root)
        );
        let output = self
            .transport
            .shell(&self.serial, &command)
            .map_err(|e| SourceError::Remote(e.to_string()))?;
        if output.status != 0 && output.stdout.is_empty() {
            return Err(SourceError::RootUnavailable {
                root: self.root.clone(),
                source: io::Error::other(output.stderr.trim().to_owned()),
            });
        }
        let mut warnings = Vec::new();
        let mut by_parent: BTreeMap<Vec<u8>, Vec<SourceEntry>> = BTreeMap::new();
        for line in output
            .stdout
            .split(|&b| b == b'\n')
            .filter(|l| !l.is_empty())
        {
            match parse_stat_line(line) {
                Some(entry) if !entry.path.is_root() => {
                    let is_dir = entry.kind == EntryKind::Dir;
                    let rel = String::from_utf8_lossy(entry.path.as_bytes()).into_owned();
                    if self
                        .ignore
                        .matched_path_or_any_parents(&rel, is_dir)
                        .is_ignore()
                    {
                        continue;
                    }
                    let parent = parent_of(entry.path.as_bytes()).to_vec();
                    by_parent.entry(parent).or_default().push(entry);
                }
                Some(_) => {}
                None => warnings.push(SourceWarning {
                    path: None,
                    kind: WarningKind::Io,
                    message: format!(
                        "cannot list {:?}: names with line breaks need the agent",
                        String::from_utf8_lossy(line)
                    ),
                }),
            }
        }
        for message in output.stderr.lines().filter(|l| !l.trim().is_empty()) {
            warnings.push(SourceWarning {
                path: None,
                kind: if message.contains("ermission denied") {
                    WarningKind::Permission
                } else {
                    WarningKind::Io
                },
                message: message.trim().to_owned(),
            });
        }
        // Children of a directory together, sorted; a directory before its contents.
        let mut stack = vec![Vec::new()];
        while let Some(parent) = stack.pop() {
            let Some(mut children) = by_parent.remove(&parent) else {
                continue;
            };
            children.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
            let dirs: Vec<Vec<u8>> = children
                .iter()
                .filter(|e| e.kind == EntryKind::Dir)
                .map(|e| e.path.as_bytes().to_vec())
                .collect();
            for entry in children {
                if visit(WalkItem::Entry(entry)).is_break() {
                    return Ok(WalkOutcome::Stopped);
                }
            }
            stack.extend(dirs.into_iter().rev());
        }
        for warning in warnings {
            if visit(WalkItem::Warning(warning)).is_break() {
                return Ok(WalkOutcome::Stopped);
            }
        }
        Ok(WalkOutcome::Complete)
    }

    fn hash_agent(
        &self,
        remote: &str,
        algo: HashAlgo,
        requests: &[HashRequest],
        each: &mut dyn FnMut(usize, HashReply) -> ControlFlow<()>,
    ) -> Result<(), SourceError> {
        let remote_error = |e: &dyn std::fmt::Display| SourceError::Remote(e.to_string());
        let mut file = tempfile::NamedTempFile::new().map_err(|e| remote_error(&e))?;
        for request in requests {
            serde_json::to_writer(&mut file, &Request::new(request))
                .map_err(|e| remote_error(&e))?;
            file.write_all(b"\n").map_err(|e| remote_error(&e))?;
        }
        file.flush().map_err(|e| remote_error(&e))?;
        let number = {
            let mut session = self.session();
            session.requests += 1;
            session.requests
        };
        let remote_file = format!(
            "{}/symbolon-requests-{}-{number}.ndjson",
            self.options.remote_dir,
            std::process::id()
        );
        self.transport
            .push(&self.serial, file.path(), &remote_file)
            .map_err(|e| remote_error(&e))?;
        self.session().pushed.push(remote_file.clone());
        let argv = vec![
            remote.to_owned(),
            "hash".to_owned(),
            self.root.clone(),
            "--algo".to_owned(),
            algo.name().to_owned(),
            "--threads".to_owned(),
            self.options.threads.to_string(),
            "--requests".to_owned(),
            remote_file.clone(),
        ];
        let result = self.run_agent(&argv, &mut |line| match line.to_reply(algo) {
            Some((index, reply)) => each(index, reply),
            None => ControlFlow::Continue(()),
        });
        self.remove(&[remote_file]);
        result.map(drop)
    }

    /// One `cat | sha256sum` per file under `pipefail`, with the file's size and time read
    /// back, in batches.
    fn hash_fallback(
        &self,
        requests: &[HashRequest],
        each: &mut dyn FnMut(usize, HashReply) -> ControlFlow<()>,
    ) -> Result<(), SourceError> {
        const BATCH: usize = 100;
        for (start, batch) in (0..).step_by(BATCH).zip(requests.chunks(BATCH)) {
            let mut names = Vec::with_capacity(batch.len());
            for (offset, request) in batch.iter().enumerate() {
                if let Ok(name) = std::str::from_utf8(request.path.as_bytes()) {
                    names.push(shell_quote(name));
                } else {
                    // Keeps the output aligned with the requests; this line is skipped below.
                    names.push(shell_quote("\u{fffd}/unlisted"));
                    let reply =
                        warning(request, "the fallback cannot pass a name that is not UTF-8");
                    if each(start + offset, reply).is_break() {
                        return Ok(());
                    }
                }
            }
            let command = format!(
                "set -o pipefail; cd {} || exit 2; for f in {}; do \
                 if h=$(cat -- \"$f\" | sha256sum) && s=$(stat -c '%s|%Y' -- \"$f\"); \
                 then printf 'ok|%s|%s\\n' \"$s\" \"${{h%% *}}\"; else printf 'fail\\n'; fi; done",
                shell_quote(&self.root),
                names.join(" ")
            );
            let output = self
                .transport
                .shell(&self.serial, &command)
                .map_err(|e| SourceError::Remote(e.to_string()))?;
            if output.status == 2 {
                return Err(SourceError::Remote(format!(
                    "cannot enter {} on the phone: {}",
                    self.root,
                    output.stderr.trim()
                )));
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let mut lines = text.lines();
            for (offset, request) in batch.iter().enumerate() {
                if std::str::from_utf8(request.path.as_bytes()).is_err() {
                    lines.next();
                    continue;
                }
                let reply = match lines.next().map(|l| l.split('|').collect::<Vec<_>>()) {
                    Some(fields) if fields.first() == Some(&"ok") => {
                        fallback_reply(request, fields.get(1..).unwrap_or_default())
                    }
                    _ => warning(request, "could not be read on the phone"),
                };
                if each(start + offset, reply).is_break() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn remove(&self, files: &[String]) {
        if files.is_empty() {
            return;
        }
        let quoted: Vec<String> = files.iter().map(|f| shell_quote(f)).collect();
        // Best effort: a phone that is gone cannot be cleaned, and keeps no running agent.
        self.transport
            .shell(&self.serial, &format!("rm -f {}", quoted.join(" ")))
            .ok();
        self.session().pushed.retain(|p| !files.contains(p));
    }
}

impl Drop for AdbSource {
    fn drop(&mut self) {
        let pushed = std::mem::take(&mut self.session().pushed);
        self.remove(&pushed);
    }
}

impl Source for AdbSource {
    fn walk(
        &self,
        visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        match self.ensure_mode() {
            Mode::Agent { remote } => self.walk_agent(&remote, visit),
            Mode::Fallback { .. } => self.walk_fallback(visit),
        }
    }

    fn open(&self, _path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "files on the phone are hashed on the phone",
        ))
    }

    fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
        let name = std::str::from_utf8(path.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let command = format!(
            "cd {} && stat -c '%F|%s|%Y|%n' -- {}",
            shell_quote(&self.root),
            shell_quote(name)
        );
        let output = self
            .transport
            .shell(&self.serial, &command)
            .map_err(io::Error::other)?;
        output
            .stdout
            .split(|&b| b == b'\n')
            .next()
            .and_then(parse_stat_line)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, output.stderr.trim().to_owned()))
    }

    fn remote_hasher(&self) -> Option<&dyn RemoteHasher> {
        Some(self)
    }
}

impl RemoteHasher for AdbSource {
    fn supports(&self, algo: HashAlgo) -> bool {
        match self.ensure_mode() {
            Mode::Agent { .. } => true,
            Mode::Fallback { .. } => algo == HashAlgo::Sha256,
        }
    }

    fn partial_digests(&self) -> bool {
        matches!(self.ensure_mode(), Mode::Agent { .. })
    }

    fn hash(
        &self,
        algo: HashAlgo,
        requests: &[HashRequest],
        each: &mut dyn FnMut(usize, HashReply) -> ControlFlow<()>,
    ) -> Result<(), SourceError> {
        match self.ensure_mode() {
            Mode::Agent { remote } => self.hash_agent(&remote, algo, requests, each),
            Mode::Fallback { .. } => self.hash_fallback(requests, each),
        }
    }
}

fn warning(request: &HashRequest, message: &str) -> HashReply {
    HashReply::Warning(SourceWarning {
        path: Some(request.path.clone()),
        kind: WarningKind::Io,
        message: message.to_owned(),
    })
}

/// `size|seconds|hex` read back by the fallback, checked against the request.
fn fallback_reply(request: &HashRequest, fields: &[&str]) -> HashReply {
    let [size, secs, hex] = fields else {
        return warning(request, "the phone gave an unexpected answer");
    };
    let listed_secs = request.mtime_ns.map(|ns| ns.div_euclid(1_000_000_000));
    if size.parse::<u64>().ok() != Some(request.size) || secs.parse::<i64>().ok() != listed_secs {
        return HashReply::Warning(SourceWarning {
            path: Some(request.path.clone()),
            kind: WarningKind::Changed,
            message: "changed since it was listed; not hashed".to_owned(),
        });
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    match bytes.and_then(|b| Digest::from_bytes(HashAlgo::Sha256, &b).ok()) {
        Some(digest) => HashReply::Full(digest),
        None => warning(request, "the phone gave an invalid SHA-256 digest"),
    }
}

/// `%F|%s|%Y|%n` from `stat -c`, with `%n` as `./relative/path`.
fn parse_stat_line(line: &[u8]) -> Option<SourceEntry> {
    let mut fields = line.splitn(4, |&b| b == b'|');
    let kind = match fields.next()? {
        b"regular file" | b"regular empty file" => EntryKind::File,
        b"directory" => EntryKind::Dir,
        b"symbolic link" => EntryKind::Symlink,
        _ => EntryKind::Other,
    };
    let size: u64 = std::str::from_utf8(fields.next()?).ok()?.parse().ok()?;
    let secs: i64 = std::str::from_utf8(fields.next()?).ok()?.parse().ok()?;
    let name = fields.next()?;
    let rel = match name {
        b"." => Vec::new(),
        _ => name.strip_prefix(b"./")?.to_vec(),
    };
    Some(SourceEntry {
        path: RelPath::from_raw(PathEncoding::Posix, rel).ok()?,
        kind,
        size: if kind == EntryKind::Dir { 0 } else { size },
        mtime_ns: secs.checked_mul(1_000_000_000),
    })
}

fn parent_of(path: &[u8]) -> &[u8] {
    path.iter()
        .rposition(|&b| b == b'/')
        .map_or(&[][..], |i| path.get(..i).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_lines_become_entries() {
        let entry = parse_stat_line(b"regular file|4|1700000000|./DCIM/a|b.jpg").unwrap();
        assert_eq!(entry.path.as_bytes(), b"DCIM/a|b.jpg");
        assert_eq!(
            (entry.kind, entry.size, entry.mtime_ns),
            (EntryKind::File, 4, Some(1_700_000_000_000_000_000))
        );
        assert_eq!(parse_stat_line(b"directory|4096|1|./DCIM").unwrap().size, 0);
        assert!(
            parse_stat_line(b"directory|4096|1|.")
                .unwrap()
                .path
                .is_root()
        );
        assert_eq!(
            parse_stat_line(b"symbolic link|7|1|./l").unwrap().kind,
            EntryKind::Symlink
        );
        assert!(parse_stat_line(b"half a name").is_none());
        assert!(parse_stat_line(b"regular file|x|1|./a").is_none());
    }

    #[test]
    fn fallback_replies_check_size_and_time() {
        let request = HashRequest {
            path: RelPath::from_raw(PathEncoding::Posix, b"a".to_vec()).unwrap(),
            size: 4,
            mtime_ns: Some(1_700_000_000_500_000_000),
            partial: false,
        };
        let hex = "61be55a8e2f6b4e172338bddf184d6dbee29c98853e0a0485ecee7f27b9af0b4";
        assert!(matches!(
            fallback_reply(&request, &["4", "1700000000", hex]),
            HashReply::Full(_)
        ));
        assert!(matches!(
            fallback_reply(&request, &["5", "1700000000", hex]),
            HashReply::Warning(SourceWarning {
                kind: WarningKind::Changed,
                ..
            })
        ));
        assert!(matches!(
            fallback_reply(&request, &["4", "1700000000", "zz"]),
            HashReply::Warning(_)
        ));
    }

    #[test]
    fn parents_are_found_by_the_last_slash() {
        assert_eq!(parent_of(b"a/b/c"), b"a/b");
        assert_eq!(parent_of(b"a"), b"");
    }
}
