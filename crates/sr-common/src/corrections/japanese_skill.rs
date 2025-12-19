pub const JAPANESE_SKILL_ENUMS: &[&str] = &["不要", "N5", "N4", "N3", "N2", "N1", "ネイティブ"];

/// 日本語スキルENUM補正
pub fn correct_japanese_skill(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if JAPANESE_SKILL_ENUMS.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    let upper = trimmed.to_uppercase();

    if upper.contains("N1") || trimmed.contains("ビジネス") {
        return Some("N1".to_string());
    }
    if upper.contains("N2") {
        return Some("N2".to_string());
    }
    if upper.contains("N3") {
        return Some("N3".to_string());
    }
    if upper.contains("N4") {
        return Some("N4".to_string());
    }
    if upper.contains("N5") {
        return Some("N5".to_string());
    }
    if trimmed.contains("ネイティブ") || trimmed.contains("母語") || trimmed.contains("母国語") {
        return Some("ネイティブ".to_string());
    }
    if trimmed.contains("不要") || trimmed.contains("不問") {
        return Some("不要".to_string());
    }

    None
}

fn japanese_skill_level(skill: &str) -> Option<u8> {
    match skill.trim() {
        "不要" => Some(0),
        "N5" => Some(1),
        "N4" => Some(2),
        "N3" => Some(3),
        "N2" => Some(4),
        "N1" => Some(5),
        "ネイティブ" => Some(6),
        _ => None,
    }
}

/// 日本語スキルKO判定
/// 戻り値: Some(true)=KO, Some(false)=合格, None=情報不足
pub fn is_japanese_ko(project_required: Option<&str>, talent_level: Option<&str>) -> Option<bool> {
    let req = project_required.and_then(japanese_skill_level)?;
    if req == 0 {
        return Some(false);
    }

    let tal = talent_level.and_then(japanese_skill_level)?;
    Some(tal < req)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_japanese_skills() {
        assert_eq!(correct_japanese_skill("ネイティブ"), Some("ネイティブ".into()));
        assert_eq!(correct_japanese_skill("n2"), Some("N2".into()));
        assert_eq!(correct_japanese_skill("日本語不問"), Some("不要".into()));
        assert_eq!(correct_japanese_skill(""), None);
    }

    #[test]
    fn evaluates_japanese_ko() {
        assert_eq!(is_japanese_ko(Some("N2"), Some("N1")), Some(false));
        assert_eq!(is_japanese_ko(Some("N2"), Some("N3")), Some(true));
        assert_eq!(is_japanese_ko(Some("不要"), None), Some(false));
        assert_eq!(is_japanese_ko(Some("N2"), None), None);
    }
}
