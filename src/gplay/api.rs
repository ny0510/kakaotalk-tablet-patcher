use std::path::Path;

use anyhow::{ Context, Result, bail };
use reqwest::Client;

use crate::gplay::auth::{ AuroraAuth, build_fdfe_headers };
use crate::gplay::protobuf::{ ProtoDecoder, find_string, find_varint, find_all_bytes, navigate };

const DETAILS_URL: &str = "https://android.clients.google.com/fdfe/details";
const PURCHASE_URL: &str = "https://android.clients.google.com/fdfe/purchase";
const DELIVERY_URL: &str = "https://android.clients.google.com/fdfe/delivery";

pub struct AppInfo {
    pub title: String,
    pub version_code: u64,
    pub version_string: String,
}

pub struct SplitInfo {
    pub name: String,
    pub download_url: String,
    pub _size: u64,
}

pub struct DeliveryInfo {
    pub download_url: String,
    pub download_size: u64,
    pub cookies: Vec<(String, String)>,
    pub splits: Vec<SplitInfo>,
}

pub async fn get_details(client: &Client, package: &str, auth: &AuroraAuth) -> Result<AppInfo> {
    let headers = build_fdfe_headers(auth);
    let url = format!("{DETAILS_URL}?doc={package}");
    let resp = client
        .get(&url)
        .headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .send().await
        .context("Failed to fetch app details")?;

    if resp.status().as_u16() == 401 {
        bail!("Auth token expired. Try again.");
    }
    if resp.status().as_u16() == 404 {
        bail!("App not found: {package}");
    }
    if !resp.status().is_success() {
        bail!("Details request failed: HTTP {}", resp.status().as_u16());
    }

    let raw = resp.bytes().await?;
    let doc_data = navigate(&raw, &[1, 2, 4]);
    if doc_data.is_empty() {
        bail!("Failed to parse app details. App may be unavailable for this device.");
    }

    let doc_fields = {
        let mut d = ProtoDecoder::new(&doc_data);
        d.read_all()
    };

    let _package_name = find_string(&doc_fields, 1);
    let title = find_string(&doc_fields, 5);

    let app_details_data = navigate(&raw, &[1, 2, 4, 13, 1]);
    let app_details_fields = {
        let mut d = ProtoDecoder::new(&app_details_data);
        d.read_all()
    };
    let version_code = find_varint(&app_details_fields, 3).unwrap_or(0);
    let version_string = find_string(&app_details_fields, 4);

    if version_code == 0 {
        bail!("Failed to parse valid versionCode from Google Play details response");
    }

    Ok(AppInfo {
        title,
        version_code,
        version_string,
    })
}

pub async fn purchase_app(
    client: &Client,
    package: &str,
    version_code: u64,
    auth: &AuroraAuth
) -> Result<()> {
    let headers = build_fdfe_headers(auth);
    let body = format!("doc={package}&ot=1&vc={version_code}");
    let resp = client
        .post(PURCHASE_URL)
        .headers(headers)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .timeout(std::time::Duration::from_secs(30))
        .send().await;

    match resp {
        Ok(r) if r.status().as_u16() == 401 => bail!("Auth token expired."),
        Ok(r) if r.status().is_success() || r.status().as_u16() == 204 => Ok(()),
        Ok(r) => {
            eprintln!("  [Warn] purchase returned HTTP {} (non-fatal)", r.status().as_u16());
            Ok(())
        }
        Err(e) => {
            eprintln!("  [Warn] purchase request failed: {e} (non-fatal)");
            Ok(())
        }
    }
}

pub async fn get_delivery(
    client: &Client,
    package: &str,
    version_code: u64,
    auth: &AuroraAuth
) -> Result<DeliveryInfo> {
    let headers = build_fdfe_headers(auth);
    let url = format!("{DELIVERY_URL}?doc={package}&ot=1&vc={version_code}");
    let resp = client
        .get(&url)
        .headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .send().await
        .context("Failed to fetch delivery info")?;

    if resp.status().as_u16() == 401 {
        bail!("Auth token expired.");
    }
    if !resp.status().is_success() {
        bail!("Delivery request failed: HTTP {}", resp.status().as_u16());
    }

    let raw = resp.bytes().await?;

    for payload_fn in &[21u32, 5, 4, 6] {
        let fields_data = navigate(&raw, &[1, *payload_fn, 2]);
        let fields = {
            let mut d = ProtoDecoder::new(&fields_data);
            d.read_all()
        };
        let download_url = find_string(&fields, 3);
        if !download_url.is_empty() && download_url.starts_with("https://") {
            let download_size = find_varint(&fields, 1).unwrap_or(0);

            let mut cookies = Vec::new();
            for cookie_bytes in find_all_bytes(&fields, 4) {
                let cf = {
                    let mut d = ProtoDecoder::new(cookie_bytes);
                    d.read_all()
                };
                let name = find_string(&cf, 1);
                let value = find_string(&cf, 2);
                if !name.is_empty() {
                    cookies.push((name, value));
                }
            }

            let mut splits = Vec::new();
            for split_bytes in find_all_bytes(&fields, 15) {
                let sf = {
                    let mut d = ProtoDecoder::new(split_bytes);
                    d.read_all()
                };
                let name = find_string(&sf, 1);
                let url = find_string(&sf, 5);
                if !url.is_empty() && url.starts_with("https://") {
                    splits.push(SplitInfo {
                        name,
                        download_url: url,
                        _size: find_varint(&sf, 2).unwrap_or(0),
                    });
                }
            }

            return Ok(DeliveryInfo {
                download_url,
                download_size,
                cookies,
                splits,
            });
        }
    }

    bail!("No download URL found in delivery response. The app may be unavailable.")
}

pub async fn download_apk(
    client: &Client,
    url: &str,
    cookies: &[(String, String)],
    dest: &Path,
    label: &str
) -> Result<()> {
    use futures_util::StreamExt;
    use indicatif::{ ProgressBar, ProgressStyle };

    let mut request = client.get(url);
    if !cookies.is_empty() {
        let cookie_header: String = cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        request = request.header("Cookie", cookie_header);
    }

    let resp = request
        .timeout(std::time::Duration::from_secs(300))
        .send().await
        .context("Failed to start APK download")?;

    if !resp.status().is_success() {
        bail!("APK download failed: HTTP {}", resp.status().as_u16());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-")
    );
    pb.set_message(format!("  {label}"));

    let mut file = std::fs::File::create(dest).context("Failed to create APK file")?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading download stream")?;
        std::io::Write::write_all(&mut file, &chunk)?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message(format!("  {label} done"));
    Ok(())
}
