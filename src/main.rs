use anyhow::{Context, Result, bail};
use base64::Engine;
use clap::{Parser, Subcommand};
use dirs::{cache_dir, config_dir, home_dir};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

const APP: &str = "lode";
const DEFAULT_REGISTRY: &str =
    "https://raw.githubusercontent.com/spdedsec/lode-registry/main/index.json";

#[derive(Parser, Debug)]
#[command(name = APP, version, about = "Fast, cross-platform binary package manager")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand, Debug)]
enum CommandKind {
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Add {
        package: String,
    },
    Remove {
        package: String,
    },
    Install {
        package: Option<String>,
    },
    Update,
    List,
    Search {
        query: String,
    },
    Info {
        package: String,
    },
    Uninstall {
        package: String,
    },
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Pack {
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Publish {
        artifact: PathBuf,
        #[arg(long)]
        registry: Option<String>,
    },
    Doctor,
}

#[derive(Subcommand, Debug)]
enum CacheCommand {
    Path,
    Clear,
}
#[derive(Subcommand, Debug)]
enum ConfigCommand {
    Show,
    SetRegistry { url: String },
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ProjectManifest {
    package: ProjectPackage,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
}
#[derive(Debug, Serialize, Deserialize, Default)]
struct ProjectPackage {
    name: String,
    version: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Lockfile {
    version: u32,
    packages: Vec<LockedPackage>,
}
#[derive(Debug, Serialize, Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
    target: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Registry {
    version: u32,
    packages: BTreeMap<String, Vec<RegistryRelease>>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RegistryRelease {
    #[serde(default)]
    name: String,
    version: String,
    description: String,
    #[serde(default)]
    license: String,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    targets: BTreeMap<String, Artifact>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Artifact {
    url: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Config {
    registry: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Init { path, name } => init_project(&path, name),
        CommandKind::Add { package } => add_dependency(&package),
        CommandKind::Remove { package } => remove_dependency(&package),
        CommandKind::Install { package } => install(package.as_deref()),
        CommandKind::Update => update(),
        CommandKind::List => list_installed(),
        CommandKind::Search { query } => search(&query),
        CommandKind::Info { package } => info(&package),
        CommandKind::Uninstall { package } => uninstall(&package),
        CommandKind::Cache { command } => cache_cmd(command),
        CommandKind::Config { command } => config_cmd(command),
        CommandKind::Pack { path, output } => pack(&path, output),
        CommandKind::Publish { artifact, registry } => publish(&artifact, registry),
        CommandKind::Doctor => doctor(),
    }
}

fn project_file() -> PathBuf {
    PathBuf::from("lode.toml")
}
fn lock_file() -> PathBuf {
    PathBuf::from("lode.lock")
}
fn config_file() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP)
        .join("config.toml")
}
fn store_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lode")
}
fn cache_path() -> PathBuf {
    cache_dir().unwrap_or_else(|| PathBuf::from(".")).join(APP)
}

fn load_config() -> Result<Config> {
    let p = config_file();
    if !p.exists() {
        return Ok(Config {
            registry: Some(DEFAULT_REGISTRY.into()),
        });
    }
    Ok(toml::from_str(&fs::read_to_string(p)?)?)
}
fn save_config(c: &Config) -> Result<()> {
    let p = config_file();
    fs::create_dir_all(p.parent().unwrap())?;
    fs::write(p, toml::to_string_pretty(c)?)?;
    Ok(())
}
fn registry_url() -> Result<String> {
    Ok(load_config()?
        .registry
        .unwrap_or_else(|| DEFAULT_REGISTRY.into()))
}

fn init_project(path: &Path, name: Option<String>) -> Result<()> {
    fs::create_dir_all(path)?;
    let root = fs::canonicalize(path)?;
    let n = name
        .or_else(|| root.file_name().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "app".into());
    let manifest = ProjectManifest {
        package: ProjectPackage {
            name: n.clone(),
            version: "0.1.0".into(),
            description: String::new(),
        },
        dependencies: BTreeMap::new(),
    };
    let p = root.join("lode.toml");
    if p.exists() {
        bail!("{} already exists", p.display());
    }
    fs::write(&p, toml::to_string_pretty(&manifest)?)?;
    println!("initialized {n} in {}", root.display());
    Ok(())
}

fn read_manifest() -> Result<ProjectManifest> {
    Ok(toml::from_str(
        &fs::read_to_string(project_file()).context("lode.toml not found; run `lode init`")?,
    )?)
}
fn write_manifest(m: &ProjectManifest) -> Result<()> {
    fs::write(project_file(), toml::to_string_pretty(m)?)?;
    Ok(())
}

fn parse_spec(spec: &str) -> Result<(String, String)> {
    if let Some((name, req)) = spec.rsplit_once('@') {
        if !name.is_empty() && !req.is_empty() {
            return Ok((name.into(), req.into()));
        }
    }
    Ok((spec.into(), "*".into()))
}
fn add_dependency(spec: &str) -> Result<()> {
    let (name, req) = parse_spec(spec)?;
    let mut m = read_manifest()?;
    m.dependencies.insert(name.clone(), req.clone());
    write_manifest(&m)?;
    println!("added {name} {req}");
    install(Some(&name))
}
fn remove_dependency(name: &str) -> Result<()> {
    let mut m = read_manifest()?;
    if m.dependencies.remove(name).is_none() {
        bail!("dependency `{name}` is not declared")
    }
    write_manifest(&m)?;
    println!("removed {name}");
    Ok(())
}

fn load_registry() -> Result<Registry> {
    let url = registry_url()?;
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(serde_json::from_str(&fs::read_to_string(path)?)?);
    }
    let body = reqwest::blocking::get(&url)
        .with_context(|| format!("fetching registry {url}"))?
        .error_for_status()?
        .text()?;
    serde_json::from_str(&body).context("invalid registry JSON")
}
fn target() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "aarch64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "aarch64")
    )))]
    {
        "unsupported"
    }
}

