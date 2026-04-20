use std::path::{ Path, PathBuf };
use std::process::Stdio;

use anyhow::{ Context, Result, bail };
use reqwest::Client;
use serde::Deserialize;

use crate::downloader::WorkDirs;

const ADOPTIUM_API: &str = "https://api.adoptium.net/v3/assets/latest/21/hotspot";

fn adoptium_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "mac",
        "linux" => "linux",
        "windows" => "windows",
        other => other,
    }
}

fn adoptium_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" | "x86-64" => "x64",
        "aarch64" | "arm64" => "aarch64",
        "x86" | "i686" | "i386" => "x32",
        other => other,
    }
}

fn archive_extension() -> &'static str {
    if std::env::consts::OS == "windows" { ".zip" } else { ".tar.gz" }
}

#[derive(Debug, Deserialize)]
struct AdoptiumResponse {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
}

fn jre_dir(dirs: &WorkDirs) -> PathBuf {
    dirs.downloads.join("jre")
}

fn java_binary(dirs: &WorkDirs) -> PathBuf {
    let jre = jre_dir(dirs);
    if std::env::consts::OS == "macos" {
        jre.join("Contents").join("Home").join("bin").join("java")
    } else if std::env::consts::OS == "windows" {
        jre.join("bin").join("java.exe")
    } else {
        jre.join("bin").join("java")
    }
}

async fn download_jre(client: &Client, dirs: &WorkDirs) -> Result<PathBuf> {
    let java_bin = java_binary(dirs);
    if java_bin.exists() {
        return Ok(java_bin);
    }

    let os = adoptium_os();
    let arch = adoptium_arch();
    let ext = archive_extension();

    let url = format!("{ADOPTIUM_API}?architecture={arch}&image_type=jre&os={os}&vendor=adoptium");

    println!("[JRE] Java not found. Downloading JRE 21 ({os}/{arch})...");

    let resp: Vec<AdoptiumResponse> = client
        .get(&url)
        .header("User-Agent", "kakaotalk-tablet-patcher")
        .send().await
        .context("Failed to query Adoptium API")?
        .json().await
        .context("Failed to parse Adoptium response")?;

    let release = resp.into_iter().next().context("No JRE release found from Adoptium")?;

    let package = release.binary.package;

    if !package.name.ends_with(ext) {
        bail!("Unexpected JRE archive format: {}. Expected {ext}", package.name);
    }

    let archive_path = dirs.downloads.join(&package.name);
    crate::downloader::download_file(client, &package.link, &archive_path, "JRE").await?;

    println!("[JRE] Extracting JRE...");
    let jre_dest = jre_dir(dirs);
    std::fs::create_dir_all(&jre_dest).context("Failed to create JRE directory")?;

    extract_archive(&archive_path, &jre_dest)?;

    let extracted_jre = find_extracted_jre(&jre_dest)?;
    if extracted_jre != jre_dest {
        move_contents(&extracted_jre, &jre_dest)?;
    }

    let _ = std::fs::remove_file(&archive_path);

    if !java_bin.exists() {
        bail!("JRE downloaded and extracted but java binary not found at {}", java_bin.display());
    }

    println!("[JRE] JRE ready: {}", java_bin.display());
    Ok(java_bin)
}

fn extract_archive(archive: &Path, dest: &Path) -> Result<()> {
    if archive.extension().is_some_and(|e| e == "gz") {
        let status = std::process::Command
            ::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(dest)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run tar")?;

        if !status.success() {
            bail!("Archive extraction failed for {}", archive.display());
        }
    } else {
        let file = std::fs::File::open(archive).context("Failed to open zip archive")?;
        let mut archive = zip::ZipArchive::new(file).context("Failed to read zip archive")?;
        archive.extract(dest).context("Failed to extract zip archive")?;
    }
    Ok(())
}

fn find_extracted_jre(jre_dir: &Path) -> Result<PathBuf> {
    let mut best: Option<PathBuf> = None;
    for entry in std::fs::read_dir(jre_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let java = if std::env::consts::OS == "macos" {
                path.join("Contents").join("Home").join("bin").join("java")
            } else {
                path.join("bin").join("java")
            };
            if java.exists() {
                best = Some(path);
                break;
            }
        }
    }
    best.context("No JRE directory found after extraction")
}

fn move_contents(from: &Path, to: &Path) -> Result<()> {
    let tmp = to.with_extension("tmp");
    std::fs::rename(from, &tmp).context("Failed to move extracted JRE")?;

    for entry in std::fs::read_dir(&tmp)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        std::fs::rename(entry.path(), dest)?;
    }

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

