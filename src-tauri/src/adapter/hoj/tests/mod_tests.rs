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
