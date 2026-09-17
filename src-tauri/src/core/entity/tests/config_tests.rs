// core/entity/config.rs 单元测试：默认值取值域 + 旧值归一（P55）

use super::*;

// ── 默认值必须落在取值域内 ──

#[test]
fn defaults_are_in_value_domain() {
    let cfg = AppConfig::default();
    // HOJ 语言显示名（与提交契约一致），而不是 Monaco id "cpp"
    assert_eq!(cfg.editor.default_language, "C++");
    // 深色主题尚未实现，默认必须是浅色
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
    assert_eq!(cfg.layout.split_ratio, 0.48);
    // 题面缓存默认开启（关闭只影响性能，不影响正确性）
    assert!(cfg.oj.cache_problem_statement);
}

// ── 旧值归一 ──

#[test]
fn normalize_maps_legacy_monaco_ids_to_display_names() {
    // 值域翻转：被上一版归一成 Monaco id 的配置映射回 HOJ 显示名（大小写不敏感）
    for (legacy, expected) in [
        ("cpp", "C++"),
        ("java", "Java"),
        ("python", "Python"),
        ("c", "C"),
        ("CPP", "C++"),
        ("Java", "Java"),
    ] {
        let mut cfg = AppConfig::default();
        cfg.editor.default_language = legacy.into();
        normalize_legacy_values(&mut cfg);
        assert_eq!(
            cfg.editor.default_language, expected,
            "旧 Monaco id {:?} 应归一为显示名",
            legacy
        );
    }
}

#[test]
fn normalize_keeps_valid_and_unknown_languages_untouched() {
    // 已是显示名或 OJ 可能提供的其他语言：一律不动（不强制回退 C++）
    for kept in ["C++", "C", "Java", "Python", "Go", "Rust"] {
        let mut cfg = AppConfig::default();
        cfg.editor.default_language = kept.into();
        normalize_legacy_values(&mut cfg);
        assert_eq!(cfg.editor.default_language, kept);
    }
}

#[test]
fn normalize_falls_back_to_default_language_for_empty() {
    // 仅空串回退默认值
    let mut cfg = AppConfig::default();
    cfg.editor.default_language = "".into();
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.editor.default_language, "C++");
}

#[test]
fn normalize_maps_dark_theme_to_light() {
    let mut cfg = AppConfig::default();
    cfg.theme.theme_name = "dark".into();
    cfg.theme.editor_theme = "vs-dark".into();
    normalize_legacy_values(&mut cfg);
    // 整机深色主题未实现：theme_name 与随它一并写入的编辑器主题一起归位
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
}

#[test]
fn normalize_keeps_light_theme_untouched() {
    let mut cfg = AppConfig::default();
    cfg.theme.theme_name = "light".into();
    cfg.theme.editor_theme = "vs".into();
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
}

#[test]
fn normalize_keeps_user_chosen_dark_editor_theme() {
    // 编辑器主题是解题页可选项：浅色界面 + 深色编辑器是**合法组合**，
    // 归一不得把用户选择改回浅色（曾无条件重置 vs-dark）
    let mut cfg = AppConfig::default();
    cfg.theme.theme_name = "light".into();
    cfg.theme.editor_theme = "vs-dark".into();
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs-dark");
}

#[test]
fn normalize_replaces_only_legacy_default_split_ratio() {
    // 0.45 恰好是旧版默认值 → 归一为现默认 0.48
    let mut cfg = AppConfig::default();
    cfg.layout.split_ratio = 0.45;
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.layout.split_ratio, 0.48);

    // 用户显式设置的其他值（含 0.45 附近的任意值）一律不动
    for kept in [0.3, 0.4, 0.44, 0.46, 0.5, 0.7] {
        let mut cfg = AppConfig::default();
        cfg.layout.split_ratio = kept;
        normalize_legacy_values(&mut cfg);
        assert_eq!(cfg.layout.split_ratio, kept);
    }
}

