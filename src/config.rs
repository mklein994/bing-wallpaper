use anyhow::anyhow;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use url::Url;

use crate::{
    opt::{Extension, Resolution},
    Opt,
};
#[cfg(test)]
pub use tests::get_test_project;

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub raw: Raw,
    params: UrlParams,
    pub project: Project,
    pub size: Resolution,
    pub ext: Extension,
}

impl Config {
    pub(crate) fn new(opt: &Opt, project: Project, raw_config: Raw) -> Self {
        let number = opt.number.or(raw_config.number).unwrap_or(8);
        let index = opt.index.or(raw_config.index);
        let market = opt
            .market
            .as_deref()
            .or(raw_config.market.as_deref())
            .filter(|x| !x.is_empty())
            .map(std::string::ToString::to_string);

        let size = opt.size.or(raw_config.size).unwrap_or_default();
        let ext = opt.ext.or(raw_config.ext).unwrap_or_default();

        Self {
            raw: raw_config,
            params: UrlParams {
                number,
                index,
                market,
            },
            project,
            size,
            ext,
        }
    }

    /// Get the URL to retrieve image metadata from
    #[must_use]
    pub fn to_url(&self) -> Url {
        self.params.to_url()
    }

    #[must_use]
    pub fn index(&self) -> Option<u8> {
        self.params.index
    }

    #[must_use]
    pub fn number(&self) -> u8 {
        self.params.number
    }

    #[must_use]
    pub fn market(&self) -> Option<String> {
        self.params.market.clone()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UrlParams {
    number: u8,
    index: Option<u8>,
    market: Option<String>,
}

impl UrlParams {
    #[must_use]
    pub fn to_url(&self) -> Url {
        Url::parse_with_params(
            &format!("{}/HPImageArchive.aspx", crate::URL_BASE),
            self.params(),
        )
        .unwrap()
    }

    fn params(&self) -> impl Iterator<Item = (&'static str, String)> {
        vec![("format", "js".to_string()), ("n", self.number.to_string())]
            .into_iter()
            .chain(
                vec![
                    ("idx", self.index.map(|x| x.to_string())),
                    ("mkt", self.market.clone()),
                ]
                .into_iter()
                .filter_map(|(k, v)| v.map(|value| (k, value))),
            )
    }
}

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct Raw {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Resolution>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<Extension>,
}

impl Raw {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let raw_config = serde_json::from_str(&contents)?;
        Ok(raw_config)
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub struct Project {
    pub config_file_path: PathBuf,
    pub data_dir: PathBuf,
    pub state_file_path: PathBuf,
}

impl Project {
    /// Try initializing a structure to track project directories
    pub(crate) fn initialize(opt: &Opt) -> anyhow::Result<Self> {
        let project_dirs = ProjectDirs::from("", "", env!("CARGO_CRATE_NAME"))
            .ok_or_else(|| anyhow!("Failed to detect project directories"))?;

        Ok(Self {
            config_file_path: if let Some(path) = &opt.config_path {
                path.clone()
            } else {
                project_dirs.config_dir().join("config.json")
            },
            data_dir: if let Some(path) = &opt.data_path {
                path.clone()
            } else {
                project_dirs.data_dir().to_path_buf()
            },
            state_file_path: if let Some(path) = &opt.state_path {
                path.clone()
            } else {
                project_dirs
                    .state_dir()
                    .map(|x| x.join("image_index.json"))
                    .ok_or_else(|| anyhow!("Failed to detect project state directory"))?
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[must_use]
    pub fn get_test_project() -> Project {
        let test_base = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/local"));
        Project {
            config_file_path: test_base
                .join("config")
                .join(env!("CARGO_CRATE_NAME"))
                .join("config.json"),
            data_dir: test_base.join("share").join(env!("CARGO_CRATE_NAME")),
            state_file_path: test_base
                .join("state")
                .join(env!("CARGO_CRATE_NAME"))
                .join("image_index.json"),
        }
    }

    #[test]
    fn with_sample_config() {
        let project = get_test_project();
        let expected = Config {
            raw: Raw {
                market: Some("en-CA".to_string()),
                ..Raw::default()
            },
            params: UrlParams {
                number: 8,
                index: None,
                market: Some("en-CA".to_string()),
            },
            project: project.clone(),
            size: Resolution::default(),
            ext: Extension::default(),
        };

        let actual = Opt::parse_from([""])
            .get_config_with_project(project)
            .unwrap();

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
            raw: Raw {
                market: Some("en-CA".to_string()),
                ..Raw::default()
            },
            params: UrlParams {
                number: 1,
                index: Some(1),
                market: Some("en-CA".to_string()),
            },
            project: project.clone(),
            size: Resolution::default(),
            ext: Extension::default(),
        };

        let actual = Opt::parse_from(vec!["", "--number", "1", "--index", "1"])
            .get_config_with_project(project)
            .unwrap();

        assert_eq!(expected, actual);
        assert_eq!(
            "https://www.bing.com/HPImageArchive.aspx?format=js&n=1&idx=1&mkt=en-CA",
            actual.to_url().as_str(),
        );
    }
}
