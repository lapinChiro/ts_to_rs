use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use ts_to_rs::directory;
use ts_to_rs::registry::TypeRegistry;
use ts_to_rs::UnsupportedSyntax;

/// TypeScript to Rust transpiler CLI tool.
#[derive(Parser, Debug)]
#[command(version, about = "Transpile TypeScript source code to Rust")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input TypeScript file or directory path
    input: Option<PathBuf>,

    /// Output Rust file or directory path (defaults to <input>.rs or <input>_rs/)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Report unsupported syntax as JSON to stdout instead of aborting on errors
    #[arg(long)]
    report_unsupported: bool,

    /// Disable built-in Web API type definitions (Response, Request, Headers, etc.)
    #[arg(long)]
    no_builtin_types: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Resolve external type definitions using the TypeScript compiler via Docker
    ResolveTypes {
        /// Path to tsconfig.json
        #[arg(long)]
        tsconfig: PathBuf,

        /// Output path for the resolved types JSON (default: .ts_to_rs/types.json)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// Docker image name for the type extraction tool.
const DOCKER_IMAGE: &str = "ts-to-rs-extract-types";

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        return match command {
            Commands::ResolveTypes { tsconfig, output } => {
                resolve_types(&tsconfig, output.as_deref())
            }
        };
    }

    let input = cli
        .input
        .context("input path is required for transpilation")?;
    let use_builtin = !cli.no_builtin_types;

    if cli.report_unsupported {
        let unsupported = if input.is_dir() {
            transpile_directory_collecting(&input, cli.output.as_deref(), use_builtin)?
        } else {
            transpile_file_collecting(&input, cli.output.as_deref(), use_builtin)?
        };
        let json = serde_json::to_string_pretty(&unsupported)?;
        println!("{json}");
        Ok(())
    } else if input.is_dir() {
        transpile_directory(&input, cli.output.as_deref(), use_builtin)
    } else {
        transpile_file(&input, cli.output.as_deref(), use_builtin)
    }
}

/// Resolves external type definitions by running tsc via Docker.
fn resolve_types(tsconfig: &Path, output: Option<&Path>) -> Result<()> {
    let tsconfig = tsconfig
        .canonicalize()
        .with_context(|| format!("tsconfig not found: {}", tsconfig.display()))?;
    let project_dir = tsconfig
        .parent()
        .context("tsconfig has no parent directory")?;

    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir.join(".ts_to_rs/types.json"));

    // Ensure Docker is available
    let docker_check = Command::new("docker").arg("--version").output();
    match docker_check {
        Ok(o) if o.status.success() => {}
        Ok(_) => {
            bail!("Docker is installed but returned an error. Ensure Docker daemon is running.")
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!(
                "Docker is not installed. Install Docker to use resolve-types.\n\
                 Alternatively, built-in Web API types are available without Docker."
            );
        }
        Err(e) => bail!("Failed to check Docker: {e}"),
    }

    // Check if image exists, build if not
    let image_check = Command::new("docker")
        .args(["image", "inspect", DOCKER_IMAGE])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !image_check.map(|s| s.success()).unwrap_or(false) {
        eprintln!("Building Docker image '{DOCKER_IMAGE}'...");
        let dockerfile_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/extract-types");
        let status = Command::new("docker")
            .args([
                "build",
                "-t",
                DOCKER_IMAGE,
                &dockerfile_dir.to_string_lossy(),
            ])
            .status()
            .context("failed to build Docker image")?;
        if !status.success() {
            bail!("Docker image build failed");
        }
    }

    // Run the extraction container
    eprintln!("Resolving types from {}...", tsconfig.display());
    let tsconfig_filename = tsconfig
        .file_name()
        .context("invalid tsconfig path")?
        .to_string_lossy();
    let output_docker = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!("{}:/project:ro", project_dir.display()),
            DOCKER_IMAGE,
            "--tsconfig",
            &format!("/project/{tsconfig_filename}"),
        ])
        .output()
        .context("failed to run Docker container")?;

    if !output_docker.status.success() {
        let stderr = String::from_utf8_lossy(&output_docker.stderr);
        bail!("Type extraction failed:\n{stderr}");
    }

    // Write output
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }
    fs::write(&output_path, &output_docker.stdout)
        .with_context(|| format!("failed to write: {}", output_path.display()))?;

    eprintln!("Wrote {}", output_path.display());
    Ok(())
}