fn resolve(reg: &Registry, name: &str, req: &str) -> Result<RegistryRelease> {
    let versions = reg
        .packages
        .get(name)
        .with_context(|| format!("package `{name}` not found"))?;
    let requirement =
        VersionReq::parse(req).with_context(|| format!("invalid version requirement `{req}`"))?;
    versions
        .iter()
        .filter_map(|r| Version::parse(&r.version).ok().map(|v| (v, r)))
        .filter(|(v, _)| requirement.matches(v))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, r)| r.clone())
        .context("no compatible version found")
}

fn install(package: Option<&str>) -> Result<()> {
    install_with_mode(package, package.is_none())
}

fn install_with_mode(package: Option<&str>, use_lock: bool) -> Result<()> {
    let manifest = read_manifest()?;
    let reg = load_registry()?;
    let names: Vec<String> = match package {
        Some(p) => vec![p.into()],
        None => manifest.dependencies.keys().cloned().collect(),
    };
    let existing_lock: Option<Lockfile> = if use_lock && lock_file().exists() {
        Some(toml::from_str(&fs::read_to_string(lock_file())?)?)
    } else {
        None
    };
    let mut lock = Lockfile {
        version: 1,
        packages: Vec::new(),
    };
    let mut queue: Vec<(String, String)> = names
        .into_iter()
        .map(|n| {
            let req = manifest
                .dependencies
                .get(&n)
                .cloned()
                .unwrap_or_else(|| "*".into());
            (n, req)
        })
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    while let Some((name, req)) = queue.pop() {
        if seen.contains(&name) {
            continue;
        }
        let release = if let Some(existing) = existing_lock.as_ref().and_then(|l| {
            l.packages
                .iter()
                .find(|p| p.name == name && p.target == target())
        }) {
            reg.packages
                .get(&name)
                .and_then(|releases| releases.iter().find(|r| r.version == existing.version))
                .cloned()
                .with_context(|| {
                    format!(
                        "locked package {name}@{} is missing from registry",
                        existing.version
                    )
                })?
        } else {
            resolve(&reg, &name, &req)?
        };
        for (dep, dep_req) in &release.dependencies {
            queue.push((dep.clone(), dep_req.clone()));
        }
        seen.insert(name.clone());
        let artifact = release.targets.get(target()).with_context(|| {
            format!(
                "{name}@{} has no artifact for {}",
                release.version,
                target()
            )
        })?;
        let bytes = download_verified(&artifact.url, &artifact.sha256)?;
        let pkg_root = store_dir()
            .join("packages")
            .join(&name)
            .join(&release.version);
        if pkg_root.exists() {
            fs::remove_dir_all(&pkg_root)?;
        }
        fs::create_dir_all(&pkg_root)?;
        extract_zip(&bytes, &pkg_root)?;
        install_shims(&pkg_root)?;
        lock.packages.push(LockedPackage {
            name: name.clone(),
            version: release.version.clone(),
            target: target().into(),
            sha256: artifact.sha256.clone(),
        });
        println!("installed {name}@{}", release.version);
    }
    if package.is_none() {
        fs::write(lock_file(), toml::to_string_pretty(&lock)?)?;
    }
    Ok(())
}

