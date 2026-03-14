use crate::io::workflow_input_from_text;
use crate::runtime::ComposeRuntime;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct ServerState {
    pub runtime: Arc<ComposeRuntime>,
    pub chain: String,
}

#[derive(Debug, Deserialize)]
pub struct InferRequest {
    pub input: Option<Value>,
    pub text: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InferResponse {
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

pub async fn serve(
    config_path: String,
    chain: String,
    host: String,
    port: u16,
) -> Result<(), String> {
    let runtime = ComposeRuntime::from_path_and_chain(&config_path, &chain).map_err(|e| e.to_string())?;
    let state = ServerState {
        runtime: Arc::new(runtime),
        chain,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/infer", post(infer))
        .route("/rpc", post(rpc))
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| e.to_string())?;
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

async fn rpc(
    State(state): State<ServerState>,
    Json(req): Json<RpcRequest>,
) -> (StatusCode, Json<RpcResponse>) {
    if req.jsonrpc != "2.0" {
        return (
            StatusCode::BAD_REQUEST,
            Json(rpc_error(req.id, -32600, "jsonrpc must be 2.0")),
        );
    }

    match req.method.as_str() {
        "ping" => (
            StatusCode::OK,
            Json(rpc_ok(req.id, json!({"ok": true, "chain": state.chain}))),
        ),
        "initialize" => (
            StatusCode::OK,
            Json(rpc_ok(
                req.id,
                json!({
                    "protocolVersion": "2026-03-01",
                    "serverInfo": {
                        "name": "agent-compose",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "tools": {}
                    }
                }),
            )),
        ),
        "tools/list" => (
            StatusCode::OK,
            Json(rpc_ok(
                req.id,
                json!({
                    "tools": [
                        {
                            "name": "infer",
                            "description": "Run chain inference",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "input": {"type": "object"},
                                    "text": {"type": "string"},
                                    "model": {"type": "string"}
                                },
                                "additionalProperties": false
                            }
                        }
                    ]
                }),
            )),
        ),
        "tools/call" => {
            let Some(params) = req.params else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(rpc_error(req.id, -32602, "missing params")),
                );
            };
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if name != "infer" {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(rpc_error(req.id, -32601, "unknown tool")),
                );
            }
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let model = args
                .get("model")
                .and_then(Value::as_str)
                .map(|v| v.to_string());
            let input = if let Some(payload) = args.get("input").cloned() {
                payload
            } else if let Some(text) = args.get("text").and_then(Value::as_str) {
                workflow_input_from_text(text)
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(rpc_error(
                        req.id,
                        -32602,
                        "provide arguments.input object or arguments.text",
                    )),
                );
            };

            match state.runtime.run(input, model).await {
                Ok(result) => (
                    StatusCode::OK,
                    Json(rpc_ok(req.id, json!({"content": [{"type": "json", "json": result}]}))),
                ),
                Err(err) => (
                    StatusCode::BAD_REQUEST,
                    Json(rpc_error(req.id, -32000, &err.to_string())),
                ),
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(rpc_error(req.id, -32601, "method not found")),
        ),
    }
}

fn rpc_ok(id: Option<Value>, result: Value) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    }
}

fn rpc_error(id: Option<Value>, code: i64, message: &str) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(json!({"code": code, "message": message})),
    }
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "status": "healthy"}))
}

async fn infer(
    State(state): State<ServerState>,
    Json(req): Json<InferRequest>,
) -> (StatusCode, Json<InferResponse>) {
    let input = if let Some(payload) = req.input {
        payload
    } else if let Some(text) = req.text {
        workflow_input_from_text(&text)
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(InferResponse {
                ok: false,
                result: None,
                error: Some("provide input object or non-empty text".to_string()),
            }),
        );
    };

    match state.runtime.run(input, req.model).await {
        Ok(result) => (
            StatusCode::OK,
            Json(InferResponse {
                ok: true,
                result: Some(result),
                error: None,
            }),
        ),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(InferResponse {
                ok: false,
                result: None,
                error: Some(err.to_string()),
            }),
        ),
    }
}
