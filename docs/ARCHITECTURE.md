# LODE architecture

```text
                    GitHub
              ┌────────────────┐
              │ registry repo │  index.json
              └───────┬────────┘
                      │ metadata
                      ▼
┌───────────┐   ┌─────────────┐   ┌─────────────────┐
│   lode    │──▶│   resolver  │──▶│ release asset   │
│ CLI       │   │ SemVer      │   │ ZIP + SHA-256   │
└─────┬─────┘   └─────────────┘   └────────┬────────┘
      │                                     │
      ▼                                     ▼
 lode.toml                              verify bytes
 lode.lock                                   │
      │                                       ▼
      └──────────────────────────────▶ ~/.lode/packages
                                           │
                                           ▼
                                      ~/.lode/bin
```

## Core decisions

- **Binary/tool packages**, not source libraries.
- **Static registry** first: no database or always-on server.
- **GitHub Releases** for immutable-ish artifact distribution.
- **SHA-256 verification** before extraction.
- **User-local installs** to avoid root/admin requirements.
- **SemVer requirements** and lockfiles for repeatable project installs.

## Next hardening milestone

Signed metadata/artifacts should be added before calling a public registry 1.0. The current checksum layer detects accidental corruption and simple substitution, but a compromised registry author could replace both the URL and digest. Signatures solve that trust problem only when signing keys are separately protected and rotated.
