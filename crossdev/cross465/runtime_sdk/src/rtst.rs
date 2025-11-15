use std::convert::TryFrom;
use std::fmt;
use std::ops::Range;

/// ASCII marker stored at the beginning of the RTST header.
pub const MAGIC: [u8; 4] = *b"RTST";
/// Protocol version supported by this crate and the 6502 macros.
pub const VERSION: u8 = 0x01;
/// Size of the RTST header in bytes.
pub const HEADER_LEN: usize = 0x10;
/// Default RTST buffer span for C64-like targets (4 KiB).
pub const REGION_SIZE_C64: usize = 0x1000;
/// Default RTST buffer span for MEGA65 targets (8 KiB).
pub const REGION_SIZE_MEGA65: usize = 0x2000;
/// Default base address for Cross465, C64, and Ultimate64 targets.
pub const BASE_C64: u32 = 0x0000_C000;
/// Default base address for MEGA65 targets.
pub const BASE_MEGA65: u32 = 0x0004_0000;

/// Named RTST base layout used by different targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseAddress {
    /// Human-friendly identifier used in docs/logs.
    pub name: &'static str,
    /// Base address where the header lives.
    pub address: u32,
    /// Total bytes reserved for the RTST region (header + payload).
    pub span: usize,
}

impl BaseAddress {
    /// Construct a new base definition.
    pub const fn new(name: &'static str, address: u32, span: usize) -> Self {
        Self { name, address, span }
    }

    /// Range covering the 16-byte header.
    pub const fn header_range(&self) -> Range<u32> {
        self.address..self.address + HEADER_LEN as u32
    }

    /// Range covering the record payload area immediately after the header.
    pub const fn records_range(&self) -> Range<u32> {
        let start = self.address + HEADER_LEN as u32;
        start..self.address + self.span as u32
    }
}

/// Canonical layout used by Cross465, C64, and Ultimate64 targets.
pub const BASE_LAYOUT_C64: BaseAddress =
    BaseAddress::new("c64", BASE_C64, REGION_SIZE_C64);
/// Canonical layout for Cross465 emulator targets.
pub const BASE_LAYOUT_CROSS465: BaseAddress =
    BaseAddress::new("cross465", BASE_C64, REGION_SIZE_C64);
/// Canonical layout for Ultimate64 hardware targets.
pub const BASE_LAYOUT_ULTIMATE64: BaseAddress =
    BaseAddress::new("ultimate64", BASE_C64, REGION_SIZE_C64);
/// Canonical layout used by MEGA65 targets.
pub const BASE_LAYOUT_MEGA65: BaseAddress =
    BaseAddress::new("mega65", BASE_MEGA65, REGION_SIZE_MEGA65);

/// RTST execution state written into the header by 6502 code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    /// Test image has not started writing records yet.
    Pending = 0,
    /// Records are being emitted.
    Running = 1,
    /// Stream finished successfully (END record emitted).
    Done = 2,
    /// Execution aborted before emitting END.
    Aborted = 3,
}

impl State {
    /// Whether the state represents a terminal condition.
    pub const fn is_terminal(self) -> bool {
        matches!(self, State::Done | State::Aborted)
    }
}

impl TryFrom<u8> for State {
    type Error = RtstError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(State::Pending),
            1 => Ok(State::Running),
            2 => Ok(State::Done),
            3 => Ok(State::Aborted),
            _ => Err(RtstError::InvalidState { raw: value }),
        }
    }
}

impl From<State> for u8 {
    fn from(state: State) -> Self {
        state as u8
    }
}

/// Header view decoded from the RTST buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    version: u8,
    state: State,
    write_pos: u16,
    total_cases: u16,
    passed_cases: u16,
    failed_cases: u16,
}

impl Header {
    /// Construct a header with default values (pending state, zero counts).
    pub fn new() -> Self {
        Self {
            version: VERSION,
            state: State::Pending,
            write_pos: 0,
            total_cases: 0,
            passed_cases: 0,
            failed_cases: 0,
        }
    }

