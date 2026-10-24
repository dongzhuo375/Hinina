# 从零开始开发一个 JS / TS 插件

> 开发教程。宿主当前可运行单文件 ES module；本文只提供通用开发脚手架，不包含首次 AC 或气球业务（参考实现见 §5）。

**异步调用契约**：所有返回 Promise 的宿主 API 都必须 await，或作为 activate/事件 handler 返回的 Promise 链的一部分；并发调用使用 `await Promise.all`。禁止依赖 `void apiCall()`、未返回的 `.then()` 或游离异步任务——activate/handler 返回后，引擎不承诺继续执行这些调用或报告拒绝；无事件订阅的插件尤其不会因未完成调用而被后台驱动。本轮不新增 drain，也不在运行时检测/拒绝"忘记 await"。`events.on` 与注销函数是同步方法，无需 await。

## 1. 准备代码

- 复制整个 [sdk](sdk/) 到本地开发目录；保留 hinina.d.ts、scaffold/index.ts，将 manifest.example.json 复制为 manifest.json。示例 ID 为 `com.example.development`，默认只声明事件和私有存储
- 要求 Node.js ≥20.6（脚本使用 `import.meta.resolve`）。构建输出为 `src-tauri/target/plugin-scaffold.js`，将其复制到插件目录并命名为 index.js（与 manifest.entry 一致）；脚本成功后也会提示此步骤

```powershell
npm run plugin:sdk:check      # 类型检查
npm run plugin:scaffold:build # esbuild 离线打包
```

- 独立工程可安装成熟的 esbuild（例如 `npm install --save-dev esbuild`），构建为 index.js。**不要使用 node/fs/fetch/import 运行时依赖**；依赖必须离线 bundle 进一个 ES module。入口固定：

```ts
import type { Hinina, JsonValue } from './hinina'
export default async function activate(hinina: Hinina, config: Readonly<JsonValue>) {
  hinina.events.on('resync.required', async (event, envelope) => {
    // 根据当前状态重新同步；不要依赖事件重放。
  })
}
```

## 2. 放入目录并显式批准

设置页查看当前数据目录。默认结构：

```text
<base_dir>/
  plugins.json                       宿主审批，不放在插件目录
  plugins/com.example.development/
    manifest.json
    index.js
  plugin-data/<插件ID的SHA-256>/       宿主私有JSON，停止/重载保留
```

- 把构建产物和 Manifest 放进代码目录；复制 [plugins.example.json](sdk/plugins.example.json) 为数据目录的 plugins.json。只有 `enabled=true` 且审批匹配的插件才加载；没有配置时不执行任何插件。也可在 plugins.json 顶层设置绝对 `codeRoot` 指向开发插件根目录，每个子目录一个插件
- 修改清单权限**不会自动获得能力**；更新宿主批准后 reload。网络 origin 需同时出现在 `manifest.network.allowOrigins` 和宿主 network 列表里，私网/HTTP 另需显式授权（见[网络规范](api/http.md)）
- 插件 `options` 配置作为 activate 第二个参数传入并深冻结；无表单 schema 实现

## 3. 启动、调试和重载

运行 `npm run tauri dev`，在桌面 WebView 开发控制台调用：

```js
await window.hininaPlugins.list()
await window.hininaPlugins.start('com.example.development')
await window.hininaPlugins.resync('com.example.development')
await window.hininaPlugins.reload('com.example.development')
await window.hininaPlugins.stop('com.example.development')
```

- enabled 插件会自动启动；已运行时再次 start 会报错。release 也按批准配置自动启动；`window.hininaPlugins` 只在 dev 提供
- 底层命令为 plugin_list/start/stop/reload/resync，不接收代码/权限/路径；前端入口仍经过 Store → Service → Bridge
- list 显示 starting/running/stopped/failed 和安全错误代码。详细操作看宿主 tracing 日志，插件 JS 错误不输出内部栈。更改源码后用 reload，旧 worker 真正退出后才加载新源码；失败时检查版本、声明/批准、入口、API 错误处理
- 没有 JS console 或逐步调试器——通过状态、私有存储和获准通知做有限开发探针，勿输出用户敏感信息

## 4. 查询当前比赛的可见历史

```ts
const context = await hinina.context.current()
const contest = await hinina.contest.current(context)
if (contest) {
  let cursor: string | null = null
  do {
    const page = await hinina.submission.list(context, {
      contestId: contest.contestId, cursor, limit: 100,
    })
    // 用submissionId去重；page.total可能为null。
    cursor = page.nextCursor
  } while (cursor !== null)
}
```

- 上例只是查询用法：实际插件需设置扫描预算、保存可恢复状态并处理 `context_expired`，不能让整个 handler 超过 30 秒；空 records 仍可能有 nextCursor
- API 仅当前比赛、当前登录用户；缺少可见历史时不得声称已经确认全站首次 AC

## 5. 参考业务插件：首次 AC 气球派发

宿主已具备事件、身份/比赛/提交历史、题目列表/全场榜单、持久 CAS、受控 HTTP、通知、日志及定期 Resync。完整参考实现在 [sdk/examples/balloon](sdk/examples/balloon/README.md)：Resync 节拍（运行时无 `setTimeout`）、封榜门控与不补派、跨页 `teamKey` 去重、配额内翻页（榜单 6 次/分钟，预算耗尽只停翻页不停轮次）、逐成功落盘的 `pending`→`sent` 幂等派发（远端幂等键 `contestId:teamKey:displayId`）、`context_expired` 停轮、`hinina.log.*` 可观测；构建用 `npm run plugin:balloon:build`，验收指南在其 README。

自行实现业务插件时仍需注意：

- 首次 AC 的业务定义、完整扫描和稳定排序、派发状态恢复、外部幂等键和重试策略都由**插件**负责
- `CAS(null)` 不能把 dispatch_failed 再变为 dispatching，崩溃后的 dispatching 也会卡住——应区分 missing 与 JSON null，显式设计可恢复状态、租期/重试预算，并让外部服务通过幂等键去重
- 本地原子 CAS 不能撤回远端已成功但客户端超时的派发
