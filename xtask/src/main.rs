use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    env, fs,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(about = "Cross-platform Prayer workspace orchestration")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Install pinned Node dependencies and build the local TypeScript SDK.
    Bootstrap,
    /// Regenerate the OpenAPI contract and TypeScript SDK.
    Generate,
    /// Fail when generated files are stale, then compile and test contracts.
    Check,
    /// Refresh SpaceMolt, generate, compile, and test the complete workspace.
    Build {
        /// Use the checked-in SpaceMolt specification without network access.
        #[arg(long)]
        offline: bool,
        /// SpaceMolt server from which to fetch the current OpenAPI specification.
        #[arg(long, default_value = "https://game.spacemolt.com")]
        base_url: String,
    },
    /// Run the Prayer API and a selected client with supervised shutdown.
    Run {
        /// Optional interactive client. Omit to run services only.
        #[arg(long, value_enum)]
        client: Option<Client>,
    },
    /// Run the public API audit.
    AuditPublicApi,
    /// Show Prayer logs.
    ShowLogs {
        #[arg(default_value_t = 80)]
        lines: usize,
    },
    /// Prune Prayer logs.
    PruneLogs,
    /// Refresh the checked-in SpaceMolt specification.
    RefreshSpacemolt {
        #[arg(long, default_value = "https://game.spacemolt.com")]
        base_url: String,
        #[arg(long, default_value_t = 2)]
        delay: u64,
        #[arg(long)]
        guides_only: bool,
        #[arg(long)]
        openapi_only: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum Client {
    Web,
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    env::set_current_dir(&root)?;
    prerequisites()?;
    match Cli::parse().command {
        Task::Bootstrap => bootstrap(&root),
        Task::Generate => generate(&root),
        Task::Check => check(&root),
        Task::Build { offline, base_url } => build(&root, &base_url, offline),
        Task::Run { client } => run_services(client),
        Task::AuditPublicApi => audit_public_api(&root),
        Task::ShowLogs { lines } => show_logs(&root, lines),
        Task::PruneLogs => prune_logs(&root),
        Task::RefreshSpacemolt {
            base_url,
            delay,
            guides_only,
            openapi_only,
        } => refresh_spacemolt(&root, &base_url, delay, guides_only, openapi_only),
    }
}

fn prerequisites() -> Result<()> {
    for tool in ["cargo", "node", "npm"] {
        let status = Command::new(tool)
            .arg("--version")
            .status()
            .with_context(|| format!("required tool `{tool}` was not found on PATH"))?;
        if !status.success() {
            bail!("required tool `{tool}` could not be executed");
        }
    }
    Ok(())
}

fn bootstrap(root: &Path) -> Result<()> {
    validate_version("cargo", &["--version"], 1, 78, None)?;
    validate_version("node", &["--version"], 22, 0, None)?;
    validate_version("npm", &["--version"], 10, 0, Some(12))?;
    let sdk = root.join("prayer-sdk-ts");
    run_in(sdk.clone(), "npm", &["ci"])?;
    run_in(sdk, "npm", &["run", "build"])?;
    run_in(root.join("reference-client-ts"), "npm", &["ci"])?;
    match env::var("SPACEMOLT_CLERK_API_KEY") {
        Ok(value) if !value.trim().is_empty() => {
            println!("SpaceMolt credentials detected; live services may be started.")
        }
        _ => println!(
            "SPACEMOLT_CLERK_API_KEY is not set. Compilation and tests work without it; live SpaceMolt connections do not."
        ),
    }
    println!("Bootstrap complete. Run `cargo xtask check`.");
    Ok(())
}

fn validate_version(
    program: &str,
    args: &[&str],
    minimum_major: u64,
    minimum_minor: u64,
    maximum_major: Option<u64>,
) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("required tool `{program}` was not found on PATH"))?;
    if !output.status.success() {
        bail!("{program} version check failed with {}", output.status);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text
        .split_whitespace()
        .find(|part| {
            part.trim_start_matches('v')
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
        .context("could not parse version output")?
        .trim_start_matches('v');
    let mut numbers = version.split('.').filter_map(|part| {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .ok()
    });
    let major = numbers.next().context("missing major version")?;
    let minor = numbers.next().unwrap_or(0);
    if (major, minor) < (minimum_major, minimum_minor)
        || maximum_major.is_some_and(|maximum| major >= maximum)
    {
        let upper = maximum_major
            .map(|value| format!(" and <{value}"))
            .unwrap_or_default();
        bail!("unsupported {program} {version}; expected >={minimum_major}.{minimum_minor}{upper}");
    }
    println!("{program} {version} is supported");
    Ok(())
}

fn require_node_dependencies(root: &Path) -> Result<()> {
    let missing = ["prayer-sdk-ts", "reference-client-ts"]
        .into_iter()
        .filter(|directory| !root.join(directory).join("node_modules").is_dir())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "Node dependencies are missing for {}. Run exactly: `cargo xtask bootstrap`",
            missing.join(", ")
        );
    }
    Ok(())
}

