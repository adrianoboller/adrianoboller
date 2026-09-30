#![forbid(unsafe_code)]

use chrono::Utc;
use phxclaw_claw_code_bridge::{ClawCodeBridge, ClawCodeBridgeConfig};
use phxclaw_process_protocol::{
    ProcessEnvelope, ProcessErrorBody, ProcessMessageKind, ProcessReply, ProcessReplyStatus,
    PROCESS_PROTOCOL_V1,
};
use phxclaw_types::new_uuid_v7;
use serde_json::{json, Value};
use std::io::{Read, Write};

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(std::io::stderr(), "{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let line = input.lines().find(|line| !line.trim().is_empty()).ok_or("empty request")?;
    let request: ProcessEnvelope<Value> = serde_json::from_str(line)?;
    if request.protocol != PROCESS_PROTOCOL_V1 {
        return Err(format!("unsupported protocol {}", request.protocol).into());
    }

    let bridge = ClawCodeBridge::new(ClawCodeBridgeConfig::from_env());
    let reply = match request.kind {
        ProcessMessageKind::Health => match bridge.health() {
            Ok(payload) => ok_reply(request.message_uuid, payload),
            Err(error) => error_reply(request.message_uuid, "claw_unavailable", &error.to_string(), true),
        },
        ProcessMessageKind::Execute => {
            let capability = request.payload.get("capability").and_then(Value::as_str).unwrap_or_default();
            let payload = request.payload.get("payload").cloned().unwrap_or_else(|| json!({}));
            match bridge.invoke(capability, &payload) {
                Ok(result) if result.exit_code == 0 => ok_reply(request.message_uuid, serde_json::to_value(result)?),
                Ok(result) => ProcessReply {
                    protocol: PROCESS_PROTOCOL_V1.into(),
                    message_uuid: new_uuid_v7(),
                    correlation_uuid: request.message_uuid,
                    status: ProcessReplyStatus::Error,
                    emitted_at: Utc::now(),
                    payload: serde_json::to_value(&result)?,
                    error: Some(ProcessErrorBody {
                        code: "claw_command_failed".into(),
                        message: format!("Claw Code exited with {}", result.exit_code),
                        retryable: false,
                    }),
                },
                Err(error) => error_reply(request.message_uuid, "claw_bridge_error", &error.to_string(), false),
            }
        }
        ProcessMessageKind::Cancel => error_reply(
            request.message_uuid,
            "cancel_not_supported",
            "Claw bridge operations are one-shot child processes",
            false,
        ),
        ProcessMessageKind::Shutdown => ok_reply(request.message_uuid, json!({"shutdown": true})),
    };

    serde_json::to_writer(std::io::stdout(), &reply)?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}

fn ok_reply(correlation_uuid: uuid::Uuid, payload: Value) -> ProcessReply<Value> {
    ProcessReply {
        protocol: PROCESS_PROTOCOL_V1.into(),
        message_uuid: new_uuid_v7(),
        correlation_uuid,
        status: ProcessReplyStatus::Ok,
        emitted_at: Utc::now(),
        payload,
        error: None,
    }
}

fn error_reply(
    correlation_uuid: uuid::Uuid,
    code: &str,
    message: &str,
    retryable: bool,
) -> ProcessReply<Value> {
    ProcessReply {
        protocol: PROCESS_PROTOCOL_V1.into(),
        message_uuid: new_uuid_v7(),
        correlation_uuid,
        status: ProcessReplyStatus::Error,
        emitted_at: Utc::now(),
        payload: json!({}),
        error: Some(ProcessErrorBody {
            code: code.into(),
            message: message.into(),
            retryable,
        }),
    }
}
