//! Console MMIO device for the cross465 Bus.
//!
//! This device lets 6502 code “print” to the host terminal by writing to an
//! MMIO window. It also keeps a buffer so tests can assert the printed output.
//!
//! ## Address window: `$DF00–$DF1F`
//! - `$DF00`: write a byte → prints a character
//! - `$DF01`: write any value → prints newline
//! - `$DF02`: write a byte → prints two hex digits (debug)
//!
//! The device is mapped by default by [`Bus::new`](crate::Bus::new). You can
//! enable a PETSCII‑ish translation mode by calling
//! [`Bus::with_console_petscii`](crate::Bus::with_console_petscii).

use crate::Memory;
use crate::MmioDevice;
use crate::{cmb_color_to_ansi, petscii_to_unicode, screen_to_petscii};
use console::style;
use console::Term;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Default character columns for the virtual console surface.
const DEFAULT_WIDTH: usize = 80;
/// Default character rows for the virtual console surface.
const DEFAULT_HEIGHT: usize = 50;

/// Single cell of console output state.
#[derive(Clone)]
pub struct ConsoleCell {
    /// Character currently painted in this cell.
    pub ch: char,
    /// Foreground colour index (C64 palette).
    pub fg: u8,
    /// Background colour index (C64 palette).
    pub bg: u8,
}

impl Default for ConsoleCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: 7,
            bg: 0,
        }
    }
}

/// Snapshot of the virtual console surface.
#[derive(Clone)]
pub struct ConsoleSnapshot {
    /// Snapshot width in characters.
    pub width: usize,
    /// Snapshot height in characters.
    pub height: usize,
    /// Row-major cell data backing this snapshot.
   pub cells: Vec<ConsoleCell>,
}

impl ConsoleSnapshot {
    #[inline]
    pub fn cell(&self, x: usize, y: usize) -> &ConsoleCell {
        &self.cells[y * self.width + x]
    }
}

/// Shared buffer of console output for host integrations (GUI, tooling, etc.).
pub struct ConsoleOutput {
    /// Logical screen width (columns).
    width: usize,
    /// Logical screen height (rows).
    height: usize,
    /// Dense row-major backing store for all cells.
    cells: Vec<ConsoleCell>,
    /// Cursor X position, clamped to the surface.
    cursor_x: usize,
    /// Cursor Y position, clamped to the surface.
    cursor_y: usize,
}

impl Default for ConsoleOutput {
    fn default() -> Self {
        Self::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }
}

