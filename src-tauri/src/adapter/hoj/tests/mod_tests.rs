// HOJ mod.rs 单元测试：parse_time / parse_samples

use super::*;

// ── parse_time ──

#[test]
fn parse_time_iso_format() {
    // "2024-01-01T08:00:00" = 2024-01-01 08:00:00 UTC
    let ts = HOJAdapter::parse_time("2024-01-01T08:00:00");
    // (2024-1970)*365 + leap(13) + MONTH_DAYS[0] + 0 = 19710 + 13 = 19723 days
    // 19723 * 86400 + 8 * 3600 = 1704096000
    assert_eq!(ts, 1_704_096_000);
}

#[test]
fn parse_time_space_separated() {
    let ts = HOJAdapter::parse_time("2024-01-01 08:00:00");
    assert_eq!(ts, 1_704_096_000);
}

#[test]
fn parse_time_leap_year_march() {
    // 2024 is leap year, month=3 > 2 → +1 day
    // (2024-1970)*365 + 13 + 59 + 0 + 1 = 19710 + 13 + 59 + 1 = 19783 days
    // 19783 * 86400 + 12 * 3600 = 1709294400
    let ts = HOJAdapter::parse_time("2024-03-01T12:00:00");
    assert_eq!(ts, 1_709_294_400);
}

#[test]
fn parse_time_non_leap_year_march() {
    // 2023 is not leap year, month=3 no adjustment
    // (2023-1970)*365 + 13 + 59 = 19345 + 13 + 59 = 19417 days
    // 19417 * 86400 + 12 * 3600 = 1677672000
    let ts = HOJAdapter::parse_time("2023-03-01T12:00:00");
    assert_eq!(ts, 1_677_672_000);
}

#[test]
fn parse_time_epoch_relative() {
    // 2000-01-01 00:00:00
    // (2000-1970)*365 + 7 = 10950 + 7 = 10957 days
    // 10957 * 86400 = 946684800
    let ts = HOJAdapter::parse_time("2000-01-01T00:00:00");
    assert_eq!(ts, 946_684_800);
}

#[test]
fn parse_time_empty() {
    assert_eq!(HOJAdapter::parse_time(""), 0);
}

#[test]
fn parse_time_too_short() {
    assert_eq!(HOJAdapter::parse_time("2024"), 0);
}

// ── parse_samples ──

#[test]
fn parse_samples_empty() {
    let samples = HOJAdapter::parse_samples("");
    assert!(samples.is_empty());
}

#[test]
fn parse_samples_single_pair() {
    let html = "<input>3 4</input><output>7</output>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "3 4");
    assert_eq!(samples[0].output, "7");
}

#[test]
fn parse_samples_two_pairs() {
    let html = "<input>A</input><output>B</output><input>C</input><output>D</output>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].input, "A");
    assert_eq!(samples[0].output, "B");
    assert_eq!(samples[1].input, "C");
    assert_eq!(samples[1].output, "D");
}

#[test]
fn parse_samples_input_only() {
    let html = "<input>1 2</input>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "1 2");
    assert_eq!(samples[0].output, "");
}

#[test]
fn parse_samples_multiline_input() {
    // 实际 HOJ 数据：输入包含多行测试用例
    let html = "<input>6\n4\n1 0 0 1\n6\n0 1 1 1</input><output>Brick\nAgain</output>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "6\n4\n1 0 0 1\n6\n0 1 1 1");
    assert_eq!(samples[0].output, "Brick\nAgain");
}

#[test]
fn parse_samples_html_entities() {
    let html = "<input>&lt;int&gt; &amp; &quot;str&quot;</input><output>&nbsp;ok</output>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, r#"<int> & "str""#);
    assert_eq!(samples[0].output, " ok");
}

#[test]
fn parse_samples_br_tags() {
    let html = "<input>line1<br>line2<br/>line3</input><output>out</output>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "line1\nline2\nline3");
    assert_eq!(samples[0].output, "out");
}

#[test]
fn parse_samples_trim_whitespace() {
    let html = "<input>\n  hello  \n</input><output>  world  </output>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "hello");
    assert_eq!(samples[0].output, "world");
}

