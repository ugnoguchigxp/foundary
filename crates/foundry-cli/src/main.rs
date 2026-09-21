use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

use clap::{Args, Parser, Subcommand};
use foundry_core::{Failure, Success};
use foundry_image::{EncodeOptions, ImageError, convert_to_webp, default_output};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "foundry", version, about = "Shared local file-processing CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List capabilities known to this installation.
    List {
        /// Emit a machine-readable result.
        #[arg(long)]
        json: bool,
    },
    /// Diagnose which capabilities are available in this environment.
    Doctor {
        #[command(subcommand)]
        command: Option<DoctorCommand>,
        /// Emit a machine-readable result.
        #[arg(long)]
        json: bool,
    },
    /// Inspect shared build caches without modifying them.
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    /// Print shell settings for the shared Rust build cache.
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    /// Create Foundry-owned, opt-in local build settings.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    /// Run a command with a project-scoped shared Rust build environment.
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
    /// Convert and inspect images.
    Image {
        #[command(subcommand)]
        command: ImageCommand,
    },
}

#[derive(Subcommand)]
enum ImageCommand {
    /// Convert one JPG, JPEG, or PNG file to WebP.
    Webp(WebpArgs),
}

#[derive(Subcommand)]
enum DoctorCommand {
    /// Diagnose the Rust, Cargo, and macOS native build toolchain.
    Rust {
        /// Emit a machine-readable result.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CacheCommand {
    /// Report the state and size of shared build caches without modifying them.
    Status {
        /// Emit a machine-readable result.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum EnvCommand {
    /// Print opt-in Rust compiler-cache settings for a shell.
    Rust {
        /// Shell syntax to emit. macOS support starts with zsh.
        #[arg(long, default_value = "zsh", value_parser = ["zsh"])]
        shell: String,
        /// Project root containing foundry-rust.toml. Adds an isolated target directory.
        #[arg(long)]
        project: Option<PathBuf>,
        /// Build unit declared by the project. Required when the project has more than one unit.
        #[arg(long)]
        unit: Option<String>,
    },
}

#[derive(Subcommand)]
enum RunCommand {
    /// Run Cargo or another build command with the declared toolchain and isolated target.
    Rust {
        /// Project root containing foundry-rust.toml.
        #[arg(long)]
        project: PathBuf,
        /// Build unit declared by the project.
        #[arg(long)]
        unit: Option<String>,
        /// Command and arguments after `--`.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<OsString>,
    },
}

#[derive(Subcommand)]
enum SetupCommand {
    /// Create the opt-in zsh profile for the shared Rust compiler cache.
    Rust {
        /// Report the action as JSON.
        #[arg(long)]
        json: bool,
        /// Check whether setup can proceed without writing the profile.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args)]
struct WebpArgs {
    /// Source JPG, JPEG, or PNG image.
    input: PathBuf,

    /// Destination WebP file. Defaults to the source directory and file stem.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Preserve decoded image pixels; cannot be combined with --quality.
    #[arg(long, conflicts_with = "quality")]
    lossless: bool,

    /// Lossy WebP quality from 0 to 100. Defaults to 80.
    #[arg(long)]
    quality: Option<f32>,

    /// Emit a machine-readable result.
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Capability {
    id: &'static str,
    description: &'static str,
    status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorCapability {
    id: &'static str,
    kind: &'static str,
    status: &'static str,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntry {
    id: &'static str,
    status: &'static str,
    path: String,
    bytes: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupResult {
    path: String,
    changed: bool,
    source_hint: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustProject {
    schema_version: u8,
    project_id: String,
    toolchain: String,
    build_units: Vec<BuildUnit>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildUnit {
    id: String,
    manifest_path: String,
}

struct ResolvedBuildEnvironment {
    project_root: PathBuf,
    target_dir: PathBuf,
    variables: Vec<(String, OsString)>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::List { json } => list(json),
        Command::Doctor { command, json } => match command {
            Some(DoctorCommand::Rust { json }) => doctor_rust(json),
            None => doctor(json),
        },
        Command::Cache {
            command: CacheCommand::Status { json },
        } => cache_status(json),
        Command::Env {
            command:
                EnvCommand::Rust {
                    shell,
                    project,
                    unit,
                },
        } => print_rust_environment(&shell, project.as_deref(), unit.as_deref()),
        Command::Setup {
            command: SetupCommand::Rust { json, dry_run },
        } => setup_rust(json, dry_run),
        Command::Run {
            command:
                RunCommand::Rust {
                    project,
                    unit,
                    command,
                },
        } => run_rust(&project, unit.as_deref(), &command),
        Command::Image {
            command: ImageCommand::Webp(args),
        } => webp(args),
    }
}

fn list(json: bool) -> ExitCode {
    let capabilities = vec![Capability {
        id: "image.webp",
        description: "Convert one JPG, JPEG, or PNG image to WebP",
        status: "ready",
    }];
    if json {
        print_json(&Success::new("list", capabilities, Vec::new()));
    } else {
        println!("image.webp\tConvert one JPG, JPEG, or PNG image to WebP");
    }
    ExitCode::SUCCESS
}

fn doctor(json: bool) -> ExitCode {
    let mut capabilities = vec![DoctorCapability {
        id: "image",
        kind: "native",
        status: "ready",
        detail: "Built-in JPG, PNG, and WebP support is available".to_owned(),
    }];
    capabilities.extend(rust_capabilities());
    if json {
        print_json(&Success::new("doctor", capabilities, Vec::new()));
    } else {
        println!("Foundry {}", env!("CARGO_PKG_VERSION"));
        println!();
        println!("Native");
        println!("  image       READY  Built-in JPG, PNG, and WebP support is available");
        print_capabilities(&rust_capabilities());
    }
    ExitCode::SUCCESS
}

fn doctor_rust(json: bool) -> ExitCode {
    let capabilities = rust_capabilities();
    if json {
        print_json(&Success::new("doctor.rust", capabilities, Vec::new()));
    } else {
        println!("Foundry {}", env!("CARGO_PKG_VERSION"));
        println!();
        println!("Rust build environment");
        print_capabilities(&capabilities);
    }
    ExitCode::SUCCESS
}

fn cache_status(json: bool) -> ExitCode {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".cargo"));
    let entries = vec![
        cache_entry("cargo.registry", cargo_home.join("registry")),
        cache_entry("cargo.git", cargo_home.join("git")),
        cache_entry(
            "sccache",
            env::var_os("SCCACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| foundry_cache_dir().join("rust-build/sccache/v1")),
        ),
        cache_entry(
            "cargo.targets",
            foundry_cache_dir().join("rust-build/targets"),
        ),
    ];
    if json {
        print_json(&Success::new("cache.status", entries, Vec::new()));
    } else {
        println!("Shared build caches (read-only)");
        for entry in entries {
            let size = entry
                .bytes
                .map(format_bytes)
                .unwrap_or_else(|| "n/a".to_owned());
            println!(
                "  {:<16} {:<14} {:>10}  {}",
                entry.id,
                entry.status.to_uppercase(),
                size,
                entry.path
            );
        }
    }
    ExitCode::SUCCESS
}

fn print_rust_environment(shell: &str, project: Option<&Path>, unit: Option<&str>) -> ExitCode {
    debug_assert_eq!(shell, "zsh");
    print!("{}", rust_environment_script());
    if let Some(project) = project {
        match resolve_build_environment(project, unit) {
            Ok(environment) => {
                println!(
                    "export CARGO_TARGET_DIR={}",
                    shell_quote(environment.target_dir.as_os_str())
                );
                for (name, value) in environment.variables {
                    if name == "CARGO_TARGET_DIR" {
                        continue;
                    }
                    println!("export {name}={}", shell_quote(&value));
                }
            }
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}

fn setup_rust(json: bool, dry_run: bool) -> ExitCode {
    let path = foundry_config_dir().join("rust-build.zsh");
    let script = rust_environment_script();
    let changed = match fs::read_to_string(&path) {
        Ok(existing) if existing == script => false,
        Ok(existing) if existing.starts_with("# Foundry-managed Rust build cache") => true,
        Ok(_) => return setup_failure(json, &path, "the existing file is not managed by Foundry"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return setup_failure(json, &path, &error.to_string()),
    };
    if !dry_run && let Err(error) = create_rust_profile(&path, &script) {
        return setup_failure(json, &path, &error.to_string());
    }
    let result = SetupResult {
        path: path.display().to_string(),
        changed,
        source_hint: format!("source {}", path.display()),
    };
    if json {
        print_json(&Success::new("setup.rust", result, Vec::new()));
    } else if dry_run {
        println!(
            "Would {} {}",
            if changed { "write" } else { "keep" },
            path.display()
        );
    } else if changed {
        println!("Wrote {}", path.display());
        println!("Activate with: {}", result.source_hint);
    } else {
        println!("Already configured: {}", path.display());
    }
    ExitCode::SUCCESS
}

fn setup_failure(json: bool, path: &Path, message: &str) -> ExitCode {
    if json {
        print_json(&Failure::new("setup.rust", "SETUP_FAILED", message));
    } else {
        eprintln!("error: cannot configure {}: {message}", path.display());
    }
    ExitCode::from(1)
}

fn rust_environment_script() -> String {
    let sccache = command_path("sccache").unwrap_or_else(|| PathBuf::from("sccache"));
    let cache = foundry_cache_dir().join("rust-build/sccache/v1");
    format!(
        "# Foundry-managed Rust build cache (opt in with: eval \"$(foundry env rust)\")\n\
export SCCACHE_DIR={}\n\
export SCCACHE_CACHE_SIZE=\"${{SCCACHE_CACHE_SIZE:-20G}}\"\n\
export CARGO_INCREMENTAL=\"${{CARGO_INCREMENTAL:-0}}\"\n\
if test -x {} || command -v sccache >/dev/null 2>&1; then\n\
  export RUSTC_WRAPPER={}\n\
fi\n",
        shell_quote(cache.as_os_str()),
        shell_quote(sccache.as_os_str()),
        shell_quote(sccache.as_os_str()),
    )
}

fn create_rust_profile(path: &Path, script: &str) -> std::io::Result<()> {
    let root = foundry_config_dir().join("rust-build/profiles/v1");
    fs::create_dir_all(&root)?;
    fs::create_dir_all(foundry_cache_dir().join("rust-build/sccache/v1"))?;
    fs::create_dir_all(foundry_cache_dir().join("rust-build/targets"))?;

    let sccache = command_path("sccache").unwrap_or_else(|| PathBuf::from("sccache"));
    let clang = command_path("clang").unwrap_or_else(|| PathBuf::from("/usr/bin/clang"));
    let clangxx = command_path("clang++").unwrap_or_else(|| PathBuf::from("/usr/bin/clang++"));

    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "profileRevision": "v1",
        "sccache": sccache,
        "clang": clang,
        "clangxx": clangxx,
        "sdkPath": command_output("xcrun", &["--show-sdk-path"]),
        "sdkVersion": command_output("xcrun", &["--show-sdk-version"]),
        "rustToolchains": command_output("rustup", &["toolchain", "list"]),
    });
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("profile manifest serializes"),
    )?;
    fs::create_dir_all(path.parent().expect("config path has a parent"))?;
    fs::write(path, script)
}

fn rust_capabilities() -> Vec<DoctorCapability> {
    let active_toolchain = command_output_outside_project("rustup", &["show", "active-toolchain"]);
    let mut capabilities = vec![command_capability(
        "rustup",
        "rust",
        active_toolchain.as_deref(),
        "rustup is unavailable",
    )];

    let cargo_detail = active_toolchain
        .as_deref()
        .and_then(|value| value.split_whitespace().next())
        .and_then(|toolchain| command_output("rustup", &["run", toolchain, "cargo", "-V"]));
    capabilities.push(command_capability(
        "cargo.active",
        "rust",
        cargo_detail.as_deref(),
        "the active Rust toolchain cannot run cargo",
    ));
    capabilities.push(command_capability(
        "xcode",
        "native",
        command_output("xcode-select", &["-p"]).as_deref(),
        "Xcode command-line tools are unavailable",
    ));
    capabilities.push(command_capability(
        "macos.sdk",
        "native",
        command_output("xcrun", &["--show-sdk-path"]).as_deref(),
        "the macOS SDK is unavailable",
    ));
    for (id, program, arguments, detail) in [
        ("clang", "clang", &["--version"][..], "clang is unavailable"),
        ("cmake", "cmake", &["--version"][..], "cmake is unavailable"),
        ("ninja", "ninja", &["--version"][..], "ninja is unavailable"),
        (
            "pkg-config",
            "pkg-config",
            &["--version"][..],
            "pkg-config is unavailable",
        ),
        (
            "sccache",
            "sccache",
            &["--version"][..],
            "sccache is not configured",
        ),
    ] {
        capabilities.push(command_capability(
            id,
            "native",
            command_output(program, arguments).as_deref(),
            detail,
        ));
    }
    capabilities
}

fn command_capability(
    id: &'static str,
    kind: &'static str,
    detail: Option<&str>,
    missing_detail: &'static str,
) -> DoctorCapability {
    match detail {
        Some(detail) => DoctorCapability {
            id,
            kind,
            status: "ready",
            detail: detail.to_owned(),
        },
        None => DoctorCapability {
            id,
            kind,
            status: "missing",
            detail: missing_detail.to_owned(),
        },
    }
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    command_output_at(None, program, arguments)
}

fn command_output_outside_project(program: &str, arguments: &[&str]) -> Option<String> {
    command_output_at(Some(Path::new("/tmp")), program, arguments)
}

fn command_output_at(
    directory: Option<&Path>,
    program: &str,
    arguments: &[&str],
) -> Option<String> {
    let mut command = ProcessCommand::new(program);
    command.env_remove("RUSTUP_TOOLCHAIN").args(arguments);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout);
    value.lines().next().map(str::to_owned)
}

fn print_capabilities(capabilities: &[DoctorCapability]) {
    for capability in capabilities {
        println!(
            "  {:<16} {:<8} {}",
            capability.id,
            capability.status.to_uppercase(),
            capability.detail
        );
    }
}

fn cache_entry(id: &'static str, path: PathBuf) -> CacheEntry {
    let bytes = directory_size(&path);
    CacheEntry {
        id,
        status: match (id, bytes.is_some()) {
            (_, true) => "ready",
            ("sccache", false) => "empty",
            _ => "not-configured",
        },
        path: path.display().to_string(),
        bytes,
    }
}

fn directory_size(path: &Path) -> Option<u64> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.is_file() {
        return Some(metadata.len());
    }
    if !metadata.is_dir() {
        return Some(0);
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(path).ok()? {
        let entry = entry.ok()?;
        let file_type = entry.file_type().ok()?;
        if file_type.is_symlink() {
            continue;
        }
        total = total.checked_add(directory_size(&entry.path())?)?;
    }
    Some(total)
}

fn home_dir() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn foundry_config_dir() -> PathBuf {
    env::var_os("FOUNDRY_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config/foundry"))
}

fn foundry_cache_dir() -> PathBuf {
    env::var_os("FOUNDRY_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("Library/Caches/Foundry"))
}

fn command_path(program: &str) -> Option<PathBuf> {
    let output = ProcessCommand::new("/usr/bin/which")
        .arg(program)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    path.canonicalize().ok().or(Some(path))
}

fn shell_quote(value: &std::ffi::OsStr) -> String {
    let value = value.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn resolve_build_environment(
    project: &Path,
    requested_unit: Option<&str>,
) -> Result<ResolvedBuildEnvironment, String> {
    let project_root = project
        .canonicalize()
        .map_err(|error| format!("cannot resolve project {}: {error}", project.display()))?;
    let config_path = project_root.join("foundry-rust.toml");
    let source = fs::read_to_string(&config_path)
        .map_err(|error| format!("cannot read {}: {error}", config_path.display()))?;
    let config: RustProject = toml::from_str(&source)
        .map_err(|error| format!("invalid {}: {error}", config_path.display()))?;
    validate_project(&config)?;
    let unit = match requested_unit {
        Some(id) => config
            .build_units
            .iter()
            .find(|unit| unit.id == id)
            .ok_or_else(|| format!("unknown build unit {id:?} for {}", config.project_id))?,
        None if config.build_units.len() == 1 => &config.build_units[0],
        None => {
            return Err(format!(
                "project {} has multiple build units; pass --unit ({})",
                config.project_id,
                config
                    .build_units
                    .iter()
                    .map(|unit| unit.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    };
    let manifest = project_root.join(&unit.manifest_path);
    if !manifest.is_file() {
        return Err(format!("manifest does not exist: {}", manifest.display()));
    }
    let command_dir = manifest
        .parent()
        .expect("manifest has a parent")
        .to_path_buf();
    let target = rust_host(&config.toolchain).unwrap_or_else(|| "aarch64-apple-darwin".to_owned());
    let checkout = format!(
        "{:016x}",
        fnv1a(project_root.as_os_str().to_string_lossy().as_bytes())
    );
    let target_dir = foundry_cache_dir()
        .join("rust-build/targets")
        .join(&config.project_id)
        .join(checkout)
        .join(&unit.id)
        .join(&target);
    let sccache = command_path("sccache").unwrap_or_else(|| PathBuf::from("sccache"));
    let clang = command_path("clang").unwrap_or_else(|| PathBuf::from("/usr/bin/clang"));
    let clangxx = command_path("clang++").unwrap_or_else(|| PathBuf::from("/usr/bin/clang++"));
    let target_env = target.replace('-', "_");
    let mut variables = vec![
        (
            "RUSTUP_TOOLCHAIN".to_owned(),
            OsString::from(&config.toolchain),
        ),
        (
            "CARGO_TARGET_DIR".to_owned(),
            target_dir.as_os_str().to_owned(),
        ),
        (
            "SCCACHE_DIR".to_owned(),
            foundry_cache_dir()
                .join("rust-build/sccache/v1")
                .into_os_string(),
        ),
        ("SCCACHE_CACHE_SIZE".to_owned(), OsString::from("20G")),
        ("CARGO_INCREMENTAL".to_owned(), OsString::from("0")),
    ];
    if command_path("sccache").is_some() {
        variables.push(("RUSTC_WRAPPER".to_owned(), sccache.as_os_str().to_owned()));
        variables.push((
            "CMAKE_C_COMPILER_LAUNCHER".to_owned(),
            sccache.as_os_str().to_owned(),
        ));
        variables.push((
            "CMAKE_CXX_COMPILER_LAUNCHER".to_owned(),
            sccache.into_os_string(),
        ));
    }
    variables.push((format!("CC_{target_env}"), clang.into_os_string()));
    variables.push((format!("CXX_{target_env}"), clangxx.into_os_string()));
    Ok(ResolvedBuildEnvironment {
        project_root: command_dir,
        target_dir,
        variables,
    })
}

fn validate_project(project: &RustProject) -> Result<(), String> {
    if project.schema_version != 1 {
        return Err(format!(
            "unsupported foundry-rust schema version {}",
            project.schema_version
        ));
    }
    validate_id("project_id", &project.project_id)?;
    if project.toolchain.trim().is_empty() {
        return Err("toolchain must not be empty".to_owned());
    }
    if project.build_units.is_empty() {
        return Err("build_units must contain at least one unit".to_owned());
    }
    for unit in &project.build_units {
        validate_id("build unit id", &unit.id)?;
        let path = Path::new(&unit.manifest_path);
        if path.is_absolute() || unit.manifest_path.split('/').any(|part| part == "..") {
            return Err(format!(
                "build unit {} has an unsafe manifest path",
                unit.id
            ));
        }
    }
    Ok(())
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!(
            "{label} must contain only lowercase ASCII letters, digits, and hyphens"
        ));
    }
    Ok(())
}

fn rust_host(toolchain: &str) -> Option<String> {
    let output = ProcessCommand::new("rustup")
        .args(["run", toolchain, "rustc", "-vV"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn run_rust(project: &Path, unit: Option<&str>, command: &[OsString]) -> ExitCode {
    let environment = match resolve_build_environment(project, unit) {
        Ok(environment) => environment,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(1);
        }
    };
    let Some((program, arguments)) = command.split_first() else {
        eprintln!("error: missing command after --");
        return ExitCode::from(2);
    };
    if let Err(error) = fs::create_dir_all(&environment.target_dir) {
        eprintln!(
            "error: cannot create target directory {}: {error}",
            environment.target_dir.display()
        );
        return ExitCode::from(1);
    }
    let status = ProcessCommand::new(program)
        .args(arguments)
        .current_dir(&environment.project_root)
        .envs(environment.variables)
        .status();
    match status {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!(
                "error: cannot run {}: {error}",
                Path::new(program).display()
            );
            ExitCode::from(1)
        }
    }
}

fn webp(args: WebpArgs) -> ExitCode {
    let output = args.output.unwrap_or_else(|| default_output(&args.input));
    match convert_to_webp(
        &args.input,
        &output,
        EncodeOptions {
            lossless: args.lossless,
            quality: args.quality,
        },
    ) {
        Ok(result) => {
            let mut warnings = Vec::new();
            if result.output_bytes >= result.input_bytes {
                warnings.push("WebP output is not smaller than the input".to_owned());
            }
            if args.json {
                print_json(&Success::new("image.webp", result, warnings));
            } else {
                let change = percent_change(result.input_bytes, result.output_bytes);
                println!(
                    "{} -> {} ({} -> {}, {change})",
                    result.input.display(),
                    result.output.display(),
                    format_bytes(result.input_bytes),
                    format_bytes(result.output_bytes),
                );
                for warning in warnings {
                    eprintln!("warning: {warning}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let (code, message) = error_detail(&error);
            if args.json {
                print_json(&Failure::new("image.webp", code, message));
            } else {
                eprintln!("error: {message}");
            }
            ExitCode::from(1)
        }
    }
}

fn error_detail(error: &ImageError) -> (&'static str, String) {
    let code = match error {
        ImageError::InputNotFound(_) => "INPUT_NOT_FOUND",
        ImageError::UnsupportedInput(_) => "INVALID_INPUT",
        ImageError::OutputExists(_) => "OUTPUT_EXISTS",
        ImageError::InvalidQuality => "INVALID_ARGUMENT",
        ImageError::Decode(_) => "INVALID_INPUT",
        ImageError::Io(_) => "IO_ERROR",
    };
    (code, error.to_string())
}

fn print_json(value: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string(value).expect("JSON result serialization must succeed")
    );
}

fn percent_change(input: u64, output: u64) -> String {
    if input == 0 {
        return "n/a".to_owned();
    }
    let percent = (1.0 - output as f64 / input as f64) * 100.0;
    if percent >= 0.0 {
        format!("{percent:.0}% smaller")
    } else {
        format!("{:.0}% larger", percent.abs())
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MIB {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}
