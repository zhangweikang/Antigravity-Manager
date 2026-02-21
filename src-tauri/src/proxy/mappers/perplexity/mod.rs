// Perplexity Mapper 模块
// 负责 OpenAI ↔ Perplexity 协议转换
//
// Perplexity API 与 OpenAI 高度兼容，主要特性：
// 1. 使用 Bearer Token 认证
// 2. 端点格式与 OpenAI 相同 (/chat/completions)
// 3. 特有字段: citations, search_recency_filter, return_images 等

pub mod request;
pub mod response;

pub use request::*;
pub use response::*;
