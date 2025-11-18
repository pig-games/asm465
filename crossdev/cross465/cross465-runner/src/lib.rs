mod artifacts;
mod asmtest;
mod assembler;
mod catalog;
mod executor;
mod expect;
mod fixtures;
mod report;

pub use asmtest::{run_asm6502_case, AsmTestBuilder, AsmTestResult};
pub use assembler::{assemble_case, default_include_paths, AssemblerConfig, AssemblyOutput};
pub use catalog::{CaseSource, Catalog, CatalogCase, CiEndpoint, CiMatrixEntry};
pub use executor::{
    backend_for_target, Cross465Backend, ExecutionConfig, ExecutionOutput, Mega65Backend,
    TargetBackend, TargetKind, Ultimate64Backend,
};
pub use expect::{
    assert_case_failed, assert_case_ok, expect_eq, expect_hash_eq, expect_in, expect_mem_eq,
    AssertError, AssertResult, CaseAccessor, CaseActuals, ExpectError, ExpectResult,
};
pub use report::{
    ActualCollections, ActualValue, CaseMetrics, CaseReport, CaseStatus, Registers, RunReport,
    RunSummary,
};

use std::collections::BTreeMap;
use std::path::PathBuf;

use artifacts::ArtifactStore;
use runtime_sdk::rtst::{RecordId, Stream};
use thiserror::Error;

use fixtures::FixtureStore;

/// Mode requested by the CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// CLI mode requested by the user (`list` vs `run`).
pub enum Mode {
    List,
    Run,
}

/// Output format for CLI commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Available output formats for list/run modes.
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
    /// Construct a config rooted at `workspace_root` with an optional catalog override.
    pub fn new(workspace_root: PathBuf, catalog_path: Option<PathBuf>) -> Self {
        Self {
            workspace_root,
            catalog_path,
        }
    }

    /// Load the manifest catalog from the explicit override or default search locations.
    pub fn catalog(&self) -> Result<Catalog, RunnerError> {
        let mut candidates = Vec::new();
        if let Some(path) = &self.catalog_path {
            if path.is_absolute() {
                candidates.push(path.clone());
            } else {
                candidates.push(self.workspace_root.join(path));
            }
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
/// Case filtering options (names passed via CLI).
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
/// Runtime options for running assemblable RTST cases.
pub struct RunOptions {
    pub target: TargetKind,
    pub personality: Option<String>,
    pub timeout_ms: Option<u64>,
    pub seed: u64,
    pub asm_override: Option<CaseSource>,
    pub tass_path: PathBuf,
    pub extra_includes: Vec<PathBuf>,
    pub fixture_dir: Option<PathBuf>,
    pub update_fixtures: bool,
    pub extra_defines: Vec<(String, String)>,
    pub tass_args: Vec<String>,
    pub artifact_dir: Option<PathBuf>,
    pub keep_success_artifacts: bool,
    pub log_metrics: bool,
    pub progress_timeout_ms: u64,
    pub transport_retries: u32,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            target: TargetKind::Cross465,
            personality: Some("modern-retro".to_string()),
            timeout_ms: Some(2_000),
            seed: 0xDEADBEEF,
            asm_override: None,
            tass_path: PathBuf::from("64tass"),
            extra_includes: Vec::new(),
            fixture_dir: None,
            update_fixtures: false,
            extra_defines: Vec::new(),
            tass_args: Vec::new(),
            artifact_dir: None,
            keep_success_artifacts: false,
            log_metrics: false,
            progress_timeout_ms: 750,
            transport_retries: 3,
        }
    }
}

