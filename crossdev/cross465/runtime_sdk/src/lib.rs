//! Cross465 Runtime SDK primitives.
//!
//! The `runtime_sdk` crate hosts shared protocol definitions used by 6502
//! assembly tests (`test_rtst.inc`) and host-side tooling (`asm6502_test!`,
//! `cross465-test-runner`).  Step 1 of the testing architecture plan introduces
//! the Runtime Test Stream (RTST) primitives exposed through the [`rtst`] module.

pub mod rtst;
