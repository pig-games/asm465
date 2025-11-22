//! Reporting structures for the Cross465 RTST runner.
//!
//! Provides the `CaseReport`/`RunReport` data returned by `run_cases` as well as
//! the serde-friendly enums so callers can emit JSON summaries.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

    pub fn as_str(&self) -> &'static str {
        match self {
            CaseStatus::Pending => "pending",
            CaseStatus::Passed => "passed",
            CaseStatus::Failed => "failed",
        }
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
    pub actual_groups: ActualCollections,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<CaseDebug>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<CaseMetrics>,
}

#[derive(Clone, Debug, Serialize)]
/// Runtime metrics captured for a case (cycles + RTST footprint).
pub struct CaseMetrics {
    pub cycles: u64,
    pub rtst_bytes: usize,
    pub write_pos: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Bucketed view of ACT_* payloads keyed by record type.
pub struct ActualCollections {
    pub scalars: BTreeMap<String, u32>,
    pub hashes: BTreeMap<String, u32>,
    #[serde(rename = "memory")]
    pub memory: BTreeMap<String, Vec<u8>>,
    pub registers: BTreeMap<String, Registers>,
    pub timings: BTreeMap<String, u32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub pc: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
/// Optional MMIO debug captures recorded alongside RTST.
pub struct CaseDebug {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console_log: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<CaseDebugDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<CaseDebugOverlay>,
}

impl CaseDebug {
    pub fn is_empty(&self) -> bool {
        self.console_log.is_none() && self.display.is_none() && self.overlay.is_none()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// Snapshot of display palette registers.
pub struct CaseDebugDisplay {
    pub border_color: u8,
    pub background_color: u8,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// Snapshot of overlay/raster instrumentation when available.
pub struct CaseDebugOverlay {
    pub raster: u16,
    pub sprite_collisions: u8,
    pub background_collisions: u8,
}

impl ActualCollections {
    pub fn insert(&mut self, key: String, value: &ActualValue) {
        match value {
            ActualValue::KeyValue { value } => {
                self.scalars.insert(key, *value);
            }
            ActualValue::Hash { hash } => {
                self.hashes.insert(key, *hash);
            }
            ActualValue::Memory { bytes } => {
                self.memory.insert(key, bytes.clone());
            }
            ActualValue::Regs {
                a,
                x,
                y,
                sp,
                status,
                pc,
            } => {
                self.registers.insert(
                    key,
                    Registers {
                        a: *a,
                        x: *x,
                        y: *y,
                        sp: *sp,
                        status: *status,
                        pc: *pc,
                    },
                );
            }
            ActualValue::Time { cycles } => {
                self.timings.insert(key, *cycles);
            }
        }
    }
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
