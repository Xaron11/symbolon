//! Property tests: the comparison engine against a small reference model (ADR-0010).
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

/// Paths chosen to collide: case variants, NFC/NFD spellings, same names in other
/// directories (moves) and other names (renames).
const PATHS: [&str; 10] = [
    "a",
    "A",
    "b",
    "café",
    "cafe\u{301}",
    "d/a",
    "d/A",
    "e/a",
    "x.txt",
    "X.TXT",
];
/// `p` and `q` have the same size but different content.
const CONTENTS: [&str; 3] = ["p", "q", "rr"];

type Location = BTreeMap<&'static str, &'static str>;

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
        for (path, contents) in files {
            let rel = RelPath::from_path(path.as_ref()).unwrap();
            let key = rel.key();
            let digest = hashed.then(|| {
                hash_bytes(HashAlgo::Blake3, contents.as_bytes())
                    .as_bytes()
                    .to_vec()
            });
            conn.execute(
                "INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, mtime_ns, hash_full, seen_scan_id)
                 VALUES (?1, ?2, ?3, ?4, 'file', ?5, 1000, ?6, 1)",
                params![location.id.get(), rel.as_bytes(), key.as_str(), key.parent(), contents.len() as u64, digest],
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

/// Files of one normalized path, by side.
type Group<'a> = BTreeMap<usize, Vec<(&'a str, &'a str)>>;

/// A path row of the model, before moves are matched.
struct PathRow {
    category: Category,
    presence: u64,
    /// The common content of an only-in row.
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

/// ADR-0010 path classification for fully hashed locations.
fn path_rows(locations: &[Location], keys: KeyOptions) -> Vec<PathRow> {
    let mut groups: BTreeMap<String, Group<'_>> = BTreeMap::new();
    for (side, files) in locations.iter().enumerate() {
        for (path, contents) in files {
            let norm = PathKey::from_stored(key_of(path)).normalized(keys);
            groups
                .entry(norm)
                .or_default()
                .entry(side)
                .or_default()
                .push((path, contents));
        }
    }
    groups
        .into_values()
        .map(|by_side| {
            let all: Vec<(usize, &str, &str)> = by_side
                .iter()
                .flat_map(|(side, list)| list.iter().map(|(p, c)| (*side, *p, *c)))
                .collect();
            let contents: BTreeSet<&str> = all.iter().map(|(_, _, c)| *c).collect();
            let sizes: BTreeSet<usize> = all.iter().map(|(_, _, c)| c.len()).collect();
            let category = if by_side.values().any(|list| list.len() > 1) {
                Category::Collision
            } else if by_side.len() >= 2 && (sizes.len() > 1 || contents.len() > 1) {
                Category::Different
            } else if by_side.len() == locations.len() {
                Category::Identical
            } else {
                Category::OnlyIn
            };
            PathRow {
                category,
                presence: by_side.keys().fold(0, |mask, side| mask | (1 << side)),
                content: (category == Category::OnlyIn).then(|| all[0].2.to_owned()),
                files: all
                    .iter()
                    .map(|(side, path, _)| (*side, key_of(path)))
                    .collect(),
            }
        })
        .collect()
}

/// The reference model: path rows, then only-in rows sharing content with an only-in row of
/// disjoint presence collapse into one moved or renamed row per content.
fn model(locations: &[Location], keys: KeyOptions) -> Vec<RowShape> {
    let rows = path_rows(locations, keys);
    let moves = |r: &PathRow| {
        r.category == Category::OnlyIn
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

fn location() -> impl Strategy<Value = Location> {
    proptest::collection::btree_map(
        proptest::sample::select(&PATHS[..]),
        proptest::sample::select(&CONTENTS[..]),
        0..6,
    )
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
    fn every_file_is_in_exactly_one_row(locations in locations(), keys in key_options()) {
        let world = world(&locations, true);
        let order: Vec<usize> = (0..world.sides).collect();
        let rows = engine(&world, &order, CompareOptions::default().keys(keys));
        let cells: Vec<&(usize, String)> = rows.iter().flat_map(|(_, _, files)| files).collect();
        let unique: BTreeSet<_> = cells.iter().collect();
        prop_assert_eq!(cells.len(), unique.len());
        prop_assert_eq!(cells.len(), locations.iter().map(BTreeMap::len).sum::<usize>());
        for (category, _, files) in &rows {
            if *category == Category::Identical {
                let sides: BTreeSet<usize> = files.iter().map(|(s, _)| *s).collect();
                prop_assert_eq!(sides.len(), world.sides, "identical rows cover every side");
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
        for (files, (s_cat, s_assumed)) in &strict {
            let (f_cat, f_assumed) = fast[files];
            prop_assert!(!s_assumed);
            prop_assert!(!matches!(s_cat, Category::Identical | Category::Moved | Category::Renamed), "nothing is verified without digests");
            if *s_cat == Category::Unverified {
                prop_assert!(f_assumed && matches!(f_cat, Category::Identical | Category::OnlyIn));
            } else {
                prop_assert_eq!((*s_cat, *s_assumed), (f_cat, f_assumed));
            }
        }
    }
}
