// 比赛服务：比赛获取、列表缓存、比赛元信息缓存、当前比赛切换、榜单查询、比赛公告与已读状态。
//
// 比赛列表带 TTL 缓存（TTL 来自配置，逐调用可变）；比赛元信息（标题/时间窗/封榜设置）
// 带固定 TTL 的内存 + 磁盘缓存 —— 题目总览页每 30s 轮询 `load_configured_contest`
// （元信息 + 题目列表两次请求），缓存元信息可把轮询请求量减半，而 ac/total 仍在
// 每次轮询实时拉取。切换比赛时发布 ContestEvent::Selected。
// 公告**不缓存**（可能含裁判组临场规则变更）；已读状态是客户端本地特性，
// 持久化在 `announcements_read/{cid}_{uid}.json`。
//
// **错误处理约定**：向上传播 Provider 错误时一律用 `AppError::context()` 补环节名，
// 不得用 `AppError::Contest(format!(...))` 重新包装 —— 变体是前端 `sessionGuard`
// 判定会话失效的唯一依据（见 `core/error.rs` 的 `context()` 文档）。
// `get_rank` 尤其关键：它是全场最高频的认证调用（每 10s 一次），
// 变体被改写会让 token 过期时榜单静默 stale、选手永远回不到登录页。
pub mod error;

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::entity::announcement::AnnouncementPage;
use crate::core::entity::contest::{Contest, ContestBundle};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, ContestEvent};
use crate::core::event::event_bus::EventBus;
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
    event_bus: Arc<EventBus>,
    storage: Arc<Storage>,
    /// 当前选中的比赛 ID
    current_contest: RwLock<Option<String>>,
    /// 比赛列表缓存（TTL 来自配置，逐调用可变，故保留自持实现）
    cache: RwLock<Option<ContestCache>>,
    /// 比赛元信息内存缓存（固定 TTL，见 [`CONTEST_META_TTL`]）
    meta_cache: TtlCache<String, Contest>,
    /// 比赛元信息磁盘缓存（跨重启；用户域数据不落盘，本项属公共数据）
    meta_disk: JsonDiskCache,
}

impl ContestService {
    /// 创建 ContestService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        event_bus: Arc<EventBus>,
        storage: Arc<Storage>,
    ) -> Self {
        Self {
            registry,
            event_bus,
            meta_cache: TtlCache::new(CONTEST_META_TTL, CONTEST_META_CAPACITY),
            meta_disk: JsonDiskCache::new(Arc::clone(&storage), CONTEST_META_NAMESPACE),
            storage,
            current_contest: RwLock::new(None),
            cache: RwLock::new(None),
        }
    }

    /// 获取比赛元信息（内存 → 磁盘 → 网络），命中即回填上游缓存。
    ///
    /// 只缓存**成功结果**：Provider 错误（含 401/403）原样上抛，绝不入缓存 ——
    /// 否则会话失效会被缓存掩盖，前端 `sessionGuard` 拿不到 `Auth` 变体。
    async fn load_contest_meta(&self, contest_id: &str) -> AppResult<Contest> {
        let key = contest_id.to_string();

        if let Some(contest) = self.meta_cache.get(&key) {
            debug!(contest_id = contest_id, hit = true, "命中比赛元信息内存缓存");
            return Ok(contest);
        }

        if let Some(contest) = self
            .meta_disk
            .read::<Contest>(contest_id, CONTEST_META_TTL)
        {
            debug!(contest_id = contest_id, hit = true, "命中比赛元信息磁盘缓存");
            self.meta_cache.insert(key, contest.clone());
            return Ok(contest);
        }

        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_contest(&oj_type)?;
        let contest = provider.get_contest(contest_id).await.map_err(|e| {
            warn!(error = %e, "获取比赛详情失败");
            e.context("获取比赛详情失败")
        })?;

        self.meta_cache.insert(key, contest.clone());
        self.meta_disk.write(contest_id, &contest);
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

        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_contest(&oj_type)?;

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

        self.event_bus
            .publish(&AppEvent::Contest(ContestEvent::ListLoaded {
                contests: contests.clone(),
            }));

        Ok(contests)
    }

    /// 选中比赛并发布事件。
    pub fn select_contest(&self, contest_id: &str) -> AppResult<()> {
        let mut current = self.current_contest.write().unwrap_or_else(|e| e.into_inner());
        *current = Some(contest_id.to_string());

        info!(contest_id = contest_id, "已选中比赛");
        self.event_bus.publish(&AppEvent::Contest(ContestEvent::Selected {
            contest_id: contest_id.to_string(),
        }));

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

    /// 获取比赛排行榜（分页）。
    ///
    /// **不做缓存**：HOJ 内榜每次实时计算（见 `doc/HOJ/HOJ-Contest-Rank-API.md` §4），
    /// 缓存反而会给出过期名次；轮询节奏由前端控制（≥10s 且加抖动错峰、后台暂停）。
    pub async fn get_rank(
        &self,
        contest_id: &str,
        query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_contest(&oj_type)?;

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
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_contest(&oj_type)?;

        info!(contest_id = contest_id, "加载比赛");
        let contest = self.load_contest_meta(contest_id).await?;

        // 私有赛需要密码
        if contest.auth == 1 {
            if let Some(_pw) = password {
                // TODO: 阶段 7 后续实现比赛注册 API 调用
                debug!("私有赛密码已提供");
            }
        }

        let problems = provider.list_contest_problems(contest_id).await.map_err(|e| {
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
    pub async fn list_announcements(
        &self,
        contest_id: &str,
        current_page: i64,
        limit: i64,
    ) -> AppResult<AnnouncementPage> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_contest(&oj_type)?;

        let page = provider
            .list_announcements(contest_id, current_page, limit)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, error = %e, "获取比赛公告失败");
                e.context("获取比赛公告")
            })?;

        debug!(contest_id = contest_id, count = page.records.len(), "比赛公告已获取");
        Ok(page)
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
