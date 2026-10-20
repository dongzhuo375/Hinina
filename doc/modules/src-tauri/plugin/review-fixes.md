# 插件审查：23项核实与修复

逐项基于 aa293fa 核对。严重性按实际影响评估；优化建议不等同于已发生的数据泄漏。测试均使用本地夹具或可注入端口，没有访问真实 OJ 账户。

| 项 | 判定与严重性 | 修复/验证 |
|---|---|---|
| 1 Hydro展示名导致历史失败 | 存在，High：合法用户整页无法查询 | 保留宿主私有owner_id，比较HOJ/Hydro稳定uid；同一Hydro fixture覆盖不同displayName和缺失udict，list/get均成功，错误owner仍拒绝 |
| 2 转换中current_context错误码 | 存在，Medium：正常切换可使插件错误退出 | 与check统一context_expired；测试切换前、中、后及旧句柄撤销 |
| 3 激活前订阅溢出 | 存在，Medium：可用性/启动竞态 | 成功activate后订阅，初始Resync补偿；阻塞激活时派发256条事件仍可正常启动 |
| 4 新OJ与旧比赛窗口 | 存在，Medium：跨作用域授权风险 | 命令外层guard和switch_scope覆盖Registry与Service清理；并发读测试在混合中间态无法取得访问快照 |
| 5 订阅锁内日志 | 存在，Medium：慢writer阻塞所有订阅 | 锁内只收集诊断，锁外写日志；测试writer在订阅/溢出日志中可重新取得订阅锁 |
| 6 无关非法approval阻塞全部插件 | 存在，Medium：故障隔离不足 | 逐项解析/校验并在invalid_plugins报告；list逐项失败，load只拒绝目标；覆盖非法间隔和未知权限 |
| 7 Manager依赖具体I/O | 存在，Medium（维护/可测性） | 注入 `Arc<dyn PluginRepository>` 和PluginApiFactory，AppContext选择Fs实现；内存仓库/假工厂测试无需文件 |
| 8 存储无限操作 | 存在，Medium：批准插件可消耗磁盘CPU | 每激活60次/分钟、工厂跨重载合计240次/分钟，初始突发相同；16在途，超限在I/O前拒绝，取消不会提前释放阻塞事务许可 |
| 9 退出/停止日志不足 | 存在，Low | failed退出warn带稳定error，正常info，stop超时warn；日志不持状态锁 |
| 10 start超时仍有活worker | 存在，Medium：生命周期/诊断不一致 | 超时先锁定failed/timeout，取消并等待回收，迟到退出不覆盖timeout；回归覆盖超时后重启及延迟回收。原生阻塞无法强杀，停止期限内无法清理则保留failed并由reaper继续跟踪，旧worker结束前拒绝同ID重启 |
| 11 结构/模块文档缺失 | 存在，Low | 补core/infra/运行时/测试/前端文件树、模块索引、前端5个模块文档及quickjs.md |
| 12 cursor并发复用 | 存在，Medium：重复记录及重复请求 | 在验证锁内原子领取，只有一个调用成功；文档规定失败也消费，需重新查询 |
| 13 check重复读会话文件 | 存在，Low（性能） | 生产ServiceQueries按revision缓存一个scope，所有显式访问变化撤销；测试多次身份读取只load一次且登出立即失效 |
| 14 每请求新Client | 事实存在，Low（优化）；原实现同时承担DNS固定 | 仅对同origin和本次重新验证的相同地址集合复用Client，最多32项；保留no_proxy/禁redirect/禁retry/实际remote_addr检查，不做跨地址盲目复用 |
| 15 guard缺must_use | 存在，Low（防误用） | AccessTransition添加must_use，文档注明持有范围 |
| 16 外部字符串写日志 | 存在，Low（日志加固） | 插件ID/operation按Debug转义，operation日志截断64字符；manifest授权ID仍严格校验 |
| 17 duplicate start污染历史 | 存在，Low | 生命周期锁内提前拒绝，不写failed；测试重复start后停止状态无错误 |
| 18 引擎错误压为unavailable | 部分存在，Low：语法/运行异常混同；超时和撤销原本已有区分 | 增加稳定script_error并更新SDK；语法/throw回归保留安全错误、不暴露栈 |
| 19 IPC压为Config | 存在，Medium（错误契约） | 插件命令直接返回PluginError，IpcError.code/raw保留code和permission，前端测试不触发认证守卫 |
| 20 两个慢测试 | 存在，Low（测试效率） | 周期测试注入短间隔，Provider超时采用Tokio虚拟时间，生产限额保持不变 |
| 21 HTTP超时窗口紧 | 存在，Low（测试稳定性） | 改为Tokio虚拟时间，无20/40ms真实时钟竞争 |
| 22 高价值回归缺失 | 存在，Medium（验证缺口） | 补Hydro所有者、真实双插件预算、公开request成功路径（fake transport）、start超时、慢激活溢出、cursor并发；实际reqwest仍用本地server验证 |
| 23 validation数量过期 | 存在，与11重复，Low | 改为不固定测试数量，结果以本轮命令输出为准；同步新增回归范围 |

HTTP公开成功路径在真实批准/地址/限流/超时包装下使用内存HttpTransport响应；真实连接、响应体和redirect策略使用本地私有transport夹具。没有为了测试公开任何loopback豁免或IPC策略开关。

验证命令：`cargo test --offline --manifest-path src-tauri/Cargo.toml`、`npm test`、`npm run build`、`npm run plugin:sdk:check`，以及本轮Rust文件的rustfmt检查。最终通过情况以当轮执行输出及交付说明为准。
