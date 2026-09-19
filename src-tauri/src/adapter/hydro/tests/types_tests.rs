// Hydro types.rs 单元测试：响应归一化、错误包络、时间、状态码、语言、榜单单元格矩阵。
//
// 夹具均为**按 `doc/Hydro/HYDRO-API.md` 手工构造**（无可用联调实例），
// 键名与 null 分布严格照文档示例，用于锁定解析契约；真实联调见缺口报告待办清单。

use super::*;

fn load(name: &str) -> Value {
    let raw = match name {
        "contest_list" => include_str!("fixtures/contest_list.json"),
        "contest_problems" => include_str!("fixtures/contest_problems.json"),
        "problem_detail" => include_str!("fixtures/problem_detail.json"),
        "record_detail" => include_str!("fixtures/record_detail.json"),
        "record_list" => include_str!("fixtures/record_list.json"),
        "scoreboard_acm" => include_str!("fixtures/scoreboard_acm.json"),
        "scoreboard_oi" => include_str!("fixtures/scoreboard_oi.json"),
        "error_privilege" => include_str!("fixtures/error_privilege.json"),
        "login_redirect" => include_str!("fixtures/login_redirect.json"),
        other => panic!("未登记的夹具: {}", other),
    };
    let mut value: Value = serde_json::from_str(raw).expect("夹具必须是合法 JSON");
    strip_nulls(&mut value);
    value
}

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_value(load(name)).expect("夹具必须能被 DTO 解析")
}

// ── strip_nulls / preview ──

#[test]
fn strip_nulls_removes_nulls_recursively_but_keeps_falsy_values() {
    let mut value = serde_json::json!({
        "a": null,
        "b": false,
        "c": 0,
        "d": "",
        "nested": { "e": null, "f": 1 },
        "list": [null, 1, null],
    });
    strip_nulls(&mut value);

    assert!(value.get("a").is_none(), "null 成员必须被剔除");
    // false / 0 / "" 不是 null：剔除会抹掉封榜、打星、零分语义
    assert_eq!(value["b"], serde_json::json!(false));
    assert_eq!(value["c"], serde_json::json!(0));
    assert_eq!(value["d"], serde_json::json!(""));
    assert!(value["nested"].get("e").is_none());
    assert_eq!(value["nested"]["f"], serde_json::json!(1));
    assert_eq!(value["list"], serde_json::json!([1]));
}

#[test]
fn preview_truncates_by_chars_not_bytes() {
    let body = "中".repeat(300);
    let out = preview(&body);
    // 200 个字符 + 省略号；按字节截断会切出乱码
    assert_eq!(out.chars().count(), 201);
    assert!(out.ends_with('…'));
    assert_eq!(preview("短"), "短");
}

// ── 错误包络 / 登录重定向 ──

#[test]
fn privilege_error_envelope_maps_to_auth() {
    let value = load("error_privilege");
    let err = parse_hydro_error(&value).expect("应识别为错误包络");
    assert!(matches!(err, AppError::Auth(_)), "未登录必须映射为 Auth");
}

#[test]
fn non_envelope_body_is_not_an_error() {
    assert!(parse_hydro_error(&serde_json::json!({ "url": "/" })).is_none());
    assert!(parse_hydro_error(&serde_json::json!([1, 2])).is_none());
}

#[test]
fn login_redirect_is_detected_only_for_login_urls() {
    let value = load("login_redirect");
    let url = login_redirect_url(&value).expect("应识别为登录重定向");
    assert!(url.starts_with("/login?redirect="));

    // 登录成功的 {"url":"/"} 与文件下载签名链接都不是会话问题
    assert!(login_redirect_url(&serde_json::json!({ "url": "/" })).is_none());
    assert!(login_redirect_url(&serde_json::json!({ "url": "https://cdn/x.zip" })).is_none());
    assert!(login_redirect_url(&serde_json::json!({})).is_none());
}

#[test]
fn extract_sid_reads_set_cookie_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.append(
        reqwest::header::SET_COOKIE,
        "sid=0123456789abcdef0123456789abcdef; Expires=Wed, 01 Jan 2026 00:00:00 GMT; Path=/"
            .parse()
            .unwrap(),
    );
    assert_eq!(
        extract_sid(&headers).as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
}

