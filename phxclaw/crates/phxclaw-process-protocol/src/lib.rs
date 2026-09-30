use chrono::{DateTime, Utc};
use phxclaw_sandbox::{SandboxBackend, SandboxError};
use phxclaw_types::{new_uuid_v7, PluginManifest};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    thread,
    time::Instant,
};
use thiserror::Error;
use uuid::Uuid;

pub const PROCESS_PROTOCOL_V1: &str = "phxclaw-process-v1";
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessMessageKind {
    Health,
    Execute,
    Cancel,
    Shutdown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessReplyStatus {
    Ok,
    Error,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEnvelope<T = Value> {
    pub protocol: String,
    pub message_uuid: Uuid,
    pub correlation_uuid: Option<Uuid>,
    pub kind: ProcessMessageKind,
    pub sent_at: DateTime<Utc>,
    pub payload: T,
}

impl<T> ProcessEnvelope<T> {
    pub fn new(kind: ProcessMessageKind, payload: T) -> Self {
        Self {
            protocol: PROCESS_PROTOCOL_V1.into(),
            message_uuid: new_uuid_v7(),
            correlation_uuid: None,
            kind,
            sent_at: Utc::now(),
            payload,
        }
    }

    pub fn correlate(mut self, uuid: Uuid) -> Self {
        self.correlation_uuid = Some(uuid);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessReply<T = Value> {
    pub protocol: String,
    pub message_uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub status: ProcessReplyStatus,
    pub emitted_at: DateTime<Utc>,
    pub payload: T,
    pub error: Option<ProcessErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Error)]
pub enum ProcessProtocolError {
    #[error("sandbox failure: {0}")]
    Sandbox(#[from] SandboxError),
    #[error("process I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("process JSON failure: {0}")]
    Json(#[from] serde_json::Error),
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    FrameTooLarge,
    #[error("unsupported process protocol: {0}")]
    UnsupportedProtocol(String),
    #[error("process returned no reply")]
    EmptyReply,
    #[error("process returned multiple frames where one was expected")]
    MultipleReplies,
    #[error("process exited unsuccessfully: {0}")]
    ProcessFailed(String),
    #[error("correlation mismatch: expected {expected}, got {actual}")]
    CorrelationMismatch { expected: Uuid, actual: Uuid },
}

pub fn encode_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, ProcessProtocolError> {
    let mut bytes = serde_json::to_vec(message)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProcessProtocolError::FrameTooLarge);
    }
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn decode_frame<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProcessProtocolError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProcessProtocolError::FrameTooLarge);
    }
    Ok(serde_json::from_slice(bytes)?)
}

#[derive(Debug, Clone)]
pub struct ProcessRunner<B: SandboxBackend> {
    sandbox: B,
}

impl<B: SandboxBackend> ProcessRunner<B> {
    pub fn new(sandbox: B) -> Self {
        Self { sandbox }
    }

    pub fn request<TReq, TResp>(
        &self,
        manifest: &PluginManifest,
        envelope: &ProcessEnvelope<TReq>,
    ) -> Result<ProcessReply<TResp>, ProcessProtocolError>
    where
        TReq: Serialize,
        TResp: DeserializeOwned,
    {
        if envelope.protocol != PROCESS_PROTOCOL_V1 {
            return Err(ProcessProtocolError::UnsupportedProtocol(
                envelope.protocol.clone(),
            ));
        }

        self.sandbox.probe()?;
        let plan = self.sandbox.plan(manifest)?;
        let input = encode_frame(envelope)?;
        let mut child = Command::new(&plan.program)
            .args(&plan.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&input)?;
            stdin.flush()?;
        }

        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                let mut stdout = Vec::new();
                let mut stderr = String::new();
                if let Some(mut handle) = child.stdout.take() {
                    handle.read_to_end(&mut stdout)?;
                }
                if let Some(mut handle) = child.stderr.take() {
                    handle.read_to_string(&mut stderr)?;
                }
                if !status.success() {
                    return Err(ProcessProtocolError::ProcessFailed(if stderr.trim().is_empty() {
                        status.to_string()
                    } else {
                        stderr.trim().to_owned()
                    }));
                }
                if stdout.len() > MAX_FRAME_BYTES {
                    return Err(ProcessProtocolError::FrameTooLarge);
                }
                let lines = stdout
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>();
                if lines.is_empty() {
                    return Err(ProcessProtocolError::EmptyReply);
                }
                if lines.len() != 1 {
                    return Err(ProcessProtocolError::MultipleReplies);
                }
                let reply: ProcessReply<TResp> = decode_frame(lines[0])?;
                if reply.protocol != PROCESS_PROTOCOL_V1 {
                    return Err(ProcessProtocolError::UnsupportedProtocol(reply.protocol));
                }
                if reply.correlation_uuid != envelope.message_uuid {
                    return Err(ProcessProtocolError::CorrelationMismatch {
                        expected: envelope.message_uuid,
                        actual: reply.correlation_uuid,
                    });
                }
                return Ok(reply);
            }
            if started.elapsed() >= plan.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SandboxError::Timeout(plan.timeout).into());
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip_is_json_lines() {
        let request = ProcessEnvelope::new(
            ProcessMessageKind::Execute,
            serde_json::json!({"capability": "decision.technical"}),
        );
        let encoded = encode_frame(&request).unwrap();
        assert_eq!(encoded.last().copied(), Some(b'\n'));
        let decoded: ProcessEnvelope<Value> = decode_frame(&encoded[..encoded.len() - 1]).unwrap();
        assert_eq!(decoded.protocol, PROCESS_PROTOCOL_V1);
        assert_eq!(decoded.message_uuid, request.message_uuid);
    }
}
