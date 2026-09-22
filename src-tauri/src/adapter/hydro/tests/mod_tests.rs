// Hydro mod.rs 单元测试：响应处置、映射 helper、评测记录投影。
//
// 夹具均为按 `doc/Hydro/HYDRO-API.md` 手工构造（无可用联调实例）。

use super::*;
use crate::core::entity::submission::JudgementStatus;
use crate::test_support::{Guarded, TempDir};

/// 状态变体的线上名（`JudgementStatus` 未派生 `PartialEq`，且前端依赖序列化名）
fn status_name(status: &JudgementStatus) -> String {
    serde_json::to_value(status)
        .expect("JudgementStatus 必须可序列化")
        .as_str()
        .expect("变体名即序列化字符串")
        .to_string()
}

fn fixture(name: &str) -> Value {
    let raw = match name {
        "contest_list" => include_str!("fixtures/contest_list.json"),
        "contest_problems" => include_str!("fixtures/contest_problems.json"),
        "problem_detail" => include_str!("fixtures/problem_detail.json"),
        "record_detail" => include_str!("fixtures/record_detail.json"),
        "record_list" => include_str!("fixtures/record_list.json"),
        other => panic!("未登记的夹具: {}", other),
    };
    let mut value: Value = serde_json::from_str(raw).expect("夹具必须是合法 JSON");
    types::strip_nulls(&mut value);
    value
}

fn response(status: u16, body: &str) -> HydroResponse {
    HydroResponse {
        status: StatusCode::from_u16(status).unwrap(),
        headers: HeaderMap::new(),
        body: body.to_string(),
        url: "https://hydro.ac/x".to_string(),
    }
}

fn tdocs() -> Vec<TdocVO> {
    let vo: ContestListVO = serde_json::from_value(fixture("contest_list")).unwrap();
    vo.tdocs.unwrap()
}

// ── 响应处置（状态码优先于解析） ──

#[test]
fn non_success_status_with_error_envelope_maps_to_auth() {
    let body = include_str!("fixtures/error_privilege.json");
    let err = response(403, body).into_value().unwrap_err();
    assert!(matches!(err, AppError::Auth(_)));
}

#[test]
fn gateway_html_error_page_reports_http_status_not_parse_failure() {
    // 5xx 网关返回 HTML：必须报成 HTTP 502 而不是「响应不是合法 JSON」，
    // 否则排障会被引向 DTO 而不是服务端
    let err = response(502, "<html><body>Bad Gateway</body></html>")
        .into_value()
        .unwrap_err();
    match err {
        AppError::Network(msg) => assert!(msg.contains("502"), "应带状态码: {}", msg),
        other => panic!("应为 Network，实际 {:?}", other),
    }
}

#[test]
fn http_401_maps_to_auth_variant() {
    let err = response(401, "unauthorized").into_value().unwrap_err();
    assert!(matches!(err, AppError::Auth(_)), "401 是标准未认证语义");
}

#[test]
fn json_login_redirect_maps_to_auth() {
    let body = include_str!("fixtures/login_redirect.json");
    let err = response(200, body).into_value().unwrap_err();
    assert!(
        matches!(err, AppError::Auth(_)),
        "JSON 化重定向必须报成 Auth，否则会话过期会被当成空数据"
    );
}

#[test]
fn success_with_unexpected_shape_reports_serialization() {
    let err = response(200, "{\"pdoc\": 1}")
        .into_json::<ProblemDetailVO>()
        .unwrap_err();
    assert!(matches!(err, AppError::Serialization(_)));
}

#[test]
fn success_payload_parses() {
    let body = include_str!("fixtures/problem_detail.json");
    let vo: ProblemDetailVO = response(200, body).into_json().unwrap();
    assert_eq!(vo.pdoc.unwrap().problem_id(), "P1000");
}

#[test]
fn http_status_error_follows_infra_variant_rules() {
    // 与 infra::http::status_error 同一判据：401 → Auth，其余（含 403）→ Network
    assert!(matches!(
        http_status_error("u", StatusCode::UNAUTHORIZED),
        AppError::Auth(_)
    ));
    assert!(matches!(
        http_status_error("u", StatusCode::FORBIDDEN),
        AppError::Network(_)
    ));
    assert!(matches!(
        http_status_error("u", StatusCode::INTERNAL_SERVER_ERROR),
        AppError::Network(_)
    ));
}