#[test]
fn extract_sid_ignores_other_cookies_and_missing_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.append(
        reqwest::header::SET_COOKIE,
        "xsid=notmine; Path=/".parse().unwrap(),
    );
    headers.append(
        reqwest::header::SET_COOKIE,
        "other=1; Path=/".parse().unwrap(),
    );
    assert!(extract_sid(&headers).is_none());
    assert!(extract_sid(&reqwest::header::HeaderMap::new()).is_none());
}

// ── 时间 ──

#[test]
fn parse_time_handles_iso_with_millis_and_z() {
    // 2026-01-01 01:00:00 UTC = 56 年（含 14 个闰年）× 365 天 + 1 小时
    assert_eq!(parse_time("2026-01-01T01:00:00.000Z"), 1_767_229_200);
    assert_eq!(parse_time("2026-01-01T01:00:00Z"), 1_767_229_200);
}

#[test]
fn parse_time_handles_space_separator_and_offsets() {
    assert_eq!(parse_time("2026-01-01 01:00:00"), 1_767_229_200);
    // 东八区同一时刻的本地写法：UTC = 本地 - 8h
    assert_eq!(parse_time("2026-01-01T09:00:00+08:00"), 1_767_229_200);
    assert_eq!(parse_time("2026-01-01T09:00:00+0800"), 1_767_229_200);
}

#[test]
fn parse_time_handles_leap_day_and_empty_input() {
    // 2024 是闰年，2 月 29 日合法
    assert!(parse_time("2024-02-29T00:00:00Z") > parse_time("2024-02-28T00:00:00Z"));
    assert_eq!(parse_time(""), 0);
    assert_eq!(parse_time("   "), 0);
}

#[test]
fn objectid_seconds_reads_creation_time_from_prefix() {
    // 0x6530f0c1 = 1697706177（2023-10-19T12:22:57Z）
    assert_eq!(objectid_seconds("6530f0c1a1b2c3d4e5f60718"), Some(1_697_706_177));
}

#[test]
fn objectid_seconds_rejects_non_objectid_shapes() {
    // 非 24 位 / 非十六进制 / 时间戳越界（0x00000000 早于 2010）一律拒绝，
    // 避免把普通字符串读成天文数字时间
    assert_eq!(objectid_seconds("6530f0c1"), None);
    assert_eq!(objectid_seconds("zzzzzzzzzzzzzzzzzzzzzzzz"), None);
    assert_eq!(objectid_seconds("000000000000000000000000"), None);
    assert_eq!(objectid_seconds("ffffffffffffffffffffffff"), None);
    assert_eq!(objectid_seconds(""), None);
}

#[test]
fn parse_duration_seconds_parses_formatted_durations() {
    assert_eq!(parse_duration_seconds("1:23:45"), Some(5025));
    assert_eq!(parse_duration_seconds("23:45"), Some(1425));
    assert_eq!(parse_duration_seconds("45"), Some(45));
    assert_eq!(parse_duration_seconds(" 0:12 "), Some(12));
    assert_eq!(parse_duration_seconds(""), None);
    assert_eq!(parse_duration_seconds("abc"), None);
    assert_eq!(parse_duration_seconds("1:x"), None);
}

// ── 状态码 ──

/// 状态变体的**线上名**（IPC 序列化值，前端按它做文案/配色映射）。
///
/// `JudgementStatus` 未派生 `PartialEq`（entity 层不可改），且我们真正关心的是
/// 「跨端序列化名对不对」，故按序列化结果断言 —— 顺带锁定了前端契约。
fn status_name(status: &JudgementStatus) -> String {
    serde_json::to_value(status)
        .expect("JudgementStatus 必须可序列化")
        .as_str()
        .expect("变体名即序列化字符串")
        .to_string()
}

