use clap::{Parser, Subcommand};

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

    ListImages,
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
