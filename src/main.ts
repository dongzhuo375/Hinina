import { createApp } from "vue";
import { createPinia } from "pinia";
import router from "@/router";
import App from "@/App.vue";
import { installSessionGuard } from "@/stores/sessionGuard";
import "@/styles/global.css";
// KaTeX 样式与字体（题面/公告的 LaTeX 公式渲染）。字体由 katex 包本地打包，
// 不经 CDN —— 符合离线客户端约束；全局引入使所有 renderMarkdown 消费方一致生效。
import "katex/dist/katex.min.css";

const app = createApp(App);

app.use(createPinia());
app.use(router);
// 组合根装配：全局会话守卫（认证类 IPC 失败 → 判定会话失效 → 清理并回登录页）
installSessionGuard(router);

app.mount("#app");
