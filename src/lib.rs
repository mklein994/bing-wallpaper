mod commands;
pub mod config;
mod jiff_serde;
pub mod opt;

use std::io::prelude::*;
use std::path::PathBuf;
use std::{collections::BTreeSet, fs::File};

use anyhow::anyhow;
use commands::{ImageFilterKind, TimeFormatKind};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use jiff::{SpanRound, Unit, Zoned};
use rand::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

pub use config::Config;
pub use config::Raw as RawConfig;
pub use opt::Opt;
use opt::{Cmd, ImagePart, RelativeFlag, ShowKind};

const URL_BASE: &str = "https://www.bing.com";

pub async fn run(opt: Opt, writer: &mut impl std::io::Write) -> anyhow::Result<()> {
    let config = opt.get_config()?;

    if let Some(cmd) = opt.cmd {
        match cmd {
            Cmd::State { url, raw, frozen } => {
                commands::print_state(writer, &config, url, raw, frozen).await?;
            }
            Cmd::ProjectDirs => commands::print_project_dirs(writer, &config)?,
            Cmd::Config { args } => commands::show_config(writer, &config, args)?,
            Cmd::ListImages {
                format,
                all,
                date,
                relative,
                now,
                approx,
                short,
                missing,
                untracked,
            } => {
                let format = if format.is_empty() {
                    if short {
                        vec![ImagePart::Time, ImagePart::Title]
                    } else {
                        ImagePart::all()
                    }
                } else if all {
                    ImagePart::all()
                } else {
                    format
                };

                let time_format: Option<TimeFormatKind> = if format.contains(&ImagePart::Time) {
                    if let Some(relative_format) = relative {
                        Some(TimeFormatKind::Relative {
                            now: now.unwrap_or_else(Zoned::now),
                            kind: relative_format.unwrap_or_default(),
                            approx,
                        })
                    } else {
                        Some(TimeFormatKind::Date(date))
                    }
                } else {
                    None
                };

                let image_filter = if missing {
                    Some(ImageFilterKind::Missing)
                } else if untracked {
                    Some(ImageFilterKind::Untracked)
                } else {
                    None
                };

                commands::list_images(
                    writer,
                    &config,
                    image_filter,
                    &format,
                    all,
                    time_format.as_ref(),
                )?;
            }
            Cmd::Update { quiet } => {
                commands::update_images(writer, &config, quiet).await?;
            }
            Cmd::Show { kind, update } => {
                commands::show(writer, &config, ShowKind::from((kind, update)))?;
            }
            Cmd::Reset {
                all,
                dry_run,
                items,
            } => commands::reset(writer, &config, all, dry_run, &items)?,
            Cmd::Deduplicate { dry_run } => deduplicate(writer, &config, dry_run)?,
            Cmd::Completion { shell } => Opt::print_completion(writer, shell),
        }
    } else if let Some(shell) = opt.completion {
        Opt::print_completion(writer, shell);
    } else {
        commands::show(writer, &config, ShowKind::Random { update: true })?;
    }

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
    new_image_data: ImageData,
    client: Client,
    config: &Config,
    quiet: bool,
) -> anyhow::Result<()> {
    let mut download_handles = vec![];
    let multi = MultiProgress::new();
    if quiet {
        multi.set_draw_target(ProgressDrawTarget::hidden());
    }

    new_image_data
        .images
        .difference(&current_image_data.images)
        .try_for_each(|image| writeln!(writer, "Tracking image {:?}...", image.title))?;

    for new_image in new_image_data.images {
        current_image_data.images.retain(|existing_image| {
            let is_new_duplicate = existing_image != &new_image
                && existing_image.canonical_id() == new_image.canonical_id();
            if is_new_duplicate {
                writeln!(writer, "Replacing image {:?}...", new_image.title)
                    .expect("Failed to write \"Replacing image\" message");
            }
            !is_new_duplicate
        });
        current_image_data.images.insert(new_image);
    }

    for image in &current_image_data.images {
        let image_path = image.absolute_file_name(config);
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

fn ensure_project_dirs_exist(project: &config::Project) -> anyhow::Result<()> {
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
                if config.exclude.iter().all(|x| !image.url_base.contains(x)) {
                    if let Some(current) = &self.current_image {
                        image.file_name(config) != *current
                    } else {
                        true
                    }
                } else {
                    false
                }
            })
            .enumerate()
            .collect::<Vec<_>>();

        let mut rng = rand::rng();
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

/// An image, as returned by the API
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone)]
struct Image {
    /// The date time the image entered circulation
    ///
    /// e.g. `"202408290400"`
    #[serde(rename = "fullstartdate", with = "jiff_serde::datetime")]
    full_start_date: Zoned,

