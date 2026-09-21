# types

## 职责
Hydro 响应 DTO 与全部归一化纯函数。**不含任何网络调用**，与 `adapter/hoj/types.rs` 同构 —— 纯函数集中在此以便单测穷尽锁定。

## 核心类型/函数

### 响应归一化
- `strip_nulls(&mut Value)` — 递归剔除 `null` 成员。Hydro 对未设置字段返回 `null` 而非省略（榜单/记录/题目文档均如此），而 serde 的 `#[serde(default)]` 只管字段缺失，显式 null 会报 `invalid type: null` 并让**整个响应**解析失败。`false`/`0`/`""` **不是** null，必须保留（否则封榜、打星、零分语义会被抹掉）
- `preview(body) -> String` — 响应体前 200 字符用于错误诊断，按**字符**而非字节截断（响应含中文题面，按字节切会落在 UTF-8 序列中间）

### 错误包络
- `HydroErrorBody` / `HydroErrorEnvelope` — `{"error":{"name","params","code"}}` 的 DTO（**没有 message 字段**）
- `parse_hydro_error(&Value) -> Option<AppError>` — 错误包络 → `AppError`，变体判定委托给 `error::HydroError` 的 `From` 实现（会话失效与业务错误的边界集中在那里）
- `login_redirect_url(&Value) -> Option<String>` — 识别 JSON 化的「未登录重定向」`{"url":"/login?redirect=..."}`。判定保守：只认指向 `/login` 的 url
- `extract_sid(&HeaderMap) -> Option<String>` — 从 `Set-Cookie: sid=<32 位>` 提取会话 ID。登录响应体里**没有** token，这是唯一的取 sid 途径；取第一段 `name=value`，忽略 `Expires`/`Path`/`SameSite` 属性段

### 时间
- `parse_time(raw) -> i64` — 已**上提到 `adapter::time`**（与 HOJ 共用同一份实现），本模块只做再导出以保持调用点与既有测试路径不变；实现细节与失败/偏移告警策略见 `adapter/time.md`
- `objectid_seconds(id) -> Option<i64>` — 从记录的 `_id`（时间型 ObjectId）反推**提交时刻**。Hydro 的记录投影只有 `judgeAt`（评测完成时刻）而没有提交时间，ObjectId 前 4 字节即创建时刻（Hydro 自身也依赖该性质：跨域查询用 `Time.getObjectID(now - 10周)` 做时间过滤）。判定严格：必须 24 位十六进制且折算结果落在 [2010, 2100)，否则 `None`
- `parse_duration_seconds("1:23:45") -> Option<i64>` — 反解 `formatSeconds` 的输出（榜单单元格只给格式化文本，而 `RankCell` 需要秒数）

### 评测状态
- `map_status(i64) -> JudgementStatus` — Hydro 全码表（0 WAITING … 33 HACK_UNSUCCESSFUL）→ 领域变体。**22 FETCHED 折入 `Pending`**（非终态）：它是「评测机已取件、尚未开跑」，若折成 `Unknown` 会让前端轮询在评测开始前就停住。**9 CANCELED 折入 `Cancelled`**（HOJ `-4` 语义精确对应，无文案落差）；11 HACKED / 30 IGNORED / 32 HACK_SUCCESSFUL / 33 HACK_UNSUCCESSFUL 在 HOJ 值域里确实没有对应变体 → `Unknown`；31 FORMAT_ERROR 取语义最近的 `PresentationError`（缺口 D9）
- `is_terminal_status(i64) -> bool` — 非终态集合 `{0, 20, 21, 22}`
- `hoj_status_to_hydro(i32) -> Option<i64>` — HOJ 状态码 → Hydro 状态码（评测页「状态筛选」参数翻译）。前端状态下拉的取值域是 **HOJ 码表（含负数，见 `adapter/hoj/types.rs::map_status`）**，而 Hydro 的 `/record?status=` 收自己的码：`5 Pending→0 WAITING`、`6 Compiling→21`、`7 Judging→20`、`9 Submitting→0`、`0 AC→1`、`-1 WA→2`、`1 TLE→3`、`2 MLE→4`、`3 RE→6`、`-2 CE→7`、`-3 PE→31 FORMAT_ERROR`、`4 SE→8`、`-4 Cancelled→9`、`15 No Status→10 ETC`。返回 `None` 表示 Hydro 没有该语义的状态（`-10` Not Submitted / `8` PA），调用方据此**明确报错**而不是静默忽略筛选条件

### 语言
- `LANG_TABLE` — Hydro 语言 key ↔ 展示名（HOJ 显示名）的双向静态表（`cc.cc17` ↔ `C++17` 等 29 项）。名字刻意选用前端 `utils/language` 能识别的前缀写法，使 Monaco 高亮、源文件名与 limits 倍率判定照常工作
- `lang_display(key) -> String` — 未知 key **原样返回**：Hydro 允许部署自定义 `setting.langs`，原样展示并原样提交回去可保证往返恒等；强行归到 C++ 会让判题端用错语言评测
- `lang_key(display) -> Option<String>` — 提交时反查
- `display_letter(index) -> String` — 0→A … 25→Z，超过 26 题退回十进制序号（与 Hydro 自身的映射一致）