async fn ensure_java(client: &Client, dirs: &WorkDirs) -> Result<PathBuf> {
    let java_check = tokio::process::Command
        ::new("java")
        .arg("-version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status().await;

    if let Ok(status) = java_check {
        if status.success() {
            let full_path = tokio::process::Command
                ::new("java")
                .arg("-XshowSettings:property")
                .arg("-version")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output().await
                .ok()
                .and_then(|o| {
                    let out = String::from_utf8_lossy(&o.stderr);
                    out.lines()
                        .find(|l| l.contains("java.home"))
                        .and_then(|l| l.rsplit('=').next())
                        .map(|s| s.trim().to_string())
                })
                .map(|home| {
                    let bin = if std::env::consts::OS == "windows" {
                        format!("{home}\\bin\\java.exe")
                    } else {
                        format!("{home}/bin/java")
                    };
                    PathBuf::from(&bin)
                })
                .unwrap_or_else(|| PathBuf::from("java"));
            return Ok(full_path);
        }
    }
    download_jre(client, dirs).await
}

pub async fn patch_apk(
    client: &Client,
    dirs: &WorkDirs,
    artifacts: &crate::downloader::KakaoTalkArtifacts
) -> Result<()> {
    let java_path = ensure_java(client, dirs).await?;

    let lspatch_jar = dirs.lspatch_jar();
    let tabletspoof_apk = dirs.tabletspoof_apk();
    let output_dir = &dirs.output;

    for (name, path) in [
        ("lspatch.jar", &lspatch_jar),
        ("TabletSpoof.apk", &tabletspoof_apk),
        ("base.apk", &artifacts.base_apk),
    ] {
        if !path.exists() {
            bail!("{name} not found at {}. Run download first.", path.display());
        }
    }

    for split in &artifacts.splits {
        if !split.exists() {
            bail!("Split APK not found: {}. Run download first.", split.display());
        }
    }

    println!("[Patch] Patching KakaoTalk with TabletSpoof via LSPatch...");
    println!("  Java        : {}", java_path.display());
    println!("  LSPatch jar : {}", lspatch_jar.display());
    println!("  Module      : {}", tabletspoof_apk.display());
    println!("  Base APK    : {}", artifacts.base_apk.display());
    let names: Vec<_> = artifacts.splits
        .iter()
        .filter_map(|p| p.file_name())
        .collect();
    println!(
        "  Splits      : {} files ({})",
        names.len(),
        names
            .iter()
            .map(|n| n.to_string_lossy())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("  Output dir  : {}", output_dir.display());

    let mut cmd = tokio::process::Command::new(&java_path);
    cmd.arg("-jar").arg(&lspatch_jar).arg(&artifacts.base_apk);

    for split in &artifacts.splits {
        cmd.arg(split);
    }

    cmd.arg("-m")
        .arg(&tabletspoof_apk)
        .arg("--sigbypasslv")
        .arg("2")
        .arg("-o")
        .arg(output_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await.context("Failed to run lspatch")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            eprintln!("{stdout}");
        }
        if !stderr.is_empty() {
            eprintln!("{stderr}");
        }
        bail!("LSPatch failed with exit code {:?}", output.status.code());
    }

    let apks = collect_patched_apks(output_dir)?;
    if apks.is_empty() {
        bail!("No patched APK found in {}", output_dir.display());
    }

    println!("[Done] Patched {} file(s), bundling into .apks...", apks.len());

    let apks_path = output_dir.join("KakaoTalk-Patched.apks");
    bundle_apks(&apks, &apks_path)?;

    println!("[Done] Output: {}", apks_path.display());
    Ok(())
}

fn collect_patched_apks(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut apks: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().ends_with(".apk") {
            apks.push(entry.path());
        }
    }
    apks.sort();
    Ok(apks)
}

fn bundle_apks(apks: &[PathBuf], dest: &Path) -> Result<()> {
    let file = std::fs::File::create(dest).context("Failed to create .apks file")?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions
        ::default()
        .compression_method(zip::CompressionMethod::Stored);

    for apk in apks {
        let name = apk.file_name().context("Invalid APK path")?.to_string_lossy();
        let mut f = std::fs::File::open(apk).context("Failed to open patched APK")?;
        zip.start_file(name.as_ref(), options)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    zip.finish().context("Failed to finalize .apks")?;
    Ok(())
}
