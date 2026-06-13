use axum::{
    http::{header, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::env;
use tower::ServiceBuilder;

use crate::mcp_public_url;
use crate::mcp_server;
use crate::tunnel_control;

pub const MCP_HTTP_PORT: u16 = 7823;
const MAX_TOOL_RESPONSE_CHARS: usize = 64 * 1024;

pub fn start_http_server() {
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .thread_name("agentdeck-mcp-http")
            .build()
            .expect("failed to start MCP HTTP runtime");

        runtime.block_on(async {
            let mcp_methods = post(handle_mcp_post)
                .get(handle_mcp_method_not_allowed)
                .delete(handle_mcp_method_not_allowed);

            let app = Router::new()
                .route("/", mcp_methods.clone())
                .route("/mcp", mcp_methods)
                .route(
                    "/.well-known/oauth-protected-resource",
                    get(handle_oauth_metadata),
                )
                .route(
                    "/.well-known/oauth-protected-resource/mcp",
                    get(handle_oauth_metadata),
                )
                .route(
                    "/mcp/.well-known/oauth-protected-resource",
                    get(handle_oauth_metadata),
                )
                .route(
                    "/.well-known/openai-apps-challenge",
                    get(handle_openai_challenge),
                )
                .layer(ServiceBuilder::new());

            let address = format!("127.0.0.1:{MCP_HTTP_PORT}");
            let listener = match tokio::net::TcpListener::bind(&address).await {
                Ok(listener) => listener,
                Err(error) => {
                    eprintln!("AgentDeck MCP HTTP server failed to bind {address}: {error}");
                    return;
                }
            };

            eprintln!("AgentDeck MCP HTTP server listening on http://{address}");
            if let Err(error) = axum::serve(listener, app).await {
                eprintln!("AgentDeck MCP HTTP server stopped: {error}");
            }
        });
    });
}

async fn handle_oauth_metadata() -> impl IntoResponse {
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "resource": mcp_public_url::mcp_public_resource_url()
        }),
    )
}

async fn handle_openai_challenge() -> axum::response::Response {
    match env::var("OPENAI_APPS_CHALLENGE_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
        .or_else(tunnel_control::openai_apps_challenge_token)
    {
        Some(token) => (
            StatusCode::OK,
            [("Content-Type", "text/plain; charset=utf-8")],
            token,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_mcp_method_not_allowed(method: Method) -> impl IntoResponse {
    if method == Method::GET || method == Method::DELETE {
        return method_not_allowed_response();
    }
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}

async fn handle_mcp_post(body: String) -> impl IntoResponse {
    if mcp_server::request_is_notification_only(&body) {
        return accepted_response();
    }

    let result = tokio::task::spawn_blocking(move || mcp_server::process_request_line(&body))
        .await
        .unwrap_or_else(|error| Err(format!("MCP request task failed: {error}")));

    match result {
        Ok(Some(response)) => json_response(StatusCode::OK, cap_tool_response_size(response)),
        Ok(None) => accepted_response(),
        Err(error) => json_response(
            StatusCode::BAD_REQUEST,
            mcp_server::internal_error_response(None, &error),
        ),
    }
}

fn cap_tool_response_size(value: Value) -> Value {
    let Some(serialized) = serde_json::to_string(&value).ok() else {
        return value;
    };
    if serialized.len() <= MAX_TOOL_RESPONSE_CHARS {
        return value;
    }

    let notice = format!(
        "AgentDeck truncated this MCP tool response to {MAX_TOOL_RESPONSE_CHARS} characters for connector compatibility."
    );
    mcp_server::truncate_tool_response(value, &notice)
}

fn accepted_response() -> axum::response::Response {
    (
        StatusCode::ACCEPTED,
        [(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")],
    )
        .into_response()
}

fn method_not_allowed_response() -> axum::response::Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [
            (header::ALLOW, "POST"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        "Method Not Allowed",
    )
        .into_response()
}

fn json_response(status: StatusCode, value: Value) -> axum::response::Response {
    let body = serde_json::to_string(&value).unwrap_or_else(|error| {
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32603,\"message\":\"{error}\"}}}}"
        )
    });
    let mut response = (status, body).into_response();
    response
        .headers_mut()
        .insert("Content-Type", HeaderValue::from_static("application/json"));
    response
        .headers_mut()
        .insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_http_port_is_seven_eight_two_three() {
        assert_eq!(MCP_HTTP_PORT, 7823);
    }

    #[test]
    fn caps_large_tool_response_payloads() {
        let oversized = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{
                    "type": "text",
                    "text": "x".repeat(MAX_TOOL_RESPONSE_CHARS + 1)
                }],
                "isError": false
            }
        });
        let capped = cap_tool_response_size(oversized);
        let text = capped["result"]["content"][0]["text"]
            .as_str()
            .expect("text");
        assert!(text.contains("truncated"));
        assert!(text.len() < MAX_TOOL_RESPONSE_CHARS + 512);
    }
}