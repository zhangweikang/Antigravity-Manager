// Perplexity 账号和请求/响应数据模型
//
// Perplexity API 与 OpenAI 高度兼容，但有一些特有字段如 citations

use serde::{Deserialize, Serialize};

/// Perplexity 账号数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityAccount {
    pub id: String,
    pub api_key: String,
    pub name: Option<String>,
    /// 账号是否启用
    #[serde(default)]
    pub enabled: bool,
    /// 是否用于代理服务
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    /// 禁用原因
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    /// 禁用时间戳
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<i64>,
    /// 创建时间戳
    pub created_at: i64,
    /// 最后使用时间戳
    pub last_used: i64,
    /// 用户自定义标签
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
}

fn default_true() -> bool {
    true
}

impl PerplexityAccount {
    pub fn new(id: String, api_key: String, name: Option<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id,
            api_key,
            name,
            enabled: true,
            proxy_enabled: true,
            disabled_reason: None,
            disabled_at: None,
            created_at: now,
            last_used: now,
            custom_label: None,
        }
    }

    pub fn update_last_used(&mut self) {
        self.last_used = chrono::Utc::now().timestamp();
    }
}

/// Perplexity 账号索引 (perplexity_accounts.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityAccountIndex {
    pub version: String,
    pub accounts: Vec<PerplexityAccountSummary>,
}

/// Perplexity 账号摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityAccountSummary {
    pub id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    pub created_at: i64,
    pub last_used: i64,
}

impl PerplexityAccountIndex {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            accounts: Vec::new(),
        }
    }
}

impl Default for PerplexityAccountIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ===== Perplexity API 请求/响应结构 =====

/// Perplexity Chat Completion 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityRequest {
    pub model: String,
    pub messages: Vec<PerplexityMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Perplexity 特有: 搜索时效过滤器
    /// 可选值: "month", "week", "day", "hour"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_recency_filter: Option<String>,
    /// Perplexity 特有: 是否返回搜索结果图片
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_images: Option<bool>,
    /// Perplexity 特有: 是否返回相关问题
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_related_questions: Option<bool>,
    /// Perplexity 特有: 搜索域名过滤
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_domain_filter: Option<Vec<String>>,
}

/// Perplexity 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityMessage {
    pub role: String,
    pub content: PerplexityContent,
}

/// Perplexity 消息内容 (支持字符串或数组格式)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PerplexityContent {
    Text(String),
    Parts(Vec<PerplexityContentPart>),
}

/// Perplexity 内容部分
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PerplexityContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

/// 图片 URL 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

/// Perplexity Chat Completion 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityResponse {
    pub id: String,
    pub model: String,
    pub object: String,
    pub created: i64,
    pub choices: Vec<PerplexityChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<PerplexityUsage>,
    /// Perplexity 特有: 引用来源 URLs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<String>>,
    /// Perplexity 特有: 相关问题
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_questions: Option<Vec<String>>,
}

/// Perplexity 选择项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityChoice {
    pub index: u32,
    pub message: PerplexityAssistantMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<PerplexityDelta>,
}

/// Perplexity 助手消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityAssistantMessage {
    pub role: String,
    pub content: String,
}

/// Perplexity 流式 Delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Perplexity 使用量统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Perplexity 流式响应块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityStreamChunk {
    pub id: String,
    pub model: String,
    pub object: String,
    pub created: i64,
    pub choices: Vec<PerplexityStreamChoice>,
    /// Perplexity 特有: 引用来源 (在最后一个 chunk 中出现)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<String>>,
}

/// Perplexity 流式选择项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityStreamChoice {
    pub index: u32,
    pub delta: PerplexityDelta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Perplexity 支持的模型列表
pub const PERPLEXITY_MODELS: &[&str] = &[
    "sonar",
    "sonar-pro",
    "sonar-reasoning",
    "sonar-reasoning-pro",
];

/// 获取 Perplexity 模型信息
pub fn get_perplexity_model_info(model: &str) -> Option<PerplexityModelInfo> {
    match model {
        "sonar" => Some(PerplexityModelInfo {
            id: "sonar".to_string(),
            name: "Sonar".to_string(),
            description: "Standard Perplexity model with web search".to_string(),
            context_length: 127072,
        }),
        "sonar-pro" => Some(PerplexityModelInfo {
            id: "sonar-pro".to_string(),
            name: "Sonar Pro".to_string(),
            description: "Advanced Perplexity model with enhanced capabilities".to_string(),
            context_length: 127072,
        }),
        "sonar-reasoning" => Some(PerplexityModelInfo {
            id: "sonar-reasoning".to_string(),
            name: "Sonar Reasoning".to_string(),
            description: "Perplexity model optimized for reasoning tasks".to_string(),
            context_length: 127072,
        }),
        "sonar-reasoning-pro" => Some(PerplexityModelInfo {
            id: "sonar-reasoning-pro".to_string(),
            name: "Sonar Reasoning Pro".to_string(),
            description: "Advanced reasoning model".to_string(),
            context_length: 127072,
        }),
        _ => None,
    }
}

/// Perplexity 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub context_length: u32,
}

/// Perplexity API 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityError {
    pub error: PerplexityErrorDetail,
}

/// Perplexity 错误详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}
