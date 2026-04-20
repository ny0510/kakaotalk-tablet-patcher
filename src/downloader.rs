use std::path::{ Path, PathBuf };

use anyhow::{ Context, Result, bail };
use futures_util::StreamExt;
use indicatif::{ ProgressBar, ProgressStyle };
use reqwest::Client;
use serde::Deserialize;

const LSPATCH_REPO: &str = "JingMatrix/LSPatch";
const TABLETSPOOF_REPO: &str = "miner7222/TabletSpoof";
const KAKAOTALK_PACKAGE: &str = "com.kakao.talk";

pub struct WorkDirs {
    pub downloads: PathBuf,
    pub output: PathBuf,
}

impl WorkDirs {
    pub fn new(base: &Path) -> Self {
        Self {
            downloads: base.join("downloads"),
            output: base.join("output"),
        }
    }

    pub fn lspatch_jar(&self) -> PathBuf {
        self.downloads.join("lspatch.jar")
    }

    pub fn tabletspoof_apk(&self) -> PathBuf {
        self.downloads.join("TabletSpoof.apk")
    }

    pub fn kakaotalk_apk(&self) -> PathBuf {
        self.downloads.join("base.apk")
    }

    pub fn kakaotalk_splits_dir(&self) -> PathBuf {
        self.downloads.join("splits")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.downloads).context("Failed to create downloads dir")?;
        std::fs::create_dir_all(&self.output).context("Failed to create output dir")?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

async fn get_latest_release(client: &Client, repo: &str) -> Result<GithubRelease> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = client
        .get(&url)
        .header("User-Agent", "kakaotalk-tablet-patcher")
        .send().await
        .context("Failed to fetch latest release")?;

    if !resp.status().is_success() {
        bail!("GitHub API returned {} for {repo}", resp.status().as_u16());
    }

    resp.json().await.context("Failed to parse GitHub release JSON")
}

pub async fn download_file(client: &Client, url: &str, dest: &Path, label: &str) -> Result<()> {
    if dest.exists() {
        println!("  [Skip] {label} already exists: {}", dest.display());
        return Ok(());
    }

    let resp = client
        .get(url)
        .header("User-Agent", "kakaotalk-tablet-patcher")
        .send().await
        .context("Failed to start download")?;

    if !resp.status().is_success() {
        bail!("Download failed with status {} for {url}", resp.status());
    }

    let total_size = resp.content_length();
    let pb = ProgressBar::new(total_size.unwrap_or(0));
    pb.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes} / {total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-")
    );
    pb.set_message(format!("  {label}"));

    let mut file = std::fs::File::create(dest).context("Failed to create output file")?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading download stream")?;
        std::io::Write::write_all(&mut file, &chunk)?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message(format!("  {label} done"));
    Ok(())
}

pub async fn download_lspatch(client: &Client, dirs: &WorkDirs) -> Result<PathBuf> {
    let jar_path = dirs.lspatch_jar();
    if jar_path.exists() {
        println!("[Skip] LSPatch jar already exists");
        return Ok(jar_path);
    }

    println!("[1/3] Fetching LSPatch latest release...");
    let release = get_latest_release(client, LSPATCH_REPO).await?;

    let asset = release.assets
        .iter()
        .find(|a| a.name == "lspatch.jar")
        .context("lspatch.jar not found in latest release")?;

    println!("  Downloading LSPatch {} ({})...", release.tag_name, asset.name);
    download_file(client, &asset.browser_download_url, &jar_path, "LSPatch").await?;

    Ok(jar_path)
}

pub async fn download_tabletspoof(client: &Client, dirs: &WorkDirs) -> Result<PathBuf> {
    let apk_path = dirs.tabletspoof_apk();
    if apk_path.exists() {
        println!("[Skip] TabletSpoof APK already exists");
        return Ok(apk_path);
    }

    println!("[2/3] Fetching TabletSpoof latest release...");
    let release = get_latest_release(client, TABLETSPOOF_REPO).await?;

    let asset = release.assets
        .iter()
        .find(|a| a.name == "TabletSpoof.apk" || a.name.ends_with(".apk") )
        .context("No APK asset found in TabletSpoof release")?;

    println!("  Downloading TabletSpoof {} ({})...", release.tag_name, asset.name);
    download_file(client, &asset.browser_download_url, &apk_path, "TabletSpoof").await?;

    Ok(apk_path)
}

pub struct KakaoTalkArtifacts {
    pub base_apk: PathBuf,
    pub splits: Vec<PathBuf>,
}

pub async fn download_kakaotalk(client: &Client, dirs: &WorkDirs) -> Result<KakaoTalkArtifacts> {
    let base_path = dirs.kakaotalk_apk();
    let splits_dir = dirs.kakaotalk_splits_dir();

    if base_path.exists() && splits_dir.exists() {
        let mut splits = Vec::new();
        for entry in std::fs::read_dir(&splits_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().ends_with(".apk") {
                splits.push(entry.path());
            }
        }
        splits.sort();
        if !splits.is_empty() {
            println!("[Skip] KakaoTalk APK already exists (base + {} splits)", splits.len());
            return Ok(KakaoTalkArtifacts { base_apk: base_path, splits });
        }
    }

    if base_path.exists() {
        let _ = std::fs::remove_file(&base_path);
    }
    if splits_dir.exists() {
        let _ = std::fs::remove_dir_all(&splits_dir);
    }

    println!("[3/3] Downloading KakaoTalk from Google Play...");

    let auth = crate::gplay::auth::fetch_anon_token(client).await?;
    println!("  Authenticated via Aurora dispenser (anonymous)");

    let info = crate::gplay::api::get_details(client, KAKAOTALK_PACKAGE, &auth).await?;
    println!("  {} v{} (versionCode: {})", info.title, info.version_string, info.version_code);

    crate::gplay::api::purchase_app(client, KAKAOTALK_PACKAGE, info.version_code, &auth).await?;

    let delivery = crate::gplay::api::get_delivery(
        client,
        KAKAOTALK_PACKAGE,
        info.version_code,
        &auth
    ).await?;
    println!("  Base APK: {} bytes, {} splits", delivery.download_size, delivery.splits.len());

    crate::gplay::api::download_apk(
        client,
        &delivery.download_url,
        &delivery.cookies,
        &base_path,
        "KakaoTalk (base)"
    ).await?;

    let mut split_paths = Vec::new();
    if !delivery.splits.is_empty() {
        std::fs::create_dir_all(&splits_dir)?;
        for split in &delivery.splits {
            let filename = if split.name.ends_with(".apk") {
                split.name.clone()
            } else {
                format!("{}.apk", split.name)
            };
            let dest = splits_dir.join(&filename);
            crate::gplay::api::download_apk(
                client,
                &split.download_url,
                &delivery.cookies,
                &dest,
                &format!("KakaoTalk ({})", split.name)
            ).await?;
            split_paths.push(dest);
        }
    }

    Ok(KakaoTalkArtifacts { base_apk: base_path, splits: split_paths })
}
