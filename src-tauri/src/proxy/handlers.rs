//! 请求处理器
//!
//! 处理各种API端点的HTTP请求
//!
//! 重构后的结构：
//! - 通用逻辑提取到 `handler_context` 和 `response_processor` 模块
//! - 各 handler 只保留独特的业务逻辑
//! - Claude 的格式转换逻辑保留在此文件（用于 OpenRouter 旧接口回退）

use super::{
    error_mapper::{get_error_message, map_proxy_error_to_status},
    handler_config::{
        CLAUDE_PARSER_CONFIG, CODEX_PARSER_CONFIG, GEMINI_PARSER_CONFIG, OPENAI_PARSER_CONFIG,
    },
    handler_context::RequestContext,
    providers::{
        get_adapter, get_claude_api_format, streaming::create_anthropic_sse_stream,
        streaming_responses::create_anthropic_sse_stream_from_responses, transform,
        transform_responses,
    },
    response_processor::{create_logged_passthrough_stream, process_response, SseUsageCollector},
    server::ProxyState,
    types::*,
    usage::parser::TokenUsage,
    ProxyError,
};
use crate::app_config::AppType;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use bytes::Bytes;
use serde_json::{json, Value};

// ============================================================================
// 健康检查和状态查询（简单端点）
// ============================================================================

/// 健康检查
pub async fn health_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

/// 获取服务状态
pub async fn get_status(State(state): State<ProxyState>) -> Result<Json<ProxyStatus>, ProxyError> {
    let status = state.status.read().await.clone();
    Ok(Json(status))
}

/// 获取 VS Code Copilot 可用模型列表
pub async fn handle_vscode_models(
    State(state): State<ProxyState>,
) -> Result<Json<VscodeModelsResponse>, ProxyError> {
    let providers = state
        .db
        .get_all_providers(AppType::VscodeCopilot.as_str())
        .map_err(|err| ProxyError::DatabaseError(err.to_string()))?;
    let enabled_ids = crate::vscode_copilot_config::read_enabled_provider_ids()
        .map_err(|err| ProxyError::DatabaseError(err.to_string()))?
        .unwrap_or_else(|| providers.keys().cloned().collect());

    let mut data = Vec::with_capacity(enabled_ids.len());
    for provider_id in enabled_ids {
        let Some(provider) = providers.get(&provider_id) else {
            continue;
        };
        if let Some(model) = provider_to_vscode_model(&provider) {
            data.push(model);
        }
    }

    Ok(Json(VscodeModelsResponse {
        object: "list".to_string(),
        data,
    }))
}

fn provider_to_vscode_model(provider: &crate::provider::Provider) -> Option<VscodeModelInfo> {
    let mut model: VscodeModelInfo =
        serde_json::from_value(provider.settings_config.clone()).unwrap_or_else(|_| {
            VscodeModelInfo {
                id: provider.id.clone(),
                name: provider.name.clone(),
                family: provider
                    .category
                    .clone()
                    .unwrap_or_else(|| "custom".to_string()),
                version: "1.0.0".to_string(),
                max_input_tokens: 128_000,
                max_output_tokens: 8_192,
                tooltip: provider.name.clone(),
                capabilities: VscodeModelCapabilities {
                    image_input: false,
                    tool_calling: true,
                },
                provider_id: provider.id.clone(),
            }
        });

    if model.id.trim().is_empty() {
        model.id = provider.id.clone();
    }
    if model.name.trim().is_empty() {
        model.name = provider.name.clone();
    }
    if model.family.trim().is_empty() {
        model.family = "custom".to_string();
    }
    if model.tooltip.trim().is_empty() {
        model.tooltip = format!("{} via CC Switch", model.name);
    }
    model.provider_id = provider.id.clone();

    if model.id.trim().is_empty() || model.name.trim().is_empty() {
        return None;
    }

    Some(model)
}

fn is_vscode_copilot_request(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("x-cc-switch-app")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("vscode-copilot"))
}

fn model_matches_vscode_provider(provider: &crate::provider::Provider, request_model: &str) -> bool {
    provider
        .settings_config
        .get("id")
        .and_then(|value| value.as_str())
        .is_some_and(|model_id| model_id == request_model)
}

