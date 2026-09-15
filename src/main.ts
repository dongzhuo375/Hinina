import { createApp } from "vue";
import { createPinia } from "pinia";
import router from "@/router";
import App from "@/App.vue";
import { installSessionGuard } from "@/stores/sessionGuard";
import "@/styles/global.css";

const app = createApp(App);

app.use(createPinia());
app.use(router);
// 组合根装配：全局会话守卫（认证类 IPC 失败 → 判定会话失效 → 清理并回登录页）
installSessionGuard(router);

app.mount("#app");
