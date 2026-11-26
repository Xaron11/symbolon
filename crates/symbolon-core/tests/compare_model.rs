//! Property tests: the comparison engine against a small reference model (ADR-0010, with
//! directories and symlinks from ADR-0019).
//!
//! Locations are written straight into the catalog (the engine is a function of catalog
//! state), which keeps cases fast and allows names that case-insensitive filesystems cannot
//! hold side by side.

#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use proptest::prelude::*;
use rusqlite::{Connection, params};
use symbolon_core::catalog::{Catalog, NewLocation};
use symbolon_core::compare::{Category, CompareOptions, Comparison, Row, RowFilter, Side};
use symbolon_core::hash::{HashAlgo, hash_bytes};
use symbolon_core::pathkey::{KeyOptions, PathKey, RelPath};
use symbolon_core::query::Page;
use symbolon_core::source::EntryKind;

/// Paths chosen to collide: case variants, NFC/NFD spellings, same names in other
/// directories (moves) and other names (renames), and `d` as both an entry and a directory.
const PATHS: [&str; 11] = [
    "a",
    "A",
    "b",
    "café",
    "cafe\u{301}",
    "d",
    "d/a",
    "d/A",
    "e/a",
    "x.txt",
    "X.TXT",
];
/// `p` and `q` have the same size but different content.
const CONTENTS: [&str; 3] = ["p", "q", "rr"];
/// Symlink targets; `x` and `y` have the same length.
const TARGETS: [&str; 3] = ["x", "y", "../zz"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Entry {
    File(&'static str),
    /// A symlink and its target, `None` when unknown.
    Link(Option<&'static str>),
    Dir,
}

impl Entry {
    const fn kind(self) -> EntryKind {
        match self {
            Self::File(_) => EntryKind::File,
            Self::Link(_) => EntryKind::Symlink,
            Self::Dir => EntryKind::Dir,
        }
    }
}

/// Every entry of a location by path, including the directories that hold them.
type Location = BTreeMap<String, Entry>;

/// A row as a comparable value: category, assumed flag, and the files as `(side, path)`.
type RowShape = (Category, bool, BTreeSet<(usize, String)>);

struct World {
    _dir: tempfile::TempDir,
    catalog: Catalog,
    sides: usize,
}

/// Writes the locations into a fresh catalog; `hashed` decides whether full digests exist.
fn world(locations: &[Location], hashed: bool) -> World {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    let conn = Connection::open(catalog.path()).unwrap();
    for (index, files) in locations.iter().enumerate() {
        let root = dir.path().join(format!("root{index}"));
        std::fs::create_dir(&root).unwrap();
        let location = catalog
            .add_location(NewLocation::local(format!("s{index}"), &root))
            .unwrap();
        for (path, entry) in files {
            let rel = RelPath::from_path(path.as_ref()).unwrap();
            let key = rel.key();
            let (size, digest, target) = match *entry {
                Entry::File(contents) => (
                    contents.len(),
                    hashed.then(|| {
                        hash_bytes(HashAlgo::Blake3, contents.as_bytes())
                            .as_bytes()
                            .to_vec()
                    }),
                    None,
                ),
                Entry::Link(target) => (target.map_or(2, str::len), None, target),
                Entry::Dir => (0, None, None),
            };
            conn.execute(
                "INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, hash_full, seen_scan_id, link_target)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1000, ?7, 1, ?8)",
                params![location.id.get(), rel.as_bytes(), key.as_str(), key.parent(), entry.kind().name(), size as u64, digest, target.map(str::as_bytes)],
            )
            .unwrap();
        }
    }
    World {
        _dir: dir,
        catalog,
        sides: locations.len(),
    }
}

fn engine(world: &World, order: &[usize], options: CompareOptions) -> Vec<RowShape> {
    let sides: Vec<Side> = order
        .iter()
        .map(|i| Side::location(format!("s{i}")))
        .collect();
    let cmp = Comparison::new(&world.catalog, &sides, options).unwrap();
    let mut rows: Vec<Row> = Vec::new();
    let mut page = Page::first(1000);
    loop {
        let result = cmp.rows(&RowFilter::default(), &page).unwrap();
        rows.extend(result.items);
        match result.next {
            Some(cursor) => page = Page::after(cursor, 1000),
            None => break,
        }
    }
    // Report sides by their original index so permuted comparisons are comparable.
    let mut shapes: Vec<RowShape> = rows
        .into_iter()
        .map(|row| {
            let files = row
                .cells
                .iter()
                .map(|c| (order[c.side], c.path.key().as_str().to_owned()))
                .collect();
            (row.category, row.assumed, files)
        })
        .collect();
    shapes.sort();
    shapes
}

/// Entries of one normalized path, by side.
type Group<'a> = BTreeMap<usize, Vec<(&'a str, Entry)>>;

/// A path row of the model, before moves are matched.
struct PathRow {
    category: Category,
    presence: u64,
    /// The common content of an only-in row of files.
    content: Option<String>,
    files: Vec<(usize, String)>,
}

fn key_of(path: &str) -> String {
    RelPath::from_path(path.as_ref())
        .unwrap()
        .key()
        .as_str()
        .to_owned()
}

/// Path classification for fully hashed locations: ADR-0010 for files, ADR-0019 for
/// directories, symlinks and kind clashes.
fn path_rows(locations: &[Location], keys: KeyOptions) -> Vec<PathRow> {
    let norm = |path: &str| PathKey::from_stored(key_of(path)).normalized(keys);
    let mut groups: BTreeMap<String, Group<'_>> = BTreeMap::new();
    for (side, entries) in locations.iter().enumerate() {
        for (path, entry) in entries {
            groups
                .entry(norm(path))
                .or_default()
                .entry(side)
                .or_default()
                .push((path, *entry));
        }
    }
    let every_side = (1 << locations.len()) - 1;
    groups
        .iter()
        .filter_map(|(key, by_side)| {
            let all: Vec<(usize, &str, Entry)> = by_side
                .iter()
                .flat_map(|(side, list)| list.iter().map(|(p, e)| (*side, *p, *e)))
                .collect();
            let presence = by_side.keys().fold(0, |mask, side| mask | (1 << side));
            let kinds: BTreeSet<&str> = all.iter().map(|(_, _, e)| e.kind().name()).collect();
            let category = if kinds == BTreeSet::from(["dir"]) {
                let beneath = format!("{key}/");
                let covered = groups.iter().any(|(other, sides)| {
                    other.starts_with(&beneath)
                        && sides.keys().any(|side| presence & (1 << side) != 0)
                });
                if presence == every_side || covered {
                    return None;
                }
                Category::OnlyIn
            } else if by_side.values().any(|list| list.len() > 1) {
                Category::Collision
            } else if kinds.len() > 1 {
                Category::Different
            } else if kinds.contains("symlink") {
                let targets: Vec<Option<&str>> = all
                    .iter()
                    .filter_map(|(_, _, e)| match e {
                        Entry::Link(target) => Some(*target),
                        _ => None,
                    })
                    .collect();
                let known: BTreeSet<&str> = targets.iter().flatten().copied().collect();
                let all_known = targets.iter().all(Option::is_some);
                if by_side.len() >= 2 && known.len() > 1 {
                    Category::Different
                } else if all_known && presence == every_side {
                    Category::Identical
                } else if all_known || by_side.len() == 1 {
                    Category::OnlyIn
                } else {
                    Category::Unverified
                }
            } else {
                let contents: BTreeSet<&str> = all
                    .iter()
                    .filter_map(|(_, _, e)| match e {
                        Entry::File(c) => Some(*c),
                        _ => None,
                    })
                    .collect();
                let sizes: BTreeSet<usize> = contents.iter().map(|c| c.len()).collect();
                if by_side.len() >= 2 && (sizes.len() > 1 || contents.len() > 1) {
                    Category::Different
                } else if presence == every_side {
                    Category::Identical
                } else {
                    Category::OnlyIn
                }
            };
            let content = match all[0].2 {
                Entry::File(c) if category == Category::OnlyIn => Some(c.to_owned()),
                _ => None,
            };
            Some(PathRow {
                category,
                presence,
                content,
                files: all
                    .iter()
                    .map(|(side, path, _)| (*side, key_of(path)))
                    .collect(),
            })
        })
        .collect()
}

/// The reference model: path rows, then only-in rows of files sharing content with an only-in
/// row of disjoint presence collapse into one moved or renamed row per content.
fn model(locations: &[Location], keys: KeyOptions) -> Vec<RowShape> {
    let rows = path_rows(locations, keys);
    let moves = |r: &PathRow| {
        r.content.is_some()
            && rows.iter().any(|o| {
                !std::ptr::eq(o, r)
                    && o.category == Category::OnlyIn
                    && o.content == r.content
                    && o.presence & r.presence == 0
            })
    };
    let mut shapes: Vec<RowShape> = Vec::new();
    let mut move_groups: BTreeMap<&str, Vec<(usize, String)>> = BTreeMap::new();
    for row in &rows {
        if moves(row) {
            move_groups
                .entry(row.content.as_deref().unwrap())
                .or_default()
                .extend(row.files.iter().cloned());
        } else {
            shapes.push((row.category, false, row.files.iter().cloned().collect()));
        }
    }
    for files in move_groups.into_values() {
        let names: BTreeSet<String> = files
            .iter()
            .map(|(_, path)| {
                PathKey::from_stored(PathKey::from_stored(path.clone()).normalized(keys))
                    .file_name()
                    .to_owned()
            })
            .collect();
        let category = if names.len() == 1 {
            Category::Moved
        } else {
            Category::Renamed
        };
        shapes.push((category, false, files.into_iter().collect()));
    }
    shapes.sort();
    shapes
}

fn entry() -> impl Strategy<Value = Entry> {
    prop_oneof![
        6 => proptest::sample::select(&CONTENTS[..]).prop_map(Entry::File),
        2 => proptest::option::of(proptest::sample::select(&TARGETS[..])).prop_map(Entry::Link),
        1 => Just(Entry::Dir),
    ]
}

/// Chosen entries plus the directories holding them; an entry that holds others is a directory.
fn location() -> impl Strategy<Value = Location> {
    proptest::collection::btree_map(proptest::sample::select(&PATHS[..]), entry(), 0..6).prop_map(
        |chosen| {
            let mut location: Location = chosen
                .into_iter()
                .map(|(path, entry)| (path.to_owned(), entry))
                .collect();
            let parents: Vec<String> = location
                .keys()
                .filter_map(|path| path.rsplit_once('/').map(|(parent, _)| parent.to_owned()))
                .collect();
            for parent in parents {
                location.insert(parent, Entry::Dir);
            }
            location
        },
    )
}

/// The kind of every entry, by `(side, path key)`.
fn kinds_of(locations: &[Location]) -> BTreeMap<(usize, String), Entry> {
    locations
        .iter()
        .enumerate()
        .flat_map(|(side, entries)| {
            entries
                .iter()
                .map(move |(path, entry)| ((side, key_of(path)), *entry))
        })
        .collect()
}

fn locations() -> impl Strategy<Value = Vec<Location>> {
    proptest::collection::vec(location(), 2..=4)
}

fn key_options() -> impl Strategy<Value = KeyOptions> {
    (any::<bool>(), any::<bool>()).prop_map(|(nfc, case_insensitive)| KeyOptions {
        nfc,
        case_insensitive,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn engine_matches_the_reference_model(locations in locations(), keys in key_options()) {
        let world = world(&locations, true);
        let order: Vec<usize> = (0..world.sides).collect();
        let got = engine(&world, &order, CompareOptions::default().keys(keys));
        prop_assert_eq!(got, model(&locations, keys));
    }

    #[test]
    fn every_file_and_symlink_is_in_exactly_one_row_and_directories_in_at_most_one(
        locations in locations(),
        keys in key_options(),
    ) {
        let world = world(&locations, true);
        let order: Vec<usize> = (0..world.sides).collect();
        let rows = engine(&world, &order, CompareOptions::default().keys(keys));
        let kinds = kinds_of(&locations);
        let cells: Vec<&(usize, String)> = rows.iter().flat_map(|(_, _, files)| files).collect();
        let unique: BTreeSet<_> = cells.iter().collect();
        prop_assert_eq!(cells.len(), unique.len());
        let compared = |cells: &[&(usize, String)]| {
            cells.iter().filter(|cell| kinds[*cell] != Entry::Dir).count()
        };
        prop_assert_eq!(
            compared(&cells),
            kinds.values().filter(|entry| **entry != Entry::Dir).count()
        );
        for (category, _, files) in &rows {
            if *category == Category::Identical {
                let sides: BTreeSet<usize> = files.iter().map(|(s, _)| *s).collect();
                prop_assert_eq!(sides.len(), world.sides, "identical rows cover every side");
            }
        }
    }

    #[test]
    fn identical_symlink_rows_share_one_known_target(locations in locations(), keys in key_options()) {
        let world = world(&locations, true);
        let order: Vec<usize> = (0..world.sides).collect();
        let kinds = kinds_of(&locations);
        for (category, _, files) in engine(&world, &order, CompareOptions::default().keys(keys)) {
            let entries: Vec<Entry> = files.iter().map(|cell| kinds[cell]).collect();
            if category != Category::Identical || !matches!(entries[0], Entry::Link(_)) {
                continue;
            }
            let sides: BTreeSet<usize> = files.iter().map(|(s, _)| *s).collect();
            prop_assert_eq!(sides.len(), world.sides);
            let targets: BTreeSet<Entry> = entries.into_iter().collect();
            prop_assert_eq!(targets.len(), 1);
            prop_assert!(!targets.contains(&Entry::Link(None)));
        }
    }

    #[test]
    fn directory_rows_are_only_in_or_kind_clashes(locations in locations(), keys in key_options()) {
        let world = world(&locations, true);
        let order: Vec<usize> = (0..world.sides).collect();
        let kinds = kinds_of(&locations);
        for (category, _, files) in engine(&world, &order, CompareOptions::default().keys(keys)) {
            let entries: BTreeSet<&str> = files.iter().map(|cell| kinds[cell].kind().name()).collect();
            if !entries.contains("dir") {
                continue;
            }
            if entries.len() == 1 {
                prop_assert_eq!(category, Category::OnlyIn);
            } else {
                prop_assert!(matches!(category, Category::Different | Category::Collision), "{category:?}");
            }
        }
    }

    #[test]
    fn side_order_does_not_change_categories(locations in locations(), keys in key_options()) {
        let world = world(&locations, true);
        let forward: Vec<usize> = (0..world.sides).collect();
        let backward: Vec<usize> = forward.iter().rev().copied().collect();
        let options = CompareOptions::default().keys(keys);
        prop_assert_eq!(engine(&world, &forward, options.clone()), engine(&world, &backward, options));
    }

    #[test]
    fn fast_mode_only_turns_unverified_rows_into_assumed_ones(locations in locations(), keys in key_options()) {
        let world = world(&locations, false);
        let order: Vec<usize> = (0..world.sides).collect();
        let by_files = |rows: Vec<RowShape>| -> BTreeMap<BTreeSet<(usize, String)>, (Category, bool)> {
            rows.into_iter().map(|(category, assumed, files)| (files, (category, assumed))).collect()
        };
        let strict = by_files(engine(&world, &order, CompareOptions::default().keys(keys)));
        let fast = by_files(engine(&world, &order, CompareOptions::default().keys(keys).assume_equal_within(Duration::ZERO)));
        prop_assert_eq!(strict.keys().collect::<Vec<_>>(), fast.keys().collect::<Vec<_>>(), "same rows");
        let kinds = kinds_of(&locations);
        for (files, (s_cat, s_assumed)) in &strict {
            let (f_cat, f_assumed) = fast[files];
            prop_assert!(!s_assumed);
            if !files.iter().all(|cell| matches!(kinds[cell], Entry::File(_))) {
                prop_assert_eq!((*s_cat, *s_assumed), (f_cat, f_assumed), "fast mode is for files only");
                continue;
            }
            prop_assert!(!matches!(s_cat, Category::Identical | Category::Moved | Category::Renamed), "nothing is verified without digests");
            if *s_cat == Category::Unverified {
                prop_assert!(f_assumed && matches!(f_cat, Category::Identical | Category::OnlyIn));
            } else {
                prop_assert_eq!((*s_cat, *s_assumed), (f_cat, f_assumed));
            }
        }
    }
}
