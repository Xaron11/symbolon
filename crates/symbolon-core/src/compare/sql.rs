//! SQL behind [`super::Comparison`]: temporary tables, loading, classification, reading.

use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, OptionalExtension as _, named_params, params};

use super::{
    Category, Cell, CompareError, CompareOptions, Direction, DuplicateSet, DuplicateTotals, Row,
    RowFilter, RowOrder, Side, SideInfo, SideSet, Summary, Totals, parse_row_cursor, row_cursor,
};
use crate::catalog::CatalogError;
use crate::hash::{Digest, PartialDigest};
use crate::pathkey::{KeyOptions, PathKey, RelPath};
use crate::query::{Cursor, HashState, Page, PageOf};
use crate::source::EntryKind;

/// `sym_norm(key, nfc, case_insensitive)`: the normalized key; `sym_name(key)`: the last
/// component. Both are deterministic, so SQLite may use them in grouping.
pub(super) fn register_functions(conn: &Connection) -> rusqlite::Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    conn.create_scalar_function("sym_norm", 3, flags, |ctx| {
        let key: String = ctx.get(0)?;
        let options = KeyOptions {
            nfc: ctx.get(1)?,
            case_insensitive: ctx.get(2)?,
        };
        Ok(PathKey::from_stored(key).normalized(options))
    })?;
    conn.create_scalar_function("sym_name", 1, flags, |ctx| {
        let key: String = ctx.get(0)?;
        Ok(PathKey::from_stored(key).file_name().to_owned())
    })?;
    conn.create_scalar_function("sym_ext", 1, flags, |ctx| {
        let name: String = ctx.get(0)?;
        Ok(extension(&name))
    })
}

/// Lowercase extension of a file name without the dot; empty for none or for dotfiles.
fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_lowercase(),
        _ => String::new(),
    }
}

pub(super) fn resolve_side(conn: &Connection, side: &Side) -> Result<SideInfo, CompareError> {
    let (id, algo, encoding): (i64, String, String) = conn
        .query_row(
            "SELECT id, hash_algo, path_encoding FROM main.locations WHERE label = ?1",
            [&side.label],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| CatalogError::LocationNotFound(side.label.clone()))?;
    let corrupt =
        |what: &str| CatalogError::Corrupt(format!("{what} of location {:?}", side.label));
    Ok(SideInfo {
        side: side.clone(),
        hash_algo: algo.parse().map_err(|_| corrupt("hash algorithm"))?,
        path_encoding: encoding.parse().map_err(|_| corrupt("path encoding"))?,
        location_id: id,
    })
}

pub(super) fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TEMP TABLE cmp_files (
            id INTEGER PRIMARY KEY,
            side INTEGER NOT NULL,
            entry_id INTEGER NOT NULL,
            raw BLOB NOT NULL,
            rel TEXT NOT NULL,
            norm TEXT NOT NULL,
            kind TEXT NOT NULL,
            target BLOB,
            size INTEGER NOT NULL,
            mtime_ns INTEGER,
            partial BLOB,
            full BLOB,
            row_id INTEGER
        );
        CREATE TEMP TABLE cmp_rows (
            id INTEGER PRIMARY KEY,
            sort_key TEXT NOT NULL,
            category TEXT NOT NULL,
            presence INTEGER NOT NULL,
            assumed INTEGER NOT NULL,
            size INTEGER,
            max_size INTEGER,
            ext TEXT NOT NULL,
            digest BLOB,
            kinds INTEGER NOT NULL,
            entries INTEGER NOT NULL,
            bytes INTEGER NOT NULL
        );",
    )
}

/// Indexes are dropped before bulk inserts and built afterwards, which is much faster than
/// maintaining them row by row.
const FILE_INDEXES: &str = "
    CREATE INDEX temp.cmp_files_norm ON cmp_files (norm, side);
    CREATE INDEX temp.cmp_files_row ON cmp_files (row_id) WHERE row_id IS NOT NULL;";
/// Only hashing on demand and duplicate listings need these, so they are built on first use.
pub(super) const LOOKUP_INDEXES: &str = "
    CREATE INDEX IF NOT EXISTS temp.cmp_files_size ON cmp_files (size, side);
    CREATE INDEX IF NOT EXISTS temp.cmp_files_full ON cmp_files (side, full);";