fn update() -> Result<()> {
    if lock_file().exists() {
        fs::remove_file(lock_file())?;
    }
    install_with_mode(None, false)
}
fn download_verified(url: &str, expected: &str) -> Result<Vec<u8>> {
    let bytes = if let Some(path) = url.strip_prefix("file://") {
        fs::read(path)?
    } else {
        reqwest::blocking::get(url)?
            .error_for_status()?
            .bytes()?
            .to_vec()
    };
    let got = hex(&Sha256::digest(&bytes));
    if !expected.eq_ignore_ascii_case(&got) {
        bail!("checksum mismatch: expected {expected}, got {got}");
    }
    Ok(bytes)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        let rel = Path::new(f.name());
        if rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            bail!("unsafe archive path: {}", f.name());
        }
        let out = dest.join(rel);
        if f.is_dir() {
            fs::create_dir_all(&out)?
        } else {
            fs::create_dir_all(out.parent().unwrap())?;
            let mut file = fs::File::create(&out)?;
            std::io::copy(&mut f, &mut file)?;
        }
    }
    Ok(())
}
fn install_shims(root: &Path) -> Result<()> {
    let bin = root.join("bin");
    if !bin.exists() {
        return Ok(());
    }
    let out = store_dir().join("bin");
    fs::create_dir_all(&out)?;
    for entry in fs::read_dir(bin)? {
        let p = entry?.path();
        if p.is_file() {
            let name = p.file_name().unwrap();
            let dst = out.join(name);
            copy_or_link(&p, &dst)?;
        }
    }
    Ok(())
}
fn copy_or_link(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_file(dst)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst)?;
    }
    #[cfg(windows)]
    {
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn list_installed() -> Result<()> {
    let root = store_dir().join("packages");
    if !root.exists() {
        println!("no packages installed");
        return Ok(());
    }
    for p in fs::read_dir(root)? {
        let p = p?.path();
        for v in fs::read_dir(&p)? {
            println!(
                "{}@{}",
                p.file_name().unwrap().to_string_lossy(),
                v?.file_name().to_string_lossy()
            );
        }
    }
    Ok(())
}
fn search(q: &str) -> Result<()> {
    let reg = load_registry()?;
    for (name, rs) in reg.packages {
        if name.contains(q)
            || rs
                .iter()
                .any(|r| r.description.to_lowercase().contains(&q.to_lowercase()))
        {
            if let Some(r) = rs.iter().max_by_key(|r| Version::parse(&r.version).ok()) {
                println!("{name:<24} {}  {}", r.version, r.description)
            }
        }
    }
    Ok(())
}
fn info(name: &str) -> Result<()> {
    let reg = load_registry()?;
    let rs = reg.packages.get(name).context("package not found")?;
    println!("{name}");
    for r in rs {
        println!(
            "  {}  {}  [{}]",
            r.version,
            r.description,
            r.targets.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}
fn uninstall(name: &str) -> Result<()> {
    let root = store_dir().join("packages").join(name);
    if !root.exists() {
        bail!("{name} is not installed")
    }
    let mut bins = Vec::new();
    for v in fs::read_dir(&root)? {
        let dir = v?.path().join("bin");
        if dir.exists() {
            for e in fs::read_dir(dir)? {
                bins.push(e?.file_name());
            }
        }
    }
    let b = store_dir().join("bin");
    for name in bins {
        let p = b.join(name);
        if p.exists() || p.is_symlink() {
            fs::remove_file(p)?;
        }
    }
    fs::remove_dir_all(&root)?;
    println!("uninstalled {name}");
    Ok(())
}

fn cache_cmd(c: CacheCommand) -> Result<()> {
    match c {
        CacheCommand::Path => println!("{}", cache_path().display()),
        CacheCommand::Clear => {
            let p = cache_path();
            if p.exists() {
                fs::remove_dir_all(p)?;
            }
            println!("cache cleared");
        }
    }
    Ok(())
}
fn config_cmd(c: ConfigCommand) -> Result<()> {
    match c {
        ConfigCommand::Show => println!("{}", toml::to_string_pretty(&load_config()?)?),
        ConfigCommand::SetRegistry { url } => {
            let mut cfg = load_config()?;
            cfg.registry = Some(url.clone());
            save_config(&cfg)?;
            println!("registry = {url}");
        }
    }
    Ok(())
}

fn pack(path: &Path, output: Option<PathBuf>) -> Result<()> {
    let root = fs::canonicalize(path)?;
    let manifest_path = root.join("lode-package.toml");
    if !manifest_path.exists() {
        bail!("{} is missing", manifest_path.display())
    }
    let meta: RegistryRelease = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
    if meta.name.is_empty() {
        bail!("lode-package.toml must contain `name` for publishing");
    }
    let default_target = target().to_string();
    let package_target = meta.targets.keys().next().unwrap_or(&default_target);
    let out = output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "{}-{}-{}.zip",
            meta.name, meta.version, package_target
        ))
    });
    let f = fs::File::create(&out)?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    add_dir_to_zip(&mut zip, &root, &root, &opts)?;
    zip.finish()?;
    println!("created {}", out.display());
    println!("sha256 {}", file_sha256(&out)?);
    Ok(())
}
fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<fs::File>,
    root: &Path,
    dir: &Path,
    opts: &zip::write::SimpleFileOptions,
) -> Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
        if p.is_dir() {
            zip.add_directory(format!("{rel}/"), *opts)?;
            add_dir_to_zip(zip, root, &p, opts)?
        } else {
            zip.start_file(rel, *opts)?;
            let mut f = fs::File::open(p)?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}