#[test]
fn normalize_is_idempotent() {
    // 归一后的配置再过一遍不应有任何变化（每次加载都会执行）
    let mut cfg = AppConfig::default();
    cfg.editor.default_language = "cpp".into();
    cfg.theme.theme_name = "dark".into();
    cfg.theme.editor_theme = "vs-dark".into();
    cfg.layout.split_ratio = 0.45;
    normalize_legacy_values(&mut cfg);
    let once = cfg.clone();
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.editor.default_language, once.editor.default_language);
    assert_eq!(cfg.theme.theme_name, once.theme.theme_name);
    assert_eq!(cfg.theme.editor_theme, once.theme.editor_theme);
    assert_eq!(cfg.layout.split_ratio, once.layout.split_ratio);
}

#[test]
fn normalize_fixes_legacy_json_from_disk() {
    // 端到端：上一版落盘的 config.json（defaultLanguage 被归一成 'cpp'）反序列化后重新归一
    let legacy = r#"{
        "editor": { "defaultLanguage": "cpp" },
        "theme": { "themeName": "dark", "editorTheme": "vs-dark" },
        "layout": { "splitRatio": 0.45 }
    }"#;
    let mut cfg: AppConfig = serde_json::from_str(legacy).expect("旧配置应能解析");
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.editor.default_language, "C++");
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
    assert_eq!(cfg.layout.split_ratio, 0.48);
}

// ── 写入路径校验与净化（M4）──

#[test]
fn validate_rejects_url_without_http_scheme() {
    // 空串 / 其他 scheme / 裸域名 / 只有 scheme 头 / 含空白，一律拒绝
    for bad in [
        "",
        "ftp://x",
        "hoj.example.com",
        "http://",
        "https://",
        "https://hoj.example.com/a b",
    ] {
        let mut cfg = AppConfig::default();
        cfg.oj.hoj_url = bad.into();
        let err = cfg.validate().expect_err(&format!("{:?} 应被拒绝", bad));
        assert_eq!(err, "服务器地址必须以 http:// 或 https:// 开头");
    }
}

#[test]
fn validate_accepts_http_and_https_case_insensitive() {
    for good in [
        "https://hoj.dongzhuo.top",
        "http://127.0.0.1:8080",
        "HTTP://OJ.EXAMPLE.COM",
        "  https://hoj.example.com  ", // 与前端一致：trim 后校验
    ] {
        let mut cfg = AppConfig::default();
        cfg.oj.hoj_url = good.into();
        assert!(cfg.validate().is_ok(), "{:?} 应通过", good);
    }
}

#[test]
fn sanitize_clamps_out_of_range_values() {
    let mut cfg = AppConfig::default();
    cfg.oj.timeout_secs = 0;
    cfg.oj.poll_interval_secs = 999;
    cfg.oj.poll_timeout_secs = 1;
    cfg.oj.cache_ttl_secs = 601;
    cfg.oj.contest_id = -5;
    cfg.editor.font_size = 100;
    cfg.editor.tab_size = 0;
    cfg.editor.auto_save_interval_secs = 1;
    cfg.layout.split_ratio = 0.95;

    assert!(cfg.sanitize(), "有字段被钳制时应返回 true");
    assert_eq!(cfg.oj.timeout_secs, 1);
    assert_eq!(cfg.oj.poll_interval_secs, 30);
    assert_eq!(cfg.oj.poll_timeout_secs, 30);
    assert_eq!(cfg.oj.cache_ttl_secs, 600);
    assert_eq!(cfg.oj.contest_id, 0);
    assert_eq!(cfg.editor.font_size, 32);
    assert_eq!(cfg.editor.tab_size, 1);
    assert_eq!(cfg.editor.auto_save_interval_secs, 5);
    assert_eq!(cfg.layout.split_ratio, 0.70);
}

