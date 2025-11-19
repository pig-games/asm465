use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use serde::Serialize;

/// Cross465 RTST CLI entry point.
///
/// Supports `--mode list` and `--mode run` flows so the RTST stream can be
/// exercised outside of cargo tests.
use cross465_runner::{
    default_personality_for_target, list_cases, run_cases, CaseFilter, CaseReport, CaseSource,
    CaseStatus, Catalog, CatalogCase, CiEndpoint, CiMatrixEntry, CiRemoteFailure, RunOptions,
    RunReport, RunSummary, RunnerConfig, RunnerError, TargetKind,
};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut workspace = resolve_workspace_arg(cli.workspace.as_ref())?;
    let mut catalog_path = resolve_catalog_path(&workspace, cli.catalog.as_ref());
    let mut catalog_for_cli = Catalog::load(&catalog_path).map_err(to_anyhow)?;
    if cli.workspace.is_none() {
        if let Some(ws_override) = catalog_for_cli.workspace_override() {
            workspace = ws_override.clone();
            catalog_path = resolve_catalog_path(&workspace, cli.catalog.as_ref());
            catalog_for_cli = Catalog::load(&catalog_path).map_err(to_anyhow)?;
        }
    }
    let workspace = workspace.canonicalize().unwrap_or(workspace);
    let mut catalog_for_cli = Some(catalog_for_cli);
    let config = RunnerConfig::new(workspace.clone(), Some(catalog_path.clone()));
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
            let catalog = catalog_for_cli
                .take()
                .unwrap_or_else(|| config.catalog().map_err(to_anyhow).expect("load catalog"));
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
            if let Some(host) = &cli.ultimate64_host {
                std::env::set_var("CROSS465_ULTIMATE64_HOST", host);
            }
            if let Some(port) = cli.ultimate64_port {
                std::env::set_var("CROSS465_ULTIMATE64_PORT", port.to_string());
            }
            if let Some(host) = &cli.asm465_host {
                std::env::set_var("CROSS465_NATIVE_HOST", host);
            }
            if let Some(port) = cli.asm465_port {
                std::env::set_var("CROSS465_NATIVE_PORT", port.to_string());
            }
            if let Some(host) = &cli.asm465_bridge_host {
                std::env::set_var("CROSS465_BRIDGE_HOST", host);
            }
            if let Some(port) = cli.asm465_bridge_port {
                std::env::set_var("CROSS465_BRIDGE_PORT", port.to_string());
            }
            if let Some(host) = &cli.asm465_ws_host {
                std::env::set_var("CROSS465_BRIDGE_WS_HOST", host);
            }
            if let Some(port) = cli.asm465_ws_port {
                std::env::set_var("CROSS465_BRIDGE_WS_PORT", port.to_string());
            }
            if let Some(cycles) = cli.asm465_max_cycles {
                std::env::set_var("CROSS465_MAX_CYCLES", cycles.to_string());
            }
            if cli.asm465_keep_alive {
                std::env::set_var("CROSS465_ASM465_KEEP_ALIVE", "1");
            }
            let fixture_dir = cli
                .fixtures
                .as_ref()
                .and_then(|opt| match opt {
                    Some(path) => Some(if path.is_absolute() {
                        path.clone()
                    } else {
                        workspace.join(path)
                    }),
                    None => Some(workspace.join("tests/fixtures")),
                })
                .or_else(|| {
                    if cli.update_fixtures {
                        Some(workspace.join("tests/fixtures"))
                    } else {
                        None
                    }
                });
            let artifact_dir = if cli.no_artifacts {
                None
            } else {
                let default_dir = workspace.join("target/cross465-runner");
                let path = cli
                    .artifacts
                    .as_ref()
                    .map_or(Some(default_dir.clone()), |opt| {
                        opt.as_ref().map(|path| {
                            if path.is_absolute() {
                                path.clone()
                            } else {
                                workspace.join(path)
                            }
                        })
                    });
                path.or(Some(default_dir))
            };
            if cli.ci_matrix && cli.personality.is_some() {
                bail!(
                    "--personality cannot be combined with --ci-matrix; specify combos in catalog"
                );
            }

            let mut base_opts = RunOptions {
                target,
                personality: cli.personality.clone(),
                timeout_ms: Some(cli.timeout_ms),
                seed: cli.seed,
                asm_override,
                tass_path: cli.tass.clone().unwrap_or_else(|| PathBuf::from("64tass")),
                extra_includes,
                fixture_dir,
                update_fixtures: cli.update_fixtures,
                extra_defines: Vec::new(),
                tass_args: Vec::new(),
                artifact_dir,
                keep_success_artifacts: cli.keep_success_artifacts,
                log_metrics: cli.log_metrics,
                progress_timeout_ms: cli.progress_timeout_ms,
                transport_retries: cli.transport_retries,
            };

            if cli.ci_matrix {
                if base_opts.asm_override.is_some() {
                    bail!("--ci-matrix cannot be combined with --asm-path/--asm-inline overrides");
                }
                let matrix_entries = catalog.ci_matrix().to_vec();
                if matrix_entries.is_empty() {
                    bail!("catalog does not define any [ci.matrix.<target>] entries");
                }
                let mut total_failed = 0usize;
                for entry in matrix_entries {
                    let target = entry.target;
                    let resolved_includes = resolve_include_paths(&workspace, &entry.includes);
                    let entry_policy = entry
                        .remote_failure
                        .map(RemoteFailurePolicy::from)
                        .unwrap_or(cli.remote_failure);
                    for personality in entry.personalities.iter().cloned() {
                        let label = describe_personality(target, personality.as_deref());
                        if cli.format == FormatArg::Text {
                            println!(
                                "\n==> target={} personality={} <==",
                                target.to_string(),
                                label
                            );
                        }
                        if let Some(endpoint) = &entry.endpoint {
                            apply_endpoint(target, endpoint);
                        }
                        let mut opts = base_opts.clone();
                        opts.target = target;
                        opts.personality = personality;
                        opts.extra_includes.extend(resolved_includes.clone());
                        opts.extra_defines.extend(entry.defines.iter().cloned());
                        opts.tass_args.extend(entry.tass_args.iter().cloned());
                        let start = Instant::now();
                        match run_cases(&config, &filter, &opts) {
                            Ok(report) => {
                                render_for_format(
                                    cli.format,
                                    &report,
                                    opts.target,
                                    &label,
                                    start.elapsed(),
                                )?;
                                total_failed += report.summary.failed;
                            }
                            Err(err) => {
                                if downgrade_remote_failure(entry_policy, &err) {
                                    warn_remote_failure(opts.target, &label, &err);
                                    continue;
                                } else {
                                    return Err(to_anyhow(err));
                                }
                            }
                        }
                    }
                }
                if total_failed > 0 {
                    process::exit(1);
                }
            } else {
                let mut entry_policy = cli.remote_failure;
                if let Some(entry) = catalog.ci_matrix().iter().find(|ci| ci.target == target) {
                    apply_entry_overrides(&workspace, entry, &mut base_opts);
                    if let Some(endpoint) = &entry.endpoint {
                        apply_endpoint(target, endpoint);
                    }
                    if let Some(mode) = entry.remote_failure {
                        entry_policy = RemoteFailurePolicy::from(mode);
                    }
                }
                let label =
                    describe_personality(base_opts.target, base_opts.personality.as_deref());
                let start = Instant::now();
                match run_cases(&config, &filter, &base_opts) {
                    Ok(report) => {
                        render_for_format(
                            cli.format,
                            &report,
                            base_opts.target,
                            &label,
                            start.elapsed(),
                        )?;
                        if report.summary.failed > 0 {
                            process::exit(1);
                        }
                    }
                    Err(err) => {
                        if downgrade_remote_failure(entry_policy, &err) {
                            warn_remote_failure(base_opts.target, &label, &err);
                            return Ok(());
                        } else {
                            return Err(to_anyhow(err));
                        }
                    }
                }
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
    #[arg(long)]
    personality: Option<String>,
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
    #[arg(long = "include", num_args = 1.., value_name = "PATH")]
    include: Vec<PathBuf>,
    #[arg(long = "case")]
    case: Vec<String>,
    #[arg(long = "asm-path")]
    asm_path: Option<PathBuf>,
    #[arg(long = "asm-inline")]
    asm_inline: Option<String>,
    #[arg(long = "fixtures", value_name = "DIR", num_args = 0..=1)]
    fixtures: Option<Option<PathBuf>>,
    #[arg(long = "update-fixtures")]
    update_fixtures: bool,
    #[arg(long = "ci-matrix")]
    ci_matrix: bool,
    #[arg(long = "artifacts", value_name = "DIR", num_args = 0..=1)]
    artifacts: Option<Option<PathBuf>>,
    #[arg(long = "no-artifacts")]
    no_artifacts: bool,
    #[arg(long = "keep-success-artifacts")]
    keep_success_artifacts: bool,
    #[arg(long = "progress-timeout-ms", default_value_t = 750)]
    progress_timeout_ms: u64,
    #[arg(long = "transport-retries", default_value_t = 3)]
    transport_retries: u32,
    #[arg(long = "log-metrics")]
    log_metrics: bool,
    #[arg(long = "remote-failure", value_enum, default_value = "error")]
    remote_failure: RemoteFailurePolicy,
    #[arg(long = "ultimate64-host", value_name = "HOST")]
    ultimate64_host: Option<String>,
    #[arg(long = "ultimate64-port", value_name = "PORT")]
    ultimate64_port: Option<u16>,
    #[arg(long = "asm465-host", value_name = "HOST")]
    asm465_host: Option<String>,
    #[arg(long = "asm465-port", value_name = "PORT")]
    asm465_port: Option<u16>,
    #[arg(long = "asm465-bridge-host", value_name = "HOST")]
    asm465_bridge_host: Option<String>,
    #[arg(long = "asm465-bridge-port", value_name = "PORT")]
    asm465_bridge_port: Option<u16>,
    #[arg(long = "asm465-ws-host", value_name = "HOST")]
    asm465_ws_host: Option<String>,
    #[arg(long = "asm465-ws-port", value_name = "PORT")]
    asm465_ws_port: Option<u16>,
    #[arg(long = "asm465-max-cycles", value_name = "CYCLES")]
    asm465_max_cycles: Option<u64>,
    #[arg(long = "asm465-keep-alive", help = "Leave auto-started asm465 runtimes alive after the run")]
    asm465_keep_alive: bool,
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
    Asm465,
    Asm465Wasm,
}

