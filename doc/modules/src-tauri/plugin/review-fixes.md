# 插件审查：逐轮核实与修复

首轮下表23项基于aa293fa核对；第二轮加固交付于6af3202；PR #41 MEDIUM跟进基于6af3202，交付于cac8106；LOW收尾基于cac8106交付于bc75b18；查询可用性与桥契约跟进基于bc75b18交付于14527dc；终轮评审修复基于14527dc交付于f667a73；复审LOW跟进基于f667a73。各节记录对应轮次的处理，后续节更新此前结论。严重性按实际影响评估；优化建议不等同于已发生的数据泄漏。测试均使用本地夹具或可注入端口，没有访问真实 OJ 账户。

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

## 复审后续加固（第二轮）

上一轮结论后的 7 项小观察一并处理：

| 项 | 处理 |
|---|---|
| HOJ 未返回 `uid` 时 owner 缺失 | 保持 fail closed，不退回展示名；新增夹具用例锁定 `owner_id == None`、插件侧整页 `provider_error`，并在 HOJ 夹具测试中断言 `owner_id == uid` |
| `switch_oj` 外层 guard 过宽 | 移除命令外层 guard，栅栏只覆盖 `switch_scope`（Registry 发布 + 缓存清理）；持久化与事件发布在 guard 释放后执行 |
| `parseAppError` 把任意 `{code}` 当插件错误 | 收紧为恰好 `{code, permission?}`；其他对象仍按通用形态解析，新增前端用例锁定真实 message 不被 code 覆盖 |
| `scope()` 持锁做文件 I/O | 缓存命中只在锁内克隆；会话/配置读取移到锁外，未命中并发计算是幂等的 |
| `owner_id` 序列化丢失 | 保持 `skip_serializing`（不跨 IPC 暴露归属），在实体、`SubmissionHistoryPage`、插件 API 文档中写明“只能消费 Provider 直接返回的记录”，并加序列化契约测试 |
| reaper 100ms 轮询 | 改为 100ms 起指数退避、2s 上限，原生调用永不返回也不会形成高频轮询 |
| 锁检查测试的 tracing 顺序敏感 | 用 `Dispatch::new` 先注册 dispatcher 再 `with_default`，消除 callsite interest 竞态（与 Manager 诊断测试同款做法） |

## PR #41 MEDIUM 跟进（基线 6af3202）

该轮按确认范围处理 M-1 至 M-8，只纳入直接相关 LOW。未 await 的调用采用文档强制 await 契约，当时未增加drain、拒绝跟踪器或后台调度；后续LOW轮追加有界拒绝诊断，仍不驱动后台调度。

| 项 | 核实与处理 |
|---|---|
| M-1 stop超时残留 | 确认；超时设failed/timeout、启动reaper，完成后保存历史；原生线程结束前继续阻止同ID启动，不能提前移除失控线程 |
| M-2 插件infra缺诊断 | 确认；五模块经plugin_diagnostics输出warn，仅阶段、类别和安全ID；IO/JSON/HTTP源错误正文不落日志，新增脱敏回归 |
| M-3 私有默认栅栏 | 确认属未来装配隐患，当前生产绑定原本正确；四个组件new强制传access，删除with_access并迁移组合根及全部测试调用点 |
| M-4 fire-and-forget | 确认未保证入口结束后继续驱动；SDK、教程、API与Runtime文档明确所有Promise调用必须await/返回Promise链，events.on等同步方法除外 |
| M-5 IP origin | 确认属纵深防御缺口，不等同现有SSRF绕过；清单与approval拒绝IPv4/IPv6字面量（含公网IP），补带scheme负例；域名夹具通过私有Resolver端口避免真实DNS |
| M-6 Store策略越层 | 确认；通知验证、五项上限、六秒过期和手动/卸载清理归Service，Store只应用状态事件 |
| M-7 错误形态契约 | 当前互配正确；Rust穷尽测试锁定code/permission字段与字符串类型，前端补permission非法类型负例 |
| M-8 文件树缺漏 | 确认；补scripts、插件infra/API测试和host/runtime/permission测试目录；同步新增诊断模块和前端Service测试 |

