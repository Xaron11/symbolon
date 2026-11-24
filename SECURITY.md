# Security and privacy

## Reporting

Report vulnerabilities privately through GitHub security advisories on this repository, not in public issues.

## What Symbolon stores and exposes

- **Catalogs are sensitive.** The catalog database and exported `.symbolon` files contain full relative paths, sizes, timestamps and content hashes of every scanned file. Share them only as you would share a directory listing of the data itself.
- **Hashes reveal content.** Anyone holding a catalog can check whether a known file (with a known hash) exists in a scanned location.
- **ADB grants broad device access.** Symbolon uses the system `adb` and its existing authorization. It never authorizes devices on its own or stores ADB keys.
- **Pushed binaries are removed.** The Android agent is pushed to `/data/local/tmp` for a scan and deleted afterwards unless you explicitly opt to keep it.
- **Writes are opt-in.** Until the write milestone, Symbolon is read-only. Write operations will always be planned, reviewed, and reversible (quarantine instead of delete).