// ── 比赛阶段 / 赛制 ──

#[test]
fn contest_status_covers_all_phases() {
    let now = 1_000_000;
    assert_eq!(HydroAdapter::contest_status(now + 60, now + 3600, now), -1);
    assert_eq!(HydroAdapter::contest_status(now - 60, now + 3600, now), 0);
    assert_eq!(HydroAdapter::contest_status(now - 3600, now - 60, now), 1);
    // 边界：正好开赛 / 正好结束都算进行中
    assert_eq!(HydroAdapter::contest_status(now, now + 60, now), 0);
    assert_eq!(HydroAdapter::contest_status(now - 60, now, now), 0);
    // 时间缺失按进行中，避免服务端漏字段把比赛判成已结束
    assert_eq!(HydroAdapter::contest_status(0, 0, now), 0);
    assert_eq!(HydroAdapter::contest_status(0, now - 60, now), 1);
}

#[test]
fn contest_type_maps_rule_to_acm_or_oi() {
    assert_eq!(HydroAdapter::contest_type_of(Some("acm")), 0);
    for rule in ["oi", "ioi", "strictioi", "ledo", "homework", ""] {
        assert_eq!(HydroAdapter::contest_type_of(Some(rule)), 1, "{}", rule);
    }
    assert_eq!(HydroAdapter::contest_type_of(None), 1);
}

#[test]
fn oi_rank_score_type_only_for_highest_score_rules() {
    assert_eq!(
        HydroAdapter::oi_rank_score_type_of(Some("oi")).as_deref(),
        Some("Highest")
    );
    assert_eq!(
        HydroAdapter::oi_rank_score_type_of(Some("strictioi")).as_deref(),
        Some("Highest")
    );
    // ioi 是「首次得分即定」、ledo 是指数衰减，与 Recent/Highest 都不等价 → 不标注
    for rule in ["ioi", "ledo", "acm", "homework"] {
        assert!(HydroAdapter::oi_rank_score_type_of(Some(rule)).is_none());
    }
}

#[test]
fn is_acm_rule_is_exact_match() {
    assert!(HydroAdapter::is_acm_rule(Some("acm")));
    assert!(!HydroAdapter::is_acm_rule(Some("ioi")));
    assert!(!HydroAdapter::is_acm_rule(None));
}

// ── 比赛映射 ──

#[test]
fn into_contest_maps_times_rule_and_lock() {
    let contests: Vec<Contest> = tdocs()
        .into_iter()
        .map(HydroAdapter::into_contest)
        .collect();

    let first = &contests[0];
    assert_eq!(first.id, "64f0c0f0f0f0f0f0f0f0f0f0");
    assert_eq!(first.title, "Example Contest");
    assert_eq!(first.start_time, 1_767_229_200);
    assert_eq!(first.end_time, 1_767_247_200);
    assert_eq!(first.contest_type, 0, "acm → 0");
    assert!(!first.seal_rank, "lockAt 为 null → 未封榜");
    assert_eq!(first.seal_rank_time, None);
    assert_eq!(first.oi_rank_score_type, None);

    // 无 endAt、只有 duration（小时）→ end = begin + 5h
    let second = &contests[1];
    assert_eq!(second.contest_type, 1, "oi → 1");
    assert_eq!(second.end_time, 1_767_247_200);
    assert_eq!(second.description, "");
    assert_eq!(
        second.oi_rank_score_type.as_deref(),
        Some("Highest"),
        "oi 每题取更高分"
    );
}

#[test]
fn into_contest_marks_locked_contest() {
    let vo: ContestProblemListVO = serde_json::from_value(fixture("contest_problems")).unwrap();
    let contest = HydroAdapter::into_contest(vo.tdoc.unwrap());
    assert!(contest.seal_rank);
    assert_eq!(contest.seal_rank_time, Some(1_767_247_200 - 3600));
}