#[test]
fn map_status_covers_full_hydro_table() {
    // Hydro 全码表（文档「评测状态码」节）：值 = 期望的线上名
    let table: &[(i64, &str)] = &[
        (0, "Pending"),
        (1, "Accepted"),
        (2, "WrongAnswer"),
        (3, "TimeLimitExceeded"),
        (4, "MemoryLimitExceeded"),
        (5, "OutputLimitExceeded"),
        (6, "RuntimeError"),
        (7, "CompilationError"),
        (8, "SystemError"),
        // 9 CANCELED 折入 Cancelled（HOJ -4 语义精确对应，无文案落差）；
        // HACKED / IGNORED / HACK_* 在 HOJ 值域里确实没有对应变体 → Unknown
        (9, "Cancelled"),
        (10, "UnknownError"),
        (11, "Unknown"),
        (20, "Running"),
        (21, "Compiling"),
        (22, "Pending"),
        (30, "Unknown"),
        (31, "PresentationError"),
        (32, "Unknown"),
        (33, "Unknown"),
    ];
    for (code, expected) in table {
        assert_eq!(
            status_name(&map_status(*code)),
            *expected,
            "Hydro 状态码 {} 映射不符",
            code
        );
    }
    assert_eq!(status_name(&map_status(99)), "Unknown");
}

#[test]
fn fetched_is_non_terminal_so_polling_continues() {
    // FETCHED(22) 折入 Pending 而非 Unknown：折成 Unknown 会让轮询在评测开始前停住
    assert!(!is_terminal_status(22));
    assert_eq!(status_name(&map_status(22)), "Pending");
    assert!(!is_terminal_status(0));
    assert!(!is_terminal_status(20));
    assert!(!is_terminal_status(21));
    for terminal in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 30, 31, 32, 33] {
        assert!(is_terminal_status(terminal), "{} 应为终态", terminal);
    }
}

#[test]
fn hoj_status_to_hydro_maps_shared_semantics() {
    // HOJ 码 → Hydro 码（HOJ 码表含负数，见 adapter/hoj/types.rs::map_status）
    assert_eq!(hoj_status_to_hydro(5), Some(0)); // Pending → WAITING
    assert_eq!(hoj_status_to_hydro(6), Some(21)); // Compiling → COMPILING
    assert_eq!(hoj_status_to_hydro(7), Some(20)); // Judging → JUDGING
    assert_eq!(hoj_status_to_hydro(9), Some(0)); // Submitting → WAITING
    assert_eq!(hoj_status_to_hydro(0), Some(1)); // AC → ACCEPTED
    assert_eq!(hoj_status_to_hydro(-1), Some(2)); // WA → WRONG_ANSWER
    assert_eq!(hoj_status_to_hydro(1), Some(3)); // TLE → TIME_LIMIT_EXCEEDED
    assert_eq!(hoj_status_to_hydro(2), Some(4)); // MLE → MEMORY_LIMIT_EXCEEDED
    assert_eq!(hoj_status_to_hydro(3), Some(6)); // RE → RUNTIME_ERROR
    assert_eq!(hoj_status_to_hydro(-2), Some(7)); // CE → COMPILE_ERROR
    assert_eq!(hoj_status_to_hydro(-3), Some(31)); // PE → FORMAT_ERROR
    assert_eq!(hoj_status_to_hydro(4), Some(8)); // SE → SYSTEM_ERROR
    assert_eq!(hoj_status_to_hydro(-4), Some(9)); // Cancelled → CANCELED
    assert_eq!(hoj_status_to_hydro(15), Some(10)); // No Status → ETC
}

#[test]
fn hoj_status_to_hydro_rejects_states_hydro_lacks() {
    // Not Submitted(-10) / PA(8) 在 Hydro 码表中不存在：必须返回 None 让调用方明确报错，
    // 而不是静默忽略筛选条件
    for code in [-10, 8, 99] {
        assert_eq!(hoj_status_to_hydro(code), None, "HOJ {} 不应有映射", code);
    }
}

// ── 语言 ──

#[test]
fn language_table_is_bijective_for_display_names() {
    // 展示名 → key 必须唯一（否则提交时会反查到错误的语言）
    let mut seen = std::collections::HashSet::new();
    for (key, display) in LANG_TABLE {
        assert!(seen.insert(*display), "展示名重复: {}", display);
        assert_eq!(lang_display(key), *display);
        assert_eq!(lang_key(display).as_deref(), Some(*key));
    }
}

#[test]
fn unknown_language_round_trips_unchanged() {
    // 部署可自定义 langs：未知 key 原样展示、原样提交回去，往返恒等
    assert_eq!(lang_display("my_custom_lang"), "my_custom_lang");
    assert_eq!(lang_key("my_custom_lang"), None);
    assert_eq!(lang_display(""), "");
}

