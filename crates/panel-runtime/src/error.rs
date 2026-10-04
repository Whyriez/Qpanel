//! Error types for runtime management.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error(
        "Versi PHP '{0}' tidak didukung atau tidak valid. Versi yang didukung: 7.4, 8.0, 8.1, 8.2, 8.3, 8.4"
    )]
    UnsupportedPhpVersion(String),

    #[error("Perintah sistem gagal: {0}")]
    SystemCommandFailed(String),

    #[error("Gagal melakukan masking pada systemd service: {0}")]
    ServiceMaskFailed(String),

    #[error("Versi runtime sedang digunakan oleh situs dan tidak dapat dihapus: {0}")]
    RuntimeInUse(String),

    #[error("Ekstensi PHP '{0}' tidak valid atau tidak termasuk dalam daftar kurasi.")]
    UnsupportedPhpExtension(String),

    #[error("Operasi I/O gagal: {0}")]
    Io(String),
}
