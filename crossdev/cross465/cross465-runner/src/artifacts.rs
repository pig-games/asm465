use crate::report::{CaseMetrics, CaseReport, CaseStatus};
use crate::{ExecutionOutput, RunnerError, TargetKind};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Handles persistence of PRG/RTST dumps for debugging failed runs.
pub struct ArtifactStore {
    root: Option<PathBuf>,
    target: TargetKind,
    keep_success: bool,
}

impl ArtifactStore {
    pub fn new(root: Option<PathBuf>, target: TargetKind, keep_success: bool) -> Self {
        Self {
            root,
            target,
            keep_success,
        }
    }

    /// Capture artifacts for backend-level failures (no RTST output).
    pub fn capture_backend_error(
        &self,
        case_name: &str,
        personality: &str,
        prg: &[u8],
        err: &RunnerError,
    ) {
        if self.root.is_none() {
            return;
        }
        let meta = ArtifactMeta::new("backend", case_name, self.target, personality, "error")
            .with_error(err.to_string());
        self.write_artifacts(case_name, prg, None, meta);
    }

    /// Capture artifacts when RTST parsing fails.
    pub fn capture_parse_error(
        &self,
        case_name: &str,
        personality: &str,
        prg: &[u8],
        exec: &ExecutionOutput,
        err: &RunnerError,
    ) {
        if self.root.is_none() {
            return;
        }
        let mut meta = ArtifactMeta::new("rtst", case_name, self.target, personality, "error")
            .with_error(err.to_string());
        meta.cycles = Some(exec.cycles);
        meta.rtst_bytes = Some(exec.rtst_region.len());
        self.write_artifacts(case_name, prg, Some(&exec.rtst_region), meta);
    }

    /// Capture artifacts after a case finished (optionally only for failures).
    pub fn capture_case(
        &self,
        report: &CaseReport,
        personality: &str,
        prg: &[u8],
        exec: &ExecutionOutput,
    ) {
        if self.root.is_none() {
            return;
        }
        if matches!(report.status, CaseStatus::Passed) && !self.keep_success {
            return;
        }
        let mut meta = ArtifactMeta::new(
            "case",
            &report.name,
            self.target,
            personality,
            report.status.as_str(),
        )
        .with_metrics(report.metrics.as_ref());
        if report.status.is_failed() {
            if let Some(message) = build_failure_message(report) {
                meta = meta.with_error(message);
            }
        }
        self.write_artifacts(&report.name, prg, Some(&exec.rtst_region), meta);
    }

    fn write_artifacts(
        &self,
        case_name: &str,
        prg: &[u8],
        rtst: Option<&[u8]>,
        meta: ArtifactMeta,
    ) {
        let Some(dir) = self.case_dir(case_name) else {
            return;
        };
        if let Err(err) = fs::create_dir_all(&dir) {
            warn_io("create artifact dir", dir.as_path(), err);
            return;
        }
        if let Err(err) = fs::write(dir.join("program.prg"), prg) {
            warn_io("write program.prg", dir.join("program.prg").as_path(), err);
        }
        if let Some(buf) = rtst {
            if let Err(err) = fs::write(dir.join("rtst.bin"), buf) {
                warn_io("write rtst.bin", dir.join("rtst.bin").as_path(), err);
            }
        }
        match serde_json::to_vec_pretty(&meta) {
            Ok(payload) => {
                if let Err(err) = fs::write(dir.join("meta.json"), payload) {
                    warn_io("write meta.json", dir.join("meta.json").as_path(), err);
                }
            }
            Err(err) => {
                eprintln!("cross465-runner: failed to serialize artifact metadata: {err}");
            }
        }
    }

    fn case_dir(&self, case_name: &str) -> Option<PathBuf> {
        self.root.as_ref().map(|root| {
            let mut dir = root.join(self.target.to_string());
            for part in case_name.split("::") {
                dir = dir.join(part);
            }
            dir
        })
    }
}

fn warn_io(action: &str, path: &Path, err: std::io::Error) {
    eprintln!(
        "cross465-runner: failed to {action} at {}: {err}",
        path.display()
    );
}

fn build_failure_message(report: &CaseReport) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(msg) = &report.message {
        if !msg.is_empty() {
            parts.push(msg.clone());
        }
    }
    if !report.asserts.is_empty() {
        parts.push(report.asserts.join(" | "));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

#[derive(Serialize)]
struct ArtifactMeta {
    stage: String,
    case: String,
    target: String,
    personality: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cycles: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rtst_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    write_pos: Option<u16>,
    timestamp_ms: u128,
}

impl ArtifactMeta {
    fn new(stage: &str, case: &str, target: TargetKind, personality: &str, status: &str) -> Self {
        Self {
            stage: stage.to_string(),
            case: case.to_string(),
            target: target.to_string().to_owned(),
            personality: personality.to_string(),
            status: status.to_string(),
            error: None,
            cycles: None,
            rtst_bytes: None,
            write_pos: None,
            timestamp_ms: now_ms(),
        }
    }

    fn with_error(mut self, message: String) -> Self {
        self.error = Some(message);
        self
    }

    fn with_metrics(mut self, metrics: Option<&CaseMetrics>) -> Self {
        if let Some(metrics) = metrics {
            self.cycles = Some(metrics.cycles);
            self.rtst_bytes = Some(metrics.rtst_bytes);
            self.write_pos = Some(metrics.write_pos);
        }
        self
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