#[test]
fn language_display_names_are_frontend_recognizable() {
    // 前端 utils/language 按前缀识别语言族，决定 Monaco 高亮与源文件名
    assert_eq!(lang_display("cc.cc17"), "C++17");
    assert!(lang_display("cc.cc17").starts_with("C++"));
    assert!(lang_display("py.py3").starts_with("Python"));
    assert!(lang_display("py.pypy3").starts_with("PyPy"));
    assert_eq!(lang_display("cs"), "C#");
}

#[test]
fn display_letter_follows_hydro_index_rule() {
    assert_eq!(display_letter(0), "A");
    assert_eq!(display_letter(25), "Z");
    // 超过 26 题没有字母可用，退回十进制序号
    assert_eq!(display_letter(26), "27");
}

// ── 值归一 ──

#[test]
fn coerce_id_accepts_number_and_string() {
    assert_eq!(coerce_id(&serde_json::json!(1000)), "1000");
    assert_eq!(coerce_id(&serde_json::json!("P1000")), "P1000");
    assert_eq!(coerce_id(&serde_json::json!(true)), "");
}

#[test]
fn coerce_i64_accepts_numeric_strings() {
    assert_eq!(coerce_i64(&serde_json::json!(12)), Some(12));
    assert_eq!(coerce_i64(&serde_json::json!("12")), Some(12));
    assert_eq!(coerce_i64(&serde_json::json!("abc")), None);
}

#[test]
fn strip_html_keeps_text_and_unescapes_entities() {
    let cell = CellVO {
        kind: Some("record".into()),
        value: Some(serde_json::json!(
            "<span class=\"icon icon-check\"></span>\n0:12:00&nbsp;"
        )),
        raw: None,
        score: None,
        style: None,
        hover: None,
    };
    assert_eq!(cell_text(&cell), "<span class=\"icon icon-check\"></span>\n0:12:00&nbsp;");
    let text = strip_html(&cell_text(&cell));
    // 首行是被剥离的图标（空行），第二行才是 AC 用时 —— 整体 trim 会破坏该结构
    assert_eq!(text.lines().next(), Some(""));
    assert_eq!(text.lines().nth(1), Some("0:12:00 "));
    assert!(!text.contains('<'));
}

#[test]
fn pending_count_requires_the_lock_marker() {
    // 封榜标记：橙色 span 里的 +N
    assert_eq!(
        pending_count("+1\n0:30:00\n<span style=\"color:orange\">+2</span>"),
        Some(2)
    );
    // AC 单元格首行的 "+N" 是「AC 前的失败次数」，不是待判次数
    assert_eq!(pending_count("+3\n0:12:00"), None);
    assert_eq!(pending_count(""), None);
}

#[test]
fn problem_status_code_maps_to_three_states() {
    assert_eq!(problem_status_code(&serde_json::json!({ "status": 1 })), 1);
    assert_eq!(problem_status_code(&serde_json::json!({ "status": 2 })), 2);
    assert_eq!(problem_status_code(&serde_json::json!({ "status": 7 })), 2);
    // 只有 rid（无 status）说明提交过但状态未知 → 尝试过
    assert_eq!(
        problem_status_code(&serde_json::json!({ "rid": "6530f0c1a1b2c3d4e5f60718" })),
        2
    );
    assert_eq!(problem_status_code(&serde_json::json!({})), 0);
}

// ── DTO 解析（夹具） ──

#[test]
fn contest_list_fixture_parses_and_derives_end_time() {
    let vo: ContestListVO = parse("contest_list");
    assert_eq!(vo.page, Some(1));
    assert_eq!(vo.tpcount, Some(1));
    let tdocs = vo.tdocs.expect("应含 tdocs");
    assert_eq!(tdocs.len(), 2);

    assert_eq!(tdocs[0].contest_id(), "64f0c0f0f0f0f0f0f0f0f0f0");
    assert_eq!(tdocs[0].rule.as_deref(), Some("acm"));
    assert_eq!(tdocs[0].pid_list(), vec!["1001".to_string()]);
    assert!(!tdocs[0].is_locked(), "lockAt 为 null 时应视为未封榜");

    // 第二条没有 endAt，只有 duration（小时）→ 由 beginAt 推算
    assert_eq!(tdocs[1].end_at, None);
    assert_eq!(tdocs[1].duration, Some(5.0));
}

