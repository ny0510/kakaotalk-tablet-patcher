mod downloader;
mod gplay;
mod patcher;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use reqwest::Client;

#[derive(Parser)]
#[command(name = "kakaotalk-tablet-patcher")]
#[command(about = "카카오톡 태블릿 버전(다중기기 로그인) 패치 도구", long_about = None)]
#[command(author = "ny64")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = ".")]
    work_dir: PathBuf,
}

#[derive(clap::Subcommand)]
enum Commands {
    #[command(about = "Download all required files (LSPatch, TabletSpoof, KakaoTalk APK)")]
    Download,
    #[command(about = "Patch KakaoTalk APK with TabletSpoof using LSPatch")]
    Patch,
    #[command(about = "Download and patch in one step")]
    Run,
}

async fn ensure_downloads(client: &Client, dirs: &downloader::WorkDirs) -> Result<downloader::KakaoTalkArtifacts> {
    downloader::download_lspatch(client, dirs).await?;
    downloader::download_tabletspoof(client, dirs).await?;
    downloader::download_kakaotalk(client, dirs).await
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let dirs = downloader::WorkDirs::new(&cli.work_dir);
    dirs.ensure_dirs()?;

    match cli.command {
        Commands::Download => {
            let client = Client::new();
            let artifacts = ensure_downloads(&client, &dirs).await?;
            println!("\nAll downloads complete (base + {} splits).", artifacts.splits.len());
        }
        Commands::Patch => {
            let client = Client::new();
            let artifacts = ensure_downloads(&client, &dirs).await?;
            println!();
            patcher::patch_apk(&client, &dirs, &artifacts).await?;
        }
        Commands::Run => {
            let client = Client::new();
            let artifacts = ensure_downloads(&client, &dirs).await?;
            println!();
            patcher::patch_apk(&client, &dirs, &artifacts).await?;
        }
    }

    Ok(())
}
