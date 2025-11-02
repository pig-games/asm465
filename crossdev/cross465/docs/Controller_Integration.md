# Generic Game Controller Integration
[← Sprite Collision and Bevy](Sprite_Collision_and_Bevy.md) | [← Platform Examples](Platform_Examples.md)

This chapter details input systems on classic 6502 platforms and their mapping to Bevy’s modern gamepad API.

---

## Commodore 64

Two DE-9 joystick ports on CIA1.  
Active-low: bit = 0 means pressed.

```asm
lda $dc00
 eor #$ff
 and #%00011111
 sta joy_state
```

---

## Atari 8-bit

Four joystick ports via PIA/POKEY.  
Analog paddles read 0–228 counts from `$D200–$D207`.

```asm
lda $d300
 eor #$ff
 and #%00011111
 sta joy_state
```

### NMI Enable Explanation (Atari)

NMI cannot be disabled by the CPU, but ANTIC’s **NMIEN ($D40E)** register controls whether internal events generate NMI:

| Bit | Meaning     | Effect                                         |
| --- | ----------- | ---------------------------------------------- |
| 7   | DLI enable  | Allows Display List Interrupts to assert NMI   |
| 6   | VBI enable  | Allows Vertical Blank Interrupts to assert NMI |
| 5   | Reset clear | Clears pending flags                           |

The CPU always responds to an NMI edge, but ANTIC gates whether it asserts one. Reading `$D40F` acknowledges and clears the source.

---

## Apple II

Analog paddles timed via RC network and `$C061` strobe.

```asm
lda $c061
wait:
  bit $c064
  beq wait
```

---

## Bevy Gamepad API

Bevy abstracts controller input across platforms using typed enums and ECS resources.

### Gamepad Identification
```rust
let pad = Gamepad::new(0);
```

### Buttons
```rust
pub enum GamepadButtonType {
    South, East, North, West,
    C, Z,
    LeftTrigger, RightTrigger,
    LeftTrigger2, RightTrigger2,
    Select, Start, Mode,
    LeftThumb, RightThumb,
    DPadUp, DPadDown, DPadLeft, DPadRight,
}
```

| Bevy | Xbox | PlayStation | Nintendo |
|------|-------|-------------|-----------|
| `South` | A | ✕ Cross | B |
| `East` | B | ◯ Circle | A |
| `North` | Y | △ Triangle | X |
| `West` | X | □ Square | Y |

### Axes
```rust
pub enum GamepadAxisType {
    LeftStickX, LeftStickY,
    RightStickX, RightStickY,
    LeftZ, RightZ,
}
```

### Checking Input
```rust
fn handle_gamepad(
    buttons: Res<Input<GamepadButton>>,
    axes: Res<Axis<GamepadAxis>>,
) {
    let pad = Gamepad::new(0);
    if buttons.just_pressed(GamepadButton::new(pad, GamepadButtonType::South)) {
        println!("Jump pressed!");
    }

    let x = axes.get(GamepadAxis::new(pad, GamepadAxisType::LeftStickX)).unwrap_or(0.0);
    let y = axes.get(GamepadAxis::new(pad, GamepadAxisType::LeftStickY)).unwrap_or(0.0);
}
```

### Events
```rust
fn read_gamepad_events(mut evr: EventReader<GamepadButtonChangedEvent>) {
    for ev in evr.read() {
        println!("Gamepad {:?} {:?} value {}", ev.gamepad, ev.button_type, ev.value);
    }
}
```

### Mapping Vintage → Modern
| Concept       | 6502 Vintage | Bevy Modern |
| ------------- | ------------- | ------------ |
| Fire button   | 1 bit         | `GamepadButtonType::South` |
| 4-way stick   | 4 bits        | D-pad or left stick |
| Analog paddle | 0–228 range   | Axis −1.0 … 1.0 |
| Polling       | MMIO read     | `Input<GamepadButton>` + `Axis<GamepadAxis>` |