const DROP_FILE_INDEXES: &str = "
    DROP INDEX IF EXISTS temp.cmp_files_norm;
    DROP INDEX IF EXISTS temp.cmp_files_size;
    DROP INDEX IF EXISTS temp.cmp_files_full;
    DROP INDEX IF EXISTS temp.cmp_files_row;";
const ROW_INDEXES: &str = "
    CREATE INDEX temp.cmp_rows_order ON cmp_rows (sort_key, id);
    CREATE INDEX temp.cmp_rows_digest ON cmp_rows (digest) WHERE digest IS NOT NULL;";
/// Indexes for the size orders, built on the first page read in that order (most comparisons
/// are never sorted by size) and dropped with the row indexes.
const SIZE_ASC_INDEX: &str = "CREATE INDEX IF NOT EXISTS temp.cmp_rows_size_asc
    ON cmp_rows (max_size IS NULL, max_size, sort_key, id)";
const SIZE_DESC_INDEX: &str = "CREATE INDEX IF NOT EXISTS temp.cmp_rows_size_desc
    ON cmp_rows (max_size IS NULL, max_size DESC, sort_key, id)";
const DROP_ROW_INDEXES: &str = "
    DROP INDEX IF EXISTS temp.cmp_rows_order;
    DROP INDEX IF EXISTS temp.cmp_rows_size_asc;
    DROP INDEX IF EXISTS temp.cmp_rows_size_desc;
    DROP INDEX IF EXISTS temp.cmp_rows_digest;";

/// Copies every side's files, symlinks and directories into `cmp_files` within one read
/// transaction (one snapshot).
pub(super) fn load(
    conn: &mut Connection,
    sides: &[SideInfo],
    options: &CompareOptions,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM temp.cmp_files; DELETE FROM temp.cmp_rows;")?;
    tx.execute_batch(DROP_FILE_INDEXES)?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO temp.cmp_files
                 (side, entry_id, raw, rel, norm, kind, target, size, mtime_ns, partial, full)
             SELECT :side, id, rel_path_raw, rel,
                    CASE WHEN :nfc OR :ci THEN sym_norm(rel, :nfc, :ci) ELSE rel END,
                    kind, link_target, size, mtime_ns, hash_partial, hash_full
             FROM (
                     SELECT id, rel_path_raw, kind, link_target, size, mtime_ns, hash_partial,
                            hash_full,
                            CASE WHEN :prefix = '' THEN path_key
                                 ELSE substr(path_key, length(:prefix) + 2) END AS rel
                     FROM main.entries
                     WHERE location_id = :location AND kind IN ('file', 'symlink', 'dir')
                       AND (:prefix = '' OR (path_key > :prefix || '/' AND path_key < :prefix || '0'))
             )",
        )?;
        for (index, side) in sides.iter().enumerate() {
            insert.execute(named_params! {
                ":side": index,
                ":location": side.location_id,
                ":prefix": side.side.subpath,
                ":nfc": options.keys.nfc,
                ":ci": options.keys.case_insensitive,
            })?;
        }
    }
    tx.execute_batch(FILE_INDEXES)?;
    tx.commit()
}

