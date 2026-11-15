# NoticeBar（顶部状态横幅）

> 源文件：`src/components/common/NoticeBar.vue`

## 职责

页面顶部的轻量状态横幅（榜单/评测页共用）：不打断阅读、绝不清空已有数据。tone 决定配色（wa = 刷新失败保留旧数据 / pending = 封榜等信息态），消息内容经默认 slot 自由组合（截断单段或双段强调），图标可经 `#icon` 覆盖默认三角警示（默认图标仅 wa tone 渲染）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `tone` | prop `'wa' \| 'pending'` | 配色族（wa 红系错误 / pending 琥珀系信息） |
| `retry` | prop `() => void?` | 传入即渲染「立即重试」按钮并转发点击（沿用 `ErrorMessage` 的 retry 惯例） |
| default slot | slot | 消息内容 |
| `#icon` | slot | 覆盖默认三角警示图标（如封榜的锁形图标） |

## 直接依赖

- `vue`（`defineProps`）

## 被依赖

- `views/RankView.vue`（封榜提示 + 榜单刷新失败条）
- `views/SubmissionsView.vue`（评测列表刷新失败条）

## 逻辑流程

容器形态（flex/shrink-0/gap/rounded/border）与 tone 配色单点维护；重试按钮的 wa 系描边样式随组件。设置页的两处简化提示（保存失败 div、钥匙门控 p）与数据目录分组的段落式提示无图标无动作，形态不同，有意不使用本组件。

## 测试

`src/components/common/__tests__/NoticeBar.spec.ts` 锁定两种 tone 的容器配色、默认图标的有无、retry 按钮渲染与转发。