    /// Parse a header from the first 16 bytes of an RTST buffer.
    pub fn parse(bytes: &[u8]) -> Result<Self, RtstError> {
        if bytes.len() < HEADER_LEN {
            return Err(RtstError::HeaderTooShort { got: bytes.len() });
        }
        if bytes[0..4] != MAGIC {
            return Err(RtstError::BadMagic {
                found: bytes[0..4].try_into().unwrap(),
            });
        }
        let version = bytes[4];
        if version != VERSION {
            return Err(RtstError::UnsupportedVersion { found: version });
        }
        let state = State::try_from(bytes[5])?;
        let write_pos = u16::from_le_bytes([bytes[6], bytes[7]]);
        let total_cases = u16::from_le_bytes([bytes[8], bytes[9]]);
        let passed_cases = u16::from_le_bytes([bytes[10], bytes[11]]);
        let failed_cases = u16::from_le_bytes([bytes[12], bytes[13]]);
        Ok(Self {
            version,
            state,
            write_pos,
            total_cases,
            passed_cases,
            failed_cases,
        })
    }

    /// Encode the header back into the provided buffer.
    pub fn encode(&self, out: &mut [u8]) -> Result<(), RtstError> {
        if out.len() < HEADER_LEN {
            return Err(RtstError::HeaderTooShort { got: out.len() });
        }
        out[0..4].copy_from_slice(&MAGIC);
        out[4] = self.version;
        out[5] = self.state.into();
        out[6..8].copy_from_slice(&self.write_pos.to_le_bytes());
        out[8..10].copy_from_slice(&self.total_cases.to_le_bytes());
        out[10..12].copy_from_slice(&self.passed_cases.to_le_bytes());
        out[12..14].copy_from_slice(&self.failed_cases.to_le_bytes());
        out[14] = 0;
        out[15] = 0;
        Ok(())
    }

    /// State stored in the header.
    pub fn state(&self) -> State {
        self.state
    }

    /// Update the recorded state.
    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    /// Number of bytes written into the record area.
    pub fn write_pos(&self) -> u16 {
        self.write_pos
    }

    /// Update the write cursor (mirrors the `WPOS` field).
    pub fn set_write_pos(&mut self, pos: u16) {
        self.write_pos = pos;
    }

    /// Total cases announced via `TEST_CASE_BEGIN`.
    pub fn total_cases(&self) -> u16 {
        self.total_cases
    }

    /// Cases marked as passed.
    pub fn passed_cases(&self) -> u16 {
        self.passed_cases
    }

    /// Cases marked as failed.
    pub fn failed_cases(&self) -> u16 {
        self.failed_cases
    }

    /// Convenience: transition to the running state.
    pub fn mark_running(&mut self) {
        self.state = State::Running;
    }

    /// Convenience: mark the header as finished.
    pub fn mark_done(&mut self) {
        self.state = State::Done;
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

/// RTST record identifiers emitted by `test_rtst.inc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordId {
    CaseStart = 0x01,
    CaseOk = 0x02,
    CaseFail = 0x03,
    Assert = 0x04,
    Msg = 0x05,
    ActualKeyValue = 0x10,
    ActualMem = 0x11,
    ActualHash = 0x12,
    ActualRegs = 0x13,
    ActualTime = 0x14,
    End = 0xFF,
}

impl RecordId {
    /// Whether the record terminates the stream.
    pub const fn is_terminator(self) -> bool {
        matches!(self, RecordId::End)
    }
}

impl TryFrom<u8> for RecordId {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let kind = match value {
            0x01 => RecordId::CaseStart,
            0x02 => RecordId::CaseOk,
            0x03 => RecordId::CaseFail,
            0x04 => RecordId::Assert,
            0x05 => RecordId::Msg,
            0x10 => RecordId::ActualKeyValue,
            0x11 => RecordId::ActualMem,
            0x12 => RecordId::ActualHash,
            0x13 => RecordId::ActualRegs,
            0x14 => RecordId::ActualTime,
            0xFF => RecordId::End,
            _ => return Err(()),
        };
        Ok(kind)
    }
}

