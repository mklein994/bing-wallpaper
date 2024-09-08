use crate::{
    opt::{ImagePart, RelativeFlag, ResetItem},
    Config, ImageData,
};

use jiff::Zoned;
use reqwest::Client;

pub fn print_project_dirs(config: &Config) -> Result<(), anyhow::Error> {
    let value = &config.project;
    let contents = serde_json::to_string_pretty(&value)?;
    println!("{contents}");
    Ok(())
}
pub fn list_images(
    config: &Config,
    format: &[ImagePart],
    all: bool,
    date: Option<&str>,
    relative: Option<RelativeFlag>,
) -> anyhow::Result<()> {
    let now = Zoned::now();
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
                        line.push(super::to_relative(&image.full_start_date, &now, relative)?);
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

        println!("{}", line.join("\t"));
    }

    Ok(())
}

pub async fn print_state(
    config: &Config,
    show_url: bool,
    raw: bool,
    frozen: bool,
) -> anyhow::Result<()> {
    if frozen {
        let state = super::get_local_state(config)?;
        let contents = serde_json::to_string_pretty(&state)?;
        println!("{contents}");
    } else {
        let url = config.to_url();
        if show_url {
            println!("{url}");
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

            println!("{contents}");
        }
    }

    Ok(())
}

pub async fn update_images(config: &Config, quiet: bool) -> anyhow::Result<()> {
    super::ensure_project_dirs_exist(&config.project)?;

    let mut state = super::get_local_state(config)?;

    let client = Client::new();
    let new_image_data = super::get_new_image_data(config, &client).await?;
    super::sync_images(&mut state.image_data, new_image_data, client, config, quiet).await?;

    let _ = super::update_random_image(&mut state, config)?;

    let contents = serde_json::to_string_pretty(&state)?;
    std::fs::write(&config.project.state_file_path, contents)?;

    Ok(())
}

pub fn show_current(config: &Config, frozen: bool) -> anyhow::Result<()> {
    let mut state = super::get_local_state(config)?;
    let image_path = if let Some(image) = state.current_image {
        Some(image)
    } else if !frozen {
        Some(super::update_random_image(&mut state, config)?)
    } else {
        None
    };

    if let Some(path) = image_path {
        println!("{}", config.project.data_dir.join(path).display());
    } else {
        anyhow::bail!("No current image set");
    }

    Ok(())
}

pub fn reset(config: &Config, all: bool, dry_run: bool, items: &[ResetItem]) -> anyhow::Result<()> {
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
            eprintln!("[DRY RUN]: Removing {:?}{count_str}...", dir.display());
        } else {
            std::fs::remove_dir_all(dir)?;
        }
    }

    if all || items.contains(&ResetItem::State) {
        if dry_run {
            eprintln!(
                "[DRY RUN]: Removing {:?}...",
                config.project.state_file_path.parent().unwrap().display()
            );
        } else {
            std::fs::remove_dir_all(config.project.state_file_path.parent().unwrap())?;
        }
    }

    Ok(())
}
