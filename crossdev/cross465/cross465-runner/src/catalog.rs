//! Catalog parsing utilities for the Cross465 test runner.
//!
//! Turns TOML manifests into strongly typed case definitions, with optional
//! deduplication and adhoc (inline) sources.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{RunnerError, TargetKind};

/// Where a catalog case gets its assembly source.
#[derive(Clone, Debug)]
pub enum CaseSource {
    File(PathBuf),
    Inline(String),
}

/// A single catalog entry describing how to assemble/run a case.
#[derive(Clone, Debug)]
pub struct CatalogCase {
    pub name: String,
    pub source: CaseSource,
    pub timeout_ms: Option<u64>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl CatalogCase {
    /// Construct an adhoc case (inline snippet) for development overrides.
    pub fn adhoc(name: String, source: CaseSource, timeout_ms: Option<u64>) -> Self {
        Self {
            name,
            source,
            timeout_ms,
            description: None,
            tags: Vec::new(),
        }
    }

    pub fn source(&self) -> &CaseSource {
        &self.source
    }
}

/// Parsed catalog consisting of deduplicated cases.
#[derive(Clone, Debug)]
pub struct Catalog {
    cases: Vec<CatalogCase>,
    ci_matrix: Vec<CiMatrixEntry>,
    workspace_override: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct CiMatrixEntry {
    pub target: TargetKind,
    pub personalities: Vec<Option<String>>,
    pub includes: Vec<PathBuf>,
    pub defines: Vec<(String, String)>,
    pub tass_args: Vec<String>,
    pub endpoint: Option<CiEndpoint>,
}

#[derive(Clone, Debug)]
pub struct CiEndpoint {
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl Catalog {
    /// Load and validate a catalog from TOML.
    pub fn load(manifest_path: &Path) -> Result<Self, RunnerError> {
        if !manifest_path.exists() {
            return Err(RunnerError::CatalogMissing {
                path: manifest_path.to_path_buf(),
            });
        }
        let data = fs::read_to_string(manifest_path).map_err(|_| RunnerError::CatalogMissing {
            path: manifest_path.to_path_buf(),
        })?;
        let parsed: CatalogToml =
            toml::from_str(&data).map_err(|source| RunnerError::CatalogParse {
                path: manifest_path.to_path_buf(),
                source,
            })?;
        let mut cases = Vec::new();
        let mut seen = HashSet::new();
        let base_dir = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        for entry in parsed.case.iter() {
            if !seen.insert(entry.name.clone()) {
                continue;
            }
            let path = base_dir.join(&entry.path);
            if !path.exists() {
                return Err(RunnerError::AssemblyMissing { path });
            }
            cases.push(CatalogCase {
                name: entry.name.clone(),
                source: CaseSource::File(path),
                timeout_ms: entry.timeout_ms,
                description: entry.description.clone(),
                tags: entry.tags.clone().unwrap_or_default(),
            });
        }
        let mut ci_matrix = Vec::new();
        if let Some(ci) = parsed.ci {
            for (target, entry) in ci.matrix {
                let parsed_target = target.parse().map_err(|_| RunnerError::CatalogInvalid {
                    message: format!("unknown target '{}' in ci.matrix", target),
                })?;
                let personalities = if entry.personalities.is_empty() {
                    vec![None]
                } else {
                    entry
                        .personalities
                        .into_iter()
                        .map(|p| if p.trim().is_empty() { None } else { Some(p) })
                        .collect()
                };
                let defines = entry
                    .define
                    .into_iter()
                    .map(|(k, v)| (k, v))
                    .collect::<Vec<_>>();
                let endpoint = entry.endpoint.map(|ep| CiEndpoint {
                    host: ep.host,
                    port: ep.port,
                });
                ci_matrix.push(CiMatrixEntry {
                    target: parsed_target,
                    personalities,
                    includes: entry.include,
                    defines,
                    tass_args: entry.tass_args,
                    endpoint,
                });
            }
        }
        let workspace_override = parsed.workspace.as_ref().map(|ws| {
            if ws.is_absolute() {
                ws.clone()
            } else {
                base_dir.join(ws)
            }
        });
        Ok(Self {
            cases,
            ci_matrix,
            workspace_override,
        })
    }

    /// Read-only view of catalog cases in load order.
    pub fn cases(&self) -> &[CatalogCase] {
        &self.cases
    }

    pub fn ci_matrix(&self) -> &[CiMatrixEntry] {
        &self.ci_matrix
    }

    pub fn workspace_override(&self) -> Option<&PathBuf> {
        self.workspace_override.as_ref()
    }
}

#[derive(Deserialize)]
struct CatalogToml {
    #[serde(rename = "case", default)]
    case: Vec<CatalogCaseToml>,
    #[serde(default)]
    ci: Option<CiToml>,
    #[serde(default)]
    workspace: Option<PathBuf>,
}

#[derive(Deserialize)]
struct CatalogCaseToml {
    name: String,
    path: PathBuf,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct CiToml {
    #[serde(default)]
    matrix: BTreeMap<String, CiMatrixEntryToml>,
}

#[derive(Deserialize)]
struct CiMatrixEntryToml {
    #[serde(default)]
    personalities: Vec<String>,
    #[serde(default = "Vec::new")]
    include: Vec<PathBuf>,
    #[serde(default)]
    define: BTreeMap<String, String>,
    #[serde(default)]
    tass_args: Vec<String>,
    #[serde(default)]
    endpoint: Option<CiEndpointToml>,
}

#[derive(Deserialize)]
struct CiEndpointToml {
    host: Option<String>,
    port: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    struct TempCatalog {
        _dir: TempDir,
        path: PathBuf,
    }

