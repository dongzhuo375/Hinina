use super::*;

#[test]
fn byte_limit_is_inclusive_and_counts_utf8_and_escaping() {
    for value in ["abc", "汉字", "\n\"\\"] {
        let expected = serde_json::to_string(value).unwrap();
        assert_eq!(encoded_len(value, expected.len()).unwrap(), expected.len());
        assert_eq!(to_string(value, expected.len()).unwrap(), expected);
        assert_eq!(
            to_string(value, expected.len() - 1),
            Err(PluginError::LimitExceeded)
        );
    }
}

#[test]
fn streaming_rejects_large_values_without_retaining_them() {
    let value = vec!["large entry"; 100_000];
    assert_eq!(encoded_len(&value, 1024), Err(PluginError::LimitExceeded));
    assert_eq!(to_string(&value, 1024), Err(PluginError::LimitExceeded));
}
