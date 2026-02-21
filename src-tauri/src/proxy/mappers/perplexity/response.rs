// Perplexity 响应映射器
// 将 Perplexity API 响应转换为 OpenAI 兼容格式

use serde_json::{json, Value};
use tracing::{debug, info};

/// Perplexity 响应 → OpenAI 格式响应转换
/// 
/// Perplexity 响应与 OpenAI 基本兼容，主要处理:
/// 1. citations 字段 (转为 annotations 或保留在 metadata)
/// 2. related_questions 字段处理
pub fn transform_perplexity_response(response: &Value) -> Value {
    let mut openai_response = response.clone();
    
    // 处理 citations - 保留在响应中以便客户端使用
    // 某些客户端 (如 Cherry Studio) 可以正确解析 citations
    if let Some(citations) = response.get("citations") {
        debug!(
            citation_count = citations.as_array().map(|a| a.len()).unwrap_or(0),
            "Processing citations"
        );
        
        // 在 choices[0].message 中添加 metadata
        if let Some(choices) = openai_response.get_mut("choices") {
            if let Some(choice) = choices.get_mut(0) {
                if let Some(message) = choice.get_mut("message") {
                    // 将 citations 添加到消息的 metadata 中
                    if let Some(obj) = message.as_object_mut() {
                        obj.insert("citations".to_string(), citations.clone());
                    }
                }
            }
        }
    }

    // 处理 related_questions - 添加到响应 metadata
    if let Some(related_questions) = response.get("related_questions") {
        debug!(
            question_count = related_questions.as_array().map(|a| a.len()).unwrap_or(0),
            "Processing related questions"
        );
        // 保留在顶层以便客户端使用
    }

    info!(
        model = response.get("model").and_then(|v| v.as_str()).unwrap_or("unknown"),
        has_citations = response.get("citations").is_some(),
        "Perplexity response transformed"
    );

    openai_response
}

/// 转换流式响应 chunk
/// 
/// Perplexity 的流式响应格式与 OpenAI 基本一致
/// citations 在最后一个 chunk 中出现
pub fn transform_stream_chunk(chunk: &Value) -> Value {
    let mut transformed = chunk.clone();
    
    // 处理 citations (出现在最后一个 chunk)
    if let Some(citations) = chunk.get("citations") {
        // 将 citations 添加到 delta 中
        if let Some(choices) = transformed.get_mut("choices") {
            if let Some(choice) = choices.get_mut(0) {
                if let Some(delta) = choice.get_mut("delta") {
                    if let Some(obj) = delta.as_object_mut() {
                        obj.insert("citations".to_string(), citations.clone());
                    }
                }
            }
        }
    }

    transformed
}

/// 格式化 SSE 数据行
pub fn format_sse_line(data: &Value) -> String {
    format!("data: {}\n\n", serde_json::to_string(data).unwrap_or_default())
}

/// 创建 SSE 结束标记
pub fn format_sse_done() -> String {
    "data: [DONE]\n\n".to_string()
}

/// 将 citations 转换为 Markdown 格式的引用文本
/// 
/// 用于在响应文本末尾添加引用来源链接
pub fn format_citations_as_markdown(citations: &[String]) -> String {
    if citations.is_empty() {
        return String::new();
    }

    let mut markdown = String::from("\n\n---\n**Sources:**\n");
    for (i, url) in citations.iter().enumerate() {
        markdown.push_str(&format!("{}. [{}]({})\n", i + 1, url, url));
    }
    markdown
}

/// 创建错误响应
pub fn create_error_response(status_code: u16, message: &str, error_type: &str) -> Value {
    json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": status_code.to_string()
        }
    })
}

/// 创建流式错误响应
pub fn create_stream_error_response(message: &str, error_type: &str) -> String {
    let error = json!({
        "error": {
            "message": message,
            "type": error_type
        }
    });
    format!("data: {}\n\n", serde_json::to_string(&error).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transform_response_with_citations() {
        let response = json!({
            "id": "resp-123",
            "model": "sonar",
            "object": "chat.completion",
            "created": 1234567890,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "citations": [
                "https://example.com/1",
                "https://example.com/2"
            ]
        });

        let transformed = transform_perplexity_response(&response);
        
        // citations 应该被添加到 message 中
        let citations = transformed["choices"][0]["message"]["citations"].as_array();
        assert!(citations.is_some());
        assert_eq!(citations.unwrap().len(), 2);
    }

    #[test]
    fn test_transform_stream_chunk() {
        let chunk = json!({
            "id": "resp-123",
            "model": "sonar",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Hello"
                }
            }]
        });

        let transformed = transform_stream_chunk(&chunk);
        assert_eq!(transformed["choices"][0]["delta"]["content"], "Hello");
    }

    #[test]
    fn test_format_citations_markdown() {
        let citations = vec![
            "https://example.com/1".to_string(),
            "https://example.com/2".to_string(),
        ];

        let markdown = format_citations_as_markdown(&citations);
        
        assert!(markdown.contains("Sources"));
        assert!(markdown.contains("https://example.com/1"));
        assert!(markdown.contains("https://example.com/2"));
    }

    #[test]
    fn test_format_citations_empty() {
        let citations: Vec<String> = vec![];
        let markdown = format_citations_as_markdown(&citations);
        assert!(markdown.is_empty());
    }

    #[test]
    fn test_format_sse_line() {
        let data = json!({"test": "value"});
        let sse = format_sse_line(&data);
        assert!(sse.starts_with("data: "));
        assert!(sse.ends_with("\n\n"));
    }

    #[test]
    fn test_create_error_response() {
        let error = create_error_response(400, "Bad request", "invalid_request");
        assert_eq!(error["error"]["message"], "Bad request");
        assert_eq!(error["error"]["type"], "invalid_request");
    }
}
