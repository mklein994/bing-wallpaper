use clap::Parser;

use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Opt {
    #[arg(long, default_value = None)]
    pub config_path: Option<PathBuf>,

    #[arg(long, default_value = None)]
    pub index: Option<u8>,

    #[arg(long, default_value = None)]
    pub number: Option<u8>,

    #[arg(long, default_value = None)]
    pub market: Option<String>,
}
