use crate::error::AnalyticsError;
use crate::models::{
    LogCheckpoint, RetentionConfig, RollupResolution, ServerMetricsPoint, ServerMetricsRollup,
    ServiceMetricsPoint, ServiceMetricsRollup, TenantMetricsPoint, TenantMetricsRollup,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;
use std::str::FromStr;

#[derive(Clone)]
pub struct AnalyticsStorage {
    pool: Pool<Sqlite>,
}

impl AnalyticsStorage {
    /// Create new storage instance connected to SQLite at specified path.
    pub async fn connect(db_path: impl AsRef<Path>) -> Result<Self, AnalyticsError> {
        let path_str = db_path.as_ref().to_string_lossy().to_string();
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{path_str}"))
            .map_err(AnalyticsError::Database)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(AnalyticsError::Database)?;

        let storage = Self { pool };
        storage.init_schema().await?;
        Ok(storage)
    }

    /// Create in-memory storage for unit testing.
    pub async fn connect_in_memory() -> Result<Self, AnalyticsError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(AnalyticsError::Database)?;

        let storage = Self { pool };
        storage.init_schema().await?;
        Ok(storage)
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Initialize database schema.
    pub async fn init_schema(&self) -> Result<(), AnalyticsError> {
        let schema = r#"
        -- 1. Server metrics: Raw (10s)
        CREATE TABLE IF NOT EXISTS metrics_server_raw (
            timestamp INTEGER NOT NULL PRIMARY KEY,
            cpu_percent REAL NOT NULL,
            cpu_cores INTEGER NOT NULL,
            memory_used_bytes INTEGER NOT NULL,
            memory_total_bytes INTEGER NOT NULL,
            swap_used_bytes INTEGER NOT NULL,
            swap_total_bytes INTEGER NOT NULL,
            load1 REAL NOT NULL,
            load5 REAL NOT NULL,
            load15 REAL NOT NULL,
            disk_used_bytes INTEGER NOT NULL,
            disk_total_bytes INTEGER NOT NULL,
            inodes_used INTEGER NOT NULL,
            inodes_total INTEGER NOT NULL,
            net_rx_bytes_sec REAL NOT NULL,
            net_tx_bytes_sec REAL NOT NULL,
            io_read_bytes_sec REAL NOT NULL,
            io_write_bytes_sec REAL NOT NULL
        );

        -- 2. Server metrics: 1-minute rollup
        CREATE TABLE IF NOT EXISTS metrics_server_1m (
            bucket_ts INTEGER NOT NULL PRIMARY KEY,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            cpu_min REAL NOT NULL,
            memory_used_avg REAL NOT NULL,
            memory_used_max INTEGER NOT NULL,
            swap_used_avg REAL NOT NULL,
            swap_used_max INTEGER NOT NULL,
            load1_avg REAL NOT NULL,
            load1_max REAL NOT NULL,
            disk_used_avg REAL NOT NULL,
            disk_used_max INTEGER NOT NULL,
            net_rx_avg REAL NOT NULL,
            net_rx_max REAL NOT NULL,
            net_tx_avg REAL NOT NULL,
            net_tx_max REAL NOT NULL,
            io_read_avg REAL NOT NULL,
            io_read_max REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            io_write_max REAL NOT NULL,
            sample_count INTEGER NOT NULL
        );

        -- 3. Server metrics: 1-hour rollup
        CREATE TABLE IF NOT EXISTS metrics_server_1h (
            bucket_ts INTEGER NOT NULL PRIMARY KEY,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            cpu_min REAL NOT NULL,
            memory_used_avg REAL NOT NULL,
            memory_used_max INTEGER NOT NULL,
            swap_used_avg REAL NOT NULL,
            swap_used_max INTEGER NOT NULL,
            load1_avg REAL NOT NULL,
            load1_max REAL NOT NULL,
            disk_used_avg REAL NOT NULL,
            disk_used_max INTEGER NOT NULL,
            net_rx_avg REAL NOT NULL,
            net_rx_max REAL NOT NULL,
            net_tx_avg REAL NOT NULL,
            net_tx_max REAL NOT NULL,
            io_read_avg REAL NOT NULL,
            io_read_max REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            io_write_max REAL NOT NULL,
            sample_count INTEGER NOT NULL
        );

        -- 4. Server metrics: 1-day rollup
        CREATE TABLE IF NOT EXISTS metrics_server_1d (
            bucket_ts INTEGER NOT NULL PRIMARY KEY,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            cpu_min REAL NOT NULL,
            memory_used_avg REAL NOT NULL,
            memory_used_max INTEGER NOT NULL,
            swap_used_avg REAL NOT NULL,
            swap_used_max INTEGER NOT NULL,
            load1_avg REAL NOT NULL,
            load1_max REAL NOT NULL,
            disk_used_avg REAL NOT NULL,
            disk_used_max INTEGER NOT NULL,
            net_rx_avg REAL NOT NULL,
            net_rx_max REAL NOT NULL,
            net_tx_avg REAL NOT NULL,
            net_tx_max REAL NOT NULL,
            io_read_avg REAL NOT NULL,
            io_read_max REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            io_write_max REAL NOT NULL,
            sample_count INTEGER NOT NULL
        );

        -- 5. Tenant metrics: Raw (10s)
        CREATE TABLE IF NOT EXISTS metrics_tenant_raw (
            timestamp INTEGER NOT NULL,
            uid INTEGER NOT NULL,
            cpu_usage_usec INTEGER NOT NULL,
            cpu_percent REAL NOT NULL,
            memory_current_bytes INTEGER NOT NULL,
            memory_peak_bytes INTEGER,
            memory_swap_bytes INTEGER,
            pids_current INTEGER NOT NULL,
            io_read_bytes_sec REAL NOT NULL,
            io_write_bytes_sec REAL NOT NULL,
            PRIMARY KEY (timestamp, uid)
        );
        CREATE INDEX IF NOT EXISTS idx_tenant_raw_uid_ts ON metrics_tenant_raw (uid, timestamp);

        -- 6. Tenant metrics: 1-minute rollup
        CREATE TABLE IF NOT EXISTS metrics_tenant_1m (
            bucket_ts INTEGER NOT NULL,
            uid INTEGER NOT NULL,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            memory_avg REAL NOT NULL,
            memory_max INTEGER NOT NULL,
            pids_avg REAL NOT NULL,
            pids_max INTEGER NOT NULL,
            io_read_avg REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            PRIMARY KEY (bucket_ts, uid)
        );
        CREATE INDEX IF NOT EXISTS idx_tenant_1m_uid_ts ON metrics_tenant_1m (uid, bucket_ts);

        -- 7. Tenant metrics: 1-hour rollup
        CREATE TABLE IF NOT EXISTS metrics_tenant_1h (
            bucket_ts INTEGER NOT NULL,
            uid INTEGER NOT NULL,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            memory_avg REAL NOT NULL,
            memory_max INTEGER NOT NULL,
            pids_avg REAL NOT NULL,
            pids_max INTEGER NOT NULL,
            io_read_avg REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            PRIMARY KEY (bucket_ts, uid)
        );
        CREATE INDEX IF NOT EXISTS idx_tenant_1h_uid_ts ON metrics_tenant_1h (uid, bucket_ts);

        -- 8. Tenant metrics: 1-day rollup
        CREATE TABLE IF NOT EXISTS metrics_tenant_1d (
            bucket_ts INTEGER NOT NULL,
            uid INTEGER NOT NULL,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            memory_avg REAL NOT NULL,
            memory_max INTEGER NOT NULL,
            pids_avg REAL NOT NULL,
            pids_max INTEGER NOT NULL,
            io_read_avg REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            PRIMARY KEY (bucket_ts, uid)
        );
        CREATE INDEX IF NOT EXISTS idx_tenant_1d_uid_ts ON metrics_tenant_1d (uid, bucket_ts);

        -- 9. Service metrics: Raw (10s)
        CREATE TABLE IF NOT EXISTS metrics_service_raw (
            timestamp INTEGER NOT NULL,
            service_name TEXT NOT NULL,
            cpu_usage_usec INTEGER NOT NULL,
            cpu_percent REAL NOT NULL,
            memory_current_bytes INTEGER NOT NULL,
            pids_current INTEGER NOT NULL,
            io_read_bytes_sec REAL NOT NULL,
            io_write_bytes_sec REAL NOT NULL,
            PRIMARY KEY (timestamp, service_name)
        );
        CREATE INDEX IF NOT EXISTS idx_service_raw_name_ts ON metrics_service_raw (service_name, timestamp);

        -- 10. Service metrics: 1-minute rollup
        CREATE TABLE IF NOT EXISTS metrics_service_1m (
            bucket_ts INTEGER NOT NULL,
            service_name TEXT NOT NULL,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            memory_avg REAL NOT NULL,
            memory_max INTEGER NOT NULL,
            pids_avg REAL NOT NULL,
            pids_max INTEGER NOT NULL,
            io_read_avg REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            PRIMARY KEY (bucket_ts, service_name)
        );
        CREATE INDEX IF NOT EXISTS idx_service_1m_name_ts ON metrics_service_1m (service_name, bucket_ts);

        -- 11. Service metrics: 1-hour rollup
        CREATE TABLE IF NOT EXISTS metrics_service_1h (
            bucket_ts INTEGER NOT NULL,
            service_name TEXT NOT NULL,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            memory_avg REAL NOT NULL,
            memory_max INTEGER NOT NULL,
            pids_avg REAL NOT NULL,
            pids_max INTEGER NOT NULL,
            io_read_avg REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            PRIMARY KEY (bucket_ts, service_name)
        );
        CREATE INDEX IF NOT EXISTS idx_service_1h_name_ts ON metrics_service_1h (service_name, bucket_ts);

        -- 12. Service metrics: 1-day rollup
        CREATE TABLE IF NOT EXISTS metrics_service_1d (
            bucket_ts INTEGER NOT NULL,
            service_name TEXT NOT NULL,
            cpu_avg REAL NOT NULL,
            cpu_max REAL NOT NULL,
            memory_avg REAL NOT NULL,
            memory_max INTEGER NOT NULL,
            pids_avg REAL NOT NULL,
            pids_max INTEGER NOT NULL,
            io_read_avg REAL NOT NULL,
            io_write_avg REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            PRIMARY KEY (bucket_ts, service_name)
        );
        CREATE INDEX IF NOT EXISTS idx_service_1d_name_ts ON metrics_service_1d (service_name, bucket_ts);

        -- Checkpoints table
        CREATE TABLE IF NOT EXISTS analytics_checkpoints (
            key TEXT PRIMARY KEY,
            last_timestamp INTEGER NOT NULL
        );

        -- Log tailer checkpoints table (P9-T2)
        CREATE TABLE IF NOT EXISTS log_tailer_checkpoints (
            file_path TEXT PRIMARY KEY,
            site_id INTEGER NOT NULL,
            inode INTEGER NOT NULL DEFAULT 0,
            file_offset INTEGER NOT NULL DEFAULT 0,
            last_processed_ts INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#;

        sqlx::query(schema)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        Ok(())
    }

    /// Insert a server raw metric point.
    pub async fn insert_server_point(&self, pt: &ServerMetricsPoint) -> Result<(), AnalyticsError> {
        let query = r#"
        INSERT OR REPLACE INTO metrics_server_raw (
            timestamp, cpu_percent, cpu_cores, memory_used_bytes, memory_total_bytes,
            swap_used_bytes, swap_total_bytes, load1, load5, load15,
            disk_used_bytes, disk_total_bytes, inodes_used, inodes_total,
            net_rx_bytes_sec, net_tx_bytes_sec, io_read_bytes_sec, io_write_bytes_sec
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        sqlx::query(query)
            .bind(pt.timestamp)
            .bind(pt.cpu_percent)
            .bind(pt.cpu_cores as i64)
            .bind(pt.memory_used_bytes as i64)
            .bind(pt.memory_total_bytes as i64)
            .bind(pt.swap_used_bytes as i64)
            .bind(pt.swap_total_bytes as i64)
            .bind(pt.load1)
            .bind(pt.load5)
            .bind(pt.load15)
            .bind(pt.disk_used_bytes as i64)
            .bind(pt.disk_total_bytes as i64)
            .bind(pt.inodes_used as i64)
            .bind(pt.inodes_total as i64)
            .bind(pt.net_rx_bytes_sec)
            .bind(pt.net_tx_bytes_sec)
            .bind(pt.io_read_bytes_sec)
            .bind(pt.io_write_bytes_sec)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        Ok(())
    }

    /// Insert batch of tenant raw metric points.
    pub async fn insert_tenant_points(
        &self,
        pts: &[TenantMetricsPoint],
    ) -> Result<(), AnalyticsError> {
        if pts.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(AnalyticsError::Database)?;
        let query = r#"
        INSERT OR REPLACE INTO metrics_tenant_raw (
            timestamp, uid, cpu_usage_usec, cpu_percent, memory_current_bytes,
            memory_peak_bytes, memory_swap_bytes, pids_current,
            io_read_bytes_sec, io_write_bytes_sec
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        for pt in pts {
            sqlx::query(query)
                .bind(pt.timestamp)
                .bind(pt.uid as i64)
                .bind(pt.cpu_usage_usec as i64)
                .bind(pt.cpu_percent)
                .bind(pt.memory_current_bytes as i64)
                .bind(pt.memory_peak_bytes.map(|v| v as i64))
                .bind(pt.memory_swap_bytes.map(|v| v as i64))
                .bind(pt.pids_current as i64)
                .bind(pt.io_read_bytes_sec)
                .bind(pt.io_write_bytes_sec)
                .execute(&mut *tx)
                .await
                .map_err(AnalyticsError::Database)?;
        }

        tx.commit().await.map_err(AnalyticsError::Database)?;
        Ok(())
    }

    /// Insert batch of service raw metric points.
    pub async fn insert_service_points(
        &self,
        pts: &[ServiceMetricsPoint],
    ) -> Result<(), AnalyticsError> {
        if pts.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(AnalyticsError::Database)?;
        let query = r#"
        INSERT OR REPLACE INTO metrics_service_raw (
            timestamp, service_name, cpu_usage_usec, cpu_percent,
            memory_current_bytes, pids_current, io_read_bytes_sec, io_write_bytes_sec
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        for pt in pts {
            sqlx::query(query)
                .bind(pt.timestamp)
                .bind(&pt.service_name)
                .bind(pt.cpu_usage_usec as i64)
                .bind(pt.cpu_percent)
                .bind(pt.memory_current_bytes as i64)
                .bind(pt.pids_current as i64)
                .bind(pt.io_read_bytes_sec)
                .bind(pt.io_write_bytes_sec)
                .execute(&mut *tx)
                .await
                .map_err(AnalyticsError::Database)?;
        }

        tx.commit().await.map_err(AnalyticsError::Database)?;
        Ok(())
    }

    /// Execute 1-minute rollup for completed buckets up to `before_timestamp`.
    pub async fn rollup_1m(
        &self,
        before_timestamp: i64,
    ) -> Result<(usize, usize, usize), AnalyticsError> {
        let max_bucket = (before_timestamp / 60) * 60;

        // 1. Server 1m rollup
        let s_query = r#"
        INSERT OR REPLACE INTO metrics_server_1m (
            bucket_ts, cpu_avg, cpu_max, cpu_min,
            memory_used_avg, memory_used_max, swap_used_avg, swap_used_max,
            load1_avg, load1_max, disk_used_avg, disk_used_max,
            net_rx_avg, net_rx_max, net_tx_avg, net_tx_max,
            io_read_avg, io_read_max, io_write_avg, io_write_max,
            sample_count
        )
        SELECT
            (timestamp / 60) * 60 AS b_ts,
            AVG(cpu_percent), MAX(cpu_percent), MIN(cpu_percent),
            AVG(memory_used_bytes), MAX(memory_used_bytes),
            AVG(swap_used_bytes), MAX(swap_used_bytes),
            AVG(load1), MAX(load1),
            AVG(disk_used_bytes), MAX(disk_used_bytes),
            AVG(net_rx_bytes_sec), MAX(net_rx_bytes_sec),
            AVG(net_tx_bytes_sec), MAX(net_tx_bytes_sec),
            AVG(io_read_bytes_sec), MAX(io_read_bytes_sec),
            AVG(io_write_bytes_sec), MAX(io_write_bytes_sec),
            COUNT(*)
        FROM metrics_server_raw
        WHERE timestamp < ?
        GROUP BY b_ts
        "#;
        let s_res = sqlx::query(s_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        // 2. Tenant 1m rollup
        let t_query = r#"
        INSERT OR REPLACE INTO metrics_tenant_1m (
            bucket_ts, uid, cpu_avg, cpu_max,
            memory_avg, memory_max, pids_avg, pids_max,
            io_read_avg, io_write_avg, sample_count
        )
        SELECT
            (timestamp / 60) * 60 AS b_ts,
            uid,
            AVG(cpu_percent), MAX(cpu_percent),
            AVG(memory_current_bytes), MAX(memory_current_bytes),
            AVG(pids_current), MAX(pids_current),
            AVG(io_read_bytes_sec), AVG(io_write_bytes_sec),
            COUNT(*)
        FROM metrics_tenant_raw
        WHERE timestamp < ?
        GROUP BY b_ts, uid
        "#;
        let t_res = sqlx::query(t_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        // 3. Service 1m rollup
        let svc_query = r#"
        INSERT OR REPLACE INTO metrics_service_1m (
            bucket_ts, service_name, cpu_avg, cpu_max,
            memory_avg, memory_max, pids_avg, pids_max,
            io_read_avg, io_write_avg, sample_count
        )
        SELECT
            (timestamp / 60) * 60 AS b_ts,
            service_name,
            AVG(cpu_percent), MAX(cpu_percent),
            AVG(memory_current_bytes), MAX(memory_current_bytes),
            AVG(pids_current), MAX(pids_current),
            AVG(io_read_bytes_sec), AVG(io_write_bytes_sec),
            COUNT(*)
        FROM metrics_service_raw
        WHERE timestamp < ?
        GROUP BY b_ts, service_name
        "#;
        let svc_res = sqlx::query(svc_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        Ok((
            s_res.rows_affected() as usize,
            t_res.rows_affected() as usize,
            svc_res.rows_affected() as usize,
        ))
    }

    /// Execute 1-hour rollup for completed buckets up to `before_timestamp`.
    pub async fn rollup_1h(
        &self,
        before_timestamp: i64,
    ) -> Result<(usize, usize, usize), AnalyticsError> {
        let max_bucket = (before_timestamp / 3600) * 3600;

        let s_query = r#"
        INSERT OR REPLACE INTO metrics_server_1h (
            bucket_ts, cpu_avg, cpu_max, cpu_min,
            memory_used_avg, memory_used_max, swap_used_avg, swap_used_max,
            load1_avg, load1_max, disk_used_avg, disk_used_max,
            net_rx_avg, net_rx_max, net_tx_avg, net_tx_max,
            io_read_avg, io_read_max, io_write_avg, io_write_max,
            sample_count
        )
        SELECT
            (bucket_ts / 3600) * 3600 AS b_ts,
            AVG(cpu_avg), MAX(cpu_max), MIN(cpu_min),
            AVG(memory_used_avg), MAX(memory_used_max),
            AVG(swap_used_avg), MAX(swap_used_max),
            AVG(load1_avg), MAX(load1_max),
            AVG(disk_used_avg), MAX(disk_used_max),
            AVG(net_rx_avg), MAX(net_rx_max),
            AVG(net_tx_avg), MAX(net_tx_max),
            AVG(io_read_avg), MAX(io_read_max),
            AVG(io_write_avg), MAX(io_write_max),
            SUM(sample_count)
        FROM metrics_server_1m
        WHERE bucket_ts < ?
        GROUP BY b_ts
        "#;
        let s_res = sqlx::query(s_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let t_query = r#"
        INSERT OR REPLACE INTO metrics_tenant_1h (
            bucket_ts, uid, cpu_avg, cpu_max,
            memory_avg, memory_max, pids_avg, pids_max,
            io_read_avg, io_write_avg, sample_count
        )
        SELECT
            (bucket_ts / 3600) * 3600 AS b_ts,
            uid,
            AVG(cpu_avg), MAX(cpu_max),
            AVG(memory_avg), MAX(memory_max),
            AVG(pids_avg), MAX(pids_max),
            AVG(io_read_avg), AVG(io_write_avg),
            SUM(sample_count)
        FROM metrics_tenant_1m
        WHERE bucket_ts < ?
        GROUP BY b_ts, uid
        "#;
        let t_res = sqlx::query(t_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let svc_query = r#"
        INSERT OR REPLACE INTO metrics_service_1h (
            bucket_ts, service_name, cpu_avg, cpu_max,
            memory_avg, memory_max, pids_avg, pids_max,
            io_read_avg, io_write_avg, sample_count
        )
        SELECT
            (bucket_ts / 3600) * 3600 AS b_ts,
            service_name,
            AVG(cpu_avg), MAX(cpu_max),
            AVG(memory_avg), MAX(memory_max),
            AVG(pids_avg), MAX(pids_max),
            AVG(io_read_avg), AVG(io_write_avg),
            SUM(sample_count)
        FROM metrics_service_1m
        WHERE bucket_ts < ?
        GROUP BY b_ts, service_name
        "#;
        let svc_res = sqlx::query(svc_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        Ok((
            s_res.rows_affected() as usize,
            t_res.rows_affected() as usize,
            svc_res.rows_affected() as usize,
        ))
    }

    /// Execute 1-day rollup for completed buckets up to `before_timestamp`.
    pub async fn rollup_1d(
        &self,
        before_timestamp: i64,
    ) -> Result<(usize, usize, usize), AnalyticsError> {
        let max_bucket = (before_timestamp / 86400) * 86400;

        let s_query = r#"
        INSERT OR REPLACE INTO metrics_server_1d (
            bucket_ts, cpu_avg, cpu_max, cpu_min,
            memory_used_avg, memory_used_max, swap_used_avg, swap_used_max,
            load1_avg, load1_max, disk_used_avg, disk_used_max,
            net_rx_avg, net_rx_max, net_tx_avg, net_tx_max,
            io_read_avg, io_read_max, io_write_avg, io_write_max,
            sample_count
        )
        SELECT
            (bucket_ts / 86400) * 86400 AS b_ts,
            AVG(cpu_avg), MAX(cpu_max), MIN(cpu_min),
            AVG(memory_used_avg), MAX(memory_used_max),
            AVG(swap_used_avg), MAX(swap_used_max),
            AVG(load1_avg), MAX(load1_max),
            AVG(disk_used_avg), MAX(disk_used_max),
            AVG(net_rx_avg), MAX(net_rx_max),
            AVG(net_tx_avg), MAX(net_tx_max),
            AVG(io_read_avg), MAX(io_read_max),
            AVG(io_write_avg), MAX(io_write_max),
            SUM(sample_count)
        FROM metrics_server_1h
        WHERE bucket_ts < ?
        GROUP BY b_ts
        "#;
        let s_res = sqlx::query(s_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let t_query = r#"
        INSERT OR REPLACE INTO metrics_tenant_1d (
            bucket_ts, uid, cpu_avg, cpu_max,
            memory_avg, memory_max, pids_avg, pids_max,
            io_read_avg, io_write_avg, sample_count
        )
        SELECT
            (bucket_ts / 86400) * 86400 AS b_ts,
            uid,
            AVG(cpu_avg), MAX(cpu_max),
            AVG(memory_avg), MAX(memory_max),
            AVG(pids_avg), MAX(pids_max),
            AVG(io_read_avg), AVG(io_write_avg),
            SUM(sample_count)
        FROM metrics_tenant_1h
        WHERE bucket_ts < ?
        GROUP BY b_ts, uid
        "#;
        let t_res = sqlx::query(t_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let svc_query = r#"
        INSERT OR REPLACE INTO metrics_service_1d (
            bucket_ts, service_name, cpu_avg, cpu_max,
            memory_avg, memory_max, pids_avg, pids_max,
            io_read_avg, io_write_avg, sample_count
        )
        SELECT
            (bucket_ts / 86400) * 86400 AS b_ts,
            service_name,
            AVG(cpu_avg), MAX(cpu_max),
            AVG(memory_avg), MAX(memory_max),
            AVG(pids_avg), MAX(pids_max),
            AVG(io_read_avg), AVG(io_write_avg),
            SUM(sample_count)
        FROM metrics_service_1h
        WHERE bucket_ts < ?
        GROUP BY b_ts, service_name
        "#;
        let svc_res = sqlx::query(svc_query)
            .bind(max_bucket)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        Ok((
            s_res.rows_affected() as usize,
            t_res.rows_affected() as usize,
            svc_res.rows_affected() as usize,
        ))
    }

    /// Prune expired metrics according to configured retention periods.
    pub async fn prune_expired(
        &self,
        current_timestamp: i64,
        retention: &RetentionConfig,
    ) -> Result<usize, AnalyticsError> {
        let mut total_deleted = 0usize;

        // 1. Raw records (default 2 hours)
        let raw_cutoff = current_timestamp - retention.raw_retention_secs;
        let q1 = sqlx::query("DELETE FROM metrics_server_raw WHERE timestamp < ?")
            .bind(raw_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        let q2 = sqlx::query("DELETE FROM metrics_tenant_raw WHERE timestamp < ?")
            .bind(raw_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        let q3 = sqlx::query("DELETE FROM metrics_service_raw WHERE timestamp < ?")
            .bind(raw_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        total_deleted += (q1.rows_affected() + q2.rows_affected() + q3.rows_affected()) as usize;

        // 2. 1-minute records (default 14 days)
        let m1_cutoff = current_timestamp - retention.one_minute_retention_secs;
        let q4 = sqlx::query("DELETE FROM metrics_server_1m WHERE bucket_ts < ?")
            .bind(m1_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        let q5 = sqlx::query("DELETE FROM metrics_tenant_1m WHERE bucket_ts < ?")
            .bind(m1_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        let q6 = sqlx::query("DELETE FROM metrics_service_1m WHERE bucket_ts < ?")
            .bind(m1_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        total_deleted += (q4.rows_affected() + q5.rows_affected() + q6.rows_affected()) as usize;

        // 3. 1-hour records (default 400 days)
        let h1_cutoff = current_timestamp - retention.one_hour_retention_secs;
        let q7 = sqlx::query("DELETE FROM metrics_server_1h WHERE bucket_ts < ?")
            .bind(h1_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        let q8 = sqlx::query("DELETE FROM metrics_tenant_1h WHERE bucket_ts < ?")
            .bind(h1_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        let q9 = sqlx::query("DELETE FROM metrics_service_1h WHERE bucket_ts < ?")
            .bind(h1_cutoff)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        total_deleted += (q7.rows_affected() + q8.rows_affected() + q9.rows_affected()) as usize;

        // 4. 1-day records (optional)
        if let Some(d1_secs) = retention.one_day_retention_secs {
            let d1_cutoff = current_timestamp - d1_secs;
            let q10 = sqlx::query("DELETE FROM metrics_server_1d WHERE bucket_ts < ?")
                .bind(d1_cutoff)
                .execute(&self.pool)
                .await
                .map_err(AnalyticsError::Database)?;
            let q11 = sqlx::query("DELETE FROM metrics_tenant_1d WHERE bucket_ts < ?")
                .bind(d1_cutoff)
                .execute(&self.pool)
                .await
                .map_err(AnalyticsError::Database)?;
            let q12 = sqlx::query("DELETE FROM metrics_service_1d WHERE bucket_ts < ?")
                .bind(d1_cutoff)
                .execute(&self.pool)
                .await
                .map_err(AnalyticsError::Database)?;
            total_deleted +=
                (q10.rows_affected() + q11.rows_affected() + q12.rows_affected()) as usize;
        }

        Ok(total_deleted)
    }

    /// Query server rollups within time range.
    pub async fn query_server_rollups(
        &self,
        start_ts: i64,
        end_ts: i64,
        resolution: RollupResolution,
    ) -> Result<Vec<ServerMetricsRollup>, AnalyticsError> {
        let table = match resolution {
            RollupResolution::Raw | RollupResolution::OneMinute => "metrics_server_1m",
            RollupResolution::OneHour => "metrics_server_1h",
            RollupResolution::OneDay => "metrics_server_1d",
        };

        let sql = format!(
            "SELECT * FROM {table} WHERE bucket_ts >= ? AND bucket_ts <= ? ORDER BY bucket_ts ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(ServerMetricsRollup {
                bucket_timestamp: row.get("bucket_ts"),
                cpu_avg: row.get("cpu_avg"),
                cpu_max: row.get("cpu_max"),
                cpu_min: row.get("cpu_min"),
                memory_used_avg: row.get("memory_used_avg"),
                memory_used_max: row.get::<i64, _>("memory_used_max") as u64,
                swap_used_avg: row.get("swap_used_avg"),
                swap_used_max: row.get::<i64, _>("swap_used_max") as u64,
                load1_avg: row.get("load1_avg"),
                load1_max: row.get("load1_max"),
                disk_used_avg: row.get("disk_used_avg"),
                disk_used_max: row.get::<i64, _>("disk_used_max") as u64,
                net_rx_avg: row.get("net_rx_avg"),
                net_rx_max: row.get("net_rx_max"),
                net_tx_avg: row.get("net_tx_avg"),
                net_tx_max: row.get("net_tx_max"),
                io_read_avg: row.get("io_read_avg"),
                io_read_max: row.get("io_read_max"),
                io_write_avg: row.get("io_write_avg"),
                io_write_max: row.get("io_write_max"),
                sample_count: row.get::<i64, _>("sample_count") as u32,
            });
        }

        Ok(results)
    }

    /// Query tenant rollups within time range.
    pub async fn query_tenant_rollups(
        &self,
        uid: u32,
        start_ts: i64,
        end_ts: i64,
        resolution: RollupResolution,
    ) -> Result<Vec<TenantMetricsRollup>, AnalyticsError> {
        let table = match resolution {
            RollupResolution::Raw | RollupResolution::OneMinute => "metrics_tenant_1m",
            RollupResolution::OneHour => "metrics_tenant_1h",
            RollupResolution::OneDay => "metrics_tenant_1d",
        };

        let sql = format!(
            "SELECT * FROM {table} WHERE uid = ? AND bucket_ts >= ? AND bucket_ts <= ? ORDER BY bucket_ts ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(uid as i64)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(TenantMetricsRollup {
                bucket_timestamp: row.get("bucket_ts"),
                uid: row.get::<i64, _>("uid") as u32,
                cpu_avg: row.get("cpu_avg"),
                cpu_max: row.get("cpu_max"),
                memory_avg: row.get("memory_avg"),
                memory_max: row.get::<i64, _>("memory_max") as u64,
                pids_avg: row.get("pids_avg"),
                pids_max: row.get::<i64, _>("pids_max") as u64,
                io_read_avg: row.get("io_read_avg"),
                io_write_avg: row.get("io_write_avg"),
                sample_count: row.get::<i64, _>("sample_count") as u32,
            });
        }

        Ok(results)
    }

    /// Query service rollups within time range.
    pub async fn query_service_rollups(
        &self,
        service_name: &str,
        start_ts: i64,
        end_ts: i64,
        resolution: RollupResolution,
    ) -> Result<Vec<ServiceMetricsRollup>, AnalyticsError> {
        let table = match resolution {
            RollupResolution::Raw | RollupResolution::OneMinute => "metrics_service_1m",
            RollupResolution::OneHour => "metrics_service_1h",
            RollupResolution::OneDay => "metrics_service_1d",
        };

        let sql = format!(
            "SELECT * FROM {table} WHERE service_name = ? AND bucket_ts >= ? AND bucket_ts <= ? ORDER BY bucket_ts ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(service_name)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(ServiceMetricsRollup {
                bucket_timestamp: row.get("bucket_ts"),
                service_name: row.get("service_name"),
                cpu_avg: row.get("cpu_avg"),
                cpu_max: row.get("cpu_max"),
                memory_avg: row.get("memory_avg"),
                memory_max: row.get::<i64, _>("memory_max") as u64,
                pids_avg: row.get("pids_avg"),
                pids_max: row.get::<i64, _>("pids_max") as u64,
                io_read_avg: row.get("io_read_avg"),
                io_write_avg: row.get("io_write_avg"),
                sample_count: row.get::<i64, _>("sample_count") as u32,
            });
        }

        Ok(results)
    }

    /// Retrieve stored checkpoint for a log file path.
    pub async fn get_log_checkpoint(
        &self,
        file_path: &str,
    ) -> Result<Option<LogCheckpoint>, AnalyticsError> {
        let query = "SELECT file_path, site_id, inode, file_offset, last_processed_ts, updated_at FROM log_tailer_checkpoints WHERE file_path = ?";
        let row = sqlx::query(query)
            .bind(file_path)
            .fetch_optional(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        if let Some(r) = row {
            let inode_i64: i64 = r.try_get("inode").map_err(AnalyticsError::Database)?;
            let offset_i64: i64 = r.try_get("file_offset").map_err(AnalyticsError::Database)?;
            Ok(Some(LogCheckpoint {
                file_path: r.try_get("file_path").map_err(AnalyticsError::Database)?,
                site_id: r.try_get("site_id").map_err(AnalyticsError::Database)?,
                inode: inode_i64.max(0) as u64,
                file_offset: offset_i64.max(0) as u64,
                last_processed_ts: r
                    .try_get("last_processed_ts")
                    .map_err(AnalyticsError::Database)?,
                updated_at: r.try_get("updated_at").map_err(AnalyticsError::Database)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Persist or update a log file checkpoint.
    pub async fn save_log_checkpoint(
        &self,
        checkpoint: &LogCheckpoint,
    ) -> Result<(), AnalyticsError> {
        let query = r#"
        INSERT INTO log_tailer_checkpoints (file_path, site_id, inode, file_offset, last_processed_ts, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(file_path) DO UPDATE SET
            site_id = excluded.site_id,
            inode = excluded.inode,
            file_offset = excluded.file_offset,
            last_processed_ts = excluded.last_processed_ts,
            updated_at = excluded.updated_at
        "#;

        sqlx::query(query)
            .bind(&checkpoint.file_path)
            .bind(checkpoint.site_id)
            .bind(checkpoint.inode as i64)
            .bind(checkpoint.file_offset as i64)
            .bind(checkpoint.last_processed_ts)
            .bind(checkpoint.updated_at)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        Ok(())
    }

    /// List all log file checkpoints.
    pub async fn list_log_checkpoints(&self) -> Result<Vec<LogCheckpoint>, AnalyticsError> {
        let query = "SELECT file_path, site_id, inode, file_offset, last_processed_ts, updated_at FROM log_tailer_checkpoints ORDER BY file_path ASC";
        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;

        let mut res = Vec::with_capacity(rows.len());
        for r in rows {
            let inode_i64: i64 = r.try_get("inode").map_err(AnalyticsError::Database)?;
            let offset_i64: i64 = r.try_get("file_offset").map_err(AnalyticsError::Database)?;
            res.push(LogCheckpoint {
                file_path: r.try_get("file_path").map_err(AnalyticsError::Database)?,
                site_id: r.try_get("site_id").map_err(AnalyticsError::Database)?,
                inode: inode_i64.max(0) as u64,
                file_offset: offset_i64.max(0) as u64,
                last_processed_ts: r
                    .try_get("last_processed_ts")
                    .map_err(AnalyticsError::Database)?,
                updated_at: r.try_get("updated_at").map_err(AnalyticsError::Database)?,
            });
        }
        Ok(res)
    }

    /// Delete a log checkpoint when a site is removed.
    pub async fn delete_log_checkpoint(&self, file_path: &str) -> Result<(), AnalyticsError> {
        let query = "DELETE FROM log_tailer_checkpoints WHERE file_path = ?";
        sqlx::query(query)
            .bind(file_path)
            .execute(&self.pool)
            .await
            .map_err(AnalyticsError::Database)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_lifecycle_and_rollups() {
        let storage = AnalyticsStorage::connect_in_memory()
            .await
            .expect("in-memory db");

        // Insert 6 raw points across 1 minute (every 10 seconds)
        let base_ts = 1770000000;
        for i in 0..6 {
            let ts = base_ts + (i * 10);
            storage
                .insert_server_point(&ServerMetricsPoint {
                    timestamp: ts,
                    cpu_percent: 10.0 + (i as f64 * 2.0),
                    cpu_cores: 4,
                    memory_used_bytes: 1000 + i as u64 * 100,
                    memory_total_bytes: 8000,
                    swap_used_bytes: 0,
                    swap_total_bytes: 2000,
                    load1: 0.5,
                    load5: 0.4,
                    load15: 0.3,
                    disk_used_bytes: 5000,
                    disk_total_bytes: 20000,
                    inodes_used: 100,
                    inodes_total: 1000,
                    net_rx_bytes_sec: 1024.0,
                    net_tx_bytes_sec: 2048.0,
                    io_read_bytes_sec: 512.0,
                    io_write_bytes_sec: 256.0,
                })
                .await
                .expect("insert server");

            storage
                .insert_tenant_points(&[TenantMetricsPoint {
                    timestamp: ts,
                    uid: 10001,
                    cpu_usage_usec: 100000 * (i as u64 + 1),
                    cpu_percent: 5.0 + i as f64,
                    memory_current_bytes: 500000 + i as u64 * 10000,
                    memory_peak_bytes: Some(600000),
                    memory_swap_bytes: None,
                    pids_current: 5 + i as u64,
                    io_read_bytes_sec: 100.0,
                    io_write_bytes_sec: 200.0,
                }])
                .await
                .expect("insert tenant");

            storage
                .insert_service_points(&[ServiceMetricsPoint {
                    timestamp: ts,
                    service_name: "nginx".to_string(),
                    cpu_usage_usec: 50000 * (i as u64 + 1),
                    cpu_percent: 2.0 + i as f64,
                    memory_current_bytes: 200000,
                    pids_current: 4,
                    io_read_bytes_sec: 50.0,
                    io_write_bytes_sec: 100.0,
                }])
                .await
                .expect("insert service");
        }

        // Run 1m rollup for timestamp base_ts + 60
        let (s_1m, t_1m, svc_1m) = storage.rollup_1m(base_ts + 60).await.expect("rollup 1m");
        assert_eq!(s_1m, 1);
        assert_eq!(t_1m, 1);
        assert_eq!(svc_1m, 1);

        // Query server rollups
        let server_rollups = storage
            .query_server_rollups(base_ts - 10, base_ts + 100, RollupResolution::OneMinute)
            .await
            .expect("query server rollups");
        assert_eq!(server_rollups.len(), 1);
        assert_eq!(server_rollups[0].sample_count, 6);
        assert_eq!(server_rollups[0].cpu_min, 10.0);
        assert_eq!(server_rollups[0].cpu_max, 20.0);
        assert_eq!(server_rollups[0].cpu_avg, 15.0);

        // Query tenant rollups
        let tenant_rollups = storage
            .query_tenant_rollups(
                10001,
                base_ts - 10,
                base_ts + 100,
                RollupResolution::OneMinute,
            )
            .await
            .expect("query tenant rollups");
        assert_eq!(tenant_rollups.len(), 1);
        assert_eq!(tenant_rollups[0].sample_count, 6);
        assert_eq!(tenant_rollups[0].uid, 10001);

        // Query service rollups
        let svc_rollups = storage
            .query_service_rollups(
                "nginx",
                base_ts - 10,
                base_ts + 100,
                RollupResolution::OneMinute,
            )
            .await
            .expect("query service rollups");
        assert_eq!(svc_rollups.len(), 1);
        assert_eq!(svc_rollups[0].service_name, "nginx");

        // Run 1h rollup
        let (s_1h, t_1h, svc_1h) = storage.rollup_1h(base_ts + 3600).await.expect("rollup 1h");
        assert_eq!(s_1h, 1);
        assert_eq!(t_1h, 1);
        assert_eq!(svc_1h, 1);

        // Prune test: with cutoff right after base_ts + 100
        let retention = RetentionConfig {
            raw_retention_secs: 10,
            one_minute_retention_secs: 10,
            one_hour_retention_secs: 10,
            one_day_retention_secs: None,
        };
        let deleted = storage
            .prune_expired(base_ts + 200, &retention)
            .await
            .expect("prune");
        // 6 server raw + 6 tenant raw + 6 service raw + 1 server 1m + 1 tenant 1m + 1 svc 1m + 1 server 1h + 1 tenant 1h + 1 svc 1h = 24
        assert!(deleted >= 18);
    }
}
