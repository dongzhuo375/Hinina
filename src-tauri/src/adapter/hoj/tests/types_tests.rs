// HOJ types.rs 单元测试：map_status / is_terminal_status

use super::*;

// ── map_status ──

#[test]
fn map_status_pending() {
    assert!(matches!(map_status(0), crate::core::entity::submission::JudgementStatus::Running));
}

#[test]
fn map_status_judging() {
    assert!(matches!(map_status(1), crate::core::entity::submission::JudgementStatus::Running));
}

#[test]
fn map_status_ce() {
    assert!(matches!(map_status(2), crate::core::entity::submission::JudgementStatus::CompilationError));
}

#[test]
fn map_status_pe() {
    // PE → WrongAnswer（无对应枚举）
    assert!(matches!(map_status(3), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_wa() {
    assert!(matches!(map_status(4), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_ac() {
    assert!(matches!(map_status(5), crate::core::entity::submission::JudgementStatus::Accepted));
}

#[test]
fn map_status_tle() {
    assert!(matches!(map_status(6), crate::core::entity::submission::JudgementStatus::TimeLimitExceeded));
}

#[test]
fn map_status_mle() {
    assert!(matches!(map_status(7), crate::core::entity::submission::JudgementStatus::MemoryLimitExceeded));
}

#[test]
fn map_status_ole() {
    // OLE → Unknown（无对应枚举）
    assert!(matches!(map_status(8), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_re() {
    assert!(matches!(map_status(9), crate::core::entity::submission::JudgementStatus::RuntimeError));
}

#[test]
fn map_status_se() {
    // SE → Unknown
    assert!(matches!(map_status(10), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_rje() {
    // RJE → Unknown
    assert!(matches!(map_status(11), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_sf() {
    // SF → WrongAnswer
    assert!(matches!(map_status(12), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_pa() {
    // Partial AC → Accepted（保守映射）
    assert!(matches!(map_status(13), crate::core::entity::submission::JudgementStatus::Accepted));
}

#[test]
fn map_status_freq() {
    // FREQ → Unknown
    assert!(matches!(map_status(14), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_ue() {
    // UE → Unknown
    assert!(matches!(map_status(15), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_negative() {
    assert!(matches!(map_status(-1), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_out_of_range() {
    assert!(matches!(map_status(999), crate::core::entity::submission::JudgementStatus::Unknown));
}

// ── is_terminal_status ──

#[test]
fn is_terminal_pending() {
    assert!(!is_terminal_status(0));
}

#[test]
fn is_terminal_judging() {
    assert!(!is_terminal_status(1));
}

#[test]
fn is_terminal_ce() {
    assert!(is_terminal_status(2));
}

#[test]
fn is_terminal_pe() {
    assert!(is_terminal_status(3));
}

#[test]
fn is_terminal_ac() {
    assert!(is_terminal_status(5));
}

#[test]
fn is_terminal_out_of_range() {
    assert!(is_terminal_status(999));
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
fn coerce_problem_status_accepts_number_bool_and_object() {
    assert_eq!(coerce_problem_status(&serde_json::json!(0)), 0);
    assert_eq!(coerce_problem_status(&serde_json::json!(1)), 1);
    assert_eq!(coerce_problem_status(&serde_json::json!(2)), 2);
    assert_eq!(coerce_problem_status(&serde_json::json!(true)), 1);
    assert_eq!(coerce_problem_status(&serde_json::json!(false)), 0);
    assert_eq!(coerce_problem_status(&serde_json::json!({"status": 2})), 2);
}

#[test]
fn coerce_problem_status_unknown_shape_falls_back_to_not_submitted() {
    // 保守策略：无法识别时按「未提交」，绝不把未做的题标成已通过
    assert_eq!(coerce_problem_status(&serde_json::json!("AC")), 0);
    assert_eq!(coerce_problem_status(&serde_json::json!(null)), 0);
    assert_eq!(coerce_problem_status(&serde_json::json!({})), 0);
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
