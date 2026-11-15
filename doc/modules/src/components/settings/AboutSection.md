# AboutSection（存储信息分组）

> 源文件：`src/components/settings/AboutSection.vue`

## 职责

展示客户端版本、存储目录与日志路径（三份同构行经 `AboutInfoRow` 渲染，逐项复制），并提供逐项复制反馈；读取中/失败回退行与末行项目仓库外链（GitHub 图标 + 仓库 slug，内联 SVG）保持手写（独有形态）。组件只呈现 `useAboutSettings` 模型，复制反馈由共享 `useTransientValue` 管理。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `model` | prop `ReturnType<typeof useAboutSettings>` | `storage/storageFailed/loadStorage/copiedKey/copyText` |

## 直接依赖

- `SettingsSectionFrame`（区块头 + 分组样式壳）
- `AboutInfoRow`（三份复制行的形态单点）
- `@/composables/useAboutSettings`（模型类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

父级挂载调用 `loadStorage()`；读取失败只在本组显示“获取失败”。点击复制调用 `copyText(key, value)`，成功才显示对应 key 的“已复制”，2 秒后回弹；剪贴板失败保持静默。

项目仓库行是模板内唯一的模板级外链锚点：点击经 document 级外链拦截器（`utils/external-links`）转交系统浏览器打开，WebView 不导航离开应用；锚点带 `target="_blank"` + `rel="noopener noreferrer"`——与 markdown 渲染面的消毒钩子同款 fail-safe（拦截器失效时 wry 拒绝新窗口，点击无动作而非整页导航走）；存储读取/失败态下该行始终可见。

## 测试

`AboutSection.spec.ts` 锁定仓库行（href/slug/内联 SVG/`target="_blank"` + `rel`/无 aria-label、失败态可见，version 经 model 透传渲染）；`useAboutSettings.spec.ts` 覆盖存储读取、复制 key 与失败静默；`SettingsView.behavior.spec.ts` 验证真实 DOM 复制链和回弹。