// ── parse_cid ──

#[test]
fn parse_cid_accepts_numeric_id() {
    assert_eq!(HOJAdapter::parse_cid("123").unwrap(), 123);
}

#[test]
fn parse_cid_rejects_invalid_id_instead_of_falling_back_to_zero() {
    // HOJ 以 cid = 0 表示「非比赛场景」：静默回退会让比赛中的提交落到练习题库，
    // 不计入榜单，而选手在赛场上完全无从察觉 —— 必须显式报错
    for invalid in ["", "abc", "12.5", "12345678901234567890123456789"] {
        let err = HOJAdapter::parse_cid(invalid)
            .expect_err(&format!("非法比赛 ID {:?} 应报错", invalid));
        assert!(
            matches!(err, AppError::Contest(_)),
            "非法比赛 ID {:?} 应报 Contest 错误，实际 {:?}",
            invalid,
            err
        );
    }
}

// ── parse_hoj_json / preview ──

#[test]
fn parse_hoj_json_strips_nulls_before_typing() {
    // 登录页故障的最小复现：sealRank 为 null 时，未经处理会让整页比赛列表解析失败
    let body = r#"{ "status": 200, "msg": "success", "data": { "records": [{ "id": 7, "sealRank": null, "rankShowName": null }], "total": 1 } }"#;
    let resp: ApiResponse<types::PageResult<ContestVO>> =
        HOJAdapter::parse_hoj_json(body, "http://oj/api/get-contest-list").expect("应能解析");
    assert!(resp.is_success());
    let page = resp.into_data().expect("data 非空");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].id, 7);
    assert!(!page.records[0].seal_rank);
}

#[test]
fn parse_hoj_json_reports_invalid_json_as_serialization_with_preview() {
    // 网关 502 会返回 HTML：必须报「不是合法 JSON」并带上响应前缀，
    // 而不是笼统的网络错误 —— 两者的处置方向完全不同
    let body = "<html><head><title>502 Bad Gateway</title></head></html>";
    let err = HOJAdapter::parse_hoj_json::<ApiResponse<ContestVO>>(body, "http://oj/api/x")
        .expect_err("HTML 不应被当成 JSON");
    match err {
        AppError::Serialization(msg) => {
            assert!(msg.contains("不是合法 JSON"), "应说明是 JSON 语法问题: {}", msg);
            assert!(msg.contains("502 Bad Gateway"), "应带响应体前缀便于定位: {}", msg);
            assert!(msg.contains("http://oj/api/x"), "应带上出错的 URL: {}", msg);
        }
        other => panic!("应为 Serialization 变体，实际 {:?}", other),
    }
}

#[test]
fn parse_hoj_json_reports_shape_mismatch_as_serialization_not_network() {
    // 回归：此前所有解析失败都被外层包成 AppError::Network，
    // 现场看到「网络错误: … 序列化错误: …」这种自相矛盾的嵌套消息，把 DTO 问题当断网查
    let body = r#"{ "status": 200, "data": { "records": "不是数组" } }"#;
    let err = HOJAdapter::parse_hoj_json::<ApiResponse<types::PageResult<ContestVO>>>(
        body,
        "http://oj/api/x",
    )
    .expect_err("records 类型不符应报错");
    assert!(
        matches!(err, AppError::Serialization(_)),
        "应为 Serialization 变体，实际 {:?}",
        err
    );
}

#[test]
fn preview_truncates_by_chars_not_bytes() {
    assert_eq!(preview("  短文本  "), "短文本", "应去除首尾空白");

    let long: String = "比赛".repeat(300);
    let p = preview(&long);
    assert!(p.ends_with('…'), "超长响应应以省略号结尾");
    // 200 个字符 + 省略号；按字节截断会切在 UTF-8 序列中间并 panic
    assert_eq!(p.chars().count(), 201);
}

