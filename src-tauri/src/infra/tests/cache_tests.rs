use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// 每个用例独立的临时 Storage 根目录（避免磁盘缓存用例相互串扰）
fn temp_storage(name: &str) -> Arc<Storage> {
    let dir = std::env::temp_dir().join(format!("hinina-cache-test-{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    Arc::new(Storage::new(dir))
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

// ── TtlCache：TTL 语义 ──

#[test]
fn ttl_cache_hits_within_ttl() {
    let cache: TtlCache<String, i32> = TtlCache::new(ms(5_000), 8);
    cache.insert("a".into(), 1);

    assert_eq!(cache.get(&"a".into()), Some(1));
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());
}

#[test]
fn ttl_cache_misses_after_expiry() {
    let cache: TtlCache<String, i32> = TtlCache::new(ms(20), 8);
    cache.insert("a".into(), 1);
    thread::sleep(ms(40));

    assert_eq!(cache.get(&"a".into()), None);
    // 过期条目不再计入存活数（即便尚未被回收）
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn ttl_cache_reinsert_refreshes_deadline() {
    let cache: TtlCache<String, i32> = TtlCache::new(ms(60), 8);
    cache.insert("a".into(), 1);
    thread::sleep(ms(40));
    // 同键重插（更新语义）：TTL 重新计时，且不占新容量
    cache.insert("a".into(), 2);
    thread::sleep(ms(40));

    assert_eq!(cache.get(&"a".into()), Some(2));
    assert_eq!(cache.len(), 1);
}

// ── TtlCache：容量与淘汰 ──

#[test]
fn ttl_cache_evicts_oldest_when_full() {
    let cache: TtlCache<String, i32> = TtlCache::new(ms(5_000), 2);
    cache.insert("a".into(), 1);
    thread::sleep(ms(5)); // 保证插入时间可区分
    cache.insert("b".into(), 2);
    thread::sleep(ms(5));
    cache.insert("c".into(), 3);

    // 最旧的 a 被淘汰，其余保留，容量恒有界
    assert_eq!(cache.get(&"a".into()), None);
    assert_eq!(cache.get(&"b".into()), Some(2));
    assert_eq!(cache.get(&"c".into()), Some(3));
    assert_eq!(cache.len(), 2);
}

#[test]
fn ttl_cache_purges_expired_before_evicting_live_entries() {
    // 容量 2：先塞满两条短 TTL 条目，等其过期后再插入新条目 ——
    // 过期项应先被清理，不得挤掉存活条目
    let cache: TtlCache<String, i32> = TtlCache::new(ms(30), 2);
    cache.insert("stale1".into(), 1);
    cache.insert("stale2".into(), 2);
    thread::sleep(ms(50));

    cache.insert("fresh1".into(), 10);
    cache.insert("fresh2".into(), 20);

    assert_eq!(cache.get(&"fresh1".into()), Some(10));
    assert_eq!(cache.get(&"fresh2".into()), Some(20));
    assert_eq!(cache.get(&"stale1".into()), None);
    assert_eq!(cache.len(), 2);
}

#[test]
fn ttl_cache_capacity_zero_is_clamped_to_one() {
    let cache: TtlCache<String, i32> = TtlCache::new(ms(5_000), 0);
    cache.insert("a".into(), 1);
    cache.insert("b".into(), 2);

    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&"b".into()), Some(2));
}

// ── TtlCache：键隔离与失效 ──

#[test]
fn ttl_cache_isolates_keys() {
    let cache: TtlCache<String, String> = TtlCache::new(ms(5_000), 8);
    cache.insert("1/A".into(), "题面A".into());
    cache.insert("2/A".into(), "题面A（另一场比赛）".into());

    assert_eq!(cache.get(&"1/A".into()).as_deref(), Some("题面A"));
    assert_eq!(cache.get(&"2/A".into()).as_deref(), Some("题面A（另一场比赛）"));
    assert_eq!(cache.get(&"1/B".into()), None);
}

#[test]
fn ttl_cache_invalidate_and_clear() {
    let cache: TtlCache<String, i32> = TtlCache::new(ms(5_000), 8);
    cache.insert("a".into(), 1);
    cache.insert("b".into(), 2);

    assert!(cache.invalidate(&"a".into()));
    assert!(!cache.invalidate(&"a".into()));
    assert_eq!(cache.len(), 1);

    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn ttl_cache_capacity_is_enforced_even_when_reviving_expired_key() {
    // 回归：容量检查曾以 `!contains_key` 为前提，导致「已过期但未回收」的键被重新
    // 插入时跳过检查（复活），存活数可超过容量上限。
    let cache: TtlCache<String, i32> = TtlCache::new(ms(50), 2);
    cache.insert("a".into(), 1);
    thread::sleep(ms(70)); // a 过期但仍在表中（无容量压力时不会被回收）

    cache.insert("b".into(), 2);
    cache.insert("c".into(), 3);
    cache.insert("a".into(), 10); // 复活已过期键

    assert_eq!(cache.len(), 2, "容量上界必须恒成立");
    assert_eq!(cache.get(&"a".into()), Some(10));
    assert_eq!(cache.get(&"c".into()), Some(3));
    assert_eq!(cache.get(&"b".into()), None, "满员时应淘汰最旧插入的 b");
}

#[test]
fn ttl_cache_insert_reclaims_expired_entries_without_pressure() {
    // 无容量压力时也回收过期条目（否则过期项常驻内存，容量语义名不副实）
    let cache: TtlCache<String, i32> = TtlCache::new(ms(30), 8);
    cache.insert("a".into(), 1);
    thread::sleep(ms(50));

    cache.insert("b".into(), 2);

    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&"a".into()), None);
    assert_eq!(cache.get(&"b".into()), Some(2));
}

