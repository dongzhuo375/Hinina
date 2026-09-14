import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [
    vue(),
    tailwindcss(),
  ],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  /// 将 Monaco Editor 的 worker 文件复制到输出目录
  optimizeDeps: {
    include: ["monaco-editor"],
  },
  worker: {
    format: "es",
  },

  /// 单元测试（Vitest）
  ///
  /// 覆盖范围：不依赖 Tauri 运行时的纯函数（utils/）、跨端错误契约（bridge/）
  /// 与 store/service 状态机；IPC 边界一律用 vi.mock 在 bridge 层打桩，
  /// 因此测试无需真实 Rust 后端或 OJ 服务器。
  /// 测试文件置于各模块的 `__tests__/` 子目录（与 Rust 端 tests/ 分离约定一致）。
  test: {
    environment: "node",
    include: ["src/**/__tests__/**/*.spec.ts"],
    clearMocks: true,
    restoreMocks: true,
  },
}));
