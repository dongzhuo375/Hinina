# settings.spec.ts（设置页浏览器验收）

> 源文件：`e2e/specs/settings.spec.ts`

## 职责

验证设置页拆分后的真实 Chromium 渲染与一条 fake IPC 保存链，覆盖 jsdom 无法证明的 scoped CSS、响应式布局和真实滚动/焦点行为。

## 核心用例

| 用例 | 覆盖 |
|------|------|
| 七个真实分区、section 样式与响应式导航保持 | `section.settings-section` 数量与 `settings-0..6` id；h2 18px；数字输入仅底边框、无圆角、等宽字体；索引 1280→175px、1024→140px；点击 About/首组后焦点与 `aria-current` 正确 |
| 真实表单保存经 update_config 落盘 | 字号输入 14→16，点击保存触发一次 `update_config` 和“已保存”；reload 后重新进入设置页仍为 16 |

## 依赖

- `e2e/fixtures.ts`（失败 IPC 时序附件）
- `e2e/fake-tauri.ts`：`loginAndEnterContest` / `callCount`；设置页挂载新增 `get_storage_info` / `local_data_usage` / `get_data_dir` 三个虚构读取命令

## 边界

- 原生目录选择器、真实剪贴板权限、真实 OJ 会话隔离、重启后的目录迁移和 WebView2 最终 sticky/smooth-scroll 表现不在 fake 浏览器中证明，仍属桌面人工验收。
- 保存用例使用字号而非比赛引用，避免修改 fake 比赛引用后 reload 因比赛不存在而掩盖设置页行为。
