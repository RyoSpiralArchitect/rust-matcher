/// リモート形態ENUM
pub const REMOTE_ONSITE_ENUMS: &[&str] = &["フル出社", "リモート併用", "フルリモート"];

/// 【第1段階】リモート形態の正規化（必ず値を返す）
pub fn normalize_remote_onsite(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "リモート併用".to_string();
    }

    if REMOTE_ONSITE_ENUMS.contains(&trimmed) {
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();

    if lower.contains("フルリモート")
        || lower.contains("完全リモート")
        || lower.contains("フルリモ")
        || lower.contains("full remote")
    {
        return "フルリモート".to_string();
    }

    if lower.contains("フル出社")
        || lower.contains("出社のみ")
        || lower.contains("常駐")
        || lower.contains("客先")
        || lower.contains("出社必須")
    {
        return "フル出社".to_string();
    }

    "リモート併用".to_string()
}

/// 【第2段階】ENUM補正（補正不可なら None）
pub fn correct_remote_onsite(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if REMOTE_ONSITE_ENUMS.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    let lower = trimmed.to_lowercase();

    if lower.contains("フルリモート")
        || lower.contains("完全リモート")
        || lower.contains("フルリモ")
    {
        return Some("フルリモート".to_string());
    }
    if lower.contains("フル出社")
        || lower.contains("出社")
        || lower.contains("常駐")
        || lower.contains("客先")
    {
        return Some("フル出社".to_string());
    }
    if lower.contains("リモート")
        || lower.contains("併用")
        || lower.contains("ハイブリッド")
        || lower.contains("一部")
    {
        return Some("リモート併用".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_falls_back_to_hybrid() {
        assert_eq!(normalize_remote_onsite(""), "リモート併用");
        assert_eq!(normalize_remote_onsite("full remote"), "フルリモート");
        assert_eq!(normalize_remote_onsite("常駐"), "フル出社");
    }

    #[test]
    fn correct_remote_patterns() {
        assert_eq!(
            correct_remote_onsite("フルリモート"),
            Some("フルリモート".into())
        );
        assert_eq!(correct_remote_onsite("客先常駐"), Some("フル出社".into()));
        assert_eq!(
            correct_remote_onsite("ハイブリッド"),
            Some("リモート併用".into())
        );
        assert_eq!(correct_remote_onsite(""), None);
    }
}
