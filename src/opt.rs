use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use jiff::Zoned;
use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};

use crate::{config::Project, Config, RawConfig};

#[derive(Debug, Parser)]
#[command(version, flatten_help = true)]
pub struct Opt {
    #[arg(long, global = true, default_value = None)]
    pub config_path: Option<PathBuf>,

    #[arg(long, global = true)]
    pub state_path: Option<PathBuf>,

    #[arg(long, global = true)]
    pub data_path: Option<PathBuf>,

    #[arg(long, global = true, default_value = None)]
    pub index: Option<u8>,

    #[arg(long, global = true, default_value = None)]
    pub number: Option<u8>,

    #[arg(long, global = true, default_value = None)]
    pub market: Option<String>,

    #[command(subcommand)]
    pub cmd: Option<Cmd>,

    #[arg(long, global = true, value_enum)]
    pub size: Option<Resolution>,

    #[arg(long, global = true, value_enum)]
    pub ext: Option<Extension>,

    #[arg(long, exclusive = true)]
    pub completion: Option<Shell>,
}

impl Opt {
    pub fn get_config(&self) -> anyhow::Result<Config> {
        let project = self.get_project()?;
        let raw_config = self.get_raw_config(&project)?;
        Ok(Config::new(self, project, raw_config))
    }

    pub fn get_config_with_project(&self, project: Project) -> anyhow::Result<Config> {
        let raw_config = self.get_raw_config(&project)?;
        Ok(Config::new(self, project, raw_config))
    }

    pub fn get_project(&self) -> anyhow::Result<Project> {
        Project::initialize(self)
    }

    pub fn get_raw_config(&self, project: &Project) -> anyhow::Result<RawConfig> {
        let raw_config = if let Some(path) = self.get_config_file(project) {
            RawConfig::from_file(path)?
        } else {
            RawConfig::default()
        };

        Ok(raw_config)
    }

    fn get_config_file<'a>(&'a self, project: &'a Project) -> Option<&'a Path> {
        self.config_path.as_deref().or_else(|| {
            let default_config_path = project.config_file_path.as_path();
            default_config_path
                .try_exists()
                .is_ok_and(|x| x)
                .then_some(default_config_path)
        })
    }

    pub fn print_completion(writer: &mut impl std::io::Write, shell: Shell) {
        use clap::CommandFactory;
        clap_complete::generate(
            shell,
            &mut Self::command(),
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            writer,
        );
    }
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Update metadata from the Internet, downloading images if necessary
    Update {
        #[arg(long)]
        quiet: bool,
    },

    /// Make an HTTP call to the metadata URL
    State {
        /// Just print the URL that would be called to get metadata
        #[arg(short, long)]
        url: bool,

        /// Return the real JSON (not trimmed) that is returned from the HTTP call
        #[arg(short, long)]
        raw: bool,

        /// Print only from the local state file; don't update
        #[arg(long)]
        frozen: bool,
    },

    /// Show the configuration
    Config {
        #[command(flatten)]
        args: ShowConfigArgs,
    },

    /// Print the resolved project directories
    ///
    /// Note that this takes other CLI flags into account.
    ProjectDirs,

    #[command(visible_alias = "list")]
    ListImages {
        /// Which columns to print
        #[arg(short, long, value_enum, value_delimiter = ',', num_args(1..), conflicts_with = "all")]
        format: Vec<ImagePart>,

        #[arg(short, long, default_value = None)]
        date: Option<String>,

        #[arg(short, long, conflicts_with = "date", value_enum)]
        relative: Option<Option<RelativeFlag>>,

        #[arg(long)]
        approx: bool,

        /// Print all columns (default if -f is not passed)
        #[arg(long)]
        all: bool,

        #[arg(long)]
        now: Option<Zoned>,

        #[arg(long)]
        short: bool,
    },

    Show {
        #[clap(flatten)]
        kind: ShowKindArg,

        #[arg(long)]
        update: bool,
    },

    Reset {
        /// Which directories to remove
        #[arg(short, long, value_enum, value_delimiter = ',', conflicts_with = "all")]
        items: Vec<ResetItem>,

        /// Don't remove anything, just show what would be removed
        #[arg(short = 'n', long, visible_alias = "pretend")]
        dry_run: bool,

        /// Reset everything (default if -i is not passed)
        #[arg(long)]
        all: bool,
    },

    Completion {
        #[arg(short, long)]
        shell: Shell,
    },
}

#[derive(Debug, Args, Clone, Copy)]
pub struct ShowConfigArgs {
    #[arg(short, default_value_t, long, value_enum)]
    pub kind: ShowConfigKind,

    #[arg(short, long)]
    pub compact: bool,

    #[arg(short, long)]
    pub path: bool,
}

