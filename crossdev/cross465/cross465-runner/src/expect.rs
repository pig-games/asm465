//! Helper APIs for inspecting and asserting RTST actual values.
//!
//! Provides ergonomic wrappers to compare ACT_* payloads emitted by 6502 tests
//! during Cargo-based Host-Expect verification.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use crate::report::{ActualValue, CaseReport, CaseStatus};

/// Result type returned by expectation helpers.
pub type ExpectResult<T> = Result<T, ExpectError>;

/// Result type returned by status assertions.
pub type AssertResult<T> = Result<T, AssertError>;

/// Accessor for host-side helper APIs.
pub trait CaseAccessor {
    fn case_report(&self) -> &CaseReport;
}

impl CaseAccessor for CaseReport {
    fn case_report(&self) -> &CaseReport {
        self
    }
}

impl<'a> CaseAccessor for &'a CaseReport {
    fn case_report(&self) -> &CaseReport {
        self
    }
}

/// Detailed expectation failures surfaced when comparing host-side actuals.
#[derive(Debug, thiserror::Error)]
pub enum ExpectError {
    #[error("case '{case}' missing actual '{key}'")]
    Missing { case: String, key: String },
    #[error("case '{case}' actual '{key}' has kind {actual_kind}, expected {expected_kind}")]
    WrongKind {
        case: String,
        key: String,
        expected_kind: &'static str,
        actual_kind: &'static str,
    },
    #[error("case '{case}' actual '{key}' value {actual:#010x} != expected {expected:#010x}")]
    ValueMismatch {
        case: String,
        key: String,
        expected: u32,
        actual: u32,
    },
    #[error("case '{case}' actual '{key}' value {actual:#010x} not in range {range:?}")]
    RangeMismatch {
        case: String,
        key: String,
        range: RangeInclusive<u32>,
        actual: u32,
    },
    #[error("case '{case}' actual '{key}' hash {actual:#010x} != expected {expected:#010x}")]
    HashMismatch {
        case: String,
        key: String,
        expected: u32,
        actual: u32,
    },
    #[error("case '{case}' actual '{key}' length {actual_len} != expected {expected_len}")]
    LengthMismatch {
        case: String,
        key: String,
        expected_len: usize,
        actual_len: usize,
    },
    #[error(
        "case '{case}' actual '{key}' first diff at offset {offset:#06x}: expected {expected:#04x}, got {actual:#04x}"
    )]
    MemoryDiff {
        case: String,
        key: String,
        offset: usize,
        expected: u8,
        actual: u8,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AssertError {
    #[error("expected case '{expected}', got '{actual}'")]
    NameMismatch { expected: String, actual: String },
    #[error("case '{name}' status {status:?} (message={message:?})")]
    UnexpectedStatus {
        name: String,
        status: CaseStatus,
        message: Option<String>,
    },
}

/// Convenience view over a case's actual values.
pub struct CaseActuals<'a> {
    case: &'a str,
    actuals: &'a BTreeMap<String, ActualValue>,
}

impl<'a> CaseActuals<'a> {
    /// Create a new view from a case name and its actual map.
    pub fn new(case: &'a str, actuals: &'a BTreeMap<String, ActualValue>) -> Self {
        Self { case, actuals }
    }

