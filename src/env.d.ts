/// <reference types="vite/client" />

/// 客户端版本号，构建期由 vite.config.ts 的 define 注入（来源 package.json version）
declare const __APP_VERSION__: string;

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<object, object, unknown>;
  export default component;
}