域名校验不替代运行时DNS/IP限制；正常未配置插件和初次未创建存储不记录故障。未await仍是不受支持用法，不声称运行时已自动执行或拒绝。

## PR #41 LOW收尾（基线cac8106）

补齐报告剩余LOW以及验证新发现3项；仅建议/无现患的条目明确记录取舍。

| 项 | 核实与处理 |
|---|---|
| atomic_write丢失IO粒度 | 存在；分别记录atomic_replace/write_callback阶段与ErrorKind，直接测试两支，禁止源错误正文 |
| invalid ID无直接回归 | 验证缺口；覆盖空值、控制字符、超长ID和host占位，确认秘密不落日志 |
| 重复stop生成多个reaper | 存在但安全收敛；worker锁内标记reaping，每代只启动一个任务，重复stop后仍最终回收并保存timeout |
| F3分配错误混入unavailable | 存在；QuickJS Allocation映射limit_exceeded，脚本Exception保留script_error |
| F4游离Promise拒绝静默 | 存在；仅跟踪引擎已观察到的拒绝，最多256个身份摘要，执行窗口结束脱敏汇总；同窗口补handler移除。不增加drain，未await仍不受支持 |
| F5桥响应无大小上限 | 存在；DTO与最终JSON包络均流式计数，含包络1MiB，超限为limit_exceeded且审计失败 |
| F6期限散落 | 存在；limits集中激活/事件/停止期限，启动期限由激活加2秒宽限推导 |
| Manager/query L1–L5 | 注明ready边界fail closed；allowlist归PluginGrant共用；identity序列化unwrap改expect；native投影页1MiB/所有保留余项4MiB；CAS审计标记读写两权限 |
| T1停止超时验证 | 前轮已覆盖失败状态/同ID禁止重启/退出历史；本轮增加重复stop单reaper断言 |
| Infra L1/L2 | remote_addr注明发送后纵深核查；并发许可先于速率令牌且移入有界future；测试并发拒绝不扣令牌及丢弃/撤销后释放 |
| Infra L3数值回绕 | 存在理论溢出；wrapping_add与取消令牌共同保护，scope缓存也验证缓存令牌未撤销；测试临界回绕和数值相同的撤销缓存 |
| Infra L4配置栅栏位置 | 原实现安全：写锁阻止新配置可见，释放前已撤销；补注释解释锁与缓存读的先后，无需改变无关编辑器配置不撤销的语义 |
| Infra L5实体排列 | SubmissionHistoryPage移至测试模块之前，宿主私有owner契约保留 |
| 前端L1/L2 | permission类型已于前轮覆盖；错误message映射中文，code/raw保留，未知code及原型键走兜底 |
| 前端L3/L4 | 构建脚本检查Node≥20.6并提示产物复制为index.js；教程同步；commands模块恢复字母序 |
| 文档LOW | 此文逐节标记基线；插件架构审计 §1–8标记历史/被§9–10取代；Architecture记录插件Infra授权依赖例外及新模块 |
| 并发验证缺口 | 生产scope旧IO阻塞、新revision先完成、旧结果拒绝；真实ContestService与Registry经switch_scope组合切换的并发断言 |

本轮回归均为离线夹具；HTTP本地成功传输的真实时钟窗口放宽为5秒，总截止测试仍用虚拟时间。资源字节预算限制JSON表示，不声称等同宿主RSS硬上限。

HTTP公开成功路径在真实批准/地址/限流/超时包装下使用内存HttpTransport响应；真实连接、响应体和redirect策略使用本地私有transport夹具。没有为了测试公开任何loopback豁免或IPC策略开关。

验证命令：`cargo test --offline --manifest-path src-tauri/Cargo.toml`、`npm test`、`npm run build`、`npm run plugin:sdk:check`，以及本轮Rust文件的rustfmt检查。最终通过情况以当轮执行输出及交付说明为准。

## 终轮评审修复（基线14527dc）