fn generate(root: &Path) -> Result<()> {
    run("cargo", &["check", "-p", "spacemolt-lib-rs"])?;
    run(
        "cargo",
        &[
            "run",
            "-p",
            "prayer-api",
            "--bin",
            "generate-openapi",
            "--",
            root.join("prayer-api/openapi/prayer-v1.json")
                .to_str()
                .unwrap(),
        ],
    )?;
    run_in(root.join("prayer-sdk-ts"), "npm", &["run", "generate"])
}

fn generated(root: &Path) -> [PathBuf; 3] {
    [
        root.join("prayer-api/openapi/prayer-v1.json"),
        root.join("prayer-sdk-ts/src/generated/types.ts"),
        root.join("prayer-sdk-ts/src/generated/api.ts"),
    ]
}

fn check(root: &Path) -> Result<()> {
    require_node_dependencies(root)?;
    let temp = env::temp_dir().join(format!(
        "prayer-generated-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(&temp)?;
    generate_to(root, &temp)?;
    let mut stale = Vec::new();
    for (path, generated_name) in
        generated(root)
            .iter()
            .zip(["prayer-v1.json", "types.ts", "api.ts"])
    {
        let actual = fs::read(path).with_context(|| {
            format!(
                "missing generated artifact {}; run `cargo xtask generate`",
                path.display()
            )
        })?;
        let expected = fs::read(temp.join(generated_name))?;
        if actual != expected {
            stale.push(path.display().to_string());
        }
    }
    fs::remove_dir_all(&temp)?;
    if !stale.is_empty() {
        bail!(
            "generated artifacts are stale: {}; run `cargo xtask generate` and commit the result",
            stale.join(", ")
        );
    }
    compile_and_test(root)
}

fn generate_to(root: &Path, destination: &Path) -> Result<()> {
    run("cargo", &["check", "-p", "spacemolt-lib-rs"])?;
    let openapi = destination.join("prayer-v1.json");
    let types = destination.join("types.ts");
    let api = destination.join("api.ts");
    run(
        "cargo",
        &[
            "run",
            "-p",
            "prayer-api",
            "--bin",
            "generate-openapi",
            "--",
            openapi.to_str().unwrap(),
        ],
    )?;
    run_in(
        root.join("prayer-sdk-ts"),
        "node",
        &[
            "scripts/generate-types.mjs",
            openapi.to_str().unwrap(),
            types.to_str().unwrap(),
        ],
    )?;
    run_in(
        root.join("prayer-sdk-ts"),
        "node",
        &[
            "scripts/generate-api.mjs",
            openapi.to_str().unwrap(),
            api.to_str().unwrap(),
        ],
    )
}

fn build(root: &Path, spacemolt_base_url: &str, offline: bool) -> Result<()> {
    require_node_dependencies(root)?;
    if offline {
        println!("Offline build: using checked-in spacemolt-openapi.json");
    } else {
        refresh_spacemolt(root, spacemolt_base_url, 0, false, true)?;
    }
    generate(root)?;
    compile_and_test(root)
}

fn compile_and_test(root: &Path) -> Result<()> {
    run("cargo", &["build", "--workspace", "--exclude", "xtask"])?;
    run("cargo", &["test", "--workspace", "--exclude", "xtask"])?;
    run("cargo", &["doc", "-p", "prayer-sdk", "--no-deps"])?;
    run(
        "cargo",
        &[
            "check",
            "--offline",
            "--manifest-path",
            "fixtures/rust-sdk-consumer/Cargo.toml",
        ],
    )?;
    run_in(root.join("prayer-sdk-ts"), "npm", &["run", "check"])?;
    run_in(root.join("prayer-sdk-ts"), "npm", &["test"])?;
    run_in(root.join("prayer-sdk-ts"), "npm", &["run", "test:package"])?;
    run_in(root.join("reference-client-ts"), "npm", &["run", "build"])?;
    run_in(root.join("reference-client-ts"), "npm", &["test"])
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    status(Command::new(program).args(args), program)
}
fn run_in(dir: PathBuf, program: &str, args: &[&str]) -> Result<()> {
    status(Command::new(program).current_dir(dir).args(args), program)
}
fn status(command: &mut Command, label: &str) -> Result<()> {
    let result = command
        .status()
        .with_context(|| format!("failed to start {label}"))?;
    if !result.success() {
        bail!("{label} failed with {result}");
    }
    Ok(())
}

fn run_services(client: Option<Client>) -> Result<()> {
    ensure_port_free("127.0.0.1:7777", "Prayer API")?;
    // Keep compilation outside the readiness window. A clean API build can take
    // longer than the startup timeout, but that does not mean startup failed.
    run(
        "cargo",
        &["build", "-p", "prayer-api", "--bin", "prayer-api"],
    )?;
    let mut api = spawn("cargo", &["run", "-p", "prayer-api", "--bin", "prayer-api"])?;
    let result = (|| {
        wait_for_port("127.0.0.1:7777", "Prayer API")?;
        match client {
            Some(Client::Web) => {
                run_in(PathBuf::from("reference-client-ts"), "npm", &["run", "dev"])
            }
            None => api
                .wait()
                .context("failed waiting for Prayer API")
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        bail!("Prayer API exited with {status}")
                    }
                }),
        }
    })();
    let _ = api.kill();
    let _ = api.wait();
    result
}

