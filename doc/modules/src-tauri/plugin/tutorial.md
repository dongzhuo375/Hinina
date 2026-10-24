# 从零开始开发一个 JS / TS 插件

宿主当前可运行单文件 ES module。本文只提供通用开发脚手架，不包含首次AC或气球业务。

**异步调用契约**：所有返回 Promise 的宿主 API 都必须 await，或作为 activate/事件 handler 返回的 Promise 链的一部分。并发调用使用 await Promise.all。禁止依赖 void apiCall()、未返回的 .then() 或游离异步任务：activate/handler 返回后，引擎不承诺继续执行这些调用或报告拒绝；无事件订阅的插件尤其不会因未完成调用而被后台驱动。本轮不新增 drain，也不在运行时检测/拒绝“忘记 await”。events.on 与注销函数是同步方法，无需 await。

## 1. 准备代码

复制整个 [sdk](sdk/) 到本地开发目录；保留 hinina.d.ts、scaffold/index.ts，将 manifest.example.json 复制为 manifest.json。示例ID为 com.example.development，默认只声明事件和私有存储。

在仓库根目录可用现有 esbuild 验证并构建：

要求Node.js ≥20.6（脚本使用import.meta.resolve）。构建输出为src-tauri/target/plugin-scaffold.js；将其复制到插件目录并命名为index.js，与manifest.entry一致。脚本成功后也会提示此步骤。

```powershell
npm run plugin:sdk:check
npm run plugin:scaffold:build
```

独立工程可安装成熟的 esbuild（例如 npm install --save-dev esbuild），构建为 index.js。不要使用 node/fs/fetch/import运行时依赖；依赖必须离线bundle进入一个ES module。入口固定：

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

把构建产物和Manifest放进代码目录。复制 [plugins.example.json](sdk/plugins.example.json) 为数据目录的plugins.json；只有enabled=true且审批匹配的插件才加载。没有配置时不执行任何插件。也可以在plugins.json顶层设置绝对codeRoot指向开发插件根目录，每个子目录一个插件。

修改清单权限不会自动获得能力；更新宿主批准后reload。网络origin需要同时出现在manifest.network.allowOrigins和宿主network列表里，私网/HTTP另需显式授权，见[网络规范](api/http.md)。插件options配置作为activate第二个参数传入并深冻结；无表单schema实现。

## 3. 启动、调试和重载

运行npm run tauri dev，在桌面WebView开发控制台调用：

```js
await window.hininaPlugins.list()
await window.hininaPlugins.start('com.example.development')
await window.hininaPlugins.resync('com.example.development')
await window.hininaPlugins.reload('com.example.development')
await window.hininaPlugins.stop('com.example.development')
```

enabled插件会自动启动，已运行时再次start会报错。release也按批准配置自动启动；window.hininaPlugins只在dev提供。底层命令为plugin_list/start/stop/reload/resync，不接收代码/权限/路径；前端入口仍经过Store → Service → Bridge。

list显示starting/running/stopped/failed和安全错误代码。详细操作看宿主tracing日志，插件JS错误不输出内部栈。更改源码后用reload，旧worker真正退出后才加载新源码；失败时检查版本、声明/批准、入口、API错误处理。没有JS console或逐步调试器，请通过状态、私有存储和获准通知做有限开发探针，勿输出用户敏感信息。

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

上例是查询用法，实际插件需设置扫描预算、保存可恢复状态并处理context_expired；不能让整个handler超过30秒。空records仍可能有nextCursor。API仅当前比赛、当前登录用户，缺少可见历史时不得声称已经确认全站首次AC。

## 5. 参考业务插件：首次 AC 气球派发

宿主已具备事件、身份/比赛/提交历史、题目列表/全场榜单、持久CAS、受控HTTP、通知、日志及定期Resync。完整的参考实现在 [sdk/examples/balloon](sdk/examples/balloon/README.md)：Resync 节拍（运行时无 `setTimeout`）、封榜门控与不补派、跨页 `teamKey` 去重、配额内翻页（榜单 6 次/分钟，预算耗尽只停翻页不停轮次）、逐成功落盘的 `pending`→`sent` 幂等派发（远端幂等键 `contestId:teamKey:displayId`）、`context_expired` 停轮、`hinina.log.*` 可观测；构建用 `npm run plugin:balloon:build`，验收指南在其 README。

自行实现业务插件时仍需注意：首次AC的业务定义、完整扫描和稳定排序、派发状态恢复、外部幂等键和重试策略都由插件负责。CAS(null)不能把dispatch_failed再变为dispatching，崩溃后的dispatching也会卡住；应区分missing与JSON null，显式设计可恢复状态、租期/重试预算，并让外部服务通过幂等键去重。本地原子CAS不能撤回远端已成功但客户端超时的派发。
