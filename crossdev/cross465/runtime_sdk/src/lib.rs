//! Cross465 Runtime SDK primitives.
//!
//! The `runtime_sdk` crate hosts shared protocol definitions used by 6502
//! assembly tests (`test_rtst.h`) and host-side tooling (`asm6502_test!`,
//! `cross465-test-runner`, and any future host-side parser that needs to interpret RTST buffers).
//!
//! This crate exposes the Runtime Test Stream (RTST) data structures and helpers
//! under the [`rtst`] module.

pub mod rtst;
