use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

/// TypeScript to Rust transpiler CLI tool.
#[derive(Parser, Debug)]
#[command(version, about = "Transpile TypeScript source code to Rust")]
struct Args {
    /// Input TypeScript file path
    input: PathBuf,

    /// Output Rust file path (defaults to <input>.rs)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let ts_source = fs::read_to_string(&args.input)
        .with_context(|| format!("failed to read input file: {}", args.input.display()))?;

    let rs_source = ts_to_rs::transpile(&ts_source)
        .with_context(|| format!("failed to transpile: {}", args.input.display()))?;

    let output_path = args
        .output
        .unwrap_or_else(|| args.input.with_extension("rs"));

    fs::write(&output_path, &rs_source)
        .with_context(|| format!("failed to write output file: {}", output_path.display()))?;

    eprintln!("Wrote {}", output_path.display());
    Ok(())
}
