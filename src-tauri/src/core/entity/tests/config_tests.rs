// core/entity/config.rs 单元测试：默认值取值域 + 旧值归一（P55）

use super::*;

// ── 默认值必须落在取值域内 ──

#[test]
fn defaults_are_in_value_domain() {
    let cfg = AppConfig::default();
    // Monaco language id，而不是显示名 "C++"
    assert_eq!(cfg.editor.default_language, "cpp");
    // 深色主题尚未实现，默认必须是浅色
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
    assert_eq!(cfg.layout.split_ratio, 0.48);
}

// ── 旧值归一 ──

#[test]
fn normalize_maps_legacy_language_display_names_to_monaco_ids() {
    for (legacy, expected) in [
        ("C++", "cpp"),
        ("Java", "java"),
        ("Python", "python"),
        ("C", "c"),
    ] {
        let mut cfg = AppConfig::default();
        cfg.editor.default_language = legacy.into();
        normalize_legacy_values(&mut cfg);
        assert_eq!(
            cfg.editor.default_language, expected,
            "旧显示名 {:?} 应归一为 Monaco id",
            legacy
        );
    }
}

#[test]
fn normalize_keeps_valid_and_unknown_languages_untouched() {
    // 已是合法 id 或用户自定义值：一律不动
    for kept in ["cpp", "java", "python", "c", "rust", ""] {
        let mut cfg = AppConfig::default();
        cfg.editor.default_language = kept.into();
        normalize_legacy_values(&mut cfg);
        assert_eq!(cfg.editor.default_language, kept);
    }
}

#[test]
fn normalize_maps_dark_theme_to_light() {
    let mut cfg = AppConfig::default();
    cfg.theme.theme_name = "dark".into();
    cfg.theme.editor_theme = "vs-dark".into();
    normalize_legacy_values(&mut cfg);
    // 深色主题未实现：归一到浅色，避免前端拿到不存在的主题名
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
    cfg.editor.default_language = "C++".into();
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
    // 端到端：旧版落盘的 config.json 反序列化后归一
    let legacy = r#"{
        "editor": { "defaultLanguage": "C++" },
        "theme": { "themeName": "dark", "editorTheme": "vs-dark" },
        "layout": { "splitRatio": 0.45 }
    }"#;
    let mut cfg: AppConfig = serde_json::from_str(legacy).expect("旧配置应能解析");
    normalize_legacy_values(&mut cfg);
    assert_eq!(cfg.editor.default_language, "cpp");
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
    assert_eq!(cfg.layout.split_ratio, 0.48);
}