#[test]
fn contest_problems_fixture_parses_and_tolerates_string_config() {
    let vo: ContestProblemListVO = parse("contest_problems");
    let pdict = vo.pdict.expect("应含 pdict");
    assert_eq!(pdict.len(), 2);
    // config 是字符串（服务端解析失败）时按缺失处理，不得让整页失败
    assert!(pdict["1000"].problem_config().is_none());
    assert_eq!(pdict["1000"].problem_id(), "P1000");
    let config = pdict["1001"].problem_config().expect("对象 config 应可解析");
    assert_eq!(config.time_limit_ms(), 1000);
    assert_eq!(config.memory_limit_mb(), 256);
    assert_eq!(config.langs, Some(vec!["cc".to_string()]));

    let tdoc = vo.tdoc.expect("应含 tdoc");
    assert!(tdoc.is_locked());
    assert_eq!(tdoc.pid_list(), vec!["1001".to_string(), "1000".to_string()]);
}

#[test]
fn problem_detail_fixture_parses() {
    let vo: ProblemDetailVO = parse("problem_detail");
    let pdoc = vo.pdoc.expect("应含 pdoc");
    assert_eq!(pdoc.problem_id(), "P1000");
    assert_eq!(pdoc.html, Some(false));
    let config = pdoc.problem_config().expect("config 应可解析");
    // 多测试点不同限时取最严者
    assert_eq!(config.time_limit_ms(), 2000);
    assert_eq!(config.memory_limit_mb(), 256);
    assert_eq!(vo.mode.as_deref(), Some("normal"));
    assert_eq!(
        problem_status_code(vo.psdoc.as_ref().unwrap()),
        1,
        "psdoc.status=1 表示已 AC"
    );
}

#[test]
fn problem_config_limits_fall_back_when_only_min_present() {
    let config: ProblemConfigVO = serde_json::from_value(serde_json::json!({
        "timeMin": 1000,
        "memoryMin": 256
    }))
    .unwrap();
    assert_eq!(config.time_limit_ms(), 1000);
    assert_eq!(config.memory_limit_mb(), 256);

    let empty: ProblemConfigVO = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(empty.time_limit_ms(), 0);
    assert_eq!(empty.memory_limit_mb(), 0);
}

#[test]
fn record_detail_fixture_parses() {
    let vo: RecordDetailVO = parse("record_detail");
    let rdoc = vo.rdoc.expect("应含 rdoc");
    assert_eq!(rdoc.record_id(), "6530f0c1a1b2c3d4e5f60718");
    assert_eq!(rdoc.status_code(), 1);
    assert_eq!(rdoc.lang.as_deref(), Some("cc.cc17"));
    assert_eq!(rdoc.code_length(), 21);
    assert!(rdoc.compiler_message().is_none(), "无编译信息时应为 None");
    assert_eq!(rdoc.terminal_metrics(), (12, 1024, 100.0));
    assert_eq!(rdoc.test_cases.as_ref().unwrap().len(), 3);
    assert_eq!(rdoc.submit_time(), 1_697_706_177, "优先取 ObjectId 前缀");
    assert_eq!(vo.udoc.as_ref().unwrap().display(), "admin");
}

#[test]
fn record_detail_submit_time_falls_back_to_judge_at() {
    let mut rdoc: RdocVO = serde_json::from_value(serde_json::json!({
        "_id": "not-an-objectid",
        "judgeAt": "2026-01-01T01:00:12.000Z"
    }))
    .unwrap();
    assert_eq!(rdoc.submit_time(), 1_767_229_212);
    rdoc.judge_at = None;
    assert_eq!(rdoc.submit_time(), 0);
}

#[test]
fn non_terminal_record_metrics_are_zeroed() {
    let rdoc: RdocVO = serde_json::from_value(serde_json::json!({
        "status": 20,
        "time": 5,
        "memory": 9,
        "score": 50
    }))
    .unwrap();
    // 未评测完的指标没有意义，必须清零（与 HOJ 侧同一约定）
    assert_eq!(rdoc.terminal_metrics(), (0, 0, 0.0));
}