/// Errors produced while parsing or constructing RTST streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtstError {
    /// Header buffer is shorter than 16 bytes.
    HeaderTooShort { got: usize },
    /// The four-byte magic does not match `RTST`.
    BadMagic { found: [u8; 4] },
    /// Version mismatch between host and 6502 image.
    UnsupportedVersion { found: u8 },
    /// Unknown state value in the header.
    InvalidState { raw: u8 },
    /// Header advertised `WPOS` that exceeds the captured record buffer.
    WposOutOfBounds { wpos: u16, available: usize },
    /// Not enough bytes left to read the record prefix (id + len).
    RecordHeaderTooShort { offset: usize, remaining: usize },
    /// Payload length exceeds the captured record buffer.
    RecordOverruns { offset: usize, len: usize, remaining: usize },
    /// Payload shorter than required for a typed view.
    RecordPayloadTooShort { offset: usize, needed: usize, actual: usize },
    /// A C-style string did not contain a trailing `0x00` byte.
    MissingCStringTerminator { offset: usize },
    /// String payload was not valid UTF‑8.
    Utf8Error { offset: usize },
    /// Attempted to emit more than 64 KiB of records via the writer.
    RecordAreaOverflow { len: usize },
    /// Typed helper requested a record kind that did not match the payload.
    UnexpectedRecordKind {
        expected: RecordId,
        actual: Option<RecordId>,
        raw: u8,
    },
}

impl fmt::Display for RtstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RtstError::HeaderTooShort { got } => {
                write!(f, "header requires {HEADER_LEN} bytes (got {got})")
            }
            RtstError::BadMagic { found } => {
                write!(f, "unexpected magic bytes: {:02X?}", found)
            }
            RtstError::UnsupportedVersion { found } => {
                write!(f, "unsupported RTST version {found}")
            }
            RtstError::InvalidState { raw } => {
                write!(f, "invalid state value {raw}")
            }
            RtstError::WposOutOfBounds { wpos, available } => write!(
                f,
                "header advertises {wpos} bytes but only {available} were captured"
            ),
            RtstError::RecordHeaderTooShort { offset, remaining } => write!(
                f,
                "record at offset {offset} truncated (only {remaining} bytes left)"
            ),
            RtstError::RecordOverruns { offset, len, remaining } => write!(
                f,
                "record at offset {offset} (len {len}) exceeds remaining {remaining} bytes"
            ),
            RtstError::RecordPayloadTooShort { offset, needed, actual } => write!(
                f,
                "record payload at offset {offset} requires {needed} bytes (got {actual})"
            ),
            RtstError::MissingCStringTerminator { offset } => write!(
                f,
                "missing string terminator near offset {offset}"
            ),
            RtstError::Utf8Error { offset } => {
                write!(f, "string payload near offset {offset} is not UTF-8")
            }
            RtstError::RecordAreaOverflow { len } => {
                write!(f, "record payload exceeds 64 KiB limit (len={len})")
            }
            RtstError::UnexpectedRecordKind {
                expected,
                actual,
                raw,
            } => {
                if let Some(actual) = actual {
                    write!(f, "expected record {expected:?} but got {actual:?}")
                } else {
                    write!(f, "expected record {expected:?} but got raw id 0x{raw:02X}")
                }
            }
        }
    }
}

impl std::error::Error for RtstError {}
/// Low-level record returned by the parser. Typed helper methods decode the
/// payload into strongly typed structures.
#[derive(Debug, Clone, Copy)]
pub struct Record<'a> {
    raw_id: u8,
    payload: &'a [u8],
    payload_offset: usize,
}

impl<'a> Record<'a> {
    fn new(raw_id: u8, payload: &'a [u8], payload_offset: usize) -> Self {
        Self {
            raw_id,
            payload,
            payload_offset,
        }
    }

    /// Record identifier without enforcing that it is known.
    pub fn raw_id(&self) -> u8 {
        self.raw_id
    }

    /// Known record identifier, if the ID is recognized.
    pub fn kind(&self) -> Option<RecordId> {
        RecordId::try_from(self.raw_id).ok()
    }

