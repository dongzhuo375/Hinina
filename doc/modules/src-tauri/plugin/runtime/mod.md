# Plugin Runtime / PluginManager

所有 Promise 返回的宿主 API 必须 await，或加入 activate/handler 返回的 Promise 链；并发用 await Promise.all。入口结束后不 drain 游离调用，不保证它们执行或报告拒绝；events.on/注销函数同步执行。停止超时保持 failed/timeout，继续跟踪旧 worker，完成后由 reaper 回收并保存历史。

选择 rquickjs 0.14（QuickJS-NG），仅 std/futures features。不启用 dyn-load/full/custom allocator。与 boa/deno_core 相比，本轮选型重视已有 Promise 桥、引擎中断/内存限制和较小依赖；Windows/MSVC 真实引擎测试通过后才装配。未进行三种引擎的体积性能基准。

入口是一个离线打包 ES module：`export default async function activate(hinina, config)`。源码<=1MiB，配置JSON<=64KiB且深冻结；宿主无 resolver/loader，不支持任意 import、原生模块、fs/process/env/fetch/socket/shell。TS 在宿主外通过 esbuild 等构建，宿主不执行 TS 源文件。

每插件独立引擎和OS worker线程，由持续存在的 Tauri Tokio handle 驱动；没有第二个 Tokio runtime。内存上限16MiB、栈256KiB、activate 10秒、一次事件交付总计30秒。同步JS由引擎 interrupt 中断；未完成Promise由取消和时间上界终止。异常/超时/内存失败停止该插件，其他插件和核心继续。

同一插件最多64个 handlers，按注册顺序串行 await；on 返回注销函数。一次事件的30秒预算包含全部 handlers。handler未捕获异常会使插件失败，需局部恢复的逻辑由插件自行 catch。所有API通过有限JSON桥，最多16个在途bridge调用；错误是安全代码，无内部栈。开发调试通过状态和宿主日志进行，没有DevTools逐步调试器或source map栈输出。

状态仅 starting/running/stopped/failed。没有安装器、暂停/恢复、热更新或复杂生命周期状态机。start/reload 重新读取宿主配置并校验文件，单插件启动失败不阻止应用启动。reload 等待旧worker真正退出再创建新实例。

停止顺序：撤销本次授权/取消调用 → 注销Host订阅 → 放弃缓存事件/等待Promise → 清理handlers并释放引擎。正常stop最多等5秒，未退出则报timeout并阻止并存重载。应用退出只做同步撤销，不等待插件，不干预工作区保存关窗流程。文件系统内核调用不能强制回滚，已发送HTTP不能撤销。

周期Resync默认60秒，可配置10..3600秒；每次成功代表提示实际入队，已有业务事件不会令提示被静默跳过。队列容量128，满/关闭时注销并令 worker 失败退出。首次/Lagged/账号/OJ/比赛事件也触发Resync。它只是重新查询提示，不代表补发，更不负责气球重试。安全性由显式AccessRevision保证，不依赖这些通知。

activate成功后才建立事件队列，以初始Resync补偿激活窗口变化。语法/运行异常为script_error，基础设施为unavailable；超时/撤销保留timeout/context_expired，详见[QuickJS](quickjs.md)。

`PluginRuntime` 是生产路径实际使用的端口：Manager 以 `Box<dyn PluginRuntime>` 持有引擎，只调用 activate/deliver_event/shutdown，测试用的 !Send 实现与 QuickJS 实现共用同一契约。事件信封由 PluginHost 订阅产生，不存在额外的 Runtime Bridge 装配类型。
