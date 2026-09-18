import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import router from "@/router";
import App from "@/App.vue";
import { installSessionGuard } from "@/stores/sessionGuard";
import {
  installWorkspacePersistenceListener,
  useWorkspaceStore,
} from "@/stores/workspaceStore";
import "@/styles/global.css";
// KaTeX 样式与字体（题面/公告的 LaTeX 公式渲染）。字体由 katex 包本地打包，
// 不经 CDN —— 符合离线客户端约束；全局引入使所有 renderMarkdown 消费方一致生效。
import "katex/dist/katex.min.css";

/// 关窗落盘的等待上限（毫秒）：超时即放行关闭，避免 IPC 无响应时窗口关不掉
const CLOSE_FLUSH_TIMEOUT_MS = 3_000;

const app = createApp(App);

app.use(createPinia());
app.use(router);
// 组合根装配：全局会话守卫（认证类 IPC 失败 → 判定会话失效 → 清理并回登录页）
installSessionGuard(router);
// 组合根装配：工作区落盘事件订阅（后台 auto-save 完成 → 清除「编辑中…」指示）
installWorkspacePersistenceListener();
// 组合根装配：关窗握手 —— 退出前把在途改动落盘
installCloseFlushGuard();

app.mount("#app");

/**
 * 关窗握手：窗口关闭前先落盘工作区。
 *
 * 编辑器改动只在 2 秒防抖后才到达后端内存，auto-save 周期最长 30 秒 ——
 * 直接关窗会丢掉这段窗口内的编辑（数据丢失，选手往往到重新打开才发现）。
 * 因此拦截 `close-requested`，落盘后再真正关闭：`preventDefault` 只对首次生效，
 * 落盘完成后 `close()` 会再次触发本回调，此时 `closing` 已置位，放行关闭。
 *
 * 落盘失败不阻断退出：卡住窗口比丢一次自动备份更糟（内容仍留在后端内存，
 * 且下次编辑会重新落盘）。
 */
function installCloseFlushGuard(): void {
  let closing = false;
  void getCurrentWindow()
    .onCloseRequested(async (event) => {
      if (closing) return;
      event.preventDefault();
      closing = true;
      try {
        // 落盘失败或后端无响应都不阻断退出：卡住窗口比丢一次自动备份更糟
        // （内容仍留在后端内存，且下次编辑会重新落盘）
        await Promise.race([
          useWorkspaceStore().saveWorkspace(),
          new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_TIMEOUT_MS)),
        ]);
      } catch (e) {
        console.error("关闭前保存工作区失败:", e);
      } finally {
        void getCurrentWindow().close();
      }
    })
    .catch((e) => {
      console.error("关窗钩子注册失败，退出前可能不落盘:", e);
    });
}