// ── TtlCache：并发（读路径不加写锁，不死锁） ──

#[test]
fn ttl_cache_supports_concurrent_reads_and_writes() {
    let cache: Arc<TtlCache<String, i32>> = Arc::new(TtlCache::new(ms(5_000), 64));
    let hits = Arc::new(AtomicUsize::new(0));

    let readers: Vec<_> = (0..4)
        .map(|worker| {
            let cache = Arc::clone(&cache);
            let hits = Arc::clone(&hits);
            thread::spawn(move || {
                for i in 0..200 {
                    if cache.get(&format!("k{}", (worker * 200 + i) % 32)).is_some() {
                        hits.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for i in 0..32 {
        cache.insert(format!("k{}", i), i as i32);
    }
    for reader in readers {
        reader.join().expect("读线程不应 panic");
    }

    assert!(hits.load(Ordering::Relaxed) > 0, "并发读应至少命中一次");
    assert!(cache.len() <= 64);
}

// ── JsonDiskCache：往返与 TTL ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sample {
    id: String,
    text: String,
}

#[test]
fn disk_cache_round_trip_within_ttl() {
    let storage = temp_storage("round-trip");
    let cache = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    let value = Sample {
        id: "1".into(),
        text: "题面正文".into(),
    };

    cache.write("1/A", &value);
    let loaded: Option<Sample> = cache.read("1/A", ms(60_000));

    assert_eq!(loaded, Some(value));
    // 落盘结构含 fetchedAt（跨重启判定 TTL 的依据）
    let raw = storage
        .read_to_string("sample_ns/1/A.json")
        .expect("缓存文件应存在");
    assert!(raw.contains("fetchedAt"), "磁盘条目必须携带 fetchedAt: {}", raw);
}

#[test]
fn disk_cache_misses_when_missing_or_key_unsafe() {
    let storage = temp_storage("miss");
    let cache = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");

    let missing: Option<Sample> = cache.read("nope", ms(60_000));
    assert_eq!(missing, None);

    // 非法键：不落盘、不读取（放弃缓存而非报错）
    cache.write("../escape", &Sample { id: "x".into(), text: "x".into() });
    let escaped: Option<Sample> = cache.read("../escape", ms(60_000));
    assert_eq!(escaped, None);
    assert!(!storage.exists("escape.json"));
}

#[test]
fn disk_cache_expires_by_fetched_at_and_deletes_file() {
    let storage = temp_storage("expiry");
    let cache = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    cache.write("1/A", &Sample { id: "1".into(), text: "旧".into() });

    // TTL 0：任何已写条目都视为过期
    let expired: Option<Sample> = cache.read("1/A", Duration::from_secs(0));
    assert_eq!(expired, None);
    // 过期即懒删除，避免残留文件被反复解析
    assert!(!storage.exists("sample_ns/1/A.json"));
}

#[test]
fn disk_cache_treats_corrupted_file_as_miss() {
    let storage = temp_storage("corrupt");
    // 先构造（落地布局标记），**再**写损坏文件 —— 反过来的话会被布局清扫删掉，
    // 用例退化成「文件缺失」而非「文件损坏」
    let cache = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    storage
        .write_string("sample_ns/1/A.json", "{ 这不是 JSON")
        .expect("写入损坏文件");

    let loaded: Option<Sample> = cache.read("1/A", ms(60_000));

    assert_eq!(loaded, None);
}

#[test]
fn disk_cache_purges_legacy_layout_once_per_namespace() {
    let storage = temp_storage("legacy-purge");
    // 模拟升级前落下的旧布局条目（键无 OJ 维度），且各 namespace 都没有布局标记
    storage
        .write_string("sample_ns/1/A.json", r#"{"fetchedAt":4102444800,"value":{"id":"old","text":"旧"}}"#)
        .expect("写入旧布局条目");
    storage
        .write_string("other_ns/1/A.json", r#"{"fetchedAt":4102444800,"value":{"id":"old","text":"旧"}}"#)
        .expect("写入另一 namespace 的旧布局条目");

    // 首次构造：只清自己的 namespace（标记是按 namespace 记的，
    // 否则「先构造者写标记」会让其余 namespace 的旧条目永不清扫）
    let cache = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    assert!(!storage.exists("sample_ns/1/A.json"), "本 namespace 的旧条目应被清扫");
    assert!(
        storage.exists("other_ns/1/A.json"),
        "不得越界清理别的 namespace（它会在自己构造时清扫）"
    );

    // 标记就位后，新写入的数据跨实例存活（不得每次启动都清）
    cache.write("1/A", &Sample { id: "new".into(), text: "新".into() });
    let reopened = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    let loaded: Option<Sample> = reopened.read("1/A", ms(60_000));
    assert_eq!(
        loaded.map(|s| s.id),
        Some("new".into()),
        "布局标记就位后不得再次清扫（否则跨重启缓存失效）"
    );

    // 另一 namespace 自己构造时清扫（验证「每个 namespace 各扫一次」）
    JsonDiskCache::new(Arc::clone(&storage), "other_ns");
    assert!(
        !storage.exists("other_ns/1/A.json"),
        "other_ns 在自己的构造时清扫"
    );
}

#[test]
fn disk_cache_survives_new_instance_and_supports_clear() {
    let storage = temp_storage("persist");
    let value = Sample { id: "1".into(), text: "跨实例".into() };
    JsonDiskCache::new(Arc::clone(&storage), "sample_ns").write("1/A", &value);

    // 新实例（模拟重启）仍能命中：TTL 计时依据落盘的 fetchedAt
    let reopened = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    let loaded: Option<Sample> = reopened.read("1/A", ms(60_000));
    assert_eq!(loaded, Some(value));

    assert!(reopened.remove("1/A"));
    assert!(!storage.exists("sample_ns/1/A.json"));

    reopened.write("2/B", &Sample { id: "2".into(), text: "x".into() });
    assert!(reopened.clear_namespace());
    assert!(!storage.exists("sample_ns"));
    // 目录已不存在时清空是幂等 no-op
    assert!(!reopened.clear_namespace());
}

#[test]
fn disk_cache_nested_keys_are_scoped_per_contest() {
    let storage = temp_storage("nested");
    let cache = JsonDiskCache::new(Arc::clone(&storage), "sample_ns");
    cache.write("1/A", &Sample { id: "1A".into(), text: "第一场".into() });
    cache.write("2/A", &Sample { id: "2A".into(), text: "第二场".into() });

    let first: Option<Sample> = cache.read("1/A", ms(60_000));
    let second: Option<Sample> = cache.read("2/A", ms(60_000));

    assert_eq!(first.map(|s| s.id), Some("1A".into()));
    assert_eq!(second.map(|s| s.id), Some("2A".into()));
    assert!(storage.exists("sample_ns/1/A.json"));
    assert!(storage.exists("sample_ns/2/A.json"));
}
