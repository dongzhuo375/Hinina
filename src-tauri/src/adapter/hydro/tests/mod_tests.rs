// Hydro mod.rs 单元测试：响应处置、映射 helper、评测记录投影。
//
// 夹具均为按 `doc/Hydro/HYDRO-API.md` 手工构造（无可用联调实例）。

use super::*;
use crate::core::entity::submission::JudgementStatus;
use crate::test_support::{Guarded, TempDir};

#[tokio::test]
async fn plugin_history_uses_uid_when_display_name_differs_or_udict_is_absent() {
    use crate::core::entity::submission::SubmissionHistoryPage;
    use crate::plugin::{
        api::submission::PluginSubmissionQuery, permission::PluginGrant, query::ScopedQueries,
        test_support::*,
    };
    for missing_udict in [false, true] {
        let mut value = fixture("record_list");
        value["udict"]["2"]["displayName"] = serde_json::json!("A Different Display Name");
        let vo: RecordListVO = serde_json::from_value(value).unwrap();
        let rdoc = &vo.rdocs.as_ref().unwrap()[0];
        let udict = if missing_udict {
            HashMap::new()
        } else {
            vo.udict.unwrap()
        };
        let record = HydroAdapter::into_submission_record(
            rdoc,
            &vo.pdict.unwrap_or_default(),
            &udict,
            &["1000".into()],
            true,
        );
        assert_eq!(record.owner_id.as_deref(), Some("2"));
        assert_eq!(
            record.username,
            if missing_udict {
                ""
            } else {
                "A Different Display Name"
            }
        );
        let source = Arc::new(Source::default());
        source.scope.lock().unwrap().user = Some(("2".into(), "admin".into()));
        *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
            records: vec![record.clone()],
            total: None,
            has_more: false,
        }];
        let m = manifest("test.hydro-owner");
        let queries = ScopedQueries::new(
            PluginGrant::authorize(&m, &m.permissions).unwrap(),
            source,
            Arc::default(),
            Default::default(),
        );
        let context = queries.current_context().await.unwrap();
        let page = queries
            .list(
                &context,
                PluginSubmissionQuery {
                    contest_id: "contest".into(),
                    problem_id: None,
                    cursor: None,
                    limit: 100,
                },
            )
            .await
            .unwrap();
        assert_eq!(page.records.len(), 1);
        assert!(queries
            .submission(&context, "contest", &record.submit_id)
            .await
            .unwrap()
            .is_some());
    }
}

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

// ── 本地 scripted HTTP 契约测试（M2 工作包 B）──
//
// 与上方 DTO/fixture 测试互补：打**真实网络栈**（reqwest → loopback server，
// `test_support::loopback`），锁定线上协议行为而非判定函数。

use crate::adapter::hydro::HydroAdapter;
use crate::core::provider::auth::AuthProvider;
use crate::infra::http::HttpClient;
use crate::test_support::loopback::{LoopbackServer, ScriptedResponse};
use std::sync::Arc;

fn loopback_adapter(base_url: &str) -> Arc<HydroAdapter> {
    Arc::new(HydroAdapter::new(
        Arc::new(HttpClient::with_timeout(std::time::Duration::from_secs(5)).unwrap()),
        base_url.to_string(),
    ))
}

/// 登录成功响应：Set-Cookie 下发 sid；body 是被序列化的重定向 {"url":"/"}。
fn login_ok(sid: &str) -> ScriptedResponse {
    ScriptedResponse::with(
        200,
        vec![(
            "Set-Cookie".into(),
            format!("sid={}; Path=/; HttpOnly", sid),
        )],
        r#"{"url":"/"}"#,
    )
}

