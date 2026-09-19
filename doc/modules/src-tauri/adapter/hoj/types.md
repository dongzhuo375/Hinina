# types

## 职责
HOJ API DTO 类型定义 + 状态码映射（`map_status` / `is_terminal_status` / `normalize_error_message`）+ 榜单/题目状态归一函数（`into_rank_row` / `cell_from_value` / `extract_problem_status_code` / `normalize_problem_status`）+ 测试点宽松转换（`lenient_case_list`）+ 响应体 null 归一（`strip_nulls`）。

## 核心类型
- `strip_nulls(&mut Value)` — 递归剔除对象中值为 `null` 的成员与数组中的 `null` 元素。HOJ 对未设置的字段返回 `null` 而不是省略，而 serde 的 `#[serde(default)]` **只在字段缺失时生效**，显式 null 会报 `invalid type: null, expected a boolean` 并让整个响应解析失败；剔除后 `null` 与「缺失」等价（非 Option 字段落到默认值，Option 字段落到 `None`），与 HOJ 语义一致。**`false` / `0` / `""` 不是 null，必须保留** —— 否则封榜、打星、零分等语义会被抹掉。由 `HOJAdapter::parse_hoj_json` 在类型化解析前统一调用
- `ApiResponse<T>` — 统一响应包装 `{status, msg, data}`。成功码是 **200 而不是 0**；`status` 可缺失（`#[serde(default)]`），缺失时按失败处理并由 `into_data()` 给出消息
- `PageResult<T>` — 分页 `{records, total, size, current, pages}`，**全部字段可缺失**：HOJ 的分页对象在不同接口上返回的键并不一致（`get-contest-list` 实测含 `records/total/size/current/orders/searchCount/pages`，文档只承诺 `records/total`），缺任何一个都不应让整页数据解析失败。带 `#[serde(bound(deserialize = "T: serde::Deserialize<'de>"))]`：`records` 上的 `default` 会让 serde 自动给 `T` 加 `Default` 约束，而各 VO 并没有（也不该有）`Default` 实现
- `LoginRequest` / `UserInfoVO` — 认证
- `ContestVO` / `ContestProblemVO` — 比赛。`ContestVO` 含榜单相关字段：`rank_show_name: Option<String>`（username/realname/nickname）、`seal_rank: bool`、`seal_rank_time: Option<String>`（ISO 字符串，可 null）、`allow_end_submit: bool`、`oi_rank_score_type: Option<String>`（OI 榜单计分规则 "Recent"/"Highest"，非 OI 赛为 null）
- `ContestRankDTO` — `POST /api/get-contest-rank` 请求体：`cid, current_page, limit, force_refresh, remove_star, keyword, contains_end, concerned_list, external_cid_list`。`force_refresh` 恒 false（非创建者/超管传 true 会被服务端忽略，封榜应以 `Contest::seal_rank` + `seal_rank_time` 自行判断）；`concerned_list` 为客户端本地维护的关注列表，本项目暂不使用；`external_cid_list` 联赛合并榜单用，单场比赛恒 null
- `ContestRankVO` — 榜单记录（ACM `ACMContestRankVO` 与 OI `OIContestRankVO` 共用的宽松 DTO）。两种赛制的 `submissionInfo` 值类型不同（ACM 为对象、OI 为得分整数），故保留为 `serde_json::Value` 后归一 —— 即使赛制判断失误或 HOJ 调整字段，也只是单元格降级为默认值，不会让整页解析失败
- `AcmSubmissionInfo` — ACM 榜单单元格明细（`submissionInfo` 的对象形态）：`error_num, try_num, is_ac, is_first_ac, ac_time, is_after_contest`
- `UserProblemStatusDTO` — `POST /api/get-user-problem-status` 请求体：`pid_list, is_contest_problem_list, cid, gid, contains_end`
- `ContestRankVO::into_rank_row(self) -> ContestRankRow` — 归一为 OJ 无关榜单行（ACM 与 OI 共用同一入口）；Option 字段 `unwrap_or_default`，`submission_info` 逐值经 `cell_from_value`
- `cell_from_value(&Value) -> RankCell`（私有）— 整数值 → OI 得分单元格；对象 → 解析 `AcmSubmissionInfo`；解析失败降级为 `RankCell::default()` 而不是让整行/整页失败（榜单是赛场高频只读数据，局部字段异常不应导致整页不可用）
- `extract_problem_status_code(&Value) -> Option<i32>` — 从 `get-user-problem-status` 的响应条目里取出 **HOJ 原始评测状态码**（`{"status": <码>, "score": …}`）。文档标注值类型为 `Object`，故对数字/布尔/对象（取 `status` 字段）三种形态都容错；无法识别返回 `None`，由调用方按「未提交」处理（保守：不会把未做的题标成已通过）
- `normalize_problem_status(i32) -> i32` — HOJ 原始码 → 前端「我的题目状态」契约 `0=未提交 / 1=已AC / 2=尝试过`：`0`（Accepted）→ `1`，`-10`（Not Submitted）/ 无法识别 → `0`，其余 → `2`。**HOJ 这里返回的是评测状态码（与提交状态同一张码表），不是 0/1/2 三态** —— 且请求体必须带 `isContestProblemList=true`，否则服务端只统计非比赛提交（实测比赛 1012：`false → {"1000":{"status":-10}}` 未提交，`true → {"1000":{"status":0}}` 已 AC）
- `ProblemVO` / `ProblemInfoVO` / `TagVO` — 题目
- `JudgeVO` — 提交后返回的 Judge 对象，**同时作为提交列表（`contest-submissions`）条目的宽松 DTO**：列表场景比提交响应多出 `uid` / `title` / `display_id` / `time` / `memory` / `score` / `oi_rank_score` 等字段，全部按可缺失处理 —— 两个接口共用一个 DTO，缺哪个都落到默认值
- `SubmissionInfoVO` / `SubmissionDetail` — 提交详情响应（`get-submission-detail`）：`SubmissionInfoVO` 仅包一层 `submission`；`SubmissionDetail` 含 `code: Option<String>`（未开分享或权限不足时为 null）、`error_message`（CE 时非空）、`judger`、`oi_rank_score` 等完整字段，全字段宽松
- `AnnouncementVO` — 比赛公告条目（`get-contest-announcement`）：`id: i64`, `title`, `content: Option<String>`（HTML，可 null）, `uid`, `username`, `create_time` / `update_time`（ISO 字符串）。**时间字段的真实键名是 `gmtCreate` / `gmtModified`**（实测），故显式 `rename` —— 若按 `rename_all = "camelCase"` 推成 `createTime`，`createdAt` 会恒为 0（界面显示 1970）
- `JudgeCaseVO` — `GET /api/get-all-case-result` 响应：`judge_case_list` / `sub_task_judge_case_vo_list` 两个列表都保留为原始 `serde_json::Value` 逐条转换（SubTask 形态在文档中不完整，单条测试点字段类型异常时只跳过该条，绝不让整个响应解析失败）+ `judge_case_mode: Option<String>`
- `JudgeCaseDTO` — 单个测试点（全字段宽松 Option）：`submit_id`, `case_id`, `status`（与提交状态同一张码表）, `time`, `memory`, `score`, `group_num`, `seq`, `mode`
- `SubTaskDTO` — 子任务分组（文档未完整给出形态，按宽松结构解析）：`group_num: Option<i64>`, `judge_case_list: Vec<Value>`
- `lenient_case_list(&[Value]) -> Vec<JudgeCaseDTO>` — 逐条宽松转换测试点列表：类型异常的条目直接跳过（warn 由调用方记录）
- `ERROR_MESSAGE_PLACEHOLDER` — HOJ 在「错误信息不可查看」时回填的占位文案（`"The error message does not support viewing."`）。出处：HOJ `JudgeManager.getSubmissionInfo` —— 只要状态不是 CE / SE / SF 就把 `errorMessage` 覆写成这句话（即 AC/WA/TLE… 全部都会带上它）
- `SubmitRequest` — 提交请求体（`pid` / `language` / `code` / `cid` / `tid` / `gid` / `isRemote`，camelCase 序列化）。**`pid` 必须经 `submit_pid` 计算**
- `submit_pid(problem_id, display_id) -> String` — 计算提交请求体的 `pid`。HOJ 的 `pid` 语义随 `cid` 变化：比赛提交（`cid != 0`）时服务端拿它查 `contest_problem.display_id`，查不到直接 `contestProblem.getId()` **NPE → HTTP 500**，故必须传**比赛内展示题号**（`"A"`）；非比赛提交按题目展示 ID 查。`display_id` 为空/纯空白时退回 `problem_id`（非比赛场景两者同源，且保证不会提交空 pid）。抽成纯函数以便单测锁定 —— 写错的表现是**所有比赛提交都 500 且服务端不留记录**，排查成本极高
- `normalize_error_message(Option<String>) -> Option<String>` — 过滤占位串与空串：它是「本状态没有错误信息」的标记而非错误内容，不过滤会让每份 AC 代码的详情页都弹出一块红色「错误信息」面板
- `map_status(i32) -> JudgementStatus` — HOJ 评测状态码 → 领域枚举。码表出自 HOJ `Constants.Judge`（`JudgeServer/.../util/Constants.java`），**取值域含负数，不是「0 起顺排」**：
  - `-10`→NotSubmitted、`-4`→Cancelled、`-3`→PresentationError、`-2`→CompilationError、`-1`→WrongAnswer
  - `0`→**Accepted**、`1`→TimeLimitExceeded、`2`→MemoryLimitExceeded、`3`→RuntimeError、`4`→SystemError
  - `5`→Pending、`6`→Compiling、`7`→Running（Judging 沿用既有 Running 语义）、`8`→PartiallyAccepted（独立变体，不折算 AC）
  - `9`→Pending（Submitting：判题机尚未接手，必须是非终态）、`10`→SubmitFailed、`15`→Unknown（No Status）
  - 码表之外一律 `Unknown`