    /// Underlying payload bytes.
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    /// Parse a CASE_START record payload.
    pub fn case_start(&self) -> Result<CaseStart<'a>, RtstError> {
        self.ensure_kind(RecordId::CaseStart)?;
        let (name, _, _) = split_cstring(self.payload, self.payload_offset)?;
        Ok(CaseStart { name })
    }

    /// Parse a CASE_OK record payload.
    pub fn case_ok(&self) -> Result<CaseOutcome<'a>, RtstError> {
        self.read_case_outcome(RecordId::CaseOk)
    }

    /// Parse a CASE_FAIL record payload.
    pub fn case_fail(&self) -> Result<CaseOutcome<'a>, RtstError> {
        self.read_case_outcome(RecordId::CaseFail)
    }

    /// Parse an ASSERT record payload.
    pub fn assert(&self) -> Result<AssertRecord<'a>, RtstError> {
        self.ensure_kind(RecordId::Assert)?;
        let (message, _, _) = split_cstring(self.payload, self.payload_offset)?;
        Ok(AssertRecord { message })
    }

    /// Parse a MSG record payload.
    pub fn message(&self) -> Result<MessageRecord<'a>, RtstError> {
        self.ensure_kind(RecordId::Msg)?;
        let (message, _, _) = split_cstring(self.payload, self.payload_offset)?;
        Ok(MessageRecord { message })
    }

    /// Parse an ACT_KV record payload.
    pub fn actual_kv(&self) -> Result<ActualKeyValue<'a>, RtstError> {
        self.ensure_kind(RecordId::ActualKeyValue)?;
        let (key, rest, rest_offset) =
            split_cstring(self.payload, self.payload_offset)?;
        require_payload(rest, 4, rest_offset)?;
        let value = u32::from_le_bytes(rest[..4].try_into().unwrap());
        Ok(ActualKeyValue { key, value })
    }

    /// Parse an ACT_TIME record payload.
    pub fn actual_time(&self) -> Result<ActualTime<'a>, RtstError> {
        self.ensure_kind(RecordId::ActualTime)?;
        let (key, rest, rest_offset) =
            split_cstring(self.payload, self.payload_offset)?;
        require_payload(rest, 4, rest_offset)?;
        let cycles = u32::from_le_bytes(rest[..4].try_into().unwrap());
        Ok(ActualTime { key, cycles })
    }

    /// Parse an ACT_HASH record payload.
    pub fn actual_hash(&self) -> Result<ActualHash<'a>, RtstError> {
        self.ensure_kind(RecordId::ActualHash)?;
        let (key, rest, rest_offset) =
            split_cstring(self.payload, self.payload_offset)?;
        require_payload(rest, 4, rest_offset)?;
        let hash = u32::from_le_bytes(rest[..4].try_into().unwrap());
        Ok(ActualHash { key, hash })
    }

    /// Parse an ACT_MEM record payload.
    pub fn actual_mem(&self) -> Result<ActualMem<'a>, RtstError> {
        self.ensure_kind(RecordId::ActualMem)?;
        let (key, rest, rest_offset) =
            split_cstring(self.payload, self.payload_offset)?;
        require_payload(rest, 2, rest_offset)?;
        let len = u16::from_le_bytes(rest[..2].try_into().unwrap()) as usize;
        let data = &rest[2..];
        let body = data
            .get(..len)
            .ok_or_else(|| RtstError::RecordOverruns {
                offset: rest_offset + 2,
                len,
                remaining: data.len(),
            })?;
        Ok(ActualMem { key, bytes: body })
    }

    /// Parse an ACT_REGS record payload.
    pub fn actual_regs(&self) -> Result<ActualRegs<'a>, RtstError> {
        self.ensure_kind(RecordId::ActualRegs)?;
        let (key, rest, rest_offset) =
            split_cstring(self.payload, self.payload_offset)?;
        require_payload(rest, 7, rest_offset)?;
        let regs = &rest[..7];
        Ok(ActualRegs {
            key,
            a: regs[0],
            x: regs[1],
            y: regs[2],
            sp: regs[3],
            status: regs[4],
            pc: u16::from_le_bytes([regs[5], regs[6]]),
        })
    }

    fn ensure_kind(&self, expected: RecordId) -> Result<(), RtstError> {
        match self.kind() {
            Some(kind) if kind == expected => Ok(()),
            other => Err(RtstError::UnexpectedRecordKind {
                expected,
                actual: other,
                raw: self.raw_id,
            }),
        }
    }

    fn read_case_outcome(&self, expected: RecordId) -> Result<CaseOutcome<'a>, RtstError> {
        self.ensure_kind(expected)?;
        require_payload(self.payload, 1, self.payload_offset)?;
        let status_code = self.payload[0];
        let message_bytes = &self.payload[1..];
        if message_bytes.is_empty() {
            return Ok(CaseOutcome {
                status_code,
                message: None,
            });
        }
        let (msg, _, _) = split_cstring(message_bytes, self.payload_offset + 1)?;
        Ok(CaseOutcome {
            status_code,
            message: (!msg.is_empty()).then_some(msg),
        })
    }
}

