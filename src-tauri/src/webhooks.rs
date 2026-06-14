//! Outbound webhook signing and dispatch.

use std::time::Duration;

use chrono::Utc;
use ring::hmac;
use serde_json::{json, Value};

pub const SIGNATURE_HEADER: &str = "AgentDeck-Signature";
pub const USER_AGENT: &str = "AgentDeck-Webhooks/0.1";

pub const EVENT_TYPES: &[&str] = &[
    "test.ping",
    "handoff.completed",
    "handoff.failed",
    "skill.completed",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResponse {
    pub status_code: u16,
    pub detail: String,
}

pub fn validate_event_type(event_type: &str) -> Result<(), String> {
    if EVENT_TYPES.contains(&event_type) {
        Ok(())
    } else {
        Err(format!("unsupported webhook event type: {event_type}"))
    }
}

pub fn validate_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 2048 {
        return Err("webhook URL must contain between 1 and 2048 characters".to_owned());
    }
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|error| format!("webhook URL is invalid: {error}"))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host(parsed.host_str()) => Ok(()),
        "http" => Err("webhook URL must use HTTPS unless targeting loopback".to_owned()),
        other => Err(format!("webhook URL scheme {other} is not supported")),
    }
}

pub fn build_envelope(event_type: &str, data: Value) -> Value {
    json!({
        "event": event_type,
        "timestamp": Utc::now().to_rfc3339(),
        "source": "agentdeck",
        "data": data,
    })
}

pub fn sign_payload(secret: &str, timestamp: &str, body: &str) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let signed = hmac::sign(&key, format!("{timestamp}.{body}").as_bytes());
    base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        signed.as_ref(),
    )
}

pub fn format_signature_header(timestamp: &str, signature: &str) -> String {
    format!("t={timestamp},v1={signature}")
}

pub fn dispatch_outbound(
    url: &str,
    secret: Option<&str>,
    event_type: &str,
    data: Value,
) -> Result<DispatchResponse, String> {
    validate_url(url)?;
    validate_event_type(event_type)?;

    let envelope = build_envelope(event_type, data);
    let body = serde_json::to_string(&envelope)
        .map_err(|error| format!("failed to encode webhook payload: {error}"))?;
    let timestamp = Utc::now().timestamp().to_string();

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create webhook client: {error}"))?;

    let mut request = client
        .post(url.trim())
        .header("Content-Type", "application/json")
        .header("X-AgentDeck-Event", event_type);
    if let Some(secret) = secret.filter(|value| !value.trim().is_empty()) {
        let signature = sign_payload(secret, &timestamp, &body);
        request = request.header(
            SIGNATURE_HEADER,
            format_signature_header(&timestamp, &signature),
        );
    }
    let response = request
        .body(body)
        .send()
        .map_err(|error| format!("webhook request failed: {error}"))?;
    let status_code = response.status().as_u16();
    let detail = if status_code >= 200 && status_code < 300 {
        format!("HTTP {status_code}")
    } else {
        let body = response
            .text()
            .unwrap_or_default()
            .chars()
            .take(240)
            .collect::<String>();
        if body.is_empty() {
            format!("HTTP {status_code}")
        } else {
            format!("HTTP {status_code}: {body}")
        }
    };

    Ok(DispatchResponse { status_code, detail })
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost") | Some("127.0.0.1") | Some("::1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_supported_event_types() {
        assert!(validate_event_type("test.ping").is_ok());
        assert!(validate_event_type("unknown.event").is_err());
    }

    #[test]
    fn accepts_https_and_loopback_http_urls() {
        assert!(validate_url("https://hooks.example.com/agentdeck").is_ok());
        assert!(validate_url("http://127.0.0.1:8787/webhook").is_ok());
        assert!(validate_url("http://localhost:3000/hooks/agentdeck").is_ok());
    }

    #[test]
    fn rejects_non_loopback_http_urls() {
        assert!(validate_url("http://example.com/hook").is_err());
    }

    #[test]
    fn signature_is_deterministic_for_same_inputs() {
        let first = sign_payload("secret", "1710000000", r#"{"event":"test.ping"}"#);
        let second = sign_payload("secret", "1710000000", r#"{"event":"test.ping"}"#);
        assert_eq!(first, second);
        assert_ne!(first, sign_payload("secret", "1710000001", r#"{"event":"test.ping"}"#));
    }

    #[test]
    fn envelope_includes_event_metadata() {
        let envelope = build_envelope("test.ping", json!({ "message": "hello" }));
        assert_eq!(envelope["event"], "test.ping");
        assert_eq!(envelope["source"], "agentdeck");
        assert_eq!(envelope["data"]["message"], "hello");
    }
}