fn file_sha256(p: &Path) -> Result<String> {
    let mut f = fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn publish(artifact: &Path, registry: Option<String>) -> Result<()> {
    let token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN is required for publishing")?;
    let repo = std::env::var("GITHUB_REPO").context("GITHUB_REPO must be owner/repository")?;
    let registry_repo = std::env::var("LODE_REGISTRY_REPO").ok();
    let registry_path = std::env::var("LODE_REGISTRY_PATH").unwrap_or_else(|_| "index.json".into());
    let api = "https://api.github.com";
    let (version, package_name) = read_zip_identity(artifact)?;
    let tag = format!("v{version}");
    let client = reqwest::blocking::Client::new();
    let release_endpoint = format!("{api}/repos/{repo}/releases/tags/{tag}");
    let release: serde_json::Value = match client
        .get(&release_endpoint)
        .bearer_auth(&token)
        .header("User-Agent", "lode")
        .send()?
    {
        r if r.status().is_success() => r.json()?,
        _ => {
            let release_body = serde_json::json!({"tag_name":tag,"name":format!("{package_name} {version}"),"draft":false,"prerelease":false,"generate_release_notes":true});
            client
                .post(format!("{api}/repos/{repo}/releases"))
                .bearer_auth(&token)
                .header("User-Agent", "lode")
                .json(&release_body)
                .send()?
                .error_for_status()?
                .json()?
        }
    };
    let upload_url = release
        .get("upload_url")
        .and_then(|v| v.as_str())
        .context("GitHub did not return upload_url")?
        .split('{')
        .next()
        .unwrap()
        .to_string();
    let filename = artifact
        .file_name()
        .and_then(|s| s.to_str())
        .context("invalid artifact filename")?;
    let bytes = fs::read(artifact)?;
    client
        .post(format!("{upload_url}?name={filename}"))
        .bearer_auth(&token)
        .header("User-Agent", "lode")
        .header("Content-Type", "application/zip")
        .body(bytes.clone())
        .send()?
        .error_for_status()?;
    let sha = hex(&Sha256::digest(&bytes));
    let public_url = format!("https://github.com/{repo}/releases/download/{tag}/{filename}");
    println!("published {filename}");
    println!("sha256: {sha}");
    println!("artifact URL: {public_url}");
    if let Some(rr) = registry_repo.or(registry) {
        update_registry(
            &client,
            &token,
            &rr,
            &registry_path,
            filename,
            &package_name,
            &version,
            &public_url,
            &sha,
        )?;
    } else {
        println!(
            "registry not updated: set LODE_REGISTRY_REPO=owner/repo to update index.json automatically"
        );
    }
    Ok(())
}

fn read_zip_identity(path: &Path) -> Result<(String, String)> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut manifest = archive
        .by_name("lode-package.toml")
        .context("package archive is missing lode-package.toml")?;
    let mut text = String::new();
    manifest.read_to_string(&mut text)?;
    let meta: RegistryRelease = toml::from_str(&text)?;
    if meta.name.is_empty() {
        bail!("package manifest has no name");
    }
    Ok((meta.version, meta.name))
}

