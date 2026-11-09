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

use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::entity::announcement::AnnouncementPage;
use crate::core::entity::contest::{Contest, ContestBundle, ContestProblem};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::{AppError, AppResult};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::contest::{AccessRejectionKind, ContestProvider};
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

/// 私有赛进入尝试记忆的键：`(oj_id, contest_id, password 指纹)`。
///
/// 密码只以哈希指纹参与键（进程内 `DefaultHasher`，固定键种子）——记忆表
/// 不落明文凭证；u64 指纹在条目量级下碰撞概率可忽略，碰撞后果仅是同 OJ
/// 同比赛的两个密码共用一条记忆（下一周期自然覆写）。
type AccessAttemptKey = (String, String, Option<u64>);

/// 凭证的进程内指纹：同密码恒同值（仅进程内语义，不跨重启、不做安全用途）。
fn credential_fingerprint(password: &Option<String>) -> Option<u64> {
    password.as_ref().map(|pw| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(pw, &mut hasher);
        hasher.finish()
    })
}

/// 尝试条目的结论。
enum AttemptOutcome {
    /// 已成功（保留期内跳过复查）
    Success,
    /// 注册被拒（保留期内复用首次消息）
    Rejected(String),
    /// 仅发起过请求（时间戳即最后尝试发起时间）：网络/序列化/会话失败、
    /// 瞬态与查询阶段业务失败——**不升级为结论**，最小重发间隔内拦住同 key
    /// 重发（在途防重 + 退避），过期后放行
    Attempted,
}

struct AccessAttempt {
    outcome: AttemptOutcome,
    recorded_at: Instant,
}

/// 结论记忆的保留时长（成功复核与注册被拒复用同值）：期内 Success 跳过复查、
/// Rejected 复用首次消息，过期后放行重新校验。
///
/// 「同一组合的尝试结果不变」并不严格成立——服务端可能撤销报名（成功不能
/// 永久可信），HOJ 的报名接口也存在瞬态业务失败（已被分类移出结论记忆，
/// 见 `ContestProvider::classify_access_rejection`）。10 分钟把错误密码下的
/// 报名 POST 压到每 10 分钟一次（远低于服务端限频量级），同时给撤销/瞬态
/// 场景自愈窗口。
const ACCESS_MEMO_RETENTION: Duration = Duration::from_secs(10 * 60);

/// 同 key 两次发起请求的最小间隔（退避）：Attempted 条目在查记忆的同一临界
/// 区内记录（并发第二个加载必然 Busy），
/// 间隔内的后续调用被拦——覆盖「注册请求在途超过轮询间隔」的防重发与
/// 「网络故障」的退避（失败结论不升级时，条目保持 Attempted）。放行后若
/// 原请求仍在途，可能并发重发一次：HTTP 超时上限通常低于该间隔，概率与
/// 后果（一次多余的幂等性不保证的 POST）可接受，结构上互斥需要每 key 锁，
/// 不值得。
const ACCESS_RETRY_MIN_INTERVAL: Duration = Duration::from_secs(45);

/// 私有赛进入尝试记忆：键 [`AccessAttemptKey`]。
///
/// **防轮询重复报名**：题目总览每 30s±5s 轮询进场链路，私有赛密码缺失/错误时
/// 若无记忆会对 OJ 每周期重复提交报名请求（可能触发服务端限频/锁定）。写回
/// 策略由 `ContestProvider::classify_access_rejection` 决定：注册被拒 →
/// Rejected（保留期内复用首次消息）；其余（瞬态/查询阶段失败/网络类）→ 条目
/// 保持 Attempted（退避）。成功条目带保留期（服务端可能撤销报名，不能永久
/// 可信）。时钟经参数注入，过期可单测。仅进程内存，不跨重启。
struct AccessAttemptMemo {
    entries: HashMap<AccessAttemptKey, AccessAttempt>,
}

/// [`AccessAttemptMemo::lookup`] 的结果。
enum AccessMemoLookup {
    /// 已成功（保留期内），直接放行
    Success,
    /// 注册被拒且仍在保留期内：带上首次消息
    Failure(String),
    /// 上次尝试仍在最小重发间隔内（在途或退避）：本轮跳过
    Busy,
    /// 无有效记忆（无记录，或条目已过期——过期条目已被顺手清除）
    Miss,
}

