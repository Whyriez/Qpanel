//! IPC client to communicate with `panel-agent` over a Unix domain socket.

use panel_proto::{
    ProtoError, RequestPayload, ResponsePayload, RpcRequest, RpcStream, VersionInfo,
};
use std::path::PathBuf;
use thiserror::Error;

#[cfg(unix)]
type IpcStream = tokio::net::UnixStream;
#[cfg(not(unix))]
type IpcStream = tokio::net::TcpStream;

#[derive(Debug, Error)]
pub enum AgentClientError {
    #[error("Failed to connect to agent socket at {path}: {source}")]
    Connect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Protocol error: {0}")]
    Proto(#[from] ProtoError),
    #[error("Agent returned error: {0}")]
    AgentError(String),
    #[error("Unexpected agent response: expected {expected}, got {got}")]
    UnexpectedResponse { expected: &'static str, got: String },
}

#[derive(Debug, Clone)]
pub struct AgentClient {
    socket_path: PathBuf,
}

impl AgentClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    async fn connect(&self) -> Result<RpcStream<IpcStream>, AgentClientError> {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| AgentClientError::Connect {
                    path: self.socket_path.clone(),
                    source: e,
                })?;
            Ok(RpcStream::new(stream))
        }
        #[cfg(not(unix))]
        {
            let port_str = if self.socket_path.exists() {
                std::fs::read_to_string(&self.socket_path).unwrap_or_else(|_| "7701".to_string())
            } else {
                "7701".to_string()
            };
            let port: u16 = port_str.trim().parse().unwrap_or(7701);
            let addr = format!("127.0.0.1:{}", port);
            let stream = tokio::net::TcpStream::connect(&addr).await.map_err(|e| {
                AgentClientError::Connect {
                    path: self.socket_path.clone(),
                    source: e,
                }
            })?;
            Ok(RpcStream::new(stream))
        }
    }

    pub async fn ping(&self) -> Result<(), AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "ping-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(req_id, RequestPayload::Ping);
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::Pong => Ok(()),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "Pong",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn version(&self) -> Result<VersionInfo, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "ver-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(req_id, RequestPayload::Version);
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::Version(info) => Ok(info),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "Version",
                got: format!("{other:?}"),
            }),
        }
    }
}