/// Loads external types from `.ts_to_rs/types.json` if it exists in the input directory.
fn load_external_types_json(input_dir: &Path) -> Option<TypeRegistry> {
    let types_json = input_dir.join(".ts_to_rs/types.json");
    if !types_json.exists() {
        return None;
    }
    let json = fs::read_to_string(&types_json).ok()?;
    match ts_to_rs::external_types::load_types_json(&json) {
        Ok(reg) => {
            eprintln!("Loaded external types from {}", types_json.display());
            Some(reg)
        }
        Err(e) => {
            eprintln!("Warning: failed to load {}: {e}", types_json.display());
            None
        }
    }
}

/// Runs `rustfmt` on the given files. Prints a warning and continues if `rustfmt` is not available.
fn run_rustfmt(paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }

    let result = Command::new("rustfmt").args(paths).status();

    match result {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("Warning: rustfmt exited with status {status}; output may not be formatted");
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Warning: rustfmt not found; output will not be formatted");
        }
        Err(e) => {
            eprintln!("Warning: failed to run rustfmt: {e}; output may not be formatted");
        }
    }
}

/// Builds a base registry from built-in types and optional external types.json.
fn build_base_registry(input_dir: &Path, use_builtin_types: bool) -> TypeRegistry {
    let mut registry = if use_builtin_types {
        ts_to_rs::external_types::load_builtin_types().unwrap_or_default()
    } else {
        TypeRegistry::new()
    };

    // Auto-detect .ts_to_rs/types.json
    if let Some(external) = load_external_types_json(input_dir) {
        registry.merge(&external);
    }

    registry
}

/// Transpiles a single file in collecting mode, returning unsupported syntax entries.
fn transpile_file_collecting(
    input: &Path,
    output: Option<&Path>,
    use_builtin_types: bool,
) -> Result<Vec<UnsupportedSyntax>> {
    let ts_source = fs::read_to_string(input)
        .with_context(|| format!("failed to read input file: {}", input.display()))?;

    let input_dir = input.parent().unwrap_or(Path::new("."));
    let registry = build_base_registry(input_dir, use_builtin_types);

    let (rs_source, unsupported) =
        ts_to_rs::transpile_collecting_with_registry(&ts_source, &registry)
            .with_context(|| format!("failed to transpile: {}", input.display()))?;

    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| input.with_extension("rs"));

    fs::write(&output_path, &rs_source)
        .with_context(|| format!("failed to write output file: {}", output_path.display()))?;

    run_rustfmt(std::slice::from_ref(&output_path));

    eprintln!("Wrote {}", output_path.display());

    let unsupported = unsupported
        .into_iter()
        .map(|u| UnsupportedSyntax {
            location: format!("{}:{}", input.display(), u.location),
            ..u
        })
        .collect();

    Ok(unsupported)
}

/// Transpiles a single TypeScript file to Rust.
fn transpile_file(input: &Path, output: Option<&Path>, use_builtin_types: bool) -> Result<()> {
    let ts_source = fs::read_to_string(input)
        .with_context(|| format!("failed to read input file: {}", input.display()))?;

    let input_dir = input.parent().unwrap_or(Path::new("."));
    let registry = build_base_registry(input_dir, use_builtin_types);

    let rs_source = ts_to_rs::transpile_with_registry(&ts_source, &registry)
        .with_context(|| format!("failed to transpile: {}", input.display()))?;

    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| input.with_extension("rs"));

    fs::write(&output_path, &rs_source)
        .with_context(|| format!("failed to write output file: {}", output_path.display()))?;

    run_rustfmt(std::slice::from_ref(&output_path));

    eprintln!("Wrote {}", output_path.display());
    Ok(())
}