async fn resolve_chat_app_type(
    state: &ProxyState,
    headers: &axum::http::HeaderMap,
    body: &Value,
) -> AppType {
    if is_vscode_copilot_request(headers) {
        return AppType::VscodeCopilot;
    }

    let request_model = body.get("model").and_then(|m| m.as_str()).unwrap_or("");
    if request_model.is_empty() {
        return AppType::Codex;
    }

    match state.db.get_all_providers(AppType::VscodeCopilot.as_str()) {
        Ok(providers) => {
            let enabled_ids = crate::vscode_copilot_config::read_enabled_provider_ids()
                .ok()
                .flatten()
                .unwrap_or_else(|| providers.keys().cloned().collect::<Vec<_>>());
            if enabled_ids.into_iter().any(|provider_id| {
                providers
                    .get(&provider_id)
                    .is_some_and(|provider| model_matches_vscode_provider(provider, request_model))
            }) {
                AppType::VscodeCopilot
            } else {
                AppType::Codex
            }
        }
        _ => AppType::Codex,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_vscode_copilot_request, model_matches_vscode_provider, provider_to_vscode_model,
    };
    use crate::provider::Provider;
    use serde_json::json;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn provider_to_vscode_model_applies_defaults() {
        let provider = Provider::with_id(
            "copilot-provider".to_string(),
            "My Model".to_string(),
            json!({
                "id": "",
                "name": "",
                "family": "",
                "base_url": "https://example.com/v1"
            }),
            None,
        );

        let model = provider_to_vscode_model(&provider).expect("model should be generated");
        assert_eq!(model.id, "copilot-provider");
        assert_eq!(model.name, "My Model");
        assert_eq!(model.family, "custom");
        assert_eq!(model.version, "1.0.0");
        assert_eq!(model.max_input_tokens, 128_000);
        assert_eq!(model.max_output_tokens, 8_192);
        assert_eq!(model.tooltip, "My Model via CC Switch");
        assert!(model.capabilities.tool_calling);
        assert_eq!(model.provider_id, "copilot-provider");
    }

    #[test]
    fn provider_to_vscode_model_uses_explicit_settings() {
        let provider = Provider::with_id(
            "provider-1".to_string(),
            "Ignored Name".to_string(),
            json!({
                "id": "gpt-4o",
                "name": "GPT-4o",
                "family": "openai",
                "version": "4o",
                "maxInputTokens": 999,
                "maxOutputTokens": 111,
                "tooltip": "Custom tooltip",
                "capabilities": {
                    "imageInput": true,
                    "toolCalling": false
                },
                "base_url": "https://api.openai.com/v1"
            }),
            None,
        );

        let model = provider_to_vscode_model(&provider).expect("model should be generated");
        assert_eq!(model.id, "gpt-4o");
        assert_eq!(model.name, "GPT-4o");
        assert_eq!(model.family, "openai");
        assert_eq!(model.version, "4o");
        assert_eq!(model.max_input_tokens, 999);
        assert_eq!(model.max_output_tokens, 111);
        assert_eq!(model.tooltip, "Custom tooltip");
        assert!(model.capabilities.image_input);
        assert!(!model.capabilities.tool_calling);
        assert_eq!(model.provider_id, "provider-1");
    }

    #[test]
    fn detects_vscode_copilot_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-cc-switch-app",
            HeaderValue::from_static("vscode-copilot"),
        );
        assert!(is_vscode_copilot_request(&headers));
    }

    #[test]
    fn model_match_uses_settings_config_id() {
        let provider = Provider::with_id(
            "provider-1".to_string(),
            "Model A".to_string(),
            json!({ "id": "model-a" }),
            None,
        );

        assert!(model_matches_vscode_provider(&provider, "model-a"));
        assert!(!model_matches_vscode_provider(&provider, "model-b"));
    }
}

// ============================================================================
// Claude API 处理器（包含格式转换逻辑）
// ============================================================================

