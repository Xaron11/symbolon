//! The agent binary on the host: its output is what core records for the same tree.

#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;
use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::process::Command;

use symbolon_core::agent::{Line, PROTOCOL, Request};
use symbolon_core::hash::HashAlgo;
use symbolon_core::scan::hash_requests;
use symbolon_core::source::{HashReply, HashRequest, LocalSource, Source as _, WalkItem};

fn agent(args: &[&str]) -> (bool, Vec<Line>) {
    let output = Command::new(env!("CARGO_BIN_EXE_symbolon-agent"))
        .args(args)
        .output()
        .unwrap();
    let lines = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (output.status.success(), lines)
}

fn tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("DCIM/Camera")).unwrap();
    fs::write(root.join("DCIM/Camera/a.jpg"), "aaaa").unwrap();
    fs::write(root.join("DCIM/b 100%.jpg"), vec![5_u8; 200_000]).unwrap();
    fs::write(root.join("skip.tmp"), "x").unwrap();
    dir
}

#[test]
fn walk_reports_what_a_local_walk_finds() {
    let dir = tree();
    let root = dir.path().to_str().unwrap();
    let (ok, lines) = agent(&["walk", root, "--ignore", "*.tmp"]);
    assert!(ok);
    assert!(
        matches!(
            lines.first(),
            Some(Line::Hello {
                protocol: PROTOCOL,
                ..
            })
        ),
        "{lines:?}"
    );
    assert!(
        matches!(lines.last(), Some(Line::Done { count: 4 })),
        "{lines:?}"
    );
    let walked: Vec<_> = lines.iter().filter_map(Line::to_entry).collect();

    let mut local = Vec::new();
    LocalSource::new(dir.path(), &["*.tmp".to_owned()])
        .unwrap()
        .walk(&mut |item| {
            if let WalkItem::Entry(entry) = item {
                local.push(entry);
            }
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(walked, local);
}

#[test]
fn hash_answers_requests_like_local_hashing() {
    let dir = tree();
    let source = LocalSource::new(dir.path(), &[]).unwrap();
    let mut requests = Vec::new();
    source
        .walk(&mut |item| {
            if let WalkItem::Entry(entry) = item
                && entry.kind == symbolon_core::source::EntryKind::File
            {
                for partial in [false, true] {
                    requests.push(HashRequest {
                        path: entry.path.clone(),
                        size: entry.size,
                        mtime_ns: entry.mtime_ns,
                        partial,
                    });
                }
            }
            ControlFlow::Continue(())
        })
        .unwrap();
    // One request whose file changed since it was listed.
    requests.push(HashRequest {
        size: requests[0].size + 1,
        ..requests[0].clone()
    });
    let file = dir.path().join("requests.ndjson");
    let text: String = requests
        .iter()
        .map(|r| serde_json::to_string(&Request::new(r)).unwrap() + "\n")
        .collect();
    fs::write(&file, text).unwrap();

    let (ok, lines) = agent(&[
        "hash",
        dir.path().to_str().unwrap(),
        "--algo",
        "blake3",
        "--requests",
        file.to_str().unwrap(),
    ]);
    assert!(ok, "{lines:?}");
    let mut remote = vec![None; requests.len()];
    for (index, reply) in lines.iter().filter_map(|l| l.to_reply(HashAlgo::Blake3)) {
        remote[index] = Some(reply);
    }
    let mut local = vec![None; requests.len()];
    hash_requests(
        &source,
        HashAlgo::Blake3,
        &requests,
        NonZeroUsize::MIN,
        |i, reply| {
            local[i] = Some(reply);
        },
    )
    .unwrap();
    let digests = |replies: &[Option<HashReply>]| -> Vec<Option<HashReply>> {
        replies
            .iter()
            .map(|r| r.clone().filter(|r| !matches!(r, HashReply::Warning(_))))
            .collect()
    };
    assert_eq!(digests(&remote), digests(&local));
    assert!(
        matches!(remote.last(), Some(Some(HashReply::Warning(_)))),
        "{lines:?}"
    );
    assert!(
        matches!(lines.last(), Some(Line::Done { count }) if *count == requests.len() as u64),
        "{lines:?}"
    );
}

#[test]
fn failures_end_with_an_error_line() {
    let (ok, lines) = agent(&["walk", "/nonexistent/symbolon-test"]);
    assert!(!ok);
    assert!(
        matches!(lines.last(), Some(Line::Error { .. })),
        "{lines:?}"
    );
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("none.ndjson");
    let (ok, lines) = agent(&[
        "hash",
        dir.path().to_str().unwrap(),
        "--algo",
        "blake3",
        "--requests",
        missing.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert!(
        matches!(lines.last(), Some(Line::Error { message }) if message.contains("request file")),
        "{lines:?}"
    );
}