#[derive(Debug, Default, ValueEnum, Clone, Copy)]
pub enum ShowConfigKind {
    Raw,
    #[default]
    Resolved,
}

#[derive(Debug, Args, Clone, Copy)]
#[group(multiple = false)]
pub struct ShowKindArg {
    #[arg(long)]
    current: bool,

    #[arg(long)]
    random: bool,

    #[arg(long)]
    latest: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ShowKind {
    Current,
    Random { update: bool },
    Latest,
}

impl From<(ShowKindArg, bool)> for ShowKind {
    fn from(value: (ShowKindArg, bool)) -> Self {
        let (kind, update) = value;
        if kind.current {
            Self::Current
        } else if kind.latest {
            Self::Latest
        } else if kind.random {
            Self::Random { update }
        } else {
            unreachable!("Unknown ShowKindArg");
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    #[default]
    Uhd,
    Resolution(u16, u16),
}

impl Resolution {
    const ALL: &[Self] = &[
        Self::Uhd,
        Self::Resolution(1920, 1200),
        Self::Resolution(1920, 1080),
        Self::Resolution(1366, 768),
        Self::Resolution(1280, 768),
        Self::Resolution(1024, 768),
        Self::Resolution(800, 600),
        Self::Resolution(800, 480),
        Self::Resolution(768, 1280),
        Self::Resolution(720, 1280),
        Self::Resolution(640, 480),
        Self::Resolution(480, 800),
        Self::Resolution(400, 240),
        Self::Resolution(320, 240),
        Self::Resolution(240, 320),
    ];
}

impl clap::ValueEnum for Resolution {
    fn value_variants<'a>() -> &'a [Self] {
        Self::ALL
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(self.to_string()))
    }
}

impl std::str::FromStr for Resolution {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "UHD" {
            Ok(Self::Uhd)
        } else {
            let (width, height) = s
                .split_once('x')
                .ok_or_else(|| anyhow::anyhow!("Invalid resolution"))?;
            let resolution = Self::Resolution(width.parse()?, height.parse()?);
            if !Self::ALL.contains(&resolution) {
                eprintln!("Warning: unknown resolution");
            }
            Ok(resolution)
        }
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uhd => write!(f, "UHD"),
            Self::Resolution(w, h) => write!(f, "{w}x{h}"),
        }
    }
}

#[derive(Debug, Default, ValueEnum, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "lower")]
#[serde(rename_all = "lowercase")]
pub enum Extension {
    #[default]
    Jpg,
    Webp,
}

impl std::str::FromStr for Extension {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jpg" => Ok(Self::Jpg),
            "webp" => Ok(Self::Webp),
            _ => anyhow::bail!("Invalid extension"),
        }
    }
}

impl std::fmt::Display for Extension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value().unwrap().get_name().fmt(f)
    }
}

mod resolution_serde {
    use super::Resolution;

    struct ResolutionVisitor;

    impl<'de> serde::Deserialize<'de> for Resolution {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_string(ResolutionVisitor)
        }
    }

    impl<'de> serde::de::Visitor<'de> for ResolutionVisitor {
        type Value = Resolution;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string matching one of the resolution values")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            v.parse().map_err(serde::de::Error::custom)
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&v)
        }
    }

    impl serde::Serialize for Resolution {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }
}

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum ResetItem {
    /// Remove downloaded images
    Images,

    /// Reset local state
    State,
}

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum ImagePart {
    Path,
    FullPath,
    Title,
    Url,
    Time,
    Current,
    Copyright,
}

impl ImagePart {
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::Current,
            Self::Time,
            Self::FullPath,
            Self::Path,
            Self::Title,
            Self::Url,
            Self::Copyright,
        ]
    }
}

#[derive(Debug, Default, ValueEnum, PartialEq, Eq, Clone, Copy)]
pub enum RelativeFlag {
    #[default]
    Long,
    Raw,
    Short,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn check_arg_sanity() {
        Opt::command().debug_assert();
    }

    fn get_expected_resolutions() -> Vec<&'static str> {
        vec![
            "UHD",
            "1920x1200",
            "1920x1080",
            "1366x768",
            "1280x768",
            "1024x768",
            "800x600",
            "800x480",
            "768x1280",
            "720x1280",
            "640x480",
            "480x800",
            "400x240",
            "320x240",
            "240x320",
        ]
    }

    #[test]
    fn check_resolution_values_clap() {
        let expected = get_expected_resolutions();

        let actual = Resolution::ALL
            .iter()
            .map(|x| x.to_possible_value().unwrap().get_name().to_string())
            .collect::<Vec<_>>();

        assert_eq!(expected, actual);
    }

    #[test]
    fn check_resolution_values_display() {
        let expected = get_expected_resolutions();

        let actual = Resolution::ALL
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>();

        assert_eq!(expected, actual);
    }
}