| 发现 | 确认与处理 |
|---|---|
| Persistent 晚于 runtime 析构 | High成立；worker panic 展开会绕过 `shutdown`，rquickjs 明确会在句柄晚于 runtime 析构时 abort 进程。`QuickJsRuntime` 增加 `Drop` 先释放 deliver，并新增“加载后不 shutdown 直接 drop”回归测试 |
| 游标先发布后编码 | Medium成立；整页（含 nextCursor 与桥包络）在插入游标前按 `MAX_CURSOR_RESPONSE_BYTES` 校验，超限不占用游标配额；新增边界回归确认 cursors 为空 |
| 本地桥调用无上限 | Medium成立；`PluginApi::admit` 统一准入，ExecutionApi 按激活 600 次/分钟限流，覆盖 context/identity/storage 等不经 Provider 预算的能力；新增准入回归 |
| scope 告警可刷屏且无归属 | Medium成立；按 AccessRevision 去重并补充 plugin_id/oj_id，禁用 OJ 的持久状态不再每次调用输出 |
| start 替换 worker 丢历史 | Low成立；`start_worker` 在移除已结束 worker 前写入最终状态，避免与 reaper 竞争 |
| panic payload 无诊断 | Low成立；记录 `worker.panic` 与 panic_kind（str/string/other），不格式化可能受插件控制的 payload |
| Runtime 死脚手架 | Low成立；删除 `PluginRuntimeBridge`，Manager 改为 `Box<dyn PluginRuntime>`，生产与测试共用同一端口 |
| 文档漂移 | Low成立；修正 api/mod.md 的 endpoint 语义与 Architecture 能力表 |
| 根目录审计快照 | Low成立；移入 doc/plugin-architecture-review.md 并更新全部引用 |

验证：Rust 全量 704 passed（plugin 99），clippy 仅保留 6 项既有告警；前端 504 passed，`npm run build`、`npm run plugin:sdk:check`、`npm run plugin:scaffold:build` 通过；改动 Rust 文件 rustfmt 通过。

## 并发订阅与 OJ 失败诊断（基线f667a73）

| 发现 | 确认与处理 |
|---|---|
| 重叠 subscribe 泄漏监听器 | P2成立；旧实现先清理再 await，两个并发调用都会通过清理并在完成后互相覆盖。改为单调代次：迟到结果立即卸载，不写入活动订阅，也不接管 dismiss；返回的卸载函数只在自身仍是活动订阅时生效并推进代次。新增“乱序完成”和“卸载旧订阅不影响新订阅”两条回归 |
| OJ 失败被压缩后无日志 | P2成立；contest.get/submission.history 直接 `map_err(|_| ProviderError)`，scope 的身份读取同样静默，release 默认级别下可能完全没有失败记录。新增 `log_provider_failure`：记录稳定操作名、OJ 标识与 AppError 变体类别，绝不输出原始消息；新增回归断言类别存在且 ProviderNotFound 文案不落日志 |

验证：Rust 全量 705 passed，clippy 仍为既有 6 项告警，改动文件 rustfmt 通过；前端 507 passed。

## 事件隔离、游标重试与目录扫描（基线ab061e8）

| 发现 | 确认与处理 |
|---|---|
| 单个 handler 异常终止整个 worker | 存在；bootstrap 直接 await，异常使 deliver 拒绝，Manager 随即跳出事件循环并标记 failed。改为逐 handler try/catch 并返回 `{failed, code}` 摘要；宿主记录一次脱敏 `event.handler` 警告后继续。新增“两个 handler 各失败一次仍各执行两轮”的真实引擎回归 |
| 抓取失败丢失分页游标 | 存在；list 先 remove 游标再抓取，失败后插件只能从第 1 页重来。抽出 `advance()`，失败时回填调用前快照（context_expired 除外）。新增 provider 校验失败后用同一游标续页的回归 |
| 无关目录条目阻断所有插件 | 存在；条目读取/规范化错误、越界链接、第 129 个条目都会让 load 直接失败。改为逐条目隔离并跳过，扫描上限128只约束“未找到目标”的情形，链接按 canonical 目标判定。新增 200 个无关目录与越界符号链接（unix）回归 |
| 超时后存储写入仍提交 | 存在；宿主调用超时只丢弃等待 future，spawn_blocking 继续执行并落盘。新增“调用已放弃”令牌（future 丢弃即置位），阻塞任务在加锁前/读取后/提交前复查。新增持锁丢弃调用后断言未写入的回归 |
| scope 检查每次都进阻塞池 | 存在；即使缓存命中也要 spawn_blocking。新增 `QuerySource::cached_scope` 无 I/O 快路径，命中直接返回。新增“缓存命中时阻塞读取不得执行”的回归 |
| CIDR 每次重复解析 / 测试文件缺 `use super::*` | 存在；`2001::/23`、`2002::/16`、`3fff::/20` 改为 OnceLock 一次解析；engine/manager/query 三个测试文件补 `use super::*;`，并新增 QuickJsRuntime 实现 PluginRuntime 的契约断言 |