### 值归一
- `coerce_id(&Value) -> String` / `coerce_i64(&Value) -> Option<i64>` / `cell_text(&CellVO) -> String` — Hydro 的 `_id`/`docId`/`pid`/单元格 `value` 混用数字与字符串
- `strip_html(raw) -> String` — 剥离标签 + 反转义常见实体。**刻意不做整体 trim**：榜单单元格的换行是结构（ACM 通过格首行是被剥离的图标、第二行才是 AC 用时），整体 trim 会把两行并成一行、让解析全部错位
- `pending_count(raw) -> Option<i32>` — 提取封榜期间「+N 次待判提交」。**必须锚定 `color:orange`**：AC 单元格首行本身就是 `+{失败次数}`，只找 `+数字` 会把「AC 前的失败次数」误当成待判次数
- `problem_status_code(&Value) -> i32` — `psdoc`/`psdict` → Hinina 三态（0 未提交 / 1 已 AC / 2 尝试过）

### DTO
`UserBriefVO`（+`display()` 回退顺序 `displayName` → `uname`）、`PdocVO`（+`problem_id()`/`doc_id_num()`/`problem_config()` 宽松解析 —— 服务端把 config.yaml 解析错误以**字符串**塞在 `config` 里，非对象一律视为缺失）、`ProblemConfigVO`（+`time_limit_ms()` 取 `timeMax`、`memory_limit_mb()` 取 `memoryMax`）、`TdocVO`（+`contest_id()`/`pid_list()`/`is_locked()`）、`ContestListVO`、`ContestDetailVO`、`ContestProblemListVO`、`ProblemDetailVO`、`SubmitVO`、`TestCaseVO`、`RdocVO`（+`record_id()`/`submit_time()`/`code_length()`/`compiler_message()`/`status_code()`/`terminal_metrics()`）、`RecordDetailVO`、`RecordListVO`、`CellVO`、`ScoreboardVO`、`LoginVO`

### 榜单归一
- `ScoreboardHeader` / `parse_header(&[CellVO])`（私有）— **按表头 `type` 定位列**（rank/user/solved/time/total_score/problem），不依赖列顺序：Hydro 各赛制的列顺序不同（`acm` = rank→user→solved→每题；`oi` 家族 = rank→user→total_score→每题；`homework` 还多一列 time），且文档内示例与速查表不一致
- `acm_rank_cell` / `oi_rank_cell`（私有）— 单元格 → `RankCell`。ACM：`is_ac` 以 `score == 100` 为准、`error_num` 取首行（`+n`/`-n`/✓）、`ac_time` 取第二行、`is_first_ac` 由 `style` 非空判定、`try_num` 由橙色 span 提取。OI 家族：只给分数（`score` = 该题原始分），`error_num`/`ac_time` 留空（缺口 D10）
- `cell_int` / `cell_duration_seconds`（私有）— 单元格整数与时长（优先数字型 `raw`，否则反解格式化文本）
- `scoreboard_rank_page(&ScoreboardVO) -> ContestRankPage` — 单元格矩阵 → 分页榜单。**单页全量**（`current = 1`、`pages = 1`、`total = size = 行数`）：Hydro 服务端不支持榜单分页。打星：`rank.value == "0"` → `-1`（Hydro 的 `db.ranked` 对 `unrank` 直接给 0）。ACM 总提交数由各题「尝试次数」求和反推（AC 题 = 失败次数 + 本次通过）；`total_time` 单位按赛制换算（Hydro 一律给秒，ACM 契约为秒、OI 契约为毫秒 → 非 ACM 赛制 ×1000）

## 直接依赖
- `core::entity::rank::{ContestRankPage, ContestRankRow, RankCell}`
- `core::entity::submission::JudgementStatus`
- `core::error::AppError`
- `adapter::hydro::error::HydroError`
- `serde` / `serde_json::Value` / `reqwest::header::HeaderMap`

## 被依赖
- `adapter::hydro::mod` — 全部 DTO 与映射函数

## 逻辑流程
1. `mod::HydroResponse::into_value()` 拿到 `Value` 后调用 `strip_nulls` → `parse_hydro_error` → `login_redirect_url`
2. `into_json::<T>()` 反序列化为对应 DTO（`PdocVO`/`RdocVO`/`TdocVO`/`ScoreboardVO` …）
3. 映射函数把 DTO 折算为领域实体；榜单走 `scoreboard_rank_page` 的「表头定位 → 逐行取列 → 单元格归一」三步

## 测试
`tests/types_tests.rs`（夹具在 `tests/fixtures/`，按文档手工构造）：
- `strip_nulls` 递归与 falsy 保留、`preview` 按字符截断
- 错误包络 → `Auth`、非包络不误判、登录重定向只认 `/login`、`set-cookie` 解析（含 `xsid` 干扰与缺失头）
- `parse_time`（再导出，用例锁定毫秒/`Z`/空格/时区偏移/闰日/空串）、`objectid_seconds`（越界与非 ObjectId 一律拒绝）、`parse_duration_seconds`
- `map_status` 全码表（含 `22 → Pending`）、`is_terminal_status` 穷尽、`hoj_status_to_hydro` 映射与拒绝
- 语言表**双射**校验（展示名唯一，往返恒等）、未知 key 透传、前端可识别的展示名、`display_letter` 边界
- `coerce_*`、`strip_html`（**保留换行结构**）、`pending_count`（必须锚定 `color:orange`）、`problem_status_code`
- 六个夹具的 DTO 解析（含 `config` 为字符串的容错、`duration` 推算、`ObjectId` 提交时间、非终态指标清零）
- 榜单矩阵归一：ACM 三行（普通/打星/封榜待判）逐字段断言、OI 分数语义、空榜单、表头无字母时的回退
