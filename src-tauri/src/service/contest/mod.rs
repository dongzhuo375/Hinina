// 比赛服务：比赛获取、列表缓存、比赛元信息缓存、当前比赛切换、榜单查询、比赛公告与已读状态。
//
// 比赛列表带 TTL 缓存（TTL 来自配置，逐调用可变）；比赛元信息（标题/时间窗/封榜设置）
// 带固定 TTL 的内存 + 磁盘缓存 —— 题目总览页每 30s 轮询 `load_configured_contest`
// （元信息 + 题目列表两次请求），缓存元信息可把轮询请求量减半，而 ac/total 仍在
// 每次轮询实时拉取。切换比赛时发布 CoreEvent::ContestSelected（事实通知）。
// 公告**不缓存**（可能含裁判组临场规则变更）；已读状态是客户端本地特性，
// 持久化在 `announcements_read/{cid}_{uid}.json`。
//
// **错误处理约定**：向上传播 Provider 错误时一律用 `AppError::context()` 补环节名，
// 不得用 `AppError::Contest(format!(...))` 重新包装 —— 变体是前端 `sessionGuard`
// 判定会话失效的唯一依据（见 `core/error.rs` 的 `context()` 文档）。
// `get_rank` 尤其关键：它是全场最高频的认证调用（每 10s 一次），
// 变体被改写会让 token 过期时榜单静默 stale、选手永远回不到登录页。
pub mod error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::entity::announcement::AnnouncementPage;
use crate::core::entity::contest::{Contest, ContestBundle};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::{AppError, AppResult};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::cache::{JsonDiskCache, TtlCache};
use crate::infra::storage::Storage;

/// 比赛列表缓存。
struct ContestCache {
    contests: Vec<Contest>,
    fetched_at: Instant,
}

/// 比赛元信息缓存 TTL。
///
/// 120s 覆盖题目总览页 30s±5s 轮询的 4 个周期（请求量 −75%），同时把
/// 「管理员改比赛时间/封榜设置」的滞后压到 2 分钟内。
/// 比赛是否结束由前端依 `endTime` 推导（`utils/contest.getContestPhase`），
/// **不受**本缓存影响 —— 时间窗本身极少变化。
const CONTEST_META_TTL: Duration = Duration::from_secs(120);

/// 比赛元信息内存缓存容量（同时打开的比赛数量级远小于此）。
const CONTEST_META_CAPACITY: usize = 8;

/// 比赛元信息磁盘缓存目录（跨重启复用；过期由 `fetchedAt` 判定）。
const CONTEST_META_NAMESPACE: &str = "cache/contest_meta";

/// 公告已读状态持久化目录（客户端本地特性，HOJ 无对应服务端接口）。
const ANNOUNCEMENTS_READ_DIR: &str = "announcements_read";

/// 公告已读状态文件结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadAnnouncementState {
    #[serde(rename = "readIds", default)]
    pub read_ids: Vec<String>,
}

