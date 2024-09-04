mod config;
mod opt;

pub use config::Config;
#[cfg(doc)]
pub use config::RawConfig;

pub use opt::Opt;

pub fn run(opt: Opt) -> anyhow::Result<()> {
    let config = Config::initialize(opt)?;
    Ok(())
}