/// Errors produced by the runner pipeline.
#[derive(Debug, Error)]
/// Errors surfaced by the runner pipeline.
pub enum RunnerError {
    #[error("catalog not found at {path}")]
    CatalogMissing { path: PathBuf },
    #[error("failed to parse catalog {path}: {source}")]
    CatalogParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid catalog configuration: {message}")]
    CatalogInvalid { message: String },
    #[error("assembly source '{path}' not found")]
    AssemblyMissing { path: PathBuf },
    #[error("64tass invocation failed: {message}")]
    AssemblerFailed { message: String },
    #[error("spawn error: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("no cases matched the provided filters")]
    NoMatchingCases,
    #[error(
        "execution timeout after {cycles} cycles (state={state:?}, wpos={wpos}, pc={pc:#06x})"
    )]
    Timeout {
        cycles: u64,
        state: Option<runtime_sdk::rtst::State>,
        wpos: u16,
        pc: u16,
    },
    #[error("protocol initialization failed: {source}")]
    ProtocolInit {
        #[source]
        source: runtime_sdk::rtst::RtstError,
    },
    #[error("no RTST progress for {elapsed_ms} ms (state={state:?}, wpos={wpos})")]
    NoProgress {
        state: Option<runtime_sdk::rtst::State>,
        wpos: u16,
        elapsed_ms: u64,
    },
    #[error("RTST buffer parse error: {0}")]
    RtstParse(#[from] runtime_sdk::rtst::RtstError),
    #[error("unsupported target '{0}' for the CPU backend")]
    UnsupportedTarget(String),
    #[error("malformed PRG image")]
    MalformedPrg,
    #[error("ultimate64 backend error: {message}")]
    Ultimate64Error { message: String },
    #[error("mega65 backend error: {message}")]
    Mega65Error { message: String },
    #[error("fixture error at {path}: {message}")]
    FixtureIo { path: PathBuf, message: String },
    #[error("asm6502 invocation error: {message}")]
    InvalidInvocation { message: String },
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
        target: opts.target,
        defines: opts.extra_defines.clone(),
        extra_args: opts.tass_args.clone(),
    };

    let fixture_store = FixtureStore::new(
        &config.workspace_root,
        opts.target,
        opts.fixture_dir.clone(),
        opts.update_fixtures,
    );
    let artifact_store = ArtifactStore::new(
        opts.artifact_dir.clone(),
        opts.target,
        opts.keep_success_artifacts,
    );
    let mut reports = Vec::new();
    let personality_label = effective_personality(opts);
    for case in &cases {
        let backend = executor::backend_for_target(opts.target);
        let assembly = assemble_case(case, &assembler_cfg)?;
        let exec = match backend.run(
            &assembly.prg,
            ExecutionConfig {
                target: opts.target,
                timeout_ms: opts.timeout_ms.unwrap_or(2_000),
                progress_timeout_ms: opts.progress_timeout_ms,
                transport_retries: opts.transport_retries,
            },
        ) {
            Ok(output) => output,
            Err(err) => {
                artifact_store.capture_backend_error(
                    &case.name,
                    &personality_label,
                    &assembly.prg,
                    &err,
                );
                return Err(err);
            }
        };
        let (mut parsed, metrics) = match parse_rtst(&exec) {
            Ok(result) => result,
            Err(err) => {
                artifact_store.capture_parse_error(
                    &case.name,
                    &personality_label,
                    &assembly.prg,
                    &exec,
                    &err,
                );
                return Err(err);
            }
        };
        if parsed.is_empty() {
            let mut placeholder = CaseReport {
                name: case.name.clone(),
                status: CaseStatus::Failed,
                status_code: None,
                message: Some("RTST stream was empty".to_string()),
                logs: Vec::new(),
                asserts: Vec::new(),
                actuals: BTreeMap::new(),
                actual_groups: ActualCollections::default(),
                metrics: Some(metrics.clone()),
            };
            fixture_store.apply(&mut placeholder)?;
            artifact_store.capture_case(&placeholder, &personality_label, &assembly.prg, &exec);
            if opts.log_metrics {
                log_case_metrics(&placeholder);
            }
            reports.push(placeholder);
            continue;
        }
        for report in &mut parsed {
            if let Err(err) = fixture_store.apply(report) {
                artifact_store.capture_case(report, &personality_label, &assembly.prg, &exec);
                if opts.log_metrics {
                    log_case_metrics(report);
                }
                return Err(err);
            }
            artifact_store.capture_case(report, &personality_label, &assembly.prg, &exec);
            if opts.log_metrics {
                log_case_metrics(report);
            }
        }
        reports.extend(parsed);
    }

    Ok(RunReport::from_cases(reports))
}