/// CASE_START payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseStart<'a> {
    pub name: &'a str,
}

/// CASE_OK / CASE_FAIL payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseOutcome<'a> {
    pub status_code: u8,
    pub message: Option<&'a str>,
}

/// ASSERT payload wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssertRecord<'a> {
    pub message: &'a str,
}

/// MSG payload wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageRecord<'a> {
    pub message: &'a str,
}

/// Host-Expect key/value payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualKeyValue<'a> {
    pub key: &'a str,
    pub value: u32,
}

/// Host-Expect memory payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualMem<'a> {
    pub key: &'a str,
    pub bytes: &'a [u8],
}

/// Host-Expect hash payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualHash<'a> {
    pub key: &'a str,
    pub hash: u32,
}

/// Host-Expect CPU register dump payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualRegs<'a> {
    pub key: &'a str,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub pc: u16,
}

/// Host-Expect time/timing payload (u32 cycles or microseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualTime<'a> {
    pub key: &'a str,
    pub cycles: u32,
}

/// Combined RTST view referencing the header and the captured record bytes.
#[derive(Debug, Clone, Copy)]
pub struct Stream<'a> {
    header: Header,
    records: &'a [u8],
}

impl<'a> Stream<'a> {
    /// Parse a buffer that contains the header followed by the record area.
    pub fn parse(region: &'a [u8]) -> Result<Self, RtstError> {
        if region.len() < HEADER_LEN {
            return Err(RtstError::HeaderTooShort { got: region.len() });
        }
        let header = Header::parse(&region[..HEADER_LEN])?;
        let available = region.len() - HEADER_LEN;
        if available < header.write_pos as usize {
            return Err(RtstError::WposOutOfBounds {
                wpos: header.write_pos,
                available,
            });
        }
        let records = &region[HEADER_LEN..HEADER_LEN + header.write_pos as usize];
        Ok(Self { header, records })
    }

    /// Create a stream view from separate header and record slices.
    pub fn from_parts(header_bytes: &'a [u8], records: &'a [u8]) -> Result<Self, RtstError> {
        let header = Header::parse(header_bytes)?;
        if records.len() < header.write_pos as usize {
            return Err(RtstError::WposOutOfBounds {
                wpos: header.write_pos,
                available: records.len(),
            });
        }
        Ok(Self { header, records: &records[..header.write_pos as usize] })
    }

    /// Parsed header reference.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Iterator over decoded records.
    pub fn iter(&self) -> RecordIter<'a> {
        RecordIter {
            buf: self.records,
            offset: 0,
        }
    }
}

/// Iterator of `Record` objects extracted from the buffer.
pub struct RecordIter<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for RecordIter<'a> {
    type Item = Result<Record<'a>, RtstError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.buf.len() {
            return None;
        }
        if self.buf.len() - self.offset < 3 {
            let err = RtstError::RecordHeaderTooShort {
                offset: self.offset,
                remaining: self.buf.len() - self.offset,
            };
            self.offset = self.buf.len();
            return Some(Err(err));
        }
        let kind = self.buf[self.offset];
        let len = u16::from_le_bytes([
            self.buf[self.offset + 1],
            self.buf[self.offset + 2],
        ]) as usize;
        let payload_start = self.offset + 3;
        if self.buf.len() - payload_start < len {
            let err = RtstError::RecordOverruns {
                offset: self.offset,
                len,
                remaining: self.buf.len() - payload_start,
            };
            self.offset = self.buf.len();
            return Some(Err(err));
        }
        let payload = &self.buf[payload_start..payload_start + len];
        let record = Record::new(kind, payload, payload_start);
        self.offset = payload_start + len;
        Some(Ok(record))
    }
}

/// Streaming writer used by tests and host tooling to build RTST buffers.
#[derive(Debug, Default)]
pub struct StreamEncoder {
    header: Header,
    records: Vec<u8>,
}

