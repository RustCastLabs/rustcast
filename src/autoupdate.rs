use std::str::FromStr;

use log::{error, info};
use sha2::{Digest, Sha256};

pub struct ReleaseInfo {
    pub version: String,
    pub zip_url: String,
    pub sha256: String,
}

pub fn get_latest_release() -> Option<ReleaseInfo> {
    let req = minreq::Request::new(
        minreq::Method::Get,
        "https://api.github.com/repos/RustCastLabs/rustcast/releases/latest",
    )
    .with_header("User-Agent", "rustcast-update-checker")
    .with_header("Accept", "application/vnd.github+json")
    .with_header("X-GitHub-Api-Version", "2022-11-28");

    let resp = req
        .send()
        .and_then(|x| x.as_str().map(serde_json::Value::from_str));

    if let Ok(Ok(val)) = resp {
        let version = val.get("name")?.as_str()?.to_string();

        let assets = val.get("assets")?.as_array()?;

        let mut zip_url = None;
        let mut sha256 = None;

        for asset in assets {
            let name = asset.get("name")?.as_str()?;
            let url = asset.get("browser_download_url")?.as_str()?.to_string();

            if name == "Rustcast-universal-macos.app.zip" {
                zip_url = Some(url);

                sha256 = asset
                    .get("digest")
                    .and_then(|d| d.as_str())
                    .and_then(|d| d.strip_prefix("sha256:"))
                    .map(|d| d.to_string());
            }
        }

        Some(ReleaseInfo {
            version,
            zip_url: zip_url?,
            sha256: sha256?,
        })
    } else {
        None
    }
}

pub fn new_version_available() -> Option<ReleaseInfo> {
    info!("Checking for new version");
    let info = get_latest_release()?;
    info!("Got latest info");
    let current = option_env!("APP_VERSION").unwrap_or("");

    if info.version != current {
        Some(info)
    } else {
        None
    }
}

pub fn verify_sha256(file_path: &std::path::Path, expected_hex: &str) -> std::io::Result<bool> {
    let bytes = std::fs::read(file_path)?;
    let digest = Sha256::digest(&bytes);
    let actual_hex = hex::encode(digest);
    Ok(actual_hex == expected_hex)
}

pub fn download_latest_app() -> Result<std::path::PathBuf, ()> {
    let info = get_latest_release().ok_or_else(|| {
        error!("Could not get latest release info");
    })?;

    info!("got latest release");

    let tmp = tempfile::tempdir().map_err(|e| {
        error!("Could not create temporary directory: {e}");
    })?;

    info!("created temp dir");

    let zip_path = tmp.path().join("Rustcast-universal-macos.app.zip");

    info!("zip path: {:?}", zip_path);
    let resp = minreq::get(&info.zip_url)
        .with_header("User-Agent", "rustcast-update-checker")
        .send()
        .map_err(|e| {
            error!("Could not download update: {e}");
        })?;

    info!("downloaded zip");

    std::fs::write(&zip_path, resp.as_bytes()).map_err(|e| {
        error!("Could not write zip to disk: {e}");
    })?;

    info!("wrote zip to disk");

    let ok = verify_sha256(&zip_path, &info.sha256).map_err(|e| {
        error!("Could not verify sha256: {e}");
    })?;

    info!("verified sha256");

    if !ok {
        error!("SHA256 mismatch — aborting update");
        return Err(());
    }

    let zip_file = std::fs::File::open(&zip_path).map_err(|e| {
        error!("Could not open zip: {e}");
    })?;

    info!("opened zip");

    let mut archive = zip::ZipArchive::new(zip_file).map_err(|e| {
        error!("Could not read zip archive: {e}");
    })?;

    info!("read zip archive. contents:");

    archive.extract(tmp.path()).map_err(|e| {
        error!("Could not extract zip: {e}");
    })?;

    if let Ok(entries) = std::fs::read_dir(tmp.path()) {
        for entry in entries.flatten() {
            info!("  extracted entry: {:?}", entry.file_name());
        }
    }

    let extracted_app = tmp.path().join("target/release/macos/Rustcast.app");

    info!("found extracted app at: {:?}", extracted_app);

    let dest = get_app_path().ok_or_else(|| {
        error!("Could not determine current app path");
    })?;

    info!("Installing update over {:?}", dest);

    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| {
            error!("Could not remove existing app: {e}");
        })?;
    }

    move_or_copy(&extracted_app, &dest).map_err(|e| {
        error!("Could not move app into place: {e}");
    })?;

    info!("Successful update");

    Ok(dest)
}

fn move_or_copy(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_recursive(src, dst)?;
            std::fs::remove_dir_all(src)
        }
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

pub fn relaunch_app() {
    let app_path = match get_app_path() {
        Some(p) => p,
        None => {
            error!("Could not determine current app path for relaunch");
            return;
        }
    };

    match std::process::Command::new("open").arg(&app_path).spawn() {
        Ok(_) => {
            info!("Relaunching app at {:?}", app_path);
            std::thread::sleep(std::time::Duration::from_millis(500));
            std::process::exit(0);
        }
        Err(e) => {
            error!("Could not relaunch app: {e}");
        }
    }
}

pub fn get_app_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;

    let mut path = exe.as_path();
    loop {
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            return Some(path.to_path_buf());
        }
        path = path.parent()?;
    }
}
