# 接入新 OJ 的改动清单与改造建议

> 2026-09-17 ｜ 只做分析，不动代码。
> 讲的是「接一个 Hydro 为什么要改 5 个既有文件」，以及建议怎么改。
> Hydro 协议本身的差异（缺什么能力、哪些有损）在 `doc/Hydro/Hydro-Adapter-设计缺口报告.md`。
>
> **落地情况（2026-09-19）**：改造 1–4 **已由 main 的 #20 全部落地**（`OjId` 取代 `OJType`、
> `ProviderSet` + `AdapterFactory`、`oj.instances`/`contest_ref`、`switch_oj`）。
> 实测效果：Hydro 适配器本轮只需在 `adapter/mod.rs` 加 2 行 + 自己的目录，
> **不再触碰 core/infra/commands**。本文其余内容保留为决策记录；
> 适配过程中新冒出来的问题见 `doc/Hydro/适配新架构的冲突记录.md`。

## 这次实际改了哪些既有文件

| 文件 | 改了什么 | 能否避免 |
|---|---|---|
| `adapter/mod.rs` | 加一行 `pub mod hydro;` | 不能，Rust 模块必须声明 |
| `core/provider/oj_type.rs` | 加 `Hydro` 变体 + `from_name` | 能 |
| `core/entity/config.rs` | `OjConfig` 加 `hydro_url`，`Default`/`validate`/`sanitize` 各加一条 | 能 |
| `core/context.rs` | 15 行注册块 | 能 |
| `commands/auth_cmd.rs` | `parse_oj_type` 加分支 | 能 |

5 个里 3 个落在 core 域层（provider / entity / context）。也就是说，现在「接一个新 OJ」等于「改领域模型」，而不是「加一个适配器」。

但问题**不在 trait**：Hydro 的四个能力全部落进现有四个 trait，没有一个方法需要改签名；20 条协议差异也全在 adapter 里吃掉了。问题集中在「OJ 的身份与配置」这一圈 —— 它们被写成了编译期常量。

一句话：**能力做成了可扩展的，身份和配置做成了写死的**。

## 1. OJType 是闭集枚举

`ProviderRegistry` 拿 `OJType` 当键（`HashMap<OJType, Arc<dyn …>>`），加 OJ 就得加变体，`OJType` 散在 11 个文件里。

比"要改枚举"更麻烦的是两个连带：

- **会话文件路径绑死变体名**：`service/auth/mod.rs:95/138/282` 用 `format!("{:?}", oj_type)` 拼 `sessions/{}.json`。哪天改个变体名，所有人丢会话，编译器不会提醒。
- **事件载荷也被闭集污染**：`core/event/app_event.rs:78` 的 `OJSwitched { oj_type: OJType }`。

另外，配置里 `last_oj_type` 本来就是 `String`（`core/entity/config.rs:46`）—— 一边 String 一边 enum，才逼出了 `parse_oj_type` 这座桥。

建议身份改成数据：

```rust
pub struct OjId(String);                     // "HOJ" / "Hydro" / …
impl OjId {
    pub fn as_str(&self) -> &str { &self.0 }
    pub fn session_file(&self) -> String { format!("{}.json", self.0) }
}
impl HydroAdapter { pub const ID: &'static str = "Hydro"; }   // 适配器自己声明身份
```

好处：接新 OJ 不用再动 core；`parse_oj_type` 整个消失；会话文件名显式且稳定。
代价：10 个文件的机械替换；id 保持 `"HOJ"` 的话 `sessions/HOJ.json` 路径不变，不用迁移数据。

## 2. OjConfig 按 OJ 名硬编码

`oj.hoj_url` 是个以某个 OJ 命名的字段，所以每接一个 OJ 就加一个字段 + 三处分支。

更麻烦的是字段形状是按 HOJ 的领域定的：`contest_id: i64` 装不下 Hydro 的 hex ObjectId（就是缺口报告的 D2）。而「当前比赛」本质上是个不透明引用，不该是数字。

建议改成「配置有哪些 OJ 实例」：

```rust
pub struct OjConfig {
    pub active: String,                 // 当前 OJ id（取代 user.lastOjType）
    pub instances: Vec<OjInstance>,     // { id, base_url, enabled, options }
    pub contest_ref: String,            // 当前比赛引用（HOJ 是数字串，Hydro 是 ObjectId）
    // 全局共享：timeout_secs / poll_* / cache_ttl_secs
}
```

好处：`validate` 退化成一个循环，新增 OJ 零校验代码；`contest_ref` 顺手解决 D2。
代价：**这是唯一要动前端的改造**（SettingsView 加 OJ 选择 + 地址，`types/config.ts` 改结构）。不过这两件事本来就是缺口 D2/D3 要做的，合并做一次就行。

## 3. 注册是手写的 4×N 行

