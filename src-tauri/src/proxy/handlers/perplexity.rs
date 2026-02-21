// Perplexity Handler
// 处理 Perplexity API 请求，路由到本地代理 127.0.0.1:8046
//
// 路由逻辑:
// 1. 检测模型名称是否包含 perplexity_ 前缀
// 2. 将请求转发到本地代理，支持 OpenAI/Claude/Gemini 协议
// 3. 响应直接透传，不做转换

use axum::{
    body::Body,
    extract::{Json, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};

use crate::proxy::config::get_perplexity_proxy_url;
use crate::proxy::mappers::perplexity::create_error_response;
use crate::proxy::server::AppState;

const MAX_RETRY_ATTEMPTS: usize = 3;

/// 处理 Perplexity Chat Completions 请求
/// 支持 OpenAI/Claude/Gemini 协议调用
pub async fn handle_chat_completions(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let trace_id = format!("perp_{}", chrono::Utc::now().timestamp_subsec_millis());

    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    info!(
        "[{}] Perplexity Chat Request: {} | stream: {}",
        trace_id, model, is_stream
    );

    // 提取实际模型名称 (去掉 perplexity_ 前缀)
    let actual_model = model.strip_prefix("perplexity_").unwrap_or(model);

    // 构建转发请求体
    let mut request_body = body.clone();
    request_body["model"] = Value::String(actual_model.to_string());

    debug!(
        "[{}] Forwarding to local proxy with model: {}",
        trace_id, actual_model
    );

    // 转发到本地代理
    let client = reqwest::Client::new();
    let proxy_url = get_perplexity_proxy_url();
    let url = format!("{}/v1/chat/completions", proxy_url);

    // 提取原始请求的 Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut last_error = String::new();

    for attempt in 0..MAX_RETRY_ATTEMPTS {
        let mut request = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body);

        // 如果有 Authorization header，透传
        if let Some(ref auth) = auth_header {
            request = request.header("Authorization", auth);
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Request failed: {}", e);
                warn!(
                    "[{}] Attempt {}/{} failed: {}",
                    trace_id,
                    attempt + 1,
                    MAX_RETRY_ATTEMPTS,
                    e
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    500 * (attempt as u64 + 1),
                ))
                .await;
                continue;
            }
        };

        let status = response.status();

        if status.is_success() {
            if is_stream {
                return handle_passthrough_stream(response, &trace_id).await;
            } else {
                return handle_passthrough_json(response, &trace_id).await;
            }
        }

        // 处理错误
        let status_code = status.as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| format!("HTTP {}", status_code));
        last_error = format!("HTTP {}: {}", status_code, error_text);

        error!(
            "[{}] Upstream error {}: {}",
            trace_id, status_code, error_text
        );

        // 429 或 503 可重试
        if status_code == 429 || status_code == 503 {
            let delay_ms = 1000 * (attempt as u64 + 1);
            warn!(
                "[{}] Rate limit hit, waiting {}ms before retry",
                trace_id, delay_ms
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            continue;
        }

        return (
            StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(create_error_response(
                status_code,
                &error_text,
                "upstream_error",
            )),
        )
            .into_response();
    }

    (
        StatusCode::BAD_GATEWAY,
        Json(create_error_response(
            502,
            &format!("All attempts failed: {}", last_error),
            "retry_exhausted",
        )),
    )
        .into_response()
}

/// 处理流式响应 - 直接透传
async fn handle_passthrough_stream(response: reqwest::Response, trace_id: &str) -> Response {
    info!("[{}] Passthrough stream response", trace_id);

    let stream = response.bytes_stream();

    // 直接透传 SSE 流，不做任何转换
    let mapped_stream = stream.map(move |chunk_result| match chunk_result {
        Ok(bytes) => Ok::<Bytes, String>(bytes),
        Err(e) => {
            error!("Stream error: {}", e);
            Ok(Bytes::from(format!("data: {{\"error\": \"{}\"}}\n\n", e)))
        }
    });

    let body = Body::from_stream(mapped_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("Connection", "keep-alive")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap()
        .into_response()
}

/// 处理 JSON 响应 - 直接透传
async fn handle_passthrough_json(response: reqwest::Response, trace_id: &str) -> Response {
    info!("[{}] Passthrough JSON response", trace_id);

    match response.json::<Value>().await {
        Ok(json_resp) => {
            debug!("[{}] Response: {:?}", trace_id, json_resp);
            (StatusCode::OK, Json(json_resp)).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to parse response: {}", trace_id, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(create_error_response(
                    502,
                    &format!("Parse error: {}", e),
                    "parse_error",
                )),
            )
                .into_response()
        }
    }
}

/// 列出可用的 Perplexity 模型
pub async fn handle_list_models(State(_state): State<AppState>) -> Response {
    let models = json!({
        "object": "list",
        "data": [
            {
                "id": "perplexity_sonar",
                "object": "model",
                "created": 1700000000,
                "owned_by": "perplexity",
                "description": "Standard Perplexity model with web search capabilities"
            },
            {
                "id": "perplexity_sonar-pro",
                "object": "model",
                "created": 1700000000,
                "owned_by": "perplexity",
                "description": "Advanced Perplexity model with enhanced capabilities"
            },
            {
                "id": "perplexity_sonar-reasoning",
                "object": "model",
                "created": 1700000000,
                "owned_by": "perplexity",
                "description": "Perplexity model optimized for reasoning tasks"
            },
            {
                "id": "perplexity_sonar-reasoning-pro",
                "object": "model",
                "created": 1700000000,
                "owned_by": "perplexity",
                "description": "Advanced reasoning model"
            }
        ]
    });

    (StatusCode::OK, Json(models)).into_response()
}

