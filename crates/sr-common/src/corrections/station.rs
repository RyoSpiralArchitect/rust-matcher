/// 駅名正規化ロジック（CD-6: normalize_station の空文字禁止）
///
/// - trim後に空なら None を返す
/// - 「駅」で終わる場合はそのまま返す
/// - それ以外は語尾に「駅」を付与する
pub fn normalize_station(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.ends_with("駅") {
        Some(trimmed.to_string())
    } else {
        Some(format!("{}駅", trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_values_return_none() {
        assert_eq!(normalize_station(""), None);
        assert_eq!(normalize_station("   "), None);
        assert_eq!(normalize_station("\t\n"), None);
    }

    #[test]
    fn appends_suffix_when_missing() {
        assert_eq!(normalize_station("新宿"), Some("新宿駅".into()));
        assert_eq!(normalize_station("  池袋  "), Some("池袋駅".into()));
    }

    #[test]
    fn keeps_existing_suffix() {
        assert_eq!(normalize_station("渋谷駅"), Some("渋谷駅".into()));
    }
}
