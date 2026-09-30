#![forbid(unsafe_code)]

use chrono::Utc;
use phxclaw_process_protocol::{
    ProcessEnvelope, ProcessErrorBody, ProcessMessageKind, ProcessReply, ProcessReplyStatus,
    PROCESS_PROTOCOL_V1,
};
use phxclaw_rustclaw_native::{
    external_user_scope, negotiate_gateway_protocol, phoenix_tool_name, rustclaw_legacy_tool_name,
    validate_schedule, McpWireMode, NativeFeatureMap, ScheduleSpec, RUSTCLAW_UPSTREAM_LICENSE,
    RUSTCLAW_UPSTREAM_VERSION,
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

    let reply = match request.kind {
        ProcessMessageKind::Health => ok(request.message_uuid, json!({
            "healthy": true,
            "upstream": "RustClaw",
            "upstream_version": RUSTCLAW_UPSTREAM_VERSION,
            "upstream_license": RUSTCLAW_UPSTREAM_LICENSE,
            "features": NativeFeatureMap::default(),
        })),
        ProcessMessageKind::Execute => execute(request.message_uuid, &request.payload),
        ProcessMessageKind::Cancel => err(request.message_uuid, "cancel_not_supported", "native compatibility operations are immediate", false),
        ProcessMessageKind::Shutdown => ok(request.message_uuid, json!({"shutdown":true})),
    };

    serde_json::to_writer(std::io::stdout(), &reply)?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}

fn execute(correlation: uuid::Uuid, payload: &Value) -> ProcessReply<Value> {
    let capability = payload.get("capability").and_then(Value::as_str).unwrap_or_default();
    let body = payload.get("payload").cloned().unwrap_or_else(|| json!({}));
    match capability {
        "rustclaw.native.gateway.negotiate" => {
            let min = body.get("min_protocol").and_then(Value::as_u64).map(|v| v as u32);
            let max = body.get("max_protocol").and_then(Value::as_u64).map(|v| v as u32);
            let supported = body.get("supported").and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_u64).map(|v| v as u32).collect::<Vec<_>>())
                .unwrap_or_else(|| vec![1]);
            match negotiate_gateway_protocol(min, max, &supported) {
                Ok(version) => ok(correlation, json!({"protocol":version})),
                Err(error) => err(correlation, "gateway_protocol_mismatch", &error.to_string(), false),
            }
        }
        "rustclaw.native.session.scope" => {
            let channel = body.get("channel").and_then(Value::as_str).unwrap_or("unknown");
            let user = body.get("external_user_id").and_then(Value::as_str).unwrap_or("unknown");
            ok(correlation, json!({"scope":external_user_scope(channel,user)}))
        }
        "rustclaw.native.cron.validate" => {
            let schedule = if let Some(seconds) = body.get("every_seconds").and_then(Value::as_u64) {
                ScheduleSpec::EverySeconds(seconds)
            } else {
                ScheduleSpec::CronExpression(body.get("cron").and_then(Value::as_str).unwrap_or("").to_owned())
            };
            match validate_schedule(&schedule) {
                Ok(()) => ok(correlation, json!({"valid":true})),
                Err(error) => err(correlation, "invalid_schedule", &error.to_string(), false),
            }
        }
        "rustclaw.native.mcp.names" => {
            let server = body.get("server").and_then(Value::as_str).unwrap_or("server");
            let tool = body.get("tool").and_then(Value::as_str).unwrap_or("tool");
            ok(correlation, json!({
                "legacy":rustclaw_legacy_tool_name(server,tool),
                "phoenix":phoenix_tool_name(server,tool),
                "supported_wire_modes":[McpWireMode::ContentLength,McpWireMode::Ndjson],
            }))
        }
        _ => err(correlation, "unsupported_capability", &format!("unsupported native capability: {capability}"), false),
    }
}

fn ok(correlation_uuid: uuid::Uuid, payload: Value) -> ProcessReply<Value> {
    ProcessReply { protocol: PROCESS_PROTOCOL_V1.into(), message_uuid: new_uuid_v7(), correlation_uuid, status: ProcessReplyStatus::Ok, emitted_at: Utc::now(), payload, error: None }
}
fn err(correlation_uuid: uuid::Uuid, code: &str, message: &str, retryable: bool) -> ProcessReply<Value> {
    ProcessReply { protocol: PROCESS_PROTOCOL_V1.into(), message_uuid: new_uuid_v7(), correlation_uuid, status: ProcessReplyStatus::Error, emitted_at: Utc::now(), payload: json!({}), error: Some(ProcessErrorBody { code: code.into(), message: message.into(), retryable }) }
}