impl AccessAttemptMemo {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    /// 条目是否过期：结论条目按保留期，Attempted 按最小重发间隔。
    /// 关联函数（无 &self）：sweep 的 retain 闭包与 entries 的可变借用并存。
    fn is_expired(attempt: &AccessAttempt, now: Instant) -> bool {
        let ttl = match attempt.outcome {
            AttemptOutcome::Attempted => ACCESS_RETRY_MIN_INTERVAL,
            AttemptOutcome::Success | AttemptOutcome::Rejected(_) => ACCESS_MEMO_RETENTION,
        };
        now.duration_since(attempt.recorded_at) >= ttl
    }

    /// 查记忆。过期条目顺手清除（等价「从未尝试」）。
    fn lookup(&mut self, key: &AccessAttemptKey, now: Instant) -> AccessMemoLookup {
        let expired = matches!(
            self.entries.get(key),
            Some(attempt) if Self::is_expired(attempt, now)
        );
        if expired {
            self.entries.remove(key);
            return AccessMemoLookup::Miss;
        }
        match self.entries.get(key) {
            None => AccessMemoLookup::Miss,
            Some(attempt) => match &attempt.outcome {
                AttemptOutcome::Success => AccessMemoLookup::Success,
                AttemptOutcome::Rejected(msg) => AccessMemoLookup::Failure(msg.clone()),
                AttemptOutcome::Attempted => AccessMemoLookup::Busy,
            },
        }
    }

    /// 写入前清扫全表过期条目：过期记录只在同 key 再查时被发现的话，
    /// 改过多次密码的用户会让旧条目无限累积（每次写回顺带清扫，表为
    /// 常量级小，全表扫描无妨）。
    fn sweep_expired(&mut self, now: Instant) {
        self.entries
            .retain(|_, attempt| !Self::is_expired(attempt, now));
    }

    fn record_success(&mut self, key: AccessAttemptKey, now: Instant) {
        self.sweep_expired(now);
        self.entries.insert(
            key,
            AccessAttempt {
                outcome: AttemptOutcome::Success,
                recorded_at: now,
            },
        );
    }

    fn record_rejected(&mut self, key: AccessAttemptKey, msg: String, now: Instant) {
        self.sweep_expired(now);
        self.entries.insert(
            key,
            AccessAttempt {
                outcome: AttemptOutcome::Rejected(msg),
                recorded_at: now,
            },
        );
    }

    /// 请求发起前记录：同 key 在最小重发间隔内的后续调用被 Busy 拦住
    ///（在途防重 + 退避）。
    fn record_attempted(&mut self, key: AccessAttemptKey, now: Instant) {
        self.sweep_expired(now);
        self.entries.insert(
            key,
            AccessAttempt {
                outcome: AttemptOutcome::Attempted,
                recorded_at: now,
            },
        );
    }
}

/// `ensure_contest_access_core` 的结果。区分「真实发起了 provider 请求」与
/// 「记忆命中复用」：后台兜底路径对前者的**注册被拒**失败升 `warn`（release
/// 默认 info 下 `debug` 不落日志，「真实原因可追溯」的披露依赖这一条；
/// Rejected 记忆已把重试压到保留期一次，不会刷屏），后者保持 `debug`——
/// 复用消息路径未发起请求，按调用频率本就该安静。
enum AccessCoreResult {
    Attempted(AppResult<()>),
    Memoized(AppResult<()>),
    Skipped,
}