#[test]
fn map_contest_problems_derives_letters_from_pids_index() {
    let vo: ContestProblemListVO = serde_json::from_value(fixture("contest_problems")).unwrap();
    // pids = [1001, 1000]：docId 1001 → 下标 0 → A；docId 1000 → 下标 1 → B
    let pids = vec!["1001".to_string(), "1000".to_string()];
    let problems = HydroAdapter::map_contest_problems(&vo, "64f0c0f0f0f0f0f0f0f0f0f0", &pids);

    assert_eq!(problems.len(), 2);
    // 输出按 docId 升序（HashMap 迭代顺序随机，必须固定）
    assert_eq!(problems[0].id, 1000);
    assert_eq!(problems[0].display_id, "B");
    assert_eq!(problems[0].problem_id, "P1000");
    assert_eq!(problems[1].id, 1001);
    assert_eq!(problems[1].display_id, "A");
    assert_eq!(problems[1].problem_id, "A1");
    assert_eq!(problems[1].ac, 8);
    assert_eq!(problems[1].total, 12);
    // 比赛 ID 如实携带（Hydro 是 24 位 hex ObjectId，故 cid 为字符串）
    assert_eq!(problems[0].cid, "64f0c0f0f0f0f0f0f0f0f0f0");
    // Hydro 无气球色
    assert_eq!(problems[0].color, "");
}

#[test]
fn map_contest_problems_without_pid_order_falls_back_to_position() {
    let vo: ContestProblemListVO = serde_json::from_value(fixture("contest_problems")).unwrap();
    let problems = HydroAdapter::map_contest_problems(&vo, "64f0c0f0f0f0f0f0f0f0f0f0", &[]);
    // 顺序表缺失时按 docId 升序的下标派生，至少保证字母唯一
    assert_eq!(problems[0].display_id, "A");
    assert_eq!(problems[1].display_id, "B");
}

// ── 题目映射 ──

#[test]
fn into_problem_maps_markdown_limits_and_languages() {
    let vo: ProblemDetailVO = serde_json::from_value(fixture("problem_detail")).unwrap();
    let problem = HydroAdapter::into_problem(&vo.pdoc.unwrap());

    assert_eq!(problem.id, "P1000");
    assert_eq!(problem.title, "A+B Problem");
    assert!(problem.description.contains("# A+B"));
    // 三段式题面只有一段有内容（Hydro 的题面是单块 Markdown）
    assert_eq!(problem.input_description, "");
    assert_eq!(problem.output_description, "");
    // Hydro 无样例字段（缺口 D6）
    assert!(problem.samples.is_empty());
    assert_eq!(problem.time_limit, 2000, "多测试点取 timeMax");
    assert_eq!(problem.memory_limit, 256);
    // 语言 key → HOJ 显示名；未知 key 原样透传（部署可自定义 langs）
    assert_eq!(
        problem.languages,
        vec!["C++", "C++17", "Python 3", "unknown_lang"]
    );
}

#[test]
fn into_problem_without_config_yields_zero_limits() {
    let pdoc: PdocVO =
        serde_json::from_value(serde_json::json!({ "docId": 1000, "pid": "P1000" })).unwrap();
    let problem = HydroAdapter::into_problem(&pdoc);
    assert_eq!(problem.time_limit, 0);
    assert_eq!(problem.memory_limit, 0);
    assert!(problem.languages.is_empty());
}

// ── 评测映射 ──

#[test]
fn into_judgement_result_passes_through_non_terminal() {
    let vo: RecordDetailVO = serde_json::from_value(fixture("record_detail")).unwrap();
    let rdoc = vo.rdoc.unwrap();
    let result = HydroAdapter::into_judgement_result(&rdoc);
    assert_eq!(status_name(&result.status), "Accepted");
    assert_eq!(result.time_ms, 12);
    assert_eq!(result.memory_kb, 1024);
    assert_eq!(result.score, 100.0);
}

#[test]
fn into_judgement_result_zeroes_metrics_while_judging() {
    let rdoc: RdocVO = serde_json::from_value(serde_json::json!({
        "_id": "6530f0c1a1b2c3d4e5f60718",
        "status": 20,
        "time": 7,
        "memory": 8,
        "score": 30
    }))
    .unwrap();
    let result = HydroAdapter::into_judgement_result(&rdoc);
    assert_eq!(status_name(&result.status), "Running");
    assert_eq!(result.time_ms, 0);
    assert_eq!(result.memory_kb, 0);
    assert_eq!(result.score, 0.0);
}