/// Classifies path rows and assigns every entry to its row: files as in ADR-0010, symlinks by
/// target, directories only when nothing compared lies beneath them, and kind clashes as
/// different (ADR-0019).
pub(super) fn classify(
    conn: &Connection,
    sides: usize,
    options: &CompareOptions,
) -> rusqlite::Result<()> {
    let all: i64 = (1_i64 << sides) - 1;
    let (fast, tolerance) = options.assume_equal_within.map_or((false, 0), |t| {
        (true, i64::try_from(t.as_nanos()).unwrap_or(i64::MAX))
    });
    conn.execute_batch(DROP_ROW_INDEXES)?;
    conn.execute(
        "INSERT INTO temp.cmp_rows
             (sort_key, category, presence, assumed, size, max_size, ext, digest, kinds, entries,
              bytes)
         SELECT norm, category, presence,
                only_files AND category IN ('identical', 'only_in') AND sides >= 2 AND fulls < n,
                CASE WHEN only_files AND sizes = 1 THEN size_max END,
                size_max,
                CASE WHEN files > 0 THEN sym_ext(sym_name(norm)) ELSE '' END,
                CASE WHEN only_files AND fulls = n AND distinct_full = 1 THEN any_full END,
                kinds, n, bytes
         FROM (
             SELECT *,
                 CASE
                     WHEN kinds = 2 THEN
                         CASE WHEN presence != :all AND NOT EXISTS (
                                  SELECT 1 FROM temp.cmp_files c
                                  WHERE c.norm > g.norm || '/' AND c.norm < g.norm || '0'
                                    AND (g.presence >> c.side) & 1 = 1)
                              THEN 'only_in' END
                     WHEN n > sides THEN 'collision'
                     WHEN kinds NOT IN (1, 2, 4) THEN 'different'
                     WHEN kinds = 4 THEN
                         CASE WHEN sides >= 2 AND min_target < max_target THEN 'different'
                              WHEN targets = n AND presence = :all THEN 'identical'
                              WHEN targets = n OR sides = 1 THEN 'only_in'
                              ELSE 'unverified' END
                     WHEN sides >= 2 AND (sizes > 1 OR distinct_full > 1 OR distinct_partial > 1)
                         THEN 'different'
                     WHEN fulls = n AND presence = :all THEN 'identical'
                     WHEN fulls = n OR sides = 1 THEN 'only_in'
                     WHEN :fast AND mtimes = n AND mtime_spread <= :tolerance
                         THEN CASE WHEN presence = :all THEN 'identical' ELSE 'only_in' END
                     ELSE 'unverified'
                 END AS category,
                 kinds = 1 AS only_files
             FROM (
                 SELECT norm,
                        count(*) AS n,
                        count(DISTINCT side) AS sides,
                        sum(DISTINCT 1 << side) AS presence,
                        max(kind = 'file') | (max(kind = 'dir') << 1) | (max(kind = 'symlink') << 2)
                            AS kinds,
                        count(CASE WHEN kind = 'file' THEN 1 END) AS files,
                        count(DISTINCT CASE WHEN kind = 'file' THEN size END) AS sizes,
                        max(CASE WHEN kind = 'file' THEN size END) AS size_max,
                        sum(CASE WHEN kind = 'file' THEN size ELSE 0 END) AS bytes,
                        count(full) AS fulls,
                        count(DISTINCT full) AS distinct_full,
                        max(full) AS any_full,
                        count(DISTINCT partial) AS distinct_partial,
                        count(mtime_ns) AS mtimes,
                        max(mtime_ns) - min(mtime_ns) AS mtime_spread,
                        count(target) AS targets,
                        min(target) AS min_target,
                        max(target) AS max_target
                 FROM temp.cmp_files
                 GROUP BY norm
             ) AS g
         )
         WHERE category IS NOT NULL
         ORDER BY norm",
        named_params! { ":all": all, ":fast": fast, ":tolerance": tolerance },
    )?;
    conn.execute_batch(ROW_INDEXES)?;
    match_moves(conn)
}

/// Replaces verified only-in rows that share a digest with a row of disjoint presence by one
/// moved or renamed row per digest (ADR-0010).
///
/// Files of path rows are found by `norm = sort_key`; only files of move rows carry a `row_id`.
fn match_moves(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.cmp_moving;
         CREATE TEMP TABLE cmp_moving AS
         SELECT r.id, r.sort_key, r.digest FROM temp.cmp_rows r
         WHERE r.category = 'only_in' AND r.digest IS NOT NULL
           AND EXISTS (
               SELECT 1 FROM temp.cmp_rows o
               WHERE o.digest = r.digest AND o.category = 'only_in' AND o.id != r.id
                 AND (o.presence & r.presence) = 0
           );
         CREATE INDEX temp.cmp_moving_key ON cmp_moving (sort_key);

         INSERT INTO temp.cmp_rows
             (sort_key, category, presence, assumed, size, max_size, ext, digest, kinds, entries,
              bytes)
         SELECT min(f.norm),
                CASE WHEN count(DISTINCT sym_name(f.norm)) = 1 THEN 'moved' ELSE 'renamed' END,
                sum(DISTINCT 1 << f.side), 0, max(f.size), max(f.size),
                sym_ext(sym_name(min(f.norm))), m.digest, 1, count(*), sum(f.size)
         FROM temp.cmp_moving m JOIN temp.cmp_files f ON f.norm = m.sort_key AND f.kind = 'file'
         GROUP BY m.digest
         ORDER BY min(f.norm);

         UPDATE temp.cmp_files SET row_id = (
             SELECT n.id FROM temp.cmp_rows n
             WHERE n.category IN ('moved', 'renamed')
               AND n.digest = (SELECT m.digest FROM temp.cmp_moving m WHERE m.sort_key = cmp_files.norm)
         )
         WHERE kind = 'file' AND norm IN (SELECT sort_key FROM temp.cmp_moving);

         DELETE FROM temp.cmp_rows WHERE id IN (SELECT id FROM temp.cmp_moving);
         DROP TABLE temp.cmp_moving;",
    )
}

