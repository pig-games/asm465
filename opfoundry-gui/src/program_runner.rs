use std::time::Duration;

use bus::{unicode_to_screen, Bus, CONSOLE_CHAR_ADDR, CONSOLE_COMMIT_ADDR};
use core6502::{Cpu, RunLimit, RunOutcome};
use runtime_sdk::rtst::{Header, State, HEADER_LEN};
use web_time::Instant;

use crate::{ProgramRunReport, RtstMonitorConfig, StartupConfig};

#[allow(clippy::result_large_err)]
pub(crate) fn run_program_with_config(
    bus: Bus,
    config: &StartupConfig,
) -> Result<(Bus, ProgramRunReport), (Bus, ProgramRunReport)> {
    let label = config.source.label();
    let data = match config.source.load_bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err((
                bus,
                ProgramRunReport {
                    outcome: None,
                    message: err,
                },
            ));
        }
    };

    if data.len() < 2 {
        return Err((
            bus,
            ProgramRunReport {
                outcome: None,
                message: format!("Program {label} is too small to contain a load address"),
            },
        ));
    }

    let load_addr = u16::from_le_bytes([data[0], data[1]]);
    let body = &data[2..];
    let mut bus = bus;
    bus.load(load_addr, body);
    let start = config.start.unwrap_or(load_addr);
    bus.set_reset_vector(start);

    let mut cpu = Cpu::new(bus);
    cpu.reset();
    let progress_timeout = config.progress_timeout_ms.map(Duration::from_millis);
    let outcome = if let Some(rtst) = config.rtst {
        match run_until_rtst_done(&mut cpu, rtst, config.max_cycles, progress_timeout) {
            Ok(outcome) => outcome,
            Err(message) => {
                let bus = cpu.into_bus();
                return Err((
                    bus,
                    ProgramRunReport {
                        outcome: None,
                        message,
                    },
                ));
            }
        }
    } else {
        cpu.run_for(config.max_cycles)
    };
    let bus = cpu.into_bus();

    let limit_desc = match outcome.limit {
        RunLimit::CycleBudget => "cycle budget",
        RunLimit::Brk => "BRK",
        RunLimit::Halted => "halted (illegal opcode)",
    };

    let summary = format!(
        "Loaded {label} at ${:04X} and ran for {} cycles (reason: {limit_desc}, start=${:04X})",
        load_addr, outcome.cycles, start
    );

    Ok((
        bus,
        ProgramRunReport {
            outcome: Some(outcome),
            message: summary,
        },
    ))
}

fn run_until_rtst_done(
    cpu: &mut Cpu,
    monitor: RtstMonitorConfig,
    max_cycles: u64,
    progress_timeout: Option<Duration>,
) -> Result<RunOutcome, String> {
    let poll_interval = 1024u64;
    let mut next_poll = poll_interval;
    let mut header_buf = [0u8; HEADER_LEN];
    let mut cycles: u64 = 0;
    let mut initialized = false;
    let mut last_wpos: u16 = 0;
    let mut last_state = State::Pending;
    let mut last_progress = Instant::now();
    let base = monitor.base as u16;

    loop {
        if cpu.halted {
            return Err("CPU halted (illegal opcode)".into());
        }
        let step_cycles = cpu.step() as u64;
        cycles = cycles.saturating_add(step_cycles);
        if cycles >= max_cycles {
            return Err(format!(
                "RTST timeout after {cycles} cycles (state={:?}, wpos={last_wpos})",
                last_state
            ));
        }
        if cycles < next_poll {
            continue;
        }
        next_poll = next_poll.saturating_add(poll_interval);
        for (i, slot) in header_buf.iter_mut().enumerate().take(HEADER_LEN) {
            *slot = cpu.bus_mut().read(base.wrapping_add(i as u16));
        }
        match Header::parse(&header_buf) {
            Ok(header) => {
                if !initialized {
                    initialized = true;
                    last_progress = Instant::now();
                }
                if header.write_pos() != last_wpos || header.state() != last_state {
                    last_progress = Instant::now();
                }
                last_wpos = header.write_pos();
                last_state = header.state();
                if header.state().is_terminal() {
                    break;
                }
            }
            Err(_) => {
                if !initialized {
                    if let Some(limit) = progress_timeout {
                        if last_progress.elapsed() >= limit {
                            return Err("RTST header was never initialised".into());
                        }
                    }
                }
                continue;
            }
        }
        if let Some(limit) = progress_timeout {
            if initialized && last_progress.elapsed() >= limit {
                return Err(format!(
                    "RTST made no progress for {:?} (state={:?}, wpos={last_wpos})",
                    limit, last_state
                ));
            }
        }
    }

    Ok(RunOutcome {
        cycles,
        limit: RunLimit::Brk,
    })
}

pub(crate) fn write_console_line(bus: &mut Bus, line: &str) {
    for ch in line.chars() {
        let screen_code = unicode_to_screen(ch);
        if screen_code == b'\n' {
            bus.write(CONSOLE_COMMIT_ADDR, 0);
        } else {
            bus.write(CONSOLE_CHAR_ADDR, screen_code);
        }
    }
    bus.write(CONSOLE_COMMIT_ADDR, 0);
}