fn ensure_port_free(address: &str, service: &str) -> Result<()> {
    if TcpStream::connect(address).is_ok() {
        bail!("{service} cannot bind {address}: the port is already in use");
    }
    Ok(())
}

fn wait_for_port(address: &str, service: &str) -> Result<()> {
    let deadline = SystemTime::now() + Duration::from_secs(30);
    while SystemTime::now() < deadline {
        if TcpStream::connect(address).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("timed out waiting for {service} readiness at {address}")
}

fn audit_public_api(root: &Path) -> Result<()> {
    let pattern = regex::Regex::new(
        r"pub (?:[A-Za-z_][A-Za-z0-9_]*:|type [A-Za-z_][A-Za-z0-9_]*\s*=|fn [A-Za-z_][A-Za-z0-9_]*[^\n]*->)[^\n]*(?:serde_json::Value|\bValue\b)",
    )?;
    println!("Public serde_json::Value API audit\n==================================");
    let mut found = false;
    for directory in [
        "spacemolt-lib-rs/src",
        "prayer-runtime/src",
        "prayer-sdk/src",
    ] {
        visit_files(&root.join(directory), &mut |path| {
            if path.extension().and_then(|v| v.to_str()) == Some("rs") {
                if let Ok(text) = fs::read_to_string(path) {
                    for (index, line) in text.lines().enumerate() {
                        if pattern.is_match(line) {
                            println!("{}:{}:{}", path.display(), index + 1, line);
                            found = true;
                        }
                    }
                }
            }
        })?;
    }
    if !found {
        println!("No public serde_json::Value occurrences found.");
    }
    Ok(())
}

fn visit_files(directory: &Path, callback: &mut impl FnMut(&Path)) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            visit_files(&path, callback)?;
        } else {
            callback(&path);
        }
    }
    Ok(())
}

fn log_root(root: &Path) -> PathBuf {
    env::var_os("PRAYER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("logs"))
}

fn show_logs(root: &Path, lines: usize) -> Result<()> {
    let logs = log_root(root);
    let latest = logs.join("latest");
    println!("Log root: {}", logs.display());
    for service in ["api", "client"] {
        let path = latest.join(format!("{service}.log"));
        if let Ok(text) = fs::read_to_string(&path) {
            println!("\n== {service}: last {lines} lines ==");
            for line in text
                .lines()
                .rev()
                .take(lines)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                println!("{line}");
            }
        }
    }
    let mut files = Vec::new();
    visit_files(&logs.join("runs"), &mut |path| {
        if let Ok(meta) = fs::metadata(path) {
            files.push((meta.len(), path.to_owned()));
        }
    })?;
    files.sort_by_key(|item| std::cmp::Reverse(item.0));
    println!("\nLargest run logs:");
    for (size, path) in files.into_iter().take(20) {
        println!("{:.1} MB  {}", size as f64 / 1_048_576.0, path.display());
    }
    Ok(())
}

