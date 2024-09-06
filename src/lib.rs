mod commands;
pub mod config;
mod jiff_serde;
pub mod opt;

use std::io::prelude::*;
use std::path::PathBuf;
use std::{collections::BTreeSet, fs::File};

use anyhow::anyhow;
use jiff::Zoned;
use rand::prelude::*;
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
            Cmd::Metadata { print, raw } => commands::print_metadata(&config, print, raw).await?,
            Cmd::ProjectDirs => commands::print_project_dirs(&config)?,
            Cmd::ListImages { ref format, all } => commands::list_images(&config, format, all)?,
            Cmd::Update => commands::update_images(&config).await?,
            Cmd::ShowCurrent { frozen } => commands::show_current(&config, frozen)?,
            Cmd::Reset {
                all,
                dry_run,
                items,
            } => commands::reset(&config, all, dry_run, &items)?,
            Cmd::Completion { shell } => Opt::print_completion(shell),
        }
    } else if let Some(shell) = opt.completion {
        Opt::print_completion(shell);
    } else {
        let mut state = get_local_state(&config)?;
        let image_path = update_random_image(&mut state, &config)?;
        println!("{}", config.project.data_dir.join(image_path).display());
    };

    Ok(())
}

fn get_local_state(config: &Config) -> anyhow::Result<AppState> {
    let path = &config.project.state_file_path;
    if path.exists() {
        let contents = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&contents)?)
    } else {
        Ok(AppState::default())
    }
}

async fn get_new_image_data(config: &Config, client: &Client) -> anyhow::Result<ImageData> {
    Ok(client.get(config.to_url()).send().await?.json().await?)
}

async fn download_image(
    client: Client,
    url: Url,
    absolute_file_name: PathBuf,
) -> anyhow::Result<()> {
    let contents = client.get(url).send().await?.bytes().await?;
    if let Some(mut file) = match File::create_new(absolute_file_name) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(err) => Err(err),
    }? {
        file.write_all(&contents)?;
    }

    Ok(())
}

async fn sync_images(
    current_image_data: &mut ImageData,
    new_image_data: ImageData,
    client: Client,
    config: &Config,
) -> anyhow::Result<()> {
    let mut download_handles = vec![];
    for image in new_image_data.images {
        let image_path = image.absolute_file_name(config);
        if !image_path.try_exists()? {
            download_handles.push(tokio::spawn(download_image(
                client.clone(),
                image.to_url(config),
                image_path,
            )));
        }

        if !current_image_data.images.contains(&image) {
            eprintln!("tracking image {:?}...", image.title);
            current_image_data.add_image(image);
        }
    }

    if !download_handles.is_empty() {
        futures::future::try_join_all(download_handles)
            .await?
            .into_iter()
            .collect::<Result<(), _>>()?;
    }
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

fn update_random_image(state: &mut AppState, config: &Config) -> anyhow::Result<PathBuf> {
    let mut rng = rand::thread_rng();

    if state.image_data.images.is_empty() {
        anyhow::bail!(
            "Looks like you don't have any images. Try running this with no subcommands."
        );
    }

    let images = state
        .image_data
        .images
        .iter()
        .filter(|x| {
            if let Some(current) = &state.current_image {
                x.file_name(config) != *current
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    let image_path = images
        .choose_weighted(&mut rng, |x| x.full_start_date.timestamp().as_second())?
        .file_name(config);

    state.current_image = Some(image_path.clone());
    state.save(config)?;

    Ok(image_path)
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AppState {
    image_data: ImageData,
    current_image: Option<PathBuf>,
}

impl AppState {
    pub fn save(&self, config: &Config) -> anyhow::Result<()> {
        let config_path = &config.project.state_file_path;
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, contents)?;
        Ok(())
    }
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone)]
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

    pub fn absolute_file_name(&self, config: &Config) -> PathBuf {
        config.project.data_dir.join(self.file_name(config))
    }

    pub fn file_name(&self, config: &Config) -> PathBuf {
        let url = self.to_url(config);
        url.query_pairs()
            .find_map(|(k, v)| {
                if k == "id" {
                    Some(PathBuf::from(format!("{}_{v}", self.hash)))
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
