mod downloader;
mod gplay;
mod patcher;

use std::path::{ Path, PathBuf };

use anyhow::{ Result, bail };
use clap::Parser;
use reqwest::Client;

#[derive(Parser)]
#[command(name = "kakaotalk-tablet-patcher")]
#[command(about = "KakaoTalk multi-login patcher for non-tablet devices", long_about = None)]
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
    #[command(about = "Patch KakaoTalk APK with TabletSpoof using LSPatch")] Patch {
        /// Path to KakaoTalk base APK file. Skips Google Play download when provided.
        #[arg(long)]
        apk: Option<PathBuf>,
        /// Path to directory containing KakaoTalk split APK files.
        #[arg(long, requires = "apk")]
        splits_dir: Option<PathBuf>,
    },
    #[command(about = "Download and patch in one step")] Run {
        /// Path to KakaoTalk base APK file. Skips Google Play download when provided.
        #[arg(long)]
        apk: Option<PathBuf>,
        /// Path to directory containing KakaoTalk split APK files.
        #[arg(long, requires = "apk")]
        splits_dir: Option<PathBuf>,
    },
}

async fn ensure_downloads(
    client: &Client,
    dirs: &downloader::WorkDirs
) -> Result<downloader::KakaoTalkArtifacts> {
    downloader::download_lspatch(client, dirs).await?;
    downloader::download_tabletspoof(client, dirs).await?;
    downloader::download_kakaotalk(client, dirs).await
}

fn collect_splits(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        bail!("Splits path is not a directory: {}", dir.display());
    }
    let mut splits = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().ends_with(".apk") {
            splits.push(entry.path());
        }
    }
    splits.sort();
    Ok(splits)
}

async fn resolve_artifacts(
    client: &Client,
    dirs: &downloader::WorkDirs,
    apk: Option<PathBuf>,
    splits_dir: Option<PathBuf>
) -> Result<downloader::KakaoTalkArtifacts> {
    downloader::download_lspatch(client, dirs).await?;
    downloader::download_tabletspoof(client, dirs).await?;

    if let Some(apk_path) = apk {
        if !apk_path.exists() {
            bail!("APK file not found: {}", apk_path.display());
        }
        if !apk_path.extension().is_some_and(|e| e == "apk") {
            bail!("Provided file does not have .apk extension: {}", apk_path.display());
        }
        let splits = if let Some(ref dir) = splits_dir { collect_splits(&dir)? } else { vec![] };
        println!("[Custom APK] Using provided APK: {}", apk_path.display());
        if !splits.is_empty() {
            println!("  With {} split(s) from: {}", splits.len(), splits_dir.unwrap().display());
        }
        Ok(downloader::KakaoTalkArtifacts { base_apk: apk_path, splits })
    } else {
        downloader::download_kakaotalk(client, dirs).await
    }
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
        Commands::Patch { apk, splits_dir } => {
            let client = Client::new();
            let artifacts = resolve_artifacts(&client, &dirs, apk, splits_dir).await?;
            println!();
            patcher::patch_apk(&client, &dirs, &artifacts).await?;
        }
        Commands::Run { apk, splits_dir } => {
            let client = Client::new();
            let artifacts = resolve_artifacts(&client, &dirs, apk, splits_dir).await?;
            println!();
            patcher::patch_apk(&client, &dirs, &artifacts).await?;
        }
    }

    Ok(())
}
