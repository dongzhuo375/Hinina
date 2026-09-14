# HOJ 比赛题目 Limits（时限 / 内存）API 详解

> 本文档是 `HOJ-API-Documentation.md` 的补充细化文档，专门讲解「比赛题目的时间限制 / 内存限制 / 栈限制」相关的接口与语义，可直接用于 HOJ 客户端实现。
>
> 相关文档：`HOJ-Contest-Rank-API.md`（比赛排行榜）、`HOJ-API-Documentation.md`（主文档）。
>
> 代码来源：
> - 前端：`hoj-vue/src/common/api.js`、`hoj-vue/src/views/oj/problem/Problem.vue`、`hoj-vue/src/components/oj/common/CodeMirror.vue`、`hoj-vue/src/views/admin/problem/Problem.vue`
> - 后端：`controller/oj/ContestController.java`、`controller/oj/ProblemController.java`、`manager/oj/ContestManager.java`、`manager/oj/JudgeManager.java`、`JudgeServer/.../JudgeContext.java`、`.../JudgeRun.java`、`.../SandboxRun.java`、`pojo/entity/problem/Problem.java`、`pojo/entity/contest/ContestProblem.java`、`sqlAndsetting/hoj.sql`

---

## 目录

1. [核心结论（先看这里）](#1-核心结论先看这里)
2. [返回 limits 的接口](#2-返回-limits-的接口)
3. [不返回 limits 的接口（避免踩空）](#3-不返回-limits-的接口避免踩空)
4. [字段含义与单位](#4-字段含义与单位)
5. [语言倍率：C/C++ 1 倍，其它语言 2 倍](#5-语言倍率cc-1-倍其它语言-2-倍)
6. [判题端实际如何使用 limits](#6-判题端实际如何使用-limits)
7. [在线调试接口（自带题目的 limits）](#7-在线调试接口自带题目的-limits)
8. [如何验证实际耗时 / 内存](#8-如何验证实际耗时--内存)
9. [客户端实现建议](#9-客户端实现建议)
10. [附录：管理员接口](#10-附录管理员接口)

---

## 1. 核心结论（先看这里）

1. **limits 是「题目」的属性，不是「比赛题目」的属性。**
   - `problem` 表才有 `time_limit` / `memory_limit` / `stack_limit`；
   - `contest_problem` 表只有 `display_id` / `cid` / `pid` / `display_title` / `color`（见 `sqlAndsetting/hoj.sql` 第 228 行），**没有任何时限或内存字段**；
   - 因此比赛题目无法在比赛中单独覆盖时限/内存，它用的就是原题（`problem` 表）的限制。
2. **比赛题目的 limits 只能通过「题目详情」类接口拿到**，题目列表类接口一律不返回 limits。
3. 获取比赛题目 limits 的**唯一接口**：

```
GET /api/get-contest-problem-details?cid={cid}&displayId={displayId}&containsEnd={containsEnd}
```

   响应中的 `data.problem.timeLimit` / `data.problem.memoryLimit` / `data.problem.stackLimit` 即为该题限制。

4. **单位**：`timeLimit` = 毫秒（ms），`memoryLimit` = MB，`stackLimit` = MB。
5. **语言倍率**：题面展示的 limits 是 **C/C++ 的限制**，其它语言在判题时**时间和内存都 ×2**（服务端 `JudgeContext` 实现，见第 5 节）。

---

## 2. 返回 limits 的接口

### 2.1 GET /api/get-contest-problem-details（比赛题目详情 —— 本文重点）

```
GET /api/get-contest-problem-details?cid={cid}&displayId={displayId}&containsEnd={containsEnd}
Authorization: {token}
```

- **认证要求**：**需要登录**（`@RequiresAuthentication`）
- **权限校验**：`contestValidator.validateContestAuth` —— 私有赛需已注册；比赛创建者 / 超级管理员可绕过
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cid` | Long | 是 | 比赛 ID |
| `displayId` | String | 是 | 比赛中的题号（`A`、`B`、`C`…） |
| `containsEnd` | Boolean | 否 | 默认 `false`；是否包含比赛结束后的提交统计 |

- **响应数据**：`ProblemInfoVO`

```json
{
  "problem": {
    "id": 1001,                 // 题目真实 ID（pid）
    "problemId": "HOJ-1001",    // 题目展示 ID
    "title": "A + B Problem",   // 注意：已被替换为比赛的 displayTitle
    "type": 0,                  // 0=ACM，1=OI
    "timeLimit": 1000,          // ★ 时间限制（ms）
    "memoryLimit": 256,         // ★ 内存限制（MB）
    "stackLimit": 128,          // ★ 栈限制（MB）
    "judgeMode": "default",     // default / spj / interactive
    "judgeCaseMode": "default", // default / subtask_lowest / subtask_average
    "ioScore": 100,             // OI 题分数
    "difficulty": null,         // 比赛未结束时被置为 null
    "source": null,             // 比赛未结束时被置为 null
    "author": null,             // 比赛未结束时被置为 null
    "auth": 1,
    "isFileIO": false,
    "ioReadFileName": null,
    "ioWriteFileName": null,
    "isRemoveEndBlank": true,
    "openCaseResult": true,
    "description": "...",
    "input": "...",
    "output": "...",
    "examples": "...",
    "hint": "..."
  },
  "tags": [],
  "languages": ["C", "C++", "Java", "Python3"],
  "problemCount": {
    "pid": 1001, "total": 120, "ac": 50,
    "wa": 40, "tle": 20, "mle": 5, "re": 3, "pe": 1, "ce": 1, "se": 0, "pa": 0
  },
  "codeTemplate": {
    "C": "#include <stdio.h>\n...",
    "C++": "#include <iostream>\n..."
  }
}
```

**比赛题目的特殊行为（`ContestManager.getContestProblemDetails`）**

| 行为 | 说明 |
|------|------|
| `problem.title` | 被替换为 `contest_problem.display_title`（比赛中显示标题） |
| `problem.source` / `author` / `difficulty` | **比赛未结束时被置为 `null`**，结束后才返回真实值 |
| `tags` | 比赛未结束时返回空数组（结束后才返回题目标签） |
| `problem.timeLimit` / `memoryLimit` / `stackLimit` | **不受比赛影响，始终返回原题限制** |
| `problem.auth == 2`（私有题） | 抛 403 `该比赛题目当前不可访问！` |
| `displayId` 不存在 | 抛 `该比赛题目不存在` |
| `problemCount` | 只统计**本场比赛**（`contest_record`）的提交，且已剔除比赛管理员与超级管理员的提交；比赛封榜时会按封榜时间截断统计 |

> **客户端提示**：limits 与比赛状态无关，任何时候（未开始/进行中/已结束）都能拿到同样的值（但未开始/未注册时会因权限被拒）。

### 2.2 GET /api/get-problem-detail（普通题目详情 —— 同样的 limits 结构）

```
GET /api/get-problem-detail?problemId={problemId}&gid={gid}
```

- **认证要求**：无（`@AnonApi`），但只能查 `auth = 1` 的公开题目
- **请求参数**：`problemId`（展示 ID，必填）、`gid`（团队 ID，可选）
- **响应数据**：`ProblemInfoVO`，`problem.timeLimit` / `memoryLimit` / `stackLimit` 语义与 2.1 完全相同
- 详见主文档 `4.2 获取题目详情`

> 客户端若要统一展示「题目限制」，建议对比赛题目调用 2.1，对普通题目调用 2.2，然后读同一个字段路径 `data.problem.{timeLimit,memoryLimit,stackLimit}`。

---

## 3. 不返回 limits 的接口（避免踩空）

以下接口经常被误认为能拿到 limits，实际**都没有**这些字段：

| 接口 | 返回类型 | 缺失的字段 | 备注 |
|------|----------|-----------|------|
| `GET /api/get-contest-problem?cid=&containsEnd=` | `List<ContestProblemVO>` | `timeLimit`/`memoryLimit`/`stackLimit` | 只有 `displayId`、`pid`、`displayTitle`、`color`、`ac`、`total` |
| `GET /api/get-problem-list` | 分页 `ProblemVO` | 同上 | 只有题目元信息与各状态统计（`ac`/`wa`/`tle`/`mle`…） |
| `GET /api/get-full-screen-problem-list?cid=` | `List<ProblemFullScreenListVO>` | 同上 | 只有 `pid`、`problemId`、`title`、`status`、`score` |
| `GET /api/contest-submissions` | 分页 `JudgeVO` | 同上 | 只有单次提交的 `time`（ms）/`memory`（KB），不是题目限制 |
| `POST /api/get-contest-rank` | 分页榜单 | 同上 | 榜单里没有 limits 字段 |

> 也就是说：**要「题目限制」必须调题目详情接口**；榜单/列表页如果同时要展示 limits，需要额外对每道题调用 2.1（建议懒加载或缓存）。

---

## 4. 字段含义与单位

来源：`pojo/entity/problem/Problem.java`

| 字段 | 类型 | 单位 | 默认值（前端后台表单） | 说明 |
|------|------|------|----------------------|------|
| `timeLimit` | Integer | **毫秒（ms）** | `1000` | CPU 时间限制 |
| `memoryLimit` | Integer | **MB** | `256` | 内存限制 |
| `stackLimit` | Integer | **MB** | `128` | 栈空间限制 |

**关于单位的两个坑**

1. `sqlAndsetting/hoj.sql` 里 `memory_limit` 的注释写的是「单位kb」，默认值 `65535`，这是**过时注释**。实际全链路按 **MB** 处理：
   - 后端 `JudgeContext`：`problem.getMemoryLimit() * 1024` → 得到 KB，证明 `memoryLimit` 是 MB；
   - 前端后台表单默认值：`memoryLimit: 256`、`stackLimit: 128`（`views/admin/problem/Problem.vue`）；
   - 题目详情页展示：`{{ problem.memoryLimit }}MB`（`views/oj/problem/Problem.vue`）。
2. `stackLimit` **不在页面上显著展示**，但真实参与判题（沙箱 `stackLimit`），客户端如需展示完整信息可以一并显示。

---

## 5. 语言倍率：C/C++ 1 倍，其它语言 2 倍

题目里的 `timeLimit` / `memoryLimit` 是**基准值**，服务端按语言做倍率放大（`JudgeServer/.../JudgeContext.java`）：

```java
// c和c++为一倍时间和空间，其它语言为2倍时间和空间
LanguageConfig languageConfig = languageConfigLoader.getLanguageConfigByName(judge.getLanguage());
if (languageConfig.getSrcName() == null
        || (!languageConfig.getSrcName().endsWith(".c")
        && !languageConfig.getSrcName().endsWith(".cpp"))) {
    problem.setTimeLimit(problem.getTimeLimit() * 2);
    problem.setMemoryLimit(problem.getMemoryLimit() * 2);
}
```

- 判断依据是**语言的源文件后缀**（`srcName` 以 `.c` / `.cpp` 结尾为一倍），不是语言名字符串；
- 因此 Java / Python / Go 等语言：**时间 ×2、内存 ×2**（栈限制不放大）；
- 前端题目页也是这么向用户解释的（`views/oj/problem/Problem.vue`）：

```
时间限制：C/C++ 1000MS，其它 2000MS
内存限制：C/C++ 256MB，其它 512MB
```

- **提交详情里返回的 `time` / `memory` 会被截断**为题目限制值（`Math.min`），所以看到 `time == timeLimit` 且状态为 TLE 是正常现象：

```java
finalJudgeRes.setMemory(Math.min(memory, problem.getMemoryLimit() * 1024)); // KB
finalJudgeRes.setTime(Math.min(time, problem.getTimeLimit()));              // ms
```

> 客户端展示建议：把 `timeLimit` / `memoryLimit` 按语言做 ×2 后再显示（与前端一致），或直接标注「C/C++ 1 倍、其它语言 2 倍」。

---

## 6. 判题端实际如何使用 limits

`JudgeRun` → `JudgeGlobalDTO` → `SandboxRun` 的换算关系（供客户端展示「实际判题阈值」时参考）：

| 项 | 计算方式 | 说明 |
|----|----------|------|
| `maxTime` | `problem.timeLimit`（已按语言 ×2） | 传入沙箱作为 `cpuLimit` |
| `cpuLimit` | `maxTime * 1000 * 1000`（ns） | 沙箱 CPU 时间上限 |
| `clockLimit` | `maxTime * 1000 * 1000 * 3`（ns） | 真实墙钟时间上限 = CPU 上限的 **3 倍** |
| `memoryLimit` | `(maxMemory + 100) * 1024 * 1024`（byte） | 即 **题目内存 + 100MB 余量**（沙箱自身开销） |
| `stackLimit` | `maxStack * 1024 * 1024`（byte） | 栈限制 |
| `testTime` | `problem.timeLimit + 200`（ms） | 评测运行时给测试点的额外时间余量 |

其它评测细节（会体现在结果里，但**不改变题目 limits 字段**）：

- `judgeMode`：`default` / `spj`（特判）/ `interactive`（交互）
- `judgeCaseMode`：`default` / `subtask_lowest` / `subtask_average` / `ergodic_without_error`
- `isFileIO` + `ioReadFileName` / `ioWriteFileName`：文件 IO 模式

---

## 7. 在线调试接口（自带题目的 limits）

在线调试（自测）**不接受客户端传入 limits**，服务端直接读取题目自身的 `timeLimit` / `memoryLimit` / `stackLimit`，因此可用来验证题目的实际限制。

### 7.1 POST /api/submit-problem-test-judge

```
POST /api/submit-problem-test-judge
Authorization: {token}
Content-Type: application/json
```

- **认证要求**：需要登录，且需要 `submit` 权限
- **请求体**：`TestJudgeDTO`

```json
{
  "pid": 1001,                  // 题目 ID（必填）
  "type": "contest",            // 评测类型：public / contest / group（必填）
  "code": "#include <iostream>...",
  "language": "C++",            // 必填，须在 HOJ 支持语言列表内
  "userInput": "1 2",           // 必填，长度 ≤ 1000 字符
  "expectedOutput": "3",        // 可选
  "isRemoteJudge": false,       // 是否为远程 OJ 题目
  "mode": "text/x-csrc"         // 远程题时用于鉴别语言
}
```

- **响应数据**：`String` —— `testJudgeKey`（用于轮询结果）
- **限制来源**（`JudgeManager.submitProblemTestJudge`）：

```java
Problem problem = problemEntityService.getById(testJudgeDto.getPid());
testJudgeReq.setMemoryLimit(problem.getMemoryLimit())
        .setTimeLimit(problem.getTimeLimit())
        .setStackLimit(problem.getStackLimit())
        ...
```

  之后同样经过 `JudgeContext.testJudge` 的**非 C/C++ ×2** 处理。
- **注意**：
  - 有提交频率限制（`defaultSubmitInterval`），超出报 403 `对不起，您使用在线调试过于频繁，请稍后再尝试！`；
  - `type` 会做权限校验（`public` / `contest` / `group` 分别对应不同权限），传错报 `请求参数type错误！`；
  - 使用了 `pid`（题目真实 ID），比赛题目上下文中可从榜单/详情拿到 `problem.id`。

### 7.2 GET /api/get-test-judge-result

```
GET /api/get-test-judge-result?testJudgeKey={testJudgeKey}
```

- **响应数据**：`TestJudgeVO`

```json
{
  "status": 5,          // 评测状态码，见主文档第 7 节
  "userInput": "1 2",
  "userOutput": "3",
  "expectedOutput": "3",
  "memory": 10240,      // 实际内存（KB）
  "time": 15,           // 实际耗时（ms）
  "stderr": "",
  "problemJudgeMode": "default"
}
```

- `status` 为 `0`（Pending）或 `1`（Judging）时需要继续轮询；结果在 Redis 中缓存 **10 分钟**。

---

## 8. 如何验证实际耗时 / 内存

客户端若要在提交详情里展示「耗时 / 内存 vs 限制」：

```
GET /api/get-submission-detail?submitId={submitId}
```

- 返回的 `submission.time` 单位 **ms**，`submission.memory` 单位 **KB**（注意与 `memoryLimit` 的 MB 不同，需要换算）。
- 相关评测状态码：

| 状态码 | 含义 | 与 limits 的关系 |
|--------|------|------------------|
| 5 | Accepted | 未超限 |
| 6 | Time Limit Exceeded (TLE) | 超出 `timeLimit`（其它语言 ×2） |
| 7 | Memory Limit Exceeded (MLE) | 超出 `memoryLimit`（其它语言 ×2） |
| 8 | Output Limit Exceeded (OLE) | 输出超限，与题目 limits 无关 |
| 9 | Runtime Error (RE) | 可能与 `stackLimit` 有关（爆栈） |
| 13 | Partially Accepted | OI 题部分通过，可能部分测试点 TLE/MLE |

> 换算提示：`memory`(KB) / 1024 = MB，可与 `memoryLimit`(MB) 直接比较。

---

## 9. 客户端实现建议

1. **数据获取路径**：比赛题目 limits 一律走 `GET /api/get-contest-problem-details`（需带 Token）；不要把 `get-contest-problem` 的列表当作 limits 来源。
2. **缓存 limits**：limits 对同一题基本不变，建议按 `pid` 缓存（内存 + 本地持久化），避免榜单/题目列表每行都请求详情。
3. **显示策略**：与网页端保持一致，展示为
   `时间限制：C/C++ {timeLimit}MS，其它 {timeLimit*2}MS`
   `内存限制：C/C++ {memoryLimit}MB，其它 {memoryLimit*2}MB`
4. **单位统一**：内部统一用 ms / MB 存储；提交详情的 `memory` 是 KB，记得 `/1024`。
5. **权限处理**：比赛未开始时或未注册私有比赛，`get-contest-problem-details` 会返回 403/401，客户端不要把它当成「拿不到 limits」而回退成默认值，应区分错误类型做提示。
6. **提交页联动**：提交代码时不需要传 limits（`/api/submit-problem-judge` 的 `SubmitJudgeDTO` 不含 limits 字段），服务端自动读取题目限制；在线调试同理。
7. **远程判题（VJ）题目**：`problem.isRemote = true` 时，`isRemoteJudge` 传 `true`，limits 仍是 HOJ 侧录入的值，可能与源 OJ 不完全一致。

---

## 10. 附录：管理员接口

若客户端需要做「题目/比赛题目管理」功能，以下接口可返回完整的 `Problem`（含 limits）。注意都需要对应角色（`root` / `admin` / `problem_admin`）：

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/admin/problem?pid={pid}` | 获取题目完整实体（含 `timeLimit`/`memoryLimit`/`stackLimit`） |
| `GET` | `/api/admin/problem/get-problem-list?limit=&currentPage=&keyword=&auth=&oj=` | 分页返回 `IPage<Problem>`（含 limits） |
| `PUT` | `/api/admin/problem` | 修改题目（请求体 `ProblemDTO`，含 limits） |
| `GET` | `/api/admin/contest/problem?pid={pid}` | 获取比赛题目对应的完整 `Problem` 实体（含 limits） |
| `PUT` | `/api/admin/contest/problem` | 修改比赛题目信息（`ContestProblem`：displayId/title/color 等，**不含 limits**） |

---

## 附：字段速查

**limits 字段（`Problem` 实体，位于 `ProblemInfoVO.problem`）**

| 字段 | 单位 | 默认值 | 说明 |
|------|------|--------|------|
| `timeLimit` | ms | 1000 | 时间限制（C/C++ 基准，其它语言 ×2） |
| `memoryLimit` | MB | 256 | 内存限制（C/C++ 基准，其它语言 ×2） |
| `stackLimit` | MB | 128 | 栈限制（不放大） |

**相关接口速查**

| 用途 | 接口 |
|------|------|
| 比赛题目 limits | `GET /api/get-contest-problem-details?cid=&displayId=&containsEnd=` |
| 普通题目 limits | `GET /api/get-problem-detail?problemId=&gid=` |
| 比赛题目列表（无 limits） | `GET /api/get-contest-problem?cid=&containsEnd=` |
| 在线调试（用题目 limits） | `POST /api/submit-problem-test-judge` → `GET /api/get-test-judge-result?testJudgeKey=` |
| 实际耗时/内存 | `GET /api/get-submission-detail?submitId=` |
| 管理员查/改 limits | `GET /api/admin/problem?pid=`、`PUT /api/admin/problem`、`GET /api/admin/contest/problem?pid=` |