/// Shared directory transpilation infrastructure: pre-scan, build registry, transpile, generate mod.rs.
///
/// The `transpile_file` callback receives `(ts_source, registry, current_file_dir)` and returns
/// `(rs_source, Vec<UnsupportedSyntax>)`. For default mode, unsupported vec is empty.
fn transpile_directory_common<F>(
    input_dir: &Path,
    output: Option<&Path>,
    use_builtin_types: bool,
    transpile_file: F,
) -> Result<Vec<UnsupportedSyntax>>
where
    F: Fn(&str, &TypeRegistry, Option<&str>) -> Result<(String, Vec<UnsupportedSyntax>)>,
{
    let ts_files = directory::collect_ts_files(input_dir)?;
    directory::validate_has_ts_files(&ts_files, input_dir)?;

    let output_dir = output
        .map(PathBuf::from)
        .unwrap_or_else(|| directory::default_output_dir(input_dir));

    // Pass 1: read all files and build shared registry
    let mut file_sources = Vec::new();
    for ts_path in &ts_files {
        let ts_source = fs::read_to_string(ts_path)
            .with_context(|| format!("failed to read: {}", ts_path.display()))?;
        file_sources.push((ts_path.clone(), ts_source));
    }

    let source_strs: Vec<&str> = file_sources.iter().map(|(_, s)| s.as_str()).collect();
    let mut shared_registry = ts_to_rs::build_shared_registry(&source_strs);

    // Merge base registry (built-in types + optional types.json), lowest priority
    let base_registry = build_base_registry(input_dir, use_builtin_types);
    let mut combined = base_registry;
    combined.merge(&shared_registry);
    shared_registry = combined;

    // Pass 2: transpile each file
    let mut all_unsupported = Vec::new();
    let mut rs_paths = Vec::new();

    for (ts_path, ts_source) in &file_sources {
        let rs_path = directory::compute_output_path(ts_path, input_dir, &output_dir)?;

        // Compute the file's directory relative to the input root (for import path resolution)
        let current_file_dir = ts_path
            .parent()
            .and_then(|p| p.strip_prefix(input_dir).ok())
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty());

        let (rs_source, unsupported) =
            transpile_file(ts_source, &shared_registry, current_file_dir)
                .with_context(|| format!("failed to transpile: {}", ts_path.display()))?;

        if let Some(parent) = rs_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }

        fs::write(&rs_path, &rs_source)
            .with_context(|| format!("failed to write: {}", rs_path.display()))?;

        eprintln!("Wrote {}", rs_path.display());
        rs_paths.push(rs_path);

        for u in unsupported {
            all_unsupported.push(UnsupportedSyntax {
                location: format!("{}:{}", ts_path.display(), u.location),
                ..u
            });
        }
    }

    // Generate mod.rs files
    let output_dirs = directory::collect_output_dirs(&output_dir)?;
    for dir in &output_dirs {
        if let Some(content) = directory::generate_mod_rs(dir)? {
            let mod_rs_path = dir.join("mod.rs");
            fs::write(&mod_rs_path, &content)
                .with_context(|| format!("failed to write: {}", mod_rs_path.display()))?;
            eprintln!("Wrote {}", mod_rs_path.display());
            rs_paths.push(mod_rs_path);
        }
    }

    run_rustfmt(&rs_paths);
    eprintln!("Converted {} file(s)", ts_files.len());

    Ok(all_unsupported)
}

/// Transpiles all `.ts` files in a directory in collecting mode, returning unsupported syntax.
fn transpile_directory_collecting(
    input_dir: &Path,
    output: Option<&Path>,
    use_builtin_types: bool,
) -> Result<Vec<UnsupportedSyntax>> {
    transpile_directory_common(
        input_dir,
        output,
        use_builtin_types,
        |source, reg, file_dir| {
            ts_to_rs::transpile_collecting_with_registry_and_path(source, reg, file_dir)
        },
    )
}

/// Transpiles all `.ts` files in a directory to Rust (default mode — errors on unsupported).
fn transpile_directory(
    input_dir: &Path,
    output: Option<&Path>,
    use_builtin_types: bool,
) -> Result<()> {
    transpile_directory_common(
        input_dir,
        output,
        use_builtin_types,
        |source, reg, file_dir| {
            let rs = ts_to_rs::transpile_with_registry_and_path(source, reg, file_dir)?;
            Ok((rs, vec![]))
        },
    )?;
    Ok(())
}
