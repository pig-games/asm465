use crate::report::{ActualCollections, Registers};
use crate::{CaseReport, CaseStatus, RunnerError, TargetKind};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

pub struct FixtureStore {
    target_dir: Option<PathBuf>,
    update: bool,
}

impl FixtureStore {
    pub fn new(
        _workspace_root: &Path,
        target: TargetKind,
        override_dir: Option<PathBuf>,
        update: bool,
    ) -> Self {
        let target_dir = override_dir.map(|root| root.join(target.to_string()));
        Self { target_dir, update }
    }

    pub fn apply(&self, case: &mut CaseReport) -> Result<(), RunnerError> {
        if self.target_dir.is_none() {
            return Ok(());
        }
        let fixture_path = self.path_for_case(&case.name);
        if self.update {
            self.write_fixture(&fixture_path, &case.actual_groups)?;
            return Ok(());
        }
        if !fixture_path.exists() {
            record_failure(
                case,
                format!("fixture missing at {}", fixture_path.display()),
            );
            return Ok(());
        }
        let expected = self.read_fixture(&fixture_path)?;
        compare_groups(case, &expected);
        Ok(())
    }

    fn path_for_case(&self, case_name: &str) -> PathBuf {
        let rel = case_name.replace("::", "/");
        self.target_dir
            .as_ref()
            .unwrap()
            .join(rel)
            .with_extension("json")
    }

    fn write_fixture(&self, path: &Path, groups: &ActualCollections) -> Result<(), RunnerError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| RunnerError::FixtureIo {
                path: parent.to_path_buf(),
                message: format!("failed to create fixture directory: {e}"),
            })?;
        }
        let data = serde_json::to_vec_pretty(groups).map_err(|e| RunnerError::FixtureIo {
            path: path.to_path_buf(),
            message: format!("failed to serialize fixture: {e}"),
        })?;
        fs::write(path, data).map_err(|e| RunnerError::FixtureIo {
            path: path.to_path_buf(),
            message: format!("failed to write fixture: {e}"),
        })
    }

    fn read_fixture(&self, path: &Path) -> Result<ActualCollections, RunnerError> {
        let data = fs::read(path).map_err(|e| RunnerError::FixtureIo {
            path: path.to_path_buf(),
            message: format!("failed to read fixture: {e}"),
        })?;
        serde_json::from_slice(&data).map_err(|e| RunnerError::FixtureIo {
            path: path.to_path_buf(),
            message: format!("failed to parse fixture: {e}"),
        })
    }
}

fn compare_groups(case: &mut CaseReport, expected: &ActualCollections) {
    let actual_scalars = case.actual_groups.scalars.clone();
    let actual_hashes = case.actual_groups.hashes.clone();
    let actual_timings = case.actual_groups.timings.clone();
    let actual_memory = case.actual_groups.memory.clone();
    let actual_regs = case.actual_groups.registers.clone();
    compare_scalar_maps(case, "scalar", &expected.scalars, &actual_scalars, |v| {
        format!("{v:#010x}")
    });
    compare_scalar_maps(case, "hash", &expected.hashes, &actual_hashes, |v| {
        format!("{v:#010x}")
    });
    compare_scalar_maps(case, "timing", &expected.timings, &actual_timings, |v| {
        format!("{v} cycles")
    });
    compare_memory(case, &expected.memory, &actual_memory);
    compare_registers(case, &expected.registers, &actual_regs);
}

fn compare_scalar_maps<F: Fn(u32) -> String>(
    case: &mut CaseReport,
    label: &str,
    expected: &BTreeMap<String, u32>,
    actual: &BTreeMap<String, u32>,
    format_value: F,
) {
    for (key, exp) in expected {
        match actual.get(key) {
            Some(act) if act == exp => {}
            Some(act) => record_failure(
                case,
                format!(
                    "fixture mismatch [{label}] '{key}': expected {}, got {}",
                    format_value(*exp),
                    format_value(*act)
                ),
            ),
            None => record_failure(
                case,
                format!("fixture mismatch [{label}] '{key}' missing from actuals"),
            ),
        }
    }
    for key in actual.keys() {
        if !expected.contains_key(key) {
            record_failure(
                case,
                format!(
                    "fixture mismatch [{label}] unexpected key '{}' (actual has {})",
                    key,
                    format_value(*actual.get(key).unwrap())
                ),
            );
        }
    }
}

fn compare_memory(
    case: &mut CaseReport,
    expected: &BTreeMap<String, Vec<u8>>,
    actual: &BTreeMap<String, Vec<u8>>,
) {
    for (key, exp) in expected {
        match actual.get(key) {
            Some(act) if act == exp => {}
            Some(act) => {
                if act.len() != exp.len() {
                    record_failure(
                        case,
                        format!(
                            "fixture mismatch [memory] '{key}': expected {} bytes, got {}",
                            exp.len(),
                            act.len()
                        ),
                    );
                    continue;
                }
                let diff = act
                    .iter()
                    .zip(exp.iter())
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                record_failure(
                    case,
                    format!(
                        "fixture mismatch [memory] '{key}' at offset {diff:#06x}: expected {:#04x}, got {:#04x}",
                        exp[diff],
                        act[diff]
                    ),
                );
            }
            None => record_failure(
                case,
                format!("fixture mismatch [memory] '{key}' missing from actuals"),
            ),
        }
    }
    for key in actual.keys() {
        if !expected.contains_key(key) {
            record_failure(
                case,
                format!("fixture mismatch [memory] unexpected key '{key}'"),
            );
        }
    }
}

fn compare_registers(
    case: &mut CaseReport,
    expected: &BTreeMap<String, Registers>,
    actual: &BTreeMap<String, Registers>,
) {
    for (key, exp) in expected {
        match actual.get(key) {
            Some(act) if act == exp => {}
            Some(act) => {
                let mut msg = String::new();
                let _ = write!(msg, "fixture mismatch [regs] '{key}':");
                if act.a != exp.a {
                    let _ = write!(msg, " A {:#04x}->{:#04x}", exp.a, act.a);
                }
                if act.x != exp.x {
                    let _ = write!(msg, " X {:#04x}->{:#04x}", exp.x, act.x);
                }
                if act.y != exp.y {
                    let _ = write!(msg, " Y {:#04x}->{:#04x}", exp.y, act.y);
                }
                if act.sp != exp.sp {
                    let _ = write!(msg, " SP {:#04x}->{:#04x}", exp.sp, act.sp);
                }
                if act.status != exp.status {
                    let _ = write!(msg, " STATUS {:#04x}->{:#04x}", exp.status, act.status);
                }
                if act.pc != exp.pc {
                    let _ = write!(msg, " PC {:#06x}->{:#06x}", exp.pc, act.pc);
                }
                record_failure(case, msg);
            }
            None => record_failure(
                case,
                format!("fixture mismatch [regs] '{key}' missing from actuals"),
            ),
        }
    }
    for key in actual.keys() {
        if !expected.contains_key(key) {
            record_failure(
                case,
                format!("fixture mismatch [regs] unexpected key '{key}'"),
            );
        }
    }
}

fn record_failure(case: &mut CaseReport, message: String) {
    if !case.status.is_failed() {
        case.status = CaseStatus::Failed;
    }
    case.asserts.push(message);
}
