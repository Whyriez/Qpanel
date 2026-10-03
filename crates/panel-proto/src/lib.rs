//! `panel-proto`: RPC protocol types, versioning, and length-delimited framing.

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Protocol version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },
    #[error("Connection closed prematurely")]
    ConnectionClosed,
    #[error("Agent error: {0}")]
    AgentError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcRequest {
    pub proto_version: u32,
    pub request_id: String,
    pub payload: RequestPayload,
}

impl RpcRequest {
    pub fn new(request_id: impl Into<String>, payload: RequestPayload) -> Self {
        Self {
            proto_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", content = "data")]
pub enum RequestPayload {
    Ping,
    Version,
    UserCreate(UserCreateRequest),
    UserSuspend(UserSuspendRequest),
    UserUnsuspend(UserUnsuspendRequest),
    UserDelete(UserDeleteRequest),
    SliceApply(SliceApplyRequest),
    SliceRemove(SliceRemoveRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceApplyRequest {
    pub uid: u32,
    pub memory_max: Option<String>,
    pub memory_high: Option<String>,
    pub cpu_quota: Option<String>,
    pub tasks_max: Option<u64>,
    pub io_weight: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceRemoveRequest {
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserCreateRequest {
    pub username: String,
    pub uid: u32,
    pub shell: Option<String>,
    pub home_dir: Option<String>,
    pub web_user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSuspendRequest {
    pub username: String,
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserUnsuspendRequest {
    pub username: String,
    pub uid: u32,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserDeleteRequest {
    pub username: String,
    pub uid: u32,
    pub purge: bool,
    pub trash_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub proto_version: u32,
    pub request_id: String,
    pub success: bool,
    pub payload: ResponsePayload,
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn ok(request_id: impl Into<String>, payload: ResponsePayload) -> Self {
        Self {
            proto_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            success: true,
            payload,
            error: None,
        }
    }

    pub fn err(request_id: impl Into<String>, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            proto_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            success: false,
            payload: ResponsePayload::Error(msg.clone()),
            error: Some(msg),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ResponsePayload {
    Pong,
    Version(VersionInfo),
    UserCreated(UserCreatedResponse),
    UserSuspended(UserSuspendedResponse),
    UserUnsuspended(UserUnsuspendedResponse),
    UserDeleted(UserDeletedResponse),
    SliceApplied(SliceAppliedResponse),
    SliceRemoved(SliceRemovedResponse),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserCreatedResponse {
    pub username: String,
    pub uid: u32,
    pub home_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSuspendedResponse {
    pub username: String,
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserUnsuspendedResponse {
    pub username: String,
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserDeletedResponse {
    pub username: String,
    pub uid: u32,
    pub purged: bool,
    pub archived_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceAppliedResponse {
    pub uid: u32,
    pub slice_name: String,
    pub dropin_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceRemovedResponse {
    pub uid: u32,
    pub slice_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionInfo {
    pub version: String,
    pub proto_version: u32,
    pub os: String,
    pub arch: String,
}

/// Helper to wrap any async IO stream with length-delimited JSON RPC framing.
pub struct RpcStream<S> {
    inner: Framed<S, LengthDelimitedCodec>,
}

impl<S> RpcStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S) -> Self {
        let codec = LengthDelimitedCodec::builder()
            .max_frame_length(16 * 1024 * 1024) // 16 MB max frame
            .new_codec();
        Self {
            inner: Framed::new(stream, codec),
        }
    }

    pub async fn send_request(&mut self, req: &RpcRequest) -> Result<(), ProtoError> {
        let bytes = serde_json::to_vec(req)?;
        self.inner.send(Bytes::from(bytes)).await?;
        Ok(())
    }

    pub async fn recv_request(&mut self) -> Result<RpcRequest, ProtoError> {
        match self.inner.next().await {
            Some(Ok(bytes)) => {
                let req: RpcRequest = serde_json::from_slice(&bytes)?;
                if req.proto_version != PROTOCOL_VERSION {
                    return Err(ProtoError::VersionMismatch {
                        expected: PROTOCOL_VERSION,
                        got: req.proto_version,
                    });
                }
                Ok(req)
            }
            Some(Err(e)) => Err(ProtoError::Io(e)),
            None => Err(ProtoError::ConnectionClosed),
        }
    }

    pub async fn send_response(&mut self, res: &RpcResponse) -> Result<(), ProtoError> {
        let bytes = serde_json::to_vec(res)?;
        self.inner.send(Bytes::from(bytes)).await?;
        Ok(())
    }

    pub async fn recv_response(&mut self) -> Result<RpcResponse, ProtoError> {
        match self.inner.next().await {
            Some(Ok(bytes)) => {
                let res: RpcResponse = serde_json::from_slice(&bytes)?;
                Ok(res)
            }
            Some(Err(e)) => Err(ProtoError::Io(e)),
            None => Err(ProtoError::ConnectionClosed),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpc_serialization_ping() {
        let req = RpcRequest::new("req-1", RequestPayload::Ping);
        let serialized = serde_json::to_string(&req).expect("serialize");
        let deserialized: RpcRequest = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(req, deserialized);
    }

    #[tokio::test]
    async fn test_rpc_roundtrip_stream() {
        let (client_io, server_io) = tokio::io::duplex(1024);
        let mut client_rpc = RpcStream::new(client_io);
        let mut server_rpc = RpcStream::new(server_io);

        let client_task = tokio::spawn(async move {
            let req = RpcRequest::new("test-123", RequestPayload::Ping);
            client_rpc.send_request(&req).await.expect("send request");
            let res = client_rpc.recv_response().await.expect("recv response");
            assert_eq!(res.request_id, "test-123");
            assert_eq!(res.payload, ResponsePayload::Pong);
        });

        let server_task = tokio::spawn(async move {
            let req = server_rpc.recv_request().await.expect("recv request");
            assert_eq!(req.payload, RequestPayload::Ping);
            let res = RpcResponse::ok(req.request_id, ResponsePayload::Pong);
            server_rpc.send_response(&res).await.expect("send response");
        });

        let _ = tokio::try_join!(client_task, server_task).expect("roundtrip successful");
    }

    #[test]
    fn test_rpc_user_lifecycle_payloads() {
        let req = RpcRequest::new(
            "user-1",
            RequestPayload::UserCreate(UserCreateRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                shell: Some("/usr/sbin/nologin".to_string()),
                home_dir: Some("/home/p_alice".to_string()),
                web_user: Some("www-data".to_string()),
            }),
        );
        let s = serde_json::to_string(&req).expect("serialize UserCreate");
        let de: RpcRequest = serde_json::from_str(&s).expect("deserialize UserCreate");
        assert_eq!(req, de);

        let res = RpcResponse::ok(
            "user-1",
            ResponsePayload::UserCreated(UserCreatedResponse {
                username: "p_alice".to_string(),
                uid: 10001,
                home_dir: "/home/p_alice".to_string(),
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize UserCreated response");
        let de_res: RpcResponse = serde_json::from_str(&s_res).expect("deserialize UserCreated");
        assert_eq!(res, de_res);
    }

    #[test]
    fn test_rpc_slice_payloads() {
        let req = RpcRequest::new(
            "slice-1",
            RequestPayload::SliceApply(SliceApplyRequest {
                uid: 10001,
                memory_max: Some("1G".to_string()),
                memory_high: Some("800M".to_string()),
                cpu_quota: Some("150%".to_string()),
                tasks_max: Some(250),
                io_weight: Some(100),
            }),
        );
        let s = serde_json::to_string(&req).expect("serialize SliceApply");
        let de: RpcRequest = serde_json::from_str(&s).expect("deserialize SliceApply");
        assert_eq!(req, de);

        let res = RpcResponse::ok(
            "slice-1",
            ResponsePayload::SliceApplied(SliceAppliedResponse {
                uid: 10001,
                slice_name: "panel-u10001.slice".to_string(),
                dropin_path: "/etc/systemd/system/panel-u10001.slice.d/50-limits.conf".to_string(),
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize SliceApplied");
        let de_res: RpcResponse = serde_json::from_str(&s_res).expect("deserialize SliceApplied");
        assert_eq!(res, de_res);

        let rem_req = RpcRequest::new(
            "slice-2",
            RequestPayload::SliceRemove(SliceRemoveRequest { uid: 10001 }),
        );
        let s_rem = serde_json::to_string(&rem_req).expect("serialize SliceRemove");
        let de_rem: RpcRequest = serde_json::from_str(&s_rem).expect("deserialize SliceRemove");
        assert_eq!(rem_req, de_rem);
    }
}