// ── 真实响应夹具（脱敏）──
//
// fixtures/contest_list_anon.json 取自 `GET /api/get-contest-list?limit=1000` 的真实响应，
// 仅替换标题/作者/简介等自由文本，完整保留键名、数字、布尔与 null 分布。
// 实测值为 null 的字段：count / gid / now / openPrint / rankShowName / sealRank / sealRankTime。
const REAL_CONTEST_LIST: &str = include_str!("fixtures/contest_list_anon.json");

#[test]
fn real_anonymous_contest_list_response_parses() {
    // 登录页匿名简报的完整解析路径：这条测试失败就等于登录页拿不到比赛列表
    let resp: ApiResponse<types::PageResult<ContestVO>> =
        HOJAdapter::parse_hoj_json(REAL_CONTEST_LIST, "http://oj/api/get-contest-list")
            .expect("真实响应应能解析（失败即复现登录页「获取比赛列表失败」）");

    assert!(resp.is_success(), "HOJ 的成功码是 200 而不是 0");
    let page = resp.into_data().expect("data 非空");
    assert_eq!(page.records.len(), 13);
    assert_eq!(page.total, 13);
    assert_eq!(page.size, 500, "实测 size=500，与请求的 limit=1000 并不相同");

    // null 字段一律落到默认值，而不是让整页解析失败
    let null_heavy = page
        .records
        .iter()
        .find(|c| c.id == 1012)
        .expect("夹具含 id=1012");
    assert!(!null_heavy.seal_rank);
    assert_eq!(null_heavy.rank_show_name, None);
    assert_eq!(null_heavy.seal_rank_time, None);
    assert_eq!(null_heavy.status, -1, "未开始");

    // 已配置的比赛必须能从列表里筛出来（登录页简报正是按 contestId 筛选）
    assert!(page.records.iter().any(|c| c.id == 1011));
}

#[test]
fn real_anonymous_contest_list_fails_without_null_stripping() {
    // 反向锁定：不去 null 就必然失败 —— 防止后来者把 strip_nulls 当成冗余「优化」掉
    let err = serde_json::from_str::<ApiResponse<types::PageResult<ContestVO>>>(REAL_CONTEST_LIST)
        .expect_err("含显式 null 的响应直接解析应当失败");
    assert!(
        err.to_string().contains("null"),
        "失败原因应是 null 与目标类型不匹配，实际: {}",
        err
    );
}

// ── auth_failure_from_body ──
//
// HOJ 把鉴权失败放在响应体（HTTP 仍是 200），必须翻译成 AppError::Auth，
// 否则前端 sessionGuard 依据 variant==='Auth' 的会话失效兜底不会触发。

fn body(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("夹具应是合法 JSON")
}

#[test]
fn body_403_login_prompt_is_auth_failure() {
    // 实测：匿名访问 get-contest-problem 返回 HTTP 200 + status=403 + "请您先登录！"
    let err = HOJAdapter::auth_failure_from_body(&body(
        r#"{ "status": 403, "msg": "请您先登录！", "data": null }"#,
    ))
    .expect("应识别为认证失败");
    assert!(matches!(err, AppError::Auth(_)), "实际 {:?}", err);
    assert!(err.user_message().contains("请您先登录"));
}

