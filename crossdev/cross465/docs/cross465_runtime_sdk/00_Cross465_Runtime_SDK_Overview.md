# Cross465 Runtime SDK — Overview (00)
Version: 1.0-draft • Target: desktop & WASM • Audience: developers and integrators

---

## 1. Scope

The **Cross465 Runtime SDK** defines the complete framework for building, packaging, and extending 6502-based runtimes and modernized derivatives.  
It provides a unified development and distribution toolchain centered around **Cross465** and **asm465**.

The SDK covers:
- Module creation (Display, Audio, Input, System, Storage).
- Personality definition and static code generation.
- Runtime packaging for desktop and WASM targets.
- Developer tooling for dynamic loading and debugging.

---

## 2. Core Components

| Component | Description |
|------------|--------------|
| **cross465-core** | Framework providing CPU, MMIO, Services, and runtime loop. |
| **cross465-pack** | Packager tool transforming TOML personalities into self-contained binaries. |
| **cross465-gen** | Personality code generator (TOML → Rust macro). |
| **asm465** | Assembler/editor and developer front-end. |
| **asm465-dev** | Developer runtime with dynamic module loading and hot-reload support. |
| **RetroModern2D/3D** | Example modules implementing the Display slot. |

---

## 3. Development Workflow

```
     ┌────────────┐
     │  asm465    │  ← edit 6502 source, run/test
     └──────┬─────┘
            │ generates
            ▼
   personality.toml  +  assets/
            │
            ▼
      cross465-pack
            │  (uses cross465-gen)
            ▼
     generated Rust (personality_gen.rs)
            │
            ▼
       cargo build → runtime binary (.exe / .wasm)
```

**Desktop workflow**
```
asm465-dev  → dynamic module load (for iteration)
cross465-pack → static runtime (for release)
```

**WASM workflow**
```
cross465-pack --wasm  → single .wasm + JS loader
```

---

## 4. Versioning

| SDK Version | Status | Focus |
|-------------:|:--------|:-------|
| **1.0** | Draft | Core traits, Packager, Codegen, Dev Loader. |
| **1.1** | Planned | Extended timing model, asset pipeline, plugin SDK. |
| **1.2** | Planned | Runtime scripting API (Rust/JS bindings). |

---

## 5. Document Map

| # | Document | Description |
|--:|:----------|:-------------|
| **00** | [Overview](00_Cross465_Runtime_SDK_Overview.md) | This document. |
| **01** | [Module SDK Spec](01_Cross465_Runtime_SDK_Module_SDK_Spec.md) | Defines traits, lifecycle, C‑ABI shim. |
| **02** | [Packager Tool Spec](02_Cross465_Runtime_SDK_Packager_Tool_Spec.md) | CLI, manifest, workflow, outputs. |
| **03** | [Personality Codegen Spec](03_Cross465_Runtime_SDK_Personality_Codegen_Spec.md) | TOML→macro, validation, IR mapping. |
| **04** | [Display Module Scaffold (RetroModern2D)](04_Cross465_Runtime_SDK_Display_Module_Scaffold_RetroModern2D.md) | Example implementation. |
| **05** | [Runtime Template Spec](05_Cross465_Runtime_SDK_Runtime_Template_Spec.md) | Static runtime skeleton. |
| **06** | [asm465 Dev Loader Spec](06_Cross465_Runtime_SDK_asm465_Dev_Loader_Spec.md) | Dynamic loading & hot‑reload. |

---

## 6. Design Principles
- **Determinism:** static builds behave identically on all platforms.
- **Modularity:** each runtime module is replaceable; interfaces are minimal.
- **Security:** no dynamic code in releases; WASM safe by design.
- **Transparency:** TOML schemas and codegen reproducible from source.

---

## 7. Roadmap

| Milestone | Goal |
|------------|------|
| **M1** | Stabilize SDK 1.0, ship RetroModern2D, first open personality. |
| **M2** | Introduce extended asset pipeline, Bevy integration (optional). |
| **M3** | Add scripting and editor integration APIs. |
| **M4** | Release SDK 1.2 and documentation site. |

---

*Part of the Cross465 Runtime SDK documentation series.*
