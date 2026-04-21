use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

const REPO: &str = "ny0510/kakaotalk-tablet-patcher";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

fn parse_semver(tag: &str) -> Option<(u32, u32, u32)> {
    let tag = tag.trim_start_matches('v');
    let parts: Vec<&str> = tag.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}

pub async fn check_for_update(client: &Client) -> Result<Option<String>> {
    let current = env!("CARGO_PKG_VERSION");
    let current_ver = match parse_semver(current) {
        Some(v) => v,
        None => {
            return Ok(None);
        }
    };

    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release: GithubRelease = client
        .get(&url)
        .header("User-Agent", "kakaotalk-tablet-patcher")
        .send().await?
        .json().await?;

    let latest_ver = match parse_semver(&release.tag_name) {
        Some(v) => v,
        None => {
            return Ok(None);
        }
    };

    if latest_ver > current_ver {
        Ok(
            Some(
                format!(
                    "A new version is available: {} → {}.\nDownload: {}",
                    current,
                    release.tag_name,
                    release.html_url
                )
            )
        )
    } else {
        Ok(None)
    }
}
