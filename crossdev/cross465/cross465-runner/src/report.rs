//! Reporting structures for the Cross465 RTST runner.
//!
//! Provides the `CaseReport`/`RunReport` data returned by `run_cases` as well as
//! the serde-friendly enums so callers can emit JSON summaries.

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Per-case status emitted by the runner reports.
pub enum CaseStatus {
    Pending,
    Passed,
    Failed,
}

impl CaseStatus {
    pub fn is_failed(&self) -> bool {
        matches!(self, CaseStatus::Failed)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
/// Host-observable values produced by RTST (used for JSON output).
pub enum ActualValue {
    KeyValue {
        value: u32,
    },
    Hash {
        hash: u32,
    },
    Memory {
        bytes: Vec<u8>,
    },
    Regs {
        a: u8,
        x: u8,
        y: u8,
        sp: u8,
        status: u8,
        pc: u16,
    },
    Time {
        cycles: u32,
    },
}

#[derive(Clone, Debug, Serialize)]
/// Summary of a single RTST case (status + logs + actuals).
pub struct CaseReport {
    pub name: String,
    pub status: CaseStatus,
    pub status_code: Option<u8>,
    pub message: Option<String>,
    pub logs: Vec<String>,
    pub asserts: Vec<String>,
    pub actuals: BTreeMap<String, ActualValue>,
}

#[derive(Clone, Debug, Serialize)]
/// Aggregate pass/fail counts for a run.
pub struct RunSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, Serialize)]
/// Full report returned by `run_cases`.
pub struct RunReport {
    pub cases: Vec<CaseReport>,
    pub summary: RunSummary,
}

impl RunReport {
    /// Build a report + summary from the provided cases.
    pub fn from_cases(cases: Vec<CaseReport>) -> Self {
        let passed = cases
            .iter()
            .filter(|c| matches!(c.status, CaseStatus::Passed))
            .count();
        let failed = cases.iter().filter(|c| c.status.is_failed()).count();
        let summary = RunSummary {
            total: cases.len(),
            passed,
            failed,
        };
        Self { cases, summary }
    }
}
