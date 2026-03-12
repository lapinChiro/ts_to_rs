use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;

use ts_to_rs::directory;

/// TypeScript to Rust transpiler CLI tool.
#[derive(Parser, Debug)]
#[command(version, about = "Transpile TypeScript source code to Rust")]
struct Args {
    /// Input TypeScript file or directory path
    input: PathBuf,

    /// Output Rust file or directory path (defaults to <input>.rs or <input>_rs/)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.input.is_dir() {
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

/// Transpiles all `.ts` files in a directory to Rust.
fn transpile_directory(input_dir: &Path, output: Option<&Path>) -> Result<()> {
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

    let mut converted = 0;
    let mut rs_paths = Vec::new();

    for ts_path in &ts_files {
        let rs_path = directory::compute_output_path(ts_path, input_dir, &output_dir)?;

        let ts_source = fs::read_to_string(ts_path)
            .with_context(|| format!("failed to read: {}", ts_path.display()))?;

        let rs_source = ts_to_rs::transpile(&ts_source)
            .with_context(|| format!("failed to transpile: {}", ts_path.display()))?;

        if let Some(parent) = rs_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }

        fs::write(&rs_path, &rs_source)
            .with_context(|| format!("failed to write: {}", rs_path.display()))?;

        eprintln!("Wrote {}", rs_path.display());
        rs_paths.push(rs_path);
        converted += 1;
    }

    // Generate mod.rs files bottom-up
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

    eprintln!("Converted {converted} file(s)");
    Ok(())
}