`ProviderRegistry` 把「一个 OJ 的四个能力」拆成四个注册方法，于是组合根每个 OJ 写 4 行 + 4 次 `as Arc<dyn …>` 强转（`core/context.rs:109-112`、`:126-129`）。连带 `provider_registry_impl.rs` 113 行维护 4 个 HashMap，以及 4 个 Service 里 **18 处** `let oj = registry.current_oj(); registry.get_xxx(&oj)?` 的样板。

建议把「一个 OJ 的 Provider 集合」变成一个值：

```rust
pub struct ProviderSet {                 // 字段用 Option，保留「先只实现部分能力」的扩展路径
    pub auth: Option<Arc<dyn AuthProvider>>,
    pub contest: Option<Arc<dyn ContestProvider>>,
    pub problem: Option<Arc<dyn ProblemProvider>>,
    pub submission: Option<Arc<dyn SubmissionProvider>>,
}
pub trait ProviderRegistry {
    fn register(&self, id: OjId, set: ProviderSet);          // 注册侧聚合
    fn current_auth(&self) -> AppResult<Arc<dyn AuthProvider>>;      // 查询侧保留能力视角
    fn current_contest(&self) -> AppResult<Arc<dyn ContestProvider>>; // （接口隔离不受损）
    // … current_problem / current_submission 同理，每个一行转发
}
// adapter/mod.rs：有哪些 OJ 变成数据
pub fn factories() -> Vec<&'static dyn AdapterFactory> { vec![&hoj::FACTORY, &hydro::FACTORY] }
```

好处：注册 4N → N 行；4 个 HashMap → 1；18 处「取 OJ → 取能力」样板变成一行 `registry.current_contest()?`（净减 80~120 行）。

**注意两点**：
- `ProviderSet` 必须用 `Option`，否则会砸掉 `开发手册.md` 里「新 Adapter 可以先只实现部分接口，逐步完善」那条扩展路径；
- **查询侧不要砍成 `current()` 返回聚合体**：那样 Service 会拿到它不需要的三个能力，接口隔离从「接口层面」降到「约定层面」。聚合只用于注册/构造，查询仍按能力拆开（见「对架构的影响」第 2 条）。

下限：`factories()` 那一行省不掉 —— 除非上 `inventory`/`linkme` 做链接期注册，不值得。

## 4. parse_oj_type 是座桥

`core/context.rs` 要解析同一个字符串来决定注册谁，而 `parse_oj_type` 在 commands 层；不把映射提到 core，core 就得反向依赖 commands。这是第 1 条的派生结果 —— 身份一旦是字符串，这座桥自己就没了。

## 5. 顺带：「切换 OJ」塞在 login 参数里

`login(username, password, ojType?)` 内部会 `set_current_oj`。OJ 选择是应用级状态（该落在 `oj.active`），不是某次登录的参数；前端目前也不传它，这能力等于悬空。建议拆成显式 `switch_oj`。优先级最低，保留现状也不算错。

## 改动量

| 改造 | Rust | Vue | 风险 |
|---|---|---|---|
| 1. 身份字符串化 | 10 文件，+250~350 行（基本都是替换） | 0 | 低；id 不变则无数据迁移 |
| 2. ProviderSet + 工厂 | 8 文件，**净减 80~120 行** | 0 | 低，前提是 ProviderSet 用 Option |
| 3. instances 配置 | 4 文件，+150 行（含旧配置迁移） | 4 文件，+150~200 行 | 中 —— 配置迁移是唯一有数据风险的点，沿用现有 `normalize_legacy_values` 那套做法 |
| 4. switch_oj | +30 行 | +20 行 | 低，可选 |

分两期比较顺：1+2 是纯内部重构（外部行为不变），可以随时做；3+4 跟缺口 D2/D3 一起做。

## 对架构的影响

**先说结论**：这是**局部重构，不是架构变更**。分层、trait 拆分、Service 边界、前端分层都不动，动的只是「注册与配置」这一圈把编译期常量换成数据。一句话概括：**不改变架构的形状，改变的是「架构里哪些东西必须随外部世界变化」**。

### 变好的

| 维度 | 现在 | 改造后 | 对应哪条原则 |
|---|---|---|---|
| Domain 稳定性 | 新增一个外部 OJ 要改 Domain 类型（`oj_type.rs`） | 外部集成点数量变化不再触碰 Domain | Clean Architecture：Domain 不该随外部集成点变化 —— **这是目前唯一一处明显违反** |
| 持久化契约 | 会话文件名 = Rust 变体名的 Debug 输出 | 显式 id | 持久化契约要稳定、显式；现在改个变体名就丢会话 |
| 事件契约 | `OJSwitched { oj_type: OJType }` 带编译期枚举 | 可序列化字符串 | EventBus 是解耦机制，事件应跨版本稳定 |
| 命令职责 | 切 OJ 是 login 的副作用，`OJSwitched` 事件发了也观察不到 | 显式 `switch_oj` + 事件 | 「状态变更走显式路径」 |
| 扩展路径 | 新 OJ = 5 文件（3 个在 core） | 新 OJ = 1 目录 + 1 行工厂 + 1 条配置 | 适配层过滤差异的初衷 |

