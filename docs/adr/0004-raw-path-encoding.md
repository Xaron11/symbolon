# 0004. Raw path encoding and path keys across platforms

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

The catalog stores `rel_path_raw BLOB` (never lossy) and a normalized `path_key TEXT` used for matching (spec §3, §4.4). Linux and Android paths are arbitrary bytes except `/` and NUL; Windows paths are arbitrary sequences of UTF-16 code units (possibly unpaired surrogates). Open question §15.2 asks how Windows maps onto the raw-bytes column. `OsStr::as_encoded_bytes` is not an option: its encoding is explicitly unspecified and must not be persisted.

## Decision

1. **Relative paths only**, components joined with `/` in both columns. The location root is stored separately.
2. **`rel_path_raw` encoding depends on the source platform**, recorded per location as `path_encoding`:
   - `posix`: the exact bytes from `OsStrExt::as_bytes` (Linux, Android, macOS later).
   - `wtf8`: Windows UTF-16 code units from `OsStrExt::encode_wide`, encoded by our own `pathkey` module as WTF-8 (UTF-8 generalized to allow unpaired surrogates). Decoding back to `OsString` is exact. Valid Unicode paths come out as plain UTF-8, so `wtf8` and `posix` raw bytes are equal for ordinary names.
3. **`path_key`** is derived from the raw path as valid UTF-8 text:
   - Valid Unicode is kept as is; byte sequences that are not valid UTF-8 (posix) and unpaired surrogates (wtf8) are escaped as `%XX` per byte (`%` itself becomes `%25`), so the key stays injective and comparable across platforms.
   - Optional per-comparison transforms: Unicode NFC, then simple case fold. Keys for the transformed variants are computed at comparison time or cached in extra indexed columns; the stored `path_key` is the untransformed, escaped form.
4. Collisions created by case folding or NFC (two raw paths, one key) are reported as a distinct situation in comparisons, never silently merged.

## Consequences

- Comparing a Windows location with a Linux location works for all valid-Unicode names; names that are not valid Unicode can only match a location from the same platform family, which is correct.
- The encoder/decoder lives in `pathkey`, is pure and gets property tests (round-trip, injectivity of the escape).
- Windows support stays a later milestone, but nothing in the schema assumes Unix.
