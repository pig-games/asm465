# Sprite Collision Detection and Bevy Integration

[← Platform Examples](Platform_Examples.md) | [→ Controller Integration](Controller_Integration.md)

This document summarizes sprite collision detection strategies on the Commodore 64, Atari 8-bit series, and Apple II, and demonstrates equivalent host-side collision logic using the Rust **Bevy** engine in asm465.

---

## Commodore 64 (VIC-II)

### Hardware Features

- **8 sprites**, each 24×21 pixels (expandable).
- **Collision latches:**
  - `$D01E`: Sprite–Sprite collisions.
  - `$D01F`: Sprite–Background collisions.
- **Clear mechanism:** Writing any value clears the latches.

### Collision Approaches

1. **Hardware-only:**
   - Poll `$D01E/$D01F` to detect collisions.
   - Cheap and instantaneous but does not identify which pair collided.
2. **Hybrid:**
   - Hardware detection + software refinement (AABB/pixel tests).
3. **Tile occupancy:**
   - Map sprite positions to character cells to detect terrain contact.
4. **Raster multiplexing:**
   - Sample and clear latches per raster band when reusing sprites.

### Example

```asm
read_colls
    lda $d01e          ; sprite-sprite latch
    sta sprite_sprite_hits
    lda $d01f          ; sprite-background latch
    sta sprite_bg_hits
    lda #$ff           ; clear both latches
    sta $d01e
    sta $d01f
    rts
```

---

## Atari 8-bit (GTIA/ANTIC)

### Hardware Features

- **Player/Missile Graphics (PMG):** 4 players + 4 missiles.
- **Collision detection:** Player↔Playfield, Missile↔Playfield, Player↔Player.
- **Clear latch:** `$D01E` (HITCLR).

### Collision Approaches

1. **Hardware-based:** Directly read GTIA collision registers.
2. **Hybrid:** Hardware class collisions + software refinement.
3. **DLI band sampling:** Sample per scanline band for multiplexed PMGs.
4. **Tile checks:** Map to ANTIC playfield cells for coarse terrain collisions.

### Example

```asm
GTIA_HITCLR = $D01E
read_gtia_colls
    lda P0PF          ; Player0 vs playfield
    sta col_p0_pf
    lda P1PF          ; Player1 vs playfield
    sta col_p1_pf
    lda P0PL          ; Player0 vs players/missiles
    sta col_p0_pl

    lda #$00          ; clear all latches
    sta GTIA_HITCLR
    rts
```

---

## Apple II

### Characteristics

- No hardware sprites or collision detection.
- Everything is software-rendered into the high-resolution framebuffer.

### Collision Approaches

1. **AABB (Axis-Aligned Bounding Box)** – fast, coarse.
2. **Swept AABB** – detects mid-frame motion collisions.
3. **Tile grid lookup** – map sprites to logical grid cells.
4. **Bitmask AND test** – per-pixel accuracy.
5. **Off-screen buffer check** – test overlaps in a shadow buffer.

### Example (Software Mask)

```asm
check_mask_rows
row_loop:
    lda (sprA_ptr),y
    and (sprB_ptr),y
    bne collision
    dey
    bpl row_loop
    rts
collision:
    rts
```

---

## Bevy Integration (Modern Host-Side)

### Purpose

In **asm465 cross/modern builds**, Bevy provides an ECS-based simulation environment for accurate and cross-platform collision testing.

### Implementation Patterns

1. **Manual AABB (Lightweight):**
   - Components: `Transform`, `Hitbox`, `SpriteId`.
   - System runs per frame to detect overlaps.
2. **Physics plugin (Rapier2D):**
   - Entities become physics colliders.
   - Built-in contact events, continuous detection, restitution, and friction.

### Example (Manual AABB System)

```rust
#[derive(Component)]
struct Hitbox { half: Vec2 }
#[derive(Component)]
struct SpriteId(u8);
#[derive(Event)]
struct CollisionEvent { a: u8, b: u8 }

fn aabb_overlap(a_pos: Vec2, a: &Hitbox, b_pos: Vec2, b: &Hitbox) -> bool {
    let dx = (a_pos.x - b_pos.x).abs();
    let dy = (a_pos.y - b_pos.y).abs();
    dx <= (a.half.x + b.half.x) && dy <= (a.half.y + b.half.y)
}

fn broadphase_and_collide(
    mut ev: EventWriter<CollisionEvent>,
    q: Query<(&Transform, &Hitbox, &SpriteId)>,
) {
    let entities: Vec<_> = q.iter().collect();
    for i in 0..entities.len() {
        let (ta, ha, ida) = entities[i];
        let pa = ta.translation.truncate();
        for j in (i + 1)..entities.len() {
            let (tb, hb, idb) = entities[j];
            let pb = tb.translation.truncate();
            if aabb_overlap(pa, ha, pb, hb) {
                ev.send(CollisionEvent { a: ida.0, b: idb.0 });
            }
        }
    }
}
```

### Bridging to 6502 Systems

| Platform     | Collision Bridge                                   |
| ------------ | -------------------------------------------------- |
| **C64**      | Write flags to `$D01E/$D01F` (VIC-II latches).     |
| **Atari**    | Emulate GTIA collision bits, trigger DLI handlers. |
| **Apple II** | Call high-level software handlers directly.        |

### Advantages

- Consistent cross-platform physics simulation.
- Deterministic testable collisions for asm465.
- Optional use of Rapier for advanced collision dynamics.

