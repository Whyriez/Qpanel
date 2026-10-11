use serde::{Deserialize, Serialize};

/// Metric point for server/host level resource utilization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerMetricsPoint {
    pub timestamp: i64,
    pub cpu_percent: f64,
    pub cpu_cores: u32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub inodes_used: u64,
    pub inodes_total: u64,
    pub net_rx_bytes_sec: f64,
    pub net_tx_bytes_sec: f64,
    pub io_read_bytes_sec: f64,
    pub io_write_bytes_sec: f64,
}

/// Metric point for an individual tenant's cgroup slice (`panel-u{uid}.slice`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantMetricsPoint {
    pub timestamp: i64,
    pub uid: u32,
    pub cpu_usage_usec: u64,
    pub cpu_percent: f64,
    pub memory_current_bytes: u64,
    pub memory_peak_bytes: Option<u64>,
    pub memory_swap_bytes: Option<u64>,
    pub pids_current: u64,
    pub io_read_bytes_sec: f64,
    pub io_write_bytes_sec: f64,
}

/// Metric point for a system service cgroup (`system.slice/{service}.service`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceMetricsPoint {
    pub timestamp: i64,
    pub service_name: String,
    pub cpu_usage_usec: u64,
    pub cpu_percent: f64,
    pub memory_current_bytes: u64,
    pub pids_current: u64,
    pub io_read_bytes_sec: f64,
    pub io_write_bytes_sec: f64,
}

/// Target resolution interval for rollups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollupResolution {
    Raw,
    OneMinute,
    OneHour,
    OneDay,
}

impl RollupResolution {
    pub fn step_secs(&self) -> i64 {
        match self {
            Self::Raw => 10,
            Self::OneMinute => 60,
            Self::OneHour => 3600,
            Self::OneDay => 86400,
        }
    }
}

/// Aggregated rollup point for server metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerMetricsRollup {
    pub bucket_timestamp: i64,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub cpu_min: f64,
    pub memory_used_avg: f64,
    pub memory_used_max: u64,
    pub swap_used_avg: f64,
    pub swap_used_max: u64,
    pub load1_avg: f64,
    pub load1_max: f64,
    pub disk_used_avg: f64,
    pub disk_used_max: u64,
    pub net_rx_avg: f64,
    pub net_rx_max: f64,
    pub net_tx_avg: f64,
    pub net_tx_max: f64,
    pub io_read_avg: f64,
    pub io_read_max: f64,
    pub io_write_avg: f64,
    pub io_write_max: f64,
    pub sample_count: u32,
}

/// Aggregated rollup point for tenant metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantMetricsRollup {
    pub bucket_timestamp: i64,
    pub uid: u32,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_avg: f64,
    pub memory_max: u64,
    pub pids_avg: f64,
    pub pids_max: u64,
    pub io_read_avg: f64,
    pub io_write_avg: f64,
    pub sample_count: u32,
}

/// Aggregated rollup point for service metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceMetricsRollup {
    pub bucket_timestamp: i64,
    pub service_name: String,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_avg: f64,
    pub memory_max: u64,
    pub pids_avg: f64,
    pub pids_max: u64,
    pub io_read_avg: f64,
    pub io_write_avg: f64,
    pub sample_count: u32,
}

/// Retention configuration adhering to PRD 5.6:
/// - 10s raw: 2 hours (7200s)
/// - 1m rollups: 14 days (14 * 86400s)
/// - 1h rollups: 400 days (400 * 86400s)
/// - 1d rollups: indefinite
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionConfig {
    pub raw_retention_secs: i64,
    pub one_minute_retention_secs: i64,
    pub one_hour_retention_secs: i64,
    pub one_day_retention_secs: Option<i64>,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            raw_retention_secs: 2 * 3600,          // 2 hours
            one_minute_retention_secs: 14 * 86400, // 14 days
            one_hour_retention_secs: 400 * 86400,  // 400 days
            one_day_retention_secs: None,          // Indefinite
        }
    }
}

/// Report after performing a metrics collection iteration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsCollectionReport {
    pub timestamp: i64,
    pub server_collected: bool,
    pub tenants_collected: usize,
    pub services_collected: usize,
}

/// Report after running rollup computation and retention pruning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollupReport {
    pub timestamp: i64,
    pub server_1m_created: usize,
    pub server_1h_created: usize,
    pub server_1d_created: usize,
    pub tenant_1m_created: usize,
    pub tenant_1h_created: usize,
    pub tenant_1d_created: usize,
    pub service_1m_created: usize,
    pub service_1h_created: usize,
    pub service_1d_created: usize,
    pub pruned_records: usize,
}

/// Parsed, structured log entry from Nginx `panel_json` log stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedLogEntry {
    pub site_id: i64,
    pub timestamp: i64,
    pub client_ip: String,
    pub host: String,
    pub method: String,
    pub uri: String,
    pub status: u16,
    pub body_bytes_sent: u64,
    pub request_time_secs: f64,
    pub upstream_response_time_secs: Option<f64>,
    pub upstream_cache_status: Option<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub protocol: Option<String>,
    pub tls_version: Option<String>,
}

/// Checkpoint offset state for crash safety and rotation recovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogCheckpoint {
    pub file_path: String,
    pub site_id: i64,
    pub inode: u64,
    pub file_offset: u64,
    pub last_processed_ts: i64,
    pub updated_at: i64,
}

/// A batch of parsed log lines along with file coordinate updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogBatch {
    pub file_path: String,
    pub site_id: i64,
    pub inode: u64,
    pub new_offset: u64,
    pub entries: Vec<ParsedLogEntry>,
}

/// Configuration parameters for log tailing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogTailerConfig {
    pub poll_interval_ms: u64,
    pub batch_size: usize,
    pub channel_capacity: usize,
    pub checkpoint_interval_ms: u64,
}

impl Default for LogTailerConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 500,
            batch_size: 100,
            channel_capacity: 256,
            checkpoint_interval_ms: 1000,
        }
    }
}

/// Configuration parameters for the multi-file log tailer engine.
#[derive(Debug, Clone)]
pub struct TailerEngineConfig {
    pub watch_directories: Vec<std::path::PathBuf>,
    pub poll_interval: std::time::Duration,
    pub batch_size: usize,
    pub channel_capacity: usize,
}

impl Default for TailerEngineConfig {
    fn default() -> Self {
        Self {
            watch_directories: vec![
                std::path::PathBuf::from("/var/log/panel/sites"),
                std::path::PathBuf::from("/var/log/nginx"),
            ],
            poll_interval: std::time::Duration::from_millis(500),
            batch_size: 100,
            channel_capacity: 256,
        }
    }
}

/// Real-time statistics of the log tailer engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TailerStats {
    pub active_tailers: usize,
    pub total_lines_processed: u64,
    pub total_bytes_processed: u64,
    pub total_batches_processed: u64,
}
