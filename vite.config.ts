import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

/// 客户端版本号在构建期注入（来源 package.json），避免 UI 里写死后与发布版本漂移
const pkgVersion: string = JSON.parse(
  readFileSync(fileURLToPath(new URL("./package.json", import.meta.url)), "utf-8"),
).version;

export default defineConfig({
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

  define: {
    __APP_VERSION__: JSON.stringify(pkgVersion),
  },

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
  /// 另含 doc/modules 下插件 SDK 示例的逻辑测试（mock Hinina 接口，同样不依赖宿主）。
  /// 测试文件置于各模块的 `__tests__/` 子目录（与 Rust 端 tests/ 分离约定一致）。
  test: {
    environment: "node",
    include: ["src/**/__tests__/**/*.spec.ts", "doc/modules/**/__tests__/**/*.spec.ts"],
    clearMocks: true,
    restoreMocks: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      reportsDirectory: "coverage",
      // 只统计 src（SDK 示例在 doc/modules，单独口径，不并入前端指标）。
      // vitest 5 在显式给出 include 后，未被测试加载的匹配文件默认以 0% 计入
      include: ["src/**"],
      exclude: [
        "src/**/__tests__/**",
        "src/test-support/**",
        "src/**/*.d.ts",
        "src/main.ts",
        // Monaco 壳组件：V8 覆盖器无法解析（vitest 本就只是将其排除并告警），显式剔除保持日志干净
        "src/components/editor/CodeEditor.vue",
      ],
    },
  },
});
