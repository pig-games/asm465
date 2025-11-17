use crate::catalog::CaseSource;
use crate::{run_cases, CaseFilter, CaseReport, RunOptions, RunnerConfig, RunnerError, TargetKind};
use std::env;
use std::path::PathBuf;

#[derive(Clone)]
enum AsmSource {
    Inline(String),
    Path(PathBuf),
}

pub struct AsmTestBuilder {
    name: Option<String>,
    personality: Option<String>,
    target: Option<String>,
    timeout_ms: Option<u64>,
    seed: u64,
    asm: Option<AsmSource>,
    workspace: Option<PathBuf>,
    include_paths: Vec<PathBuf>,
    fixture_dir: Option<PathBuf>,
    update_fixtures: bool,
    defines: Vec<(String, String)>,
    tass_args: Vec<String>,
}

impl AsmTestBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            personality: None,
            target: None,
            timeout_ms: None,
            seed: 0xDEADBEEF,
            asm: None,
            workspace: None,
            include_paths: Vec::new(),
            fixture_dir: None,
            update_fixtures: false,
            defines: Vec::new(),
            tass_args: Vec::new(),
        }
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn personality(&mut self, personality: impl Into<String>) -> &mut Self {
        self.personality = Some(personality.into());
        self
    }

    pub fn target(&mut self, target: impl Into<String>) -> &mut Self {
        self.target = Some(target.into());
        self
    }

    pub fn timeout_ms(&mut self, timeout: u64) -> &mut Self {
        self.timeout_ms = Some(timeout);
        self
    }

    pub fn seed(&mut self, seed: u64) -> &mut Self {
        self.seed = seed;
        self
    }

    pub fn asm(&mut self, source: impl Into<String>) -> &mut Self {
        self.asm = Some(AsmSource::Inline(source.into()));
        self
    }

    pub fn asm_path(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.asm = Some(AsmSource::Path(path.into()));
        self
    }

    pub fn workspace(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.workspace = Some(path.into());
        self
    }

    pub fn include(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.include_paths.push(path.into());
        self
    }

    pub fn fixtures(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.fixture_dir = Some(path.into());
        self
    }

    pub fn update_fixtures(&mut self, flag: bool) -> &mut Self {
        self.update_fixtures = flag;
        self
    }

    pub fn define(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.defines.push((key.into(), value.into()));
        self
    }

    pub fn tass_arg(&mut self, arg: impl Into<String>) -> &mut Self {
        self.tass_args.push(arg.into());
        self
    }

    pub fn run(self) -> Result<AsmTestResult, RunnerError> {
        run_asm6502_case(self)
    }
}

pub struct AsmTestResult {
    case: CaseReport,
}

impl AsmTestResult {
    pub fn case(&self) -> &CaseReport {
        &self.case
    }

    pub fn into_case(self) -> CaseReport {
        self.case
    }
}

pub fn run_asm6502_case(builder: AsmTestBuilder) -> Result<AsmTestResult, RunnerError> {
    let name = builder.name.ok_or_else(|| RunnerError::InvalidInvocation {
        message: "asm6502_test! requires `name`".to_string(),
    })?;
    let personality = builder
        .personality
        .unwrap_or_else(|| "modern-retro".to_string());
    let target = builder
        .target
        .ok_or_else(|| RunnerError::InvalidInvocation {
            message: "asm6502_test! requires `target`".to_string(),
        })
        .and_then(|value| parse_target(&value))?;
    let asm = builder.asm.ok_or_else(|| RunnerError::InvalidInvocation {
        message: "asm6502_test! requires either `asm` or `asm_path`".to_string(),
    })?;
    let workspace = builder.workspace.unwrap_or_else(default_workspace_root);

    let case_source = match asm {
        AsmSource::Inline(src) => CaseSource::Inline(src),
        AsmSource::Path(path) => {
            let absolute = if path.is_absolute() {
                path
            } else {
                workspace.join(path)
            };
            CaseSource::File(absolute)
        }
    };

    // ensure inline source includes trailing newline? not required.
    // Determine include paths relative to workspace.
    let include_paths: Vec<PathBuf> = builder
        .include_paths
        .into_iter()
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace.join(p)
            }
        })
        .collect();

    let fixture_dir = builder.fixture_dir.map(|dir| {
        if dir.is_absolute() {
            dir
        } else {
            workspace.join(dir)
        }
    });

    let mut run_opts = RunOptions::default();
    run_opts.target = target;
    run_opts.personality = personality;
    run_opts.timeout_ms = builder.timeout_ms;
    run_opts.seed = builder.seed;
    run_opts.asm_override = Some(case_source);
    run_opts.extra_includes = include_paths;
    run_opts.fixture_dir = fixture_dir;
    run_opts.update_fixtures = builder.update_fixtures;
    run_opts.extra_defines = builder.defines.clone();
    run_opts.tass_args = builder.tass_args.clone();

    let config = RunnerConfig::new(workspace.clone(), None);
    let filter = CaseFilter {
        names: vec![name.clone()],
    };
    let report = run_cases(&config, &filter, &run_opts)?;
    let mut fallback = None;
    for case in report.cases.into_iter() {
        if case.name == name {
            return Ok(AsmTestResult { case });
        }
        if fallback.is_none() {
            fallback = Some(case);
        }
    }
    let case = fallback.ok_or_else(|| RunnerError::InvalidInvocation {
        message: "no case produced by asm6502_test invocation".to_string(),
    })?;

    Ok(AsmTestResult { case })
}

