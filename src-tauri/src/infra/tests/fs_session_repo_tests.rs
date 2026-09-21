use std::sync::Arc;

use super::*;
use crate::core::provider::oj_id::OjId;
use crate::infra::storage::Storage;
use crate::test_support::TempDir;

/// 建仓库 + 持有临时目录守卫（Drop 时回收）。
fn make_repo(name: &str) -> (FsSessionRepository, TempDir) {
    let dir = TempDir::named(name);
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    (FsSessionRepository::new(storage), dir)
}

#[test]
fn save_then_load_roundtrip() {
    let (repo, _dir) = make_repo("hinina-session-repo-roundtrip");
    let session = Session::new("HOJ", "u-1", "alice", "t-1");

    repo.save(&session).unwrap();

    let loaded = repo.load(&OjId::new("HOJ")).unwrap().unwrap();
    assert_eq!(loaded, session);
}

#[test]
fn load_missing_session_returns_none() {
    let (repo, _dir) = make_repo("hinina-session-repo-missing");
    assert_eq!(repo.load(&OjId::new("HOJ")).unwrap(), None);
}

#[test]
fn sessions_are_isolated_per_oj() {
    let (repo, _dir) = make_repo("hinina-session-repo-per-oj");
    repo.save(&Session::new("HOJ", "u-1", "alice", "t-hoj"))
        .unwrap();
    repo.save(&Session::new("QDUOJ", "u-2", "bob", "t-qdu"))
        .unwrap();

    assert_eq!(
        repo.load(&OjId::new("HOJ")).unwrap().unwrap().token,
        "t-hoj"
    );
    assert_eq!(
        repo.load(&OjId::new("QDUOJ")).unwrap().unwrap().token,
        "t-qdu"
    );
}

#[test]
fn remove_is_idempotent() {
    let (repo, _dir) = make_repo("hinina-session-repo-remove");
    repo.save(&Session::new("HOJ", "u-1", "alice", "t-1"))
        .unwrap();

    repo.remove(&OjId::new("HOJ")).unwrap();
    assert_eq!(repo.load(&OjId::new("HOJ")).unwrap(), None);
    // 再删一次不应报错（登出可能被重复触发）
    repo.remove(&OjId::new("HOJ")).unwrap();
}

/// 轮换：会话存在 → 只改 token，其余字段保持不变。
#[test]
fn rotate_token_updates_only_token() {
    let (repo, _dir) = make_repo("hinina-session-repo-rotate");
    repo.save(&Session::new("HOJ", "u-1", "alice", "old"))
        .unwrap();

    assert!(repo.rotate_token(&OjId::new("HOJ"), "new").unwrap());

    let loaded = repo.load(&OjId::new("HOJ")).unwrap().unwrap();
    assert_eq!(loaded.token, "new");
    assert_eq!(loaded.user_id, "u-1");
    assert_eq!(loaded.username, "alice");
    assert_eq!(loaded.oj_id, "HOJ");
}

/// 轮换：会话不存在 → `Ok(false)`，**不得凭空创建**会话文件。
///
/// 这是「原子读-改-写」的实际意义：先 `load` 再 `save` 的写法会在
/// 「登出已删除会话」之后把一条幽灵会话写回磁盘。
#[test]
fn rotate_token_on_missing_session_does_not_resurrect_it() {
    let (repo, _dir) = make_repo("hinina-session-repo-rotate-missing");

    assert!(!repo.rotate_token(&OjId::new("HOJ"), "new").unwrap());
    assert_eq!(repo.load(&OjId::new("HOJ")).unwrap(), None);

    // 删除后轮换同样不得复活
    repo.save(&Session::new("HOJ", "u-1", "alice", "t-1"))
        .unwrap();
    repo.remove(&OjId::new("HOJ")).unwrap();
    assert!(!repo.rotate_token(&OjId::new("HOJ"), "new").unwrap());
    assert_eq!(repo.load(&OjId::new("HOJ")).unwrap(), None);
}

/// 损坏的会话文件如实报错，不静默当成「无会话」—— 掩盖问题会让选手以为从未登录。
#[test]
fn corrupt_session_file_reports_error() {
    let (repo, dir) = make_repo("hinina-session-repo-corrupt");
    std::fs::create_dir_all(dir.join("sessions")).unwrap();
    std::fs::write(dir.join("sessions").join("HOJ.json"), "{ not json").unwrap();

    assert!(repo.load(&OjId::new("HOJ")).is_err());
}

/// 并发轮换不丢更新：所有调用都成功，且最终落盘的一定是某次轮换的完整 token
/// （文件必须仍是合法 JSON，不得出现交错的半截内容）。
#[test]
fn concurrent_rotations_keep_file_consistent() {
    let dir = TempDir::named("hinina-session-repo-concurrent");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsSessionRepository::new(storage));
    repo.save(&Session::new("HOJ", "u-1", "alice", "old"))
        .unwrap();

    let tokens: Vec<String> = (0..8).map(|i| format!("tok-{i}")).collect();
    let handles: Vec<_> = tokens
        .iter()
        .map(|token| {
            let repo = Arc::clone(&repo);
            let token = token.clone();
            std::thread::spawn(move || repo.rotate_token(&OjId::new("HOJ"), &token))
        })
        .collect();

    for handle in handles {
        assert!(handle.join().unwrap().unwrap(), "每次轮换都应成功");
    }

    let final_token = repo.load(&OjId::new("HOJ")).unwrap().unwrap().token;
    assert!(
        tokens.contains(&final_token),
        "最终 token 必须是某次轮换写入的完整值，实际 {final_token}"
    );
}