/// 处理 /v1/messages 请求（Claude API）
///
/// Claude 处理器包含独特的格式转换逻辑：
/// - 过去用于 OpenRouter 的 OpenAI Chat Completions 兼容接口（Anthropic ↔ OpenAI 转换）
/// - 现在 OpenRouter 已推出 Claude Code 兼容接口，默认不再启用该转换（逻辑保留以备回退）
pub async fn handle_messages(
    State(state): State<ProxyState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Result<axum::response::Response, ProxyError> {
    let mut ctx =
        RequestContext::new(&state, &body, &headers, AppType::Claude, "Claude", "claude").await?;

    let is_stream = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    // 转发请求
    let forwarder = ctx.create_forwarder(&state);
    let result = match forwarder
        .forward_with_retry(
            &AppType::Claude,
            "/v1/messages",
            body.clone(),
            headers,
            ctx.get_providers(),
        )
        .await
    {
        Ok(result) => result,
        Err(mut err) => {
            if let Some(provider) = err.provider.take() {
                ctx.provider = provider;
            }
            log_forward_error(&state, &ctx, is_stream, &err.error);
            return Err(err.error);
        }
    };

    ctx.provider = result.provider;
    let response = result.response;

    // 检查是否需要格式转换（OpenRouter 等中转服务）
    let adapter = get_adapter(&AppType::Claude);
    let needs_transform = adapter.needs_transform(&ctx.provider);

    // Claude 特有：格式转换处理
    if needs_transform {
        return handle_claude_transform(response, &ctx, &state, &body, is_stream).await;
    }

    // 通用响应处理（透传模式）
    process_response(response, &ctx, &state, &CLAUDE_PARSER_CONFIG).await
}

/// Claude 格式转换处理（独有逻辑）
///
/// 支持 OpenAI Chat Completions 和 Responses API 两种格式的转换
async fn handle_claude_transform(
    response: reqwest::Response,
    ctx: &RequestContext,
    state: &ProxyState,
    _original_body: &Value,
    is_stream: bool,
) -> Result<axum::response::Response, ProxyError> {
    let status = response.status();
    let api_format = get_claude_api_format(&ctx.provider);

    if is_stream {
        // 根据 api_format 选择流式转换器
        let stream = response.bytes_stream();
        let sse_stream: Box<
            dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send + Unpin,
        > = if api_format == "openai_responses" {
            Box::new(Box::pin(create_anthropic_sse_stream_from_responses(stream)))
        } else {
            Box::new(Box::pin(create_anthropic_sse_stream(stream)))
        };

        // 创建使用量收集器
        let usage_collector = {
            let state = state.clone();
            let provider_id = ctx.provider.id.clone();
            let model = ctx.request_model.clone();
            let status_code = status.as_u16();
            let start_time = ctx.start_time;

            SseUsageCollector::new(start_time, move |events, first_token_ms| {
                if let Some(usage) = TokenUsage::from_claude_stream_events(&events) {
                    let latency_ms = start_time.elapsed().as_millis() as u64;
                    let state = state.clone();
                    let provider_id = provider_id.clone();
                    let model = model.clone();

                    tokio::spawn(async move {
                        log_usage(
                            &state,
                            &provider_id,
                            "claude",
                            &model,
                            &model,
                            usage,
                            latency_ms,
                            first_token_ms,
                            true,
                            status_code,
                        )
                        .await;
                    });
                } else {
                    log::debug!("[Claude] OpenRouter 流式响应缺少 usage 统计，跳过消费记录");
                }
            })
        };

        // 获取流式超时配置
        let timeout_config = ctx.streaming_timeout_config();

        let logged_stream = create_logged_passthrough_stream(
            sse_stream,
            "Claude/OpenRouter",
            Some(usage_collector),
            timeout_config,
        );

        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "Content-Type",
            axum::http::HeaderValue::from_static("text/event-stream"),
        );
        headers.insert(
            "Cache-Control",
            axum::http::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            "Connection",
            axum::http::HeaderValue::from_static("keep-alive"),
        );

        let body = axum::body::Body::from_stream(logged_stream);
        return Ok((headers, body).into_response());
    }

    // 非流式响应转换 (OpenAI/Responses → Anthropic)
    let response_headers = response.headers().clone();

    let body_bytes = response.bytes().await.map_err(|e| {
        log::error!("[Claude] 读取响应体失败: {e}");
        ProxyError::ForwardFailed(format!("Failed to read response body: {e}"))
    })?;

    let body_str = String::from_utf8_lossy(&body_bytes);

    let upstream_response: Value = serde_json::from_slice(&body_bytes).map_err(|e| {
        log::error!("[Claude] 解析上游响应失败: {e}, body: {body_str}");
        ProxyError::TransformError(format!("Failed to parse upstream response: {e}"))
    })?;

    // 根据 api_format 选择非流式转换器
    let anthropic_response = if api_format == "openai_responses" {
        transform_responses::responses_to_anthropic(upstream_response)
    } else {
        transform::openai_to_anthropic(upstream_response)
    }
    .map_err(|e| {
        log::error!("[Claude] 转换响应失败: {e}");
        e
    })?;

    // 记录使用量
    if let Some(usage) = TokenUsage::from_claude_response(&anthropic_response) {
        let model = anthropic_response
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        let latency_ms = ctx.latency_ms();

        let request_model = ctx.request_model.clone();
        tokio::spawn({
            let state = state.clone();
            let provider_id = ctx.provider.id.clone();
            let model = model.to_string();
            async move {
                log_usage(
                    &state,
                    &provider_id,
                    "claude",
                    &model,
                    &request_model,
                    usage,
                    latency_ms,
                    None,
                    false,
                    status.as_u16(),
                )
                .await;
            }
        });
    }

    // 构建响应
    let mut builder = axum::response::Response::builder().status(status);

    for (key, value) in response_headers.iter() {
        if key.as_str().to_lowercase() != "content-length"
            && key.as_str().to_lowercase() != "transfer-encoding"
        {
            builder = builder.header(key, value);
        }
    }

    builder = builder.header("content-type", "application/json");

    let response_body = serde_json::to_vec(&anthropic_response).map_err(|e| {
        log::error!("[Claude] 序列化响应失败: {e}");
        ProxyError::TransformError(format!("Failed to serialize response: {e}"))
    })?;

    let body = axum::body::Body::from(response_body);
    builder.body(body).map_err(|e| {
        log::error!("[Claude] 构建响应失败: {e}");
        ProxyError::Internal(format!("Failed to build response: {e}"))
    })
}

