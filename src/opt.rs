use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Opt {
    #[arg(long, global = true, default_value = None)]
    pub config_path: Option<PathBuf>,

    #[arg(long, global = true, default_value = None)]
    pub index: Option<u8>,

    #[arg(long, global = true, default_value = None)]
    pub number: Option<u8>,

    #[arg(long, global = true, default_value = None)]
    pub market: Option<String>,

    #[command(subcommand)]
    pub cmd: Option<Cmd>,

    #[arg(long)]
    pub size: Option<String>,

    #[arg(long)]
    pub ext: Option<String>,

    #[arg(long)]
    pub completion: Option<Shell>,
}

impl Opt {
    pub fn print_completion(shell: Shell) {
        use clap::CommandFactory;
        clap_complete::generate(
            shell,
            &mut Self::command(),
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            &mut std::io::stdout(),
        );
    }
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Update metadata from the Internet, downloading images if necessary
    Update,

    /// Make an HTTP call to the metadata URL
    Metadata {
        /// Just print the URL that would be called to get metadata
        #[arg(short, long)]
        print: bool,

        /// Return the real JSON (not trimmed) that is returned from the HTTP call
        #[arg(short, long)]
        raw: bool,
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

        /// Print all columns (default if -f is not passed)
        #[arg(long)]
        all: bool,
    },

    ShowCurrent {
        /// Read only from the downloaded state file; don't update if missing
        #[arg(long)]
        frozen: bool,
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
}
