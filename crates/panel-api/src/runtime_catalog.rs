//! Persisted, read-only release discovery. This module never installs runtimes.
use crate::{
    auth::{RequireAdminCsrf, RequireAuth},
    routes::AppState,
};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{NaiveDate, Utc};
use panel_runtime::{node::NodeVersionEntry, python::PythonVersionEntry};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet};

pub const NODE_INDEX: &str = "https://nodejs.org/dist/index.json";
pub const NODE_SCHEDULE: &str =
    "https://raw.githubusercontent.com/nodejs/Release/main/schedule.json";
pub const PHP_RELEASES: &str = "https://www.php.net/releases/active.php";
pub const PYTHON_CYCLES: &str = "https://peps.python.org/api/python-releases.json";
pub const PYTHON_DOWNLOADS: &str = "https://raw.githubusercontent.com/astral-sh/uv/main/crates/uv-python-managed/download-metadata.json";
const MAX_AGE: i64 = 7 * 86400;
static REFRESH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub enabled: bool,
    pub interval_hours: u16,
}
impl Default for Schedule {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 24,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache<T> {
    pub checked_at: Option<i64>,
    pub refreshed_at: Option<i64>,
    pub error: Option<String>,
    pub data: T,
}
impl<T: Default> Default for Cache<T> {
    fn default() -> Self {
        Self {
            checked_at: None,
            refreshed_at: None,
            error: None,
            data: T::default(),
        }
    }
}
impl<T> Cache<T> {
    pub fn fresh(&self, now: i64) -> bool {
        self.refreshed_at
            .is_some_and(|at| at <= now && now - at < MAX_AGE)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseLine {
    pub start: String,
    pub end: String,
    pub lts: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeCatalog {
    pub releases: Vec<NodeVersionEntry>,
    pub schedule: BTreeMap<String, ReleaseLine>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpRelease {
    pub branch: String,
    pub latest: String,
}

pub fn version_key(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.strip_prefix('v').unwrap_or(version).split('.');
    let mut next = || {
        parts
            .next()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))?
            .parse::<u32>()
            .ok()
    };
    let key = (next()?, next()?, next()?);
    if parts.next().is_some() {
        None
    } else {
        Some(key)
    }
}
fn date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}
pub fn node_supported(version: &str, catalog: &NodeCatalog, today: NaiveDate) -> bool {
    let Some((major, _, _)) = version_key(version) else {
        return false;
    };
    catalog
        .schedule
        .get(&format!("v{major}"))
        .is_some_and(|line| {
            date(&line.start).is_some_and(|d| d <= today)
                && date(&line.end).is_some_and(|d| today < d)
                && line
                    .lts
                    .as_deref()
                    .and_then(date)
                    .is_some_and(|d| d <= today)
        })
}
pub async fn node_installable(pool: &SqlitePool, version: &str) -> Result<bool, String> {
    let cache = node_cache(pool).await?;
    let today = Utc::now().date_naive();
    Ok(cache.fresh(Utc::now().timestamp())
        && node_choices(&cache.data, today)
            .iter()
            .any(|r| r.normalized_version() == version))
}
pub fn node_support_status(version: &str, catalog: &NodeCatalog, today: NaiveDate) -> &'static str {
    if node_supported(version, catalog, today) {
        return "lts";
    }
    let line =
        version_key(version).and_then(|(major, _, _)| catalog.schedule.get(&format!("v{major}")));
    match line.and_then(|line| date(&line.end)) {
        Some(end) if today >= end => "eol",
        Some(_) => "not_lts",
        None => "unverified",
    }
}
/// Five newest patches per supported LTS line; never offer prereleases or EOL.
pub fn node_choices(catalog: &NodeCatalog, today: NaiveDate) -> Vec<NodeVersionEntry> {
    let mut releases: Vec<_> = catalog
        .releases
        .iter()
        .filter(|r| {
            r.is_lts()
                && node_supported(&r.version, catalog, today)
                && r.date.as_deref().and_then(date).is_some_and(|d| d <= today)
        })
        .cloned()
        .collect();
    releases.sort_by_key(|r| std::cmp::Reverse(version_key(&r.version)));
    let mut counts = BTreeMap::new();
    let mut seen = BTreeSet::new();
    releases.retain(|r| {
        let Some((major, _, _)) = version_key(&r.version) else {
            return false;
        };
        let count = counts.entry(major).or_insert(0);
        if *count >= 5 || !seen.insert(r.version.clone()) {
            return false;
        }
        *count += 1;
        true
    });
    releases
}
pub fn parse_php(value: Value) -> Result<Vec<PhpRelease>, String> {
    let mut releases = Vec::new();
    for branches in value
        .as_object()
        .ok_or("Katalog PHP bukan object")?
        .values()
    {
        for (branch, entry) in branches.as_object().ok_or("Cabang PHP tidak valid")? {
            let latest = entry["version"].as_str().ok_or("Versi PHP hilang")?;
            if version_key(latest).is_some() && latest.starts_with(&format!("{branch}.")) {
                releases.push(PhpRelease {
                    branch: branch.clone(),
                    latest: latest.into(),
                });
            }
        }
    }
    if releases.is_empty() {
        return Err("Katalog PHP kosong".into());
    }
    Ok(releases)
}
pub fn parse_python(
    downloads: Value,
    cycles: &Value,
    today: NaiveDate,
) -> Result<Vec<PythonVersionEntry>, String> {
    let mut versions = BTreeMap::new();
    for item in downloads
        .as_object()
        .ok_or("Katalog Python tidak valid")?
        .values()
    {
        if item["name"] != "cpython"
            || item["os"] != "linux"
            || item["libc"] != "gnu"
            || item["arch"]["family"].as_str() != Some(std::env::consts::ARCH)
            || !item["arch"]["variant"].is_null()
            || !item["prerelease"].is_null()
            || !item["variant"].is_null()
        {
            continue;
        }
        let (Some(major), Some(minor), Some(patch)) = (
            item["major"].as_u64(),
            item["minor"].as_u64(),
            item["patch"].as_u64(),
        ) else {
            continue;
        };
        let branch = format!("{major}.{minor}");
        let cycle = &cycles["metadata"][&branch];
        if !matches!(cycle["status"].as_str(), Some("bugfix" | "security"))
            || !cycle["end_of_life"]
                .as_str()
                .and_then(date)
                .is_some_and(|d| today < d)
            || !cycle["first_release"]
                .as_str()
                .and_then(date)
                .is_some_and(|d| d <= today)
        {
            continue;
        }
        let version = format!("{major}.{minor}.{patch}");
        versions
            .entry((major, minor))
            .and_modify(|v: &mut String| {
                if version_key(&version) > version_key(v) {
                    *v = version.clone();
                }
            })
            .or_insert(version);
    }
    let mut entries: Vec<_> = versions
        .into_iter()
        .rev()
        .take(5)
        .map(|((major, minor), version)| PythonVersionEntry {
            version,
            major_minor: format!("{major}.{minor}"),
            release_date: None,
            is_default: false,
            is_lts_or_stable: true,
            description: "CPython stabil, masih didukung".into(),
        })
        .collect();
    if entries.is_empty() {
        return Err("Tidak ada Python stabil yang didukung".into());
    }
    if let Some(first) = entries.first_mut() {
        first.is_default = true;
    }
    Ok(entries)
}

pub async fn read_setting<T: DeserializeOwned + Default>(
    pool: &SqlitePool,
    key: &str,
) -> Result<T, String> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key=?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    value
        .map(|v| serde_json::from_str(&v).map_err(|e| e.to_string()))
        .unwrap_or_else(|| Ok(T::default()))
}
pub async fn save_setting<T: Serialize>(
    pool: &SqlitePool,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO settings(key,value,updated_at) VALUES(?,?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at")
        .bind(key).bind(json).bind(Utc::now().timestamp()).execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}
pub async fn node_cache(pool: &SqlitePool) -> Result<Cache<NodeCatalog>, String> {
    read_setting(pool, "runtime.catalog.node").await
}
pub async fn php_cache(pool: &SqlitePool) -> Result<Cache<Vec<PhpRelease>>, String> {
    read_setting(pool, "runtime.catalog.php").await
}
pub async fn python_cache(pool: &SqlitePool) -> Result<Cache<Vec<PythonVersionEntry>>, String> {
    read_setting(pool, "runtime.catalog.python").await
}

async fn fetch(url: &'static str) -> Result<Value, String> {
    // Fixed provider URLs, bounded output and runtime; no API-supplied destination.
    let output = tokio::process::Command::new("curl")
        .kill_on_drop(true)
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--max-time",
            "15",
            "--max-filesize",
            "8388608",
            url,
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Sumber resmi tidak dapat dihubungi; katalog terakhir dipertahankan".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
pub fn due(checked: Option<i64>, hours: u16, now: i64) -> bool {
    checked.is_none_or(|at| at > now || now.saturating_sub(at) >= i64::from(hours) * 3600)
}
pub async fn store_refresh<T: Default + Serialize + DeserializeOwned>(
    pool: &SqlitePool,
    key: &str,
    result: Result<T, String>,
    now: i64,
) -> Result<(), String> {
    let mut cache: Cache<T> = read_setting(pool, key).await?;
    cache.checked_at = Some(now);
    match result {
        Ok(data) => {
            cache.data = data;
            cache.refreshed_at = Some(now);
            cache.error = None;
        }
        Err(error) => cache.error = Some(error),
    }
    save_setting(pool, key, &cache).await
}
pub async fn refresh(pool: &SqlitePool, force: bool) -> Result<(), String> {
    let _guard = REFRESH_LOCK.lock().await;
    let config: Schedule = read_setting(pool, "runtime.catalog.schedule").await?;
    if !force && !config.enabled {
        return Ok(());
    }
    let now = Utc::now().timestamp();
    let today = Utc::now().date_naive();
    let node = node_cache(pool).await?;
    if force || due(node.checked_at, config.interval_hours, now) {
        let result = async {
            let (index, schedule) = tokio::try_join!(fetch(NODE_INDEX), fetch(NODE_SCHEDULE))?;
            let releases: Vec<NodeVersionEntry> =
                serde_json::from_value(index).map_err(|e| e.to_string())?;
            let schedule = serde_json::from_value(schedule).map_err(|e| e.to_string())?;
            let catalog = NodeCatalog { releases, schedule };
            if node_choices(&catalog, today).is_empty() {
                return Err("Katalog Node.js tidak memiliki LTS yang didukung".into());
            }
            Ok(catalog)
        }
        .await;
        store_refresh(pool, "runtime.catalog.node", result, now).await?;
    }
    let php = php_cache(pool).await?;
    if force || due(php.checked_at, config.interval_hours, now) {
        let result = match fetch(PHP_RELEASES).await {
            Ok(data) => parse_php(data),
            Err(e) => Err(e),
        };
        store_refresh(pool, "runtime.catalog.php", result, now).await?;
    }
    let python = python_cache(pool).await?;
    if force || due(python.checked_at, config.interval_hours, now) {
        let result = async {
            let (downloads, cycles) =
                tokio::try_join!(fetch(PYTHON_DOWNLOADS), fetch(PYTHON_CYCLES))?;
            parse_python(downloads, &cycles, today)
        }
        .await;
        store_refresh(pool, "runtime.catalog.python", result, now).await?;
    }
    Ok(())
}
pub async fn run_worker(pool: SqlitePool, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _=interval.tick()=> { if let Err(error)=refresh(&pool,false).await { tracing::warn!(%error,"Sinkronisasi katalog runtime gagal"); } },
            changed=shutdown.changed()=> { if changed.is_err() || *shutdown.borrow() { break; } }
        }
    }
}
async fn status(pool: &SqlitePool) -> Result<Value, String> {
    let config: Schedule = read_setting(pool, "runtime.catalog.schedule").await?;
    let mut sources = Vec::new();
    for runtime in ["php", "node", "python"] {
        let key = format!("runtime.catalog.{runtime}");
        let cache: Cache<Value> = read_setting(pool, &key).await?;
        sources.push(serde_json::json!({"runtime":runtime,"checked_at":cache.checked_at,"refreshed_at":cache.refreshed_at,"error":cache.error,"stale":!cache.fresh(Utc::now().timestamp())}));
    }
    Ok(
        serde_json::json!({"enabled":config.enabled,"interval_hours":config.interval_hours,"sources":sources}),
    )
}
fn response(result: Result<Value, String>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}
pub async fn get_status(State(state): State<AppState>, RequireAuth(_): RequireAuth) -> Response {
    response(status(&state.pool).await)
}
pub async fn update_schedule(
    State(state): State<AppState>,
    _: RequireAdminCsrf,
    Json(config): Json<Schedule>,
) -> Response {
    if ![6, 12, 24, 168].contains(&config.interval_hours) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"Interval harus 6, 12, 24 atau 168 jam"})),
        )
            .into_response();
    }
    if let Err(error) = save_setting(&state.pool, "runtime.catalog.schedule", &config).await {
        return response(Err(error));
    }
    response(status(&state.pool).await)
}
pub async fn refresh_handler(State(state): State<AppState>, _: RequireAdminCsrf) -> Response {
    if let Err(error) = refresh(&state.pool, true).await {
        return response(Err(error));
    }
    response(status(&state.pool).await)
}
