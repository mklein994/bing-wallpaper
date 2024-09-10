mod commands;
pub mod config;
mod jiff_serde;
pub mod opt;

use std::io::prelude::*;
use std::path::PathBuf;
use std::{collections::BTreeSet, fs::File};

use anyhow::anyhow;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use jiff::{SpanRound, Unit, Zoned};
use rand::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

pub use config::Config;
use config::Project;
pub use config::Raw as RawConfig;
pub use opt::Opt;
use opt::{Cmd, RelativeFlag, ShowKind};

const URL_BASE: &str = "https://www.bing.com";

pub async fn run(opt: Opt, writer: &mut impl std::io::Write) -> anyhow::Result<()> {
    let config = Config::initialize(&opt)?;

    if let Some(cmd) = opt.cmd {
        match cmd {
            Cmd::State { url, raw, frozen } => {
                commands::print_state(writer, &config, url, raw, frozen).await?;
            }
            Cmd::ProjectDirs => commands::print_project_dirs(writer, &config)?,
            Cmd::ListImages {
                ref format,
                all,
                date,
                relative,
                now,
                approx,
            } => commands::list_images(
                writer,
                &config,
                format,
                all,
                date.as_deref(),
                relative.map(Option::unwrap_or_default),
                approx,
                &now.unwrap_or_else(Zoned::now),
            )?,
            Cmd::Update { quiet } => commands::update_images(writer, &config, quiet).await?,
            Cmd::Show { kind, update } => {
                commands::show(writer, &config, ShowKind::from((kind, update)))?;
            }
            Cmd::Reset {
                all,
                dry_run,
                items,
            } => commands::reset(writer, &config, all, dry_run, &items)?,
            Cmd::Completion { shell } => Opt::print_completion(writer, shell),
        }
    } else if let Some(shell) = opt.completion {
        Opt::print_completion(writer, shell);
    } else {
        let mut state = get_local_state(&config)?;
        let image_path = &state.get_random_image(&config)?;
        state.current_image = Some(image_path.clone());
        state.save(&config)?;
        writeln!(
            writer,
            "{}",
            config.project.data_dir.join(image_path).display()
        )?;
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
    multi: MultiProgress,
) -> anyhow::Result<()> {
    let mut file = File::create_new(absolute_file_name)?;
    let response = client.get(url).send().await?;
    let length = response.content_length().unwrap();
    let progress = multi.add(ProgressBar::new(length));
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let bytes = item?;
        progress.set_position(bytes.len() as u64);
        file.write_all(&bytes)?;
    }

    progress.finish();

    Ok(())
}

async fn sync_images(
    writer: &mut impl std::io::Write,
    current_image_data: &mut ImageData,
    new_image_data: &mut ImageData,
    client: Client,
    config: &Config,
    quiet: bool,
) -> anyhow::Result<()> {
    let mut download_handles = vec![];
    let multi = MultiProgress::new();
    if quiet {
        multi.set_draw_target(ProgressDrawTarget::hidden());
    }

    current_image_data
        .images
        .difference(&new_image_data.images)
        .try_for_each(|image| writeln!(writer, "Tracking image {:?}...", image.title))?;

    current_image_data.images.append(&mut new_image_data.images);
    for image in &current_image_data.images {
        let image_path = config.project.data_dir.join(image.file_name(config));
        if !image_path.try_exists()? {
            download_handles.push(tokio::spawn(download_image(
                client.clone(),
                image.to_url(config),
                image_path,
                multi.clone(),
            )));
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

    pub fn get_random_image(&self, config: &Config) -> anyhow::Result<PathBuf> {
        if self.image_data.images.is_empty() {
            anyhow::bail!(
                "Looks like you don't have any images. Try running this with no subcommands."
            );
        }

        let images = self
            .image_data
            .images
            .iter()
            .filter(|image| {
                if let Some(current) = &self.current_image {
                    image.file_name(config) != *current
                } else {
                    true
                }
            })
            .enumerate()
            .collect::<Vec<_>>();

        let mut rng = rand::thread_rng();
        let image_path = images
            .choose_weighted(&mut rng, |(index, _)| index + 1)
            .map(|(_, image)| image)?
            .file_name(config);

        Ok(image_path)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ImageData {
    images: BTreeSet<Image>,
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
            self.url_base, config.size, config.ext
        ))
        .unwrap()
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

fn to_relative(
    start: &Zoned,
    end: &Zoned,
    flag: RelativeFlag,
    approx: bool,
) -> anyhow::Result<String> {
    let round = SpanRound::new().largest(Unit::Year).relative(end);
    let round = if approx {
        round.smallest(Unit::Day)
    } else {
        round
    };

    let diff = start.until(end)?.round(round)?;

    if let RelativeFlag::Raw = flag {
        return Ok(diff.to_string());
    }

    let mut fmt = vec![];
    macro_rules! fmt {
        ($var:ident, $short:literal, $single:literal, $plural:literal, $get:expr) => {
            let $var = $get;
            if $var > 0 {
                fmt.push(if let RelativeFlag::Short = flag {
                    format!("{}{}", $var, $short)
                } else {
                    format!("{} {}", $var, if $var == 1 { $single } else { $plural })
                });
            }
        };
    }

    fmt!(year, "y", "year", "years", diff.get_years());
    fmt!(month, "mo", "month", "months", diff.get_months());
    fmt!(day, "d", "day", "days", diff.get_days());
    fmt!(hour, "h", "hour", "hours", diff.get_hours());
    fmt!(minute, "m", "minute", "minutes", diff.get_minutes());
    fmt!(second, "s", "second", "seconds", diff.get_seconds());

    if fmt.is_empty() {
        if approx {
            fmt.push("today".to_string());
        } else {
            fmt.push("now".to_string());
        }
    }

    Ok(fmt.join(", "))
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