#[test]
fn into_submission_record_maps_display_id_and_language() {
    let vo: RecordListVO = serde_json::from_value(fixture("record_list")).unwrap();
    let pdict = vo.pdict.clone().unwrap_or_default();
    let udict = vo.udict.clone().unwrap_or_default();
    let pids = vo.tdoc.as_ref().unwrap().pid_list();

    let records: Vec<SubmissionRecord> = vo
        .rdocs
        .unwrap()
        .iter()
        .map(|rdoc| HydroAdapter::into_submission_record(rdoc, &pdict, &udict, &pids, true))
        .collect();

    assert_eq!(records[0].submit_id, "6530f0c1a1b2c3d4e5f60718");
    assert_eq!(records[0].display_id, "A", "pid 1000 在 pids 的下标 0");
    assert_eq!(records[0].display_pid, "P1000");
    assert_eq!(records[0].title, "A+B Problem");
    assert_eq!(records[0].username, "admin");
    assert_eq!(records[0].language, "C++");
    assert_eq!(status_name(&records[0].status), "Accepted");
    assert_eq!(records[0].submit_time, 1_697_706_177);
    // ACM 题的 score 必须为 None（Hydro 对 ACM 也返回 0/100）
    assert_eq!(records[0].score, None);

    assert_eq!(records[1].display_id, "B");
    assert_eq!(records[1].language, "Python 3");
    assert_eq!(status_name(&records[1].status), "Running");
}

#[test]
fn into_submission_record_keeps_score_for_oi() {
    let vo: RecordListVO = serde_json::from_value(fixture("record_list")).unwrap();
    let pdict = vo.pdict.clone().unwrap_or_default();
    let udict = vo.udict.clone().unwrap_or_default();
    let record =
        HydroAdapter::into_submission_record(&vo.rdocs.unwrap()[0], &pdict, &udict, &[], false);
    assert_eq!(record.score, Some(100.0));
    assert_eq!(record.display_id, "", "无顺序表时展示字母留空");
}

#[test]
fn into_submission_detail_maps_code_and_compiler_text() {
    let vo: RecordDetailVO = serde_json::from_value(fixture("record_detail")).unwrap();
    let detail =
        HydroAdapter::into_submission_detail(&vo.rdoc.unwrap(), vo.udoc.as_ref(), vo.pdoc.as_ref());
    assert_eq!(detail.submit_id, "6530f0c1a1b2c3d4e5f60718");
    assert_eq!(detail.pid, "1000");
    assert_eq!(detail.display_pid, "P1000");
    assert_eq!(detail.username, "admin");
    assert_eq!(detail.language, "C++17");
    assert_eq!(detail.code, "int main(){return 0;}");
    assert_eq!(detail.length, 21);
    assert_eq!(detail.judger.as_deref(), Some("1"));
    assert_eq!(detail.error_message, None);
    assert_eq!(detail.score, Some(100.0));
    assert_eq!(detail.oi_rank_score, None, "Hydro 无该字段");
}

#[test]
fn into_submission_detail_joins_compiler_texts() {
    let rdoc: RdocVO = serde_json::from_value(serde_json::json!({
        "_id": "6530f0c1a1b2c3d4e5f60718",
        "status": 7,
        "compilerTexts": ["main.cpp:3:5: error: expected ';'", "", "  1 error generated."]
    }))
    .unwrap();
    let detail = HydroAdapter::into_submission_detail(&rdoc, None, None);
    assert_eq!(status_name(&detail.status), "CompilationError");
    let message = detail.error_message.expect("CE 必须有编译信息");
    assert!(message.contains("expected ';'"));
    assert!(message.contains("1 error generated"));
    assert!(!message.contains("\n\n"), "空行应被过滤");
    // 非终态之外的 CE 是终态，score 透出（服务端给 0）
    assert_eq!(detail.score, Some(0.0));
}

#[test]
fn non_terminal_detail_hides_score() {
    let rdoc: RdocVO =
        serde_json::from_value(serde_json::json!({ "status": 21, "score": 0 })).unwrap();
    let detail = HydroAdapter::into_submission_detail(&rdoc, None, None);
    assert_eq!(detail.score, None, "评测未完成时 score 无意义");
    assert_eq!(detail.language, "");
}

// ── 测试点映射 ──