fn parse_rtst(exec: &ExecutionOutput) -> Result<(Vec<CaseReport>, CaseMetrics), RunnerError> {
    let stream = Stream::parse(&exec.rtst_region)?;
    let metrics = CaseMetrics {
        cycles: exec.cycles,
        rtst_bytes: exec.rtst_region.len(),
        write_pos: stream.header().write_pos(),
    };
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
                        actual_groups: ActualCollections::default(),
                        metrics: None,
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
                        let key = actual.key.to_string();
                        entry
                            .actual_groups
                            .scalars
                            .insert(key.clone(), actual.value);
                        entry.actuals.insert(
                            key,
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
                        let key = hash.key.to_string();
                        entry.actual_groups.hashes.insert(key.clone(), hash.hash);
                        entry
                            .actuals
                            .insert(key, ActualValue::Hash { hash: hash.hash });
                    }
                }
            }
            Some(RecordId::ActualMem) => {
                if let Some(name) = current.clone() {
                    if let Some(entry) = cases.get_mut(&name) {
                        let mem = record.actual_mem()?;
                        let key = mem.key.to_string();
                        entry
                            .actual_groups
                            .memory
                            .insert(key.clone(), mem.bytes.to_vec());
                        entry.actuals.insert(
                            key,
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
                        let key = regs.key.to_string();
                        entry.actual_groups.registers.insert(
                            key.clone(),
                            Registers {
                                a: regs.a,
                                x: regs.x,
                                y: regs.y,
                                sp: regs.sp,
                                status: regs.status,
                                pc: regs.pc,
                            },
                        );
                        entry.actuals.insert(
                            key,
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
                        let key = time.key.to_string();
                        entry.actual_groups.timings.insert(key.clone(), time.cycles);
                        entry.actuals.insert(
                            key,
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

    let mut ordered_cases: Vec<_> = order
        .into_iter()
        .filter_map(|name| cases.remove(&name))
        .collect();
    for case in &mut ordered_cases {
        case.metrics = Some(metrics.clone());
    }
    Ok((ordered_cases, metrics))
}

fn log_case_metrics(report: &CaseReport) {
    if let Some(metrics) = &report.metrics {
        println!(
            "metric case={} status={} cycles={} rtst_bytes={} write_pos={}",
            report.name,
            report.status.as_str(),
            metrics.cycles,
            metrics.rtst_bytes,
            metrics.write_pos,
        );
    }
}

fn effective_personality(opts: &RunOptions) -> String {
    opts.personality
        .clone()
        .or_else(|| default_personality_for_target(opts.target).map(|s| s.to_string()))
        .unwrap_or_else(|| format!("{}-builtin", opts.target.to_string()))
}

/// Default MMIO personality associated with a target (if any).
pub fn default_personality_for_target(target: TargetKind) -> Option<&'static str> {
    match target {
        TargetKind::Cross465 => Some("modern-retro"),
        TargetKind::Ultimate64 => None,
        TargetKind::Mega65 => Some("modern-retro"),
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;
    use runtime_sdk::rtst::{RecordId, State, StreamEncoder};

    fn payload_with_key(key: &str, mut rest: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(key.len() + 1 + rest.len());
        buf.extend_from_slice(key.as_bytes());
        buf.push(0);
        buf.append(&mut rest);
        buf
    }

    fn kv_payload(key: &str, value: u32) -> Vec<u8> {
        payload_with_key(key, value.to_le_bytes().to_vec())
    }

    fn hash_payload(key: &str, hash: u32) -> Vec<u8> {
        payload_with_key(key, hash.to_le_bytes().to_vec())
    }

    fn time_payload(key: &str, cycles: u32) -> Vec<u8> {
        payload_with_key(key, cycles.to_le_bytes().to_vec())
    }

    fn mem_payload(key: &str, bytes: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        body.extend_from_slice(bytes);
        payload_with_key(key, body)
    }

    fn regs_payload(key: &str, regs: (u8, u8, u8, u8, u8, u16)) -> Vec<u8> {
        let mut body = Vec::new();
        body.push(regs.0);
        body.push(regs.1);
        body.push(regs.2);
        body.push(regs.3);
        body.push(regs.4);
        body.extend_from_slice(&regs.5.to_le_bytes());
        payload_with_key(key, body)
    }

    #[test]
    fn parse_rtst_populates_actual_groups() {
        let mut enc = StreamEncoder::new();
        enc.push_record(RecordId::CaseStart, b"demo\0").unwrap();
        enc.push_record(RecordId::ActualKeyValue, kv_payload("score", 0x10))
            .unwrap();
        enc.push_record(RecordId::ActualHash, hash_payload("frame", 0xDEADBEEF))
            .unwrap();
        enc.push_record(
            RecordId::ActualMem,
            mem_payload("dump", &[0x00, 0x01, 0x02]),
        )
        .unwrap();
        enc.push_record(
            RecordId::ActualRegs,
            regs_payload("regs", (1, 2, 3, 4, 0x24, 0x1234)),
        )
        .unwrap();
        enc.push_record(RecordId::ActualTime, time_payload("elapsed", 500))
            .unwrap();
        enc.push_record(RecordId::CaseOk, &[0]).unwrap();
        enc.push_record(RecordId::End, &[]).unwrap();
        enc.set_counts(1, 1, 0).set_state(State::Done);
        let buffer = enc.finish();
        let exec = ExecutionOutput {
            rtst_region: buffer,
            cycles: 0,
        };
        let (cases, metrics) = parse_rtst(&exec).expect("parse");
        assert_eq!(cases.len(), 1);
        let case = &cases[0];
        assert_eq!(case.actual_groups.scalars.get("score"), Some(&0x10));
        assert_eq!(case.actual_groups.hashes.get("frame"), Some(&0xDEADBEEF));
        assert_eq!(
            case.actual_groups.memory.get("dump").map(|v| v.as_slice()),
            Some(&[0x00, 0x01, 0x02][..])
        );
        assert_eq!(
            case.actual_groups.registers.get("regs").map(|r| r.a),
            Some(1)
        );
        assert_eq!(case.actual_groups.timings.get("elapsed"), Some(&500u32));
        assert_eq!(case.actuals.len(), 5);
        assert_eq!(metrics.rtst_bytes, exec.rtst_region.len());
        assert!(metrics.write_pos > 0);
    }
}
