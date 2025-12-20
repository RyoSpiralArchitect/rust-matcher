/// 英語スキルENUM: ["不要", "読み書き", "会話", "ビジネス", "上級ビジネス", "ネイティブ"]
pub fn correct_english_skill(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let valid = [
        "不要",
        "読み書き",
        "会話",
        "ビジネス",
        "上級ビジネス",
        "ネイティブ",
    ];
    if valid.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    let lower = trimmed.to_lowercase();

    if trimmed.contains("ネイティブ") || lower.contains("native") {
        return Some("ネイティブ".to_string());
    }
    if trimmed.contains("上級ビジネス")
        || lower.contains("advanced business")
        || lower.contains("fluent")
    {
        return Some("上級ビジネス".to_string());
    }
    if trimmed.contains("ビジネス") || lower.contains("business") {
        return Some("ビジネス".to_string());
    }
    if trimmed.contains("会話") || lower.contains("conversation") || lower.contains("speaking") {
        return Some("会話".to_string());
    }
    if trimmed.contains("読み書き") || lower.contains("reading") || lower.contains("writing") {
        return Some("読み書き".to_string());
    }
    if trimmed.contains("不要") || trimmed.contains("不問") || lower.contains("none") {
        return Some("不要".to_string());
    }

    None
}

/// 英語スキルレベルの順序比較（KO判定用）
pub fn english_skill_level(skill: &str) -> i32 {
    match skill {
        "不要" => 0,
        "読み書き" => 1,
        "会話" => 2,
        "ビジネス" => 3,
        "上級ビジネス" => 4,
        "ネイティブ" => 5,
        _ => -1,
    }
}

/// 英語スキルKO判定
/// project_required: 案件が要求する英語レベル
/// talent_level: 人材が持つ英語レベル
pub fn is_english_ko(project_required: Option<&str>, talent_level: Option<&str>) -> bool {
    match (project_required, talent_level) {
        (None, _) => false,
        (Some("不要"), _) => false,
        (Some(_), None) => true,
        (Some(req), Some(tal)) => {
            let req_level = english_skill_level(req);
            let tal_level = english_skill_level(tal);
            tal_level < req_level
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_english_skills() {
        assert_eq!(
            correct_english_skill("ネイティブ"),
            Some("ネイティブ".into())
        );
        assert_eq!(
            correct_english_skill("Business conversation"),
            Some("ビジネス".into())
        );
        assert_eq!(correct_english_skill("reading"), Some("読み書き".into()));
        assert_eq!(correct_english_skill("  "), None);
    }

    #[test]
    fn evaluates_ko_levels() {
        assert!(!is_english_ko(Some("不要"), None));
        assert!(is_english_ko(Some("ビジネス"), Some("会話")));
        assert!(!is_english_ko(Some("ビジネス"), Some("ネイティブ")));
    }
}
