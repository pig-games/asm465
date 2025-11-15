use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::RunnerError;

#[derive(Clone, Debug)]
pub enum CaseSource {
    File(PathBuf),
    Inline(String),
}

#[derive(Clone, Debug)]
pub struct CatalogCase {
    pub name: String,
    pub source: CaseSource,
    pub timeout_ms: Option<u64>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl CatalogCase {
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

#[derive(Clone, Debug)]
pub struct Catalog {
    cases: Vec<CatalogCase>,
}

impl Catalog {
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
        Ok(Self { cases })
    }

    pub fn cases(&self) -> &[CatalogCase] {
        &self.cases
    }
}

#[derive(Deserialize)]
struct CatalogToml {
    #[serde(rename = "case", default)]
    case: Vec<CatalogCaseToml>,
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
