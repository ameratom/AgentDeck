use axum::{
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::env;
use tower::ServiceBuilder;

use crate::mcp_server;
use crate::tunnel_control;

pub const MCP_HTTP_PORT: u16 = 7823;

pub fn start_http_server() {
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .thread_name("agentdeck-mcp-http")
            .build()
            .expect("failed to start MCP HTTP runtime");

        runtime.block_on(async {
            let app = Router::new()
                .route("/", post(handle_mcp_request))
                .route("/mcp", post(handle_mcp_request))
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
            "resource": format!("http://127.0.0.1:{MCP_HTTP_PORT}/mcp")
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

async fn handle_mcp_request(body: String) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || mcp_server::process_request_line(&body))
        .await
        .unwrap_or_else(|error| Err(format!("MCP request task failed: {error}")));

    match result {
        Ok(Some(response)) => json_response(StatusCode::OK, response),
        Ok(None) => (
            StatusCode::NO_CONTENT,
            [("Access-Control-Allow-Origin", "*")],
            String::new(),
        )
            .into_response(),
        Err(error) => json_response(
            StatusCode::BAD_REQUEST,
            mcp_server::internal_error_response(None, &error),
        ),
    }
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
}
