//! Assembly front-end for the Cross465 RTST runner.
//!
//! This module knows how to map catalog entries to 64tass invocations, wiring up
//! the include paths needed for shared headers (`test_rtst.h`, platform maps,
//! etc.) and returning the compiled PRG bytes ready for execution.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::NamedTempFile;

use crate::catalog::{CaseSource, CatalogCase};
use crate::{RunnerError, TargetKind};

/// Configuration for invoking 64tass when assembling a case.
pub struct AssemblerConfig {
    pub workspace_root: PathBuf,
    pub tass_path: PathBuf,
    pub include_paths: Vec<PathBuf>,
    pub target: TargetKind,
}

/// Resulting PRG bytes from assembling a case.
pub struct AssemblyOutput {
    pub prg: Vec<u8>,
}

/// Default include paths (shared + platform-specific) for the given target.
pub fn default_include_paths(root: &Path, target: TargetKind) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let common = root.join("native/src/include");
    if common.exists() {
        paths.push(common);
    }
    let alias = match target {
        TargetKind::Cross465 => "cross465",
        TargetKind::Ultimate64 => "ultimate64",
        TargetKind::Mega65 => "mega65",
    };
    let platform = root.join(format!("native/src/platform/{alias}/include"));
    if platform.exists() {
        paths.push(platform);
    }
    paths
}

/// Assemble a catalog case into a PRG using the provided config.
pub fn assemble_case(
    case: &CatalogCase,
    cfg: &AssemblerConfig,
) -> Result<AssemblyOutput, RunnerError> {
    let mut temp_source: Option<NamedTempFile> = None;
    let source_path = match case.source() {
        CaseSource::File(path) => path.clone(),
        CaseSource::Inline(src) => {
            let mut file = NamedTempFile::new_in(&cfg.workspace_root).map_err(|e| {
                RunnerError::AssemblerFailed {
                    message: format!("unable to create temp file: {e}"),
                }
            })?;
            file.write_all(src.as_bytes())
                .map_err(|e| RunnerError::AssemblerFailed {
                    message: format!("failed to write inline assembly: {e}"),
                })?;
            let path = file.path().to_path_buf();
            temp_source = Some(file);
            path
        }
    };

    if !source_path.exists() {
        return Err(RunnerError::AssemblyMissing { path: source_path });
    }

    let temp_output =
        NamedTempFile::new_in(&cfg.workspace_root).map_err(|e| RunnerError::AssemblerFailed {
            message: format!("unable to create temp output: {e}"),
        })?;
    let output_path = temp_output.path().to_path_buf();

    let mut cmd = Command::new(&cfg.tass_path);
    cmd.current_dir(&cfg.workspace_root);
    cmd.arg("-q").arg("-C").arg("-a").arg("-B");
    cmd.arg("-D")
        .arg(format!("TARGET_{}:=1", cfg.target.as_define_suffix()));
    for include in &cfg.include_paths {
        cmd.arg("-I").arg(include);
    }
    cmd.arg(&source_path);
    cmd.arg("-o").arg(&output_path);

    let output = cmd.output()?;
    if !output.status.success() {
        let mut message = String::from("64tass failed");
        if !output.stdout.is_empty() {
            message.push_str("\nstdout:\n");
            message.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            message.push_str("\nstderr:\n");
            message.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        return Err(RunnerError::AssemblerFailed { message });
    }

    let prg = fs::read(&output_path).map_err(|e| RunnerError::AssemblerFailed {
        message: format!("failed to read assembler output: {e}"),
    })?;

    drop(temp_source);
    drop(temp_output);

    Ok(AssemblyOutput { prg })
}
