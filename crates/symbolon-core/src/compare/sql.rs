//! SQL behind [`super::Comparison`]: temporary tables, loading, classification, reading.

use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, OptionalExtension as _, named_params, params};

use super::{
    Cell, CompareError, CompareOptions, DuplicateSet, DuplicateTotals, Row, RowFilter, Side,
    SideInfo, SideSet, Summary, Totals, parse_row_cursor, row_cursor,
};
use crate::catalog::CatalogError;
use crate::hash::{Digest, PartialDigest};
use crate::pathkey::{KeyOptions, PathKey, RelPath};
use crate::query::{Cursor, HashState, Page, PageOf};

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
            name TEXT NOT NULL,
            size INTEGER NOT NULL,
            mtime_ns INTEGER,
            partial BLOB,
            full BLOB,
            row_id INTEGER
        );
        CREATE INDEX temp.cmp_files_norm ON cmp_files (norm, side);
        CREATE INDEX temp.cmp_files_row ON cmp_files (row_id, side, rel);
        CREATE INDEX temp.cmp_files_side_full ON cmp_files (side, full);
        CREATE INDEX temp.cmp_files_side_size ON cmp_files (side, size);
        CREATE TEMP TABLE cmp_rows (
            id INTEGER PRIMARY KEY,
            sort_key TEXT NOT NULL,
            category TEXT NOT NULL,
            presence INTEGER NOT NULL,
            assumed INTEGER NOT NULL,
            size INTEGER,
            max_size INTEGER NOT NULL,
            ext TEXT NOT NULL,
            digest BLOB,
            files INTEGER NOT NULL,
            bytes INTEGER NOT NULL
        );
        CREATE INDEX temp.cmp_rows_order ON cmp_rows (sort_key, id);
        CREATE INDEX temp.cmp_rows_digest ON cmp_rows (digest) WHERE digest IS NOT NULL;",
    )
}

