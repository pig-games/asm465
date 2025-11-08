# Pointer Decoders & Address Semantics
[← Prototype and Conformance](09_Prototype_and_Conformance.md)

Pointer decoders interpret MMIO pointer writes (such as sprite, screen, or charset pointers) into structured values that modern backends can use.

---

## 1. Purpose

Pointer registers were often used to index graphics or screen memory blocks in 6502‑based systems.  
A pointer decoder formalizes this by producing a normalized pair:
```
(index, subindex)
```
where **index** identifies the main resource (e.g., sprite number, screen page) and **subindex** identifies a variant (e.g., animation frame).

---

## 2. Declarative Schema

```toml
[pointer_decoder.<name>]
source   = { table = "07F8..07FF", kind = "sprite", entries = 8 }
block    = 64
subindex = { bits = 2 }
index    = { from = "pointer", shift = 2 }
```

- `source`: table or register defining the pointer(s).  
- `block`: memory block size per entry (e.g., 64 bytes per sprite).  
- `subindex.bits`: low bits used as variant selector.  
- `index.shift`: shifts the pointer right by `subindex.bits` to find the base index.

---

## 3. Integration with Adapters

Adapters receive decoded `(index, subindex)` pairs and forward them to backend methods:

```rust
fn set_sprite_variant(index: u8, subindex: u8);
fn set_screen_base(addr: u16);
fn set_charset_base(addr: u16);
```

---

## 4. Extension Bits

Additional bits can extend the variant space — for example from bank or mode registers.  
The number of extension bits `E` augments the subindex space as:

```
subindex = (P & ((1<<S)-1)) | ((BANK & ((1<<E)-1)) << S)
index    =  P >> S
```

This allows up to `(S + E)` bits of variant selection, enabling richer personality mappings.

---

## 5. Cross‑references

For scaling flags (C64 `$D01D` / `$D017`) appended as variant bits and transform fallback behaviour, see  
➡ [Scaling‑aware Pointer Decode (Option A)](07_Mapping_DSL_and_Examples.md#12-scalingaware-pointer-decode-option-a-with-fallback).

---

## 6. Summary

Pointer decoders cleanly separate pointer arithmetic from engine logic.  
They provide a declarative, reusable way to express vintage memory pointer semantics and modern variant selection, compatible with scatter/fanout‑aware adapters and scaling‑aware personalities.