/// 私有赛进入校验的独立执行体：调用 provider 的凭证进入能力
/// （`ContestProvider::ensure_contest_access`），带进程内防重复尝试记忆。
///
/// 状态全部 owned（memo / 纪元 Arc、provider Arc、键字段），错误驱动路径与
/// **后台兜底任务**（`run_best_effort_backfill`，经 `tokio::spawn`）共用同一实现。
///
/// `oj_id` 由调用方与 provider 经 registry 同源快照传入（切 OJ 竞态下防键错配）。
/// 记忆命中失败时复用首次的错误消息 —— 轮询重试时用户看到的仍是同一条
/// 可操作的引导，而不是题目列表接口返回的裸服务端报错。查记忆与 Miss 时
/// 记录 Attempted 在**同一临界区**完成（并发的第二个加载必然 Busy：在途防
/// 重发 + 退避）；完成后按 `classify_access_rejection` 写回——仅注册被拒
/// 升级为 Rejected 结论，其余保持 Attempted（退避）。
#[allow(clippy::too_many_arguments)]
async fn ensure_contest_access_core(
    memo: Arc<Mutex<AccessAttemptMemo>>,
    epoch_counter: Arc<AtomicU64>,
    provider: Arc<dyn ContestProvider>,
    oj_id: String,
    contest_id: String,
    password: Option<String>,
    failure_log: tracing::Level,
) -> AccessCoreResult {
    let key: AccessAttemptKey = (oj_id, contest_id.clone(), credential_fingerprint(&password));
    // 纪元捕获：logout/login/reset/切OJ 若发生在途（见 access_epoch 字段文档），
    // 请求结果不得写回清理后的记忆
    let epoch = epoch_counter.load(Ordering::SeqCst);
    let now = Instant::now();

    // **单临界区**完成「查记忆 + Miss 时记录 Attempted」：并发的第二个加载在
    // 第一个 core 记录完放锁后只会看到 Attempted → Busy——双双发起注册 POST
    // 的窗口结构性闭合（此前查与记分两次加锁，两次读取之间并发可双双 Miss，
    // 前端 loadInFlight 去重只是压低概率）。纪元校验在同一临界区内生效：
    // 清理发生在捕获与记录之间时不得把旧账号的时间戳写进新会话的记忆
    //（否则新账号首次尝试会被无辜拦 45s）。
    let lookup = {
        let mut memo = memo.lock().unwrap_or_else(|e| e.into_inner());
        let outcome = memo.lookup(&key, now);
        if matches!(outcome, AccessMemoLookup::Miss)
            && epoch_counter.load(Ordering::SeqCst) == epoch
        {
            memo.record_attempted(key.clone(), now);
        }
        outcome
    };
    match lookup {
        AccessMemoLookup::Success => {
            debug!(contest_id = contest_id, "私有赛进入已成功过，跳过重复请求");
            return AccessCoreResult::Memoized(Ok(()));
        }
        AccessMemoLookup::Failure(msg) => {
            debug!(
                contest_id = contest_id,
                "私有赛进入失败仍在保留期内，复用首次消息"
            );
            return AccessCoreResult::Memoized(Err(
                AppError::Contest(msg).context("私有赛进入校验失败")
            ));
        }
        AccessMemoLookup::Busy => {
            debug!(
                contest_id = contest_id,
                "距上次尝试不足最小重发间隔（在途或退避），本轮跳过"
            );
            return AccessCoreResult::Skipped;
        }
        // 已在本临界区记录 Attempted，直接发起请求（失败不升级结论时条目
        // 自然退避）
        AccessMemoLookup::Miss => {}
    }

    let result = provider
        .ensure_contest_access(&contest_id, password.as_deref())
        .await;
    {
        let mut memo = memo.lock().unwrap_or_else(|e| e.into_inner());
        // **锁内复核纪元**：清理持同一把锁（先递增再清），与「检查+写回」互斥
        //——清理不可能插在检查与插回之间，交错窗口彻底闭合
        if epoch_counter.load(Ordering::SeqCst) == epoch {
            match &result {
                Ok(()) => memo.record_success(key, now),
                // 仅**注册被拒**升级为结论（密码错误等确定性结果：同密码同
                // 结果，保留期内复用首次消息防 30s 轮询重复提交报名）；
                // 权限查询阶段失败、「请稍后再试」类瞬态、网络/序列化/会话
                // 失败一律不升级——Attempted 条目留在原地（最小间隔退避），
                // 过期放行自愈
                Err(AppError::Contest(msg))
                    if provider.classify_access_rejection(result.as_ref().unwrap_err())
                        == AccessRejectionKind::Rejected =>
                {
                    debug!(contest_id = contest_id, "私有赛注册被拒，记入防重试记忆");
                    memo.record_rejected(key, msg.clone(), now);
                }
                // 其余：瞬态、查询阶段失败、网络/序列化/会话类——不升级结论，
                // Attempted 条目退避，过期后照常重试
                _ => {}
            }
        } else {
            debug!(
                contest_id = contest_id,
                "私有赛进入期间记忆已被清理（登出/换账号），放弃写回旧账号结果"
            );
        }
    }
    AccessCoreResult::Attempted(result.map_err(|e| {
        // 运行时选级别：tracing 的日志宏（event!、warn!、debug!）首参都只接受
        // 常量级别，两个分支是宏限制下的最小写法（级别之外完全一致）。错误
        // 驱动 WARN（用户可见错误的来源）、后台 DEBUG（防轮询刷屏）
        match failure_log {
            tracing::Level::WARN => {
                warn!(contest_id = contest_id, error = %e, "私有赛进入校验失败")
            }
            _ => debug!(contest_id = contest_id, error = %e, "私有赛进入校验失败"),
        }
        e.context("私有赛进入校验失败")
    }))
}