/// 登录：POST /login（rememberme 开启），sid 取自 Set-Cookie；
/// 登录后的用户上下文探测失败时身份降级（uname 用输入值、id 空、token 为裸 sid）。
#[tokio::test]
async fn login_extracts_sid_and_next_request_carries_session() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-abc-123"),
        // 探测 GET / 失败（500）：身份降级路径 —— 不让一次探测失败否定已建立的会话
        ScriptedResponse::with(500, vec![], "boom"),
    ])
    .await;
    let adapter = loopback_adapter(server.url());

    let user = adapter
        .login("alice", "pw")
        .await
        .expect("探测失败不应否定登录");

    assert_eq!(user.username, "alice");
    assert_eq!(user.id, "", "探测失败时身份降级为空 id");
    assert_eq!(user.token, "sid-abc-123", "User.token 保存裸 sid");

    let reqs = server.requests();
    let login = &reqs[0];
    assert_eq!(login.method, "POST");
    assert_eq!(login.path, "/login");
    let body: serde_json::Value = serde_json::from_str(&login.body).unwrap();
    assert_eq!(body["uname"], "alice");
    assert_eq!(body["password"], "pw");
    assert_eq!(
        body["rememberme"], true,
        "赛场场景必须开启记住我（3h → 30d）"
    );
    assert!(
        login
            .header("accept")
            .unwrap_or("")
            .contains("application/json"),
        "Accept: application/json 是让重定向序列化为 200 的前提（否则丢 Set-Cookie）"
    );

    let probe = &reqs[1];
    assert_eq!(probe.method, "GET");
    assert_eq!(probe.path, "/");
    assert!(
        probe
            .header("cookie")
            .unwrap_or("")
            .contains("sid=sid-abc-123"),
        "后续请求应经 Cookie store 携带 sid"
    );
    assert_eq!(
        probe.header("authorization"),
        Some("Bearer sid-abc-123"),
        "持会话时附加 Bearer 头（该头一旦出现即覆盖 Cookie）"
    );

    server.shutdown();
}

/// 响应无 Set-Cookie: sid：认证失败（不是把无会话当成功）。
#[tokio::test]
async fn login_without_sid_cookie_fails_as_auth() {
    let server = LoopbackServer::start(vec![ScriptedResponse::json(r#"{"url":"/"}"#)]).await;
    let adapter = loopback_adapter(server.url());

    let err = adapter.login("alice", "pw").await.unwrap_err();
    assert!(
        matches!(err, crate::core::error::AppError::Auth(ref m) if m.contains("sid") || m.contains("Cookie")),
        "缺 sid 应为 Auth 错误，实得 {:?}",
        err
    );

    server.shutdown();
}

/// 登录降级身份不进已验证用户缓存（M2 评审 P2）：探测失败的登录仍成功，
/// 但随后的 validate_session 必须真实回访服务端 —— 探测失败判 Unknown（Err）、
/// 服务端给出身份后恢复 Ok(true)，而不是被占位用户缓存无条件判有效。
#[tokio::test]
async fn validate_session_does_not_trust_degraded_cached_identity() {
    // 探测走 raw GET：5xx 由 infra 重试（首次 + 2 次重试 = 一次探测 3 个请求）
    let probe_500 = || ScriptedResponse::with(500, vec![], "boom");
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        // 登录后的用户上下文探测失败（3 个请求）→ 身份降级（登录成功，uid 空）
        probe_500(),
        probe_500(),
        probe_500(),
        // validate_session 的探测同样失败（3 个请求）→ 三态契约的「无法判定」（Err）
        probe_500(),
        probe_500(),
        probe_500(),
        // 服务端恢复：给出真实身份 → 判有效
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
    ])
    .await;
    let adapter = loopback_adapter(server.url());

    adapter
        .login("alice", "pw")
        .await
        .expect("探测失败不应否定登录");
    // 登录 = POST + 1 次探测（探测 500 经 infra GET 重试，一次探测占多个请求，
    // 故这里只断言「探测确实发生过」，不与重试次数耦合）
    assert!(server.requests().len() >= 2);

    // 关键断言：若占位用户被写入了缓存，这里会直接 Ok(true) 且不发请求
    let requests_before_validate = server.requests().len();
    let verdict = adapter.validate_session().await;
    assert!(
        verdict.is_err(),
        "探测失败属「无法判定」，不得因降级身份缓存判有效: {:?}",
        verdict
    );
    assert!(
        server.requests().len() > requests_before_validate,
        "validate_session 必须真实回访服务端（缓存未命中）"
    );

    assert!(
        adapter
            .validate_session()
            .await
            .expect("服务端给出身份后应判有效"),
        "恢复路径应返回 Ok(true)"
    );

    server.shutdown();
}

// ── 会话校验与身份缓存（PR #52 评审 P0 两项）──

/// 已缓存身份的 validate_session 仍必须真实回访服务端（评审 P0-1）：
/// 身份缓存只服务展示查询；校验走缓存会让「服务器吊销 sid」在进程内不可见。
#[tokio::test]
async fn validate_session_revalidates_despite_cached_identity() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        // 登录探测成功：真实身份进缓存
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
        // validate 的全新探测：服务端不可达（3 个请求 = 首次 + 2 次重试）
        ScriptedResponse::with(500, vec![], "boom"),
        ScriptedResponse::with(500, vec![], "boom"),
        ScriptedResponse::with(500, vec![], "boom"),
    ])
    .await;
    let adapter = loopback_adapter(server.url());

    let user = adapter.login("alice", "pw").await.unwrap();
    assert_eq!(user.id, "7", "登录探测成功后身份应已缓存");
    let requests_after_login = server.requests().len();

    // 若校验错误地走了缓存，这里会直接 Ok(true) 且零请求
    let verdict = adapter.validate_session().await;
    assert!(
        verdict.is_err(),
        "服务端不可达属「无法判定」（Err），不得因缓存身份判有效: {:?}",
        verdict
    );
    assert!(
        server.requests().len() > requests_after_login,
        "validate_session 必须回访服务端，不得被身份缓存短路"
    );

    server.shutdown();
}

