mod assembler;
mod catalog;
mod executor;
mod report;

pub use assembler::{assemble_case, default_include_paths, AssemblerConfig, AssemblyOutput};
pub use catalog::{CaseSource, Catalog, CatalogCase};
pub use executor::{CpuBackend, ExecutionConfig, ExecutionOutput, TargetKind};
pub use report::{ActualValue, CaseReport, CaseStatus, RunReport, RunSummary};

use std::collections::BTreeMap;
use std::path::PathBuf;

use runtime_sdk::rtst::{RecordId, Stream};
use thiserror::Error;

/// Mode requested by the CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    List,
    Run,
}

/// Output format for CLI commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Configuration shared by list and run commands.
#[derive(Clone, Debug)]
pub struct RunnerConfig {
    pub workspace_root: PathBuf,
    pub catalog_path: Option<PathBuf>,
}

impl RunnerConfig {
    pub fn new(workspace_root: PathBuf, catalog_path: Option<PathBuf>) -> Self {
        Self {
            workspace_root,
            catalog_path,
        }
    }

    pub fn catalog(&self) -> Result<Catalog, RunnerError> {
        let mut candidates = Vec::new();
        if let Some(path) = &self.catalog_path {
            candidates.push(self.workspace_root.join(path));
        } else {
            candidates.push(
                self.workspace_root
                    .join("crossdev/cross465/tests/catalog.toml"),
            );
            candidates.push(self.workspace_root.join("tests/catalog.toml"));
        }
        for candidate in &candidates {
            if candidate.exists() {
                return Catalog::load(candidate);
            }
        }
        let fallback = candidates.into_iter().next().unwrap_or_else(|| {
            self.workspace_root
                .join("crossdev/cross465/tests/catalog.toml")
        });
        Err(RunnerError::CatalogMissing { path: fallback })
    }
}

/// Filtering options used for list/run commands.
#[derive(Clone, Debug, Default)]
pub struct CaseFilter {
    pub names: Vec<String>,
}

impl CaseFilter {
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn matches(&self, name: &str) -> bool {
        if self.names.is_empty() {
            return true;
        }
        self.names.iter().any(|filter| filter == name)
    }
}

/// Runtime options for `run` mode.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub target: TargetKind,
    pub personality: String,
    pub timeout_ms: Option<u64>,
    pub seed: u64,
    pub asm_override: Option<CaseSource>,
    pub tass_path: PathBuf,
    pub extra_includes: Vec<PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            target: TargetKind::Cross465,
            personality: "modern-retro".to_string(),
            timeout_ms: Some(2_000),
            seed: 0xDEADBEEF,
            asm_override: None,
            tass_path: PathBuf::from("64tass"),
            extra_includes: Vec::new(),
        }
    }
}

/// Errors produced by the runner pipeline.
#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("catalog not found at {path}")]
    CatalogMissing { path: PathBuf },
    #[error("failed to parse catalog {path}: {source}")]
    CatalogParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("assembly source '{path}' not found")]
    AssemblyMissing { path: PathBuf },
    #[error("64tass invocation failed: {message}")]
    AssemblerFailed { message: String },
    #[error("spawn error: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("no cases matched the provided filters")]
    NoMatchingCases,
    #[error("execution timeout after {cycles} cycles (state={state:?}, wpos={wpos})")]
    Timeout {
        cycles: u64,
        state: Option<runtime_sdk::rtst::State>,
        wpos: u16,
    },
    #[error("RTST buffer parse error: {0}")]
    RtstParse(#[from] runtime_sdk::rtst::RtstError),
    #[error("unsupported target '{0}' for the CPU backend")]
    UnsupportedTarget(String),
    #[error("malformed PRG image")]
    MalformedPrg,
}

/// Discover cases according to the provided filters.
pub fn list_cases(
    config: &RunnerConfig,
    filter: &CaseFilter,
) -> Result<Vec<CatalogCase>, RunnerError> {
    let catalog = config.catalog()?;
    let cases: Vec<_> = catalog
        .cases()
        .iter()
        .filter(|case| filter.matches(&case.name))
        .cloned()
        .collect();
    if cases.is_empty() {
        return Err(RunnerError::NoMatchingCases);
    }
    Ok(cases)
}

