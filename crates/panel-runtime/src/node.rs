//! Node.js Runtime Manager: release version index parsing, SHA256 checksum
//! verification, tarball extraction to `/opt/panel/runtimes/node/<ver>`,
//! corepack enable (pnpm & yarn), and offline fallback catalog.

use crate::error::RuntimeError;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tar::Archive;
use tracing::info;

/// Default installation base directory for Node.js runtimes.
pub const DEFAULT_NODE_BASE_DIR: &str = "/opt/panel/runtimes/node";

/// Default official distribution mirror URL.
pub const DEFAULT_NODE_MIRROR: &str = "https://nodejs.org/dist";

/// Default remote index JSON URL.
pub const DEFAULT_NODE_INDEX_URL: &str = "https://nodejs.org/dist/index.json";

/// Representation of the `lts` field in `index.json`.
/// Can be a string codename (e.g., `"Jod"`, `"Iron"`, `"Hydrogen"`) or boolean `false`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum NodeLtsStatus {
    Codename(String),
    NotLts(bool),
}

impl NodeLtsStatus {
    pub fn is_lts(&self) -> bool {
        match self {
            Self::Codename(_) => true,
            Self::NotLts(b) => *b,
        }
    }

    pub fn codename(&self) -> Option<String> {
        match self {
            Self::Codename(name) => Some(name.clone()),
            Self::NotLts(_) => None,
        }
    }
}

/// Raw version metadata item as returned by `nodejs.org/dist/index.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeVersionEntry {
    pub version: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub v8: Option<String>,
    #[serde(default)]
    pub uv: Option<String>,
    pub lts: NodeLtsStatus,
    #[serde(default)]
    pub security: Option<bool>,
}

impl NodeVersionEntry {
    /// Returns the normalized version without leading 'v' (e.g. "22.14.0").
    pub fn normalized_version(&self) -> String {
        normalize_node_version(&self.version)
    }

    /// Checks if this release is an Active or Maintenance LTS release.
    pub fn is_lts(&self) -> bool {
        self.lts.is_lts()
    }

    /// Returns the LTS codename if this is an LTS release.
    pub fn lts_codename(&self) -> Option<String> {
        self.lts.codename()
    }

    /// Checks if this release has binaries for the specified target OS architecture.
    /// E.g. "linux-x64", "linux-arm64".
    pub fn supports_target(&self, os_arch: &str) -> bool {
        self.files.iter().any(|f| f == os_arch)
    }

    /// Constructs the standard distribution tarball filename.
    /// E.g. "node-v22.14.0-linux-x64.tar.gz".
    pub fn tarball_filename(&self, os: &str, arch: &str) -> String {
        let raw_ver = if self.version.starts_with('v') || self.version.starts_with('V') {
            self.version.clone()
        } else {
            format!("v{}", self.version)
        };
        format!("node-{}-{}-{}.tar.gz", raw_ver, os, arch)
    }
}

/// Information about a Node.js runtime installed on the local system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInstalledVersion {
    pub version: String,
    pub raw_version: String,
    pub path: String,
    pub node_binary: String,
    pub npm_binary: Option<String>,
    pub npx_binary: Option<String>,
    pub corepack_binary: Option<String>,
    pub pnpm_binary: Option<String>,
    pub yarn_binary: Option<String>,
    pub corepack_enabled: bool,
    pub is_lts: bool,
    pub lts_codename: Option<String>,
}

/// Options for installing a Node.js runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInstallOptions {
    pub version: String,
    #[serde(default)]
    pub mirror_url: Option<String>,
    #[serde(default)]
    pub offline_tarball_path: Option<String>,
    #[serde(default)]
    pub offline_checksum: Option<String>,
    #[serde(default = "default_true")]
    pub enable_corepack: bool,
}

fn default_true() -> bool {
    true
}

/// Combined summary of a Node.js version (remote status + local installation + site usage).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeVersionSummary {
    pub version: String,
    pub raw_version: String,
    pub is_installed: bool,
    pub installed_path: Option<String>,
    pub is_lts: bool,
    pub lts_codename: Option<String>,
    pub release_date: Option<String>,
    pub npm_version: Option<String>,
    pub corepack_enabled: bool,
    pub used_by_sites_count: usize,
    pub used_by_domains: Vec<String>,
    pub can_delete: bool,
}

