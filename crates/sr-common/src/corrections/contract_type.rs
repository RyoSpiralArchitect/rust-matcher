/// 人材契約形態ENUM: ["正社員", "契約社員", "直個人"]
pub fn correct_talent_contract_type(input: &str, is_primary: bool) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return if is_primary {
            Some("直個人".to_string())
        } else {
            None
        };
    }

    let valid = ["正社員", "契約社員", "直個人"];
    if valid.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    if trimmed.contains("正社員") {
        return Some("正社員".to_string());
    }
    if trimmed.contains("契約") {
        return Some("契約社員".to_string());
    }
    if trimmed.contains("個人") || trimmed.contains("フリー") {
        return Some("直個人".to_string());
    }

    if is_primary {
        Some("直個人".to_string())
    } else {
        None
    }
}

/// 性別ENUM: ["男性", "女性", "その他/無回答"]
pub fn correct_gender(input: &str) -> Option<String> {
    let trimmed = input.trim();

    let valid = ["男性", "女性", "その他/無回答"];
    if valid.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    if trimmed.contains('男') {
        return Some("男性".to_string());
    }
    if trimmed.contains('女') {
        return Some("女性".to_string());
    }

    Some("その他/無回答".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_talent_contract_type_conservatively() {
        assert_eq!(
            correct_talent_contract_type("  ", true),
            Some("直個人".into())
        );
        assert_eq!(
            correct_talent_contract_type("フリーランス", true),
            Some("直個人".into())
        );
        assert_eq!(
            correct_talent_contract_type("契約形態:契約", true),
            Some("契約社員".into())
        );
        assert_eq!(
            correct_talent_contract_type("正社員希望", false),
            Some("正社員".into())
        );
        assert_eq!(correct_talent_contract_type("", false), None);
    }

    #[test]
    fn corrects_gender_variants() {
        assert_eq!(correct_gender("男性"), Some("男性".into()));
        assert_eq!(correct_gender("女性"), Some("女性".into()));
        assert_eq!(correct_gender("その他"), Some("その他/無回答".into()));
        assert_eq!(correct_gender("   "), Some("その他/無回答".into()));
    }
}
