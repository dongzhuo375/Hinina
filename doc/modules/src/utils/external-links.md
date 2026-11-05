# external-links（外链拦截：WebView 永不导航离开应用）

> 源文件：`src/utils/external-links.ts`

## 职责

document 级 click 委托：把 markdown 渲染面（题面/比赛简介/登录页简报/公告——覆盖全部 `renderMarkdown` 消费方，清单见 `markdown.md`）里的 http(s) 锚点转交系统默认浏览器。WebView 内点这类链接的默认行为是把整个客户端导航走（分层审计 2026-10「外链无出路」缺口）——本模块 + Rust `open_external`（`app_cmd.md`）共同闭合它：拦截在前端、scheme 白名单最后一道在 Rust 命令、转交经 `tauri-plugin-opener` 的 Rust API（`Cargo.toml`）；插件默认注入的点击脚本已在注册时关闭（`main.rs`），本命令是唯一转交通道。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `installExternalLinkOpener` | `(open: (url: string) => void, doc?: Document) => () => void` | 装配 document 级 `click` + `auxclick` 委托，返回解绑函数（应用生命周期内不解除）。拦截规则：**左键（含 Ctrl/Meta/Shift/Alt 修饰）与中键都拦**——WebView 没有新标签语义，修饰键/中键的默认策略就是新窗口请求被拒绝（点了没反应），所以一并转交；右键不拦（上下文菜单）；`closest('a')`；`href` 经 `new URL(href, location.href)` 解析后为 **http/https 且 host ≠ 当前 host** → `preventDefault()` + `open(绝对 URL)`。`open` 由装配方注入（`main.ts` → `systemService.openExternal`，失败仅记日志）——注入即测试点。应用内 RouterLink（自带 preventDefault）、`#` 片段、同 host 锚点由同一条判据（解析后同 host / 非 http(s)）自然放行，无重复规则 |
| `resolveExternalHref`（私有） | `(anchor, doc) => URL \| null` | 锚点 → 解析后的外链 URL；非外链（无 href/非 http(s)/同 host/解析失败）返回 null |

## 直接依赖

- 无运行时依赖（浏览器 `URL`/`document` API）
- `open` 回调由装配方注入——本模块不 import service/bridge

## 被依赖

- `src/main.ts`（组合根装配，唯一生产消费方；`main.spec.ts` 锁定装配计数）
- `src/utils/__tests__/external-links.spec.ts`

## 逻辑流程

`click`（主键）/ `auxclick`（中键）→ document 冒泡 → `defaultPrevented` → 放行 → `closest('a')` 无 → 放行 → `resolveExternalHref` 为 null（应用内/非 http(s)/`#` 片段）→ 放行 → 否则 `preventDefault()` + `open(绝对 URL)`。右键（`auxclick` button 2）不进入本流程。

## 测试

`src/utils/__tests__/external-links.spec.ts`（10 用例，jsdom）：http(s) 外链转交（含原始 query；**断言 `defaultPrevented === true`**——防「转交了但没拦默认导航」的假绿）、协议相对 `//host` 解析、`closest` 内层元素委托、同 host/`#` 片段/非 http(s) 不转交（断言 `defaultPrevented === false`，放行默认动作）、**修饰键（左键）同样转交**、**中键转交 / 右键放行**、`defaultPrevented` 不重复转交、非锚点不转交、detach 后 click 与 auxclick 均失效。