/// A [`RowFilter`] translated into the parameters of [`FILTER`].
pub(super) struct FilterParams {
    categories: String,
    extension: Option<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    prefix: Option<String>,
    presence: Option<i64>,
    kinds: Option<i64>,
}

impl FilterParams {
    pub(super) fn new(filter: &RowFilter, options: &CompareOptions) -> Self {
        let prefix = filter.prefix.as_deref().map(|prefix| {
            RelPath::from_path(std::path::Path::new(prefix)).map_or_else(
                |_| PathKey::from_stored(prefix.to_owned()).normalized(options.keys),
                |rel| rel.key().normalized(options.keys),
            )
        });
        Self {
            categories: filter.categories.iter().fold(String::new(), |mut list, c| {
                list.push(',');
                list.push_str(c.name());
                list.push(',');
                list
            }),
            extension: filter.extension.clone(),
            min_size: filter.min_size,
            max_size: filter.max_size,
            prefix: prefix.filter(|p| !p.is_empty()),
            presence: filter.presence.map(|set| set.bits().cast_signed()),
            kinds: (!filter.kinds.is_empty()).then(|| {
                filter
                    .kinds
                    .iter()
                    .fold(0, |bits, kind| bits | kind_bit(*kind))
            }),
        }
    }
}

/// Row conditions shared by paging and summaries; every `NULL` parameter matches everything.
const FILTER: &str = "
    (:categories = '' OR instr(:categories, ',' || category || ',') > 0)
    AND (:ext IS NULL OR ext = :ext)
    AND (:min_size IS NULL OR max_size >= :min_size)
    AND (:max_size IS NULL OR max_size <= :max_size)
    AND (:prefix IS NULL OR sort_key = :prefix
         OR (sort_key > :prefix || '/' AND sort_key < :prefix || '0'))
    AND (:presence IS NULL OR presence = :presence)
    AND (:kinds IS NULL OR kinds & :kinds != 0)";

pub(super) fn summary(conn: &Connection, filter: &FilterParams) -> Result<Summary, CompareError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT category, count(*), sum(entries), sum(bytes), sum(assumed)
         FROM temp.cmp_rows WHERE {FILTER} GROUP BY category"
    ))?;
    let mut summary = Summary::default();
    let rows = stmt.query_map(
        named_params! {
            ":categories": filter.categories,
            ":ext": filter.extension,
            ":min_size": filter.min_size,
            ":max_size": filter.max_size,
            ":prefix": filter.prefix,
            ":presence": filter.presence,
            ":kinds": filter.kinds,
        },
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                Totals {
                    rows: row.get(1)?,
                    entries: row.get(2)?,
                    bytes: row.get(3)?,
                    assumed_rows: row.get(4)?,
                },
            ))
        },
    )?;
    for row in rows {
        let (category, totals) = row?;
        summary.categories.insert(category.parse()?, totals);
    }
    Ok(summary)
}

/// `ORDER BY` terms of `order`; every order ends in a unique key so pages never overlap. Each
/// matches an index term for term, so deep offsets skip index entries instead of sorting the
/// table.
fn order_by(order: RowOrder) -> &'static str {
    match order {
        RowOrder::Path(Direction::Asc) => "sort_key, id",
        RowOrder::Path(Direction::Desc) => "sort_key DESC, id DESC",
        RowOrder::Size(Direction::Asc) => "max_size IS NULL, max_size, sort_key, id",
        RowOrder::Size(Direction::Desc) => "max_size IS NULL, max_size DESC, sort_key, id",
    }
}

