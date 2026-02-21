// Perplexity 请求映射器
// 将 OpenAI 格式请求转换为 Perplexity API 格式

use serde_json::{json, Value};
use tracing::{debug, info};

/// OpenAI 格式 → Perplexity 格式请求转换
/// 
/// Perplexity API 与 OpenAI 高度兼容，主要处理:
/// 1. 模型名称映射 (如 perplexity-sonar -> sonar)
/// 2. 添加 Perplexity 特有参数
/// 3. 过滤不支持的参数
pub fn transform_to_perplexity_request(request: &Value) -> Value {
    let mut perplexity_request = json!({});
    
    // 1. 模型名称处理
    if let Some(model) = request.get("model").and_then(|v| v.as_str()) {
        let mapped_model = map_model_name(model);
        perplexity_request["model"] = json!(mapped_model);
        debug!(original_model = model, mapped_model = %mapped_model, "Model mapping");
    }

    // 2. 消息内容 - 直接传递
    if let Some(messages) = request.get("messages") {
        perplexity_request["messages"] = messages.clone();
    }

    // 3. 标准 OpenAI 参数 - 直接透传
    let standard_params = [
        "stream",
        "max_tokens",
        "temperature",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
    ];
    
    for param in standard_params {
        if let Some(value) = request.get(param) {
            perplexity_request[param] = value.clone();
        }
    }

    // 4. Perplexity 特有参数
    let perplexity_params = [
        "search_recency_filter",  // 搜索时效过滤器: "month", "week", "day", "hour"
        "return_images",          // 是否返回图片
        "return_related_questions", // 是否返回相关问题
        "search_domain_filter",   // 搜索域名过滤
        "top_k",                  // Top-K 采样
    ];
    
    for param in perplexity_params {
        if let Some(value) = request.get(param) {
            perplexity_request[param] = value.clone();
        }
    }

    info!(
        model = request.get("model").and_then(|v| v.as_str()).unwrap_or("unknown"),
        stream = request.get("stream").and_then(|v| v.as_bool()).unwrap_or(false),
        "Perplexity request transformed"
    );

    perplexity_request
}

/// 模型名称映射
/// 
/// 支持的映射:
/// - perplexity-sonar -> sonar
/// - perplexity-sonar-pro -> sonar-pro
/// - perplexity-* -> * (去除前缀)
/// - 其他保持不变
fn map_model_name(model: &str) -> &str {
    // 去除 perplexity- 前缀
    if let Some(stripped) = model.strip_prefix("perplexity-") {
        return stripped;
    }
    
    // 直接返回原始模型名称
    model
}

/// 验证请求格式
pub fn validate_perplexity_request(request: &Value) -> Result<(), String> {
    // 检查必需字段
    if request.get("model").is_none() {
        return Err("Missing required field: model".to_string());
    }
    
    if request.get("messages").is_none() {
        return Err("Missing required field: messages".to_string());
    }
    
    // 检查 messages 格式
    if let Some(messages) = request.get("messages") {
        if !messages.is_array() {
            return Err("Field 'messages' must be an array".to_string());
        }
        
        let messages_arr = messages.as_array().unwrap();
        if messages_arr.is_empty() {
            return Err("Field 'messages' cannot be empty".to_string());
        }
        
        // 检查每个消息的结构
        for (i, msg) in messages_arr.iter().enumerate() {
            if msg.get("role").is_none() {
                return Err(format!("Message at index {} missing 'role' field", i));
            }
            if msg.get("content").is_none() {
                return Err(format!("Message at index {} missing 'content' field", i));
            }
        }
    }
    
    // 检查 search_recency_filter 有效值
    if let Some(filter) = request.get("search_recency_filter").and_then(|v| v.as_str()) {
        let valid_filters = ["month", "week", "day", "hour"];
        if !valid_filters.contains(&filter) {
            return Err(format!(
                "Invalid search_recency_filter: '{}'. Valid values: {:?}",
                filter, valid_filters
            ));
        }
    }
    
    Ok(())
}

/// 构建 Perplexity API 请求 URL
pub fn build_perplexity_url(endpoint: &str) -> String {
    const PERPLEXITY_BASE_URL: &str = "https://api.perplexity.ai";
    format!("{}/{}", PERPLEXITY_BASE_URL, endpoint.trim_start_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transform_basic_request() {
        let request = json!({
            "model": "sonar",
            "messages": [
                {"role": "user", "content": "Hello, world!"}
            ]
        });

        let transformed = transform_to_perplexity_request(&request);
        
        assert_eq!(transformed["model"], "sonar");
        assert!(transformed["messages"].is_array());
    }

    #[test]
    fn test_model_name_mapping() {
        assert_eq!(map_model_name("perplexity-sonar"), "sonar");
        assert_eq!(map_model_name("perplexity-sonar-pro"), "sonar-pro");
        assert_eq!(map_model_name("sonar"), "sonar");
        assert_eq!(map_model_name("custom-model"), "custom-model");
    }

    #[test]
    fn test_perplexity_specific_params() {
        let request = json!({
            "model": "sonar",
            "messages": [{"role": "user", "content": "test"}],
            "search_recency_filter": "week",
            "return_images": true,
            "return_related_questions": true
        });

        let transformed = transform_to_perplexity_request(&request);
        
        assert_eq!(transformed["search_recency_filter"], "week");
        assert_eq!(transformed["return_images"], true);
        assert_eq!(transformed["return_related_questions"], true);
    }

    #[test]
    fn test_validate_request_missing_model() {
        let request = json!({
            "messages": [{"role": "user", "content": "test"}]
        });

        let result = validate_perplexity_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("model"));
    }

    #[test]
    fn test_validate_request_invalid_filter() {
        let request = json!({
            "model": "sonar",
            "messages": [{"role": "user", "content": "test"}],
            "search_recency_filter": "invalid"
        });

        let result = validate_perplexity_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("search_recency_filter"));
    }

    #[test]
    fn test_build_url() {
        assert_eq!(
            build_perplexity_url("chat/completions"),
            "https://api.perplexity.ai/chat/completions"
        );
        assert_eq!(
            build_perplexity_url("/chat/completions"),
            "https://api.perplexity.ai/chat/completions"
        );
    }
}