- `is_terminal_status(i32) -> bool` — 是否终态（需要停止轮询）：`!matches!(status, 5 | 6 | 7 | 9)`。与 `map_status` 的产物必须等价（`map_status(code).is_terminal() == is_terminal_status(code)`，有测试逐码锁定）

## 序列化约定
- 所有响应 DTO 使用 `#[serde(rename_all = "camelCase")]`，将 Rust snake_case 字段映射到 HOJ 的 camelCase JSON（如 `start_time` ↔ `startTime`、`submit_id` ↔ `submitId`、`role_list` ↔ `roleList`）。
- **例外**：`AcmSubmissionInfo` 的字段名按 HOJ 原始 JSON 显式 `rename`（`isAC` / `isFirstAC` / `ACTime` 大小写不规则，不能依赖 `rename_all = "camelCase"` 的自动推导）。
- 服务端可能返回 `null` 的字段（如 `nickname`、`avatar`、`description`、`examples`、`seal_rank_time`）声明为 `Option<String>` 而非 `String`。但**仅靠 Option 与 `#[serde(default)]` 并不足以容错**：HOJ 对未设置字段返回显式 `null`，非 Option 字段（如 `seal_rank: bool`、`color: String`、`time: i64`）遇到 `null` 仍会解析失败，故统一由 `strip_nulls` 在解析入口处理（详见「核心类型」）。新增字段无需逐个标注，但**新增非 Option 字段时必须确认它已在 `strip_nulls` 的覆盖路径上**（即经 `parse_hoj_json` 解析）。
- 请求 DTO（`ContestRankDTO` / `UserProblemStatusDTO`）仅 `Serialize`，同样 camelCase。