/// Copies every side's files into `cmp_files` within one read transaction (one snapshot).
pub(super) fn load(
    conn: &mut Connection,
    sides: &[SideInfo],
    options: &CompareOptions,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM temp.cmp_files; DELETE FROM temp.cmp_rows;")?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO temp.cmp_files (side, entry_id, raw, rel, norm, name, size, mtime_ns, partial, full)
             SELECT :side, id, rel_path_raw, rel, norm, sym_name(norm), size, mtime_ns, hash_partial, hash_full
             FROM (
                 SELECT id, rel_path_raw, rel, sym_norm(rel, :nfc, :ci) AS norm, size, mtime_ns,
                        hash_partial, hash_full
                 FROM (
                     SELECT id, rel_path_raw, size, mtime_ns, hash_partial, hash_full,
                            CASE WHEN :prefix = '' THEN path_key
                                 ELSE substr(path_key, length(:prefix) + 2) END AS rel
                     FROM main.entries
                     WHERE location_id = :location AND kind = 'file'
                       AND (:prefix = '' OR (path_key > :prefix || '/' AND path_key < :prefix || '0'))
                 )
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
    tx.commit()
}

/// Classifies path rows (ADR-0010, in order: collision, different, identical, only-in,
/// unverified or assumed) and assigns every file to its row.
pub(super) fn classify(
    conn: &Connection,
    sides: usize,
    options: &CompareOptions,
) -> rusqlite::Result<()> {
    let all: i64 = (1_i64 << sides) - 1;
    let (fast, tolerance) = options.assume_equal_within.map_or((false, 0), |t| {
        (true, i64::try_from(t.as_nanos()).unwrap_or(i64::MAX))
    });
    conn.execute(
        "INSERT INTO temp.cmp_rows
             (sort_key, category, presence, assumed, size, max_size, ext, digest, files, bytes)
         SELECT norm, category, presence,
                category IN ('identical', 'only_in') AND sides >= 2 AND fulls < n,
                CASE WHEN sizes = 1 THEN size_max END,
                size_max,
                sym_ext(sym_name(norm)),
                CASE WHEN fulls = n AND distinct_full = 1 THEN any_full END,
                n, bytes
         FROM (
             SELECT *,
                 CASE
                     WHEN n > sides THEN 'collision'
                     WHEN sides >= 2 AND (sizes > 1 OR distinct_full > 1 OR distinct_partial > 1)
                         THEN 'different'
                     WHEN fulls = n AND presence = :all THEN 'identical'
                     WHEN fulls = n OR sides = 1 THEN 'only_in'
                     WHEN :fast AND mtimes = n AND mtime_spread <= :tolerance
                         THEN CASE WHEN presence = :all THEN 'identical' ELSE 'only_in' END
                     ELSE 'unverified'
                 END AS category
             FROM (
                 SELECT norm,
                        count(*) AS n,
                        count(DISTINCT side) AS sides,
                        sum(DISTINCT 1 << side) AS presence,
                        count(DISTINCT size) AS sizes,
                        max(size) AS size_max,
                        sum(size) AS bytes,
                        count(full) AS fulls,
                        count(DISTINCT full) AS distinct_full,
                        max(full) AS any_full,
                        count(DISTINCT partial) AS distinct_partial,
                        count(mtime_ns) AS mtimes,
                        max(mtime_ns) - min(mtime_ns) AS mtime_spread
                 FROM temp.cmp_files
                 GROUP BY norm
             )
         )
         ORDER BY norm",
        named_params! { ":all": all, ":fast": fast, ":tolerance": tolerance },
    )?;
    conn.execute(
        "UPDATE temp.cmp_files SET row_id =
             (SELECT id FROM temp.cmp_rows r WHERE r.sort_key = cmp_files.norm)",
        [],
    )?;
    match_moves(conn)
}

/// Replaces verified only-in rows that share a digest with a row of disjoint presence by one
/// moved or renamed row per digest (ADR-0010).
fn match_moves(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.cmp_moving;
         CREATE TEMP TABLE cmp_moving AS
         SELECT r.id, r.digest FROM temp.cmp_rows r
         WHERE r.category = 'only_in' AND r.digest IS NOT NULL
           AND EXISTS (
               SELECT 1 FROM temp.cmp_rows o
               WHERE o.digest = r.digest AND o.category = 'only_in' AND o.id != r.id
                 AND (o.presence & r.presence) = 0
           );
         CREATE INDEX temp.cmp_moving_id ON cmp_moving (id);

         INSERT INTO temp.cmp_rows
             (sort_key, category, presence, assumed, size, max_size, ext, digest, files, bytes)
         SELECT min(f.norm),
                CASE WHEN count(DISTINCT f.name) = 1 THEN 'moved' ELSE 'renamed' END,
                sum(DISTINCT 1 << f.side), 0, max(f.size), max(f.size),
                sym_ext(sym_name(min(f.norm))), m.digest, count(*), sum(f.size)
         FROM temp.cmp_moving m JOIN temp.cmp_files f ON f.row_id = m.id
         GROUP BY m.digest
         ORDER BY min(f.norm);

         UPDATE temp.cmp_files SET row_id = (
             SELECT n.id FROM temp.cmp_rows n
             WHERE n.category IN ('moved', 'renamed')
               AND n.digest = (SELECT m.digest FROM temp.cmp_moving m WHERE m.id = cmp_files.row_id)
         )
         WHERE row_id IN (SELECT id FROM temp.cmp_moving);

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
    AND (:presence IS NULL OR presence = :presence)";

pub(super) fn summary(conn: &Connection, filter: &FilterParams) -> Result<Summary, CompareError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT category, count(*), sum(files), sum(bytes), sum(assumed)
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
        },
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                Totals {
                    rows: row.get(1)?,
                    files: row.get(2)?,
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

pub(super) fn rows(
    conn: &Connection,
    sides: &[SideInfo],
    filter: &FilterParams,
    page: &Page,
) -> Result<PageOf<Row>, CompareError> {
    let (after_key, after_id) = page
        .after
        .as_ref()
        .map(parse_row_cursor)
        .transpose()?
        .unwrap_or_else(|| (String::new(), 0));
    let limit = page.limit.clamp(1, crate::query::MAX_PAGE);
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT id, sort_key, category, presence, assumed, size FROM temp.cmp_rows
         WHERE (sort_key, id) > (:key, :id) AND {FILTER}
         ORDER BY sort_key, id LIMIT :limit"
    ))?;
    let heads = stmt
        .query_map(
            named_params! {
                ":key": after_key,
                ":id": after_id,
                ":categories": filter.categories,
                ":ext": filter.extension,
                ":min_size": filter.min_size,
                ":max_size": filter.max_size,
                ":prefix": filter.prefix,
                ":presence": filter.presence,
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

    let mut cells_stmt = conn.prepare_cached(
        "SELECT side, raw, rel, size, mtime_ns, partial, full FROM temp.cmp_files
         WHERE row_id = ?1 ORDER BY side, rel",
    )?;
    let mut items = Vec::with_capacity(heads.len());
    let mut last = None;
    for (id, key, category, presence, assumed, size) in heads {
        let cells = cells_stmt
            .query_map(params![id], RawCell::from_row)?
            .map(|cell| cell?.into_cell(sides))
            .collect::<Result<Vec<_>, CompareError>>()?;
        last = Some((key.clone(), id));
        items.push(Row {
            category: category.parse()?,
            key,
            presence: SideSet::from_bits(presence.cast_unsigned()),
            assumed,
            size,
            cells,
        });
    }
    let next = (items.len() == limit as usize)
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
             WHERE side = ?1 AND full IS NOT NULL AND size > 0 AND full > ?2
             GROUP BY full HAVING count(*) > 1 ORDER BY full LIMIT ?3",
        )?
        .query_map(params![side, after, limit], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, u64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut cells_stmt = conn.prepare_cached(
        "SELECT side, raw, rel, size, mtime_ns, partial, full FROM temp.cmp_files
         WHERE side = ?1 AND full = ?2 ORDER BY rel",
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
    let mut totals = vec![DuplicateTotals::default(); sides];
    let mut stmt = conn.prepare_cached(
        "SELECT side, count(*), sum(n), sum((n - 1) * size) FROM (
             SELECT side, count(*) AS n, max(size) AS size FROM temp.cmp_files
             WHERE full IS NOT NULL AND size > 0 GROUP BY side, full HAVING n > 1
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
            size: row.get(3)?,
            mtime_ns: row.get(4)?,
            partial: row.get(5)?,
            full: row.get(6)?,
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