/// Run cases and return the parsed RTST report.
pub fn run_cases(
    config: &RunnerConfig,
    filter: &CaseFilter,
    opts: &RunOptions,
) -> Result<RunReport, RunnerError> {
    let catalog = config.catalog()?;
    let cases: Vec<_> = match &opts.asm_override {
        Some(override_input) => vec![CatalogCase::adhoc(
            filter
                .names
                .get(0)
                .cloned()
                .unwrap_or_else(|| "adhoc::inline".to_string()),
            override_input.clone(),
            opts.timeout_ms,
        )],
        None => catalog
            .cases()
            .iter()
            .filter(|case| filter.matches(&case.name))
            .cloned()
            .collect(),
    };

    if cases.is_empty() {
        return Err(RunnerError::NoMatchingCases);
    }

    let mut include_paths = default_include_paths(&config.workspace_root, opts.target);
    include_paths.extend(opts.extra_includes.clone());
    let assembler_cfg = AssemblerConfig {
        workspace_root: config.workspace_root.clone(),
        tass_path: opts.tass_path.clone(),
        include_paths,
    };

    let mut reports = Vec::new();
    for case in &cases {
        let assembly = assemble_case(case, &assembler_cfg)?;
        let exec = CpuBackend::execute(
            &assembly.prg,
            ExecutionConfig {
                target: opts.target,
                timeout_ms: opts.timeout_ms.unwrap_or(2_000),
            },
        )?;
        let mut parsed = parse_rtst(&exec)?;
        if parsed.is_empty() {
            parsed.push(CaseReport {
                name: case.name.clone(),
                status: CaseStatus::Failed,
                status_code: None,
                message: Some("RTST stream was empty".to_string()),
                logs: Vec::new(),
                asserts: Vec::new(),
                actuals: BTreeMap::new(),
            });
        }
        reports.extend(parsed);
    }

    Ok(RunReport::from_cases(reports))
}

fn parse_rtst(exec: &ExecutionOutput) -> Result<Vec<CaseReport>, RunnerError> {
    let stream = Stream::parse(&exec.rtst_region)?;
    let mut order = Vec::new();
    let mut cases: BTreeMap<String, CaseReport> = BTreeMap::new();
    let mut current: Option<String> = None;

    for record in stream.iter() {
        let record = record?;
        match record.kind() {
            Some(RecordId::CaseStart) => {
                let info = record.case_start()?;
                current = Some(info.name.to_string());
                order.push(info.name.to_string());
                cases
                    .entry(info.name.to_string())
                    .or_insert_with(|| CaseReport {
                        name: info.name.to_string(),
                        status: CaseStatus::Pending,
                        status_code: None,
                        message: None,
                        logs: Vec::new(),
                        asserts: Vec::new(),
                        actuals: BTreeMap::new(),
                    });
            }
            Some(RecordId::CaseOk) => {
                let outcome = record.case_ok()?;
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        entry.status = CaseStatus::Passed;
                        entry.status_code = Some(outcome.status_code);
                        entry.message = outcome.message.map(|s| s.to_string());
                    }
                }
            }
            Some(RecordId::CaseFail) => {
                let outcome = record.case_fail()?;
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        entry.status = CaseStatus::Failed;
                        entry.status_code = Some(outcome.status_code);
                        entry.message = outcome.message.map(|s| s.to_string());
                    }
                }
            }
            Some(RecordId::Msg) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        entry.logs.push(record.message()?.message.to_string());
                    }
                }
            }
            Some(RecordId::Assert) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        entry.asserts.push(record.assert()?.message.to_string());
                    }
                }
            }
            Some(RecordId::ActualKeyValue) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        let actual = record.actual_kv()?;
                        entry.actuals.insert(
                            actual.key.to_string(),
                            ActualValue::KeyValue {
                                value: actual.value,
                            },
                        );
                    }
                }
            }
            Some(RecordId::ActualHash) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        let hash = record.actual_hash()?;
                        entry
                            .actuals
                            .insert(hash.key.to_string(), ActualValue::Hash { hash: hash.hash });
                    }
                }
            }
            Some(RecordId::ActualMem) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        let mem = record.actual_mem()?;
                        entry.actuals.insert(
                            mem.key.to_string(),
                            ActualValue::Memory {
                                bytes: mem.bytes.to_vec(),
                            },
                        );
                    }
                }
            }
            Some(RecordId::ActualRegs) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        let regs = record.actual_regs()?;
                        entry.actuals.insert(
                            regs.key.to_string(),
                            ActualValue::Regs {
                                a: regs.a,
                                x: regs.x,
                                y: regs.y,
                                sp: regs.sp,
                                status: regs.status,
                                pc: regs.pc,
                            },
                        );
                    }
                }
            }
            Some(RecordId::ActualTime) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        let time = record.actual_time()?;
                        entry.actuals.insert(
                            time.key.to_string(),
                            ActualValue::Time {
                                cycles: time.cycles,
                            },
                        );
                    }
                }
            }
            Some(RecordId::End) | None => {}
        }
    }

    let ordered_cases: Vec<_> = order
        .into_iter()
        .filter_map(|name| cases.remove(&name))
        .collect();
    Ok(ordered_cases)
}
