use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use serde::Serialize;

use cross465_runner::{
    list_cases, run_cases, CaseFilter, CaseReport, CaseSource, CaseStatus, CatalogCase, RunOptions,
    RunReport, RunSummary, RunnerConfig, RunnerError, TargetKind,
};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace = cli
        .workspace
        .clone()
        .unwrap_or(std::env::current_dir().context("failed to resolve workspace")?);
    let workspace = workspace.canonicalize().unwrap_or(workspace);
    let config = RunnerConfig::new(workspace.clone(), cli.catalog.clone());
    let filter = CaseFilter {
        names: cli.case.clone(),
    };

    match cli.mode {
        ModeArg::List => {
            let cases = list_cases(&config, &filter).map_err(to_anyhow)?;
            match cli.format {
                FormatArg::Text => render_list_text(&cases),
                FormatArg::Json => render_list_json(&cases)?,
            }
        }
        ModeArg::Run => {
            let target = cli.target.into();
            let mut extra_includes = Vec::new();
            for extra in &cli.include {
                let path = if extra.is_absolute() {
                    extra.clone()
                } else {
                    workspace.join(extra)
                };
                extra_includes.push(path);
            }
            let asm_override = build_override_source(&cli, &workspace)?;
            let opts = RunOptions {
                target,
                personality: cli.personality.clone(),
                timeout_ms: Some(cli.timeout_ms),
                seed: cli.seed,
                asm_override,
                tass_path: cli.tass.clone().unwrap_or_else(|| PathBuf::from("64tass")),
                extra_includes,
            };
            let start = Instant::now();
            let report = run_cases(&config, &filter, &opts).map_err(to_anyhow)?;
            match cli.format {
                FormatArg::Text => render_run_text(&report, start.elapsed())?,
                FormatArg::Json => render_run_json(&report, &cli, start.elapsed())?,
            }
            if report.summary.failed > 0 {
                process::exit(1);
            }
        }
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Cross465 Runtime SDK Test Runner")]
struct Cli {
    #[arg(long, value_enum, default_value = "run")]
    mode: ModeArg,
    #[arg(long, value_enum, default_value = "cross465")]
    target: TargetArg,
    #[arg(long, default_value = "modern-retro")]
    personality: String,
    #[arg(long, value_enum, default_value = "text")]
    format: FormatArg,
    #[arg(long, default_value_t = 2000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 0xDEADBEEF)]
    seed: u64,
    #[arg(long)]
    catalog: Option<PathBuf>,
    #[arg(long)]
    workspace: Option<PathBuf>,
    #[arg(long)]
    tass: Option<PathBuf>,
    #[arg(long = "include")]
    include: Vec<PathBuf>,
    #[arg(long = "case")]
    case: Vec<String>,
    #[arg(long = "asm-path")]
    asm_path: Option<PathBuf>,
    #[arg(long = "asm-inline")]
    asm_inline: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ModeArg {
    List,
    Run,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TargetArg {
    Cross465,
    Ultimate64,
    Mega65,
}

impl From<TargetArg> for TargetKind {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Cross465 => TargetKind::Cross465,
            TargetArg::Ultimate64 => TargetKind::Ultimate64,
            TargetArg::Mega65 => TargetKind::Mega65,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FormatArg {
    Text,
    Json,
}

fn build_override_source(cli: &Cli, workspace: &Path) -> Result<Option<CaseSource>> {
    match (&cli.asm_path, &cli.asm_inline) {
        (Some(path), None) => {
            let resolved = if path.is_absolute() {
                path.clone()
            } else {
                workspace.join(path)
            };
            Ok(Some(CaseSource::File(resolved)))
        }
        (None, Some(src)) => Ok(Some(CaseSource::Inline(src.clone()))),
        (Some(_), Some(_)) => bail!("use either --asm-path or --asm-inline"),
        (None, None) => Ok(None),
    }
}

fn to_anyhow(err: RunnerError) -> anyhow::Error {
    anyhow::Error::new(err)
}

fn render_list_text(cases: &[CatalogCase]) {
    for case in cases {
        println!("{}", case.name);
    }
    println!("total {} cases", cases.len());
}

fn render_list_json(cases: &[CatalogCase]) -> Result<()> {
    #[derive(Serialize)]
    struct CaseEntry<'a> {
        name: &'a str,
    }
    #[derive(Serialize)]
    struct Payload<'a> {
        total: usize,
        cases: Vec<CaseEntry<'a>>,
    }
    let entries = cases
        .iter()
        .map(|case| CaseEntry { name: &case.name })
        .collect();
    let payload = Payload {
        total: cases.len(),
        cases: entries,
    };
    serde_json::to_writer_pretty(io::stdout(), &payload)?;
    println!();
    Ok(())
}

fn render_run_text(report: &RunReport, duration: std::time::Duration) -> Result<()> {
    println!("running {} tests", report.summary.total);
    for case in &report.cases {
        let status = match case.status {
            CaseStatus::Passed => "ok",
            CaseStatus::Failed => "FAILED",
            CaseStatus::Pending => "pending",
        };
        println!("test {:<48} ... {}", case.name, status);
        if case.status.is_failed() {
            if let Some(msg) = &case.message {
                println!("    message: {}", msg);
            }
            for assert in &case.asserts {
                println!("    assert: {}", assert);
            }
        }
    }
    let status = if report.summary.failed == 0 {
        "ok"
    } else {
        "FAILED"
    };
    println!();
    println!(
        "test result: {status}. {} passed; {} failed; 0 ignored; 0 measured; 0 filtered out",
        report.summary.passed, report.summary.failed
    );
    println!("finished in {:.2?}", duration);
    Ok(())
}

fn render_run_json(report: &RunReport, cli: &Cli, duration: std::time::Duration) -> Result<()> {
    #[derive(Serialize)]
    struct Payload<'a> {
        target: &'a str,
        personality: &'a str,
        duration_ms: u128,
        summary: &'a RunSummary,
        cases: &'a [CaseReport],
    }
    let payload = Payload {
        target: match cli.target {
            TargetArg::Cross465 => "cross465",
            TargetArg::Ultimate64 => "ultimate64",
            TargetArg::Mega65 => "mega65",
        },
        personality: &cli.personality,
        duration_ms: duration.as_millis(),
        summary: &report.summary,
        cases: &report.cases,
    };
    serde_json::to_writer_pretty(io::stdout(), &payload)?;
    println!();
    Ok(())
}