impl From<TargetArg> for TargetKind {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Cross465 => TargetKind::Cross465,
            TargetArg::Ultimate64 => TargetKind::Ultimate64,
            TargetArg::Mega65 => TargetKind::Mega65,
            TargetArg::Asm465 => TargetKind::Asm465Native,
            TargetArg::Asm465Wasm => TargetKind::Asm465Wasm,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum FormatArg {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum RemoteFailurePolicy {
    Error,
    Warn,
}

impl From<CiRemoteFailure> for RemoteFailurePolicy {
    fn from(value: CiRemoteFailure) -> Self {
        match value {
            CiRemoteFailure::Error => RemoteFailurePolicy::Error,
            CiRemoteFailure::Warn => RemoteFailurePolicy::Warn,
        }
    }
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

fn render_run_json(
    report: &RunReport,
    target: TargetKind,
    personality: &str,
    duration: std::time::Duration,
) -> Result<()> {
    #[derive(Serialize)]
    struct Payload<'a> {
        target: String,
        personality: &'a str,
        duration_ms: u128,
        summary: &'a RunSummary,
        cases: &'a [CaseReport],
    }
    let payload = Payload {
        target: target.to_string().to_string(),
        personality,
        duration_ms: duration.as_millis(),
        summary: &report.summary,
        cases: &report.cases,
    };
    serde_json::to_writer_pretty(io::stdout(), &payload)?;
    println!();
    Ok(())
}

fn render_for_format(
    format: FormatArg,
    report: &RunReport,
    target: TargetKind,
    personality: &str,
    duration: Duration,
) -> Result<()> {
    match format {
        FormatArg::Text => render_run_text(report, duration),
        FormatArg::Json => render_run_json(report, target, personality, duration),
    }
}

fn describe_personality(target: TargetKind, explicit: Option<&str>) -> String {
    if let Some(value) = explicit {
        value.to_string()
    } else if let Some(default) = default_personality_for_target(target) {
        default.to_string()
    } else {
        format!("{}-builtin", target.to_string())
    }
}

fn resolve_include_path(workspace: &Path, include: &Path) -> PathBuf {
    if include.is_absolute() {
        include.to_path_buf()
    } else {
        workspace.join(include)
    }
}

fn resolve_include_paths(workspace: &Path, includes: &[PathBuf]) -> Vec<PathBuf> {
    includes
        .iter()
        .map(|p| resolve_include_path(workspace, p))
        .collect()
}

fn apply_entry_overrides(workspace: &Path, entry: &CiMatrixEntry, opts: &mut RunOptions) {
    let resolved = resolve_include_paths(workspace, &entry.includes);
    opts.extra_includes.extend(resolved);
    opts.extra_defines.extend(entry.defines.iter().cloned());
    opts.tass_args.extend(entry.tass_args.iter().cloned());
}

fn apply_endpoint(target: TargetKind, endpoint: &CiEndpoint) {
    match target {
        TargetKind::Ultimate64 => {
            if let Some(host) = &endpoint.host {
                env::set_var("CROSS465_ULTIMATE64_HOST", host);
            }
            if let Some(port) = endpoint.port {
                env::set_var("CROSS465_ULTIMATE64_PORT", port.to_string());
            }
        }
        TargetKind::Mega65 => {
            // Future: add serial endpoint support.
        }
        TargetKind::Asm465Native => {
            if let Some(host) = &endpoint.host {
                env::set_var("CROSS465_NATIVE_HOST", host);
            }
            if let Some(port) = endpoint.port {
                env::set_var("CROSS465_NATIVE_PORT", port.to_string());
            }
        }
        TargetKind::Asm465Wasm => {
            if let Some(host) = &endpoint.host {
                env::set_var("CROSS465_BRIDGE_HOST", host);
                env::set_var("CROSS465_BRIDGE_WS_HOST", host);
            }
            if let Some(port) = endpoint.port {
                env::set_var("CROSS465_BRIDGE_PORT", port.to_string());
            }
        }
        TargetKind::Cross465 => {}
    }
}

fn default_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(3)
        .unwrap_or(manifest_dir.as_path())
        .to_path_buf()
}

fn resolve_workspace_arg(arg: Option<&PathBuf>) -> Result<PathBuf> {
    let path = match arg {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => env::current_dir()
            .context("failed to resolve workspace")?
            .join(path),
        None => default_workspace_root(),
    };
    Ok(path)
}

fn resolve_catalog_path(workspace: &Path, cli_path: Option<&PathBuf>) -> PathBuf {
    let path = match cli_path {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => workspace.join(path),
        None => workspace.join("crossdev/cross465/tests/catalog.toml"),
    };
    path
}

fn downgrade_remote_failure(policy: RemoteFailurePolicy, err: &RunnerError) -> bool {
    matches!(policy, RemoteFailurePolicy::Warn) && is_remote_transport_error(err)
}

fn is_remote_transport_error(err: &RunnerError) -> bool {
    matches!(
        err,
        RunnerError::Ultimate64Error { .. }
            | RunnerError::Mega65Error { .. }
            | RunnerError::Asm465Error { .. }
    )
}

fn warn_remote_failure(target: TargetKind, personality: &str, err: &RunnerError) {
    eprintln!(
        "warning: remote run skipped for target={} personality={} => {}",
        target.to_string(),
        personality,
        err
    );
}
