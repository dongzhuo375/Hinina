/// 会话代际计数器（叶子模块）。
///
/// 由 `stores/session.ts` 的 `clearDomainState()` 递增；各 store 在「请求发起 →
/// await → 写回」路径上捕获并比对：清理只重置 state、停止定时器，**并不取消
/// 已发出的请求**，旧会话的迟到响应若直接落库会在清理后重建状态
/// （评审 P1：submit_code 重建提交并重启轮询；P2：load_workspace 恢复旧代码；
/// 同类写入方：提交历史、我的题目状态、榜单、比赛、公告）。
///
/// 刻意独立成**不依赖任何 store 的叶子模块**：`session.ts` 是编排层（import 各
/// store），store 若反向 import 它会形成环，违反「store 只持状态」的分层约定
/// （评审四轮 #6）。
let sessionGeneration = 0

/** 当前会话代际（在请求发起时捕获，写回前比对） */
export function getSessionGeneration(): number {
  return sessionGeneration
}

/** 递增会话代际 —— 仅 `clearDomainState()`（含 OJ 切换）调用 */
export function bumpSessionGeneration(): void {
  sessionGeneration += 1
}
