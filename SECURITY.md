# Security policy

Do not use pre-1.0 LODE releases for installing untrusted packages on sensitive machines.

## Current protections

- HTTPS is used for HTTP(S) registry/artifact downloads through reqwest + rustls.
- SHA-256 artifact verification occurs before extraction.
- ZIP entries containing parent-directory traversal are rejected.
- Packages are installed to a user-local directory by default.

## Not yet implemented

- signed registry metadata
- package signatures
- key rotation / revocation
- malware scanning
- sandboxed package install scripts (LODE intentionally has no install scripts)
- formal supply-chain provenance

Report security issues privately to the repository owner rather than opening a public issue when exploitation details are involved.
