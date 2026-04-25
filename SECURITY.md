# Security Policy

Axion is in developer preview. Report security issues privately through GitHub Security Advisories when available; otherwise open a minimal issue that avoids exploit details and ask for a private disclosure channel.

## Security Model

Bridge access is deny-by-default through manifest capabilities. Remote navigation is denied unless explicitly allowed. Filesystem commands are restricted to Axion app-data paths and reject absolute paths, parent traversal, and symlinks.

See `docs/security.md` for the current public security model.
