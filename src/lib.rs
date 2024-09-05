pub mod config;
mod jiff_serde;
pub mod opt;

use std::io::prelude::*;
use std::path::PathBuf;
use std::{collections::BTreeSet, fs::File};

use anyhow::anyhow;
use jiff::Zoned;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

pub use config::Config;
use config::Project;
#[cfg(doc)]
pub use config::Raw as RawConfig;
use opt::Cmd;
pub use opt::Opt;

const URL_BASE: &str = "https://www.bing.com";

pub async fn run(opt: Opt) -> anyhow::Result<()> {
    let config = Config::initialize(&opt)?;

    if let Some(cmd) = opt.cmd {
        match cmd {
            Cmd::Metadata { print, raw } => {
                let url = config.to_url();
                if print {
                    println!("{url}");
                } else if raw {
                    let json = reqwest::get(url).await?.json::<serde_json::Value>().await?;
                    print_json(&json)?;
                } else {
                    let json = reqwest::get(url).await?.json::<ImageData>().await?;
                    print_json(&json)?;
                }
            }

            Cmd::ProjectDirs => {
                print_json(&config.project)?;
            }

            Cmd::ListImages => {
                let state = get_local_state(&config)?;
                for image in state.images {
                    println!(
                        "{}\t{}",
                        image.file_name(&config).display(),
                        image.to_url(&config),
                    );
                }
            }
        }
    } else {
        ensure_project_dirs_exist(&config.project)?;

        let mut state = get_local_state(&config)?;

        let client = Client::new();

        let new_state = get_new_state(&config, &client).await?;

        for image in new_state.images {
            let image_path = config.project.data_dir.join(image.file_name(&config));
            if !image_path.try_exists()? {
                download_image(&config, &client, &image).await?;
            }

            if !state.images.contains(&image) {
                state.add_image(image);
            }
        }

        let contents = serde_json::to_string_pretty(&state)?;
        std::fs::write(&config.project.state_file_path, contents)?;
    };

    Ok(())
}

fn get_local_state(config: &Config) -> anyhow::Result<ImageData> {
    let path = &config.project.state_file_path;
    if path.exists() {
        let contents = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&contents)?)
    } else {
        Ok(ImageData::default())
    }
}

async fn get_new_state(config: &Config, client: &Client) -> anyhow::Result<ImageData> {
    Ok(client.get(config.to_url()).send().await?.json().await?)
}

async fn download_image(config: &Config, client: &Client, image: &Image) -> anyhow::Result<()> {
    let url = image.to_url(config);
    let file_name = image.file_name(config);
    let contents = client.get(url).send().await?.bytes().await?;
    if let Some(mut file) = match File::create_new(file_name) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(err) => Err(err),
    }? {
        file.write_all(&contents)?;
    }

    Ok(())
}

/// Simple helper function to pretty-print a value as JSON to stdout
fn print_json(value: impl serde::Serialize) -> anyhow::Result<()> {
    let contents = serde_json::to_string_pretty(&value)?;
    println!("{contents}");
    Ok(())
}

fn ensure_project_dirs_exist(project: &Project) -> anyhow::Result<()> {
    if !project.data_dir.try_exists()? {
        std::fs::create_dir(&project.data_dir)?;
    }

    let state_dir = project
        .state_file_path
        .parent()
        .ok_or_else(|| anyhow!("The state file path is not inside a directory"))?;
    if !state_dir.try_exists()? {
        std::fs::create_dir(state_dir)?;
    }

    Ok(())
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ImageData {
    images: BTreeSet<Image>,
}

impl ImageData {
    pub fn add_image(&mut self, image: Image) {
        self.images.insert(image);
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct Image {
    #[serde(rename = "fullstartdate", with = "jiff_serde::datetime")]
    full_start_date: Zoned,

    #[serde(rename = "enddate", with = "jiff_serde::date")]
    end_date: Zoned,

    #[serde(rename = "hsh")]
    hash: String,

    title: String,

    url: String,

    #[serde(rename = "urlbase")]
    url_base: String,

    copyright: String,

    #[serde(rename = "copyrightlink")]
    copyright_link: String,
}

impl Image {
    pub fn to_url(&self, config: &Config) -> Url {
        Url::parse(&format!(
            "{URL_BASE}{}_{}.{}",
            self.url_base, &config.size, &config.ext
        ))
        .unwrap()
    }

    pub fn file_name(&self, config: &Config) -> PathBuf {
        let url = self.to_url(config);
        url.query_pairs()
            .find_map(|(k, v)| {
                if k == "id" {
                    Some(config.project.data_dir.join(format!("{}_{v}", self.hash)))
                } else {
                    None
                }
            })
            .unwrap()
    }
}

impl std::hash::Hash for Image {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
        self.title.hash(state);
        self.url.hash(state);
        self.url_base.hash(state);
        self.copyright.hash(state);
        self.copyright_link.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ensure_test_project_dirs_exist() {
        ensure_project_dirs_exist(&get_test_project()).unwrap();
    }

    pub fn get_test_project() -> Project {
        let test_base = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/local"));
        Project::new(
            test_base
                .join("config")
                .join(env!("CARGO_CRATE_NAME"))
                .join("config.json"),
            test_base.join("share").join(env!("CARGO_CRATE_NAME")),
            test_base
                .join("state")
                .join(env!("CARGO_CRATE_NAME"))
                .join("image_index.json"),
        )
    }
}