fn prune_logs(root: &Path) -> Result<()> {
    let runs = log_root(root).join("runs");
    fs::create_dir_all(&runs)?;
    let retention: u64 = env::var("PRAYER_LOG_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(14);
    let maximum: u64 = env::var("PRAYER_LOG_TOTAL_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500)
        * 1_048_576;
    let cutoff = SystemTime::now() - Duration::from_secs(retention * 86_400);
    let mut directories = Vec::new();
    for entry in fs::read_dir(&runs)? {
        let path = entry?.path();
        if path.is_dir() {
            let modified = fs::metadata(&path)?.modified()?;
            if modified < cutoff {
                fs::remove_dir_all(&path)?;
            } else {
                directories.push((modified, path));
            }
        }
    }
    directories.sort_by_key(|item| item.0);
    while directory_size(&runs)? > maximum {
        if directories.is_empty() {
            break;
        }
        let (_, path) = directories.remove(0);
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn directory_size(path: &Path) -> Result<u64> {
    let mut size = 0;
    visit_files(path, &mut |file| {
        size += fs::metadata(file).map(|m| m.len()).unwrap_or(0)
    })?;
    Ok(size)
}

fn refresh_spacemolt(
    root: &Path,
    base: &str,
    delay: u64,
    guides_only: bool,
    openapi_only: bool,
) -> Result<()> {
    if guides_only && openapi_only {
        bail!("--guides-only and --openapi-only are mutually exclusive");
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let mut targets = Vec::new();
    if !openapi_only {
        for entry in fs::read_dir(root.join("docs/v2")).context("no docs/v2 guide directory")? {
            let path = entry?.path();
            if path.extension().and_then(|v| v.to_str()) == Some("json") {
                let name = path.file_stem().unwrap().to_string_lossy();
                targets.push((
                    format!("{}/api/v2/{name}/help", base.trim_end_matches('/')),
                    path,
                ));
            }
        }
        targets.sort_by(|a, b| a.1.cmp(&b.1));
    }
    if !guides_only {
        targets.push((
            format!("{}/api/v2/openapi.json", base.trim_end_matches('/')),
            root.join("spacemolt-openapi.json"),
        ));
    }
    for (index, (url, path)) in targets.iter().enumerate() {
        println!("Fetching {url}");
        let bytes = client.get(url).send()?.error_for_status()?.bytes()?;
        let document = serde_json::from_slice::<serde_json::Value>(&bytes)
            .with_context(|| format!("{url} returned invalid JSON"))?;
        if path.file_name().and_then(|name| name.to_str()) == Some("spacemolt-openapi.json") {
            let openapi = document
                .get("openapi")
                .and_then(serde_json::Value::as_str)
                .context("SpaceMolt OpenAPI download omitted the openapi version")?;
            let game_version = document
                .pointer("/info/x-gameserver-version")
                .and_then(serde_json::Value::as_str)
                .context("SpaceMolt OpenAPI download omitted info.x-gameserver-version")?;
            let api_version = document
                .pointer("/info/version")
                .and_then(serde_json::Value::as_str)
                .context("SpaceMolt OpenAPI download omitted info.version")?;
            let paths = document
                .get("paths")
                .and_then(serde_json::Value::as_object)
                .context("SpaceMolt OpenAPI download omitted paths")?
                .len();
            let schemas = document
                .pointer("/components/schemas")
                .and_then(serde_json::Value::as_object)
                .context("SpaceMolt OpenAPI download omitted components.schemas")?
                .len();
            println!(
                "Fetched SpaceMolt {game_version} (API {api_version}, OpenAPI {openapi}, {paths} paths, {schemas} schemas)"
            );
        }
        replace_file(path, &bytes)?;
        if index + 1 < targets.len() {
            thread::sleep(Duration::from_secs(delay));
        }
    }
    Ok(())
}

fn replace_file(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("{} has no UTF-8 file name", path.display()))?;
    let temporary = parent.join(format!(
        ".{file_name}.download-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::write(&temporary, contents)
        .with_context(|| format!("writing temporary download {}", temporary.display()))?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("removing previous {}", path.display()))?;
    }
    fs::rename(&temporary, path).with_context(|| {
        format!(
            "replacing {} with validated download {}",
            path.display(),
            temporary.display()
        )
    })
}
fn spawn(program: &str, args: &[&str]) -> Result<Child> {
    Command::new(program)
        .args(args)
        .spawn()
        .with_context(|| format!("failed to start {program}"))
}