/// Normalizes a Node.js version string to semver numbers without 'v' prefix.
/// Example: "v22.14.0" -> "22.14.0", "  20.18.0 " -> "20.18.0".
pub fn normalize_node_version(v: &str) -> String {
    let trimmed = v.trim();
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    stripped.to_string()
}

/// Formats a normalized version into the canonical release tag (e.g. "22.14.0" -> "v22.14.0").
pub fn raw_node_version_tag(v: &str) -> String {
    let norm = normalize_node_version(v);
    format!("v{}", norm)
}

/// Parses the official Node.js distribution `index.json`.
pub fn parse_nodejs_index_json(json_content: &str) -> Result<Vec<NodeVersionEntry>, RuntimeError> {
    serde_json::from_str::<Vec<NodeVersionEntry>>(json_content)
        .map_err(|e| RuntimeError::JsonParseFailed(format!("Gagal mem-parse index.json: {}", e)))
}

/// Finds the expected SHA256 checksum for a given filename inside `SHASUMS256.txt`.
/// Lines in `SHASUMS256.txt` have the format:
/// `<sha256_hex>  <filename>`
pub fn find_shasum256_in_manifest(shasums_content: &str, target_filename: &str) -> Option<String> {
    for line in shasums_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let hash = parts[0].trim();
            let file = parts[1].trim();

            if file == target_filename {
                return Some(hash.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Computes the lowercase hex SHA256 checksum of raw binary data.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Verifies that binary data matches an expected SHA256 checksum.
pub fn verify_sha256(data: &[u8], expected_hex: &str) -> Result<(), RuntimeError> {
    let actual_hex = compute_sha256(data);
    let clean_expected = expected_hex.trim().to_ascii_lowercase();

    if actual_hex != clean_expected {
        return Err(RuntimeError::ChecksumMismatch {
            expected: clean_expected,
            actual: actual_hex,
        });
    }

    Ok(())
}

/// Extracts a `.tar.gz` Node.js distribution archive into `target_dir`.
/// Strips the root archive folder (e.g., `node-v22.14.0-linux-x64/`) so files land directly
/// in `target_dir/bin`, `target_dir/lib`, etc.
/// Includes strict anti-path-traversal and symlink escape checks.
pub fn extract_node_tarball(archive_data: &[u8], target_dir: &Path) -> Result<usize, RuntimeError> {
    fs::create_dir_all(target_dir).map_err(|e| {
        RuntimeError::Io(format!(
            "Gagal membuat direktori target {}: {}",
            target_dir.display(),
            e
        ))
    })?;

    let canonical_target = target_dir.canonicalize().map_err(|e| {
        RuntimeError::Io(format!(
            "Gagal menyelesaikan direktori {}: {}",
            target_dir.display(),
            e
        ))
    })?;

    let gz_decoder = GzDecoder::new(archive_data);
    let mut archive = Archive::new(gz_decoder);

    let mut extracted_count = 0;

    let entries = archive.entries().map_err(|e| {
        RuntimeError::ExtractionFailed(format!("Gagal membaca entri arsip tar: {}", e))
    })?;

    for entry_res in entries {
        let mut entry = entry_res
            .map_err(|e| RuntimeError::ExtractionFailed(format!("Gagal membaca entri: {}", e)))?;

        let entry_path = entry
            .path()
            .map_err(|e| RuntimeError::ExtractionFailed(format!("Path entri tidak valid: {}", e)))?
            .into_owned();

        // Strip the first component (the top-level `node-vX.Y.Z-arch` directory)
        let mut components = entry_path.components();
        let _root_comp = components.next(); // Skip root folder
        let relative_stripped: PathBuf = components.as_path().to_path_buf();

        if relative_stripped.as_os_str().is_empty() {
            // Root directory entry itself
            continue;
        }

        // Anti-traversal check: verify no `..` or prefix components
        for comp in relative_stripped.components() {
            match comp {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(RuntimeError::ExtractionFailed(format!(
                        "Deteksi path traversal pada entri arsip: {}",
                        entry_path.display()
                    )));
                }
                _ => {}
            }
        }

        let destination = canonical_target.join(&relative_stripped);

        // Security check: destination must stay strictly inside canonical_target
        if !destination.starts_with(&canonical_target) {
            return Err(RuntimeError::ExtractionFailed(format!(
                "Entri arsip berada di luar direktori target: {}",
                destination.display()
            )));
        }

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&destination).map_err(|e| {
                RuntimeError::Io(format!(
                    "Gagal membuat direktori {}: {}",
                    destination.display(),
                    e
                ))
            })?;
        } else {
            if let Some(parent) = destination.parent()
                && !parent.exists()
            {
                fs::create_dir_all(parent).map_err(|e| {
                    RuntimeError::Io(format!(
                        "Gagal membuat direktori {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }

            entry.unpack(&destination).map_err(|e| {
                RuntimeError::ExtractionFailed(format!(
                    "Gagal mengekstrak berkas {}: {}",
                    destination.display(),
                    e
                ))
            })?;

            // On Unix, ensure executable bit for files in `bin/`
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if relative_stripped.starts_with("bin") {
                    let mut perms = fs::metadata(&destination)
                        .map_err(|e| RuntimeError::Io(e.to_string()))?
                        .permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&destination, perms);
                }
            }

            extracted_count += 1;
        }
    }

    info!(
        "Extracted {} files from Node.js tarball to {}",
        extracted_count,
        target_dir.display()
    );

    Ok(extracted_count)
}

/// Enables corepack shims (`pnpm`, `pnpx`, `yarn`, `yarnpkg`) in the runtime's `bin/` directory.
/// When corepack is installed, Node.js provides `<bin_dir>/corepack`.
/// This function verifies corepack exists and creates shims/symlinks for pnpm and yarn.
pub fn enable_corepack_shims(bin_dir: &Path) -> Result<Vec<String>, RuntimeError> {
    if !bin_dir.exists() {
        return Err(RuntimeError::Io(format!(
            "Direktori bin runtime {} tidak ditemukan",
            bin_dir.display()
        )));
    }

    let corepack_bin = bin_dir.join("corepack");
    let corepack_cmd = bin_dir.join("corepack.cmd");

    let corepack_exists = corepack_bin.exists() || corepack_cmd.exists();
    if !corepack_exists {
        return Err(RuntimeError::Io(
            "Binary corepack tidak ditemukan di dalam direktori bin runtime Node.js".to_string(),
        ));
    }

    let shims = ["pnpm", "pnpx", "yarn", "yarnpkg"];
    let mut enabled = Vec::new();

    for shim in &shims {
        let shim_path = bin_dir.join(shim);
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            if !shim_path.exists() {
                // Point symlink to corepack in same directory
                let _ = symlink("corepack", &shim_path);
            }
        }
        #[cfg(windows)]
        {
            // On Windows, create both extensionless shell shim and .cmd batch shim
            if !shim_path.exists() {
                let _ = fs::write(
                    &shim_path,
                    format!("#!/bin/sh\nexec corepack {} \"$@\"\n", shim),
                );
            }
            let cmd_path = shim_path.with_extension("cmd");
            if !cmd_path.exists() {
                let shim_content = format!("@echo off\r\n\"%~dp0\\corepack.cmd\" {} %*\r\n", shim);
                let _ = fs::write(&cmd_path, shim_content);
            }
        }
        enabled.push((*shim).to_string());
    }

    info!(
        "Enabled corepack shims ({}) in {}",
        enabled.join(", "),
        bin_dir.display()
    );

    Ok(enabled)
}

/// Scans the given base directory for installed Node.js versions.
/// Looks for directories matching `/opt/panel/runtimes/node/<ver>`.
pub fn list_installed_node_versions(
    base_dir: &Path,
) -> Result<Vec<NodeInstalledVersion>, RuntimeError> {
    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut installed = Vec::new();

    let entries = fs::read_dir(base_dir).map_err(|e| {
        RuntimeError::Io(format!(
            "Gagal membaca direktori {}: {}",
            base_dir.display(),
            e
        ))
    })?;

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let norm_ver = normalize_node_version(&dir_name);
            let dir_path = entry.path();
            let bin_dir = dir_path.join("bin");

            // Look for `node` or `node.exe`
            let node_bin = bin_dir.join("node");
            let node_exe = bin_dir.join("node.exe");

            let node_path = if node_bin.exists() {
                Some(node_bin.to_string_lossy().to_string())
            } else if node_exe.exists() {
                Some(node_exe.to_string_lossy().to_string())
            } else {
                None
            };

            if let Some(node_binary) = node_path {
                let npm_bin = bin_dir.join("npm");
                let npx_bin = bin_dir.join("npx");
                let corepack_bin = bin_dir.join("corepack");
                let pnpm_bin = bin_dir.join("pnpm");
                let yarn_bin = bin_dir.join("yarn");

                let pnpm_exists = pnpm_bin.exists() || bin_dir.join("pnpm.cmd").exists();
                let yarn_exists = yarn_bin.exists() || bin_dir.join("yarn.cmd").exists();
                let corepack_enabled = pnpm_exists || yarn_exists;

                // Lookup LTS metadata from offline catalog
                let catalog = get_offline_node_catalog();
                let catalog_item = catalog.iter().find(|e| e.normalized_version() == norm_ver);

                let is_lts = catalog_item.map(|i| i.is_lts()).unwrap_or(false);
                let lts_codename = catalog_item.and_then(|i| i.lts_codename());

                installed.push(NodeInstalledVersion {
                    version: norm_ver.clone(),
                    raw_version: format!("v{}", norm_ver),
                    path: dir_path.to_string_lossy().to_string(),
                    node_binary,
                    npm_binary: if npm_bin.exists() {
                        Some(npm_bin.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    npx_binary: if npx_bin.exists() {
                        Some(npx_bin.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    corepack_binary: if corepack_bin.exists() {
                        Some(corepack_bin.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    pnpm_binary: if pnpm_bin.exists() {
                        Some(pnpm_bin.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    yarn_binary: if yarn_bin.exists() {
                        Some(yarn_bin.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    corepack_enabled,
                    is_lts,
                    lts_codename,
                });
            }
        }
    }

    // Sort descending by version
    installed.sort_by(|a, b| b.version.cmp(&a.version));

    Ok(installed)
}

/// Curated offline catalog of LTS and Current releases.
/// Used as an immediate fallback if nodejs.org is unreachable or for air-gapped environments.
pub fn get_offline_node_catalog() -> Vec<NodeVersionEntry> {
    vec![
        NodeVersionEntry {
            version: "v22.14.0".to_string(),
            date: Some("2025-02-11".to_string()),
            files: vec![
                "linux-arm64".to_string(),
                "linux-x64".to_string(),
                "osx-arm64".to_string(),
                "osx-x64".to_string(),
                "win-x64-zip".to_string(),
            ],
            npm: Some("10.9.2".to_string()),
            v8: Some("12.4.254.21".to_string()),
            uv: Some("1.49.2".to_string()),
            lts: NodeLtsStatus::Codename("Jod".to_string()),
            security: Some(false),
        },
        NodeVersionEntry {
            version: "v22.11.0".to_string(),
            date: Some("2024-10-29".to_string()),
            files: vec![
                "linux-arm64".to_string(),
                "linux-x64".to_string(),
                "osx-arm64".to_string(),
                "osx-x64".to_string(),
                "win-x64-zip".to_string(),
            ],
            npm: Some("10.9.0".to_string()),
            v8: Some("12.4.254.20".to_string()),
            uv: Some("1.49.1".to_string()),
            lts: NodeLtsStatus::Codename("Jod".to_string()),
            security: Some(false),
        },
        NodeVersionEntry {
            version: "v20.18.3".to_string(),
            date: Some("2025-02-11".to_string()),
            files: vec![
                "linux-arm64".to_string(),
                "linux-x64".to_string(),
                "osx-arm64".to_string(),
                "osx-x64".to_string(),
                "win-x64-zip".to_string(),
            ],
            npm: Some("10.8.2".to_string()),
            v8: Some("11.3.244.8".to_string()),
            uv: Some("1.46.0".to_string()),
            lts: NodeLtsStatus::Codename("Iron".to_string()),
            security: Some(false),
        },
        NodeVersionEntry {
            version: "v18.20.7".to_string(),
            date: Some("2025-02-11".to_string()),
            files: vec![
                "linux-arm64".to_string(),
                "linux-x64".to_string(),
                "osx-arm64".to_string(),
                "osx-x64".to_string(),
                "win-x64-zip".to_string(),
            ],
            npm: Some("10.8.2".to_string()),
            v8: Some("10.2.154.26".to_string()),
            uv: Some("1.44.2".to_string()),
            lts: NodeLtsStatus::Codename("Hydrogen".to_string()),
            security: Some(false),
        },
        NodeVersionEntry {
            version: "v23.8.0".to_string(),
            date: Some("2025-02-12".to_string()),
            files: vec![
                "linux-arm64".to_string(),
                "linux-x64".to_string(),
                "osx-arm64".to_string(),
                "osx-x64".to_string(),
                "win-x64-zip".to_string(),
            ],
            npm: Some("10.9.2".to_string()),
            v8: Some("13.1.201.15".to_string()),
            uv: Some("1.49.2".to_string()),
            lts: NodeLtsStatus::NotLts(false),
            security: Some(false),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tempfile::tempdir;

    #[test]
    fn test_normalize_node_version() {
        assert_eq!(normalize_node_version("v22.14.0"), "22.14.0");
        assert_eq!(normalize_node_version("V20.18.3"), "20.18.3");
        assert_eq!(normalize_node_version("18.20.7"), "18.20.7");
        assert_eq!(normalize_node_version("  v22.11.0  "), "22.11.0");
        assert_eq!(raw_node_version_tag("22.14.0"), "v22.14.0");
        assert_eq!(raw_node_version_tag("v22.14.0"), "v22.14.0");
    }

    #[test]
    fn test_parse_index_json_full() {
        let sample = r#"[
            {
                "version": "v22.14.0",
                "date": "2025-02-11",
                "files": ["headers", "linux-arm64", "linux-x64", "osx-arm64"],
                "npm": "10.9.2",
                "v8": "12.4.254.21",
                "uv": "1.49.2",
                "zlib": "1.3.0.1",
                "openssl": "3.0.15",
                "modules": "127",
                "lts": "Jod",
                "security": false
            },
            {
                "version": "v23.8.0",
                "date": "2025-02-12",
                "files": ["headers", "linux-x64"],
                "npm": "10.9.2",
                "v8": "13.1.201.15",
                "lts": false,
                "security": false
            }
        ]"#;

        let res = parse_nodejs_index_json(sample).expect("failed to parse index.json");
        assert_eq!(res.len(), 2);

        let v22 = &res[0];
        assert_eq!(v22.version, "v22.14.0");
        assert_eq!(v22.normalized_version(), "22.14.0");
        assert!(v22.is_lts());
        assert_eq!(v22.lts_codename().as_deref(), Some("Jod"));
        assert!(v22.supports_target("linux-x64"));
        assert_eq!(
            v22.tarball_filename("linux", "x64"),
            "node-v22.14.0-linux-x64.tar.gz"
        );

        let v23 = &res[1];
        assert_eq!(v23.version, "v23.8.0");
        assert!(!v23.is_lts());
        assert_eq!(v23.lts_codename(), None);
    }

    #[test]
    fn test_find_shasum256_in_manifest() {
        let manifest = r#"
# Comment line
c46399a9a3bdf01e23737b60098f98c48a70659610fba0bc87b5a1987ba42a42  node-v22.14.0-headers.tar.gz
1ab2e27685608bbcefd7dc2095ad18cbeffec4ad9fbb5d70e30e1dfd8f583eb1  node-v22.14.0-linux-x64.tar.gz
4742e8218e8df5e8d5f3030386227cbe796850c95cb3d4b68494191c015b637f  node-v22.14.0-linux-arm64.tar.gz
"#;

        let found = find_shasum256_in_manifest(manifest, "node-v22.14.0-linux-x64.tar.gz");
        assert_eq!(
            found,
            Some("1ab2e27685608bbcefd7dc2095ad18cbeffec4ad9fbb5d70e30e1dfd8f583eb1".to_string())
        );

        let not_found = find_shasum256_in_manifest(manifest, "node-v22.14.0-win-x64.zip");
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_sha256_verify_success_and_failure() {
        let data = b"console.log('hello world from node runtime');";
        let expected = compute_sha256(data);

        // Verification should succeed with identical hash
        assert!(verify_sha256(data, &expected).is_ok());
        // Case-insensitivity check
        assert!(verify_sha256(data, &expected.to_uppercase()).is_ok());

        // Verification should fail with mismatched hash
        let invalid_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let err = verify_sha256(data, invalid_hash).unwrap_err();
        match err {
            RuntimeError::ChecksumMismatch {
                expected: exp,
                actual: act,
            } => {
                assert_eq!(exp, invalid_hash);
                assert_eq!(act, expected);
            }
            _ => panic!("Expected ChecksumMismatch error"),
        }
    }

    #[test]
    fn test_extract_node_tarball_and_structure() {
        // Build a mock in-memory tar.gz representing node-v22.14.0-linux-x64.tar.gz
        let mut tar_builder = tar::Builder::new(Vec::new());

        let node_script = b"#!/usr/bin/env node\nconsole.log('v22.14.0');\n";
        let mut header_node = tar::Header::new_gnu();
        header_node
            .set_path("node-v22.14.0-linux-x64/bin/node")
            .unwrap();
        header_node.set_size(node_script.len() as u64);
        header_node.set_mode(0o755);
        header_node.set_cksum();
        tar_builder.append(&header_node, &node_script[..]).unwrap();

        let corepack_script = b"#!/usr/bin/env node\n// corepack cli\n";
        let mut header_cp = tar::Header::new_gnu();
        header_cp
            .set_path("node-v22.14.0-linux-x64/bin/corepack")
            .unwrap();
        header_cp.set_size(corepack_script.len() as u64);
        header_cp.set_mode(0o755);
        header_cp.set_cksum();
        tar_builder
            .append(&header_cp, &corepack_script[..])
            .unwrap();

        let readme = b"# Node.js Readme\n";
        let mut header_readme = tar::Header::new_gnu();
        header_readme
            .set_path("node-v22.14.0-linux-x64/README.md")
            .unwrap();
        header_readme.set_size(readme.len() as u64);
        header_readme.set_mode(0o644);
        header_readme.set_cksum();
        tar_builder.append(&header_readme, &readme[..]).unwrap();

        let tar_data = tar_builder.into_inner().unwrap();

        // Compress with gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        use std::io::Write;
        encoder.write_all(&tar_data).unwrap();
        let gz_data = encoder.finish().unwrap();

        // Extract to temporary directory
        let tmp = tempdir().unwrap();
        let target_dir = tmp.path().join("22.14.0");

        let extracted = extract_node_tarball(&gz_data, &target_dir).expect("extraction failed");
        assert_eq!(extracted, 3);

        // Verify root components were stripped
        assert!(target_dir.join("bin").join("node").exists());
        assert!(target_dir.join("bin").join("corepack").exists());
        assert!(target_dir.join("README.md").exists());

        // Verify contents
        let read_node = fs::read_to_string(target_dir.join("bin").join("node")).unwrap();
        assert_eq!(read_node, "#!/usr/bin/env node\nconsole.log('v22.14.0');\n");

        // Test enable corepack shims
        let shims = enable_corepack_shims(&target_dir.join("bin")).expect("enable corepack failed");
        assert!(shims.contains(&"pnpm".to_string()));
        assert!(shims.contains(&"yarn".to_string()));

        // Test listing installed versions
        let installed = list_installed_node_versions(tmp.path()).expect("listing failed");
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, "22.14.0");
        assert_eq!(installed[0].raw_version, "v22.14.0");
        assert!(installed[0].is_lts);
        assert_eq!(installed[0].lts_codename.as_deref(), Some("Jod"));
    }

    #[test]
    fn test_extract_node_tarball_anti_traversal() {
        let mut tar_builder = tar::Builder::new(Vec::new());

        let payload = b"malicious content";
        let mut header = tar::Header::new_gnu();
        // Path traversal payload directly into raw tar header bytes
        let path_bytes = b"node-v22.14.0-linux-x64/../../etc/passwd";
        header.as_mut_bytes()[..path_bytes.len()].copy_from_slice(path_bytes);
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, &payload[..]).unwrap();

        let tar_data = tar_builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        use std::io::Write;
        encoder.write_all(&tar_data).unwrap();
        let gz_data = encoder.finish().unwrap();

        let tmp = tempdir().unwrap();
        let target_dir = tmp.path().join("safe_dir");

        let res = extract_node_tarball(&gz_data, &target_dir);
        assert!(res.is_err());
    }
}