### 会变差的（以及怎么缓解）

这三条是这次改造真实的代价，不写出来就是自欺：

1. **编译期穷尽检查没了**。现在是 `match oj_type { HOJ => …, Hydro => … }`，漏了编译器会提醒；字符串化后「漏注册」变成运行时问题。
   缓解：启动时校验 active 的 OJ 是否已注册并 warn；`get` 返回 `ProviderNotFound`；加一条测试断言 `factories()` 的 id 唯一且都能构建 —— 把「漏实现」的检查从编译期挪到测试层。
   （别想着「保留 enum 内建清单 + `Custom` 兜底」，那样加内建 OJ 还是要改 enum，等于没解决。）

2. **接口隔离可能被削弱**。如果 Service 改成 `registry.current()?.contest`，它就拿到一个含四项能力的聚合体，而它只需要一项 —— 接口隔离从「接口层面」降到「约定层面」。
   缓解：聚合只用于**注册/构造**，查询仍按能力拆开（`registry.current_contest()?`，一行转发）。这样 ISP 完整保留，18 处样板照样清零。
   → 顺带修正我之前的说法：**查询侧不该砍，砍的是注册侧**（不是「12 方法 → 6」）。

3. **组合根「一眼看全依赖图」变弱**。现在 `context.rs` 逐行 `HOJAdapter::new(http, base, bus)`，谁依赖什么一眼可见；改成 `for factory in factories()` 后，要看 `AdapterFactory::build` 才知道。
   缓解：`build` 签名固定，只接收 `AdapterDeps`（http / storage / event_bus 等 **infra 依赖**）。**禁止把 Service 塞进 `AdapterDeps`** —— 与「插件只能访问 `plugin/api`，禁止直接调用内部 Service」同理；否则适配器会反向依赖应用层，依赖边界彻底糊掉。这条要写进约束。

### 不变的部分（别顺手改）

分层顺序与职责（Domain → Application → Adapter → Infra → Plugin）、Provider trait 拆分本身（Hydro 已经验证过）、EventBus 原则、Workspace First、无 SQL、前端 View→Store→Service→Bridge、插件系统的定位（v0.x 仍只预留接口）。

另外**别顺手做「多 OJ 同时在线」**：会牵出 `HttpClient` 全局 Cookie jar 共享（缺口 D14）、会话隔离、`current_oj` 语义一串问题，当前产品形态也不需要。

### 一个额外的正向收益

`AdapterFactory { fn id(); fn build(deps, instance) }` 这个形状，几乎就是 v1.0 插件 manifest 的雏形（id + 构造 + 配置）。将来把「编译期工厂列表」换成「运行时扫描插件目录」，上层（registry / context / Service）不用再改 —— 等于这次顺带把插件化的门留好了，而不是现在就去实现它。

### 落地时要同步的文档

`开发手册.md` §4.2 的「OJ 类型与注册」描述、`Architecture.md` 的「关键设计约束」；`doc/problem.md` 的问题 11（「OJType 缺少 Custom 变体」）可以标掉 —— 解法不是加变体，是取消闭集。

## 改造后适配器要不要改

实测：`adapter/hydro/**` 只 import 了 `core::entity::*`，对 `OJType` / `OjConfig` / Registry **零引用**。所以：

- 改造 1、3、4：适配器**一行都不用改**；
- 改造 2：`adapter/hydro/mod.rs` 加个 `FACTORY`（约 15 行），把四个 trait 实现包进 `ProviderSet`。

改造落地后应该被简化或删掉的代码（也用来验证改造有没有打中要害）：

| 位置 | 现在 | 改造后 |
|---|---|---|
| `OJType::from_name` | 桥接用 | 删 |
| `commands/auth_cmd.rs::parse_oj_type` | 委托 `from_name` | 删，改成校验 id 是否已注册 |
| `core/context.rs:96` | `from_name(...).unwrap_or(HOJ)` | `OjId::new(&oj.active)` + 未注册 warn |
| `core/context.rs:109-129` | 15 行手写注册 | 一个 for 循环，约 8 行 |
| `config.rs` 的 `hydro_url` 及两处校验分支 | 本次新增 | 由 instances 承载，删 |
| `service/auth/mod.rs:95/138/282` | `format!("{:?}", oj_type)` | `OjId::session_file()` |
| 4 个 Service 的 18 处 `current_oj()` | 取 OJ → 取能力 | `registry.current()?.contest` |

`doc/problem.md` 的问题 11（「OJType 缺少 Custom 变体」）也可以借这次标掉 —— 解法不是加变体，是取消闭集。

## 不建议做的

- 插件化 / 动态加载 Adapter：那是 v1.0 插件系统的目标，现在做是过度设计
- `inventory` / `linkme` 链接期注册：为了省一行不值得
- 给 OJ 私有旋钮建强类型枚举：那会让「新 OJ 要改 core」原样复活，`options: Map` 就够
- 多 OJ 同时在线：见上