    impl TempCatalog {
        fn new(extra: &str) -> Self {
            Self::with_prefix("", extra)
        }

        fn with_prefix(prefix: &str, suffix: &str) -> Self {
            let dir = TempDir::new().expect("tempdir");
            fs::write(dir.path().join("case.s"), ".byte 0\n").expect("write case");
            let mut manifest = String::new();
            if !prefix.is_empty() {
                manifest.push_str(prefix);
                if !prefix.ends_with('\n') {
                    manifest.push('\n');
                }
                manifest.push('\n');
            }
            manifest.push_str("[[case]]\nname = \"demo::case\"\npath = \"case.s\"\n");
            manifest.push_str(suffix);
            manifest.push('\n');
            let path = dir.path().join("catalog.toml");
            fs::write(&path, manifest).expect("write manifest");
            Self { _dir: dir, path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    #[test]
    fn catalog_without_ci_has_empty_matrix() {
        let tmp = TempCatalog::new("");
        let catalog = Catalog::load(tmp.path()).expect("load");
        assert!(catalog.ci_matrix().is_empty());
    }

    #[test]
    fn catalog_parses_ci_matrix() {
        let extra = r#"[ci.matrix.cross465]
personalities = ["modern-retro", "c64-compat"]
"#;
        let tmp = TempCatalog::new(extra);
        let catalog = Catalog::load(tmp.path()).expect("load");
        assert_eq!(catalog.ci_matrix().len(), 1);
        let entry = &catalog.ci_matrix()[0];
        assert_eq!(entry.target, TargetKind::Cross465);
        assert_eq!(
            entry.personalities,
            vec![
                Some("modern-retro".to_string()),
                Some("c64-compat".to_string())
            ]
        );
    }

    #[test]
    fn catalog_allows_empty_personality_list() {
        let extra = r#"[ci.matrix.ultimate64]
personalities = []
"#;
        let tmp = TempCatalog::new(extra);
        let catalog = Catalog::load(tmp.path()).expect("load");
        assert_eq!(catalog.ci_matrix().len(), 1);
        let entry = &catalog.ci_matrix()[0];
        assert_eq!(entry.target, TargetKind::Ultimate64);
        assert_eq!(entry.personalities, vec![None]);
    }

    #[test]
    fn catalog_parses_entry_options() {
        let extra = r#"
[ci.matrix.ultimate64]
personalities = []
include = ["native/src/include"]
tass_args = ["-DFAST"]

[ci.matrix.ultimate64.define]
FEATURE = "1"

[ci.matrix.ultimate64.endpoint]
host = "192.168.0.64"
port = 6510
"#;
        let tmp = TempCatalog::new(extra);
        let catalog = Catalog::load(tmp.path()).expect("load");
        let entry = &catalog.ci_matrix()[0];
        assert_eq!(entry.includes, vec![PathBuf::from("native/src/include")]);
        assert_eq!(
            entry.defines,
            vec![("FEATURE".to_string(), "1".to_string())]
        );
        assert_eq!(entry.tass_args, vec!["-DFAST".to_string()]);
        let endpoint = entry.endpoint.as_ref().expect("endpoint");
        assert_eq!(endpoint.host.as_deref(), Some("192.168.0.64"));
        assert_eq!(endpoint.port, Some(6510));
    }

    #[test]
    fn catalog_reports_workspace_override() {
        let prefix = r#"workspace = "../../..""#;
        let tmp = TempCatalog::with_prefix(prefix, "");
        let catalog = Catalog::load(tmp.path()).expect("load");
        let base = tmp.path().parent().unwrap();
        let expected = base.join("../../..");
        let actual = catalog.workspace_override().expect("workspace override");
        assert_eq!(actual, &expected);
    }

    #[test]
    fn catalog_rejects_unknown_target() {
        let extra = r#"[ci.matrix.unknown]
personalities = ["modern-retro"]
"#;
        let tmp = TempCatalog::new(extra);
        let err = Catalog::load(tmp.path()).unwrap_err();
        matches!(err, RunnerError::CatalogInvalid { .. });
    }
}