#[test]
fn into_judge_case_maps_fields_and_defaults() {
    let cases: Vec<types::TestCaseVO> = serde_json::from_value(serde_json::json!([
        { "id": 1, "subtaskId": 2, "status": 2, "score": 0, "time": 15, "memory": 2048 },
        {}
    ]))
    .unwrap();

    let first = HydroAdapter::into_judge_case(&cases[0], 0);
    assert_eq!(first.case_id, 1);
    assert_eq!(first.seq, 1);
    assert_eq!(status_name(&first.status), "WrongAnswer");
    assert_eq!(first.time_ms, 15);
    assert_eq!(first.memory_kb, 2048);
    assert_eq!(first.group_num, Some(2));

    // 缺失字段落默认值：id 回退序号、状态回退 WAITING(0)
    let second = HydroAdapter::into_judge_case(&cases[1], 1);
    assert_eq!(second.case_id, 2);
    assert_eq!(second.seq, 2);
    assert_eq!(status_name(&second.status), "Pending");
    assert_eq!(second.group_num, None);
    assert_eq!(second.score, None);
}

#[test]
fn group_sub_tasks_groups_by_subtask_id_in_order() {
    let cases: Vec<types::TestCaseVO> = serde_json::from_value(serde_json::json!([
        { "id": 1, "subtaskId": 2, "status": 1 },
        { "id": 2, "subtaskId": 1, "status": 1 },
        { "id": 3, "status": 1 },
        { "id": 4, "subtaskId": 2, "status": 2 }
    ]))
    .unwrap();
    let mapped: Vec<JudgeCase> = cases
        .iter()
        .enumerate()
        .map(|(index, case)| HydroAdapter::into_judge_case(case, index))
        .collect();

    let groups = HydroAdapter::group_sub_tasks(&mapped);
    assert_eq!(groups.len(), 2, "无 subtaskId 的测试点不参与分组");
    assert_eq!(groups[0].group_num, 1);
    assert_eq!(groups[0].cases.len(), 1);
    assert_eq!(groups[1].group_num, 2);
    assert_eq!(groups[1].cases.len(), 2);
    assert_eq!(groups[1].cases[1].case_id, 4);
}

#[test]
fn group_sub_tasks_is_empty_for_flat_judging() {
    let cases: Vec<types::TestCaseVO> =
        serde_json::from_value(serde_json::json!([{ "id": 1, "status": 1 }])).unwrap();
    let mapped: Vec<JudgeCase> = cases
        .iter()
        .enumerate()
        .map(|(index, case)| HydroAdapter::into_judge_case(case, index))
        .collect();
    assert!(HydroAdapter::group_sub_tasks(&mapped).is_empty());
}

// ── 内部状态 ──