impl ConsoleOutput {
    /// Create a new surface with the supplied character dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        let mut output = Self {
            width,
            height,
            cells: vec![ConsoleCell::default(); width * height],
            cursor_x: 0,
            cursor_y: 0,
        };
        output.clear();
        output
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn clamp_coords(&self, x: u8, y: u8) -> (usize, usize) {
        (
            (x as usize).min(self.width.saturating_sub(1)),
            (y as usize).min(self.height.saturating_sub(1)),
        )
    }

    fn scroll_up(&mut self) {
        self.cells.drain(0..self.width);
        self.cells
            .extend(std::iter::repeat(ConsoleCell::default()).take(self.width));
        self.cursor_y = self.height.saturating_sub(1);
    }

    fn advance_line(&mut self) {
        if self.cursor_y + 1 >= self.height {
            self.scroll_up();
        } else {
            self.cursor_y += 1;
        }
        self.cursor_x = 0;
    }

    /// Paint a single character at the current cursor position.
    pub fn write_char(&mut self, ch: char, fg: u8, bg: u8) {
        if ch == '\n' {
            self.advance_line();
            return;
        }
        if self.cursor_x >= self.width {
            self.advance_line();
        }
        let idx = self.index(self.cursor_x, self.cursor_y);
        self.cells[idx] = ConsoleCell { ch, fg, bg };
        self.cursor_x += 1;
        if self.cursor_x >= self.width {
            self.advance_line();
        }
    }

    /// Paint an entire string, honouring embedded newlines.
    pub fn write_str(&mut self, text: &str, fg: u8, bg: u8) {
        for ch in text.chars() {
            self.write_char(ch, fg, bg);
        }
    }

    /// Advance the cursor to the next line, scrolling if necessary.
    pub fn newline(&mut self) {
        self.advance_line();
    }

    /// Clear the screen and reset the cursor to the home position.
    pub fn clear(&mut self) {
        self.cells.fill(ConsoleCell::default());
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Position the cursor, clamping values to the screen bounds.
    pub fn set_cursor(&mut self, x: u8, y: u8) {
        let (cx, cy) = self.clamp_coords(x, y);
        self.cursor_x = cx;
        self.cursor_y = cy;
    }

    /// Produce an immutable snapshot of the current surface.
    pub fn snapshot(&self) -> ConsoleSnapshot {
        ConsoleSnapshot {
            width: self.width,
            height: self.height,
            cells: self.cells.clone(),
        }
    }

    /// Render the surface to a simple monochrome text representation.
    pub fn to_plain_string(&self) -> String {
        let mut out = String::new();
        for row in 0..self.height {
            let start = row * self.width;
            let end = start + self.width;
            let line: String = self.cells[start..end].iter().map(|cell| cell.ch).collect();
            let line_trimmed = line.trim_end_matches(' ');
            out.push_str(line_trimmed);
            if row + 1 < self.height {
                out.push('\n');
            }
        }
        out
    }
}

/// Console MMIO device for host-side text output.
pub struct ConsoleMmio {
    ram: Arc<Mutex<Memory>>,
    /// Accumulates printed output for inspection (tests, tooling, etc.).
    pub term: Term,
    /// If `true`, interpret bytes using a PETSCII‑ish mapping; otherwise a
    /// simple ASCII‑ish pass‑through is used.
    pub petscii_mode: bool,
    pub x: u8,
    pub y: u8,
    pub color: u8,
    pub bg_color: u8,
    pub lptr: u8,
    pub hptr: u8,
    pub plength: u8,
    output: Arc<Mutex<ConsoleOutput>>,
}

impl ConsoleMmio {
    /// Create a new console device with an empty buffer and ASCII‑ish mode.
    pub fn new(ram: Arc<Mutex<Memory>>) -> Self {
        let term = Term::stdout();
        term.style().force_styling(true);
        Self {
            ram,
            term,
            petscii_mode: true,
            x: 0,
            y: 0,
            color: 7,
            bg_color: 0,
            lptr: 0,
            hptr: 0,
            plength: 0,
            output: Arc::new(Mutex::new(ConsoleOutput::default())),
        }
    }

    /// Print a single character byte according to the current mode.
    fn push_char(&mut self, b: u8) {
        let ch = if self.petscii_mode {
            petscii_to_unicode(screen_to_petscii(b))
        } else {
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else if b == 0x0D {
                // Treat CR as newline for convenience.
                '\n'
            } else {
                // Placeholder for non‑printables/high bytes in ASCII-ish mode.
                '·'
            }
        };
        if ch == '\n' {
            self.newline();
            return;
        }
        write!(
            &self.term,
            "{}",
            &format!(
                "{}",
                style(ch)
                    .fg(cmb_color_to_ansi(self.color))
                    .bg(cmb_color_to_ansi(self.bg_color))
            )
        )
        .unwrap();
        self.output
            .lock()
            .unwrap()
            .write_char(ch, self.color, self.bg_color);
    }

    /// Print a newline (also pushes '\n' to the buffer).
    fn newline(&mut self) {
        self.term.write_line("").unwrap();
        self.output.lock().unwrap().newline();
    }