pub(super) fn rows(
    conn: &Connection,
    sides: &[SideInfo],
    filter: &FilterParams,
    order: RowOrder,
    page: &Page,
) -> Result<PageOf<Row>, CompareError> {
    let offset = page.checked_offset()?;
    if offset.is_none() && order != RowOrder::default() {
        return Err(CatalogError::Corrupt("cursor pages are path ordered".into()).into());
    }
    match order {
        RowOrder::Size(Direction::Asc) => conn.execute_batch(SIZE_ASC_INDEX)?,
        RowOrder::Size(Direction::Desc) => conn.execute_batch(SIZE_DESC_INDEX)?,
        RowOrder::Path(_) => {}
    }
    let (after_key, after_id) = page
        .after
        .as_ref()
        .map(parse_row_cursor)
        .transpose()?
        .unwrap_or_else(|| (String::new(), 0));
    let limit = page.limit.clamp(1, crate::query::MAX_PAGE);
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT id, sort_key, category, presence, assumed, size FROM temp.cmp_rows
         WHERE (:offset IS NOT NULL OR (sort_key, id) > (:key, :id)) AND {FILTER}
         ORDER BY {} LIMIT :limit OFFSET coalesce(:offset, 0)",
        order_by(order)
    ))?;
    let heads = stmt
        .query_map(
            named_params! {
                ":offset": offset.map(u64::cast_signed),
                ":key": after_key,
                ":id": after_id,
                ":categories": filter.categories,
                ":ext": filter.extension,
                ":min_size": filter.min_size,
                ":max_size": filter.max_size,
                ":prefix": filter.prefix,
                ":presence": filter.presence,
                ":kinds": filter.kinds,
                ":limit": limit,
            },
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<u64>>(5)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let mut move_cells = conn.prepare_cached(
        "SELECT side, raw, rel, kind, target, size, mtime_ns, partial, full FROM temp.cmp_files
         WHERE row_id = ?1 ORDER BY side, rel",
    )?;
    let mut path_cells = conn.prepare_cached(
        "SELECT side, raw, rel, kind, target, size, mtime_ns, partial, full FROM temp.cmp_files
         WHERE norm = ?1 AND row_id IS NULL ORDER BY side, rel",
    )?;
    let mut items = Vec::with_capacity(heads.len());
    let mut last = None;
    for (id, key, category, presence, assumed, size) in heads {
        let category: Category = category.parse()?;
        let cells = if matches!(category, Category::Moved | Category::Renamed) {
            move_cells.query_map(params![id], RawCell::from_row)?
        } else {
            path_cells.query_map(params![key], RawCell::from_row)?
        }
        .map(|cell| cell?.into_cell(sides))
        .collect::<Result<Vec<_>, CompareError>>()?;
        last = Some((key.clone(), id));
        items.push(Row {
            category,
            key,
            presence: SideSet::from_bits(presence.cast_unsigned()),
            assumed,
            size,
            cells,
        });
    }
    let next = (offset.is_none() && items.len() == limit as usize)
        .then(|| last.map(|(key, id)| row_cursor(&key, id)))
        .flatten();
    Ok(PageOf { items, next })
}

pub(super) fn duplicates(
    conn: &Connection,
    sides: &[SideInfo],
    side: usize,
    page: &Page,
) -> Result<PageOf<DuplicateSet>, CompareError> {
    conn.execute_batch(LOOKUP_INDEXES)?;
    let info = sides
        .get(side)
        .ok_or_else(|| CatalogError::Corrupt(format!("side {side} out of range")))?;
    let after = page
        .after
        .as_ref()
        .map(|c| hex_decode(c.as_str()))
        .transpose()?
        .unwrap_or_default();
    let limit = page.limit.clamp(1, crate::query::MAX_PAGE);
    let groups = conn
        .prepare_cached(
            "SELECT full, max(size) FROM temp.cmp_files
             WHERE side = ?1 AND kind = 'file' AND full IS NOT NULL AND size > 0 AND full > ?2
             GROUP BY full HAVING count(*) > 1 ORDER BY full LIMIT ?3",
        )?
        .query_map(params![side, after, limit], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, u64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut cells_stmt = conn.prepare_cached(
        "SELECT side, raw, rel, kind, target, size, mtime_ns, partial, full FROM temp.cmp_files
         WHERE side = ?1 AND kind = 'file' AND full = ?2 ORDER BY rel",
    )?;
    let mut items = Vec::with_capacity(groups.len());
    for (raw_digest, size) in groups {
        let cells = cells_stmt
            .query_map(params![side, raw_digest], RawCell::from_row)?
            .map(|cell| cell?.into_cell(sides))
            .collect::<Result<Vec<_>, CompareError>>()?;
        let digest = Digest::from_bytes(info.hash_algo, &raw_digest)
            .map_err(|e| CatalogError::Corrupt(e.to_string()))?;
        items.push(DuplicateSet {
            side,
            digest,
            size,
            cells,
        });
    }
    let next = (items.len() == limit as usize)
        .then(|| {
            items
                .last()
                .map(|set| Cursor::from_string(set.digest.to_hex()))
        })
        .flatten();
    Ok(PageOf { items, next })
}

pub(super) fn duplicate_totals(
    conn: &Connection,
    sides: usize,
) -> Result<Vec<DuplicateTotals>, CompareError> {
    conn.execute_batch(LOOKUP_INDEXES)?;
    let mut totals = vec![DuplicateTotals::default(); sides];
    let mut stmt = conn.prepare_cached(
        "SELECT side, count(*), sum(n), sum((n - 1) * size) FROM (
             SELECT side, count(*) AS n, max(size) AS size FROM temp.cmp_files
             WHERE kind = 'file' AND full IS NOT NULL AND size > 0 GROUP BY side, full HAVING n > 1
         ) GROUP BY side",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, usize>(0)?,
            DuplicateTotals {
                groups: row.get(1)?,
                files: row.get(2)?,
                redundant_bytes: row.get(3)?,
            },
        ))
    })?;
    for row in rows {
        let (side, side_totals) = row?;
        if let Some(slot) = totals.get_mut(side) {
            *slot = side_totals;
        }
    }
    Ok(totals)
}

