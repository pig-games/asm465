# Signal → Register Value Builders
[← Personality File](03-Personality-Spec.md) • [→ Migration & Tests](05-Migration-and-Tests.md)

## Purpose
Pack **logical backend signals** (e.g., `dpad_up`, `button_a`, `axis_lx`) into **platform-specific register values** declaratively.

---

## 1) Schema (per sparse entry)
```toml
[[map]]
decode = { sparse = [
  { addr="DC00", kind="input", id="JoyPort2",
    value_builder = {
      width = 1,
      bits  = [
        { bit=0, src="dpad_up",    active_low=true  },
        { bit=1, src="dpad_down",  active_low=true  },
        { bit=2, src="dpad_left",  active_low=true  },
        { bit=3, src="dpad_right", active_low=true  },
        { bit=4, src="button_a",   active_low=true  },
      ],
      fields = [
        { lsb=5, msb=7, src="throttle3", kind="int", clamp=[0,7] }
      ],
      const_set = "00000000",
      post = { invert_byte=false }
    }
  }
]}
```

### bits[]
Maps boolean signals to bits.
- `bit`: bit position (0–7)
- `src`: boolean signal name
- `active_low`: optional, invert this bit
- `byte_index`: optional for multi-byte builders

### fields[]
Pack numeric signals into bitfields or bytes.
- `lsb`, `msb`: bit range
- `src`: signal name
- `kind`: `"int"` or `"float"`
- `scale`, `clamp`, `offset`: range transformations

---

## 2) Examples

### C64 Joystick (active-low)
```toml
{ addr="DC00", kind="input", id="JoyPort2",
  value_builder = { width=1, bits=[
    { bit=0, src="p0.dpad_up",    active_low=true },
    { bit=1, src="p0.dpad_down",  active_low=true },
    { bit=2, src="p0.dpad_left",  active_low=true },
    { bit=3, src="p0.dpad_right", active_low=true },
    { bit=4, src="p0.button_a",   active_low=true },
  ]}
}
```

### Atari: PORTA + TRIG0
```toml
{ addr="D300", kind="input", id="PortA",
  value_builder = { width=1, bits=[
    { bit=0, src="p0.dpad_up",    active_low=true },
    { bit=1, src="p0.dpad_down",  active_low=true },
    { bit=2, src="p0.dpad_left",  active_low=true },
    { bit=3, src="p0.dpad_right", active_low=true },
    { bit=4, src="p1.dpad_up",    active_low=true },
    { bit=5, src="p1.dpad_down",  active_low=true },
    { bit=6, src="p1.dpad_left",  active_low=true },
    { bit=7, src="p1.dpad_right", active_low=true },
  ]}
},
{ addr="D010", kind="input", id="Trig0",
  value_builder = { width=1, bits=[{ bit=0, src="p0.button_a", active_low=true }], const_set="11111110" }
}
```

### Float Axis Scaling
```toml
{ addr="DF40", kind="input", id="AxisX",
  value_builder = { width=1, fields=[
    { lsb=0, msb=7, src="p0.axis_x", kind="float", scale={ from=[-1.0,1.0], to=[0,255] }, clamp=[0,255] }
  ]}
}
```

---

## 3) Signal Resolution API

```rust
pub trait InputSignals {
    fn get_bool(&self, name: &str) -> Option<bool>;
    fn get_int (&self, name: &str) -> Option<i32>;
    fn get_f32 (&self, name: &str) -> Option<f32>;
}
```

All `src` names are resolved once per tick by the input module; for performance, they’re compiled to IDs when the personality loads.

---

## 4) Read Path Summary
1. Bus decodes address → `(kind, RegId, Transform, value_builder?)`
2. If `value_builder`: build value from signals
3. Else: `module.read(RegId)`
4. Apply transform (invert/masks/hooks)
5. Return value

---

## 5) Testing
- Verify C64 active-low joystick packing
- Verify Atari split TRIGs
- Verify float scaling (−1..1 → 0..255)
- Verify multi-byte packing for word registers
