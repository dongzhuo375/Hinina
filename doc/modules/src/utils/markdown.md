# markdown（题面/简介/公告渲染、公式、消毒与图片地址改写）

> 源文件：`src/utils/markdown.ts`

## 职责

把 HOJ 返回的 Markdown + LaTeX 混合文本（题目描述、比赛简介、公告正文）渲染为 HTML：`$...$` / `$$...$$` 公式经 KaTeX 渲染，在唯一出口做 DOMPurify 消毒（保住公式结构），并把其中的相对 URL 改写为指向 OJ 服务端的绝对地址。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `renderMarkdown` | `(markdown: string, baseUrl: string) => string` | marked 同步渲染（含 KaTeX 公式）→ **DOMPurify 出口消毒** → 相对 `src`/`href` 改写；空文本返回空串；渲染抛错时**退回原文按纯文本输出**（题面显示降级好过白屏） |
| `resolveUrl` | `(path, base) => string`（私有） | 已是绝对 URL（`http(s)://`、`data:`、`//`）原样返回，否则 `base + path` |

## 直接依赖

- `marked`（同步 parse）
- `marked-katex-extension` + `katex`（tokenizer 层接管 `$/$$`，模块加载时 `marked.use()` 一次性装配）
- `dompurify`（出口消毒）

## 被依赖

- `components/problem/ProblemStatement.vue` — 题面描述/输入/输出三节
- `components/layout/TopBar.vue` — 比赛简介弹层
- `views/LoginView.vue` — 登录页右侧简报
- `views/AnnouncementsView.vue` — 公告正文

## 逻辑流程

```
模块加载：marked.use(markedKatex({ nonStandard, throwOnError:false, strict:'ignore' }))

renderMarkdown(md, baseUrl):
  md 为空 → ''
  marked.parse(md) → html（抛错 → html = md 原文）
  DOMPurify.isSupported →
    sanitize(html, {
      USE_PROFILES: { html, mathMl, svg },   // KaTeX 输出 MathML + SVG，只开 html 档会把公式剥成乱码
      ADD_TAGS: [semantics, annotation],     // KaTeX 无障碍树（LaTeX 源码）不在预置白名单，不补会被剥掉
      ADD_ATTR: [encoding],
    })
  baseUrl 非空 → 去尾斜杠后正则替换 (src|href)="/…" → "{base}/…"
     例：![x](/api/public/img/a.png) → <img src="https://hoj…/api/public/img/a.png">
```

设计要点：

- **为什么必须在 tokenizer 层接管公式**：LaTeX 里的 `_`、`*`、`\` 会被 markdown 先行吃掉
  （`$x_1$` → `$x<em>1$`），「先渲染后找 $」无从还原。
- **`nonStandard: true`（中文题面关键开关）**：标准规则要求 `$` 前空格/行首，中文行文无空格
  （`保证$1 \le n \le 10^5$成立`）会整段失配。已知取舍：成对货币 `$`（`价格 $5 和 $10`）会被
  误判为公式——ACM 题面极少出现货币，取中文数学式优先（有测试锁定）。
- **容错**：`throwOnError: false` 让写错的 LaTeX 降级为红色字面文本而非白屏；
  `strict: 'ignore'` 不再刷 console 警告；`trust` 保持 false 禁用 `\href` 等注入宏。
- **消毒档位必须含 `mathMl` 与 `svg`**：KaTeX 视觉树用 SVG 画可伸缩根号/括号、
  `<span class="katex-mathml">` 里的 MathML 供读屏器；只开 `html` 档公式会碎成乱码。
  `<semantics>`/`<annotation>`（annotation 内为 LaTeX 源码）不在 DOMPurify 预置白名单，
  经 `ADD_TAGS` 补入——mXSS 载体 `<annotation-xml>` 仍被拒。
- **出口统一消毒（P49/P63 关闭）**：marked 默认放行内联 HTML，而题面/简介/公告全部来自
  OJ 服务端且编辑者面较宽（兼容任意 HOJ/QDUOJ/HUSTOJ 部署，不宜按"服务端完全可信"假设）；
  在唯一出口消毒使所有 `v-html` 调用点自动受保护，未来新增调用点也不易遗漏。
- **Node 测试环境降级**：DOMPurify 依赖 DOM，无 jsdom 时 `isSupported === false`，跳过消毒
  仅做渲染；生产环境恒为 WebView，消毒必然生效。消毒激活态的行为由
  `__tests__/markdown-dom.spec.ts`（jsdom 环境）独立锁定。
- **为什么必须改写**：HOJ 题面图片用相对路径（`/api/public/img/…`），不改写会指向前端
  dev server / Tauri 本地源而不是 OJ 服务端，图片全部 404。
- 只改写以 `/` 开头的站内相对路径；`data:`、协议相对 `//` 与绝对 URL 不动。
- `baseUrl` 为空串时跳过改写（config.service 读取失败的兜底），相对路径按原样输出——
  宁可图裂也不阻断题面文本。
- 输出经 `v-html` 渲染，消费方用 scoped `.prose :deep()` 收敛样式；块级公式横向滚动由
  `styles/global.css` 的 `.katex-display` 全局样式提供（题面/公告/简介共用）。
