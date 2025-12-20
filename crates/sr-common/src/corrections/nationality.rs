/// 日本国籍かどうかの簡易判定
///
/// - "日本" を含む（例: "日本国籍", "日本人"）
/// - 英語表記の "japan", "japanese" を含む（大文字小文字無視）
/// - 前後の空白・全角空白は無視
pub fn is_japanese_nationality(value: &str) -> bool {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return false;
    }

    let normalized = trimmed
        .to_lowercase()
        .replace([' ', '\t', '\n', '\r', '\u{3000}'], "");

    normalized.contains("日本") || normalized.contains("japan")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_japanese_variants() {
        assert!(is_japanese_nationality("日本"));
        assert!(is_japanese_nationality(" 日本国籍 "));
        assert!(is_japanese_nationality("日本人"));
        assert!(is_japanese_nationality("JAPAN"));
        assert!(is_japanese_nationality("Japanese"));
    }

    #[test]
    fn rejects_non_japanese_or_empty() {
        assert!(!is_japanese_nationality("USA"));
        assert!(!is_japanese_nationality("フランス"));
        assert!(!is_japanese_nationality(""));
    }
}