#[test]
fn body_401_is_always_auth_failure() {
    // 401 语义明确，即使消息里没提登录也按会话失效处理
    let err = HOJAdapter::auth_failure_from_body(&body(r#"{ "status": 401, "msg": "unauthorized" }"#))
        .expect("应识别为认证失败");
    assert!(matches!(err, AppError::Auth(_)));
}

#[test]
fn body_403_permission_error_is_not_auth_failure() {
    // 保守判定的意义：私有赛未注册/需要密码不是会话问题，
    // 误判会把已登录的选手踢回登录页
    for msg in ["该比赛需要密码", "您没有权限访问该比赛", "contest not registered"] {
        let json = format!(r#"{{ "status": 403, "msg": "{}" }}"#, msg);
        assert!(
            HOJAdapter::auth_failure_from_body(&body(&json)).is_none(),
            "业务性 403 不应被判为会话失效: {}",
            msg
        );
    }
}

#[test]
fn body_success_and_other_errors_are_not_auth_failure() {
    assert!(HOJAdapter::auth_failure_from_body(&body(r#"{ "status": 200, "msg": "success" }"#)).is_none());
    assert!(HOJAdapter::auth_failure_from_body(&body(r#"{ "status": 400, "msg": "参数错误" }"#)).is_none());
    assert!(HOJAdapter::auth_failure_from_body(&body(r#"{ "status": 500, "msg": "服务器异常" }"#)).is_none());
    assert!(HOJAdapter::auth_failure_from_body(&body(r#"{ "msg": "缺少 status" }"#)).is_none());
}

#[test]
fn parse_hoj_json_surfaces_body_auth_failure_as_auth_variant() {
    // 端到端：调用点拿到的是 Auth 变体，而不是被包成 Contest/Serialization
    let err = HOJAdapter::parse_hoj_json::<ApiResponse<types::PageResult<ContestVO>>>(
        r#"{ "status": 403, "msg": "请您先登录！", "data": null }"#,
        "http://oj/api/get-contest-problem",
    )
    .expect_err("应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "认证失败必须保留 Auth 变体，实际 {:?}",
        err
    );
}

// ── session_validity_from_response（三态契约）──
//
// 这条判据直接决定选手会不会在赛前被误踢回登录页：
// Ok(false) 会让 AuthService 清磁盘会话 + 发布 SessionExpired + 前端登出，
// 而反复重登可能触发 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。
// 因此「无法判定」必须走 Err（映射为 Unknown、保留本地会话），绝不能折成 Ok(false)。

fn ok_resp(status: i32) -> AppResult<ApiResponse<serde_json::Value>> {
    Ok(ApiResponse {
        status,
        msg: Some("success".into()),
        data: None,
    })
}

#[test]
fn validity_success_is_valid() {
    assert_eq!(
        HOJAdapter::session_validity_from_response(ok_resp(200)).expect("不应报错"),
        true
    );
}

#[test]
fn validity_auth_error_is_definitively_invalid() {
    // 两条路径都归到 Auth 变体：HTTP 401（infra status_error）
    // 与 HTTP 200 + 体内 status=403「请您先登录！」（parse_hoj_json）
    for err in [
        AppError::Auth("HTTP 401 Unauthorized: http://oj/api/get-user-auth-info".into()),
        AppError::Auth("请您先登录！（HOJ status=403）".into()),
    ] {
        assert_eq!(
            HOJAdapter::session_validity_from_response(Err(err)).expect("不应报错"),
            false,
            "服务端明确判定失效应返回 Ok(false)"
        );
    }
}

#[test]
fn validity_network_error_propagates_as_unknown_not_invalid() {
    // 回归：此前 `Err(_) => Ok(false)` 把网络异常当成「服务端判定失效」，
    // 使 SessionValidity::Unknown 分支对 HOJ 成为死代码
    let err = HOJAdapter::session_validity_from_response(Err(AppError::Network(
        "GET 请求失败: connection reset".into(),
    )))
    .expect_err("网络异常必须上抛，不能折成 Ok(false)");
    assert!(
        matches!(err, AppError::Network(_)),
        "变体应保留为 Network，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("HOJ 会话校验"),
        "应带上环节名: {}",
        err.user_message()
    );
}

#[test]
fn validity_serialization_error_propagates_not_invalid() {
    // 响应解析失败同样属于「无法判定」：DTO 与服务端不匹配不代表 token 失效
    let err = HOJAdapter::session_validity_from_response(Err(AppError::Serialization(
        "HOJ 响应字段不匹配".into(),
    )))
    .expect_err("解析失败必须上抛");
    assert!(matches!(err, AppError::Serialization(_)), "实际 {:?}", err);
}

#[test]
fn validity_non_success_body_status_is_unknown() {
    // status=500/400 既不是鉴权失败也不是成功，无法据此断定会话状态
    for status in [400, 500] {
        let err = HOJAdapter::session_validity_from_response(ok_resp(status))
            .expect_err(&format!("status={} 应上抛为无法判定", status));
        assert!(
            matches!(err, AppError::Unknown(_)),
            "status={} 应为 Unknown 变体，实际 {:?}",
            status,
            err
        );
    }
}

// ── 公告 / 提交列表 / 测试点夹具（按文档构造，未经真实联调校正 —— P57 待联调清单）──
//
// 以下三个夹具均按 doc/HOJ/HOJ-API-Documentation.md §3.7 / §3.9 / §5.6 的响应形状
// 手工构造，保留了 HOJ 的显式 null 风格与 status:200 信封；字段名与真实服务端的
// 出入需在联调时校正（P57）。

const ANNOUNCEMENT_LIST: &str = include_str!("fixtures/announcement_list.json");
const CONTEST_SUBMISSIONS: &str = include_str!("fixtures/contest_submissions.json");
const CASE_RESULT: &str = include_str!("fixtures/case_result.json");

#[test]
fn announcement_list_fixture_parses_and_maps() {
    // 按文档构造，未经真实联调校正 —— P57 待联调清单
    let resp: ApiResponse<types::PageResult<AnnouncementVO>> =
        HOJAdapter::parse_hoj_json(ANNOUNCEMENT_LIST, "http://oj/api/get-contest-announcement")
            .expect("公告夹具应能解析");
    let page = resp.into_data().expect("data 非空");
    assert_eq!(page.records.len(), 2);
    assert_eq!(page.total, 2);
    assert_eq!(page.pages, 1);

    let first = HOJAdapter::into_announcement(page.records.into_iter().next().unwrap());
    assert_eq!(first.id, "9001", "id 应转为字符串");
    assert_eq!(first.title, "开赛通知");
    assert_eq!(first.author, "admin", "username 映射为 author");
    assert!(first.content.contains("比赛已开始"));
    // 时间为秒级时间戳，且与 parse_time 同一算法
    assert_eq!(first.created_at, HOJAdapter::parse_time("2026-09-15T08:00:00"));
    assert!(first.created_at > 1_700_000_000);
    assert!(first.updated_at >= first.created_at);
}

#[test]
fn announcement_null_content_and_times_degrade_to_defaults() {
    // 按文档构造，未经真实联调校正 —— P57 待联调清单
    // 第二条公告 content / updateTime / uid 均为 null：去 null 后落默认值而不是解析失败
    let resp: ApiResponse<types::PageResult<AnnouncementVO>> =
        HOJAdapter::parse_hoj_json(ANNOUNCEMENT_LIST, "http://oj/api/get-contest-announcement")
            .expect("公告夹具应能解析");
    let mut page = resp.into_data().expect("data 非空");
    let second = page.records.remove(1);
    assert_eq!(second.id, 9002);
    let a = HOJAdapter::into_announcement(second);
    assert_eq!(a.content, "", "content 为 null 应回退空串");
    assert_eq!(a.updated_at, 0, "updateTime 为 null 应回退 0");

    // 反向锁定：不去 null 直接解析必然失败（strip_nulls 不可省）
    assert!(
        serde_json::from_str::<ApiResponse<types::PageResult<AnnouncementVO>>>(ANNOUNCEMENT_LIST)
            .is_err(),
        "含显式 null 的公告响应直接解析应当失败"
    );
}

#[test]
fn contest_submissions_fixture_parses_and_maps() {
    // 按文档构造，未经真实联调校正 —— P57 待联调清单
    let resp: ApiResponse<types::PageResult<JudgeVO>> =
        HOJAdapter::parse_hoj_json(CONTEST_SUBMISSIONS, "http://oj/api/contest-submissions")
            .expect("提交列表夹具应能解析");
    let page = resp.into_data().expect("data 非空");
    assert_eq!(page.records.len(), 2);
    assert_eq!(page.size, 20);

    let records: Vec<SubmissionRecord> =
        page.records.into_iter().map(HOJAdapter::into_submission_record).collect();

    let r1 = &records[0];
    assert_eq!(r1.submit_id, "12345", "submitId i64 → String");
    assert_eq!(r1.pid, "1061", "pid i64 → String");
    assert_eq!(r1.display_pid, "HOJ-1061");
    assert_eq!(r1.display_id, "A");
    assert_eq!(r1.title, "A + B Problem");
    assert!(matches!(r1.status, JudgementStatus::Accepted));
    assert_eq!(r1.time_ms, 15);
    assert_eq!(r1.memory_kb, 10240);
    assert_eq!(r1.length, 256);
    assert_eq!(r1.submit_time, HOJAdapter::parse_time("2026-09-15T10:00:00"));

    // 第二条：status=13 → PartiallyAccepted（P41：不再折算 AC），null 数值全部落 0
    let r2 = &records[1];
    assert!(
        matches!(r2.status, JudgementStatus::PartiallyAccepted),
        "PA 应有独立变体，实际 {:?}",
        r2.status
    );
    assert_eq!(r2.display_id, "", "displayId 为 null 应回退空串");
    assert_eq!(r2.time_ms, 0);
    assert_eq!(r2.memory_kb, 0);
    assert_eq!(r2.length, 0);
    assert_eq!(r2.score, Some(60.0));
}

#[test]
fn contest_submissions_fixture_fails_without_null_stripping() {
    // 按文档构造，未经真实联调校正 —— P57 待联调清单
    let err = serde_json::from_str::<ApiResponse<types::PageResult<JudgeVO>>>(CONTEST_SUBMISSIONS)
        .expect_err("含显式 null 的提交列表直接解析应当失败");
    assert!(err.to_string().contains("null"), "实际: {}", err);
}

#[test]
fn case_result_fixture_parses_and_maps() {
    // 按文档构造，未经真实联调校正 —— P57 待联调清单
    let resp: ApiResponse<types::JudgeCaseVO> =
        HOJAdapter::parse_hoj_json(CASE_RESULT, "http://oj/api/get-all-case-result")
            .expect("测试点夹具应能解析");
    let vo = resp.into_data().expect("data 非空");
    assert_eq!(vo.judge_case_mode.as_deref(), Some("default"));
    assert!(
        vo.sub_task_judge_case_vo_list.is_empty(),
        "subTaskJudgeCaseVoList 为 null 时应落空列表"
    );

    let cases: Vec<JudgeCase> = types::lenient_case_list(&vo.judge_case_list)
        .into_iter()
        .map(HOJAdapter::into_judge_case)
        .collect();
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].case_id, 1);
    assert_eq!(cases[0].seq, 1);
    assert!(matches!(cases[0].status, JudgementStatus::Accepted));
    assert_eq!(cases[0].time_ms, 10);
    assert_eq!(cases[0].memory_kb, 5120);
    assert_eq!(cases[0].group_num, None);
    assert_eq!(cases[1].case_id, 2);
}

#[test]
fn case_result_malformed_entries_are_skipped_not_fatal() {
    // 按文档构造，未经真实联调校正 —— P57 待联调清单
    // SubTask 形态文档不完整：单条测试点/单个分组类型异常时只跳过该条，
    // 绝不让整个响应解析失败（测试点面板降级展示好过整页报错）
    let body = r#"{
        "status": 200, "msg": "success",
        "data": {
            "judgeCaseList": [
                { "caseId": 1, "status": 5, "time": 10, "memory": 100, "seq": 1 },
                "不是测试点对象",
                { "caseId": 3, "status": "不是数字" }
            ],
            "subTaskJudgeCaseVoList": [
                { "groupNum": 1, "judgeCaseList": [{ "caseId": 9, "status": 4, "seq": 1, "groupNum": 1 }] },
                42
            ],
            "judgeCaseMode": "subtask_lowest"
        }
    }"#;
    let resp: ApiResponse<types::JudgeCaseVO> =
        HOJAdapter::parse_hoj_json(body, "http://oj/api/get-all-case-result").expect("应能解析");
    let vo = resp.into_data().expect("data 非空");

    let cases = types::lenient_case_list(&vo.judge_case_list);
    assert_eq!(cases.len(), 1, "两条异常测试点应被跳过");
    assert_eq!(cases[0].case_id.unwrap(), 1);

    let sub_tasks: Vec<types::SubTaskDTO> = vo
        .sub_task_judge_case_vo_list
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    assert_eq!(sub_tasks.len(), 1, "非对象分组应被跳过");
    assert_eq!(sub_tasks[0].group_num, Some(1));
    assert_eq!(types::lenient_case_list(&sub_tasks[0].judge_case_list).len(), 1);
    assert_eq!(vo.judge_case_mode.as_deref(), Some("subtask_lowest"));
}

// ── 提交详情 DTO → 完整实体映射 ──

#[test]
fn submission_detail_maps_full_entity() {
    // get_judgement 只投影轮询所需的四个字段；提交详情面板需要完整实体
    let body = r#"{
        "status": 200, "msg": "success",
        "data": {
            "submission": {
                "submitId": 12345, "pid": 1061, "displayPid": "HOJ-1061",
                "uid": "uuid-alice", "username": "alice",
                "submitTime": "2026-09-15T10:00:00", "status": 2,
                "errorMessage": "error: expected ';' before '}' token",
                "time": null, "memory": null, "score": null,
                "length": 256, "language": "C++",
                "code": "int main(){}", "cid": 1011,
                "judger": null, "oiRankScore": null
            },
            "codeShare": true
        }
    }"#;
    let resp: ApiResponse<types::SubmissionInfoVO> =
        HOJAdapter::parse_hoj_json(body, "http://oj/api/get-submission-detail").expect("应能解析");
    let info = resp.into_data().expect("data 非空");
    let detail = HOJAdapter::into_submission_detail(info.submission);

    assert_eq!(detail.submit_id, "12345");
    assert_eq!(detail.pid, "1061");
    assert_eq!(detail.username, "alice");
    assert_eq!(detail.submit_time, HOJAdapter::parse_time("2026-09-15T10:00:00"));
    assert!(matches!(detail.status, JudgementStatus::CompilationError));
    assert_eq!(detail.code, "int main(){}");
    assert!(detail.error_message.unwrap().contains("expected ';'"));
    assert_eq!(detail.language, "C++");
    assert_eq!(detail.length, 256);
    // 评测未完成：time/memory 为 null 落 0
    assert_eq!(detail.time_ms, 0);
    assert_eq!(detail.memory_kb, 0);
    assert_eq!(detail.judger, None);
    assert_eq!(detail.oi_rank_score, None);
}

#[test]
fn submission_detail_null_code_degrades_to_empty_string() {
    // 未开分享或权限不足时 code 可能为 null：详情面板显示空而不是整页报错
    let body = r#"{
        "status": 200,
        "data": {
            "submission": {
                "submitId": 1, "pid": 2, "displayPid": "HOJ-2", "username": "u",
                "submitTime": null, "status": 5, "code": null, "language": null
            }
        }
    }"#;
    let resp: ApiResponse<types::SubmissionInfoVO> =
        HOJAdapter::parse_hoj_json(body, "http://oj/api/get-submission-detail").expect("应能解析");
    let detail = HOJAdapter::into_submission_detail(resp.into_data().expect("data 非空").submission);
    assert_eq!(detail.code, "");
    assert_eq!(detail.language, "");
    assert_eq!(detail.submit_time, 0, "submitTime 为 null 应回退 0");
}

// ── ContestVO.oiRankScoreType → Contest 实体 ──

#[test]
fn contest_vo_maps_oi_rank_score_type() {
    let vo: ContestVO = serde_json::from_str(
        r#"{ "id": 1011, "title": "OI 赛", "oiRankScoreType": "Highest" }"#,
    )
    .expect("解析失败");
    let contest = HOJAdapter::into_contest(vo);
    assert_eq!(contest.oi_rank_score_type.as_deref(), Some("Highest"));

    // 非 OI 赛 / 列表接口不返回该字段 → None
    let vo: ContestVO =
        serde_json::from_str(r#"{ "id": 1012, "title": "ACM 赛" }"#).expect("解析失败");
    assert_eq!(HOJAdapter::into_contest(vo).oi_rank_score_type, None);
}