// ============================================================================
// Codex API 处理器
// ============================================================================

/// 处理 /v1/chat/completions 请求（OpenAI Chat Completions API - Codex CLI）
pub async fn handle_chat_completions(
    State(state): State<ProxyState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Result<axum::response::Response, ProxyError> {
    let app_type = resolve_chat_app_type(&state, &headers, &body).await;
    let (tag, app_type_str) = match app_type {
        AppType::VscodeCopilot => ("VSCode Copilot", "vscode-copilot"),
        _ => ("Codex", "codex"),
    };
    let mut ctx = RequestContext::new(&state, &body, &headers, app_type.clone(), tag, app_type_str)
        .await?;

    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let forwarder = ctx.create_forwarder(&state);
    let providers = if matches!(app_type, AppType::VscodeCopilot) {
        let request_model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");
        ctx.get_providers()
            .into_iter()
            .filter(|provider| model_matches_vscode_provider(provider, request_model))
            .collect::<Vec<_>>()
    } else {
        ctx.get_providers()
    };
    let result = match forwarder
        .forward_with_retry(
            &app_type,
            "/chat/completions",
            body,
            headers,
            providers,
        )
        .await
    {
        Ok(result) => result,
        Err(mut err) => {
            if let Some(provider) = err.provider.take() {
                ctx.provider = provider;
            }
            log_forward_error(&state, &ctx, is_stream, &err.error);
            return Err(err.error);
        }
    };

    ctx.provider = result.provider;
    let response = result.response;

    process_response(response, &ctx, &state, &OPENAI_PARSER_CONFIG).await
}

/// 处理 /v1/responses 请求（OpenAI Responses API - Codex CLI 透传）
pub async fn handle_responses(
    State(state): State<ProxyState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Result<axum::response::Response, ProxyError> {
    let mut ctx =
        RequestContext::new(&state, &body, &headers, AppType::Codex, "Codex", "codex").await?;

    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let forwarder = ctx.create_forwarder(&state);
    let result = match forwarder
        .forward_with_retry(
            &AppType::Codex,
            "/responses",
            body,
            headers,
            ctx.get_providers(),
        )
        .await
    {
        Ok(result) => result,
        Err(mut err) => {
            if let Some(provider) = err.provider.take() {
                ctx.provider = provider;
            }
            log_forward_error(&state, &ctx, is_stream, &err.error);
            return Err(err.error);
        }
    };

    ctx.provider = result.provider;
    let response = result.response;

    process_response(response, &ctx, &state, &CODEX_PARSER_CONFIG).await
}

/// 处理 /v1/responses/compact 请求（OpenAI Responses Compact API - Codex CLI 透传）
pub async fn handle_responses_compact(
    State(state): State<ProxyState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Result<axum::response::Response, ProxyError> {
    let mut ctx =
        RequestContext::new(&state, &body, &headers, AppType::Codex, "Codex", "codex").await?;

    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let forwarder = ctx.create_forwarder(&state);
    let result = match forwarder
        .forward_with_retry(
            &AppType::Codex,
            "/responses/compact",
            body,
            headers,
            ctx.get_providers(),
        )
        .await
    {
        Ok(result) => result,
        Err(mut err) => {
            if let Some(provider) = err.provider.take() {
                ctx.provider = provider;
            }
            log_forward_error(&state, &ctx, is_stream, &err.error);
            return Err(err.error);
        }
    };

    ctx.provider = result.provider;
    let response = result.response;

    process_response(response, &ctx, &state, &CODEX_PARSER_CONFIG).await
}

// ============================================================================
// Gemini API 处理器
// ============================================================================

/// 处理 Gemini API 请求（透传，包括查询参数）
pub async fn handle_gemini(
    State(state): State<ProxyState>,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Result<axum::response::Response, ProxyError> {
    // Gemini 的模型名称在 URI 中
    let mut ctx = RequestContext::new(&state, &body, &headers, AppType::Gemini, "Gemini", "gemini")
        .await?
        .with_model_from_uri(&uri);

    // 提取完整的路径和查询参数
    let endpoint = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(uri.path());

    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let forwarder = ctx.create_forwarder(&state);
    let result = match forwarder
        .forward_with_retry(
            &AppType::Gemini,
            endpoint,
            body,
            headers,
            ctx.get_providers(),
        )
        .await
    {
        Ok(result) => result,
        Err(mut err) => {
            if let Some(provider) = err.provider.take() {
                ctx.provider = provider;
            }
            log_forward_error(&state, &ctx, is_stream, &err.error);
            return Err(err.error);
        }
    };

    ctx.provider = result.provider;
    let response = result.response;

    process_response(response, &ctx, &state, &GEMINI_PARSER_CONFIG).await
}

// ============================================================================
// 使用量记录（保留用于 Claude 转换逻辑）
// ============================================================================

fn log_forward_error(
    state: &ProxyState,
    ctx: &RequestContext,
    is_streaming: bool,
    error: &ProxyError,
) {
    use super::usage::logger::UsageLogger;

    let logger = UsageLogger::new(&state.db);
    let status_code = map_proxy_error_to_status(error);
    let error_message = get_error_message(error);
    let request_id = uuid::Uuid::new_v4().to_string();

    if let Err(e) = logger.log_error_with_context(
        request_id,
        ctx.provider.id.clone(),
        ctx.app_type_str.to_string(),
        ctx.request_model.clone(),
        status_code,
        error_message,
        ctx.latency_ms(),
        is_streaming,
        Some(ctx.session_id.clone()),
        None,
    ) {
        log::warn!("记录失败请求日志失败: {e}");
    }
}

/// 记录请求使用量
#[allow(clippy::too_many_arguments)]
async fn log_usage(
    state: &ProxyState,
    provider_id: &str,
    app_type: &str,
    model: &str,
    request_model: &str,
    usage: TokenUsage,
    latency_ms: u64,
    first_token_ms: Option<u64>,
    is_streaming: bool,
    status_code: u16,
) {
    use super::usage::logger::UsageLogger;

    let logger = UsageLogger::new(&state.db);

    let (multiplier, pricing_model_source) =
        logger.resolve_pricing_config(provider_id, app_type).await;
    let pricing_model = if pricing_model_source == "request" {
        request_model
    } else {
        model
    };

    let request_id = uuid::Uuid::new_v4().to_string();

    if let Err(e) = logger.log_with_calculation(
        request_id,
        provider_id.to_string(),
        app_type.to_string(),
        model.to_string(),
        request_model.to_string(),
        pricing_model.to_string(),
        usage,
        multiplier,
        latency_ms,
        first_token_ms,
        status_code,
        None,
        None, // provider_type
        is_streaming,
    ) {
        log::warn!("[USG-001] 记录使用量失败: {e}");
    }
}