#[test]
fn sanitize_keeps_valid_config_unchanged() {
    // 默认配置全部落在取值域内
    let mut cfg = AppConfig::default();
    assert!(!cfg.sanitize(), "无越界字段应返回 false");
    assert_eq!(cfg, AppConfig::default());

    // 用户合法自定义值同样不动
    let mut cfg = AppConfig::default();
    cfg.oj.timeout_secs = 120;
    cfg.oj.poll_interval_secs = 30;
    cfg.oj.poll_timeout_secs = 3600;
    cfg.oj.cache_ttl_secs = 0;
    cfg.oj.contest_id = 1011;
    cfg.editor.font_size = 8;
    cfg.editor.tab_size = 8;
    cfg.editor.auto_save_interval_secs = 300;
    cfg.editor.default_language = "Java".into();
    cfg.theme.editor_theme = "vs-dark".into();
    cfg.layout.split_ratio = 0.30;
    // 布尔开关无值域，sanitize 不得改写用户选择
    cfg.oj.cache_problem_statement = false;
    let before = cfg.clone();
    assert!(!cfg.sanitize());
    assert_eq!(cfg, before);
}

#[test]
fn sanitize_normalizes_default_language() {
    // 旧 Monaco id → 显示名；非空未知值保留（OJ 可能提供 Go/Rust 等）；仅空串回退默认
    for (raw, expected) in [
        ("cpp", "C++"),
        ("java", "Java"),
        ("C++", "C++"),
        ("Go", "Go"),
        ("Rust", "Rust"),
        ("", "C++"),
    ] {
        let mut cfg = AppConfig::default();
        cfg.editor.default_language = raw.into();
        cfg.sanitize();
        assert_eq!(cfg.editor.default_language, expected, "raw={:?}", raw);
    }
}

#[test]
fn sanitize_normalizes_editor_theme() {
    // 取值域 = Monaco 内置 vs / vs-dark；未知值（手改配置 / 自定义主题名）回退默认，
    // 否则 setTheme 静默无效，界面与配置不一致
    for (raw, expected) in [
        ("vs", "vs"),
        ("vs-dark", "vs-dark"),
        ("", "vs"),
        ("dracula", "vs"),
        ("VS-DARK", "vs"),
    ] {
        let mut cfg = AppConfig::default();
        cfg.theme.editor_theme = raw.into();
        cfg.sanitize();
        assert_eq!(cfg.theme.editor_theme, expected, "raw={:?}", raw);
    }
}

#[test]
fn sanitize_trims_url_and_handles_non_finite_ratio() {
    let mut cfg = AppConfig::default();
    cfg.oj.hoj_url = "  https://hoj.example.com  ".into();
    cfg.layout.split_ratio = f64::NAN;
    assert!(cfg.sanitize());
    assert_eq!(cfg.oj.hoj_url, "https://hoj.example.com");
    // NaN 不可钳制：回退默认值（NaN.clamp 会原样返回 NaN）
    assert_eq!(cfg.layout.split_ratio, 0.48);
}

#[test]
fn sanitize_fixes_hand_edited_json_from_disk() {
    // 端到端：手改 config.json 绕过前端校验后，写入路径兜底收敛
    let edited = r#"{
        "oj": { "hojUrl": "https://hoj.dongzhuo.top", "timeoutSecs": 9999, "contestId": -1 },
        "editor": { "fontSize": 3, "defaultLanguage": "Haskell" },
        "layout": { "splitRatio": 0.05 }
    }"#;
    let mut cfg: AppConfig = serde_json::from_str(edited).expect("应能解析");
    cfg.validate().expect("URL 合法应通过");
    assert!(cfg.sanitize());
    assert_eq!(cfg.oj.timeout_secs, 120);
    assert_eq!(cfg.oj.contest_id, 0);
    assert_eq!(cfg.editor.font_size, 8);
    // 未知非空语言不再强制回退：OJ 可能提供 Haskell 等
    assert_eq!(cfg.editor.default_language, "Haskell");
    assert_eq!(cfg.layout.split_ratio, 0.30);
}
