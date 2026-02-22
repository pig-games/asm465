//! Service API types used by opFoundry's JSON protocol.
//!
//! These types model the external service API shared between the native TCP
//! listener, the WebSocket bridge, and the opfoundry-server crate.

use std::path::PathBuf;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

/// Source for a PRG payload that should be executed by the emulator.
#[derive(Debug, Clone)]
pub enum ProgramSource {
    File(PathBuf),
    Inline { name: Option<String>, data: Vec<u8> },
}

impl ProgramSource {
    pub(crate) fn load_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            #[cfg(any(feature = "native-file-dialog", feature = "native-service"))]
            ProgramSource::File(path) => std::fs::read(path)
                .map_err(|err| format!("Failed to read {}: {err}", path.display())),
            #[cfg(not(any(feature = "native-file-dialog", feature = "native-service")))]
            ProgramSource::File(_) => Err("File sources are not supported on this platform".into()),
            ProgramSource::Inline { data, .. } => Ok(data.clone()),
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            ProgramSource::File(path) => path.display().to_string(),
            ProgramSource::Inline { name, data } => name
                .clone()
                .unwrap_or_else(|| format!("inline program ({} bytes)", data.len())),
        }
    }
}

/// Configuration used when pre-loading a PRG before the app renders.
#[derive(Clone)]
pub struct StartupConfig {
    pub source: ProgramSource,
    pub max_cycles: u64,
    pub start: Option<u16>,
    pub rtst: Option<RtstMonitorConfig>,
    pub progress_timeout_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub struct RtstMonitorConfig {
    pub base: u32,
    pub span: u32,
}

/// Details about a bounded CPU run triggered by the host.
pub(crate) struct ProgramRunReport {
    pub outcome: Option<core6502::RunOutcome>,
    pub message: String,
}

/// Command variants exchanged with the external service API.
#[derive(Debug)]
pub enum ServiceCommand {
    RunProgram {
        source: ProgramSource,
        max_cycles: Option<u64>,
        start: Option<u16>,
        rtst: Option<RtstMonitorConfig>,
        progress_timeout_ms: Option<u64>,
    },
    ReadMemory {
        address: u32,
        length: u32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceResponseMessage {
    pub status: ServiceStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycles: Option<u64>,
}

impl ServiceResponseMessage {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Ok,
            message: message.into(),
            data: None,
            bridge_id: None,
            cycles: None,
        }
    }

    pub fn ok_with_data(message: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            status: ServiceStatus::Ok,
            message: message.into(),
            data: Some(BASE64_STANDARD.encode(bytes)),
            bridge_id: None,
            cycles: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Error,
            message: message.into(),
            data: None,
            bridge_id: None,
            cycles: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Ok,
    Error,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ServiceRequestPayload {
    RunPrg {
        path: String,
        #[serde(default)]
        max_cycles: Option<u64>,
        #[serde(default)]
        start: Option<u16>,
        #[serde(default)]
        rtst_base: Option<u32>,
        #[serde(default)]
        rtst_span: Option<u32>,
        #[serde(default)]
        progress_timeout_ms: Option<u64>,
    },
    RunPrgData {
        data: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        max_cycles: Option<u64>,
        #[serde(default)]
        start: Option<u16>,
        #[serde(default)]
        rtst_base: Option<u32>,
        #[serde(default)]
        rtst_span: Option<u32>,
        #[serde(default)]
        progress_timeout_ms: Option<u64>,
    },
    ReadMem {
        address: u32,
        length: u32,
    },
}

impl ServiceRequestPayload {
    pub fn into_command(self) -> Result<ServiceCommand, String> {
        match self {
            ServiceRequestPayload::RunPrg {
                path,
                max_cycles,
                start,
                rtst_base,
                rtst_span,
                progress_timeout_ms,
            } => {
                if path.is_empty() {
                    return Err("run_prg requires a non-empty path".into());
                }
                let rtst = parse_rtst_config(rtst_base, rtst_span)?;
                Ok(ServiceCommand::RunProgram {
                    source: ProgramSource::File(PathBuf::from(path)),
                    max_cycles,
                    start,
                    rtst,
                    progress_timeout_ms,
                })
            }
            ServiceRequestPayload::RunPrgData {
                data,
                name,
                max_cycles,
                start,
                rtst_base,
                rtst_span,
                progress_timeout_ms,
            } => {
                if data.trim().is_empty() {
                    return Err("run_prg_data requires a non-empty base64 payload".into());
                }
                let decoded = BASE64_STANDARD
                    .decode(data.as_bytes())
                    .map_err(|err| format!("invalid base64 payload for run_prg_data: {err}"))?;
                let rtst = parse_rtst_config(rtst_base, rtst_span)?;
                Ok(ServiceCommand::RunProgram {
                    source: ProgramSource::Inline {
                        name,
                        data: decoded,
                    },
                    max_cycles,
                    start,
                    rtst,
                    progress_timeout_ms,
                })
            }
            ServiceRequestPayload::ReadMem { address, length } => {
                if length == 0 || length > 0x10000 {
                    return Err("read_mem length must be between 1 and 65536 bytes".into());
                }
                Ok(ServiceCommand::ReadMemory { address, length })
            }
        }
    }
}

fn parse_rtst_config(
    base: Option<u32>,
    span: Option<u32>,
) -> Result<Option<RtstMonitorConfig>, String> {
    match (base, span) {
        (Some(base), Some(span)) => {
            if span == 0 {
                return Err("rtst_span must be greater than zero".into());
            }
            if base >= 0x1_0000 || base + span > 0x1_0000 {
                return Err("rtst_base/span must be within the 64 KB address space".into());
            }
            Ok(Some(RtstMonitorConfig { base, span }))
        }
        (None, None) => Ok(None),
        _ => Err("rtst_base and rtst_span must be provided together".into()),
    }
}
