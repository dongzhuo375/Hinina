# AboutInfoRow（About 信息行）

> 源文件：`src/components/settings/AboutSection.vue` 同目录 `AboutInfoRow.vue`

## 职责

About 分组内「dt 标签 + 等宽截断值 + 逐项复制按钮」的信息行。三份同构行（客户端版本/存储目录/日志文件）的形态单点；复制反馈（「已复制」2 秒回弹）不持有状态，经 `copiedKey/copyText` 由 `useAboutSettings` 模型透传。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `label` | prop `string` | dt 标签文本 |
| `value` | prop `string` | dd 值（truncate + font-mono） |
| `copyKey` | prop `string` | 复制反馈去重键（version/baseDir/logPath） |
| `title` | prop `string?` | 悬停完整值；短值（版本号）省略，不渲染 title 属性 |
| `copiedKey` | prop `string \| null` | 当前复制项，命中时按钮文案切换「已复制」 |
| `copyText` | prop `(key, text) => Promise<void>` | 复制动作，透传到模型 |

## 直接依赖

- `vue`（`defineProps`）

## 被依赖

- `components/settings/AboutSection.vue`

## 逻辑流程

纯展示组件：点击按钮以 `(copyKey, value)` 调用 `copyText`；文案由 `copiedKey === copyKey` 判定。行序由调用方模板决定——版本行必须最早出现（`SettingsView.behavior.spec` 取 About 区第一个 button 断言复制反馈）。

## 测试

`src/components/settings/__tests__/AboutInfoRow.spec.ts` 锁定 dt/dd/按钮结构、`(copyKey, value)` 透传、title 有无分支与已复制/复制文案切换。
