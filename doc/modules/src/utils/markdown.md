# markdown（题面/简介渲染与图片地址改写）

> 源文件：`src/utils/markdown.ts`

## 职责

把 HOJ 返回的 Markdown 文本（题目描述、比赛简介）渲染为 HTML，并把其中的相对 URL 改写为指向 OJ 服务端的绝对地址。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `renderMarkdown` | `(markdown: string, baseUrl: string) => string` | marked 同步渲染 → 相对 `src`/`href` 改写；空文本返回空串；渲染抛错时**退回原文按纯文本输出**（题面显示降级好过白屏） |
| `resolveUrl` | `(path, base) => string`（私有） | 已是绝对 URL（`http(s)://`、`data:`、`//`）原样返回，否则 `base + path` |

## 直接依赖

- `marked`（同步 parse）

## 被依赖

- `components/problem/ProblemStatement.vue` — 题面描述/输入/输出三节
- `components/layout/TopBar.vue` — 比赛简介弹层
- `views/LoginView.vue` — 登录页右侧简报

## 逻辑流程

```
renderMarkdown(md, baseUrl):
  md 为空 → ''
  marked.parse(md) → html（抛错 → html = md 原文）
  baseUrl 非空 → 去尾斜杠后正则替换 (src|href)="/…" → "{base}/…"
     例：![x](/api/public/img/a.png) → <img src="https://hoj…/api/public/img/a.png">
```

设计要点：

- **为什么必须改写**：HOJ 题面图片用相对路径（`/api/public/img/…`），不改写会指向前端
  dev server / Tauri 本地源而不是 OJ 服务端，图片全部 404。
- 只改写以 `/` 开头的站内相对路径；`data:`、协议相对 `//` 与绝对 URL 不动。
- `baseUrl` 为空串时跳过改写（config.service 读取失败的兜底），相对路径按原样输出——
  宁可图裂也不阻断题面文本。
- 输出经 `v-html` 渲染，消费方用 scoped `.prose :deep()` 收敛样式；内容源是受信的
  OJ 服务端题面，未做额外 sanitize（v0.x 约定）。