/// 健康检查端点
pub async fn handle_health() -> Response {
    (
        StatusCode::OK,
        Json(
            json!({"status": "ok", "provider": "perplexity", "proxy": get_perplexity_proxy_url()}),
        ),
    )
        .into_response()
}

// ============================================================================================
// [NEW] Multi-Protocol Support for Perplexity Proxy
// ============================================================================================

/// 将 Claude 协议请求转发到本地 Perplexity 代理
///
/// 逻辑：
/// 1. 剥离 perplexity_ 前缀
/// 2. 保持 /v1/messages 路径
/// 3. 转发并透传响应
pub async fn divert_to_local_proxy_claude(headers: HeaderMap, body: Value) -> Response {
    let trace_id = format!("perp_c_{}", chrono::Utc::now().timestamp_subsec_millis());

    // 1. 解析并修改请求体
    let mut new_body = body.clone();
    let original_model = new_body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string(); // [FIX] Clone to String to avoid borrowing from new_body
    let actual_model = original_model
        .strip_prefix("perplexity_")
        .unwrap_or(&original_model); // actual_model is now &str from the new String
    new_body["model"] = Value::String(actual_model.to_string());

    info!(
        "[{}] Diverting Claude request to Perplexity Proxy: {} -> {}",
        trace_id, original_model, actual_model
    );

    let client = reqwest::Client::new();
    let proxy_url = get_perplexity_proxy_url();
    let url = format!("{}/v1/messages", proxy_url);

    // 2. 转发请求
    forward_request_generic(client, url, new_body, headers, &trace_id).await
}

/// 将 Gemini 协议请求转发到本地 Perplexity 代理
///
/// 逻辑：
/// 1. 从路径中提取并修改模型名
/// 2. 保持 /v1beta/models/{model}:{action} 结构
/// 3. 转发并透传响应
pub async fn divert_to_local_proxy_gemini(
    headers: HeaderMap,
    body: Value,
    original_path: String, // e.g., "perplexity_gemini-pro:generateContent"
) -> Response {
    let trace_id = format!("perp_g_{}", chrono::Utc::now().timestamp_subsec_millis());

    // 1. 解析路径参数
    // original_path 是 handle_generate 传来的 model_action 部分，例如 "perplexity_gemini-1.5-pro:generateContent"
    let (model_part, action_part) = if let Some((m, a)) = original_path.rsplit_once(':') {
        (m, a)
    } else {
        (original_path.as_str(), "generateContent")
    };

    let actual_model = model_part.strip_prefix("perplexity_").unwrap_or(model_part);

    info!(
        "[{}] Diverting Gemini request to Perplexity Proxy: {} -> {}",
        trace_id, model_part, actual_model
    );

    let client = reqwest::Client::new();
    let proxy_url = get_perplexity_proxy_url();
    // 构造目标 URL: {proxy}/v1beta/models/{actual_model}:{action}
    let url = format!(
        "{}/v1beta/models/{}:{}",
        proxy_url, actual_model, action_part
    );

    // 2. 转发请求
    forward_request_generic(client, url, body, headers, &trace_id).await
}

/// 通用请求转发辅助函数
async fn forward_request_generic(
    client: reqwest::Client,
    url: String,
    body: Value,
    headers: HeaderMap,
    trace_id: &str,
) -> Response {
    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 提取 Auth Header
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut last_error = String::new();

    for attempt in 0..MAX_RETRY_ATTEMPTS {
        let mut request = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(ref auth) = auth_header {
            request = request.header("Authorization", auth);
        }

        // [NEW] 转发 Accept 头以支持 SSE
        if let Some(accept) = headers.get("accept").and_then(|v| v.to_str().ok()) {
            request = request.header("Accept", accept);
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let is_sse = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.contains("event-stream"))
                        .unwrap_or(false);

                    if is_sse || is_stream {
                        return handle_passthrough_stream(response, trace_id).await;
                    } else {
                        return handle_passthrough_json(response, trace_id).await;
                    }
                }

                //Handle Error
                let status_code = status.as_u16();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| format!("HTTP {}", status_code));
                last_error = format!("HTTP {}: {}", status_code, error_text);

                warn!(
                    "[{}] Upstream error {}: {}",
                    trace_id, status_code, error_text
                );

                if status_code == 429 || status_code == 503 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        1000 * (attempt as u64 + 1),
                    ))
                    .await;
                    continue;
                }

                return (
                    StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(create_error_response(
                        status_code,
                        &error_text,
                        "upstream_error",
                    )),
                )
                    .into_response();
            }
            Err(e) => {
                last_error = format!("Connection failed: {}", e);
                warn!("[{}] Attempt {} failed: {}", trace_id, attempt + 1, e);
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    500 * (attempt as u64 + 1),
                ))
                .await;
            }
        }
    }

    (
        StatusCode::BAD_GATEWAY,
        Json(create_error_response(
            502,
            &format!("All attempts failed: {}", last_error),
            "retry_exhausted",
        )),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_prefix_stripping() {
        let model = "perplexity_sonar";
        let actual = model.strip_prefix("perplexity_").unwrap_or(model);
        assert_eq!(actual, "sonar");

        let model2 = "sonar";
        let actual2 = model2.strip_prefix("perplexity_").unwrap_or(model2);
        assert_eq!(actual2, "sonar");
    }
}