验证：Rust 全量 711 passed（新增 6 个回归，逐项验证过“去掉修复即失败”），clippy 仍为既有 6 项告警，改动文件 rustfmt 通过。

## 查询可用性与桥契约跟进（基线bc75b18）

| 发现 | 确认与修复 |
|---|---|
| 已有OJ地址修改后所有插件查询不可用 | Medium成立；Registry保留原Adapter是设置页“重启后生效”的既有约定，问题在scope强制等于待生效URL。改为绑定实际运行endpoint；配置变更仍撤销旧context，新context保留原身份键。删除/禁用仍拒绝，缺endpoint/不可用/待重启均有脱敏warn |
| submission.get多页扫描超过入口期限 | Medium成立；整体扫描最多5秒，保留100页及每native页预算。Runtime宿主调用在当前入口硬截止前1秒返回timeout，晚发起请求也有catch处理时间；未捕获错误或超出硬期限仍失败。取消/丢弃扫描用RAII清理内部及未发布游标 |
| stringify抛裸TypeError | Low成立；循环引用、BigInt及异常toJSON统一拒绝冻结的invalid_input对象，不执行host调用，不泄漏异常消息 |
| manager/query聚合挂载测试 | Low成立（仓库约定）；移除tests/mod.rs，所属模块直接#[path]挂载测试，共享support由plugin根直接挂载为cfg(test)的test_support |

真实引擎分别覆盖激活/事件扫描超时及晚发起请求的可捕获性，虚拟时间验证总期限、游标与许可释放；URL回归使用真实配置/注册表且验证诊断不含URL/凭据。保留既有地址重启契约，不热替换有会话和缓存的Provider。

本轮通过完整Rust测试、前端测试/生产构建、SDK类型检查、脚手架构建和改动Rust文件的rustfmt检查。Clippy全部targets完成，仅保留六项既有告警；前端构建保留既有大chunk提示，未新增依赖或联网测试。

## 复审LOW跟进（基线f667a73）

| 发现 | 确认与修复 |
|---|---|
| pluginStore.subscribe 重入泄漏监听器 | Low成立；重复调用先卸载旧订阅（解除Tauri监听并按Service契约清理其可见通知）再建立新订阅，组件重挂载/HMR不再产生重复通知或半失效dismiss；前端回归锁定二次订阅后旧通道事件不再入列 |
| submission.get总期限与单页超时相等 | Low成立；总期限5秒恰等于单页最坏5秒，慢OJ首页即耗尽整个查找预算。提高为8秒（单页最坏+scope检查与快页余量），不变量「总期限+1秒宽限<激活10秒」仍成立并由既有测试锁定；两处虚拟时间测试同步改为5秒页延迟/8秒总期限 |

验证：cargo test 全量704 passed（含真实引擎扫描超时用例）；npm test 505 passed（新增1项重订阅回归）；clippy仅保留既有告警，改动Rust文件rustfmt通过。

## 代码评审跟进修复（基线4621ea3）

本轮处理 PR #41 全量代码评审的 5 项 Medium + 6 项 Low，重点修复 4621ea3 引入的自我一致性回归；每项均配回归测试并验证过“去掉修复即失败”。

