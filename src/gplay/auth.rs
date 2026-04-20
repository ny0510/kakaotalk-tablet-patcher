use anyhow::{ Context, Result, bail };
use reqwest::Client;
use serde::{ Deserialize, Serialize };

const AURORA_DISPENSER_URL: &str = "https://auroraoss.com/api/auth";

#[derive(Debug, Clone, Serialize)]
struct DeviceProfile {
    #[serde(rename = "UserReadableName")]
    user_readable_name: &'static str,
    #[serde(rename = "Build.HARDWARE")]
    build_hardware: &'static str,
    #[serde(rename = "Build.RADIO")]
    build_radio: &'static str,
    #[serde(rename = "Build.BOOTLOADER")]
    build_bootloader: &'static str,
    #[serde(rename = "Build.FINGERPRINT")]
    build_fingerprint: &'static str,
    #[serde(rename = "Build.BRAND")]
    build_brand: &'static str,
    #[serde(rename = "Build.DEVICE")]
    build_device: &'static str,
    #[serde(rename = "Build.VERSION.SDK_INT")]
    build_version_sdk_int: &'static str,
    #[serde(rename = "Build.VERSION.RELEASE")]
    build_version_release: &'static str,
    #[serde(rename = "Build.MODEL")]
    build_model: &'static str,
    #[serde(rename = "Build.MANUFACTURER")]
    build_manufacturer: &'static str,
    #[serde(rename = "Build.PRODUCT")]
    build_product: &'static str,
    #[serde(rename = "Build.ID")]
    build_id: &'static str,
    #[serde(rename = "Build.TYPE")]
    build_type: &'static str,
    #[serde(rename = "Build.TAGS")]
    build_tags: &'static str,
    #[serde(rename = "Build.SUPPORTED_ABIS")]
    build_supported_abis: &'static str,
    #[serde(rename = "Platforms")]
    platforms: &'static str,
    #[serde(rename = "Screen.Density")]
    screen_density: &'static str,
    #[serde(rename = "Screen.Width")]
    screen_width: &'static str,
    #[serde(rename = "Screen.Height")]
    screen_height: &'static str,
    #[serde(rename = "Locales")]
    locales: &'static str,
    #[serde(rename = "SharedLibraries")]
    shared_libraries: &'static str,
    #[serde(rename = "Features")]
    features: &'static str,
    #[serde(rename = "GSF.version")]
    gsf_version: &'static str,
    #[serde(rename = "Vending.version")]
    vending_version: &'static str,
    #[serde(rename = "Vending.versionString")]
    vending_version_string: &'static str,
    #[serde(rename = "TimeZone")]
    time_zone: &'static str,
    #[serde(rename = "Client")]
    client: &'static str,
    #[serde(rename = "GL.Version")]
    gl_version: &'static str,
    #[serde(rename = "GL.Extensions")]
    gl_extensions: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuroraAuth {
    #[serde(rename = "authToken")]
    pub auth_token: String,
    #[serde(rename = "gsfId")]
    pub gsf_id: Option<String>,
    #[serde(rename = "dfeCookie", default)]
    pub dfe_cookie: Option<String>,
    #[serde(rename = "deviceCheckInConsistencyToken", default)]
    pub device_check_in_consistency_token: Option<String>,
    #[serde(rename = "deviceConfigToken", default)]
    pub device_config_token: Option<String>,
}

fn pixel_4a_profile() -> DeviceProfile {
    DeviceProfile {
        user_readable_name: "Pixel 4a",
        build_hardware: "sunfish",
        build_radio: "unknown",
        build_bootloader: "unknown",
        build_fingerprint: "google/sunfish/sunfish:13/TQ3A.230805.001/10316531:user/release-keys",
        build_brand: "google",
        build_device: "sunfish",
        build_version_sdk_int: "33",
        build_version_release: "13",
        build_model: "Pixel 4a",
        build_manufacturer: "Google",
        build_product: "sunfish",
        build_id: "TQ3A.230805.001",
        build_type: "user",
        build_tags: "release-keys",
        build_supported_abis: "arm64-v8a,armeabi-v7a,armeabi",
        platforms: "arm64-v8a,armeabi-v7a,armeabi",
        screen_density: "440",
        screen_width: "1080",
        screen_height: "2340",
        locales: "ko-KR,en-US",
        shared_libraries: "android.ext.shared,com.google.android.gms,com.google.android.maps",
        features: "android.hardware.audio.output,android.hardware.bluetooth,android.hardware.camera,android.hardware.camera.autofocus,android.hardware.camera.front,android.hardware.fingerprint,android.hardware.location,android.hardware.location.gps,android.hardware.microphone,android.hardware.screen.portrait,android.hardware.sensor.accelerometer,android.hardware.telephony,android.hardware.touchscreen,android.hardware.touchscreen.multitouch,android.hardware.usb.host,android.hardware.wifi,android.software.webview",
        gsf_version: "223616055",
        vending_version: "82151710",
        vending_version_string: "21.5.17-21 [0] [PR] 326734551",
        time_zone: "America/New_York",
        client: "android-google",
        gl_version: "196610",
        gl_extensions: "GL_OES_EGL_image,GL_OES_EGL_image_external,GL_OES_EGL_sync",
    }
}

pub async fn fetch_anon_token(client: &Client) -> Result<AuroraAuth> {
    let profile = pixel_4a_profile();
    let resp = client
        .post(AURORA_DISPENSER_URL)
        .header("User-Agent", "com.aurora.store-4.6.1-70")
        .header("Content-Type", "application/json")
        .json(&profile)
        .timeout(std::time::Duration::from_secs(30))
        .send().await
        .context("Failed to contact Aurora token dispenser")?;

    if !resp.status().is_success() {
        bail!("Aurora dispenser returned HTTP {}", resp.status().as_u16());
    }

    let auth: AuroraAuth = resp.json().await.context("Failed to parse Aurora auth response")?;

    if auth.auth_token.is_empty() {
        bail!("Aurora dispenser returned empty auth token");
    }

    Ok(auth)
}

pub fn build_fdfe_headers(auth: &AuroraAuth) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {}", auth.auth_token).parse().unwrap()
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        "Android-Finsky/41.2.29-23 [0] [PR] 639844241 (api=3,versionCode=84122900,sdk=34,device=sunfish,hardware=sunfish,product=sunfish,platformVersionRelease=14,model=Pixel%204a,buildId=TQ3A.230805.015,isWideScreen=0,supportedAbis=arm64-v8a;armeabi-v7a;armeabi)"
            .parse()
            .unwrap()
    );
    headers.insert("X-DFE-Device-Id", auth.gsf_id.as_deref().unwrap_or("").parse().unwrap());
    headers.insert(reqwest::header::ACCEPT, "application/x-protobuf".parse().unwrap());
    headers.insert(
        "X-DFE-Encoded-Targets",
        "CAESN/qigQYC2AMBFfUbyA7SM5Ij/CvfBoIDgxXrBPsDlQUdMfOLAfoFrwEHgAcBrQYhoA0cGt4MKK0Y2gI"
            .parse()
            .unwrap()
    );
    headers.insert("X-DFE-Client-Id", "am-android-google".parse().unwrap());
    headers.insert("X-DFE-Network-Type", "4".parse().unwrap());
    headers.insert("X-DFE-Content-Filters", "".parse().unwrap());
    headers.insert("X-Limit-Ad-Tracking-Enabled", "false".parse().unwrap());
    headers.insert("X-Ad-Id", "".parse().unwrap());
    headers.insert("Accept-Language", "ko-KR,en-US;q=0.9".parse().unwrap());
    headers.insert("X-DFE-UserLanguages", "ko_KR,en_US".parse().unwrap());
    headers.insert("X-DFE-Request-Params", "timeoutMs=4000".parse().unwrap());
    headers.insert("X-DFE-No-Prefetch", "true".parse().unwrap());

    if let Some(ref cookie) = auth.dfe_cookie {
        headers.insert("X-DFE-Cookie", cookie.parse().unwrap());
    }
    if let Some(ref token) = auth.device_check_in_consistency_token {
        headers.insert("X-DFE-Device-Checkin-Consistency-Token", token.parse().unwrap());
    }
    if let Some(ref token) = auth.device_config_token {
        headers.insert("X-DFE-Device-Config-Token", token.parse().unwrap());
    }

    headers
}
