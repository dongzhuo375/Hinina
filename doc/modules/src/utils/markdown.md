# markdown（题面/简介/公告渲染、公式、容器、消毒与图片地址改写）

> 源文件：`src/utils/markdown.ts`

## 职责

把 HOJ 返回的 Markdown + LaTeX 混合文本（题目描述、比赛简介、公告正文）渲染为 HTML：`$...$` / `$$...$$` 公式经 KaTeX 渲染，Vditor `:::` 排版容器（居中/居右）在块级接管，在唯一出口做 DOMPurify 消毒（保住公式结构；**只为安全剥**，`<big>`/`<font size>`/`<center>` 等表现性标记按作者原意保真渲染——曾一度剥离后回退，与 HOJ 网页端等价），并把相对 URL 改写为指向 OJ 服务端的绝对地址。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `renderMarkdown` | `(markdown: string, baseUrl: string) => string` | marked 同步渲染（含 KaTeX 公式）→ **DOMPurify 出口消毒**（钩子内完成相对 `src`/`href` 改写与外链加固）；空文本返回空串；渲染抛错时**退回原文按纯文本输出**（题面显示降级好过白屏） |
| `createPurifier()`（模块私有） | 创建本模块专用的 `DOMPurify(window)` 实例并注册 `afterSanitizeAttributes` 钩子；无 DOM（Node 测试）返回 null | 钩子做三件事：①**空 href 占位化**——`href` 存在但 `trim()` 为空（`[x]()` / `<a href=" ">`）时移除 href：空 href 解析后等于当前文档 URL，点击会整页重载（丢防抖内的编辑器输入/草稿），无目标就不该可点；②**相对 URL 改写**——只遍历真实元素的 `src`/`href` 属性（`/` 开头 → `{baseUrl}` + 路径），文本节点/代码块里的同形字面量天然不受影响；改写基址先经 `validRewriteBase` 校验（只接受 http/https 或 `//host`，防 `javascript:` 在消毒后注入）；③**外链 fail-safe**：仅对 http/https 且 host ≠ 应用 host 的 `<a>` 注入 `target="_blank"` + `rel="noopener noreferrer"`（同 host/`mailto:`/`tel:`/`#` 不加，否则变死链接）。钩子只挂局部实例，**不污染全局 `DOMPurify` 单例** |
| `validRewriteBase`（私有） | `(baseUrl: string) => string` | 改写基址校验：空/畸形/非 http(s) scheme 一律返回空串（不改写）——改写发生在消毒之后，必须堵住危险 scheme |
| `resolveUrl` | `(path, base) => string`（私有） | 已是绝对 URL（`http(s)://`、`data:`、`//`）原样返回，否则 `base + path` |

## 直接依赖

- `marked`（同步 parse）
- `marked-katex-extension` + `katex`（tokenizer 层接管 `$/$$`，模块加载时 `marked.use()` 一次性装配）
- `dompurify`（出口消毒；局部实例）
- `@/utils/external-links`（`resolveExternalUrl`：外链判据与拦截器共用，防漂移）

## 被依赖

- `components/problem/ProblemStatement.vue` — 题面描述/输入/输出三节
- `components/layout/TopBar.vue` — 比赛简介弹层
- `views/LoginView.vue` — 登录页右侧简报
- `views/AnnouncementsView.vue` — 公告正文

## 逻辑流程

```
模块加载：marked.use(markedKatex({ nonStandard, throwOnError:false, strict:'ignore' }))
         marked.use(vditorContainer)   // ::: 容器块级扩展
         createPurifier()              // 局部 DOMPurify(window) 实例 + 属性级钩子
                                       // （相对 URL 改写 + 外链加固，不污染全局）

renderMarkdown(md, baseUrl):
  md 为空 → ''
  marked.parse(md) → html（抛错 → html = md 原文）
  purifier 非空（有 DOM）→ rewriteBase = validRewriteBase(baseUrl)（http/https 或 //host，否则空）
    sanitize(html, {
      USE_PROFILES: { html, mathMl, svg },   // KaTeX 输出 MathML + SVG，只开 html 档会把公式剥成乱码
      ADD_TAGS: [semantics, annotation],     // KaTeX 无障碍树（LaTeX 源码）不在预置白名单，不补会被剥掉
      ADD_ATTR: [encoding],
      FORBID_TAGS: [style, form],            // 显式禁：html 档白名单实际含二者；<style> mid-body 可全文档改写+CSS 外泄
    })                                       //   （leading 被剥只是 head 位置伪象）；<form> 外域 action 保留且导航两层无覆盖，
                                             //   提交残余=整页重载丢草稿。二者均剥标签保/弃内容见源码注释；其余剥离只为安全，
                                             //   表现性标记（big/font 等）保真渲染
    // 钩子（每元素，afterSanitizeAttributes）：
    //   0) <a> 且 href 存在但 trim() 为空 → 移除 href（占位链接，防点击整页重载）
    //   1) src/href 以 / 开头 → 改写为 {baseUrl} + 路径（属性级，文本/代码块不受影响）
    //   2) <a> 且解析后 http(s) 且 host ≠ 应用 host → target=_blank + rel=noopener noreferrer
    finally rewriteBase = ''
  // 无 DOM（Node 测试）→ 不消毒也不改写；生产恒为 WebView
```

设计要点：

- **为什么必须在 tokenizer 层接管公式**：LaTeX 里的 `_`、`*`、`\` 会被 markdown 先行吃掉
  （`$x_1$` → `$x<em>1$`），「先渲染后找 $」无从还原。
- **Vditor `:::` 容器（块级扩展）**：HOJ 管理端编辑器"居中/居右"按钮产出
  `::: hljs-center … :::`。`start` 钩子让容器能打断紧贴的正文段落（无空行分隔）；
  内部内容递归走完整块级管线（公式/列表照常解析）；类名限定 `[\w-]+` 杜绝 class
  属性注入；未闭合容器按字面降级为文本（手写错误不丢内容）。对齐语义由
  global.css 的 `.hljs-center/right/left` 提供。
- **剥离原则：只为安全剥**：脚本/事件属性由 DOMPurify 白名单负责；表现性标记
  （`<big>`/`<font size>`/`<center>`，CF 导入题源惯例）按作者原意保真渲染，与
  HOJ 网页端一致——浏览器 `font-size: larger` 的放大比例（约 1.2×）在本客户端
  与 HOJ 网页端等价。曾一度剥离 `<big>` 保客户端排印比例，后按"内容标记是数据、
  应保真渲染"的原则回退（jsdom 契约测试锁定保真行为）。
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
