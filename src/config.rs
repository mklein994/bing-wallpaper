use anyhow::anyhow;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use url::Url;

use crate::Opt;

const URL_BASE: &str = "https://www.bing.com";

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    params: UrlParams,
    project: Project,
}

impl Config {
    /// Merge the config with options passed on the command line
    pub fn initialize(opt: Opt) -> anyhow::Result<Self> {
        let project = Project::initialize()?;
        Self::initialize_with_project(opt, project)
    }

    /// Merge the config with options, using a custom project config
    fn initialize_with_project(opt: Opt, project: Project) -> anyhow::Result<Self> {
        let raw_config = if let Some(config_path) = opt.config_path.as_ref().or_else(|| {
            let default_config_path = &project.config_file_path;
            if let Ok(true) = default_config_path.try_exists() {
                Some(default_config_path)
            } else {
                None
            }
        }) {
            let contents = std::fs::read_to_string(config_path)?;
            serde_json::from_str(&contents)?
        } else {
            RawConfig::default()
        };

        Ok(Self {
            params: UrlParams {
                number: opt.number.or(raw_config.number).unwrap_or(8),
                index: opt.index.or(raw_config.index),
                market: opt.market.or(raw_config.market),
            },
            project,
        })
    }

    /// Get the URL to retrieve image metadata from
    #[must_use]
    pub fn to_url(&self) -> Url {
        self.params.to_url()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UrlParams {
    number: u8,
    index: Option<u8>,
    market: Option<String>,
}

impl UrlParams {
    pub fn to_url(&self) -> Url {
        Url::parse_with_params(&format!("{URL_BASE}/HPImageArchive.aspx"), self.params()).unwrap()
    }

    fn params(&self) -> impl Iterator<Item = (&'static str, String)> {
        vec![("format", "js".to_string()), ("n", self.number.to_string())]
            .into_iter()
            .chain(
                vec![
                    ("idx", self.index.map(|x| x.to_string())),
                    (
                        "mkt",
                        self.market.as_ref().map(std::string::ToString::to_string),
                    ),
                ]
                .into_iter()
                .filter_map(|(k, v)| v.map(|value| (k, value))),
            )
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RawConfig {
    pub number: Option<u8>,
    pub index: Option<u8>,
    pub market: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Project {
    config_file_path: PathBuf,
    data_dir: PathBuf,
    state_dir: PathBuf,
}

impl Project {
    /// Try initializing a structure to track project directories
    fn initialize() -> anyhow::Result<Self> {
        let project_dirs = ProjectDirs::from("", "", env!("CARGO_CRATE_NAME"))
            .ok_or_else(|| anyhow!("Failed to detect project directories"))?;
        Ok(Self {
            config_file_path: project_dirs.config_dir().join("config.json").clone(),
            data_dir: project_dirs.data_dir().to_path_buf(),
            state_dir: project_dirs
                .state_dir()
                .map(Path::to_path_buf)
                .ok_or_else(|| anyhow!("Failed to detect project state directory"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn with_sample_config() {
        let project = get_test_project();
        let expected = Config {
            params: UrlParams {
                number: 8,
                index: None,
                market: Some("en-CA".to_string()),
            },
            project: project.clone(),
        };

        let actual = Config::initialize_with_project(Opt::parse_from([""]), project).unwrap();

        assert_eq!(expected, actual);
        assert_eq!(
            "https://www.bing.com/HPImageArchive.aspx?format=js&n=8&mkt=en-CA",
            actual.to_url().as_str(),
        );
    }

    #[test]
    fn check_opt_overrides_to_config() {
        let project = get_test_project();
        let expected = Config {
            params: UrlParams {
                number: 1,
                index: Some(1),
                market: Some("en-CA".to_string()),
            },
            project: project.clone(),
        };

        let actual = Config::initialize_with_project(
            Opt::parse_from(vec!["", "--number", "1", "--index", "1"]),
            project,
        )
        .unwrap();

        assert_eq!(expected, actual);
        assert_eq!(
            "https://www.bing.com/HPImageArchive.aspx?format=js&n=1&idx=1&mkt=en-CA",
            actual.to_url().as_str(),
        );
    }

    fn get_test_project() -> Project {
        let test_base = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/local"));
        Project {
            config_file_path: test_base
                .join("config")
                .join(env!("CARGO_CRATE_NAME"))
                .join("config.json"),
            data_dir: test_base.join("share").join(env!("CARGO_CRATE_NAME")),
            state_dir: test_base.join("state").join(env!("CARGO_CRATE_NAME")),
        }
    }
}
