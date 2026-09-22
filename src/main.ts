import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import router from "@/router";
import App from "@/App.vue";
import { installSessionGuard } from "@/guards/sessionGuard";
import {
  installWorkspacePersistenceListener,
  useWorkspaceStore,
} from "@/stores/workspaceStore";
import { useAnnouncementStore } from "@/stores/announcementStore";
import { onAnnouncementsPublished } from "@/bridge/announcement.bridge";
import { createCloseGuard } from "@/utils/close-guard";
import "@/styles/global.css";
// KaTeX 样式与字体（题面/公告的 LaTeX 公式渲染）。字体由 katex 包本地打包，
// 不经 CDN —— 符合离线客户端约束；全局引入使所有 renderMarkdown 消费方一致生效。
import "katex/dist/katex.min.css";

const app = createApp(App);

app.use(createPinia());
app.use(router);
// 组合根装配：全局会话守卫（认证类 IPC 失败 → 判定会话失效 → 清理并回登录页）
installSessionGuard(router);
// 组合根装配：工作区落盘事件订阅（后台 auto-save 完成 → 清除「编辑中…」指示）
installWorkspacePersistenceListener();
// 组合根装配：新公告事件订阅（Rust 侧检测到新公告 → 即时点亮未读红点）
installAnnouncementListener();
// 组合根装配：关窗握手 —— 退出前把在途改动落盘
installCloseFlushGuard();

app.mount("#app");

/**
 * 新公告通知：Rust 侧比对公告基线后发布 `CoreEvent::AnnouncementChanged`，
 * 经 `src-tauri/src/main.rs` 的事件桥转发到此通道。
 *
 * 公告轮询仍在前端按 60s 节拍跑（拉取本身必须有人发起），但「有新公告」这一
 * 状态变更走事件总线 —— 事件到达即刷新列表，红点随即点亮，不必等下一个周期。
 *
 * **事件只是刷新触发，不是状态来源**：事件丢失时红点退化为下一轮轮询发现。
 */
function installAnnouncementListener(): void {
  void onAnnouncementsPublished(({ contestId }) => {
    const store = useAnnouncementStore();
    // 只认当前比赛的公告：切比赛瞬间可能有在途事件（旧比赛的新公告）
    if (store.contestId && store.contestId !== contestId) return;
    void store.refresh();
  }).catch((e) => {
    console.error("新公告事件订阅失败，红点将退化为轮询发现:", e);
  });
}

/**
 * 关窗握手：窗口关闭前先落盘工作区。
 *
 * 落盘与「一定能关上」的取舍集中在 `utils/close-guard`（含状态机与双层时间上界），
 * 此处只负责把真实窗口操作注入进去。
 */
function installCloseFlushGuard(): void {
  const appWindow = getCurrentWindow();
  const guard = createCloseGuard({
    flush: () => useWorkspaceStore().saveWorkspace(),
    close: () => appWindow.close(),
    // 收尾用 destroy：落盘已完成，不需要再走一遍 close-requested 往返
    //（那个往返正是原实现「窗口关不掉」的失效点）
    destroy: () => appWindow.destroy(),
  });

  void appWindow
    .onCloseRequested((event) => {
      if (guard.handleRequest()) event.preventDefault();
    })
    .catch((e) => {
      console.error("关窗钩子注册失败，退出前可能不落盘:", e);
    });
}

