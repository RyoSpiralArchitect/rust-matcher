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

/// 契約形態比較用に派遣/準委任へ正規化する
/// - 派遣を含む場合は "派遣"
/// - 準委任/業務委託/SES/受託を含む場合は "準委任契約"
/// - それ以外の不明値は None として扱い、強制的な HardKo を避ける
pub fn normalize_contract_type_for_matching(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        if trimmed.contains("派遣") {
            return Some("派遣".to_string());
        }

        if trimmed.contains("個人") || trimmed.contains("フリー") {
            return Some("直個人".to_string());
        }

        let lower = trimmed.to_ascii_lowercase();
        if trimmed.contains("準委任")
            || trimmed.contains("業務委託")
            || trimmed.contains("業務委任")
            || trimmed.contains("受託")
            || lower.contains("ses")
        {
            return Some("準委任契約".to_string());
        }

        let corrected = correct_contract_type(trimmed);

        if corrected == "準委任契約" && trimmed != corrected {
            None
        } else {
            Some(corrected)
        }
    })
}

/// 案件契約形態ENUM: ["準委任契約", "派遣"]
/// sponto-platform enum_corrections.js correctContractType() と同期
pub fn correct_contract_type(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "準委任契約".to_string(); // Default
    }

    let valid = ["準委任契約", "派遣"];
    if valid.contains(&trimmed) {
        return trimmed.to_string();
    }

    if trimmed.contains("派遣") {
        return "派遣".to_string();
    }

    "準委任契約".to_string() // Default
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

    #[test]
    fn corrects_contract_type_for_projects() {
        assert_eq!(correct_contract_type("準委任契約"), "準委任契約");
        assert_eq!(correct_contract_type("派遣契約"), "派遣");
        assert_eq!(correct_contract_type(""), "準委任契約");
        assert_eq!(correct_contract_type("業務委託"), "準委任契約");
    }

    #[test]
    fn normalizes_contract_types_for_matching() {
        assert_eq!(
            normalize_contract_type_for_matching(Some("業務委託(SES)")),
            Some("準委任契約".into())
        );
        assert_eq!(
            normalize_contract_type_for_matching(Some("紹介予定派遣")),
            Some("派遣".into())
        );
        assert_eq!(
            normalize_contract_type_for_matching(Some("フリーランス")),
            Some("直個人".into())
        );
        assert_eq!(normalize_contract_type_for_matching(Some("正社員")), None);
        assert_eq!(normalize_contract_type_for_matching(Some("   ")), None);
    }
}
