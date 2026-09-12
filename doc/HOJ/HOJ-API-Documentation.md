# HOJ (Hydro Online Judge) API 参考文档

> 基于 HOJ 后端 `DataBackup` 与前端 `hoj-vue` 提取的关键 API 记录。
>
> 本文档服务于 Hinina 项目 —— 为 HOJ Adapter（`adapter/hoj`）实现四个组合 trait 提供精确的 API 参考。

---

## 目录

1. [通用约定](#1-通用约定)
2. [认证与登录（AuthProvider）](#2-认证与登录)
3. [比赛（ContestProvider）](#3-比赛)
4. [题目（ProblemProvider）](#4-题目)
5. [提交与评测（SubmissionProvider）](#5-提交与评测)
6. [账户信息](#6-账户信息)
7. [评测状态码](#7-评测状态码)
8. [附录：其他 API](#8-附录其他-api)

---

## 1. 通用约定

### 1.1 基础地址

```
{baseUrl}/api
```

所有接口均以 `/api` 为前缀。

### 1.2 统一响应格式

```json
{
  "status": 200,       // HTTP 状态码，200 表示成功
  "msg": "success",    // 消息
  "data": {}           // 业务数据，类型因接口不同而异
}
```

- 成功：`status === 200`
- 401：Token 过期或未登录
- 403：无权限

### 1.3 认证机制

- **认证方式**：JWT（JSON Web Token）
- **Token 传递**：HTTP 请求头 `Authorization: {token}`
- **Token 获取**：登录成功后从响应 `data` 字段返回（`UserInfoVO`），同时 HTTP 响应头 `authorization` 包含 token。
- **Token 续约**：服务端在响应头 `refresh-token` 中指示前端更新 token。前端从响应头 `authorization` 获取新 token 并覆盖本地存储。
- **存储方式**：前端将 token 存储在 `localStorage` 中。
- **登出机制**：调用 logout 接口后，服务端清除 Redis 中缓存的 JWT。
- **安全**：`/api/login` 无需 Authorization 头；密码明文提交（需 HTTPS）。

### 1.4 分页格式

分页查询统一使用 MyBatis-Plus `IPage` / `Page` 格式：

```json
{
  "records": [],        // 当前页数据列表
  "total": 100,         // 总记录数
  "size": 20,           // 每页大小
  "current": 1,         // 当前页码
  "pages": 5            // 总页数
}
```

---

## 2. 认证与登录

> 对应 trait：`AuthProvider`

### 2.1 登录

```
POST /api/login
```

- **认证要求**：无（`@AnonApi`）
- **请求体**（JSON）：

```json
{
  "username": "string",  // 必填，用户名
  "password": "string"   // 必填，密码（明文）
}
```

- **响应数据**：`UserInfoVO`

```json
{
  "uid": "string",          // 用户ID（UUID）
  "username": "string",     // 用户名
  "nickname": "string",     // 昵称
  "avatar": "string",       // 头像URL
  "titleName": "string",    // 头衔名称
  "titleColor": "string",   // 头衔背景颜色
  "email": "string",        // 邮箱
  "number": "string",       // 学号
  "gender": "string",       // 性别
  "school": "string",       // 学校
  "course": "string",       // 专业
  "signature": "string",    // 个性签名
  "realname": "string",     // 真实姓名
  "github": "string",       // GitHub地址
  "blog": "string",         // 博客地址
  "cfUsername": "string",   // Codeforces用户名
  "roleList": ["string"]    // 角色列表
}
```

- **注意**：token 通过 HTTP 响应头 `authorization` 返回。
- **重要**：登录接口 **不需要验证码**。密码为**明文**传递，服务端自行对收到的密码做 MD5（非加盐）后与数据库中的 MD5 比对（见 `PassportManager.login` 的 `SecureUtil.md5(loginDto.getPassword())`）。客户端**不要**自行 MD5。防暴力破解策略为：同一 IP + 同一用户名 30 分钟内最多尝试 20 次，超出后锁定 30 分钟。

### 2.2 登出

```
GET /api/logout
```

- **认证要求**：需要认证（`@RequiresAuthentication`）
- **请求参数**：无
- **响应数据**：无（`Void`）

### 2.3 注册

```
POST /api/register
```

- **认证要求**：无（`@AnonApi`）
- **请求体**：`RegisterDTO`（包含 username, password, email, code 等）
- **响应数据**：无

> **MVP 备注**：Hinina MVP 不需要注册功能，由目标 OJ 服务端提供。此接口仅供参考。
### 2.5 获取注册邮箱验证码

```
GET /api/get-register-code?email={email}
```

- **认证要求**：无
- **响应数据**：`RegisterCodeVO`
- **说明**：发送注册验证码到邮箱。MVP 阶段不需要。（算术验证码）

```
GET /api/captcha
```

- **认证要求**：无
- **响应数据**：`CaptchaVO`

```json
{
  "img": "data:image/png;base64,...",  // 算术题图片 Base64（90×30px，2位数运算，如 "3+5=?"）
  "captchaKey": "uuid-string"           // 凭证 key，存入 Redis，有效期 30 分钟
}
```

- **说明**：HOJ 使用算术验证码（`ArithmeticCaptcha`），而非传统扭曲字符验证码。由 `CommonController` 提供。
- **使用场景**：仅在**重置密码**（`POST /api/apply-reset-password`）时需要，传入 `captcha`（用户输入的答案，小写）+ `captchaKey`。**登录不需要验证码。**

---

## 3. 比赛

> 对应 trait：`ContestProvider`

### 3.1 获取比赛列表

```
GET /api/get-contest-list
```

- **认证要求**：无（`@AnonApi`）
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `limit` | Integer | 否 | 每页数量 |
| `currentPage` | Integer | 否 | 当前页码（从1开始） |
| `status` | Integer | 否 | 比赛状态：-1=未开始，0=进行中，1=已结束 |
| `type` | Integer | 否 | 赛制：0=ACM，1=OI |
| `keyword` | String | 否 | 搜索关键词（匹配标题） |

- **响应数据**：分页的 `ContestVO` 列表

```json
{
  "records": [{
    "id": 1,                    // 比赛ID
    "author": "string",         // 创建者用户名
    "title": "string",          // 比赛标题
    "type": 0,                  // 赛制：0=ACM，1=OI
    "description": "string",    // 比赛说明（Markdown/HTML）
    "status": 0,                // -1=未开始，0=进行中，1=已结束
    "source": 0,                // 来源：0=原创，其他=克隆赛ID
    "auth": 0,                  // 权限：0=公开，1=私有（需密码），2=保护
    "now": "2024-01-01T00:00:00",  // 服务器当前时间
    "startTime": "2024-01-01T08:00:00",
    "endTime": "2024-01-01T12:00:00",
    "duration": 14400,          // 比赛时长（秒）
    "sealRank": false,          // 是否封榜
    "openPrint": false,         // 是否开放打印
    "sealRankTime": null,       // 封榜起始时间
    "rankShowName": "username", // 排行榜显示方式
    "openRank": true,           // 是否开放排行榜
    "oiRankScoreType": "Recent",// OI排行榜得分方式：Recent/Highest
    "count": 100,               // 报名人数
    "gid": null,                // 团队ID
    "allowEndSubmit": false     // 是否允许结束后交题
  }],
  "total": 10
}
```

### 3.2 获取比赛详情

```
GET /api/get-contest-info?cid={cid}
```

- **认证要求**：需要认证
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cid` | Long | 是 | 比赛ID |

- **响应数据**：`ContestVO`（结构同 3.1）

### 3.3 注册比赛（私有赛）

```
POST /api/register-contest
```

- **认证要求**：需要认证
- **请求体**：

```json
{
  "cid": 1,             // 比赛ID
  "password": "string"  // 比赛密码
}
```

### 3.4 获取比赛访问权限

```
GET /api/get-contest-access?cid={cid}
```

- **认证要求**：需要认证
- **响应数据**：`AccessVO`（包含 `access` 布尔字段）

### 3.5 获取比赛题目列表

```
GET /api/get-contest-problem?cid={cid}&containsEnd={containsEnd}
```

- **认证要求**：无（但需要比赛访问权）
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cid` | Long | 是 | 比赛ID |
| `containsEnd` | Boolean | 否 | 默认 false，是否包含已结束比赛的题目 |

- **响应数据**：`List<ContestProblemVO>`

```json
[{
  "id": 1,                    // 内部ID
  "displayId": "A",           // 比赛中题目序号（如A、B、C）
  "cid": 1,                   // 比赛ID
  "pid": 1001,                // 题目真实ID
  "displayTitle": "string",   // 比赛中显示标题（可能覆盖原题名）
  "color": "#FF0000",         // 气球颜色
  "ac": 50,                   // AC 数
  "total": 120                // 总提交数
}]
```

### 3.6 获取比赛题目详情

```
GET /api/get-contest-problem-details?cid={cid}&displayId={displayId}&containsEnd={containsEnd}
```

- **认证要求**：需要认证
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cid` | Long | 是 | 比赛ID |
| `displayId` | String | 是 | 比赛中题目序号（如 "A"） |
| `containsEnd` | Boolean | 否 | 默认 false |

- **响应数据**：`ProblemInfoVO`（结构同 4.2 题目详情）

### 3.7 获取比赛提交列表

```
GET /api/contest-submissions
```

- **认证要求**：需要认证
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `contestID` | Long | 是 | 比赛ID |
| `limit` | Integer | 否 | 每页数量 |
| `currentPage` | Integer | 否 | 页码 |
| `onlyMine` | Boolean | 否 | 只看自己的提交 |
| `problemID` | String | 否 | 题目展示ID筛选 |
| `status` | Integer | 否 | 评测状态筛选 |
| `username` | String | 否 | 用户名筛选 |
| `beforeContestSubmit` | Boolean | 是 | 是否包含赛前提交 |
| `completeProblemID` | Boolean | 否 | 默认 false |

- **响应数据**：分页的 `JudgeVO` 列表（结构同 5.3）

### 3.8 获取比赛排行榜

```
POST /api/get-contest-rank
```

- **认证要求**：需要认证
- **请求体**：`ContestRankDTO`（包含 cid, limit, currentPage 等）
- **响应数据**：分页排名数据

### 3.9 获取比赛公告

```
GET /api/get-contest-announcement?cid={cid}&limit={limit}&currentPage={currentPage}
```

- **认证要求**：需要认证
- **响应数据**：分页的 `AnnouncementVO` 列表

---

## 4. 题目

> 对应 trait：`ProblemProvider`

### 4.1 获取题目列表

```
GET /api/get-problem-list
```

- **认证要求**：无（`@AnonApi`）
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `limit` | Integer | 否 | 每页数量 |
| `currentPage` | Integer | 否 | 页码 |
| `keyword` | String | 否 | 搜索关键词 |
| `tagId` | List\<Long\> | 否 | 标签ID数组（可多选） |
| `difficulty` | Integer | 否 | 难度筛选 |
| `oj` | String | 否 | OJ来源筛选 |

- **响应数据**：分页的 `ProblemVO` 列表

```json
{
  "records": [{
    "pid": 1001,               // 题目ID
    "problemId": "HOJ-1001",   // 题目展示ID
    "title": "A + B Problem",  // 题目标题
    "difficulty": 1,           // 难度等级
    "type": 0,                 // 题目类型
    "tags": [{
      "id": 1,
      "name": "数学",
      "color": "#409EFF"
    }],
    "total": 500,              // 总提交数
    "ac": 300,                 // AC数
    "mle": 10,                 // MLE数
    "tle": 50,                 // TLE数
    "re": 20,                  // RE数
    "pe": 5,                   // PE数
    "ce": 15,                  // CE数
    "wa": 100,                 // WA数
    "se": 0,                   // SE数
    "pa": 0,                   // Partial AC数（IO题目）
    "score": 100               // IO题目总分
  }],
  "total": 100
}
```

### 4.2 获取题目详情

```
GET /api/get-problem-detail?problemId={problemId}&gid={gid}
```

- **认证要求**：无（`@AnonApi`）—— 只能查询公开题目（auth=1）
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `problemId` | String | 是 | 题目展示ID（如 "HOJ-1001"） |
| `gid` | Long | 否 | 团队ID（团队题目时传入） |

- **响应数据**：`ProblemInfoVO`

```json
{
  "problem": {
    "id": 1001,                          // 题目ID
    "problemId": "HOJ-1001",            // 展示ID
    "title": "A + B Problem",           // 标题
    "author": "admin",                   // 作者
    "type": 0,                           // 类型
    "timeLimit": 1000,                   // 时间限制（ms）
    "memoryLimit": 256,                  // 内存限制（MB）
    "stackLimit": 128,                   // 栈限制（MB）
    "description": "题目描述HTML/Markdown",
    "input": "输入描述HTML/Markdown",
    "output": "输出描述HTML/Markdown",
    "examples": "样例（HTML，含输入输出）",
    "hint": "提示HTML/Markdown",
    "source": "题目来源",
    "auth": 1,                           // 1=公开
    "ioScore": 100,                      // IO题目总分
    "difficulty": 1,                     // 难度
    "judgeMode": "default",              // 评测模式：default/spj/interactive
    "judgeCaseMode": "default",          // 测试点模式：default/subtask_lowest/subtask_average
    "spjCode": null,
    "spjLanguage": null,
    "isRemoveEndBlank": true,
    "openCaseResult": true,
    "isUploadCase": true,
    "gid": null
  },
  "tags": [{ "id": 1, "name": "数学", "color": "#409EFF" }],
  "languages": ["C", "C++", "Java", "Python"],
  "problemCount": {
    "total": 500,
    "ac": 300,
    "wa": 100
  },
  "codeTemplate": {
    "C": "#include <stdio.h>\nint main() {\n  return 0;\n}",
    "C++": "#include <iostream>\nusing namespace std;\nint main() {\n  return 0;\n}"
  }
}
```

### 4.3 获取用户题目状态

```
POST /api/get-user-problem-status
```

- **认证要求**：需要认证
- **请求体**：

```json
{
  "pidList": ["HOJ-1001", "HOJ-1002"],
  "isContestProblemList": false,
  "cid": 0,
  "gid": null,
  "containsEnd": false
}
```

- **响应数据**：`HashMap<Long, Object>` — key 为 pid，value 为状态对象（0=未提交，1=已AC，2=尝试过）

### 4.4 获取所有题目标签

```
GET /api/get-all-problem-tags?oj={oj}
```

- **认证要求**：无
- **响应数据**：标签列表

### 4.5 随机获取一道题目

```
GET /api/get-random-problem
```

- **认证要求**：无
- **响应数据**：`RandomProblemVO`（包含 problemId）

### 4.6 获取代码模板

```
GET /api/get-problem-code-template?pid={pid}
```

- **认证要求**：需要认证
- **响应数据**：`HashMap<String, String>` — key=语言，value=模板代码

### 4.7 获取最近AC代码

```
GET /api/get-last-ac-code?pid={pid}&cid={cid}
```

- **认证要求**：需要认证
- **响应数据**：`LastAcceptedCodeVO`

---

## 5. 提交与评测

> 对应 trait：`SubmissionProvider`

### 5.1 提交代码

```
POST /api/submit-problem-judge
```

- **认证要求**：需要认证 + 权限 `submit`
- **请求体**：

```json
{
  "pid": "HOJ-1001",     // 必填，题目展示ID
  "language": "C++",     // 必填，编程语言
  "code": "#include...", // 必填，代码内容
  "cid": 0,              // 必填，比赛ID（非比赛提交填 0）
  "tid": null,           // 训练ID
  "gid": null,           // 团队ID
  "isRemote": false      // 是否远程评测
}
```

- **响应数据**：`Judge` 对象（提交记录，核心字段如下）

```json
{
  "submitId": 12345,           // 提交ID
  "pid": 1001,                 // 题目ID
  "displayPid": "HOJ-1001",   // 题目展示ID
  "uid": "uuid-string",       // 用户ID
  "username": "user1",         // 用户名
  "status": 0,                 // 初始状态：0=Pending
  "submitTime": "2024-01-01T08:00:00",
  "share": false,
  "length": 256,
  "language": "C++",
  "cid": 0,
  "cpid": 0,
  "gid": null,
  "judger": null,
  "ip": "127.0.0.1"
}
```

### 5.2 获取提交详情

```
GET /api/get-submission-detail?submitId={submitId}
```

- **认证要求**：无（`@AnonApi`）
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `submitId` | Long | 是 | 提交ID |

- **响应数据**：`SubmissionInfoVO`

```json
{
  "submission": {
    "submitId": 12345,
    "pid": 1001,
    "displayPid": "HOJ-1001",
    "uid": "uuid",
    "username": "user1",
    "submitTime": "2024-01-01T08:00:00",
    "status": 5,                  // 评测状态（见第7节状态码）
    "share": false,
    "errorMessage": null,          // 错误信息（CE时存编译错误）
    "time": 15,                    // 运行时间（ms）
    "memory": 10240,               // 运行内存（KB）
    "score": null,                 // IO题目得分
    "length": 256,                 // 代码长度
    "code": "#include...",         // 源代码
    "language": "C++",
    "cid": 0,
    "cpid": 0,
    "gid": null,
    "judger": "judge-server-1",
    "ip": "127.0.0.1",
    "oiRankScore": null,
    "vjudgeSubmitId": null,
    "vjudgeUsername": null,
    "vjudgePassword": null,
    "isManual": false
  },
  "codeShare": true
}
```

### 5.3 获取提交列表

```
GET /api/get-submission-list
```

- **认证要求**：无（`@AnonApi`）
- **请求参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `limit` | Integer | 否 | 每页数量 |
| `currentPage` | Integer | 否 | 页码 |
| `onlyMine` | Boolean | 否 | 只看自己的提交 |
| `problemID` | String | 否 | 题目展示ID筛选 |
| `status` | Integer | 否 | 评测状态筛选 |
| `username` | String | 否 | 用户名筛选 |
| `completeProblemID` | Boolean | 否 | 默认 false |
| `gid` | Long | 否 | 团队ID |

- **响应数据**：分页的 `JudgeVO` 列表

```json
{
  "records": [{
    "uid": "uuid",
    "submitId": 12345,
    "username": "user1",
    "pid": 1001,
    "displayPid": "HOJ-1001",
    "title": "A + B Problem",
    "displayId": null,
    "submitTime": "2024-01-01T08:00:00",
    "status": 5,
    "share": false,
    "time": 15,
    "memory": 10240,
    "score": null,
    "oiRankScore": null,
    "length": 256,
    "language": "C++",
    "cid": 0,
    "cpid": 0,
    "source": null,
    "judger": "judge-server-1",
    "ip": "127.0.0.1",
    "isManual": false
  }]
}
```

### 5.4 轮询提交状态 — 批量

```
POST /api/check-submissions-status
```

- **认证要求**：无（`@AnonApi`）
- **请求体**：

```json
{
  "submitIds": [12345, 12346, 12347],
  "cid": 0
}
```

- **响应数据**：`HashMap<Long, Object>` — key 为 submitId，value 为 `JudgeVO` 对象

> **设计要点**：此接口用于批量轮询 Pending/Judging 状态的提交，避免循环单次查询。

### 5.5 轮询提交状态 — 比赛专用

```
POST /api/check-contest-submissions-status
```

- **认证要求**：需要认证
- **请求体**：同 5.4
- **说明**：会检查封榜状态，封榜后可能不返回详细评测结果

### 5.6 获取测试点详情

```
GET /api/get-all-case-result?submitId={submitId}
```

- **认证要求**：无（`@AnonApi`）
- **响应数据**：`JudgeCaseVO`

```json
{
  "judgeCaseList": [{
    "submitId": 12345,
    "caseId": 1,
    "status": 0,              // 0=AC
    "time": 10,               // 运行时间（ms）
    "memory": 5120,           // 运行内存（KB）
    "score": null,            // IO题目该测试点得分
    "groupNum": null,
    "seq": 1,
    "mode": "default"
  }],
  "subTaskJudgeCaseVoList": null,
  "judgeCaseMode": "default"
}
```

### 5.7 在线调试

```
POST /api/submit-problem-test-judge
```

- **认证要求**：需要认证 + 权限 `submit`
- **请求体**：`TestJudgeDTO`（pid, language, code, userInput, 等）
- **响应数据**：`String`（testJudgeKey，用于后续查询结果）

```
GET /api/get-test-judge-result?testJudgeKey={testJudgeKey}
```

- **响应数据**：`TestJudgeVO`（包含输出、状态等）

### 5.8 重新提交

```
GET /api/resubmit?submitId={submitId}
```

- **认证要求**：需要认证
- **说明**：当远程判题提交失败超过 60 秒后，重试提交

### 5.9 更新提交分享权限

```
PUT /api/submission
```

- **认证要求**：需要认证
- **请求体**：`Judge` 对象（仅 share 字段有效）

---

## 6. 账户信息

### 6.1 获取用户主页信息

```
GET /api/get-user-home-info?uid={uid}&username={username}
```

- **认证要求**：不强制
- **响应数据**：`UserHomeVO`（包含 AC 题目列表、提交统计、Rating 分数等）

### 6.2 获取用户权限信息

```
GET /api/get-user-auth-info
```

- **认证要求**：需要认证
- **响应数据**：`UserAuthInfoVO`（包含 role, permissions）

### 6.3 修改用户信息

```
POST /api/change-userInfo
```

- **认证要求**：需要认证
- **请求体**：`UserInfoVO`

### 6.4 修改密码

```
POST /api/change-password
```

- **认证要求**：需要认证
- **请求体**：`ChangePasswordDTO`

### 6.5 获取编程语言列表

```
GET /api/languages?all={all}
```

- **认证要求**：无
- **说明**：获取 OJ 支持的所有编程语言

---

## 7. 评测状态码

| 状态码 | 含义 | 缩写 |
|--------|------|------|
| 0 | Pending（等待中） | Pending |
| 1 | Judging（评判中） | Judging |
| 2 | Compile Error（编译错误） | CE |
| 3 | Presentation Error（格式错误） | PE |
| 4 | Wrong Answer（答案错误） | WA |
| 5 | Accepted（通过） | AC |
| 6 | Time Limit Exceeded（时间超限） | TLE |
| 7 | Memory Limit Exceeded（内存超限） | MLE |
| 8 | Output Limit Exceeded（输出超限） | OLE |
| 9 | Runtime Error（运行错误） | RE |
| 10 | System Error（系统错误） | SE |
| 11 | Remote Judge Error（远程评测错误） | RJE |
| 12 | Submitted Failed（提交失败） | SF |
| 13 | Partially Accepted（部分通过，IO 题目） | PA |
| 14 | Submit Frequent Limit Exceeded（提交过于频繁） | FREQ |
| 15 | Unknown Error（未知错误） | UE |

**分类：**
- **终态**：CE(2), PE(3), WA(4), AC(5), TLE(6), MLE(7), OLE(8), RE(9), SE(10), RJE(11), SF(12), PA(13), FREQ(14), UE(15)
- **非终态（需轮询）**：Pending(0), Judging(1)

> **Hinina 实现要点**：提交后拿到 submitId，批量查询 `check-submissions-status` 直到非 0/1 状态。轮询建议间隔 1-2 秒。

---

## 8. 附录：其他 API

> 以下 API 不在 MVP 核心范围内，但为完整参考列出。

### 8.1 首页相关

| 接口 | 方法 | 说明 |
|------|------|------|
| `/api/get-website-config` | GET | 获取网站配置 |
| `/api/home-carousel` | GET | 首页轮播图 |
| `/api/get-recent-contest` | GET | 最近比赛 |
| `/api/get-recent-other-contest` | GET | 最近其他OJ比赛 |
| `/api/get-common-announcement` | GET | 公共公告（分页） |
| `/api/get-recent-seven-ac-rank` | GET | 近7天AC排行 |
| `/api/get-recent-updated-problem` | GET | 最近更新题目 |
| `/api/get-last-week-submission-statistics` | GET | 上周提交统计 |
| `/api/get-rank-list` | GET | ACM/OI排行榜 |

### 8.2 训练模块

| 接口 | 方法 | 说明 |
|------|------|------|
| `/api/get-training-category` | GET | 训练分类 |
| `/api/get-training-list` | GET | 训练列表（分页） |
| `/api/get-training-detail` | GET | 训练详情 |
| `/api/register-training` | POST | 注册训练 |
| `/api/get-training-access` | GET | 训练访问权限 |
| `/api/get-training-problem-list` | GET | 训练题目列表 |
| `/api/get-training-problem-details` | GET | 训练题目详情 |
| `/api/get-training-rank` | GET | 训练排行 |

### 8.3 讨论模块

| 接口 | 方法 | 说明 |
|------|------|------|
| `/api/discussion-category` | GET | 讨论分类 |
| `/api/get-discussion-list` | GET | 讨论列表（分页） |
| `/api/get-discussion-detail` | GET | 讨论详情 |
| `/api/discussion` | POST/PUT/DELETE | 创建/修改/删除讨论 |
| `/api/discussion-like` | GET | 点赞/点踩 |
| `/api/discussion-report` | POST | 举报讨论 |
| `/api/comments` | GET | 评论列表 |
| `/api/comment` | POST/DELETE | 创建/删除评论 |
| `/api/comment-like` | GET | 评论点赞 |
| `/api/reply` | GET/POST/DELETE | 回复相关 |

### 8.4 团队（Group）模块

| 接口 | 方法 | 说明 |
|------|------|------|
| `/api/get-group-list` | GET | 团队列表 |
| `/api/get-group-detail` | GET | 团队详情 |
| `/api/group` | POST/PUT/DELETE | 创建/修改/删除团队 |
| `/api/group/get-member-list` | GET | 团队成员列表 |
| `/api/group/member` | POST/PUT/DELETE | 成员管理 |
| `/api/group/get-announcement-list` | GET | 团队公告 |
| `/api/group/get-problem-list` | GET | 团队题目列表 |

### 8.5 管理后台

所有管理接口以 `/api/admin/` 为前缀，包括题目管理、比赛管理、用户管理、公告管理、评测管理等，不在本 MVP 文档范围内。

---

## 附录 A: Hinina HOJ Adapter 实现映射

| Hinina Trait | HOJ API | 对应的 Controller |
|---|---|---|
| `AuthProvider.login(username, password)` | `POST /api/login` | PassportController |
| `AuthProvider.logout()` | `GET /api/logout` | PassportController |
| `ContestProvider.get_contest_list(...)` | `GET /api/get-contest-list` | ContestController |
| `ContestProvider.get_contest_info(cid)` | `GET /api/get-contest-info` | ContestController |
| `ContestProvider.get_contest_problems(cid)` | `GET /api/get-contest-problem` | ContestController |
| `ContestProvider.get_contest_ranking(cid)` | `POST /api/get-contest-rank` | ContestController |
| `ProblemProvider.get_problem_list(...)` | `GET /api/get-problem-list` | ProblemController |
| `ProblemProvider.get_problem_detail(problemId)` | `GET /api/get-problem-detail` | ProblemController |
| `ProblemProvider.get_problem_tags()` | `GET /api/get-all-problem-tags` | ProblemController |
| `SubmissionProvider.submit(...)` | `POST /api/submit-problem-judge` | JudgeController |
| `SubmissionProvider.get_submission(submitId)` | `GET /api/get-submission-detail` | JudgeController |
| `SubmissionProvider.get_submission_list(...)` | `GET /api/get-submission-list` | JudgeController |
| `SubmissionProvider.check_status(submitIds)` | `POST /api/check-submissions-status` | JudgeController |
| `SubmissionProvider.check_contest_status(submitIds, cid)` | `POST /api/check-contest-submissions-status` | JudgeController |
| `SubmissionProvider.get_case_result(submitId)` | `GET /api/get-all-case-result` | JudgeController |

---

> **文档版本**：v1.0
> **生成日期**：2024
> **来源**：HOJ-master 项目源码（`DataBackup` 模块 + `hoj-vue` 前端）