#[test]
fn record_list_fixture_parses() {
    let vo: RecordListVO = parse("record_list");
    assert_eq!(vo.page, Some(1));
    assert_eq!(vo.rdocs.as_ref().unwrap().len(), 2);
    let tdoc = vo.tdoc.expect("应含 tdoc");
    assert_eq!(tdoc.rule.as_deref(), Some("acm"));
    assert_eq!(vo.pdict.as_ref().unwrap().len(), 2);
    assert_eq!(vo.udict.as_ref().unwrap()["3"].display(), "alice");
}

// ── 榜单单元格矩阵 ──

#[test]
fn acm_scoreboard_maps_to_rank_rows() {
    let vo: ScoreboardVO = parse("scoreboard_acm");
    let page = scoreboard_rank_page(&vo);

    assert_eq!(page.current, 1, "Hydro 无服务端分页 → 单页");
    assert_eq!(page.pages, 1);
    assert_eq!(page.records.len(), 3);
    assert_eq!(page.total, 3);

    let alice = &page.records[0];
    assert_eq!(alice.rank, 1);
    assert_eq!(alice.uid, "3");
    assert_eq!(alice.username, "Alice", "displayName 优先于 uname");
    assert_eq!(alice.realname, "Alice");
    assert_eq!(alice.ac, 2);
    // 1:23:45 = 5025 秒（ACM 的 total_time 单位是秒）
    assert_eq!(alice.total_time, 5025);
    assert_eq!(alice.total, 2, "总提交数 = 各题尝试次数之和");
    assert_eq!(alice.total_score, None, "ACM 无总分列");

    let cell_a = &alice.submission_info["A"];
    assert!(cell_a.is_ac);
    assert!(cell_a.is_first_ac, "style 非空即首 A 高亮");
    assert_eq!(cell_a.error_num, 0, "首行是 ✓ 图标 → 无失败尝试");
    assert_eq!(cell_a.ac_time, Some(720), "0:12:00 = 720 秒");
    assert_eq!(cell_a.score, None, "ACM 单元格不透出分数");

    let cell_b = &alice.submission_info["B"];
    assert!(!cell_b.is_ac);
    assert_eq!(cell_b.error_num, 1, "-1 → 失败 1 次");
    assert_eq!(cell_b.ac_time, None);

    // 打星：rank.value == "0" → -1（前端据此判定不计排名）
    let bob = &page.records[1];
    assert_eq!(bob.rank, -1);
    assert_eq!(bob.username, "bob");
    assert_eq!(bob.ac, 0);
    assert_eq!(bob.total, 2);
    assert_eq!(bob.submission_info["A"].error_num, 2);
    assert!(!bob.submission_info["A"].is_ac);
    // 空单元格（无任何提交）不得被当成通过
    assert!(!bob.submission_info["B"].is_ac);
    assert_eq!(bob.submission_info["B"].error_num, 0);

    let carol = &page.records[2];
    assert_eq!(carol.rank, 3);
    assert_eq!(carol.avatar, "https://hydro.ac/avatar/5");
    assert_eq!(carol.total_time, 2700);
    let cell_a = &carol.submission_info["A"];
    assert!(cell_a.is_ac);
    assert!(!cell_a.is_first_ac);
    assert_eq!(cell_a.error_num, 1, "+1 → AC 前失败 1 次");
    assert_eq!(cell_a.try_num, Some(2), "封榜橙色 span 的 +2 是待判次数");
    assert_eq!(carol.total, 3);
}

#[test]
fn oi_scoreboard_maps_scores_not_attempts() {
    let vo: ScoreboardVO = parse("scoreboard_oi");
    let page = scoreboard_rank_page(&vo);

    let alice = &page.records[0];
    assert_eq!(alice.rank, 1);
    assert_eq!(alice.total_score, Some(150));
    assert_eq!(alice.ac, 1, "OI 无 solved 列 → 按满分题数计");
    assert_eq!(alice.total, 0, "OI 不推导总提交数");
    assert_eq!(alice.submission_info["A"].score, Some(100));
    assert!(alice.submission_info["A"].is_ac);
    assert_eq!(alice.submission_info["B"].score, Some(50));
    assert!(!alice.submission_info["B"].is_ac);
    assert!(alice.time_info.is_empty(), "Hydro 每题单元格不含用时");

    let bob = &page.records[1];
    assert_eq!(bob.total_score, Some(0));
    assert_eq!(bob.submission_info["A"].score, Some(0));
    assert!(!bob.submission_info["B"].is_ac);
}

