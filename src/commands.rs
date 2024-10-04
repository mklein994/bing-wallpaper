use std::{collections::BTreeSet, path::PathBuf};

use crate::{
    opt::{ImagePart, RelativeFlag, ResetItem, ShowConfigArgs, ShowConfigKind, ShowKind},
    Config, ImageData, RawConfig,
};

use jiff::Zoned;
use reqwest::Client;

pub fn print_project_dirs(
    writer: &mut impl std::io::Write,
    config: &Config,
) -> Result<(), anyhow::Error> {
    let value = &config.project;
    let contents = serde_json::to_string_pretty(&value)?;
    writeln!(writer, "{contents}")?;
    Ok(())
}

pub enum TimeFormatKind {
    Date(Option<String>),
    Relative {
        now: Zoned,
        kind: RelativeFlag,
        approx: bool,
    },
}

struct TimeFormat<'a> {
    date: &'a Zoned,
    kind: &'a TimeFormatKind,
}

impl std::fmt::Display for TimeFormat<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            TimeFormatKind::Date(Some(ref format)) => jiff::fmt::strtime::format(format, self.date)
                .unwrap()
                .fmt(f),
            TimeFormatKind::Date(None) => self.date.fmt(f),
            TimeFormatKind::Relative {
                ref now,
                kind,
                approx,
            } => super::to_relative(self.date, now, *kind, *approx)
                .unwrap()
                .fmt(f),
        }
    }
}

