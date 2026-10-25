//! 脚本化 loopback HTTP server：契约测试的共享底座（仅 `cfg(test)`）。
//!
//! 服务 `127.0.0.1:0`（随机端口），按顺序弹出预置响应并录制收到的每个请求
//! （方法 / 路径+查询 / 请求头 / 请求体）。HOJ / Hydro 适配器与 Command 层的
//! 契约测试用它断言「真实网络栈发出的请求长什么样」，替代对判定函数的孤岛测试。
//!
//! 约定：
//! - 每个连接按 HTTP/1.1 keep-alive **循环处理多个请求**（reqwest 会复用连接池
//!   里的连接——曾因「Connection: close 后立即关闭 socket」与连接池复用竞态，
//!   导致下一个 POST 偶发 "error sending request"，评审第三轮 #10 顺带修复）；
//! - 响应队列耗尽后再来请求会得到 500（并计入录制）—— 用例里多出的意外
//!   请求因此显式暴露，而不是被静默吞掉；
//! - 请求头录制为小写键值对，查询串保留在 `path` 中原样不拆。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// 一条预置响应。
#[derive(Debug, Clone)]
pub struct ScriptedResponse {
    pub status: u16,
    /// 附加响应头（如 `Set-Cookie`、`Refresh-Token`）；`Content-Length` 自动生成
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// 发送本响应前的延迟：用于制造「请求在途期间测试侧继续操作」的真实竞态
    /// （如探测进行中换号登录）。
    pub delay: std::time::Duration,
}

impl ScriptedResponse {
    /// 200 + JSON 体（最常用形态）。
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: body.into(),
            delay: std::time::Duration::ZERO,
        }
    }

    /// 指定状态码 + 响应头 + 体。
    pub fn with(status: u16, headers: Vec<(String, String)>, body: impl Into<String>) -> Self {
        Self {
            status,
            headers,
            body: body.into(),
            delay: std::time::Duration::ZERO,
        }
    }

    /// 延迟 `delay` 后再发送本响应（并发竞态测试用）。
    pub fn delayed(mut self, delay: std::time::Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// 录制到的一条请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    /// 路径 + 查询串（如 `/api/login`、`/api/contest-submissions?limit=20`）
    pub path: String,
    /// 小写头名 → 值
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl RecordedRequest {
    /// 取首个命中的头（小写）。
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// 运行中的脚本 server。
pub struct LoopbackServer {
    url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: JoinHandle<()>,
}

impl LoopbackServer {
    /// 启动并返回 server。`responses` 按请求到达顺序逐一弹出。
    pub async fn start(responses: Vec<ScriptedResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback bind 失败");
        let addr = listener.local_addr().expect("local_addr 失败");
        let queue: VecDeque<ScriptedResponse> = responses.into();
        let queue = Arc::new(Mutex::new(queue));
        let requests: Arc<Mutex<Vec<RecordedRequest>>> = Arc::new(Mutex::new(Vec::new()));

        let handle = tokio::spawn(run_server(
            listener,
            Arc::clone(&queue),
            Arc::clone(&requests),
        ));

        Self {
            url: format!("http://{}", addr),
            requests,
            handle,
        }
    }

    /// 注入给适配器/配置的 base_url。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 已录制的请求快照。
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("requests 锁中毒").clone()
    }

    /// 停止 server（accept 循环 abort；在途连接最多处理完当前请求）。
    pub fn shutdown(self) {
        self.handle.abort();
    }
}

async fn run_server(
    listener: TcpListener,
    queue: Arc<Mutex<VecDeque<ScriptedResponse>>>,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let queue = Arc::clone(&queue);
        let requests = Arc::clone(&requests);
        // 慢客户端由测试超时兜底；连接按 keep-alive 循环处理直到对端关闭
        tokio::spawn(async move {
            loop {
                let Some(record) = read_request(&mut socket).await else {
                    return;
                };
                // 录制与弹出必须在同一临界区：带延迟的响应若在弹出后 sleep，
                // 后续连接会抢先弹出它（响应分配序 ≠ 请求到达序），脚本彻底错位
                let response = {
                    let mut queue = queue.lock().expect("queue 锁中毒");
                    requests
                        .lock()
                        .expect("requests 锁中毒")
                        .push(record.clone());
                    queue.pop_front().unwrap_or_else(|| {
                        // 队列耗尽：显式 500，让「测试多打了请求」在断言里现形
                        ScriptedResponse {
                            status: 500,
                            headers: Vec::new(),
                            body: "scripted queue exhausted".into(),
                            delay: std::time::Duration::ZERO,
                        }
                    })
                };
                if !response.delay.is_zero() {
                    // 延迟必须在锁外：持锁 sleep 会冻结整个 server
                    tokio::time::sleep(response.delay).await;
                }

                let mut head = format!(
                    "HTTP/1.1 {} {}\r\nContent-Length: {}\r\n",
                    response.status,
                    reason_phrase(response.status),
                    response.body.len(),
                );
                for (k, v) in &response.headers {
                    head.push_str(&format!("{}: {}\r\n", k, v));
                }
                head.push_str("\r\n");

                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(response.body.as_bytes()).await;
                let _ = socket.flush().await;
            } // keep-alive 循环：继续读同连接上的下一个请求
        });
    }
}

/// 读取一个完整请求（头 + Content-Length 体）；解析失败返回 `None` 并断开。
async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<RecordedRequest> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    // 读到头部结束符
    let head_end = loop {
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos;
        }
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 64 * 1024 {
            return None; // 头部异常超大，按坏请求处理
        }
    };

    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim().to_string();
        if k == "content-length" {
            content_length = v.parse().unwrap_or(0);
        }
        headers.push((k, v));
    }

    // 读体（可能已随头部一起到达）
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < content_length {
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Some(RecordedRequest {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 底座自身必须可用：真实 reqwest GET 打到脚本 server，请求被录制、响应被消费。
    #[tokio::test]
    async fn serves_scripted_response_and_records_request() {
        let server = LoopbackServer::start(vec![ScriptedResponse::json("{\"ok\":true}")]).await;

        // no_proxy：自测打 loopback，环境代理会劫持 127.0.0.1 请求
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let resp = client
            .get(format!("{}/probe?x=1", server.url()))
            .header("X-Custom", "v")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "{\"ok\":true}");

        let reqs = server.requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].method, "GET");
        assert_eq!(reqs[0].path, "/probe?x=1");
        assert_eq!(reqs[0].header("x-custom"), Some("v"));
        server.shutdown();
    }

    /// 队列耗尽 → 500：意外的多余请求必须显式失败而不是悬空。
    #[tokio::test]
    async fn exhausted_queue_answers_500() {
        let server = LoopbackServer::start(vec![]).await;
        // no_proxy：自测打 loopback，环境代理会劫持 127.0.0.1 请求
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let resp = client.get(server.url().to_string()).send().await.unwrap();
        assert_eq!(resp.status(), 500);
        assert_eq!(server.requests().len(), 1);
        server.shutdown();
    }
}