    /// The date this image will stop being circulated, as per the API
    ///
    /// e.g. `"20240830"`
    #[serde(rename = "enddate", with = "jiff_serde::date")]
    end_date: Zoned,

    /// The hash for the image, as returned by the API
    ///
    /// Skipped because it hasn't proven to be a reliable way to detect duplicates
    #[serde(rename = "hsh", skip)]
    hash: String,

    /// The title of the image
    ///
    /// e.g. `"A Catalan cliffhanger"`
    title: String,

    /// The path to the image, as returned by the API
    ///
    /// e.g. `"/th?id=OHR.CastellfollitSpain_EN-CA7493953677_1920x1080.jpg&rf=LaDigue_1920x1080.jpg&pid=hp"`
    url: String,

    /// The base part of the URL path, without image size or other parameters
    ///
    /// e.g. `"/th?id=OHR.CastellfollitSpain_EN-CA7493953677"`
    #[serde(rename = "urlbase")]
    url_base: String,

    /// A copyright description for the image
    ///
    /// e.g. `"Castellfollit de la Roca, Catalonia, Spain (© Eloi_Omella/Getty Images)"`
    copyright: String,

    /// The URL to the copyright for the image
    ///
    /// e.g. `"https://www.bing.com/search?q=Castellfollit+de+la+Roca+Spain&form=hpcapt&filters=HpDate%3a%2220240829_0400%22"`
    #[serde(rename = "copyrightlink")]
    copyright_link: String,
}

impl Image {
    pub fn canonical_id(&self) -> &str {
        &self.copyright_link
    }

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
                    let date = jiff::fmt::strtime::format("%F", &self.full_start_date).unwrap();
                    Some(PathBuf::from(format!("{date}_{v}")))
                } else {
                    None
                }
            })
            .unwrap()
    }

    pub fn absolute_file_name(&self, config: &Config) -> PathBuf {
        config.project.data_dir.join(self.file_name(config))
    }
}

impl std::hash::Hash for Image {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
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

fn deduplicate(
    writer: &mut impl std::io::Write,
    config: &Config,
    dry_run: bool,
) -> anyhow::Result<()> {
    macro_rules! d {
        ($($args:tt)+) => {
            if dry_run {
                writer.write_fmt(format_args!("DRY RUN: {}\n", format_args!($($args)+)))?
            } else {
                writer.write_fmt(format_args!("{}\n", format_args!($($args)+)))?
            }
        }
    }

    let mut state = get_local_state(config)?;

    let mut deduped: BTreeSet<Image> = BTreeSet::new();
    let mut removed: Vec<Image> = vec![];

    for image in state.image_data.images {
        if let Some(existing) = deduped
            .iter()
            .find(|x| x.canonical_id() == image.canonical_id())
            .cloned()
        {
            if image > existing {
                deduped.remove(&existing);
                removed.push(existing);
                deduped.insert(image);
            } else {
                removed.push(image);
            }
        } else {
            deduped.insert(image);
        }
    }

    if removed.is_empty() {
        d!("No duplicates found.");
        return Ok(());
    }

    for image in &removed {
        let path = image.absolute_file_name(config);
        d!("Removing duplicate {:?}...", image.title);
        if !dry_run && path.try_exists()? {
            std::fs::remove_file(&path)?;
        }
    }

    d!("Removed {} duplicate(s).", removed.len());

    state.image_data.images = deduped;

    // Clear current image if it was a duplicate
    if let Some(ref current) = state.current_image {
        let still_present = state
            .image_data
            .images
            .iter()
            .any(|x| &x.file_name(config) == current);
        if !still_present {
            d!("Current image was a duplicate; clearing selection.");
            state.current_image = None;
        }
    }

    if !dry_run {
        state.save(config)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::get_test_project;

    #[test]
    fn ensure_test_project_dirs_exist() {
        ensure_project_dirs_exist(&get_test_project()).unwrap();
    }
}
