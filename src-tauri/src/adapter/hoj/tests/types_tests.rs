// HOJ types.rs 单元测试：map_status / is_terminal_status / normalize_error_message
//
// 码表来源：HOJ `Constants.Judge`（JudgeServer/.../util/Constants.java）。
// 注意取值域含负数 —— 曾因把它当成「0 起顺排」而导致所有 AC 显示为 Pending。

use super::*;

// ── map_status ──

#[test]
fn map_status_not_submitted() {
    assert!(matches!(
        map_status(-10),
        crate::core::entity::submission::JudgementStatus::NotSubmitted
    ));
}

#[test]
fn map_status_cancelled() {
    assert!(matches!(
        map_status(-4),
        crate::core::entity::submission::JudgementStatus::Cancelled
    ));
}

#[test]
fn map_status_ce() {
    assert!(matches!(map_status(-2), crate::core::entity::submission::JudgementStatus::CompilationError));
}

#[test]
fn map_status_pe() {
    assert!(matches!(map_status(-3), crate::core::entity::submission::JudgementStatus::PresentationError));
}

#[test]
fn map_status_wa() {
    assert!(matches!(map_status(-1), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_ac() {
    // 关键回归：0 是 Accepted 而非 Pending。实测 HOJ 提交 1166（status=0，time=2ms，
    // memory=532KB）是一份通过的代码，旧表把它显示为 Pending 并无限轮询
    assert!(matches!(map_status(0), crate::core::entity::submission::JudgementStatus::Accepted));
}

#[test]
fn map_status_tle() {
    assert!(matches!(map_status(1), crate::core::entity::submission::JudgementStatus::TimeLimitExceeded));
}

#[test]
fn map_status_mle() {
    assert!(matches!(map_status(2), crate::core::entity::submission::JudgementStatus::MemoryLimitExceeded));
}

#[test]
fn map_status_re() {
    assert!(matches!(map_status(3), crate::core::entity::submission::JudgementStatus::RuntimeError));
}

#[test]
fn map_status_se() {
    assert!(matches!(map_status(4), crate::core::entity::submission::JudgementStatus::SystemError));
}

#[test]
fn map_status_pending() {
    assert!(matches!(map_status(5), crate::core::entity::submission::JudgementStatus::Pending));
}

#[test]
fn map_status_compiling() {
    assert!(matches!(map_status(6), crate::core::entity::submission::JudgementStatus::Compiling));
}

#[test]
fn map_status_judging() {
    // Judging 沿用既有 Running 语义（轮询判据不变）
    assert!(matches!(map_status(7), crate::core::entity::submission::JudgementStatus::Running));
}

#[test]
fn map_status_pa() {
    // Partial AC 使用独立变体，不折算为 Accepted
    assert!(matches!(map_status(8), crate::core::entity::submission::JudgementStatus::PartiallyAccepted));
}

#[test]
fn map_status_submitting_is_non_terminal() {
    // 9 = Submitting（判题机尚未接手）：折入 Pending，必须是非终态，
    // 否则轮询会在开跑前就停住
    assert!(matches!(map_status(9), crate::core::entity::submission::JudgementStatus::Pending));
}

#[test]
fn map_status_sf() {
    assert!(matches!(map_status(10), crate::core::entity::submission::JudgementStatus::SubmitFailed));
}

#[test]
fn map_status_no_status() {
    // 15 = No Status
    assert!(matches!(map_status(15), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_out_of_range() {
    assert!(matches!(map_status(999), crate::core::entity::submission::JudgementStatus::Unknown));
    assert!(matches!(map_status(11), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_covers_full_hoj_code_table() {
    // 全码表锁定（HOJ Constants.Judge）：任何一格改动都会让此测试失败
    use crate::core::entity::submission::JudgementStatus::*;
    let expected = [
        (-10, NotSubmitted),
        (-4, Cancelled),
        (-3, PresentationError),
        (-2, CompilationError),
        (-1, WrongAnswer),
        (0, Accepted),
        (1, TimeLimitExceeded),
        (2, MemoryLimitExceeded),
        (3, RuntimeError),
        (4, SystemError),
        (5, Pending),
        (6, Compiling),
        (7, Running),
        (8, PartiallyAccepted),
        (9, Pending),
        (10, SubmitFailed),
        (15, Unknown),
    ];
    for (code, want) in expected {
        assert_eq!(
            format!("{:?}", map_status(code)),
            format!("{:?}", want),
            "状态码 {} 映射错误",
            code
        );
    }
}

#[test]
fn map_status_and_is_terminal_status_agree() {
    // 两张表必须等价：map_status 的产物经 is_terminal() 应与 is_terminal_status 同判。
    // 分开维护是为了让 adapter 能在不构造领域变体的情况下判定，代价是可能漂移
    for code in -10..=16 {
        assert_eq!(
            map_status(code).is_terminal(),
            is_terminal_status(code),
            "状态码 {} 的两处终态判据不一致",
            code
        );
    }
}

// ── is_terminal_status ──

#[test]
fn is_terminal_full_table() {
    // 非终态仅 5 Pending / 6 Compiling / 7 Judging / 9 Submitting
    for code in [5, 6, 7, 9] {
        assert!(!is_terminal_status(code), "状态码 {} 应为非终态", code);
    }
    for code in [-10, -4, -3, -2, -1, 0, 1, 2, 3, 4, 8, 10, 15] {
        assert!(is_terminal_status(code), "状态码 {} 应为终态", code);
    }
    assert!(is_terminal_status(999));
}

// ── normalize_error_message ──

#[test]
fn normalize_error_message_drops_hoj_placeholder() {
    // HOJ 对「非 CE/SE/SF」的状态一律回填这句占位文案（JudgeManager.getSubmissionInfo），
    // 不过滤会让每份 AC 代码的详情页都弹出一块红色错误面板
    assert_eq!(
        normalize_error_message(Some(ERROR_MESSAGE_PLACEHOLDER.to_string())),
        None
    );
    assert_eq!(normalize_error_message(Some("   ".to_string())), None);
    assert_eq!(normalize_error_message(None), None);
}

#[test]
fn normalize_error_message_keeps_real_error() {
    let msg = "main.cpp:3:5: error: 'x' was not declared in this scope".to_string();
    assert_eq!(normalize_error_message(Some(msg.clone())), Some(msg));
}

// ── 榜单 DTO 解析（依据 doc/HOJ/HOJ-Contest-Rank-API.md，待内网联调校正）──

/// ACM 榜单行原始 JSON：字段名严格按 HOJ 响应，含不规则大小写 isAC / isFirstAC / ACTime
const ACM_RANK_ROW_JSON: &str = r#"{
  "rank": 1,
  "uid": "a1b2c3d4",
  "username": "alice",
  "realname": "张三",
  "nickname": "Alice",
  "school": "XX大学",
  "gender": "female",
  "avatar": "https://example.com/a.png",
  "totalTime": 3720,
  "total": 5,
  "ac": 3,
  "submissionInfo": {
    "A": { "errorNum": 1, "isAC": true, "isFirstAC": true, "ACTime": 600 },
    "B": { "errorNum": 2 },
    "D": { "errorNum": 0, "tryNum": 1 }
  }
}"#;

#[test]
fn acm_rank_row_parses_irregular_field_names() {
    let vo: ContestRankVO = serde_json::from_str(ACM_RANK_ROW_JSON).expect("ACM 榜单行解析失败");
    let row = vo.into_rank_row();

    assert_eq!(row.rank, 1);
    assert_eq!(row.username, "alice");
    assert_eq!(row.realname, "张三");
    assert_eq!(row.school, "XX大学");
    assert_eq!(row.gender, "female");
    assert_eq!(row.ac, 3);
    assert_eq!(row.total, 5);
    assert_eq!(row.total_time, 3720);
    assert_eq!(row.total_score, None);

    // isAC / isFirstAC / ACTime 的不规则大小写必须被显式 rename 命中
    let a = row.submission_info.get("A").expect("缺少 A 列");
    assert!(a.is_ac);
    assert!(a.is_first_ac);
    assert_eq!(a.ac_time, Some(600));
    assert_eq!(a.error_num, 1);

    // 只有 errorNum 的格子 = 未通过
    let b = row.submission_info.get("B").expect("缺少 B 列");
    assert!(!b.is_ac);
    assert_eq!(b.error_num, 2);
    assert_eq!(b.try_num, None);

    // 封榜期间只有 tryNum，不写 isAC / ACTime
    let d = row.submission_info.get("D").expect("缺少 D 列");
    assert_eq!(d.try_num, Some(1));
    assert!(!d.is_ac);
    assert_eq!(d.ac_time, None);

    // 未出现的题目 = 无任何提交记录（前端渲染空格子）
    assert!(!row.submission_info.contains_key("C"));
}

#[test]
fn oi_rank_row_normalizes_score_and_time_info() {
    let json = r#"{
      "rank": 1, "uid": "u-oi", "username": "bob",
      "totalScore": 280, "totalTime": 3560,
      "submissionInfo": { "A": 100, "B": 80 },
      "timeInfo": { "A": 1200 }
    }"#;
    let row = serde_json::from_str::<ContestRankVO>(json)
        .expect("OI 榜单行解析失败")
        .into_rank_row();

    assert_eq!(row.total_score, Some(280));
    assert_eq!(row.total_time, 3560);
    // OI 的 submissionInfo 值是整数得分，归一到 RankCell.score
    assert_eq!(row.submission_info.get("A").and_then(|c| c.score), Some(100));
    assert_eq!(row.submission_info.get("B").and_then(|c| c.score), Some(80));
    assert!(!row.submission_info.get("A").expect("缺少 A").is_ac);
    assert_eq!(row.time_info.get("A"), Some(&1200));
    // OI 行没有 ac / total 字段 → 归零而不是整行解析失败
    assert_eq!(row.ac, 0);
    assert_eq!(row.total, 0);
}

#[test]
fn star_team_keeps_rank_minus_one_and_defaults_optional_text() {
    let json = r#"{ "rank": -1, "uid": "star", "username": "*" }"#;
    let row = serde_json::from_str::<ContestRankVO>(json)
        .expect("解析失败")
        .into_rank_row();

    assert_eq!(row.rank, -1, "打星队伍 rank 必须保留 -1");
    // 可选文本字段缺失时回退空串，前端按字符串处理不必判 null
    assert_eq!(row.realname, "");
    assert_eq!(row.school, "");
    assert_eq!(row.gender, "");
}

#[test]
fn malformed_cell_degrades_instead_of_failing_row() {
    // 单元格既不是整数也不是合法明细对象 → 该格降级为默认值，整行仍可用。
    // 榜单是赛场高频只读数据，局部字段异常不应导致整页不可用。
    let json = r#"{ "rank": 7, "uid": "u", "username": "x", "submissionInfo": { "A": "not-a-cell" } }"#;
    let row = serde_json::from_str::<ContestRankVO>(json)
        .expect("解析失败")
        .into_rank_row();

    let cell = row.submission_info.get("A").expect("缺少 A 列");
    assert!(!cell.is_ac);
    assert_eq!(cell.error_num, 0);
    assert_eq!(cell.score, None);
    assert_eq!(row.rank, 7);
}

#[test]
fn rank_dto_serializes_camel_case_and_never_force_refreshes() {
    let dto = ContestRankDTO {
        cid: 1,
        current_page: 2,
        limit: 50,
        force_refresh: false,
        remove_star: true,
        keyword: Some("XX大学".into()),
        contains_end: false,
        concerned_list: Vec::new(),
        external_cid_list: None,
    };
    let json = serde_json::to_value(&dto).expect("序列化失败");

    assert_eq!(json["cid"], 1);
    assert_eq!(json["currentPage"], 2);
    assert_eq!(json["removeStar"], true);
    assert_eq!(json["containsEnd"], false);
    assert_eq!(
        json["forceRefresh"], false,
        "非比赛创建者/超管传 true 会被服务端忽略，必须恒为 false"
    );
    assert_eq!(json["externalCidList"], serde_json::Value::Null);
}

// ── 用户题目状态归一 ──

#[test]
fn extract_problem_status_code_accepts_number_bool_and_object() {
    assert_eq!(extract_problem_status_code(&serde_json::json!(-10)), Some(-10));
    assert_eq!(extract_problem_status_code(&serde_json::json!(0)), Some(0));
    // 布尔形态：true = 已 AC（= HOJ 的 0），false = 未提交（= HOJ 的 -10）
    assert_eq!(extract_problem_status_code(&serde_json::json!(true)), Some(0));
    assert_eq!(extract_problem_status_code(&serde_json::json!(false)), Some(-10));
    // 实测形态：{"status": <码>, "score": …}
    assert_eq!(extract_problem_status_code(&serde_json::json!({"status": -10})), Some(-10));
    assert_eq!(extract_problem_status_code(&serde_json::json!({"status": 0})), Some(0));
}

#[test]
fn extract_problem_status_code_unknown_shape_is_none() {
    assert_eq!(extract_problem_status_code(&serde_json::json!("AC")), None);
    assert_eq!(extract_problem_status_code(&serde_json::json!(null)), None);
    assert_eq!(extract_problem_status_code(&serde_json::json!({})), None);
}

#[test]
fn normalize_problem_status_maps_hoj_codes_to_frontend_contract() {
    // 0 = Accepted → 1 已通过
    assert_eq!(normalize_problem_status(0), 1);
    // -10 = Not Submitted → 0 未提交
    assert_eq!(normalize_problem_status(-10), 0);
    // 其余（含负数与各类失败）→ 2 尝试过
    for code in [-4, -3, -2, -1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15] {
        assert_eq!(normalize_problem_status(code), 2, "状态码 {} 应归一为「尝试过」", code);
    }
}

#[test]
fn problem_status_end_to_end_matches_live_server_samples() {
    // 实测样本（比赛 1012，本人已 AC 该题，isContestProblemList=true）：
    //   {"1000":{"score":null,"status":0}}    → 已通过
    //   {"1000":{"status":-10}}（false 分支）  → 未提交
    let ac = serde_json::json!({"score": null, "status": 0});
    let not_submitted = serde_json::json!({"status": -10});
    assert_eq!(
        extract_problem_status_code(&ac).map(normalize_problem_status),
        Some(1)
    );
    assert_eq!(
        extract_problem_status_code(&not_submitted).map(normalize_problem_status),
        Some(0)
    );
}

// ── 提交请求体 ──

#[test]
fn submit_pid_prefers_contest_display_id() {
    // 回归背景：比赛提交必须传比赛内展示题号。传数字 pid 会让 HOJ 在
    // `contestProblem.getId()` 上 NPE 返回 HTTP 500（实测），且服务端不留任何记录
    assert_eq!(submit_pid("1000", "A"), "A");
    assert_eq!(submit_pid("1000", "B1"), "B1");
    // display_id 两端空白应被裁掉（服务端按精确值查 display_id）
    assert_eq!(submit_pid("1000", "  A  "), "A");
}

#[test]
fn submit_pid_falls_back_to_problem_id_without_display_id() {
    // 非比赛场景两者同源；回退保证「没有比赛上下文」时不会提交空 pid
    assert_eq!(submit_pid("1000", ""), "1000");
    assert_eq!(submit_pid("1000", "   "), "1000");
    assert_eq!(submit_pid(" 1000 ", ""), "1000");
    assert_eq!(submit_pid("HOJ-1001", ""), "HOJ-1001");
}

#[test]
fn submit_request_serializes_with_hoj_field_names() {
    // 锁定 wire 格式：pid / language / code / cid / tid / gid / isRemote
    // （字段名漂移会让 HOJ 校验失败或落到错误分支）
    let body = SubmitRequest {
        pid: submit_pid("1000", "A"),
        language: "C++".to_string(),
        code: "int main(){}".to_string(),
        cid: 1012,
        tid: None,
        gid: None,
        is_remote: false,
    };
    let json: serde_json::Value = serde_json::to_value(&body).expect("序列化失败");
    assert_eq!(json["pid"], "A", "比赛提交的 pid 必须是展示题号");
    assert_eq!(json["language"], "C++");
    assert_eq!(json["code"], "int main(){}");
    assert_eq!(json["cid"], 1012);
    assert!(json.get("isRemote").is_some(), "字段名必须是 isRemote");
    assert_eq!(json["isRemote"], false);
    assert!(json.get("tid").is_some() && json["tid"].is_null());
    assert!(json.get("gid").is_some() && json["gid"].is_null());
}

// ── 比赛详情新增字段（榜单显示名 / 封榜 / 赛后提交）──

#[test]
fn contest_vo_parses_rank_and_seal_fields() {
    let json = r#"{
      "id": 1, "title": "T", "type": 0, "status": 0,
      "startTime": "2024-01-01T08:00:00", "endTime": "2024-01-01T13:00:00",
      "auth": 1, "rankShowName": "realname", "sealRank": true,
      "sealRankTime": "2024-01-01T12:00:00", "allowEndSubmit": true
    }"#;
    let vo: ContestVO = serde_json::from_str(json).expect("ContestVO 解析失败");

    assert_eq!(vo.rank_show_name.as_deref(), Some("realname"));
    assert!(vo.seal_rank);
    assert_eq!(vo.seal_rank_time.as_deref(), Some("2024-01-01T12:00:00"));
    assert!(vo.allow_end_submit);
}

#[test]
fn contest_vo_tolerates_missing_rank_fields() {
    // get-contest-list 不返回榜单相关字段，必须靠 serde default 解析成功
    let vo: ContestVO = serde_json::from_str(r#"{ "id": 2, "title": "T2" }"#)
        .expect("ContestVO 解析失败");

    assert_eq!(vo.rank_show_name, None);
    assert!(!vo.seal_rank);
    assert_eq!(vo.seal_rank_time, None);
    assert!(!vo.allow_end_submit);
}

// ── strip_nulls 与 null 容错 ──
//
// 回归背景：HOJ 对未设置的字段返回 null 而不是省略（实测 get-contest-list 的
// sealRank / rankShowName / count / now / openPrint 全为 null），而 serde 的
// #[serde(default)] 只在字段**缺失**时生效，显式 null 会让整个响应解析失败 ——
// 登录页因此拿不到比赛列表，界面报「获取比赛列表失败」。

#[test]
fn strip_nulls_removes_null_members_and_array_items() {
    let mut value: serde_json::Value = serde_json::from_str(
        r#"{
            "keep": 1,
            "drop": null,
            "nested": { "a": null, "b": "x" },
            "list": [null, { "c": null, "d": 2 }, 3],
            "falseButNotNull": false,
            "zeroButNotNull": 0
        }"#,
    )
    .expect("测试夹具本身应是合法 JSON");

    strip_nulls(&mut value);

    let obj = value.as_object().expect("顶层应仍是对象");
    assert!(!obj.contains_key("drop"), "null 成员应被剔除");
    assert_eq!(obj.get("keep").and_then(|v| v.as_i64()), Some(1));
    // false 与 0 不是 null，必须保留（否则封榜、打星、零分等语义会被抹掉）
    assert_eq!(obj.get("falseButNotNull").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(obj.get("zeroButNotNull").and_then(|v| v.as_i64()), Some(0));
    let nested = obj.get("nested").and_then(|v| v.as_object()).expect("nested 应保留");
    assert!(!nested.contains_key("a"));
    assert_eq!(nested.get("b").and_then(|v| v.as_str()), Some("x"));
    let list = obj.get("list").and_then(|v| v.as_array()).expect("list 应保留");
    assert_eq!(list.len(), 2, "数组中的 null 元素应被剔除");
}

#[test]
fn contest_vo_tolerates_hoj_null_fields() {
    // 与 get-contest-list 实测响应同形：未设置的字段一律是 null 而不是缺失
    let body = r#"{
        "id": 1012, "author": "someone", "title": "测试赛", "type": 0,
        "description": "<p>desc</p>", "status": -1, "source": 0, "auth": 0,
        "now": null, "startTime": "2026-09-21T16:00:00.000+0000",
        "endTime": "2026-09-29T16:00:00.000+0000", "duration": 691200,
        "sealRank": null, "openPrint": null, "sealRankTime": null,
        "rankShowName": null, "openRank": false, "oiRankScoreType": "Recent",
        "count": null, "gid": null, "allowEndSubmit": false
    }"#;

    // 未经去 null 处理时必然失败 —— 这正是线上故障的成因，锁死它防止回退
    assert!(
        serde_json::from_str::<ContestVO>(body).is_err(),
        "显式 null 直接喂给 serde 应当失败，否则说明本测试已失去意义"
    );

    let mut value: serde_json::Value = serde_json::from_str(body).expect("夹具应是合法 JSON");
    strip_nulls(&mut value);
    let vo: ContestVO = serde_json::from_value(value).expect("去 null 后应能解析");

    assert_eq!(vo.id, 1012);
    assert_eq!(vo.status, -1);
    assert!(!vo.seal_rank, "null 的 sealRank 应落到默认 false");
    assert_eq!(vo.rank_show_name, None);
    assert_eq!(vo.seal_rank_time, None);
    assert!(!vo.allow_end_submit);
    assert_eq!(vo.duration, 691200);
}

#[test]
fn page_result_accepts_documented_and_degenerate_shapes() {
    // 文档只承诺 records/total；实测还会带 size/current/orders/searchCount/pages
    let minimal = r#"{ "records": [{ "id": 1 }], "total": 1 }"#;
    let page: PageResult<ContestVO> =
        serde_json::from_str(minimal).expect("最小分页形状应可解析");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.total, 1);
    assert_eq!(page.pages, 0, "未返回的字段落到默认值");

    // records / total 为 null 也不应让整页失败
    let mut degenerate: serde_json::Value =
        serde_json::from_str(r#"{ "records": null, "total": null }"#).expect("夹具合法");
    strip_nulls(&mut degenerate);
    let page: PageResult<ContestVO> =
        serde_json::from_value(degenerate).expect("空分页应可解析");
    assert!(page.records.is_empty());
    assert_eq!(page.total, 0);
}

#[test]
fn submission_detail_tolerates_null_judge_fields() {
    // 评测未完成时 HOJ 的 time / memory / score 为 null；
    // 此前会让整条轮询链路解析失败，表现为提交后状态永远停在 Pending
    let body = r#"{
        "submitId": 42, "pid": 1061, "displayPid": "A", "username": "u",
        "submitTime": "2026-09-14 20:00:00", "status": 1,
        "errorMessage": null, "time": null, "memory": null, "score": null,
        "length": null, "language": "C++", "code": null, "cid": 1011
    }"#;
    let mut value: serde_json::Value = serde_json::from_str(body).expect("夹具合法");
    strip_nulls(&mut value);
    let detail: SubmissionDetail = serde_json::from_value(value).expect("去 null 后应能解析");
    assert_eq!(detail.submit_id, 42);
    assert_eq!(detail.status, 1);
    assert_eq!(detail.time, 0);
    assert_eq!(detail.memory, 0);
    assert_eq!(detail.score, None);
}

#[test]
fn acm_submission_info_tolerates_null_cells() {
    // 封榜期间单元格可能只给 tryNum，其余字段为 null
    let body = r#"{ "errorNum": null, "tryNum": 3, "isAC": null, "isFirstAC": null, "ACTime": null, "isAfterContest": null }"#;
    let mut value: serde_json::Value = serde_json::from_str(body).expect("夹具合法");
    strip_nulls(&mut value);
    let info: AcmSubmissionInfo = serde_json::from_value(value).expect("去 null 后应能解析");
    assert_eq!(info.error_num, 0);
    assert_eq!(info.try_num, Some(3));
    assert!(!info.is_ac);
    assert!(!info.is_first_ac);
    assert_eq!(info.ac_time, None);
}

#[test]
fn api_response_null_data_becomes_none() {
    let body = r#"{ "status": 401, "msg": "未登录", "data": null }"#;
    let mut value: serde_json::Value = serde_json::from_str(body).expect("夹具合法");
    strip_nulls(&mut value);
    let resp: ApiResponse<ContestVO> = serde_json::from_value(value).expect("应能解析");
    assert!(!resp.is_success(), "status=401 不是成功（成功恒为 200）");
    assert_eq!(resp.into_data().unwrap_err(), "未登录");
}