    /// Retrieve the raw actual value associated with `key`.
    fn get(&self, key: &str) -> ExpectResult<&'a ActualValue> {
        self.actuals.get(key).ok_or_else(|| ExpectError::Missing {
            case: self.case.to_string(),
            key: key.to_string(),
        })
    }

    /// Expect a `KeyValue` record equal to `expected`.
    pub fn expect_eq(&self, key: &str, expected: u32) -> ExpectResult<()> {
        let actual = self.get_u32(key)?;
        if actual == expected {
            Ok(())
        } else {
            Err(ExpectError::ValueMismatch {
                case: self.case.to_string(),
                key: key.to_string(),
                expected,
                actual,
            })
        }
    }

    /// Expect a `KeyValue` record within the provided inclusive range.
    pub fn expect_in(&self, key: &str, range: RangeInclusive<u32>) -> ExpectResult<()> {
        let actual = self.get_u32(key)?;
        if range.contains(&actual) {
            Ok(())
        } else {
            Err(ExpectError::RangeMismatch {
                case: self.case.to_string(),
                key: key.to_string(),
                range,
                actual,
            })
        }
    }

    /// Expect a `Hash` record equal to `expected`.
    pub fn expect_hash_eq(&self, key: &str, expected: u32) -> ExpectResult<()> {
        match self.get(key)? {
            ActualValue::Hash { hash } => {
                if *hash == expected {
                    Ok(())
                } else {
                    Err(ExpectError::HashMismatch {
                        case: self.case.to_string(),
                        key: key.to_string(),
                        expected,
                        actual: *hash,
                    })
                }
            }
            other => Err(ExpectError::WrongKind {
                case: self.case.to_string(),
                key: key.to_string(),
                expected_kind: "hash",
                actual_kind: kind_label(other),
            }),
        }
    }

    /// Expect a `Memory` record to match the provided bytes exactly.
    pub fn expect_mem_eq(&self, key: &str, expected: &[u8]) -> ExpectResult<()> {
        let actual = self.get_bytes(key)?;
        if actual.len() != expected.len() {
            return Err(ExpectError::LengthMismatch {
                case: self.case.to_string(),
                key: key.to_string(),
                expected_len: expected.len(),
                actual_len: actual.len(),
            });
        }
        for (idx, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
            if exp != act {
                return Err(ExpectError::MemoryDiff {
                    case: self.case.to_string(),
                    key: key.to_string(),
                    offset: idx,
                    expected: *exp,
                    actual: *act,
                });
            }
        }
        Ok(())
    }

    /// Return the numeric value stored under `key`.
    pub fn get_u32(&self, key: &str) -> ExpectResult<u32> {
        match self.get(key)? {
            ActualValue::KeyValue { value } => Ok(*value),
            other => Err(ExpectError::WrongKind {
                case: self.case.to_string(),
                key: key.to_string(),
                expected_kind: "key_value",
                actual_kind: kind_label(other),
            }),
        }
    }

    /// Return the memory dump stored under `key`.
    pub fn get_bytes(&self, key: &str) -> ExpectResult<&'a [u8]> {
        match self.get(key)? {
            ActualValue::Memory { bytes } => Ok(bytes),
            other => Err(ExpectError::WrongKind {
                case: self.case.to_string(),
                key: key.to_string(),
                expected_kind: "memory",
                actual_kind: kind_label(other),
            }),
        }
    }

    /// Return the register snapshot stored under `key`.
    pub fn get_regs(&self, key: &str) -> ExpectResult<RegsView> {
        match self.get(key)? {
            ActualValue::Regs {
                a,
                x,
                y,
                sp,
                status,
                pc,
            } => Ok(RegsView {
                a: *a,
                x: *x,
                y: *y,
                sp: *sp,
                status: *status,
                pc: *pc,
            }),
            other => Err(ExpectError::WrongKind {
                case: self.case.to_string(),
                key: key.to_string(),
                expected_kind: "regs",
                actual_kind: kind_label(other),
            }),
        }
    }

    /// Return the timing measurement stored under `key`.
    pub fn get_cycles(&self, key: &str) -> ExpectResult<u32> {
        match self.get(key)? {
            ActualValue::Time { cycles } => Ok(*cycles),
            other => Err(ExpectError::WrongKind {
                case: self.case.to_string(),
                key: key.to_string(),
                expected_kind: "time",
                actual_kind: kind_label(other),
            }),
        }
    }
}

impl CaseReport {
    /// Convenience accessor for host-expect helpers.
    pub fn actuals_view(&self) -> CaseActuals<'_> {
        CaseActuals::new(&self.name, &self.actuals)
    }
}

/// Assert that the case succeeded with the expected name.
pub fn assert_case_ok<T: CaseAccessor + ?Sized>(case: &T, expected_name: &str) -> AssertResult<()> {
    let report = case.case_report();
    if report.name != expected_name {
        return Err(AssertError::NameMismatch {
            expected: expected_name.to_string(),
            actual: report.name.clone(),
        });
    }
    if matches!(report.status, CaseStatus::Passed) {
        Ok(())
    } else {
        Err(AssertError::UnexpectedStatus {
            name: report.name.clone(),
            status: report.status.clone(),
            message: report.message.clone(),
        })
    }
}

/// Assert that the case failed with the expected name.
pub fn assert_case_failed<T: CaseAccessor + ?Sized>(
    case: &T,
    expected_name: &str,
) -> AssertResult<()> {
    let report = case.case_report();
    if report.name != expected_name {
        return Err(AssertError::NameMismatch {
            expected: expected_name.to_string(),
            actual: report.name.clone(),
        });
    }
    if matches!(report.status, CaseStatus::Failed) {
        Ok(())
    } else {
        Err(AssertError::UnexpectedStatus {
            name: report.name.clone(),
            status: report.status.clone(),
            message: report.message.clone(),
        })
    }
}