#[test]
fn adapter_identity_state_round_trips() {
    let http = Arc::new(HttpClient::new().expect("HttpClient 应可构造"));
    let adapter = HydroAdapter::new(http, "https://hydro.ac/".to_string());

    assert_eq!(adapter.base_url, "https://hydro.ac", "尾部斜杠应被去掉");
    assert!(adapter.session().is_none());
    assert!(matches!(adapter.require_session(), Err(AppError::Auth(_))));

    adapter.restore_token("0123456789abcdef0123456789abcdef");
    assert_eq!(
        adapter.session().as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert!(adapter.require_session().is_ok());
    // 空 token 不回注（AuthService 在会话文件损坏时可能传空串）
    adapter.restore_token("");
    assert!(adapter.session().is_some());

    adapter.clear_identity();
    assert!(adapter.session().is_none());
    assert!(adapter.cached_user().is_none());
}

#[test]
fn pid_cache_skips_empty_lists_and_hits_after_store() {
    let http = Arc::new(HttpClient::new().expect("HttpClient 应可构造"));
    let adapter = HydroAdapter::new(http, "https://hydro.ac".to_string());

    adapter.store_pids("tid", &[]);
    assert!(adapter.cached_pids("tid").is_none(), "空顺序表不缓存");

    adapter.store_pids("tid", &["1000".to_string()]);
    assert_eq!(adapter.cached_pids("tid"), Some(vec!["1000".to_string()]));

    // 键按比赛隔离（不同比赛的顺序表不得串号）
    assert!(adapter.cached_pids("other-tid").is_none());
    // 过期语义由 infra `TtlCache` 承担（见 infra/tests/cache_tests.rs），
    // 适配器只需保证「写入即命中、空表不写、键隔离」
}

#[test]
fn url_joins_without_double_slash() {
    let http = Arc::new(HttpClient::new().expect("HttpClient 应可构造"));
    let adapter = HydroAdapter::new(http, "https://hydro.ac/".to_string());
    assert_eq!(adapter.url("/login"), "https://hydro.ac/login");
    assert_eq!(
        adapter.url("/p/P1000?tid=abc"),
        "https://hydro.ac/p/P1000?tid=abc"
    );
}

// ── 共用解析入口（GET / POST 两条通道共用） ──

#[test]
fn parse_value_maps_error_envelope_to_auth() {
    let body = include_str!("fixtures/error_privilege.json");
    let err = HydroResponse::parse_value(body, "https://hydro.ac/p/P1000").unwrap_err();
    assert!(matches!(err, AppError::Auth(_)), "未登录必须映射为 Auth");
}

#[test]
fn parse_value_detects_login_redirect_on_get_path() {
    // GET 走 infra，非 2xx 已在 infra 内变成 Err；而「匿名」在 Hydro 里是
    // HTTP 200 + {"url":"/login?…"}，必须由本入口识别，否则会话过期会被
    // 当成「成功但数据为空」，前端永远回不到登录页
    let body = include_str!("fixtures/login_redirect.json");
    let err = HydroResponse::parse_value(body, "https://hydro.ac/p/P1000").unwrap_err();
    assert!(matches!(err, AppError::Auth(_)));
}

#[test]
fn parse_value_reports_non_json_body_as_serialization() {
    let err = HydroResponse::parse_value("<html>oops</html>", "https://hydro.ac/x").unwrap_err();
    match err {
        AppError::Serialization(msg) => {
            assert!(
                msg.contains("不是合法 JSON"),
                "应指明是 JSON 解析问题: {}",
                msg
            );
        }
        other => panic!("应为 Serialization，实际 {:?}", other),
    }
}

#[test]
fn parse_value_passes_through_success_payload() {
    let body = include_str!("fixtures/problem_detail.json");
    let value = HydroResponse::parse_value(body, "https://hydro.ac/p/P1000").expect("应解析成功");
    assert_eq!(value["pdoc"]["pid"], serde_json::json!("P1000"));
    // strip_nulls 生效：夹具里的 null 成员已被剔除
    assert!(value["pdoc"].get("difficulty").is_none());
}

// ── 工厂契约 ──

fn test_deps(tag: &str) -> Guarded<AdapterDeps> {
    let dir = TempDir::named(&format!("hinina-test-hydro-{}", tag));
    let deps = crate::adapter::test_adapter_deps(Arc::new(crate::infra::storage::Storage::new(
        dir.to_path_buf(),
    )));
    Guarded::new(deps, dir)
}

/// Hydro 身份契约：id 决定会话文件名，且必须与配置实例的 `id` 一致。
#[test]
fn hydro_factory_id_matches_session_file_contract() {
    assert_eq!(HydroAdapter::ID, "Hydro");
    assert_eq!(
        crate::core::provider::oj_id::OjId::new(HydroAdapter::ID).session_file(),
        "Hydro.json"
    );
    assert_eq!(FACTORY.id(), HydroAdapter::ID);
}

/// 工厂四能力齐备（Hydro 是完整适配器，不是骨架）。
///
/// 通用断言（`adapter::tests::factory_ids_unique_and_buildable`）为保住
/// 「先实现部分接口」的扩展路径只查「至少一个能力」，故完整能力在此显式锁定 ——
/// 漏装能力不该等到运行期 `ProviderNotFound`。
#[test]
fn hydro_factory_provides_all_four_capabilities() {
    let deps = test_deps("factory-capabilities");
    let set = FACTORY.build(&deps, "https://hydro.ac/");
    assert!(set.auth.is_some(), "Hydro 缺 Auth 能力");
    assert!(set.contest.is_some(), "Hydro 缺 Contest 能力");
    assert!(set.problem.is_some(), "Hydro 缺 Problem 能力");
    assert!(set.submission.is_some(), "Hydro 缺 Submission 能力");
    // base_url 的尾斜杠由适配器构造时归一
    let adapter = HydroAdapter::new(Arc::clone(&deps.http_client), "https://hydro.ac/".into());
    assert_eq!(adapter.url("/login"), "https://hydro.ac/login");
}