/// 服务端已吊销（匿名）：缓存的旧身份必须被推翻，判失效并清除（评审 P0-1）。
#[tokio::test]
async fn validate_session_reports_invalid_when_server_says_anonymous() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
        // validate 的全新探测：服务端判定匿名（_id == 0）
        ScriptedResponse::json(r#"{"UserContext":{"_id":0}}"#),
    ])
    .await;
    let adapter = loopback_adapter(server.url());

    adapter.login("alice", "pw").await.unwrap();

    let verdict = adapter.validate_session().await;
    assert!(
        !verdict.unwrap(),
        "服务端匿名 = 会话已失效，缓存身份不得掩盖"
    );

    server.shutdown();
}

/// 换号登录：新身份不得沿用上一个账号的缓存（评审 P0-2）。
#[tokio::test]
async fn login_switching_accounts_refreshes_cached_identity() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
        login_ok("sid-2"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":9,"uname":"bob"}}"#),
    ])
    .await;
    let adapter = loopback_adapter(server.url());

    let first = adapter.login("alice", "pw").await.unwrap();
    assert_eq!(first.id, "7");

    let second = adapter.login("bob", "pw").await.expect("换号登录应成功");
    assert_eq!(
        second.id, "9",
        "换号后身份必须来自新探测（旧缓存未清会返回 alice 的 7）"
    );
    assert_eq!(second.token, "sid-2");

    server.shutdown();
}

/// 探测进行中换号：慢响应晚到必须作废，不得把旧身份写回缓存、
/// 不得以旧会话的判定结论清除新会话（评审第三轮 #1/#2 的真实竞态）。
#[tokio::test]
async fn probe_response_during_account_switch_is_discarded() {
    let alice_ctx = || ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#);
    // 队列按请求序（login 自带一次探测，别忘了它）：
    // [0] login(A) POST → [1] 登录 A 的探测（立即，alice 身份入缓存）
    // → [2] validate(A) 的探测（延迟 600ms——它就是将被作废的过期响应）
    // → [3] login(B) POST → [4] 登录 B 的探测（bob 身份）
    let server = LoopbackServer::start(vec![
        login_ok("sid-A"),
        alice_ctx(),
        alice_ctx().delayed(std::time::Duration::from_millis(600)),
        login_ok("sid-B"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":9,"uname":"bob"}}"#),
        // 尾部：B 会话的再次校验（断言新会话不受晚到响应波及）
        ScriptedResponse::json(r#"{"UserContext":{"_id":9,"uname":"bob"}}"#),
    ])
    .await;
    let adapter = loopback_adapter(server.url());

    let alice = adapter.login("alice", "pw").await.unwrap();
    assert_eq!(alice.token, "sid-A");

    // 发起 validate(A)（携带 sid-A，响应延迟 600ms），等它的请求确实到达服务端
    let validating = Arc::clone(&adapter);
    let validate = tokio::spawn(async move { validating.validate_session().await });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while server.requests().len() < 3 && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(server.requests().len() >= 3, "validate 的探测应已发出");

    // 探测在途期间完成换号登录 B
    let bob = match adapter.login("bob", "pw").await {
        Ok(u) => u,
        Err(e) => {
            for (i, r) in server.requests().iter().enumerate() {
                eprintln!(
                    "REQ[{i}] {} {} auth={:?}",
                    r.method,
                    r.path,
                    r.header("authorization")
                );
            }
            panic!("login(B) 失败: {e}");
        }
    };
    assert_eq!(bob.token, "sid-B");
    assert_eq!(bob.id, "9", "B 的身份来自 B 的探测");

    // 晚到的 A 响应到达：代际保护必须作废它
    let verdict = validate
        .await
        .expect("validate 任务不应 panic")
        .expect_err("过期响应属「无法判定」，不得得出任何结论");
    assert!(
        verdict.to_string().contains("会话已变更"),
        "应因会话代际变更而作废: {}",
        verdict
    );

    // 新会话（B）不受影响：再次校验真实回访并判有效
    assert!(
        adapter.validate_session().await.expect("B 会话应判有效"),
        "换号后的新会话不得被 A 的晚到响应波及"
    );

    server.shutdown();
}
