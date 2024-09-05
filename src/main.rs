use clap::Parser;

use bing_wallpaper::Opt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    bing_wallpaper::run(opt).await
}
