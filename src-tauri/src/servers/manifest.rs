//! Which server packages setup downloads: id, version, URL (+ mirrors), SHA-256, the folder inside the
//! archive to unpack and the folder under `bin\`. The list ships inside Lampion (`manifest.json`); the
//! `manifest.json` asset of the latest GitHub release replaces it when reachable, so a moved or updated
//! download needs no new Lampion version.

use serde::Deserialize;
use std::time::Duration;

const EMBEDDED: &str = include_str!("manifest.json");

/// Packages setup must have. Each lives in `bin\<id>` (see `core::paths`).
pub const REQUIRED: [&str; 4] = ["apache", "php", "mariadb", "phpmyadmin"];

/// The GitHub repository whose releases carry the manifest; a fork sets `LAMPION_REPO` when building.
pub fn repo() -> &'static str {
    option_env!("LAMPION_REPO").unwrap_or("akitb/lampion-windows")
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub vc_runtime: VcRuntime,
    pub packages: Vec<Package>,
}

/// The Microsoft Visual C++ runtime Apache and PHP need.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VcRuntime {
    pub url: String,
    /// Oldest acceptable version, e.g. "14.50".
    pub min_version: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub url: String,
    /// Tried in order when `url` fails (same file, same SHA-256).
    #[serde(default)]
    pub mirrors: Vec<String>,
    pub sha256: String,
    /// The folder inside the archive whose contents are unpacked ("" = the archive root).
    #[serde(default)]
    pub strip: String,
    /// The folder under `bin\`.
    pub target: String,
}

impl Package {
    pub fn urls(&self) -> Vec<&str> {
        std::iter::once(self.url.as_str()).chain(self.mirrors.iter().map(String::as_str)).collect()
    }

    pub fn title(&self) -> String {
        format!("{} {}", self.name, self.version)
    }
}

/// "14.50" or "14.50.35719" → (14, 50, 35719).
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().unwrap_or(Ok(0)).ok()?;
    let build = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, build))
}

/// A folder name that cannot climb out of `bin\` or name a drive.
fn plain_name(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Parses and checks a manifest. It may come from the network, so everything that becomes a download
/// or a path is validated: HTTPS only, a 64-digit SHA-256, plain folder names.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| format!("The package list is not valid: {e}"))?;
    let https = |u: &str| u.starts_with("https://");
    if !https(&m.vc_runtime.url) || parse_version(&m.vc_runtime.min_version).is_none() {
        return Err("The package list has an invalid Visual C++ runtime entry.".into());
    }
    for p in &m.packages {
        if !https(&p.url) || !p.mirrors.iter().all(|u| https(u)) {
            return Err(format!("The package list allows only https:// downloads ({}).", p.id));
        }
        if p.sha256.len() != 64 || !p.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("The package list has no valid SHA-256 for {}.", p.id));
        }
        if !plain_name(&p.target) || !(p.strip.is_empty() || plain_name(&p.strip)) {
            return Err(format!("The package list has an invalid folder name for {}.", p.id));
        }
        if REQUIRED.contains(&p.id.as_str()) && p.target != p.id {
            return Err(format!("The package list must unpack {} into bin\\{}.", p.id, p.id));
        }
    }
    for id in REQUIRED {
        if !m.packages.iter().any(|p| p.id == id) {
            return Err(format!("The package list has no {id} package."));
        }
    }
    Ok(m)
}

/// The list built into this version of Lampion.
pub fn embedded() -> Manifest {
    parse(EMBEDDED).expect("the built-in manifest.json is valid (unit-tested)")
}

/// The newest list: the latest release's `manifest.json` on GitHub, else the built-in one.
/// Returns where it came from, for the activity log.
pub async fn load() -> (Manifest, String) {
    match fetch_latest().await {
        Ok(m) => (m, format!("the latest release of {}", repo())),
        Err(e) => {
            crate::core::activity::info("Setup", &format!("Using the package list built into Lampion ({e})."));
            (embedded(), "Lampion's built-in list".into())
        }
    }
}

async fn fetch_latest() -> Result<Manifest, String> {
    let url = format!("https://github.com/{}/releases/latest/download/manifest.json", repo());
    let resp = super::download::client()?
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("GitHub is not reachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("no manifest.json in the latest release ({})", resp.status()));
    }
    parse(&resp.text().await.map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_manifest_is_valid() {
        let m = embedded();
        for id in REQUIRED {
            let p = m.packages.iter().find(|p| p.id == id).unwrap();
            assert!(p.url.ends_with(".zip"), "{id}");
            assert_eq!(p.target, id);
        }
        let php = m.packages.iter().find(|p| p.id == "php").unwrap();
        assert!(php.url.contains("-Win32-"), "PHP must be the Thread Safe build (has php8apache2_4.dll)");
        assert!(!php.url.to_lowercase().contains("nts"));
        assert_eq!(php.urls().len(), 2);
        assert!(parse_version(&m.vc_runtime.min_version).is_some());
    }

    fn sample(edit: impl Fn(&mut serde_json::Value)) -> String {
        let mut v: serde_json::Value = serde_json::from_str(EMBEDDED).unwrap();
        edit(&mut v);
        v.to_string()
    }

    #[test]
    fn rejects_unsafe_entries() {
        assert!(parse(&sample(|_| {})).is_ok());
        assert!(parse(&sample(|v| v["packages"][0]["url"] = "http://example.com/a.zip".into())).is_err());
        assert!(parse(&sample(|v| v["packages"][0]["sha256"] = "abc".into())).is_err());
        assert!(parse(&sample(|v| v["packages"][0]["target"] = "..".into())).is_err());
        assert!(parse(&sample(|v| v["packages"][0]["target"] = "x\\..\\..".into())).is_err());
        assert!(parse(&sample(|v| v["packages"][0]["strip"] = "../up".into())).is_err());
        assert!(parse(&sample(|v| v["packages"][1]["target"] = "elsewhere".into())).is_err());
        assert!(parse(&sample(|v| {
            v["packages"].as_array_mut().unwrap().retain(|p| p["id"] != "mariadb");
        }))
        .is_err());
        assert!(parse("{not json").is_err());
    }

    #[test]
    fn optional_fields_default() {
        let text = sample(|v| {
            let p = &mut v["packages"][0];
            p.as_object_mut().unwrap().remove("strip");
            p.as_object_mut().unwrap().remove("mirrors");
        });
        let m = parse(&text).unwrap();
        assert_eq!(m.packages[0].strip, "");
        assert!(m.packages[0].mirrors.is_empty());
    }

    #[test]
    fn versions() {
        assert_eq!(parse_version("14.50"), Some((14, 50, 0)));
        assert_eq!(parse_version("v14.44.35211.00"), Some((14, 44, 35211)));
        assert_eq!(parse_version("x"), None);
        assert!(parse_version("14.44.35211").unwrap() < parse_version("14.50").unwrap());
    }
}