### asm465 Integration
For **cross465/modern** targets (implemented in `bus/src/adapters/input.rs` and `asm465/src/lib.rs`):
- Cache raw Bevy button/axis events per gamepad in a host-side state block so other systems (MMIO adapters, developer tools) can read both the *current* and *last* non-release values.
- Write the cached state into `$DC00/$DC01` (C64) or `$D300/$D301` (Atari); conversions happen inside the controller adapter so personalities always see legacy-active-low bits.
- Map Bevy axis values into analog paddle registers (`$D200+` or `$C064+`). When an axis reaches the edge of its range (≈0.0/1.0, or 0/255 after scaling), the adapter also toggles the corresponding D-pad bit so analog sticks and paddle-only devices behave like digital joysticks.
- Ignore release-only events when recording the "last" value; the developer tools panel should continue showing the most recent meaningful press until a new press arrives.
- Expose per-pad telemetry (pads 0 and 1) and keyboard history in the developer tools panel so testers can confirm both the modern state and the translated MMIO view in real time.

This allows modern controllers to emulate 6502-era inputs in simulations and development tools.

---

# MMIO Layouts for Vintage Controller Input

These tables list the memory-mapped I/O addresses for joysticks, paddles, and input devices on each vintage platform.

---

## Commodore 64 (CIA1 + SID)

### CIA1 – Port Registers (`$DC00–$DC0F`)
| Address | Name | Purpose |
|----------|------|----------|
| `$DC00` | **PRA** | Joystick Port 2 directions + fire. Bits 0–4 active low: Up, Down, Left, Right, Fire. |
| `$DC01` | **PRB** | Joystick Port 1 directions + fire (bits 0–4 active low). |
| `$DC02` | **DDRA** | Data Direction A (0 = input). |
| `$DC03` | **DDRB** | Data Direction B (0 = input). |
| `$DC04/$DC05` | **Timer A Low/High** | (Timing only). |
| `$DC06/$DC07` | **Timer B Low/High** |  |
| `$DC0D` | **ICR** | Interrupt Control/Status. |
| `$DC0E/$DC0F` | **CRA/CRB** | Control Registers. |

### SID – Paddle Inputs (`$D419–$D41A`)
| Address | Name | Range | Purpose |
|----------|------|--------|----------|
| `$D419` | POTX | 0–255 | Paddle X read |
| `$D41A` | POTY | 0–255 | Paddle Y read |

```asm
lda #$00
sta $dc02
sta $dc03
lda $dc00
eor #$ff
and #%00011111
sta joy2
```

---

## Atari 8-bit (PIA + GTIA + POKEY)

### PIA – Direction Switches (`$D300–$D303`)
| Address | Name | Purpose |
|----------|------|----------|
| `$D300` | **PORTA** | Joysticks 0–1 directions (bits 0–3 = Stick 0, 4–7 = Stick 1, active low). |
| `$D301` | **PORTB** | Joysticks 2–3 directions (bits 0–3 = Stick 2, 4–7 = Stick 3). |

### GTIA – Triggers and Console Keys
| Address | Name | Purpose |
|----------|------|----------|
| `$D010–$D013` | **TRIG0–3** | Fire buttons (active low). |
| `$D01F` | **CONSOL** | Console buttons (OPTION/SELECT/START). |

### POKEY – Analog Paddles
| Address | Name | Purpose |
|----------|------|----------|
| `$D200–$D207` | **POT0–POT7** | Paddle values (0–228). |
| `$D208` | **ALLPOT** | Paddle ready flags. |

```asm
lda $d300
eor #$ff
sta dir01
lda $d010
eor #$ff
and #$01
sta trig0
lda $d200
sta pad0
```

---

## Apple II (Game I/O `$C060–$C07F`)

| Address | Name | Purpose |
|----------|------|----------|
| `$C060–$C063` | PB0–PB3 | Pushbuttons 0–3 (active low). |
| `$C064–$C067` | PDL0–PDL3 | Paddle timing bits. |
| `$C070` | PTRIG | Paddle strobe (start timing). |

```asm
lda $c070
ldx #$00
wait_pdl0:
 bit $c064
 bpl wait_pdl0
 stx paddle0_ticks
```

---

**References**
- *Commodore 64 Programmer’s Reference Guide* (1982, Commodore)  
- *Atari Hardware Manual* (Atari Inc., 1979–1982)  
- *Apple II Reference Manual* (Apple Computer, 1978)
