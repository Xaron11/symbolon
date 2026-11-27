# Security and privacy

## Reporting

Report vulnerabilities privately through [GitHub private vulnerability reporting](https://github.com/Xaron11/symbolon/security/advisories/new), not in public issues.

## What Symbolon stores and exposes

- **Catalogs are sensitive.** The catalog database and exported `.symbolon` files contain the relative path, size, timestamp and content hash of every scanned file. They also contain each location's absolute root path (which usually includes your user name) and its ignore patterns. Share them only as you would share a directory listing of the data itself.
- **Hashes reveal content.** Anyone holding a catalog can check whether a known file (with a known hash) exists in a scanned location.
- **ADB grants broad device access.** Symbolon uses the system `adb` and its existing authorization. It never authorizes devices on its own or stores ADB keys.
- **Pushed binaries are removed.** The Android agent is pushed to `/data/local/tmp` for a scan and deleted afterwards, also when the scan fails. Phone scans and the development recipe `just android-probe` both follow this rule.
- **Writes are opt-in.** Until the write milestone, Symbolon is read-only. Write operations will always be planned, reviewed, and reversible (quarantine instead of delete).
