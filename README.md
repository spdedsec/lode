# LODE

**LODE** is a fast, cross-platform binary package manager written in Rust.

> Small CLI. Deterministic installs. Checksums first.

LODE is designed as a real deployable tool rather than a Cargo clone. It manages **prebuilt application/tool packages**. A package is a ZIP archive containing a `bin/` directory and package metadata. The public registry is intentionally static: package metadata can live in a GitHub repository while release artifacts live in GitHub Releases. That keeps the service cheap, inspectable, and easy to operate.

## What works

- `lode init`
- `lode add <name>@<semver-req>`
- `lode remove <name>`
- `lode install [name]`
- `lode update`
- `lode list`
- `lode search <query>`
- `lode info <name>`
- `lode uninstall <name>`
- checksum verification (SHA-256) before extraction
- archive path traversal protection
- Linux x86_64/aarch64 and Windows x86_64/aarch64 target selection
- user-local installation under `~/.lode` (Windows uses the user's home directory)
- configurable registry URL, including `file://` for local testing
- package creation with `lode pack`
- GitHub-oriented publishing workflow scaffold
- unit tests for version selection and CLI parsing

## Why this scope

A general OS package manager would immediately drag the project into distro databases, system permissions, signing infrastructure, native dependency solving, installers, and platform-specific policy. LODE instead owns the part we can make excellent: **portable application/tool packages** with deterministic metadata, release artifacts, checksums, local installation, and a simple registry.

## Quick start

Install Rust using rustup, then:

```bash
cargo build --release
cargo test
cargo run -- doctor
```

Create a project:

```bash
mkdir hello-app
cd hello-app
../target/release/lode init --name hello-app
```

Configure a local registry while developing:

```bash
lode config set-registry file:///absolute/path/to/index.json
```

Then:

```bash
lode search hello
lode add hello@^1.0
lode list
```

## Package layout

```text
hello-1.0.0/
├── lode-package.toml
├── bin/
│   └── hello(.exe)
└── share/
    └── README.md
```

The `bin/` directory is what LODE exposes through `~/.lode/bin`.

## Registry format

```json
{
  "version": 1,
  "packages": {
    "hello": [
      {
        "version": "1.0.0",
        "description": "Example command",
        "license": "MIT",
        "targets": {
          "x86_64-unknown-linux-gnu": {
            "url": "https://github.com/ORG/hello/releases/download/v1.0.0/hello-1.0.0-x86_64-unknown-linux-gnu.zip",
            "sha256": "..."
          }
        }
      }
    ]
  }
}
```

## Production release model

1. Build LODE on GitHub Actions.
2. Publish Linux/Windows binaries as GitHub Release assets.
3. Maintain a separate `lode-registry` repository containing `index.json`.
4. Each registry artifact entry includes a release URL and SHA-256 digest.
5. Clients fetch metadata and verify bytes before extracting them.

The client needs **no token for public installation**. A GitHub token is only required for publishing and updating registry repositories. See `docs/PUBLISHING.md`.

## Security model

LODE currently treats the registry metadata and artifact digest as the trust boundary. It refuses a package if its SHA-256 digest does not match the registry. ZIP extraction rejects `..` path traversal entries.

Before a public 1.0 release, add signed metadata/artifacts (Sigstore/cosign or Minisign), key rotation, immutable release policy, dependency provenance, and a documented threat model.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

## License

MIT.

## Installation from GitHub Releases

After you create your own GitHub repository and releases, set `LODE_REPO=OWNER/LODE` and use:

```bash
curl -fsSL https://raw.githubusercontent.com/OWNER/LODE/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
$env:LODE_REPO = "OWNER/LODE"
irm https://raw.githubusercontent.com/OWNER/LODE/main/scripts/install.ps1 | iex
```

The release workflow publishes SHA-256 checksums and the installers verify them before installing.
