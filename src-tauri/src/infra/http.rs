/// HTTP 客户端封装（基于 Reqwest）。
///
/// 提供统一的超时、重试、UA、Cookie Store 管理。
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    /// 创建默认 HttpClient（30 秒超时、cookie_store 已启用）
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .user_agent(format!("Hinina/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { client })
    }

    /// 获取内部 reqwest::Client 引用
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}