| 发现 | 确认与处理 |
|---|---|
| 游标恢复打穿内存记账 | Medium成立；`advance()` 原先赋值后做 `encoded_len` 校验，超限页失败时回填的是含超限页且 `buffered_bytes` 陈旧的游标，绕过 1MiB 单页/4MiB 聚合/32 游标上限。改为在局部变量完成抓取、owner/条数与编码尺寸校验，全部通过后才一次性写回游标，失败时保持真正的调用前快照。新增“超限页失败后同游标重试可续页”回归 |
| cached_scope 快路径谓词手工复制且零覆盖 | Medium成立；快/慢路径各复制一份命中谓词，快路径在令牌撤销后可能静默返回 stale-owner 作用域。抽出单一事实来源 `scope_cache_hit(cached, snapshot)` 供两路径共用，结构上消除漂移；新增谓词单测（命中/revision 不符/缓存令牌撤销/ live 令牌撤销），并在 ABA 用例断言快路径同样拒绝已撤销令牌 |
| handler 失败告警无节流 | Medium成立；worker 不再因失败退出后，持续失败 handler 会按事件速率无限 warn，与 f667a73 建立的每-revision 去重约定冲突。改为按 code 节流：首次立即、60秒（`HANDLER_WARN_INTERVAL`）内静默累计、到期合并一条带累计计数 |
| handler 失败丢失再对齐触发点 | Medium成立；`deliver` 摘要只用于打日志即丢弃，Manager 无从判断需再对齐。新增 `DeliveryOutcome { failed_handlers }` 作为 `deliver_event` 返回；Manager 在 handler 失败时请求一次 Resync，跳过 Resync 通知自身避免失败循环，周期 tick 兜底。Runtime 回归断言 failed_handlers 计数 |
| callsite Dispatch 注册是语义空操作 | Medium成立；`Dispatch::new`+`dispatcher::with_default` 与旧 `subscriber::with_default` 完全等价（后者内部即前者），注释因果论断不成立。撤回为 `subscriber::with_default` 并移除错误注释；改用写入计数断言，事件被 interest 缓存丢失时测试显式失败而非空过 |
| scope warn 去重键过粗 | Low成立；`last_scope_warning` 仅按 revision 去重，`ServiceQueries` 为全进程单例，同修订下其他插件同类失败被静默且归因指向首个命中插件。改为 `scope_warnings` 按 (revision, plugin_id) 集合、随修订清空，每插件首次失败各带自身归因 |
| 目录扫描总条目无界 | Low成立；扫描计数只统计目录，含大量普通文件的 code root 每次 load 被完整遍历；且开始跟随目录符号链接未声明。新增 `MAX_SCAN_ENTRIES=1024` 总条目上限（复用 enumerate 索引），注释与模块文档声明符号链接跟随及 canonical 边界；新增“大量文件触发条目上限截断返回 limit_exceeded”回归 |
| 扫描测试依赖 read_dir 顺序 | Low成立；原测试建 202 目录（超 128 上限）断言目标可寻，哈希序文件系统约 37% 概率失败。噪声目录降到上限以下使发现与顺序无关；另增“目标目录不存在且超上限返回 limit_exceeded 而非 not_found”的确定性回归 |
| HTTP 头黑名单缺 content-type | Low成立；reqwest `header()` 为 append 语义，插件带 body 且自带 content-type 会与宿主强制的 application/json 重复矛盾。`validate_shape` 黑名单加入 `content-type`（由宿主按 body 形态独占决定），新增负例 |
| errorCode 白名单注释夸大 | Low成立；bootstrap 只约束 `[a-z_]{1,40}` 字符集/长度，不校验是否属宿主变体，插件可伪造 code。注释改为准确表述（字符集有界、不可注入但可能由插件提供，按不可信诊断提示处理） |
| SDK 扫描期限文档漂移 | Low成立；`hinina.d.ts` 仍写 submission.get 扫描 5 秒，代码已 8 秒（cce5203 同步 6 个文档唯独漏此文件）。改为 8 秒 |

验证：cargo test 全量 716 passed（新增 5 个回归：超限游标快照/共享谓词/目录上限截断/条目上限截断/handler 失败触发 Resync）；npm test 507 passed（前端 src/ 未改动）；clippy 仅保留 6 项既有告警；改动 Rust 文件 rustfmt 通过；`npm run plugin:sdk:check` 通过（hinina.d.ts 注释同步）。
