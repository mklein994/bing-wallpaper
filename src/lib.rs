pub mod config;
mod jiff_serde;
pub mod opt;

use std::collections::BTreeSet;

use anyhow::anyhow;
use jiff::Zoned;
use serde::{Deserialize, Serialize};

pub use config::Config;
use config::Project;
#[cfg(doc)]
pub use config::RawConfig;
use opt::Cmd;
pub use opt::Opt;

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
        }
    } else {
        ensure_project_dirs_exist(&config.project)?;
        todo!()
    };

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

    if !project.state_file_path.try_exists()? {
        std::fs::create_dir(
            project
                .state_file_path
                .parent()
                .ok_or_else(|| anyhow!("The state file path is not inside a directory"))?,
        )?;
    }

    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
struct ImageData {
    images: BTreeSet<Image>,
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