/// The bit of `kind` in `cmp_rows.kinds`.
const fn kind_bit(kind: EntryKind) -> i64 {
    match kind {
        EntryKind::File => 1,
        EntryKind::Dir => 2,
        EntryKind::Symlink => 4,
        EntryKind::Other => 0,
    }
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, CatalogError> {
    let invalid = || CatalogError::Corrupt("invalid cursor".into());
    if !hex.len().is_multiple_of(2) {
        return Err(invalid());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(hex.get(i..i + 2).ok_or_else(invalid)?, 16).map_err(|_| invalid())
        })
        .collect()
}

struct RawCell {
    side: usize,
    raw: Vec<u8>,
    rel: String,
    kind: String,
    target: Option<Vec<u8>>,
    size: u64,
    mtime_ns: Option<i64>,
    partial: Option<Vec<u8>>,
    full: Option<Vec<u8>>,
}

impl RawCell {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            side: row.get(0)?,
            raw: row.get(1)?,
            rel: row.get(2)?,
            kind: row.get(3)?,
            target: row.get(4)?,
            size: row.get(5)?,
            mtime_ns: row.get(6)?,
            partial: row.get(7)?,
            full: row.get(8)?,
        })
    }

    fn into_cell(self, sides: &[SideInfo]) -> Result<Cell, CompareError> {
        let info = sides
            .get(self.side)
            .ok_or_else(|| CatalogError::Corrupt(format!("side {} out of range", self.side)))?;
        let rel = self.rel;
        let corrupt = |e: &dyn std::fmt::Display| CatalogError::Corrupt(format!("{rel:?}: {e}"));
        let digest = self
            .full
            .map(|bytes| Digest::from_bytes(info.hash_algo, &bytes))
            .transpose()
            .map_err(|e| corrupt(&e))?;
        let partial = self
            .partial
            .map(|bytes| PartialDigest::from_bytes(info.hash_algo, &bytes))
            .transpose()
            .map_err(|e| corrupt(&e))?;
        let hash_state = match (&digest, &partial) {
            (Some(_), _) => HashState::Verified,
            (None, Some(_)) => HashState::Partial,
            (None, None) => HashState::Unhashed,
        };
        Ok(Cell {
            side: self.side,
            path: RelPath::from_raw(info.path_encoding, self.raw).map_err(|e| corrupt(&e))?,
            key: PathKey::from_stored(rel.clone()),
            kind: self.kind.parse().map_err(|e| corrupt(&e))?,
            link_target: self.target,
            size: self.size,
            mtime_ns: self.mtime_ns,
            hash_state,
            digest,
            partial,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_normalize_and_split_keys() {
        let conn = Connection::open_in_memory().unwrap();
        register_functions(&conn).unwrap();
        let (norm, name): (String, String) = conn
            .query_row(
                "SELECT sym_norm('Cafe\u{301}/README.md', 1, 1), sym_name('a/b/c.txt')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((norm.as_str(), name.as_str()), ("café/readme.md", "c.txt"));
    }

    #[test]
    fn extensions_are_lowercase_and_ignore_dotfiles() {
        assert_eq!(extension("IMG_1.JPG"), "jpg");
        assert_eq!(extension("archive.tar.gz"), "gz");
        assert_eq!(extension(".bashrc"), "");
        assert_eq!(extension("README"), "");
    }
}
