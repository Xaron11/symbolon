//! `LocalSource` against generated directory trees.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;
use std::io::Read as _;
use std::ops::ControlFlow;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use symbolon_core::pathkey::RelPath;
use symbolon_core::source::{
    EntryKind, LocalSource, Source, SourceEntry, SourceError, WalkItem, WalkOutcome, WarningKind,
};

fn walk(source: &LocalSource) -> (Vec<SourceEntry>, Vec<(String, WarningKind)>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let outcome = source
        .walk(&mut |item| {
            match item {
                WalkItem::Entry(entry) => entries.push(entry),
                WalkItem::Warning(w) => warnings.push((
                    w.path
                        .map_or_else(String::new, |p| p.key().as_str().to_owned()),
                    w.kind,
                )),
            }
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(outcome, WalkOutcome::Complete);
    (entries, warnings)
}

fn keys(entries: &[SourceEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| e.path.key().as_str().to_owned())
        .collect()
}

fn write(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[test]
fn lists_each_directory_sorted_before_descending_with_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("z.bin"), &[0; 10]);
    write(&root.join("a/b.txt"), b"hello");
    fs::create_dir_all(root.join("a/c")).unwrap();
    write(&root.join("empty"), b"");
    let mtime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    fs::File::options()
        .write(true)
        .open(root.join("a/b.txt"))
        .unwrap()
        .set_modified(mtime)
        .unwrap();

    let (entries, warnings) = walk(&LocalSource::new(root, &[]).unwrap());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(keys(&entries), ["a", "empty", "z.bin", "a/b.txt", "a/c"]);

    let by_key = |key: &str| {
        entries
            .iter()
            .find(|e| e.path.key().as_str() == key)
            .unwrap()
    };
    assert_eq!(by_key("a").kind, EntryKind::Dir);
    assert_eq!(by_key("a").size, 0);
    assert_eq!(by_key("a/b.txt").kind, EntryKind::File);
    assert_eq!(by_key("a/b.txt").size, 5);
    assert_eq!(by_key("a/b.txt").mtime_ns, Some(1_700_000_000_000_000_000));
    assert_eq!(by_key("empty").size, 0);
    assert_eq!(by_key("z.bin").size, 10);
}

#[test]
fn unicode_and_percent_names_keep_their_text() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("zdjęcia/żółw.jpg"), b"x");
    write(&dir.path().join("100%.txt"), b"x");
    let (entries, _) = walk(&LocalSource::new(dir.path(), &[]).unwrap());
    assert_eq!(
        keys(&entries),
        ["100%25.txt", "zdjęcia", "zdjęcia/żółw.jpg"]
    );
}

#[test]
fn ignore_patterns_skip_files_and_whole_directories() {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        "a.tmp",
        "keep.tmp",
        "cache/big.bin",
        "src/cache/x",
        "src/main.rs",
    ] {
        write(&dir.path().join(path), b"x");
    }
    let patterns = ["*.tmp", "!keep.tmp", "cache/"].map(String::from);
    let (entries, _) = walk(&LocalSource::new(dir.path(), &patterns).unwrap());
    assert_eq!(keys(&entries), ["keep.tmp", "src", "src/main.rs"]);
}

#[test]
fn invalid_ignore_patterns_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let result = LocalSource::new(dir.path(), &["a{b".to_owned()]);
    assert!(matches!(result, Err(SourceError::InvalidPattern { pattern, .. }) if pattern == "a{b"));
}

#[test]
fn a_missing_root_is_an_error_not_an_empty_location() {
    let dir = tempfile::tempdir().unwrap();
    let missing = LocalSource::new(dir.path().join("unplugged"), &[]).unwrap();
    assert!(matches!(
        missing.walk(&mut |_| ControlFlow::Continue(())),
        Err(SourceError::RootUnavailable { .. })
    ));
    write(&dir.path().join("file"), b"x");
    let file = LocalSource::new(dir.path().join("file"), &[]).unwrap();
    assert!(file.walk(&mut |_| ControlFlow::Continue(())).is_err());
}

#[test]
fn the_visitor_can_stop_the_walk() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c", "d"] {
        write(&dir.path().join(name), b"x");
    }
    let source = LocalSource::new(dir.path(), &[]).unwrap();
    let mut seen = 0;
    let outcome = source
        .walk(&mut |_| {
            seen += 1;
            if seen == 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .unwrap();
    assert_eq!((outcome, seen), (WalkOutcome::Stopped, 2));
}

#[test]
fn open_and_stat_read_single_entries() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("d/f.txt"), b"contents");
    let source = LocalSource::new(dir.path(), &[]).unwrap();
    let path = RelPath::from_path("d/f.txt".as_ref()).unwrap();

    let mut text = String::new();
    source
        .open(&path)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "contents");

    let (entries, _) = walk(&source);
    let walked = entries.into_iter().find(|e| e.path == path).unwrap();
    assert_eq!(source.stat(&path).unwrap(), walked);

    let gone = RelPath::from_path("d/gone".as_ref()).unwrap();
    assert_eq!(
        source.stat(&gone).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
}