#[allow(clippy::too_many_arguments)]
fn update_registry(
    client: &reqwest::blocking::Client,
    token: &str,
    repo: &str,
    path: &str,
    filename: &str,
    package_name: &str,
    version: &str,
    url: &str,
    sha: &str,
) -> Result<()> {
    let endpoint = format!("https://api.github.com/repos/{repo}/contents/{path}");
    let current = client
        .get(&endpoint)
        .bearer_auth(token)
        .header("User-Agent", "lode")
        .send()?;
    let (mut index, sha_ref) = if current.status().is_success() {
        let v: serde_json::Value = current.json()?;
        let content = v
            .get("content")
            .and_then(|x| x.as_str())
            .context("registry content missing")?
            .replace('\n', "");
        let raw = base64::engine::general_purpose::STANDARD.decode(content)?;
        (
            serde_json::from_slice::<Registry>(&raw)?,
            v.get("sha").and_then(|x| x.as_str()).map(str::to_owned),
        )
    } else {
        (
            Registry {
                version: 1,
                packages: BTreeMap::new(),
            },
            None,
        )
    };
    let target_key = if filename.contains("x86_64-unknown-linux-gnu") {
        "x86_64-unknown-linux-gnu"
    } else if filename.contains("x86_64-pc-windows-msvc") {
        "x86_64-pc-windows-msvc"
    } else if filename.contains("aarch64-unknown-linux-gnu") {
        "aarch64-unknown-linux-gnu"
    } else if filename.contains("aarch64-pc-windows-msvc") {
        "aarch64-pc-windows-msvc"
    } else {
        bail!("cannot infer target from filename {filename}")
    };
    let releases = index.packages.entry(package_name.to_string()).or_default();
    let release_index = releases.iter().position(|r| r.version == version);
    if let Some(i) = release_index {
        releases[i].targets.insert(
            target_key.into(),
            Artifact {
                url: url.into(),
                sha256: sha.into(),
            },
        );
    } else {
        releases.push(RegistryRelease {
            name: package_name.to_string(),
            version: version.into(),
            description: "Published by LODE".into(),
            license: "".into(),
            dependencies: BTreeMap::new(),
            targets: BTreeMap::from([(
                target_key.into(),
                Artifact {
                    url: url.into(),
                    sha256: sha.into(),
                },
            )]),
        });
    }
    releases.sort_by(|a, b| {
        Version::parse(&b.version)
            .ok()
            .cmp(&Version::parse(&a.version).ok())
    });
    let body = serde_json::to_vec_pretty(&index)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(body);
    let mut payload = serde_json::json!({"message":format!("publish {package_name}@{version}"),"content":encoded});
    if let Some(s) = sha_ref {
        payload["sha"] = serde_json::Value::String(s);
    }
    client
        .put(&endpoint)
        .bearer_auth(token)
        .header("User-Agent", "lode")
        .json(&payload)
        .send()?
        .error_for_status()?;
    println!("registry updated: {repo}/{path}");
    Ok(())
}

fn doctor() -> Result<()> {
    println!("LODE doctor");
    println!("  platform : {}", target());
    println!("  home     : {}", store_dir().display());
    println!("  cache    : {}", cache_path().display());
    println!("  registry : {}", registry_url()?);
    println!("  git      : {}", command_exists("git"));
    println!("  curl     : {}", command_exists("curl"));
    println!(
        "  PATH has Lode bin: {}",
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|x| x == store_dir().join("bin")))
            .unwrap_or(false)
    );
    Ok(())
}
fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_specs() {
        assert_eq!(
            parse_spec("foo@^1.2").unwrap(),
            ("foo".into(), "^1.2".into())
        );
        assert_eq!(parse_spec("foo").unwrap(), ("foo".into(), "*".into()));
    }
    #[test]
    fn resolves_highest_compatible() {
        let mut targets = BTreeMap::new();
        targets.insert(
            "x86_64-unknown-linux-gnu".into(),
            Artifact {
                url: "file:///x".into(),
                sha256: "00".into(),
            },
        );
        let mut packages = BTreeMap::new();
        packages.insert(
            "foo".into(),
            vec![
                RegistryRelease {
                    name: "foo".into(),
                    version: "1.0.0".into(),
                    description: "old".into(),
                    license: "MIT".into(),
                    dependencies: BTreeMap::new(),
                    targets: targets.clone(),
                },
                RegistryRelease {
                    name: "foo".into(),
                    version: "1.2.0".into(),
                    description: "new".into(),
                    license: "MIT".into(),
                    dependencies: BTreeMap::new(),
                    targets,
                },
            ],
        );
        let r = Registry {
            version: 1,
            packages,
        };
        assert_eq!(resolve(&r, "foo", "^1.0").unwrap().version, "1.2.0");
    }
}
