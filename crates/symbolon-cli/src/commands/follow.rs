//! Before a location's files are read: follow its device when it moved, and ask when it is
//! not clear which directory is the location (ADR-0016).

use std::path::Path;

use symbolon_core::catalog::{Catalog, LocationRoot, Whereabouts};

use crate::output;
use crate::prompt;

/// Whether a location can be read now.
pub(super) enum Ready {
    Yes,
    /// Not read, and why.
    Skip(String),
}

/// What `scan` does with a location's whereabouts. A missing root is left for the scan to
/// report unless `skip_missing` (scanning every location: unplugged devices are normal).
pub(super) fn prepare(
    catalog: &Catalog,
    label: &str,
    accept_other: bool,
    skip_missing: bool,
    quiet: bool,
) -> anyhow::Result<Ready> {
    Ok(match catalog.locate(label)? {
        Whereabouts::Moved { from, to } => {
            follow(catalog, label, &from, &to, quiet)?;
            Ready::Yes
        }
        Whereabouts::Ambiguous { root, candidates } => {
            let options: Vec<String> = candidates.iter().map(|c| shown(c)).collect();
            if !prompt::interactive() {
                return Ok(Ready::Skip(format!(
                    "location {label:?} is not at {}, and its device is mounted at several places: \
                     {}; choose one with `symbolon location set {label} --root <PATH>`",
                    shown(&root),
                    options.join(", ")
                )));
            }
            let question = format!(
                "Location {label:?} is not at {}, and its device is mounted at several places.",
                shown(&root)
            );
            match prompt::choose(&question, &options)?.and_then(|i| candidates.get(i)) {
                Some(choice) => {
                    follow(catalog, label, &root, choice, quiet)?;
                    Ready::Yes
                }
                None => Ready::Skip(format!("{label}: no directory chosen")),
            }
        }
        Whereabouts::OtherDevice {
            root,
            expected,
            found,
        } => {
            let found = found.map_or_else(
                || "a directory without a device ID".to_owned(),
                |f| format!("device {f}"),
            );
            let accepted = accept_other
                || (prompt::interactive()
                    && prompt::yes(&format!(
                        "Location {label:?} expects device {expected} at {}, but {found} is there. \
                         Scan it as {label:?}, replacing what is recorded?",
                        shown(&root)
                    ))?);
            if accepted {
                catalog.relocate(label, &root)?;
                if !quiet {
                    eprintln!("{label}: using {found} at {}", shown(&root));
                }
                Ready::Yes
            } else if prompt::interactive() {
                Ready::Skip(format!("{label}: not scanned"))
            } else {
                Ready::Skip(format!(
                    "location {label:?} expects device {expected} at {}, but {found} is there; \
                     plug its device in, or pass --yes to scan what is there as {label:?}",
                    shown(&root)
                ))
            }
        }
        Whereabouts::Missing(root) if skip_missing => Ready::Skip(format!(
            "{label}: skipped, {} is not available",
            shown(&root)
        )),
        _ => Ready::Yes,
    })
}

/// Follows a location whose device moved, without asking: the device is unambiguous.
pub(super) fn follow_moves(catalog: &Catalog, label: &str, quiet: bool) -> anyhow::Result<()> {
    if let Whereabouts::Moved { from, to } = catalog.locate(label)? {
        follow(catalog, label, &from, &to, quiet)?;
    }
    Ok(())
}

fn follow(
    catalog: &Catalog,
    label: &str,
    from: &Path,
    to: &Path,
    quiet: bool,
) -> anyhow::Result<()> {
    catalog.relocate(label, to)?;
    if !quiet {
        eprintln!(
            "{label}: its device is now at {} (was {}); following it",
            shown(to),
            shown(from)
        );
    }
    Ok(())
}

/// A path as `location list` shows roots: without Windows' `\\?\` prefix, control
/// characters escaped.
fn shown(path: &Path) -> String {
    let display = LocationRoot::Local(path.to_path_buf()).display();
    output::printable(&display).into_owned()
}
