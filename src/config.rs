use anyhow::anyhow;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

use crate::Opt;

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    params: UrlParams,
    pub project: Project,
    pub size: String,
    pub ext: String,
}

impl Config {
    /// Merge the config with options passed on the command line
    pub fn initialize(opt: &Opt) -> anyhow::Result<Self> {
        let project = Project::initialize()?;
        Self::initialize_with_project(opt, project)
    }

    /// Merge the config with options, using a custom project config
    fn initialize_with_project(opt: &Opt, project: Project) -> anyhow::Result<Self> {
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
            Raw::default()
        };

        Ok(Self {
            params: UrlParams {
                number: opt.number.or(raw_config.number).unwrap_or(8),
                index: opt.index.or(raw_config.index),
                market: opt
                    .market
                    .as_deref()
                    .or(raw_config.market.as_deref())
                    .filter(|x| !x.is_empty())
                    .map(std::string::ToString::to_string),
            },
            project,
            size: opt
                .size
                .as_deref()
                .or(raw_config.size.as_deref())
                .unwrap_or("UHD")
                .to_string(),
            ext: opt
                .ext
                .as_deref()
                .or(raw_config.size.as_deref())
                .unwrap_or("jpg")
                .to_string(),
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

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Raw {
    pub number: Option<u8>,
    pub index: Option<u8>,
    pub market: Option<String>,
    pub size: Option<String>,
    pub ext: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub struct Project {
    config_file_path: PathBuf,
    pub data_dir: PathBuf,
    pub state_file_path: PathBuf,
}

impl Project {
    /// Try initializing a structure to track project directories
    fn initialize() -> anyhow::Result<Self> {
        let project_dirs = ProjectDirs::from("", "", env!("CARGO_CRATE_NAME"))
            .ok_or_else(|| anyhow!("Failed to detect project directories"))?;
        Ok(Self {
            config_file_path: project_dirs.config_dir().join("config.json").clone(),
            data_dir: project_dirs.data_dir().to_path_buf(),
            state_file_path: project_dirs
                .state_dir()
                .map(|x| x.join("image_index.json"))
                .ok_or_else(|| anyhow!("Failed to detect project state directory"))?,
        })
    }

    #[cfg(test)]
    #[must_use]
    pub fn new(config_file_path: PathBuf, data_dir: PathBuf, state_file_path: PathBuf) -> Self {
        Self {
            config_file_path,
            data_dir,
            state_file_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::tests::get_test_project;

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
            size: "UHD".to_string(),
            ext: "jpg".to_string(),
        };

        let actual = Config::initialize_with_project(&Opt::parse_from([""]), project).unwrap();

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
            size: "UHD".to_string(),
            ext: "jpg".to_string(),
        };

        let actual = Config::initialize_with_project(
            &Opt::parse_from(vec!["", "--number", "1", "--index", "1"]),
            project,
        )
        .unwrap();

        assert_eq!(expected, actual);
        assert_eq!(
            "https://www.bing.com/HPImageArchive.aspx?format=js&n=1&idx=1&mkt=en-CA",
            actual.to_url().as_str(),
        );
    }
}