/// 提交侧兜底的**真后台**执行体：报名是尽力而为，成功/失败都只记日志
///（失败分级：**注册被拒升 `warn`**——Rejected 记忆已把重试压到保留期一次、
/// 不会刷屏，且 release 默认 info 下 debug 不落日志；Network 等其余保持
/// `debug`——退避期内每 30s 轮询一次，`warn` 会刷屏）——查看侧不受任何影响。
async fn run_best_effort_backfill(
    memo: Arc<Mutex<AccessAttemptMemo>>,
    epoch_counter: Arc<AtomicU64>,
    provider: Arc<dyn ContestProvider>,
    oj_id: String,
    contest_id: String,
    password: Option<String>,
) {
    let result = ensure_contest_access_core(
        memo,
        epoch_counter,
        // 克隆留出本尊：分类失败级别（warn/debug）要在返回后按类别判断
        provider.clone(),
        oj_id,
        contest_id.clone(),
        password,
        tracing::Level::DEBUG,
    )
    .await;
    match result {
        AccessCoreResult::Attempted(Ok(())) => {
            debug!(contest_id = contest_id, "私有赛后台报名完成（提交侧兜底）")
        }
        AccessCoreResult::Attempted(Err(e)) => {
            // 真实发起了请求且失败：注册被拒（确定性结果）升 **warn**——
            // release 默认 info 下 debug 不落日志，「openRank + 错误密码时
            // 真实原因可追溯」的披露依赖这一条；Rejected 记忆已把重试压到
            // 保留期一次，不会刷屏。其余（Network 类退避等）保持 debug。
            if provider.classify_access_rejection(&e) == AccessRejectionKind::Rejected {
                warn!(
                    contest_id = contest_id,
                    error = %e,
                    "私有赛后台报名被拒（不影响查看；改密码后下次轮询自动重试）"
                );
            } else {
                debug!(
                    contest_id = contest_id,
                    error = %e,
                    "私有赛后台报名未生效（不影响查看）"
                );
            }
        }
        // 记忆命中复用：本轮未发起任何请求，保持 debug（按调用频率本就该安静）
        AccessCoreResult::Memoized(Ok(())) => {
            debug!(contest_id = contest_id, "私有赛后台报名已生效（记忆命中）")
        }
        AccessCoreResult::Memoized(Err(e)) => debug!(
            contest_id = contest_id,
            error = %e,
            "私有赛后台报名复用保留期内的失败消息（本轮未发起请求）"
        ),
        AccessCoreResult::Skipped => debug!(
            contest_id = contest_id,
            "距上次尝试不足最小重发间隔，本轮兜底跳过"
        ),
    }
}

/// 比赛服务。
pub struct ContestService {
    access: Arc<crate::core::access::AccessRevision>,
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
    /// 私有赛进入尝试记忆（防轮询重复报名；失败项带保留期）。
    ///
    /// 见 [`AccessAttemptMemo`] 与 [`ACCESS_MEMO_RETENTION`]。
    /// 账号级状态的对冲：报名是账号级持久状态，同 OJ 换账号后旧记忆不再成立，
    /// 故 `logout` / 成功 `login` / `reset_client` / `on_oj_switched` 都显式清空
    /// （见 [`Self::clear_access_attempts`]）。设置页改密码自然换 key。
    /// 仅进程内存，不跨重启。**Arc 包装**：提交侧兜底以 `tokio::spawn` 跑成
    /// 真后台（见 `run_best_effort_backfill`），后台任务需要 owned 状态。
    access_attempts: Arc<Mutex<AccessAttemptMemo>>,
    /// 进入记忆的纪元（[`Self::clear_access_attempts`] 时递增）。
    ///
    /// 清理是**时点**操作：logout/login 期间仍在途的旧会话加载可能在清理之后
    /// 才完成并写回记忆。进入流程在发起请求前捕获纪元、写回前**在 memo 锁内**
    /// 校验（清理持同一把锁先递增再清，与「检查+写回」互斥，窗口彻底闭合）。
    /// Arc 包装理由同上。
    access_epoch: Arc<AtomicU64>,
}