fn parse_target(value: &str) -> Result<TargetKind, RunnerError> {
    value.parse().map_err(|_| RunnerError::InvalidInvocation {
        message: format!("unknown target '{value}'"),
    })
}

fn default_workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .join("../../..")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../../.."))
}

#[macro_export]
macro_rules! asm6502_test {
    ( $( $field:ident = $value:tt ),+ $(,)? ) => {{
        let mut builder = $crate::AsmTestBuilder::new();
        $(
            $crate::__asm6502_option!(builder, $field, $value);
        )+
        builder.run()
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __asm6502_option {
    ($builder:expr, name, $value:tt) => { $builder.name($value); };
    ($builder:expr, personality, $value:tt) => { $builder.personality($value); };
    ($builder:expr, target, $value:tt) => { $builder.target($value); };
    ($builder:expr, timeout_ms, $value:tt) => { $builder.timeout_ms($value); };
    ($builder:expr, seed, $value:tt) => { $builder.seed($value); };
    ($builder:expr, asm, $value:tt) => { $builder.asm($value); };
    ($builder:expr, asm_path, $value:tt) => { $builder.asm_path($value); };
    ($builder:expr, workspace, $value:tt) => { $builder.workspace($value); };
    ($builder:expr, fixtures, $value:tt) => { $builder.fixtures($value); };
    ($builder:expr, update_fixtures, $value:tt) => { $builder.update_fixtures($value); };
    ($builder:expr, include, [$($path:tt),* $(,)?]) => {
        $( $builder.include($path); )*
    };
    ($builder:expr, include, $value:tt) => {
        $builder.include($value);
    };
    ($builder:expr, defines, [$(($key:tt, $value:tt)),* $(,)?]) => {
        $( $builder.define($key, $value); )*
    };
    ($builder:expr, defines, [$($key:tt => $value:tt),* $(,)?]) => {
        $( $builder.define($key, $value); )*
    };
    ($builder:expr, tass_args, [$($value:tt),* $(,)?]) => {
        $( $builder.tass_arg($value); )*
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CaseStatus;
    use std::process::Command;
    use tempfile::TempDir;

    fn have_64tass() -> bool {
        Command::new("64tass").arg("--version").output().is_ok()
    }

    #[test]
    fn builder_runs_inline_case() {
        if !have_64tass() {
            eprintln!("skipping asm6502_test builder test (64tass not found)");
            return;
        }
        let mut builder = AsmTestBuilder::new();
        builder
            .name("math::inline_builder")
            .target("cross465")
            .personality("modern-retro")
            .timeout_ms(2_000)
            .asm(
                r#"
rtst.BASE = $C000

.if TARGET_ULTIMATE64
.include "platformmacros.h"
* = $0801
    BasicUpstart(start)
.endif

* = $2000
    jmp start
.include "test_rtst.h"
start:
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
    lda #2
    clc
    adc #3
    cmp #5
    bne fail
    .ctest.OK 0, OK_MSG
    jmp done
fail:
    .ctest.FAIL 1, FAIL_MSG
done:
    .rtst.end

CASE_NAME: .null "math::inline_builder"
OK_MSG: .null "ok"
FAIL_MSG: .null "fail"
"#,
            );
        let result = builder.run().expect("run asm case");
        assert!(matches!(result.case().status, CaseStatus::Passed));
    }

    #[test]
    fn macro_expands_successfully() {
        if !have_64tass() {
            eprintln!("skipping asm6502_test macro test (64tass not found)");
            return;
        }
        let result = asm6502_test!(
            name = "math::inline_macro",
            target = "cross465",
            personality = "modern-retro",
            timeout_ms = 2_000,
            asm = r#"
rtst.BASE = $C000
INLINE_SWITCH :?= 0

.if TARGET_ULTIMATE64
.include "platformmacros.h"
* = $0801
    BasicUpstart(start)
.endif

* = $2000
    jmp start
.include "test_rtst.h"
start:
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
    lda #1
    clc
    adc #1
    cmp #2
    bne fail
    .ctest.OK 0, OK_MSG
    jmp done
fail:
    .ctest.FAIL 1, FAIL_MSG
done:
    .rtst.end

CASE_NAME: .null "math::inline_macro"
OK_MSG: .null "macro ok"
FAIL_MSG: .null "macro fail"
"#
        )
        .expect("macro run");
        assert!(matches!(result.case().status, CaseStatus::Passed));
    }

    #[test]
    fn macro_can_pass_defines() {
        if !have_64tass() {
            eprintln!("skipping asm6502_test macro define test (64tass not found)");
            return;
        }
        let result = asm6502_test!(
            name = "math::define_macro",
            target = "cross465",
            personality = "modern-retro",
            timeout_ms = 2_000,
            include = ["native/src/include", "native/src/platform/cross465/include"],
            defines = [("INLINE_SWITCH", "1")],
            asm = r#"
rtst.BASE = $C000
INLINE_SWITCH :?= 0

.if TARGET_ULTIMATE64
.include "platformmacros.h"
* = $0801
    BasicUpstart(start)
.endif

* = $2000
    jmp start
.include "test_rtst.h"
start:
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
.if INLINE_SWITCH
    .ctest.OK 0, OK_MSG
.else
    .ctest.FAIL 1, FAIL_MSG
.endif
    .rtst.end

CASE_NAME: .null "math::define_macro"
OK_MSG: .null "defines ok"
FAIL_MSG: .null "defines fail"
"#
        )
        .expect("macro defines run");
        assert!(matches!(result.case().status, CaseStatus::Passed));
    }

    #[test]
    fn builder_runs_file_case_with_defines() {
        if !have_64tass() {
            eprintln!("skipping asm6502_test file test (64tass not found)");
            return;
        }
        let mut builder = AsmTestBuilder::new();
        builder
            .name("math::add_basic")
            .target("cross465")
            .personality("modern-retro")
            .asm_path("crossdev/cross465/tests/cases/math_add_basic.s")
            .define("CUSTOM_CONST", "5");
        let result = builder.run().expect("run file case");
        assert!(matches!(result.case().status, CaseStatus::Passed));
    }

    #[test]
    fn builder_respects_fixture_flags() {
        if !have_64tass() {
            eprintln!("skipping asm6502_test fixture test (64tass not found)");
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let fixture_root = temp.path().join("fixtures");

        // Run once to generate fixtures.
        let mut builder = AsmTestBuilder::new();
        builder
            .name("math::add_basic")
            .target("cross465")
            .personality("modern-retro")
            .asm_path("crossdev/cross465/tests/cases/math_add_basic.s")
            .fixtures(fixture_root.clone())
            .update_fixtures(true);
        builder.run().expect("generate fixture");

        let expected = fixture_root
            .join("cross465")
            .join("math")
            .join("add_basic.json");
        assert!(
            expected.exists(),
            "expected fixture to be written to {}",
            expected.display()
        );

        // Run again without update to ensure fixture is consumed.
        let mut builder = AsmTestBuilder::new();
        builder
            .name("math::add_basic")
            .target("cross465")
            .personality("modern-retro")
            .asm_path("crossdev/cross465/tests/cases/math_add_basic.s")
            .fixtures(fixture_root)
            .update_fixtures(false);
        let result = builder.run().expect("run with fixtures");
        assert!(matches!(result.case().status, CaseStatus::Passed));
    }
}
