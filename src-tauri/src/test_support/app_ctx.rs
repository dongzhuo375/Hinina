//! 全真 `AppContext` 测试构造器（仅 `cfg(test)`）。
//!
//! M2 起命令层测试不再复制等价逻辑：预写 `config.json`（HOJ 指向脚本 server）
//! 后走 `AppContext::init` 装配**完整真实栈**（真实配置/注册表/服务/事件总线，
//! 网络指向 loopback），命令提取出的 `*_impl(&AppContext)` 直接在其上执行。

use super::TempDir;
use crate::core::context::AppContext;
use crate::core::entity::config::{AppConfig, OjConfig, OjInstance};
use crate::infra::data_dir::DataDirSource;

/// 构造全真上下文：HOJ 实例指向 `base_url`（通常是 loopback server）。
///
/// 返回 `(ctx, 目录守卫)`——守卫必须被调用方持有，目录随用例结束回收。
pub async fn context_with_hoj(base_url: &str) -> (AppContext, TempDir) {
    let dir = TempDir::unique("hinina-test-ctx");
    let config = AppConfig {
        oj: OjConfig {
            active: "HOJ".into(),
            instances: vec![OjInstance {
                id: "HOJ".into(),
                base_url: base_url.to_string(),
                enabled: true,
                options: serde_json::Map::new(),
            }],
            ..OjConfig::default()
        },
        ..AppConfig::default()
    };
    std::fs::create_dir_all(dir.path()).unwrap();
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string(&config).unwrap(),
    )
    .unwrap();

    let ctx = AppContext::init(dir.to_path_buf(), dir.to_path_buf(), DataDirSource::Default)
        .await
        .expect("AppContext::init 失败");
    (ctx, dir)
}

/// 在 `submissions/{oj_id}/` 下造一份提交留档，返回其字节数。
///
/// `stale` 为真时把修改时间拨回保留窗口之外（`SNAPSHOT_KEEP_DAYS` 之上），
/// 使其命中 `purge_local_data` 的过期判据。
pub fn seed_snapshot(base: &std::path::Path, oj_id: &str, submit_id: &str, stale: bool) -> u64 {
    let path = base
        .join("submissions")
        .join(oj_id)
        .join(format!("{submit_id}.cpp"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let content = format!("// snapshot {submit_id}\nint main(){{return 0;}}\n");
    std::fs::write(&path, content.as_bytes()).unwrap();
    if stale {
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(31 * 24 * 3600);
        let f = std::fs::File::options().write(true).open(&path).unwrap();
        f.set_modified(old).unwrap();
    }
    std::fs::metadata(&path).unwrap().len()
}