impl ContestService {
    /// 创建 ContestService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        event_bus: Arc<CoreEventBus>,
        storage: Arc<Storage>,
        access: Arc<crate::core::access::AccessRevision>,
    ) -> Self {
        let cache = Arc::new(RwLock::new(None));
        let meta_cache = Arc::new(TtlCache::new(CONTEST_META_TTL, CONTEST_META_CAPACITY));
        let meta_disk = Arc::new(JsonDiskCache::new(
            Arc::clone(&storage),
            CONTEST_META_NAMESPACE,
        ));
        Self {
            access,
            registry,
            event_bus,
            meta_cache,
            meta_disk,
            storage,
            current_contest: RwLock::new(None),
            cache,
            announcement_baseline: Arc::new(RwLock::new(HashMap::new())),
            access_attempts: Arc::new(Mutex::new(AccessAttemptMemo::new())),
            access_epoch: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 清空私有赛进入尝试记忆（`logout` / `login`（成败皆清——provider 先接受
    /// 新凭证再落盘，落盘失败时适配器已持新 token）/ `reset_client` /
    /// `on_oj_switched` 调用）。
    ///
    /// 报名状态是**账号级**的：同 OJ 换账号后，旧账号的成功记忆会让新账号跳过
    /// 进入校验（随后题目接口报「请先到比赛首页输入比赛密码进行注册」——不含
    /// 登录关键词，不触发会话守卫，页面停在无法自愈的错误上），失败记忆则会让
    /// 本可访问的新账号继续看到旧错误。登出/登录即进程内信号，换账号必须清。
    ///
    /// **先持 memo 锁再递增纪元**：写回侧在锁内复核纪元后才插回（见
    /// `ensure_contest_access_core`），清理与「检查+写回」互斥——清理不可能
    /// 插在检查与插回之间，纳秒级交错窗口彻底闭合。
    pub fn clear_access_attempts(&self) {
        let mut memo = self
            .access_attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.access_epoch.fetch_add(1, Ordering::SeqCst);
        memo.clear();
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
        let _transition = self.access.begin();
        *self
            .current_contest
            .write()
            .unwrap_or_else(|e| e.into_inner()) = None;
        // 中毒也必须完成失效（into_inner）：跳过会让旧 OJ 的榜单缓存跨切换存活
        *self.cache.write().unwrap_or_else(|e| e.into_inner()) = None;
        // 私有赛进入尝试记忆一并失效：不同 OJ 的报名状态互不相干，
        // 保留会让切回旧 OJ 时跳过本该重新发起的报名
        self.clear_access_attempts();
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

    /// Read-only remote metadata; never selects a contest or reads the disk cache.
    pub async fn read_metadata(&self, contest_id: &str) -> AppResult<Contest> {
        self.registry
            .current_contest()?
            .get_contest(contest_id)
            .await
    }

    /// 只读比赛题目列表；不选中比赛、不落缓存。
    ///
    /// 插件查询路径专用：与 `load_contest_with_problems`（自动选中 + 元信息缓存）
    /// 是两条刻意分开的取数路径——插件查询绑定运行中的作用域，不应有选中比赛的
    /// 副作用，也不消费为页面轮询服务的缓存。
    pub async fn list_problems(&self, contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        let provider = self.registry.current_contest()?;
        provider
            .list_contest_problems(contest_id)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, error = %e, "获取比赛题目列表失败");
                e.context("获取比赛题目列表失败")
            })
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
        if current.as_deref() != Some(contest_id) {
            let _transition = self.access.begin();
            *current = Some(contest_id.to_string());
        }

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
        // 同款约定）：容器是 `Option<ContestCache>`，
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
    /// 单比赛模式入口：从配置文件读取 contest_id 后调用此方法，
    /// 一次性获取比赛详情 + 题目列表 + 自动选中。
    ///
    /// **缓存策略**：比赛元信息走内存 → 磁盘 → 网络（TTL 120s）；题目列表
    /// **每次实时拉取** —— 题目总览页轮询它就是为了刷新 ac/total 计数，
    /// 缓存题目列表等于让轮询失去意义。
    ///
    /// **私有赛**（`auth == 1`）**错误驱动进入**：题目列表先行 —— 已报名用户、
    /// 比赛管理员、开放了赛外榜单的私有赛（HOJ 对这些用户不校验报名）**同步
    /// 进入流程零额外请求**（配置了密码时的后台兜底仍会发起一次不阻塞的
    /// 权限查询，见下）；仅当题目列表失败且适配器判定属「需凭证进入」类
    /// （[`ContestProvider::is_credential_rejection`]——`Contest` 变体只是必要
    /// 条件，「比赛还未开始」等业务失败同样落在该变体，不得误触发）时才走
    /// [`ensure_contest_access_core`] 并重试一次。密码来自配置
    /// `oj.contest_password`（命令层传入，去空白后空串归一化为 None），
    /// 未配置/错误时报可操作的引导。
    ///
    /// **提交侧兜底**：题目拉取成功但未报名的选手（openRank 私有赛可见不可交
    /// ——HOJ 的提交通道对 `auth == 1` 始终要求报名，与 openRank 无关）在配置了
    /// 密码时后台报名（**`tokio::spawn` 真后台**，记忆防重复，不阻塞本次加载
    /// 返回——首载/记忆过期时的一次权限查询不再拖慢 30s 轮询）；失败只留日志
    /// 不影响查看——查看侧不得被它挡住，管理员未配密码也不受。无 tokio 上下文
    /// 时退化为同步执行（与 `on_oj_switched` 的退化先例一致）。已知限制见
    /// `doc/modules/src-tauri/service/contest/mod.md` 设计要点。
    pub async fn load_contest_with_problems(
        &self,
        contest_id: &str,
        password: Option<&str>,
    ) -> AppResult<ContestBundle> {
        // provider 与 oj_id 必须**同源**快照（registry 按捕获 id 反查 provider）：
        // 两次独立读取在切换竞态下会拿到 A 的 provider 配 B 的 id，
        // 进入记忆键错配会让结果跨 OJ 泄漏
        let (provider, oj_id) = self.registry.current_contest_snapshot()?;
        let oj_id = oj_id.as_str().to_string();

        info!(contest_id = contest_id, "加载比赛");
        let contest = self.load_contest_meta(contest_id).await?;

        let problems = match provider.list_contest_problems(contest_id).await {
            Ok(problems) => problems,
            Err(err) if contest.auth == 1 && provider.is_credential_rejection(&err) => {
                // 首个错误必须留痕：进入失败时用户看到的是引导文案，
                // 原始原因只能在这里追溯；进入成功则本次失败已被修复
                debug!(contest_id = contest_id, error = %err, "题目列表失败属需凭证类，尝试凭证进入");
                match ensure_contest_access_core(
                    Arc::clone(&self.access_attempts),
                    Arc::clone(&self.access_epoch),
                    Arc::clone(&provider),
                    oj_id.clone(),
                    contest_id.to_string(),
                    password.map(str::to_string),
                    tracing::Level::WARN,
                )
                .await
                {
                    // 真实请求与记忆命中复用对错误驱动路径行为一致：
                    // 成功 → 重试题目；失败 → 上抛进入的引导/复用错误
                    AccessCoreResult::Attempted(Ok(())) | AccessCoreResult::Memoized(Ok(())) => {}
                    AccessCoreResult::Attempted(Err(access_err))
                    | AccessCoreResult::Memoized(Err(access_err)) => return Err(access_err),
                    // 距上次尝试不足最小重发间隔（在途或退避）：本轮未发起
                    // 任何请求，原始题目错误上抛——45s 窗口内的暂态展示，
                    // 下周期即有真实结果（成功/引导文案）
                    AccessCoreResult::Skipped => {
                        return Err(err.context("获取比赛题目列表失败"));
                    }
                }
                provider
                    .list_contest_problems(contest_id)
                    .await
                    .map_err(|e| {
                        warn!(error = %e, "获取比赛题目列表失败");
                        e.context("获取比赛题目列表失败")
                    })?
            }
            Err(err) => {
                warn!(error = %err, "获取比赛题目列表失败");
                return Err(err.context("获取比赛题目列表失败"));
            }
        };

        // 提交侧兜底（best-effort，见方法文档）：**spawn 成真后台**——
        // 不阻塞本次加载返回（首载/记忆过期时的一次权限查询或报名不再拖慢
        // 30s 轮询）；失败只留日志，查看侧不受影响。无 tokio 上下文时退化为
        // 同步执行（与 on_oj_switched 的退化先例一致）。
        if contest.auth == 1 && password.is_some() {
            let fut = run_best_effort_backfill(
                Arc::clone(&self.access_attempts),
                Arc::clone(&self.access_epoch),
                Arc::clone(&provider),
                oj_id,
                contest_id.to_string(),
                password.map(str::to_string),
            );
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn(fut);
                }
                Err(_) => fut.await,
            }
        }

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
