# HOJ 比赛排行榜（Contest Rank）API 详解

> 本文档是 `HOJ-API-Documentation.md` 的**补充细化文档**，专门讲解「前端获取比赛排名」所涉及的全部接口。
> 已有文档的 `3.8 获取比赛排行榜` 只有一个概述，本文把它以及配套的关联接口完整展开，可直接用于 HOJ 客户端实现。
>
> 代码来源：
> - 前端：`hoj-vue/src/common/api.js`、`hoj-vue/src/views/oj/contest/children/*`、`hoj-vue/src/views/oj/contest/outside/*`
> - 后端：`hoj-springboot/DataBackup/src/main/java/top/hcode/hoj/controller/oj/ContestController.java`、`manager/oj/ContestManager.java`、`manager/oj/ContestRankManager.java`、`manager/oj/ContestCalculateRankManager.java`

---

## 目录

1. [接口总览](#1-接口总览)
2. [核心接口：POST /api/get-contest-rank（内榜）](#2-核心接口post-apiget-contest-rank内榜)
3. [榜单类型：ACM 与 OI](#3-榜单类型ac-与-oi)
4. [封榜（sealRank）与榜单缓存](#4-封榜sealrank与榜单缓存)
5. [关联接口](#5-关联接口)
6. [外榜（Outside Scoreboard）](#6-外榜outside-scoreboard)
7. [管理员接口](#7-管理员接口)
8. [前端完整调用流程（可直接照抄）](#8-前端完整调用流程可直接照抄)
9. [客户端实现注意事项](#9-客户端实现注意事项)

---

## 1. 接口总览

| 方法 | 路径 | 认证 | 用途 |
|------|------|------|------|
| `POST` | `/api/get-contest-rank` | 需要登录 | **核心**：获取比赛内榜排行榜（ACM / OI 自动区分） |
| `GET` | `/api/get-contest-info` | 需要登录 | 比赛详情（赛制、封榜、榜单显示名等，榜单的前置数据） |
| `GET` | `/api/get-contest-problem` | 匿名 | 比赛题目列表（榜单表格的列头：`displayId`、气球颜色） |
| `POST` | `/api/get-contest-outside-scoreboard` | 匿名 | 比赛**外榜**排名数据（外榜页面用，带 15s 缓存） |
| `GET` | `/api/get-contest-outsize-info` | 匿名 | 外榜所需的比赛信息 + 题目列表（**注意路径拼写是 `outsize`**） |
| `GET` | `/api/file/download-contest-rank` | 需要登录 | 导出排行榜 CSV 文件 |
| `GET` | `/api/contest-submissions` | 需要登录 | 点击榜单单元格后查看对应提交（详见主文档 3.7） |
| `GET` | `/api/get-contest-ac-info` | 需要登录（比赛管理员） | 列表形式查看各用户 AC 情况 |
| `PUT` | `/api/check-contest-ac-info` | 需要登录（比赛管理员） | 确认某条提交的 AC 情况 |

通用响应格式（与主文档一致）：

```json
{
  "status": 200,
  "msg": "success",
  "data": {}
}
```

分页响应统一为 MyBatis-Plus `IPage`：

```json
{
  "records": [],
  "total": 100,
  "size": 50,
  "current": 1,
  "pages": 2
}
```

---

## 2. 核心接口：POST /api/get-contest-rank（内榜）

```
POST /api/get-contest-rank
Authorization: {token}
Content-Type: application/json
```

- **认证要求**：`@RequiresAuthentication`，**必须携带有效 Token**，否则返回 401。
- **权限校验**：后端会执行 `validateContestAuth`：
  - 私有赛（`auth = 1`）必须已通过 `/api/register-contest` 注册，否则 403；
  - 比赛创建者 / 超级管理员可绕过。
- **返回内容**：当前登录用户（`currentUserId`）与「关注列表」中的用户的排名会被**复制一份放到列表最前面**（见第 9 节注意事项），因此 `total` 会包含这些前置的重复条目。

### 2.1 请求体 `ContestRankDTO`

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cid` | Long | **是** | 比赛 ID。为空报错 `错误：cid不能为空` |
| `currentPage` | Integer | 否 | 页码，从 1 开始；为空或 `<1` 时默认 `1` |
| `limit` | Integer | 否 | 每页大小；为空或 `<1` 时默认 `50` |
| `forceRefresh` | Boolean | 否 | 是否强制实时计算榜单（绕过缓存）；默认 `false`。**仅比赛创建者/超级管理员能生效**，其他人会被强制置为 `false` |
| `removeStar` | Boolean | 否 | 是否移除打星队伍（`*` 打星账号）；默认 `false` |
| `keyword` | String | 否 | 搜索关键词，匹配**学校**或**榜单显示名**（大小写不敏感，前后空格自动裁剪） |
| `containsEnd` | Boolean | 否 | 是否展示比赛结束后的提交结果；只有比赛 `allowEndSubmit = true` 时才真正生效 |
| `concernedList` | List\<String\> | 否 | 关注用户 uid 列表（**客户端本地维护**，见第 9 节） |
| `externalCidList` | List\<Integer\> | 否 | 需要合并显示到本榜单的**其它比赛 ID 列表**（多场联赛合并榜单用），普通场景传 `null` |

前端实际组装的请求体（`contestRankMixin.js`）：

```json
{
  "currentPage": 1,
  "limit": 50,
  "cid": 1,
  "forceRefresh": false,
  "removeStar": false,
  "concernedList": ["uuid-a", "uuid-b"],
  "keyword": null,
  "containsEnd": false
}
```

### 2.2 响应数据

- 分页 `IPage`，`records` 中每一行的结构**取决于比赛赛制**：
  - `contest.type == 0`（ACM）→ `List<ACMContestRankVO>`
  - `contest.type == 1`（OI）→ `List<OIContestRankVO>`

### 2.3 ACM 榜单记录 `ACMContestRankVO`

| 字段 | 类型 | 说明 |
|------|------|------|
| `rank` | Integer | 排名；**`-1` 表示打星队伍**（不参与排名） |
| `uid` | String | 用户 ID（UUID） |
| `username` | String | 用户名 |
| `realname` | String | 真实姓名 |
| `nickname` | String | 昵称 |
| `school` | String | 学校 |
| `gender` | String | 性别（`female` 时前端给用户单元格加背景色） |
| `avatar` | String | 头像 URL |
| `totalTime` | Long | 总罚时（秒），= Σ(`errorNum * 20 * 60` + `ACTime`) |
| `total` | Integer | 该用户的总提交数（比赛内所有题目） |
| `ac` | Integer | AC 的题目数 |
| `submissionInfo` | Object | **按题目聚合的明细**，key 为题目 `displayId`（如 `"A"`），value 见 2.5 |
| `isWinAward` | Boolean | 是否得奖（比赛配置了奖项时才存在） |
| `awardName` | String | 奖项名称（如「金牌」） |
| `awardBackground` | String | 奖项背景色 |
| `awardColor` | String | 奖项文字颜色 |

### 2.4 OI 榜单记录 `OIContestRankVO`

| 字段 | 类型 | 说明 |
|------|------|------|
| `rank` | Integer | 排名；`-1` 表示打星队伍 |
| `uid` / `username` / `realname` / `nickname` / `school` / `gender` / `avatar` | String | 同 ACM |
| `totalScore` | Integer | 总得分 |
| `totalTime` | Integer | 总耗时（**毫秒**），仅统计 AC（得分）提交的 `useTime` 之和 |
| `submissionInfo` | Object | key 为题目 `displayId`，value 为该题**得分**（Integer） |
| `timeInfo` | Object | key 为题目 `displayId`，value 为该题 AC 提交的**最优耗时（毫秒）**；没有 AC 的题不出现 |
| `isWinAward` / `awardName` / `awardBackground` / `awardColor` | - | 同 ACM |

> OI 榜单排序规则：先按 `totalScore` 降序，再按 `totalTime` 升序。
> 得分规则由比赛的 `oiRankScoreType` 决定：`Recent`（取最后一次提交）或 `Highest`（取最高分）。

### 2.5 ACM `submissionInfo` 明细字段

`submissionInfo` 的值是 `HashMap<String, Object>`，字段由后端 `ContestCalculateRankManager` 计算得出：

| 字段 | 类型 | 出现条件 | 说明 |
|------|------|----------|------|
| `errorNum` | Integer | 总是有 | 未通过次数（含罚时）；**注意：当该题 AC 时，前端会自行 +1 用于显示 `错误次数+1`** |
| `tryNum` | Integer | 封榜期间提交时 | 封榜时段内的提交次数（封榜后不显示题目通过状态，只显示尝试次数） |
| `isAC` | Boolean | AC 后 | 该题是否已通过（封榜时段内的提交不记录 `isAC`） |
| `isFirstAC` | Boolean | AC 后 | 是否是一血（相同提交时间也算一血） |
| `ACTime` | Long | AC 后 | AC 时的比赛进度（**秒**，相对 `startTime`） |
| `isAfterContest` | Boolean | 赛后提交且 `containsEnd=true` | 该 AC 是否发生在比赛结束后（前端显示时加 `*` 号） |

`errorNum` 之外，未被记录的题目不会出现在 `submissionInfo` 中（前端据此渲染空格子）。

### 2.6 请求 / 响应示例

**请求**

```
POST /api/get-contest-rank
Authorization: eyJhbGciOiJIUzI1NiJ9...
```

```json
{
  "cid": 1,
  "currentPage": 1,
  "limit": 50,
  "forceRefresh": false,
  "removeStar": false,
  "keyword": null,
  "containsEnd": false,
  "concernedList": []
}
```

**响应（ACM）**

```json
{
  "status": 200,
  "msg": "success",
  "data": {
    "records": [
      {
        "rank": 1,
        "uid": "a1b2c3d4-....",
        "username": "alice",
        "realname": "张三",
        "nickname": "Alice",
        "school": "XX大学",
        "gender": "female",
        "avatar": "https://.../avatar.png",
        "totalTime": 3720,
        "total": 5,
        "ac": 3,
        "submissionInfo": {
          "A": { "errorNum": 1, "isAC": true, "isFirstAC": true, "ACTime": 600 },
          "B": { "errorNum": 2 },
          "C": { "errorNum": 0, "isAC": true, "isFirstAC": false, "ACTime": 3120 },
          "D": { "errorNum": 0, "tryNum": 1 }
        }
      }
    ],
    "total": 120,
    "size": 50,
    "current": 1,
    "pages": 3
  }
}
```

**响应（OI）**

```json
{
  "status": 200,
  "msg": "success",
  "data": {
    "records": [
      {
        "rank": 1,
        "uid": "a1b2c3d4-....",
        "username": "bob",
        "realname": "李四",
        "nickname": null,
        "school": "YY大学",
        "gender": "male",
        "avatar": "https://.../avatar.png",
        "totalScore": 280,
        "totalTime": 3560,
        "submissionInfo": { "A": 100, "B": 80, "C": 100 },
        "timeInfo": { "A": 1200, "C": 2360 }
      }
    ],
    "total": 88,
    "size": 50,
    "current": 1,
    "pages": 2
  }
}
```

---

## 3. 榜单类型：ACM 与 OI

判断用哪个 VO 解析，有两种等价方式：

1. **来自比赛详情**（推荐）：`GET /api/get-contest-info` 的 `type` 字段，`0 = ACM`、`1 = OI`（对应前端 `RULE_TYPE`）。
2. **来自榜单本身**：记录中同时存在 `ac` 与 `total` 字段 → ACM；存在 `totalScore` 字段 → OI。

前端组件选择逻辑（`ContestRank.vue`）：

```js
this.contestRuleType === RULE_TYPE.ACM ? 'ACMContestRank' : 'OIContestRank'
```

比赛状态 `status`：`-1 = 未开始（Scheduled）`、`0 = 进行中（Running）`、`1 = 已结束（Ended）`。
比赛未开始时榜单通常为空（无参赛记录）；已结束的比赛前端会禁用「刷新」按钮（`refreshDisabled = status === 1`）。

---

## 4. 封榜（sealRank）与榜单缓存

**封榜逻辑（服务端）**

- 比赛开启封榜（`sealRank = true`）且当前时间处于 `[sealRankTime, endTime]` 区间时，对普通用户隐藏封榜时段内的通过状态：
  - 封榜时段内的提交只累加 `tryNum`，不写入 `isAC` / `ACTime` / `isFirstAC`；
  - `errorNum` 也保持不变。
- **比赛创建者 / 超级管理员**可以通过 `forceRefresh: true` 查看真实（不封榜）榜单；普通用户传 `forceRefresh` 无效。
- 判断当前请求是否处于封榜：`contestValidator.isSealRank(uid, contest, forceRefresh, isRoot)`。

**缓存差异**

| 接口 | 缓存行为 |
|------|----------|
| `POST /api/get-contest-rank`（内榜） | **每次都实时计算**（不走 Redis 排序结果缓存） |
| `POST /api/get-contest-outside-scoreboard`（外榜） | 当 `forceRefresh = false` 时使用 Redis 缓存，默认 **15 秒** |

**前端刷新策略**：比赛未结束时自动轮询（内榜 `ACMContestRank`/`OIContestRank` 每 **10 秒**，外榜每 **30 秒**），刷新时 `forceRefresh = false` 以命中缓存。

---

## 5. 关联接口

### 5.1 GET /api/get-contest-info（榜单前置数据）

```
GET /api/get-contest-info?cid={cid}
```

- **认证要求**：需要登录
- 关键字段（用于渲染榜单）：`type`（赛制）、`status`、`startTime`、`endTime`、`duration`、`sealRank`、`sealRankTime`、`openRank`（是否开启外榜）、`rankShowName`（榜单显示名规则：`username` / `realname` / `nickname`）、`oiRankScoreType`、`allowEndSubmit`、`now`（服务器当前时间）。

### 5.2 GET /api/get-contest-problem（榜单表格列头）

```
GET /api/get-contest-problem?cid={cid}&containsEnd={containsEnd}
```

- **认证要求**：匿名（但私有赛需要访问权限）
- 返回 `List<ContestProblemVO>`，榜单表格按 `displayId`（`A`、`B`、`C`…）顺序生成列，并用 `color` 渲染气球颜色。

### 5.3 GET /api/file/download-contest-rank（导出 CSV）

```
GET /api/file/download-contest-rank?cid={cid}&forceRefresh={forceRefresh}&removeStar={removeStar}&containEnd={containEnd}
```

- **认证要求**：需要登录
- `forceRefresh` 必填；`removeStar` 默认 `false`；`containEnd` 默认 `false`。
- 直接返回文件流（CSV），前端用 `window.open` / 文件下载方式处理。

### 5.4 GET /api/contest-submissions（点击单元格查看提交）

榜单单元格点击后会跳转到提交列表页，参数：

```
GET /api/contest-submissions?contestID={cid}&username={username}&problemID={displayId}&completeProblemID=true&status=0...
```

- 详见主文档 `3.7 获取比赛提交列表`。
- 注意参数名是 `contestID`（不是 `cid`），题目筛选参数是 `problemID`（值为 `displayId`）。

---

## 6. 外榜（Outside Scoreboard）

外榜用于**未登录/非参赛用户**查看公开榜单（例如大屏、分享页）。所有接口都是匿名可访问的（`ContestScoreboardController` 类级 `@AnonApi`），但服务端会校验：

- `contest.openRank === true`（比赛必须开启外榜），否则 403 `本场比赛未开启外榜，禁止访问外榜！`；
- 比赛不能处于「筹备中（未开始）」，否则 403 `本场比赛正在筹备中，禁止访问外榜！`。

### 6.1 GET /api/get-contest-outsize-info

```
GET /api/get-contest-outsize-info?cid={cid}
```

> ⚠️ 路径拼写就是 `outsize`（不是 `outside`），前端 `api.js` 中也是这么写的，客户端的 URL 不能「修正」。

- **认证要求**：无
- **响应**：`ContestOutsideInfoVO`

```json
{
  "contest": { "id": 1, "title": "...", "type": 0, "status": 0, "openRank": true, "startTime": "...", "endTime": "...", "now": "...", "sealRank": true },
  "problemList": [
    { "cid": 1, "displayId": "A", "pid": 1001, "displayTitle": "...", "color": "#FF0000" }
  ]
}
```

### 6.2 POST /api/get-contest-outside-scoreboard

```
POST /api/get-contest-outside-scoreboard
Content-Type: application/json
```

- **认证要求**：无（可携带 Token，服务端若识别到登录用户会推断其 uid，以便前置显示「我」的排名）
- **请求体**：与 `ContestRankDTO` 完全一致（`cid`、`currentPage`、`limit`、`forceRefresh`、`removeStar`、`keyword`、`containsEnd`、`concernedList`、`externalCidList`）
- **响应**：分页 `IPage`，`records` 结构与内榜一致（ACM/OI 对应 VO）
- **差异**：
  - `forceRefresh` 对非比赛管理者无效（会被置为 `false`）；
  - 默认启用 **15 秒缓存**；
  - `externalCidList` 可传入其它比赛 ID 合并榜单。

```json
{
  "cid": 1,
  "currentPage": 1,
  "limit": 50,
  "forceRefresh": false,
  "removeStar": true,
  "keyword": null,
  "containsEnd": false,
  "concernedList": []
}
```

---

## 7. 管理员接口

### 7.1 GET /api/get-contest-ac-info

```
GET /api/get-contest-ac-info?cid={cid}&currentPage={currentPage}&limit={limit}
```

- **认证要求**：需要登录，且必须是比赛管理者
- **响应**：`IPage<ContestRecord>`，以「提交记录」为单位分页展示 AC 情况（用于管理员人工核对）。

### 7.2 PUT /api/check-contest-ac-info

```
PUT /api/check-contest-ac-info
Content-Type: application/json
```

- **认证要求**：需要登录，且必须是比赛管理者
- **请求体**：`CheckACDTO`

```json
{
  "id": 12345,     // 比赛记录 id（contest_record 主键）
  "cid": 1,        // 比赛 id
  "checked": true  // 是否确认该条 AC
}
```

- **响应**：`Void`（`status = 200` 表示成功）

---

## 8. 前端完整调用流程（可直接照抄）

以比赛内榜（`ACMContestRank.vue` + `contestRankMixin.js`）为例：

```
1. 进入比赛详情页 → GET /api/get-contest-info?cid=1        // 初始化 contest（type / status / sealRank / rankShowName）
2. 获取题目列头   → GET /api/get-contest-problem?cid=1      // 生成 A、B、C… 列
3. 获取榜单       → POST /api/get-contest-rank               // body: cid, currentPage, limit, forceRefresh, removeStar, keyword, containsEnd, concernedList
4. total = res.data.data.total
5. page === 1 时用 records 渲染折线图（前 10 名 AC 时间轴）
6. 用 records 渲染表格
7. status !== 1（比赛未结束）→ 每 10s 轮询第 3 步
```

**客户端需要自行补充计算**（后端不返回，前端在 `applyToTable` 中处理）：

| 计算字段 | 规则 |
|----------|------|
| `rankShowName` | 取 `contest.rankShowName` 对应字段（`username`/`realname`/`nickname`），为空则回退 `username` |
| `specificTime` | ACM：`ACTime` 秒 → `HH:MM:SS` 格式（仅展示用） |
| ACM 单元格显示 | `isAC` → 显示 `ACTime`（分钟，`isAfterContest` 时前面加 `*`）；`tryNum != null`（封榜期间）→ 显示 `{errorNum}+{tryNum} tries`；否则显示 `{errorNum} try/tries`。注意：AC 行的 `errorNum` 前端已 `+1`（把本次 AC 计入），所以显示值比原始响应大 1 |
| `isAfterContest` | 为 `true` 时在 `ACTime` 前加 `*` |
| `isConcerned` | `concernedList.includes(row.uid)` |
| `cellClassName` | `isFirstAC` → `first-ac`；`isAC` → `ac`；`isAfterContest && isAC` → `after-ac`；`tryNum > 0` → `try`；`errorNum != 0` → `wa`（OI：`oi-100` / `oi-between` / `oi-0`） |
| `userCellClassName` | `rank == -1` → `bg-star`；`gender == 'female'` → `bg-female` |

**关注列表（concernedList）存储**：纯客户端 `localStorage` 行为，key 为
```
CONTEST_RANK_CONCERNED_{contestID}
```
（对应前端 `buildContestRankConcernedKey(contestID)`）。更新关注后需要**重新请求榜单**（`forceRefresh: true` 触发刷新）。

---

## 9. 客户端实现注意事项

1. **必须登录才能看内榜**：`POST /api/get-contest-rank` 是 `@RequiresAuthentication`，客户端需始终带 `Authorization` 头；建议对 401 做统一跳转登录处理。
2. **`total` 包含前置条目**：服务端会把「当前登录用户」和「关注列表用户」复制一份插到 `records` 最前面，因此 `total` 会略大于真实人数，且第 1 页可能出现重复行。客户端去重时建议按 `uid` 去重，不要用 `total` 推算真实人数（需要真实人数时以最后一页的 `rank` 最大值为准，或忽略打星 `-1`）。
3. **`forceRefresh` 不是万能的**：非比赛创建者/超管传 `true` 会被服务端忽略，仍返回封榜后的榜单。客户端不要依赖它判断封榜状态，应以 `contest.sealRank` + `contest.sealRankTime` 自行判断。
4. **`keyword` 只匹配学校/榜单显示名**，不匹配用户名（除非 `rankShowName` 为 `username`）。搜索时服务端会重新在**全量排名**上过滤再分页，因此 `total` 会随关键词变化。
5. **`containsEnd` 仅在 `allowEndSubmit = true` 时生效**；否则传了也不会显示赛后提交。
6. **`externalCidList` 用于联赛合并榜单**：普通单场比赛保持 `null` 即可，传错会把别的比赛记录混进来。
7. **外榜接口无需 Token**，适合做「未登录可浏览的公开榜单」，但要注意 `openRank = false` 或比赛未开始时会被 403 拒绝。
8. **路径拼写陷阱**：外榜信息接口是 `get-contest-outsize-info`（少一个 `d`），提交列表参数名是 `contestID` 而非 `cid`，复制主文档参数名时务必核对。
9. **性能建议**：榜单为全量计算后分页，`limit` 越大单次耗时越长；客户端列表建议 `limit = 50`，轮询间隔 `≥10s`，并在页面切到后台时暂停轮询。
10. **大屏滚动榜**：前端 `ScrollBoard.vue` 通过 `POST /api/get-contest-rank` 仅取 `limit: 10` 拿到 `total`，用 `total` 的 10%/20%/30% 计算金/银/铜牌人数，再打开独立的 `hoj-scrollBoard` 页面展示；客户端若需复刻，可直接复用此思路。

---

## 附：字段速查

**ACM（`ACMContestRankVO`）**：`rank, isWinAward, awardName, awardBackground, awardColor, uid, username, realname, nickname, school, gender, avatar, totalTime(秒), total, ac, submissionInfo{displayId: {errorNum, tryNum, isAC, isFirstAC, ACTime(秒), isAfterContest}}`

**OI（`OIContestRankVO`）**：`rank, isWinAward, awardName, awardBackground, awardColor, uid, username, realname, nickname, gender, avatar, school, totalScore, totalTime(毫秒), submissionInfo{displayId: 得分}, timeInfo{displayId: 毫秒}`

**请求 DTO（`ContestRankDTO`）**：`cid, limit, currentPage, forceRefresh, removeStar, keyword, containsEnd, concernedList, externalCidList`