pub fn list_images(
    writer: &mut impl std::io::Write,
    config: &Config,
    image_filter: Option<ImageFilterKind>,
    format: &[ImagePart],
    all: bool,
    time_format: Option<&TimeFormatKind>,
) -> anyhow::Result<()> {
    let state = super::get_local_state(config)?;
    if state.image_data.images.is_empty() {
        anyhow::bail!("No images found. Try running with the \"update\" subcommand.");
    }

    if let Some(filter) = image_filter {
        let local_images = get_local_images(config)?;
        let tracked_images: BTreeSet<PathBuf> = state
            .image_data
            .images
            .iter()
            .map(|image| config.project.data_dir.join(image.file_name(config)))
            .collect();

        if let ImageFilterKind::Untracked = filter {
            for image in local_images.difference(&tracked_images) {
                let mut line = vec![];
                for part in format {
                    match part {
                        ImagePart::Path => {
                            line.push(image.file_name().unwrap().to_str().unwrap().to_string());
                        }
                        ImagePart::FullPath => line.push(image.display().to_string()),
                        _ => {}
                    }
                }
                writeln!(writer, "{}", line.join("\t"))?;
            }
        }

        return Ok(());
    };

    let images = if let Some(ImageFilterKind::Missing) = image_filter {
        let local_images = get_local_images(config)?;
        state
            .image_data
            .images
            .into_iter()
            .filter(|image| !local_images.contains(&image.file_name(config)))
            .collect()
    } else {
        state.image_data.images
    };

    for image in images {
        let mut line: Vec<String> = vec![];
        let order = if all || format.is_empty() {
            &ImagePart::all()
        } else {
            format
        };
        for item in order {
            match item {
                ImagePart::Path => {
                    line.push(image.file_name(config).display().to_string());
                }
                ImagePart::FullPath => {
                    let path = config
                        .project
                        .data_dir
                        .join(image.file_name(config))
                        .display()
                        .to_string();
                    line.push(path);
                }
                ImagePart::Title => line.push(image.title.clone()),
                ImagePart::Url => line.push(image.to_url(config).to_string()),
                ImagePart::Time => {
                    let time = TimeFormat {
                        date: &image.full_start_date,
                        kind: time_format.as_ref().unwrap(),
                    };
                    line.push(time.to_string());
                }
                ImagePart::Current => line.push(
                    state
                        .current_image
                        .as_ref()
                        .is_some_and(|x| x == &image.file_name(config))
                        .to_string(),
                ),
                ImagePart::Copyright => line.push(image.copyright.to_string()),
            }
        }

        writeln!(writer, "{}", line.join("\t"))?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
pub enum ImageFilterKind {
    Missing,
    Untracked,
}

pub async fn print_state(
    writer: &mut impl std::io::Write,
    config: &Config,
    show_url: bool,
    raw: bool,
    frozen: bool,
) -> anyhow::Result<()> {
    if frozen {
        let state = super::get_local_state(config)?;
        let contents = serde_json::to_string_pretty(&state)?;
        writeln!(writer, "{contents}")?;
    } else {
        let url = config.to_url();
        if show_url {
            writeln!(writer, "{url}")?;
        } else {
            macro_rules! fetch_and_format_json {
                ($kind:ty) => {{
                    let value = reqwest::get(url).await?.json::<$kind>().await?;
                    Ok::<String, anyhow::Error>(serde_json::to_string_pretty(&value)?)
                }};
            }

            let contents = if raw {
                fetch_and_format_json!(serde_json::Value)?
            } else {
                fetch_and_format_json!(ImageData)?
            };

            writeln!(writer, "{contents}")?;
        }
    }

    Ok(())
}

pub async fn update_images(
    writer: &mut impl std::io::Write,
    config: &Config,
    quiet: bool,
) -> anyhow::Result<()> {
    super::ensure_project_dirs_exist(&config.project)?;

    let mut state = super::get_local_state(config)?;

    let client = Client::new();
    let mut new_image_data = super::get_new_image_data(config, &client).await?;
    super::sync_images(
        writer,
        &mut state.image_data,
        &mut new_image_data,
        client,
        config,
        quiet,
    )
    .await?;

    let random_image = state.get_random_image(config)?;

    state.current_image = Some(random_image);

    state.save(config)?;

    let contents = serde_json::to_string_pretty(&state)?;
    std::fs::write(&config.project.state_file_path, contents)?;

    Ok(())
}

pub fn show(
    writer: &mut impl std::io::Write,
    config: &Config,
    kind: ShowKind,
) -> anyhow::Result<()> {
    let mut state = super::get_local_state(config)?;
    let image_path = match kind {
        ShowKind::Current => state.current_image,
        ShowKind::Random { update } => {
            let random = state.get_random_image(config)?;
            if update {
                state.current_image = Some(random);
                state.save(config)?;
                state.current_image
            } else {
                Some(random)
            }
        }
        ShowKind::Latest => state
            .image_data
            .images
            .iter()
            .max_by_key(|x| &x.full_start_date)
            .map(|x| x.file_name(config)),
    };

    if let Some(path) = image_path {
        writeln!(writer, "{}", config.project.data_dir.join(path).display())?;
    } else {
        anyhow::bail!("No current image set");
    }

    Ok(())
}

fn get_local_images(config: &Config) -> anyhow::Result<BTreeSet<PathBuf>> {
    std::fs::read_dir(&config.project.data_dir)?
        .map(|file| file.map(|f| f.path()).map_err(anyhow::Error::from))
        .collect::<Result<_, _>>()
}

pub fn reset(
    writer: &mut impl std::io::Write,
    config: &Config,
    all: bool,
    dry_run: bool,
    items: &[ResetItem],
) -> anyhow::Result<()> {
    if all || items.contains(&ResetItem::Images) {
        let dir = &config.project.data_dir;
        if dry_run {
            let count = if dir.try_exists()? {
                Some(dir.read_dir()?.count())
            } else {
                None
            };

            let count_str = match count {
                Some(1) => " (1 image)",
                Some(x) => &format!(" ({x} images)"),
                None => "",
            };
            writeln!(
                writer,
                "[DRY RUN]: Removing {:?}{count_str}...",
                dir.display()
            )?;
        } else {
            std::fs::remove_dir_all(dir)?;
        }
    }

    if all || items.contains(&ResetItem::State) {
        if dry_run {
            writeln!(
                writer,
                "[DRY RUN]: Removing {:?}...",
                config.project.state_file_path.parent().unwrap().display()
            )?;
        } else {
            std::fs::remove_dir_all(config.project.state_file_path.parent().unwrap())?;
        }
    }

    Ok(())
}

pub fn show_config(
    writer: &mut impl std::io::Write,
    config: &Config,
    args: ShowConfigArgs,
) -> anyhow::Result<()> {
    if args.path {
        writeln!(writer, "{}", config.project.config_file_path.display())?;
    } else {
        let raw = match args.kind {
            ShowConfigKind::Raw => &config.raw,
            ShowConfigKind::Resolved => &RawConfig {
                index: config.index(),
                market: config.market(),
                number: Some(config.number()),
                size: Some(config.size),
                ext: Some(config.ext),
            },
        };

        if args.compact {
            serde_json::to_writer(&mut *writer, raw)?;
        } else {
            serde_json::to_writer_pretty(&mut *writer, raw)?;
            writeln!(writer)?;
        }
    }

    Ok(())
}