#[test]
fn empty_scoreboard_yields_empty_single_page() {
    let vo = ScoreboardVO {
        tdoc: None,
        rows: Some(Vec::new()),
        udict: None,
        pdict: None,
    };
    let page = scoreboard_rank_page(&vo);
    assert!(page.records.is_empty());
    assert_eq!(page.pages, 1);
    assert_eq!(page.total, 0);
}

#[test]
fn scoreboard_without_problem_letters_falls_back_to_index_order() {
    // 表头没给字母时按题目列顺序派生 A/B
    let vo: ScoreboardVO = serde_json::from_value(serde_json::json!({
        "tdoc": { "rule": "acm" },
        "rows": [
            [{ "type": "rank", "value": "#" }, { "type": "problem", "raw": 1000 }, { "type": "problem", "raw": 1001 }],
            [{ "type": "rank", "value": "1" }, { "type": "record", "value": "-1" }, { "type": "record", "value": "" }]
        ]
    }))
    .unwrap();
    let page = scoreboard_rank_page(&vo);
    assert_eq!(page.records[0].submission_info["A"].error_num, 1);
    assert!(page.records[0].submission_info.contains_key("B"));
}

// ── UserBriefVO ──

#[test]
fn user_brief_display_falls_back_to_uname() {
    let with_display: UserBriefVO = serde_json::from_value(
        serde_json::json!({ "_id": 1, "uname": "a", "displayName": "A 队" }),
    )
    .unwrap();
    assert_eq!(with_display.display(), "A 队");

    let without: UserBriefVO =
        serde_json::from_value(serde_json::json!({ "_id": 1, "uname": "a" })).unwrap();
    assert_eq!(without.display(), "a");

    let empty: UserBriefVO = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(empty.display(), "");
}

// ── 域相关重定向（`{"url":"/d/..."}`）──
//
// 背景：Hydro 的域解析是「路径前缀 /d/:domainId/ 优先，但 Host 反查出的域与路径域
// 不一致时 302」。带 Accept: application/json 时该重定向表现为 HTTP 200 +
// `{"url":"/d/<推断域>/..."}`。若把它当普通响应漏进 DTO 解析，会报「响应字段不匹配」——
// 把「base_url 里的域前缀不对」这种配置问题伪装成 DTO 问题。
//
// 这类重定向同时还有第二种来源：未加入域 → `/d/<域>/domain/join?...`（业务问题）。
// 两者无法从响应区分，故错误消息如实列出两种可能，且**不能**用 Auth 变体
// （那会触发前端登出，把「域不对」误判成会话失效）。

#[test]
fn domain_redirect_is_translated_to_actionable_error() {
    let value = serde_json::json!({ "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0" });
    let err = domain_redirect_error(&value).expect("应识别为域相关重定向");
    match err {
        AppError::Unknown(msg) => {
            assert!(msg.contains("/d/system/"), "应保留目标地址供定位: {}", msg);
            assert!(msg.contains("域前缀"), "应点明可能原因: {}", msg);
        }
        other => panic!("应为 Unknown（不得触发登出），实际 {:?}", other),
    }
}

#[test]
fn domain_redirect_ignores_non_domain_urls() {
    // 登出成功后的 {"url":"/"}：不是重定向问题，必须原样通过
    assert!(domain_redirect_error(&serde_json::json!({ "url": "/" })).is_none());
    // 文件下载的绝对签名链接：含 /d/ 但不以 /d/ 开头 → 不误判
    assert!(domain_redirect_error(&serde_json::json!({
        "url": "https://hydro.ac/d/system/file/1.in?sig=abc"
    }))
    .is_none());
    // 登录重定向由 login_redirect_url 负责，此处不重复认领
    assert!(domain_redirect_error(&serde_json::json!({ "url": "/login?redirect=%2Fp%2F1000" })).is_none());
    assert!(domain_redirect_error(&serde_json::json!({})).is_none());
}