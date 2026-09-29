# Publishing a LODE package

LODE uses GitHub Releases for binary artifacts and a separate static repository for the registry index.

## Credentials

You need a GitHub token with permission to create releases in the package repository and update the registry repository.

Set:

```bash
export GITHUB_TOKEN=ghp_or_fine_grained_token
export GITHUB_REPO=spdedsec/lode
export LODE_REGISTRY_REPO=spdedsec/lode-registry
export LODE_REGISTRY_PATH=index.json
```

On PowerShell:

```powershell
$env:GITHUB_TOKEN = "your-token"
$env:GITHUB_REPO = "spdedsec/lode"
$env:LODE_REGISTRY_REPO = "spdedsec/lode-registry"
$env:LODE_REGISTRY_PATH = "index.json"
```

The token should be a least-privilege fine-grained token scoped only to the repositories involved.

## Build a package

A package directory contains `lode-package.toml` plus the files that should be installed:

```text
example-tool/
├── lode-package.toml
├── bin/
│   └── example-tool
└── share/
    └── README.md
```

Create the artifact:

```bash
lode pack ./example-tool --output example-tool-1.0.0-x86_64-unknown-linux-gnu.zip
```

Then publish it:

```bash
lode publish example-tool-1.0.0-x86_64-unknown-linux-gnu.zip
```

The publisher creates a GitHub Release, uploads the artifact, computes SHA-256, and updates the registry index through GitHub's Contents API.

For a Windows artifact, repeat the build/publish process from a Windows GitHub runner or machine. The same release tag can contain multiple target assets; future releases should use the dedicated CI publisher to make multi-target publishing atomic.