#[cfg(unix)]
mod unix {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    use symbolon_core::pathkey::PathEncoding;

    use super::*;

    #[test]
    fn non_utf8_names_are_listed_losslessly() {
        let dir = tempfile::tempdir().unwrap();
        let name = std::ffi::OsStr::from_bytes(b"bad\xffname");
        write(&dir.path().join(name), b"x");
        let (entries, _) = walk(&LocalSource::new(dir.path(), &[]).unwrap());
        assert_eq!(keys(&entries), ["bad%FFname"]);
        assert_eq!(entries[0].path.as_bytes(), b"bad\xffname");
    }

    #[test]
    fn symlinks_are_recorded_and_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("target.txt"), b"data");
        symlink("target.txt", dir.path().join("link")).unwrap();
        symlink(".", dir.path().join("loop")).unwrap();
        symlink("nowhere", dir.path().join("dangling")).unwrap();

        let (entries, warnings) = walk(&LocalSource::new(dir.path(), &[]).unwrap());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(keys(&entries), ["dangling", "link", "loop", "target.txt"]);
        let link = entries
            .iter()
            .find(|e| e.path.key().as_str() == "link")
            .unwrap();
        assert_eq!(link.kind, EntryKind::Symlink);
        assert_eq!(link.size, "target.txt".len() as u64);
    }

    #[test]
    fn symlinks_record_their_target_without_following_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("a.txt"), b"a");
        write(&root.join("sub/f"), b"f");
        symlink("a.txt", root.join("rel")).unwrap();
        symlink("missing/x", root.join("dangling")).unwrap();
        symlink("../outside", root.join("up")).unwrap();
        symlink("sub", root.join("dirlink")).unwrap();
        symlink(std::ffi::OsStr::from_bytes(b"a\xffb"), root.join("odd")).unwrap();

        let (entries, warnings) = walk(&LocalSource::new(root, &[]).unwrap());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            !keys(&entries).iter().any(|k| k.starts_with("dirlink/")),
            "{entries:?}"
        );
        let targets: Vec<(String, Option<Vec<u8>>)> = entries
            .iter()
            .map(|e| (e.path.key().as_str().to_owned(), e.link_target.clone()))
            .collect();
        let target = |t: &[u8]| Some(t.to_vec());
        assert_eq!(
            targets,
            [
                ("a.txt".to_owned(), None),
                ("dangling".to_owned(), target(b"missing/x")),
                ("dirlink".to_owned(), target(b"sub")),
                ("odd".to_owned(), target(b"a\xffb")),
                ("rel".to_owned(), target(b"a.txt")),
                ("sub".to_owned(), None),
                ("up".to_owned(), target(b"../outside")),
                ("sub/f".to_owned(), None),
            ]
        );
    }

    /// A listed path whose directory, or the file itself, is replaced by a symlink before it is
    /// hashed must not be read through the link (#82).
    #[test]
    fn open_never_follows_symlinks_swapped_in_after_the_walk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        write(&root.join("sub/file"), b"inside");
        write(&root.join("leaf"), b"inside");
        write(&dir.path().join("outside/file"), b"secret");
        write(&dir.path().join("outside/leaf"), b"secret");
        let source = LocalSource::new(&root, &[]).unwrap();
        let rel = |key: &str| RelPath::from_raw(PathEncoding::native(), key.into()).unwrap();
        let mut contents = String::new();
        source
            .open(&rel("sub/file"))
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "inside");

        fs::rename(root.join("sub"), root.join("sub.orig")).unwrap();
        symlink(dir.path().join("outside"), root.join("sub")).unwrap();
        fs::remove_file(root.join("leaf")).unwrap();
        symlink(dir.path().join("outside/leaf"), root.join("leaf")).unwrap();
        assert!(source.open(&rel("sub/file")).is_err(), "directory symlink");
        assert!(source.open(&rel("leaf")).is_err(), "file symlink");
    }

    #[test]
    fn open_refuses_special_files_without_blocking() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("fifo");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status();
        if !made.is_ok_and(|status| status.success()) {
            return;
        }
        let source = LocalSource::new(dir.path(), &[]).unwrap();
        let rel = RelPath::from_raw(PathEncoding::native(), b"fifo".to_vec()).unwrap();
        assert!(source.open(&rel).is_err());
    }

    #[test]
    fn unreadable_directories_become_warnings_and_the_walk_continues() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("locked/secret"), b"x");
        write(&dir.path().join("open/file"), b"x");
        let locked = dir.path().join("locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_dir(&locked).is_ok() {
            // Running as root: permissions are not enforced, nothing to test.
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }

        let (entries, warnings) = walk(&LocalSource::new(dir.path(), &[]).unwrap());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(keys(&entries), ["locked", "open", "open/file"]);
        assert_eq!(warnings, [("locked".to_owned(), WarningKind::Permission)]);
    }
}
