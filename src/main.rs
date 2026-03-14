use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;

use ts_to_rs::directory;
use ts_to_rs::registry::{build_registry, TypeRegistry};
use ts_to_rs::UnsupportedSyntax;

/// TypeScript to Rust transpiler CLI tool.
#[derive(Parser, Debug)]
#[command(version, about = "Transpile TypeScript source code to Rust")]
struct Args {
    /// Input TypeScript file or directory path
    input: PathBuf,

    /// Output Rust file or directory path (defaults to <input>.rs or <input>_rs/)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Report unsupported syntax as JSON to stdout instead of aborting on errors
    #[arg(long)]
    report_unsupported: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.report_unsupported {
        let unsupported = if args.input.is_dir() {
            transpile_directory_collecting(&args.input, args.output.as_deref())?
        } else {
            transpile_file_collecting(&args.input, args.output.as_deref())?
        };
        let json = serde_json::to_string_pretty(&unsupported)?;
        println!("{json}");
        Ok(())
    } else if args.input.is_dir() {
        transpile_directory(&args.input, args.output.as_deref())
    } else {
        transpile_file(&args.input, args.output.as_deref())
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

/// Transpiles a single file in collecting mode, returning unsupported syntax entries.
fn transpile_file_collecting(
    input: &Path,
    output: Option<&Path>,
) -> Result<Vec<UnsupportedSyntax>> {
    let ts_source = fs::read_to_string(input)
        .with_context(|| format!("failed to read input file: {}", input.display()))?;

    let (rs_source, unsupported) = ts_to_rs::transpile_collecting(&ts_source)
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
fn transpile_file(input: &Path, output: Option<&Path>) -> Result<()> {
    let ts_source = fs::read_to_string(input)
        .with_context(|| format!("failed to read input file: {}", input.display()))?;

    let rs_source = ts_to_rs::transpile(&ts_source)
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
/// The `transpile_file` callback receives `(ts_source, registry)` and returns
/// `(rs_source, Vec<UnsupportedSyntax>)`. For default mode, unsupported vec is empty.
fn transpile_directory_common<F>(
    input_dir: &Path,
    output: Option<&Path>,
    transpile_file: F,
) -> Result<Vec<UnsupportedSyntax>>
where
    F: Fn(&str, &TypeRegistry) -> Result<(String, Vec<UnsupportedSyntax>)>,
{
    let ts_files = directory::collect_ts_files(input_dir)?;
    directory::validate_has_ts_files(&ts_files, input_dir)?;

    let output_dir = output.map(PathBuf::from).unwrap_or_else(|| {
        let mut name = input_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        name.push_str("_rs");
        input_dir.with_file_name(name)
    });

    // Pass 1: pre-scan all files and build shared registry
    let mut file_registries: HashMap<PathBuf, TypeRegistry> = HashMap::new();
    let mut file_sources: HashMap<PathBuf, String> = HashMap::new();
    for ts_path in &ts_files {
        let ts_source = fs::read_to_string(ts_path)
            .with_context(|| format!("failed to read: {}", ts_path.display()))?;
        if let Ok(module) = ts_to_rs::parser::parse_typescript(&ts_source) {
            let reg = build_registry(&module);
            file_registries.insert(ts_path.clone(), reg);
        }
        file_sources.insert(ts_path.clone(), ts_source);
    }

    let mut shared_registry = TypeRegistry::new();
    for reg in file_registries.values() {
        shared_registry.merge(reg);
    }

    // Pass 2: transpile each file
    let mut all_unsupported = Vec::new();
    let mut rs_paths = Vec::new();

    for ts_path in &ts_files {
        let rs_path = directory::compute_output_path(ts_path, input_dir, &output_dir)?;
        let ts_source = &file_sources[ts_path];

        let (rs_source, unsupported) = transpile_file(ts_source, &shared_registry)
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
) -> Result<Vec<UnsupportedSyntax>> {
    transpile_directory_common(input_dir, output, |source, reg| {
        ts_to_rs::transpile_collecting_with_registry(source, reg)
    })
}

/// Transpiles all `.ts` files in a directory to Rust (default mode — errors on unsupported).
fn transpile_directory(input_dir: &Path, output: Option<&Path>) -> Result<()> {
    transpile_directory_common(input_dir, output, |source, reg| {
        let rs = ts_to_rs::transpile_with_registry(source, reg)?;
        Ok((rs, vec![]))
    })?;
    Ok(())
}
