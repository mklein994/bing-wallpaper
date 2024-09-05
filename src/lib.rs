pub mod config;
mod jiff_serde;
pub mod opt;

use std::collections::BTreeSet;

use jiff::Zoned;
use serde::{Deserialize, Serialize};

pub use config::Config;
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
