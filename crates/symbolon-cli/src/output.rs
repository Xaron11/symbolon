//! Output formats (ADR-0011): tables for people, JSON and CSV for scripts. Only results go to
//! stdout.

use std::io::{self, Write};
use std::time::SystemTime;

use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    Table,
    Json,
    Csv,
}

/// A result printable in every format: serialized as it is for JSON and CSV (exact values,
/// stable names), shown as friendly cells in tables.
pub(crate) trait Record: Serialize {
    /// Field names, in serialization order; the CSV header even when there are no records.
    const FIELDS: &'static [&'static str];
    /// Table column headers.
    const HEADERS: &'static [&'static str];
    /// Table cells, in the order of `HEADERS`.
    fn cells(&self) -> Vec<String>;
}

/// Writes `records` to stdout: `{"<name>": [...]}` in JSON.
pub(crate) fn list<R: Record>(format: Format, name: &str, records: &[R]) -> anyhow::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    match format {
        Format::Table => table(&mut out, R::HEADERS, records.iter().map(Record::cells))?,
        Format::Json => {
            serde_json::to_writer_pretty(&mut out, &Named(name, records))?;
            writeln!(out)?;
        }
        Format::Csv => {
            let mut writer = csv::WriterBuilder::new()
                .has_headers(false)
                .from_writer(&mut out);
            writer.write_record(R::FIELDS)?;
            for record in records {
                writer.serialize(record)?;
            }
            writer.flush()?;
        }
    }
    out.flush()?;
    Ok(())
}

/// `{"<name>": value}`, serialized directly so fields keep their declaration order.
struct Named<'a, T: ?Sized>(&'a str, &'a T);

impl<T: Serialize + ?Sized> Serialize for Named<'_, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(self.0, self.1)?;
        map.end()
    }
}

/// Columns padded to their widest cell, two spaces apart.
fn table(
    out: &mut impl Write,
    headers: &[&str],
    rows: impl Iterator<Item = Vec<String>>,
) -> io::Result<()> {
    let rows: Vec<Vec<String>> = rows.collect();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |out: &mut dyn Write, cells: &mut dyn Iterator<Item = &str>| -> io::Result<()> {
        let mut text = String::new();
        for (cell, width) in cells.zip(&widths) {
            text.push_str(cell);
            text.extend(std::iter::repeat_n(' ', width - cell.chars().count() + 2));
        }
        writeln!(out, "{}", text.trim_end())
    };
    line(out, &mut headers.iter().copied())?;
    for row in &rows {
        line(out, &mut row.iter().map(String::as_str))?;
    }
    Ok(())
}

/// Sizes for people: `512 B`, `1.5 KiB`, `3.2 GiB`.
pub(crate) fn size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "one decimal is shown; precision beyond 2^52 bytes does not matter"
    )]
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// RFC 3339 in UTC, to the second; times before 1970 are shown as the epoch.
pub(crate) fn time(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// Checks that `R::FIELDS` names the serialized fields in order; every record type calls this
/// in its tests.
#[cfg(test)]
pub(crate) fn assert_fields<R: Record>(sample: &R) {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.serialize(sample).unwrap();
    let text = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(text.lines().next(), Some(R::FIELDS.join(",").as_str()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_rfc3339_utc() {
        let at = |secs: u64| time(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs));
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(at(1_709_251_199), "2024-02-29T23:59:59Z");
        assert_eq!(at(1_798_761_600), "2027-01-01T00:00:00Z");
        assert_eq!(at(4_102_444_800), "2100-01-01T00:00:00Z");
        assert_eq!(
            time(SystemTime::UNIX_EPOCH - std::time::Duration::from_secs(1)),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn sizes_use_binary_units() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1024), "1.0 KiB");
        assert_eq!(size(1536), "1.5 KiB");
        assert_eq!(size(5 * 1024 * 1024 * 1024), "5.0 GiB");
        assert_eq!(size(u64::MAX), "16.0 EiB");
    }

    #[test]
    fn tables_align_columns_and_trim_line_ends() {
        let mut out = Vec::new();
        let rows = vec![
            vec!["a".to_owned(), "long cell".to_owned(), String::new()],
            vec!["wider".to_owned(), "x".to_owned(), "ü".to_owned()],
        ];
        table(&mut out, &["one", "two", "three"], rows.into_iter()).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "one    two        three\na      long cell\nwider  x          ü\n"
        );
    }
}
