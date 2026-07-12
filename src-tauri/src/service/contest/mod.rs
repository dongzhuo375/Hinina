// 比赛服务：比赛获取、列表缓存、当前比赛切换。
//
// 比赛列表带 TTL 缓存，切换比赛时发布 ContestEvent::Selected。
pub mod error;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use tracing::{debug, info, warn};

use crate::core::entity::contest::Contest;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, ContestEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::provider::registry::ProviderRegistry;

/// 比赛列表缓存。
struct ContestCache {
    contests: Vec<Contest>,
    fetched_at: Instant,
}

/// 比赛服务。
pub struct ContestService {
    registry: Arc<dyn ProviderRegistry>,
    event_bus: Arc<EventBus>,
    /// 当前选中的比赛 ID
    current_contest: RwLock<Option<String>>,
    /// 比赛列表缓存
    cache: RwLock<Option<ContestCache>>,
}

impl ContestService {
    /// 创建 ContestService。
    pub fn new(registry: Arc<dyn ProviderRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            registry,
            event_bus,
            current_contest: RwLock::new(None),
            cache: RwLock::new(None),
        }
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
            AppError::Contest(format!("获取比赛列表失败: {}", e))
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

        // 使用 TTL=0 强制刷新
        self.list_contests(0).await
    }
}
