use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct RtstMonitorConfig {
    pub base: u32,
    pub span: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Ok,
    Error,
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
    pub fn validate(&self) -> Result<(), String> {
        match self {
            ServiceRequestPayload::RunPrg {
                path,
                rtst_base,
                rtst_span,
                ..
            } => {
                if path.is_empty() {
                    return Err("run_prg requires a non-empty path".into());
                }
                validate_rtst(*rtst_base, *rtst_span)
            }
            ServiceRequestPayload::RunPrgData {
                data,
                rtst_base,
                rtst_span,
                ..
            } => {
                if data.trim().is_empty() {
                    return Err("run_prg_data requires a non-empty base64 payload".into());
                }
                validate_rtst(*rtst_base, *rtst_span)
            }
            ServiceRequestPayload::ReadMem { length, .. } => {
                if *length == 0 || *length > 0x10000 {
                    return Err("read_mem length must be between 1 and 65536 bytes".into());
                }
                Ok(())
            }
        }
    }
}

pub fn parse_rtst_config(
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

fn validate_rtst(base: Option<u32>, span: Option<u32>) -> Result<(), String> {
    parse_rtst_config(base, span).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rtst_accepts_valid_range() {
        let parsed = parse_rtst_config(Some(0x200), Some(0x40)).expect("valid range should pass");
        assert_eq!(parsed.map(|cfg| (cfg.base, cfg.span)), Some((0x200, 0x40)));
    }

    #[test]
    fn parse_rtst_rejects_partial_config() {
        let err = parse_rtst_config(Some(0x200), None).expect_err("partial config should fail");
        assert!(err.contains("provided together"));
    }

    #[test]
    fn read_mem_validation_rejects_zero_length() {
        let payload = ServiceRequestPayload::ReadMem {
            address: 0,
            length: 0,
        };
        let err = payload.validate().expect_err("zero length should fail");
        assert!(err.contains("between 1 and 65536"));
    }
}