impl StreamEncoder {
    /// Create an empty stream encoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record with the provided payload.
    pub fn push_record(
        &mut self,
        kind: RecordId,
        payload: impl AsRef<[u8]>,
    ) -> Result<&mut Self, RtstError> {
        let payload = payload.as_ref();
        if payload.len() > u16::MAX as usize {
            return Err(RtstError::RecordAreaOverflow { len: payload.len() });
        }
        let start_len = self.records.len();
        self.records.push(kind as u8);
        self.records
            .extend_from_slice(&(payload.len() as u16).to_le_bytes());
        self.records.extend_from_slice(payload);
        if self.records.len() > u16::MAX as usize {
            self.records.truncate(start_len);
            return Err(RtstError::RecordAreaOverflow {
                len: self.records.len(),
            });
        }
        self.header.write_pos = self.records.len() as u16;
        Ok(self)
    }

    /// Update header counters after writing cases.
    pub fn set_counts(
        &mut self,
        total: u16,
        passed: u16,
        failed: u16,
    ) -> &mut Self {
        self.header.total_cases = total;
        self.header.passed_cases = passed;
        self.header.failed_cases = failed;
        self
    }

    /// Explicitly override the header state.
    pub fn set_state(&mut self, state: State) -> &mut Self {
        self.header.state = state;
        self
    }

    /// Finalize and return the concatenated header+record buffer.
    pub fn finish(self) -> Vec<u8> {
        let mut out = vec![0u8; HEADER_LEN + self.records.len()];
        self.header.encode(&mut out[..HEADER_LEN]).unwrap();
        out[HEADER_LEN..].copy_from_slice(&self.records);
        out
    }
}

fn split_cstring<'a>(
    bytes: &'a [u8],
    offset: usize,
) -> Result<(&'a str, &'a [u8], usize), RtstError> {
    let Some(pos) = bytes.iter().position(|b| *b == 0) else {
        return Err(RtstError::MissingCStringTerminator { offset });
    };
    let (head, rest) = bytes.split_at(pos + 1);
    let value = std::str::from_utf8(&head[..pos])
        .map_err(|_| RtstError::Utf8Error { offset })?;
    Ok((value, rest, offset + pos + 1))
}

fn require_payload(bytes: &[u8], needed: usize, offset: usize) -> Result<(), RtstError> {
    if bytes.len() < needed {
        return Err(RtstError::RecordPayloadTooShort {
            offset,
            needed,
            actual: bytes.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cstr(text: &str) -> Vec<u8> {
        let mut data = Vec::from(text.as_bytes());
        data.push(0);
        data
    }

    #[test]
    fn record_iteration_preserves_order() {
        let mut encoder = StreamEncoder::new();
        encoder
            .push_record(RecordId::CaseStart, cstr("case::one"))
            .unwrap();
        encoder
            .push_record(RecordId::Msg, cstr("log::msg"))
            .unwrap();
        encoder.push_record(RecordId::End, []).unwrap();
        let bytes = encoder.finish();

        let stream = Stream::parse(&bytes).unwrap();
        let kinds: Vec<_> = stream
            .iter()
            .map(|rec| rec.unwrap().kind().unwrap())
            .collect();
        assert_eq!(
            kinds,
            vec![RecordId::CaseStart, RecordId::Msg, RecordId::End]
        );
    }

    #[test]
    fn detects_wpos_out_of_bounds() {
        let mut encoder = StreamEncoder::new();
        encoder
            .push_record(RecordId::CaseStart, cstr("case::tiny"))
            .unwrap();
        let mut bytes = encoder.finish();
        bytes.truncate(HEADER_LEN + 2);

        let err = Stream::parse(&bytes).unwrap_err();
        assert!(matches!(err, RtstError::WposOutOfBounds { .. }));
    }

    #[test]
    fn unknown_record_id_survives_iteration() {
        let mut header = Header::new();
        header.set_state(State::Running);
        header.set_write_pos(3);
        let mut bytes = vec![0u8; HEADER_LEN + 3];
        header.encode(&mut bytes[..HEADER_LEN]).unwrap();
        bytes[HEADER_LEN] = 0x99;
        bytes[HEADER_LEN + 1] = 0;
        bytes[HEADER_LEN + 2] = 0;

        let stream = Stream::parse(&bytes).unwrap();
        let mut iter = stream.iter();
        let record = iter.next().unwrap().unwrap();
        assert_eq!(record.raw_id(), 0x99);
        assert!(record.kind().is_none());
        assert!(iter.next().is_none());
    }
}