/// 比赛服务。
pub struct ContestService {
    registry: Arc<dyn ProviderRegistry>,
    event_bus: Arc<CoreEventBus>,
    storage: Arc<Storage>,
    /// 当前选中的比赛 ID
    current_contest: RwLock<Option<String>>,
    /// 比赛列表缓存（TTL 来自配置，逐调用可变，故保留自持实现）
    cache: Arc<RwLock<Option<ContestCache>>>,
    /// 比赛元信息内存缓存（固定 TTL，见 [`CONTEST_META_TTL`]）
    meta_cache: Arc<TtlCache<String, Contest>>,
    /// 比赛元信息磁盘缓存（跨重启；用户域数据不落盘，本项属公共数据）
    meta_disk: Arc<JsonDiskCache>,
    /// 上次拉取到的公告 ID 基线（contest_id → 公告 ID 列表）。
    ///
    /// 用于检测「新公告」并发布 `CoreEvent::AnnouncementChanged`。
    /// **每个比赛一条基线**：同一进程内可能先看 1011 再看 1012，
    /// 只留一份会让切回旧比赛时把已有公告误判成新公告（红点误报）。
    announcement_baseline: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ContestService {
    /// 创建 ContestService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        event_bus: Arc<CoreEventBus>,
        storage: Arc<Storage>,
    ) -> Self {
        let cache = Arc::new(RwLock::new(None));
        let meta_cache = Arc::new(TtlCache::new(CONTEST_META_TTL, CONTEST_META_CAPACITY));
        let meta_disk = Arc::new(JsonDiskCache::new(
            Arc::clone(&storage),
            CONTEST_META_NAMESPACE,
        ));
        Self {
            registry,
            event_bus,
            meta_cache,
            meta_disk,
            storage,
            current_contest: RwLock::new(None),
            cache,
            announcement_baseline: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// OJ 切换后的缓存清理（**由 `switch_oj` 命令显式调用**，不订阅事件）。
    ///
    /// 为什么不走事件消费者：`switch_oj` 返回后紧接着就可能有新 OJ 的查询进来，
    /// 「清缓存」是切换正确性的一部分，不能依赖异步投递的时机（消费者可能落后、
    /// 可能不存在）。旧实现靠 `EventBus` 的同步投递保证「发布即已清」，
    /// 那种保证建立在「发布方与订阅者同栈」的隐含前提上 —— 改成 broadcast 后
    /// 该前提不再成立，故显式化。
    ///
    /// 两段语义不同：
    /// - **内存段（同步）**：便宜且让当前会话立刻回到干净状态；
    /// - **磁盘段（异步）**：纯空间回收 —— 缓存键自带 **OJ 维度**
    ///   （`{oj}/{contest_id}`），跨 OJ 撞号在结构上不可能，延迟清理不影响正确性。
    ///   无 tokio 上下文时（纯同步调用 / 单元测试）退化为同步执行。
    pub fn on_oj_switched(&self) {
        if let Ok(mut c) = self.cache.write() {
            *c = None;
        }
        self.meta_cache.clear();
        info!("OJ 已切换：清空比赛列表与元信息内存缓存");

        let meta_disk = Arc::clone(&self.meta_disk);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = meta_disk.clear_namespace();
                debug!("OJ 已切换：清理比赛元信息磁盘缓存");
            });
        } else {
            let _ = meta_disk.clear_namespace();
            debug!("OJ 已切换：清理比赛元信息磁盘缓存（无 tokio 上下文，同步执行）");
        }
    }

    /// 获取比赛元信息（内存 → 磁盘 → 网络），命中即回填上游缓存。
    ///
    /// 只缓存**成功结果**：Provider 错误（含 401/403）原样上抛，绝不入缓存 ——
    /// 否则会话失效会被缓存掩盖，前端 `sessionGuard` 拿不到 `Auth` 变体。
    async fn load_contest_meta(&self, contest_id: &str) -> AppResult<Contest> {
        // 键带 OJ 维度：跨 OJ 同 cid 不会互相命中（不依赖「切 OJ 时清理及时」）
        let key = format!("{}/{}", self.registry.current_id(), contest_id);

        if let Some(contest) = self.meta_cache.get(&key) {
            debug!(
                cache = "contest_meta",
                contest_id = contest_id,
                hit = true,
                "命中比赛元信息内存缓存"
            );
            return Ok(contest);
        }

        if let Some(contest) = self.meta_disk.read::<Contest>(&key, CONTEST_META_TTL) {
            debug!(
                cache = "contest_meta",
                contest_id = contest_id,
                hit = true,
                "命中比赛元信息磁盘缓存"
            );
            self.meta_cache.insert(key, contest.clone());
            return Ok(contest);
        }

        let provider = self.registry.current_contest()?;
        let contest = provider.get_contest(contest_id).await.map_err(|e| {
            warn!(error = %e, "获取比赛详情失败");
            e.context("获取比赛详情失败")
        })?;

        self.meta_cache.insert(key.clone(), contest.clone());
        self.meta_disk.write(&key, &contest);
        Ok(contest)
    }

    /// 获取比赛列表，优先使用缓存（TTL 由 Config 控制）。
    ///
    /// 默认 TTL 为 60 秒，如果缓存未过期则直接返回。
    pub async fn list_contests(&self, cache_ttl_secs: u64) -> AppResult<Vec<Contest>> {
        // 检查缓存
        {
            let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
            if let Some(ref c) = *cache {
                if c.fetched_at.elapsed().as_secs() < cache_ttl_secs {
                    debug!("使用缓存的比赛列表");
                    return Ok(c.contests.clone());
                }
            }
        }

        let provider = self.registry.current_contest()?;

        info!("获取比赛列表");
        let contests = provider.list_contests().await.map_err(|e| {
            warn!(error = %e, "获取比赛列表失败");
            e.context("获取比赛列表失败")
        })?;

        debug!(count = contests.len(), "比赛列表已获取");

        // 更新缓存
        {
            let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
            *cache = Some(ContestCache {
                contests: contests.clone(),
                fetched_at: Instant::now(),
            });
        }

        Ok(contests)
    }

    /// 选中比赛并发布 `CoreEvent::ContestSelected`（事实通知）。
    ///
    /// **不再发布 `ListLoaded`**：比赛列表是查询结果，真实数据由 IPC 返回给调用方；
    /// 把查询结果伪装成事件既浪费一次全量克隆，又制造「前端可能靠事件拿数据」
    /// 的错误预期（前端必须经 IPC 查询）。
    pub fn select_contest(&self, contest_id: &str) -> AppResult<()> {
        let mut current = self
            .current_contest
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *current = Some(contest_id.to_string());

        info!(contest_id = contest_id, "已选中比赛");
        self.event_bus.publish(CoreEvent::ContestSelected {
            contest_id: contest_id.to_string(),
        });

        Ok(())
    }

    /// 获取当前选中的比赛 ID。
    pub fn current_contest_id(&self) -> Option<String> {
        self.current_contest
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// 强制刷新比赛列表（跳过缓存）。
    pub async fn refresh(&self) -> AppResult<Vec<Contest>> {
        // 清空缓存
        {
            let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
            *cache = None;
        }
        // 元信息缓存同步失效：强制刷新意味着「要服务端真值」，两层都要清
        // （磁盘按 namespace 粗粒度清空：比赛元信息体量小，重拉代价可忽略）
        self.meta_cache.clear();
        self.meta_disk.clear_namespace();

        // 使用 TTL=0 强制刷新
        self.list_contests(0).await
    }

    /// 清空本服务的全部缓存（比赛列表 + 元信息内存 + 元信息磁盘）。
    ///
    /// 供设置页「重置客户端」使用：与 [`ContestService::refresh`] 的区别是
    /// **清完不重拉**（何时补拉由调用方决定），且不限于当前比赛。
    ///
    /// **刻意不清公告基线**（`announcement_baseline`）：它不是缓存，而是「已经告诉过
    /// 用户哪些公告」的记忆。清掉它会让清空之后新发布的公告在下一次拉取时被当成
    /// 「首次拉取」而**漏报**（红点不亮）。重置语义下要不要连它一起忘掉，由调用方
    /// 显式决定（见 [`ContestService::clear_announcement_baseline`]）。
    pub fn clear_caches(&self) {
        // 锁中毒统一 `into_inner` 取回内部数据（与 `TtlCache` / `provider_registry_impl`
        // 同款约定，见 provider_registry_impl.rs 头注释）：容器是 `Option<ContestCache>`，
        // panic 不会让它结构不一致，而「静默跳过清理」会让重置**留下脏缓存** ——
        // 那是比重置失败更糟的静默后果。
        *self.cache.write().unwrap_or_else(|e| e.into_inner()) = None;
        self.meta_cache.clear();
        let disk = self.meta_disk.clear_namespace();
        info!(disk_cleared = disk, "已清空比赛列表与元信息缓存");
    }

    /// 忘掉公告基线（**仅「重置客户端」用**）。
    ///
    /// 与 `clear_caches` 分开是有意的：基线不是缓存，两者语义不同。
    /// 「清缓存」保留基线（否则清空后新发的公告会漏报）；「重置」则应当连
    /// 「已告知过哪些公告」一起忘掉 —— 重置后一切皆未见，留着基线没有意义。
    pub fn clear_announcement_baseline(&self) {
        // 同 `clear_caches`：锁中毒取回内部数据继续（容器是 HashMap，结构不会不一致）
        self.announcement_baseline
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        info!("已清空公告基线（重置客户端）");
    }

    /// 清空全部公告已读状态（`announcements_read/`）。
    ///
    /// 仅「重置客户端」用：已读状态是**客户端本地事实**（HOJ 无对应接口），
    /// 清掉的表现是红点全部复亮 —— 这正是重置应有的语义。
    /// 返回是否确实删除了目录（不存在时返回 `false`，不算失败）。
    pub fn clear_announcement_read_state(&self) -> bool {
        if !self.storage.exists(ANNOUNCEMENTS_READ_DIR) {
            return false;
        }
        match self.storage.remove_all(ANNOUNCEMENTS_READ_DIR) {
            Ok(()) => {
                info!("已清空公告已读状态（重置客户端）");
                true
            }
            Err(e) => {
                warn!(error = %e, "清空公告已读状态失败");
                false
            }
        }
    }

    /// 获取比赛排行榜（分页）。
    ///
    /// **不做缓存**：HOJ 内榜每次实时计算（见 `doc/HOJ/HOJ-Contest-Rank-API.md` §4），
    /// 缓存反而会给出过期名次；轮询节奏由前端控制（≥10s 且加抖动错峰、后台暂停）。
    pub async fn get_rank(
        &self,
        contest_id: &str,
        query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        let provider = self.registry.current_contest()?;

        let page = provider
            .get_contest_rank(contest_id, query)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, error = %e, "获取比赛榜单失败");
                e.context("获取比赛榜单失败")
            })?;

        debug!(
            contest_id = contest_id,
            rows = page.records.len(),
            total = page.total,
            "比赛榜单已获取"
        );
        Ok(page)
    }

    /// 加载指定比赛及其题目列表，并自动选中。
    ///
    /// 阶段 7 单比赛模式入口：从配置文件读取 contest_id 后调用此方法，
    /// 一次性获取比赛详情 + 题目列表 + 自动选中。
    ///
    /// **缓存策略**：比赛元信息走内存 → 磁盘 → 网络（TTL 120s）；题目列表
    /// **每次实时拉取** —— 题目总览页轮询它就是为了刷新 ac/total 计数，
    /// 缓存题目列表等于让轮询失去意义。
    pub async fn load_contest_with_problems(
        &self,
        contest_id: &str,
        password: Option<&str>,
    ) -> AppResult<ContestBundle> {
        let provider = self.registry.current_contest()?;

        info!(contest_id = contest_id, "加载比赛");
        let contest = self.load_contest_meta(contest_id).await?;

        // 私有赛需要密码
        if contest.auth == 1 {
            if let Some(_pw) = password {
                // TODO: 阶段 7 后续实现比赛注册 API 调用
                debug!("私有赛密码已提供");
            }
        }

        let problems = provider
            .list_contest_problems(contest_id)
            .await
            .map_err(|e| {
                warn!(error = %e, "获取比赛题目列表失败");
                e.context("获取比赛题目列表失败")
            })?;

        // 自动选中
        self.select_contest(contest_id)?;

        debug!(problem_count = problems.len(), "比赛加载完成");
        Ok(ContestBundle { contest, problems })
    }

    /// 获取比赛公告（分页）。
    ///
    /// **不做缓存**：公告可能包含裁判组临场发布的规则变更，必须每次拉取最新数据。
    ///
    /// 每次拉取都会与上次结果对比，**出现新公告 ID 时发布
    /// `CoreEvent::AnnouncementChanged`** —— 红点提醒属状态变更，
    /// 由 `main.rs` 的事件桥转发到 webview，前端据此即时点亮红点。
    /// 首次拉取（该比赛尚无基线）不发事件：没有基线可比，发了等于一开机就亮红点。
    ///
    /// **事件只是刷新触发**：公告内容仍由本方法的返回值承载，事件丢失时
    /// 下一次轮询（前端 60s±10s）会重新发现 —— 轮询不可被事件替代。
    pub async fn list_announcements(
        &self,
        contest_id: &str,
        current_page: i64,
        limit: i64,
    ) -> AppResult<AnnouncementPage> {
        let provider = self.registry.current_contest()?;

        let page = provider
            .list_announcements(contest_id, current_page, limit)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, error = %e, "获取比赛公告失败");
                e.context("获取比赛公告")
            })?;

        self.publish_new_announcements(contest_id, &page);

        debug!(
            contest_id = contest_id,
            count = page.records.len(),
            "比赛公告已获取"
        );
        Ok(page)
    }

    /// 与上次结果对比并发布新公告事件（纯内存操作，无 I/O）。
    ///
    /// 基线**只在成功拉取后更新**：失败时保留旧基线，下一次成功拉取仍能正确
    /// 报出期间新增的公告。基线按比赛隔离，切换比赛不会互相污染。
    fn publish_new_announcements(&self, contest_id: &str, page: &AnnouncementPage) {
        let current: Vec<String> = page.records.iter().map(|a| a.id.clone()).collect();

        let new_ids = {
            // 锁中毒统一 `into_inner` 取回内部数据（与 `TtlCache` / `provider_registry_impl`
            // 同款约定）：基线是 `HashMap<String, Vec<String>>`，panic 不会让它结构不一致，
            // 而「跳过检测」会让中毒后**永久**不再报新公告 —— 静默失效比按正常路径继续更糟
            let mut baselines = self
                .announcement_baseline
                .write()
                .unwrap_or_else(|e| e.into_inner());
            match baselines.get(contest_id) {
                None => {
                    // 首次拉取：只建基线，不发事件
                    baselines.insert(contest_id.to_string(), current);
                    None
                }
                Some(previous) => {
                    let fresh: Vec<String> = current
                        .iter()
                        .filter(|id| !previous.contains(id))
                        .cloned()
                        .collect();
                    baselines.insert(contest_id.to_string(), current);
                    (!fresh.is_empty()).then_some(fresh)
                }
            }
        };

        let Some(new_ids) = new_ids else {
            return;
        };

        info!(
            contest_id = contest_id,
            new_count = new_ids.len(),
            "检测到新比赛公告"
        );
        self.event_bus.publish(CoreEvent::AnnouncementChanged {
            contest_id: contest_id.to_string(),
            new_ids,
        });
    }

    // ── 公告已读状态（客户端本地特性）──

    /// 读取某用户在某比赛下已读的公告 ID 列表。
    ///
    /// 文件不存在视为「从未读过」；文件损坏只告警并降级为空列表 ——
    /// 已读状态是纯 UI 便利特性，任何情况下都不应阻断公告展示。
    pub fn get_read_announcement_ids(&self, contest_id: &str, uid: &str) -> AppResult<Vec<String>> {
        let path = Self::read_state_path(contest_id, uid)?;
        let raw = match self.storage.read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => return Ok(Vec::new()), // 不存在或读取失败：按未读处理
        };
        match serde_json::from_str::<ReadAnnouncementState>(&raw) {
            Ok(state) => Ok(state.read_ids),
            Err(e) => {
                warn!(path = %path, error = %e, "公告已读状态文件损坏，降级为空列表");
                Ok(Vec::new())
            }
        }
    }

    /// 标记公告为已读：与既有记录合并去重后落盘。
    pub fn mark_announcements_read(
        &self,
        contest_id: &str,
        uid: &str,
        ids: &[String],
    ) -> AppResult<()> {
        let path = Self::read_state_path(contest_id, uid)?;

        // 合并去重（保留首次出现顺序）；旧状态损坏时从空列表重建
        let mut merged = self.get_read_announcement_ids(contest_id, uid)?;
        for id in ids {
            if !merged.iter().any(|existing| existing == id) {
                merged.push(id.clone());
            }
        }

        let state = ReadAnnouncementState { read_ids: merged };
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| AppError::Serialization(format!("公告已读状态序列化失败: {}", e)))?;
        self.storage
            .write_string(&path, &json)
            .map_err(|e| e.context("写入公告已读状态失败"))?;
        debug!(contest_id = contest_id, path = %path, "公告已读状态已保存");
        Ok(())
    }

    /// 构造已读状态文件路径：`announcements_read/{cid}_{uid}.json`。
    ///
    /// cid / uid 来自会话与前端入参，必须拒绝路径分隔符，防止写出存储根目录之外的文件。
    fn read_state_path(contest_id: &str, uid: &str) -> AppResult<String> {
        fn sanitize(part: &str, name: &str) -> AppResult<String> {
            if part.is_empty() || part.contains(['/', '\\', ':']) || part.contains("..") {
                return Err(AppError::Io(format!("{} 含非法路径字符: {}", name, part)));
            }
            Ok(part.to_string())
        }
        let cid = sanitize(contest_id, "contest_id")?;
        let uid = sanitize(uid, "uid")?;
        Ok(format!("{}/{}_{}.json", ANNOUNCEMENTS_READ_DIR, cid, uid))
    }
}

#[cfg(test)]
#[path = "tests/contest_tests.rs"]
mod tests;