## 测试
`src-tauri/src/adapter/hoj/tests/types_tests.rs` 锁定：`map_status` 逐码测试 + `map_status_covers_full_hoj_code_table` **全码表一次性锁定**（HOJ `Constants.Judge`，任何一格改动都会让此测试失败）+ `map_status_and_is_terminal_status_agree`（两张表逐码等价）、`is_terminal_status` 判据（`is_terminal_full_table`：仅 5/6/7/9 非终态）、`normalize_error_message` 过滤占位串与空串但保留真实错误、`submit_pid` 优先展示题号 / 空白裁剪 / 无展示题号时回退真实 ID、`SubmitRequest` wire 字段名（含 `isRemote`）与比赛提交 `pid == "A"`、ACM 榜单行不规则字段名（`isAC`/`isFirstAC`/`ACTime`）解析、OI 榜单行得分与 `timeInfo` 归一、打星行 `rank=-1` 保留且可选文本取默认值、畸形单元格降级为默认而不拖垮整行、`ContestRankDTO` camelCase 序列化且 `forceRefresh` 恒 false、`extract_problem_status_code` 数字/布尔/对象三形态与未知形态返回 `None`、`normalize_problem_status` 码值归一（含实测样本端到端）、`ContestVO` 榜单/封榜新字段解析与字段缺失容错。

> **回归背景（本轮）**：码表曾整体错位（`0→Pending`、`5→Accepted`、`13→PA`），后果是所有 AC 提交被显示为 Pending 并无限轮询 —— 实测提交 1166 服务端 `status:0` + `time:2ms`/`memory:532KB` 是 AC，界面却永远转圈；评测页状态筛选选「Accepted」实际筛的是 HOJ 的 Pending。修改码表时**必须对照 HOJ 源码或真实响应**，不要凭「0 开头即排队中」的直觉推排。

null 容错专项（回归自「登录页拿不到比赛列表」故障）：`strip_nulls` 剔除 null 成员与 null 数组元素且保留 `false`/`0`、`ContestVO` 对真实 null 分布可解析（并反向断言不去 null 必然失败）、`PageResult` 接受文档最小形状与 `records`/`total` 为 null 的退化形状、`SubmissionDetail` 容忍评测未完成时的 `time`/`memory`/`score` 为 null、`AcmSubmissionInfo` 容忍封榜期只有 `tryNum` 的单元格、`ApiResponse` 的 `data: null` 归为 `None` 且 `status=401` 判为失败。