/// Expect a key/value actual to match.
pub fn expect_eq<T: CaseAccessor + ?Sized>(case: &T, key: &str, expected: u32) -> ExpectResult<()> {
    case.case_report().actuals_view().expect_eq(key, expected)
}

/// Expect a key/value actual to fall within the provided range.
pub fn expect_in<T: CaseAccessor + ?Sized>(
    case: &T,
    key: &str,
    range: RangeInclusive<u32>,
) -> ExpectResult<()> {
    case.case_report().actuals_view().expect_in(key, range)
}

/// Expect a hash actual to match the provided value.
pub fn expect_hash_eq<T: CaseAccessor + ?Sized>(
    case: &T,
    key: &str,
    expected: u32,
) -> ExpectResult<()> {
    case.case_report()
        .actuals_view()
        .expect_hash_eq(key, expected)
}

/// Expect a memory dump to match a provided slice.
pub fn expect_mem_eq<T: CaseAccessor + ?Sized>(
    case: &T,
    key: &str,
    expected: &[u8],
) -> ExpectResult<()> {
    case.case_report()
        .actuals_view()
        .expect_mem_eq(key, expected)
}

/// Lightweight register view used by expectation helpers.
#[derive(Clone, Copy, Debug)]
pub struct RegsView {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub pc: u16,
}

fn kind_label(value: &ActualValue) -> &'static str {
    match value {
        ActualValue::KeyValue { .. } => "key_value",
        ActualValue::Hash { .. } => "hash",
        ActualValue::Memory { .. } => "memory",
        ActualValue::Regs { .. } => "regs",
        ActualValue::Time { .. } => "time",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{ActualCollections, ActualValue, CaseStatus};

    fn build_report() -> CaseReport {
        let mut actuals = BTreeMap::new();
        let mut groups = ActualCollections::default();

        let scalar = ActualValue::KeyValue { value: 0x1234 };
        groups.insert("score".into(), &scalar);
        actuals.insert("score".into(), scalar);

        let hash = ActualValue::Hash { hash: 0xDEADBEEF };
        groups.insert("fb_hash".into(), &hash);
        actuals.insert("fb_hash".into(), hash);

        let mem = ActualValue::Memory {
            bytes: vec![0x00, 0x01, 0xFF],
        };
        groups.insert("dump".into(), &mem);
        actuals.insert("dump".into(), mem);

        let regs = ActualValue::Regs {
            a: 1,
            x: 2,
            y: 3,
            sp: 4,
            status: 0x24,
            pc: 0x1234,
        };
        groups.insert("regs".into(), &regs);
        actuals.insert("regs".into(), regs);

        let time = ActualValue::Time { cycles: 42 };
        groups.insert("elapsed".into(), &time);
        actuals.insert("elapsed".into(), time);

        CaseReport {
            name: "demo".into(),
            status: CaseStatus::Passed,
            status_code: None,
            message: None,
            logs: Vec::new(),
            asserts: Vec::new(),
            actuals,
            actual_groups: groups,
        }
    }

    #[test]
    fn expect_eq_passes() {
        let report = build_report();
        report.actuals_view().expect_eq("score", 0x1234).unwrap();
    }

    #[test]
    fn expect_mem_diff_surfaces_offset() {
        let report = build_report();
        let err = report
            .actuals_view()
            .expect_mem_eq("dump", &[0x00, 0x02, 0xFF])
            .unwrap_err();
        match err {
            ExpectError::MemoryDiff { offset, .. } => assert_eq!(offset, 1),
            _ => panic!("unexpected error {err:?}"),
        }
    }

    #[test]
    fn expect_in_rejects_outside_range() {
        let report = build_report();
        let err = report
            .actuals_view()
            .expect_in("score", 0x2000..=0x2FFF)
            .unwrap_err();
        matches!(err, ExpectError::RangeMismatch { .. });
    }

    #[test]
    fn assert_case_ok_helper_succeeds() {
        let report = build_report();
        assert_case_ok(&report, "demo").unwrap();
    }

    #[test]
    fn assert_case_ok_helper_detects_status() {
        let mut report = build_report();
        report.status = CaseStatus::Failed;
        let err = assert_case_ok(&report, "demo").unwrap_err();
        matches!(err, AssertError::UnexpectedStatus { .. });
    }

    #[test]
    fn assert_case_failed_helper_succeeds() {
        let mut report = build_report();
        report.status = CaseStatus::Failed;
        assert_case_failed(&report, "demo").unwrap();
    }

    #[test]
    fn expect_eq_helper_works() {
        let report = build_report();
        expect_eq(&report, "score", 0x1234).unwrap();
    }
}