    /// Print a byte as two hexadecimal digits (debugging helper).
    fn push_hex(&mut self, b: u8) {
        let s = format!("{b:02X}");
        self.term.write(&s.as_bytes()).unwrap();
        self.output
            .lock()
            .unwrap()
            .write_str(&s, self.color, self.bg_color);
    }

    /// Clear the internal output buffer (handy for test setup/teardown).
    pub fn clear(&mut self) {
        self.term.clear_screen().unwrap();
        let mut output = self.output.lock().unwrap();
        output.clear();
    }

    pub fn set_x(&mut self, b: u8) {
        self.x = b;
    }

    pub fn set_y(&mut self, b: u8) {
        self.y = b;
    }

    pub fn set_location(&mut self) {
        self.term
            .move_cursor_to(self.x.into(), self.y.into())
            .unwrap();
        self.output.lock().unwrap().set_cursor(self.x, self.y);
    }

    pub fn set_color(&mut self, b: u8) {
        self.color = b;
    }

    pub fn set_bg_color(&mut self, b: u8) {
        self.bg_color = b;
    }

    pub fn set_lptr(&mut self, b: u8) {
        self.lptr = b;
    }

    pub fn set_hptr(&mut self, b: u8) {
        self.hptr = b;
    }

    /// Read a NUL-terminated string from RAM at addr (u16) and print it.
    pub fn print(&mut self, high: u8) {
        // Compose 16-bit pointer from high/low registers
        self.set_hptr(high);
        let mut addr = (((self.hptr as u16) << 8) | (self.lptr as u16)) as u16;
        let start = addr;
        loop {
            // limit RAM lock scope so we don't hold an immutable borrow across
            // the mutable self.push_char(...) call
            let b = {
                let mem = self.ram.lock().unwrap();
                mem.read(addr)
            };
            if b == 0 {
                break;
            }
            match petscii_to_unicode(screen_to_petscii(b)) {
                '!' => {
                    addr = addr.wrapping_add(1);
                    let c = {
                        let mem = self.ram.lock().unwrap();
                        mem.read(addr)
                    };
                    match petscii_to_unicode(screen_to_petscii(c)) {
                        'n' => self.newline(),
                        _ => self.push_char(c),
                    }
                }
                _ => self.push_char(b),
            }
            addr = addr.wrapping_add(1);
        }
        self.set_lptr((addr & 0x00FF) as u8);
        self.set_hptr((addr >> 8) as u8);
        self.plength = (addr - start) as u8;
        //println!("length: {}", self.plength);
    }

    /// Shared output buffer handle for host integrations.
    pub fn output(&self) -> Arc<Mutex<ConsoleOutput>> {
        Arc::clone(&self.output)
    }
}

impl MmioDevice for ConsoleMmio {
    /// Returns 0 for all addresses; the registers are write‑only in this device.
    fn read(&mut self, addr: u16) -> u8 {
        let val = match addr & 0x001F {
            0x00 | 0x01 | 0x02 => 0, // write-only registers
            0x04 => self.x,
            0x05 => self.y,
            0x07 => self.color,
            0x08 => self.bg_color,
            0x09 => self.lptr,
            0x0a => self.hptr,
            0x0b => self.plength,
            _ => 0,
        };
        //println!("ConsoleMmio: read {:#06x} => {}", addr, val);
        val
    }

    /// Dispatch writes to the appropriate “register”.
    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x001F {
            0x00 => self.push_char(value),
            0x01 => self.newline(),
            0x02 => self.push_hex(value),
            0x03 => self.clear(),
            0x04 => self.set_x(value),
            0x05 => self.set_y(value),
            0x06 => self.set_location(),
            0x07 => self.set_color(value),
            0x08 => self.set_bg_color(value),
            0x09 => self.set_lptr(value),
            0x0a => self.set_hptr(value),
            0x0b => self.print(value), // only requires previous call to set_lptr(val), the passed value is the high ptr for the text to be printed.
            _ => { /* reserved for future features (cursor, color, clear, etc.) */ }
        }
    }
}
