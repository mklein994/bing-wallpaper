use crate::{
    opt::{ImagePart, RelativeFlag, ResetItem, ShowKind},
    Config, ImageData,
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
pub fn list_images(
    writer: &mut impl std::io::Write,
    config: &Config,
    format: &[ImagePart],
    all: bool,
    date: Option<&str>,
    relative: Option<RelativeFlag>,
    approx: bool,
    now: &Zoned,
) -> anyhow::Result<()> {
    let date_format = |datetime: &Zoned| -> anyhow::Result<String> {
        match date {
            Some(f) => Ok(jiff::fmt::strtime::format(f, datetime)?.to_string()),
            None => Ok(datetime.to_string()),
        }
    };

    let state = super::get_local_state(config)?;
    if state.image_data.images.is_empty() {
        anyhow::bail!("No images found. Try running with the \"update\" subcommand.");
    }

    for image in state.image_data.images {
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
                    line.push(image.absolute_file_name(config).display().to_string());
                }
                ImagePart::Title => line.push(image.title.clone()),
                ImagePart::Url => line.push(image.to_url(config).to_string()),
                ImagePart::Time => {
                    if let Some(relative) = relative {
                        line.push(super::to_relative(
                            &image.full_start_date,
                            now,
                            relative,
                            approx,
                        )?);
                    } else {
                        line.push(date_format(&image.full_start_date)?);
                    }
                }
                ImagePart::Current => line.push(
                    state
                        .current_image
                        .as_ref()
                        .is_some_and(|x| x == &image.file_name(config))
                        .to_string(),
                ),
            }
        }

        writeln!(writer, "{}", line.join("\t"))?;
    }

    Ok(())
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
