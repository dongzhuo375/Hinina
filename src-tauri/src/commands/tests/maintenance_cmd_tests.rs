// `reset_client` / `purge_local_data` / `local_data_usage` 提取实现的全真上下文测试
// （M2 工作包 A）。锁定「精确删除范围」：可重建的缓存全清、不可重建的本地事实
// （工作区/配置/会话/留档）分毫不动——这是设置页两个危险按钮的行为边界。

use super::*;
use crate::test_support::app_ctx::{context_with_hoj, seed_snapshot};
use crate::test_support::loopback::LoopbackServer;

#[tokio::test]
async fn reset_client_clears_caches_but_keeps_local_facts() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_hoj(server.url()).await;

    // 可重建的缓存与公告已读：应被清
    ctx.storage
        .write_string("cache/contest_meta/1.json", "{}")
        .unwrap();
    ctx.storage
        .write_string("cache/problem_statement/A.json", "{}")
        .unwrap();
    ctx.storage
        .write_string("announcements_read/hoj.json", "[1,2]")
        .unwrap();
    // 本地事实：应保留
    ctx.storage
        .write_string("workspaces/c1/p1/code.cpp", "int main(){}")
        .unwrap();
    ctx.storage
        .write_string("sessions/hoj.json", "{\"username\":\"a\"}")
        .unwrap();
    seed_snapshot(dir.path(), "hoj", "s1", false);

    reset_client_impl(&ctx).expect("重置应成功");

    assert!(
        !ctx.storage.exists("cache/contest_meta/1.json"),
        "比赛元信息缓存应清除"
    );
    assert!(
        !ctx.storage.exists("cache/problem_statement/A.json"),
        "题面缓存应清除"
    );
    assert!(
        !ctx.storage.exists("announcements_read/hoj.json"),
        "公告已读状态应清除"
    );
    assert!(
        ctx.storage.exists("workspaces/c1/p1/code.cpp"),
        "工作区代码绝不能被重置删除"
    );
    assert!(ctx.storage.exists("sessions/hoj.json"), "会话不属重置范围");
    assert!(
        ctx.storage.exists("submissions/hoj/s1.cpp"),
        "提交留档不属重置范围（过期回收走 purge）"
    );

    server.shutdown();
}

#[tokio::test]
async fn purge_removes_only_stale_snapshots_and_reports_numbers() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_hoj(server.url()).await;

    let stale_bytes = seed_snapshot(dir.path(), "hoj", "stale-1", true);
    let fresh_bytes = seed_snapshot(dir.path(), "hoj", "fresh-1", false);

    let report = purge_local_data_impl(&ctx, false, true);

    assert_eq!(report.removed_snapshots, 1, "只应删除过期留档");
    assert_eq!(
        report.freed_bytes, stale_bytes,
        "释放字节应为过期留档的实际体积"
    );
    assert!(
        !dir.join("submissions/hoj/stale-1.cpp").exists(),
        "过期留档应被删除"
    );
    assert!(
        dir.join("submissions/hoj/fresh-1.cpp").exists(),
        "窗口内留档必须保留"
    );
    // 日志未勾选：log_cleared=false 且日志文件未被清空
    assert!(!report.log_cleared, "未勾选日志时 log_cleared 应为 false");

    server.shutdown();
    let _ = fresh_bytes;
}

#[tokio::test]
async fn purge_clears_log_content_when_selected() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_hoj(server.url()).await;
    let log_path = dir.join("logs/hinina.log");
    // init 后 logger 已写入启动日志（文件必然存在且非空）
    assert!(log_path.exists(), "init 后日志文件应已存在");

    // 崩溃文件清理注入空实现：真实实现会删除 %TEMP%/hinina-panic.log，
    // 测试不得触碰开发机上的真实崩溃记录（见 tests/common 的沙箱约定）
    let report = purge_local_data_with(&ctx, true, false, || Ok(0));

    assert!(report.log_cleared, "日志文件存在且可写时应清空成功");
    assert_eq!(
        std::fs::metadata(&log_path).unwrap().len(),
        0,
        "日志内容应被清空（文件保留）"
    );

    server.shutdown();
}

#[tokio::test]
async fn local_data_usage_reports_seeded_footprint() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_hoj(server.url()).await;

    let stale_bytes = seed_snapshot(dir.path(), "HOJ", "stale-1", true);
    let fresh_bytes = seed_snapshot(dir.path(), "HOJ", "fresh-1", false);
    // 日志体积：写入已知字节后断言（tracing 全局订阅器每进程只装一次，
    // 并行测试里本目录的文件层可能收不到事件，不能依赖 init 日志非空）
    let log_bytes = 1234u64;
    std::fs::write(dir.join("logs/hinina.log"), vec![b'x'; log_bytes as usize]).unwrap();

    let usage = local_data_usage_impl(&ctx);

    assert_eq!(usage.snapshot_total_count, 2);
    assert_eq!(usage.snapshot_stale_count, 1);
    assert_eq!(usage.snapshot_total_bytes, stale_bytes + fresh_bytes);
    assert_eq!(usage.snapshot_stale_bytes, stale_bytes);
    assert_eq!(usage.log_bytes, log_bytes);
    assert!(usage.log_path.ends_with("hinina.log"));
    // 崩溃记录路径固定为临时目录下的 hinina-panic.log；字节数取决于开发机
    // 是否留有真实崩溃记录，不做定量断言（测试不得读写真实文件）
    assert!(usage.crash_path.ends_with("hinina-panic.log"));

    server.shutdown();
}
