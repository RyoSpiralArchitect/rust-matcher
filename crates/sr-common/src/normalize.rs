use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

static RE_PREFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(?:(?:RE|FW|FWD|ＲＥ|ＦＷ|ＦＷＤ)\s*[:：]\s*)+").unwrap());

static RE_BRACKETS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:[【\[\(（［〔〈《<\{][^】\]\)）］〕〉》>\}]*[】\]\)）］〕〉》>\}]\s*)+")
        .unwrap()
});

/// 件名の正規化（RE/FW 多重プレフィックスと先頭タグを除去）
///
/// 空文字を返さない契約:
/// 1. prefix を剥がして s1 を作る
/// 2. s1 から括弧タグを剥がして s2 を作る
/// 3. s2 が空なら s1 を返す。s1 も空なら元の subject を返す（全て trim 済み）
/// 4. 入力が空なら空文字を返す
pub fn normalize_subject(subject: &str) -> String {
    let original_trimmed = subject.trim();

    if original_trimmed.is_empty() {
        return String::new();
    }

    let s1 = RE_PREFIX.replace(subject, "");
    let s1_trimmed = s1.trim();

    let s2 = RE_BRACKETS.replace(s1_trimmed, "");
    let s2_trimmed = s2.trim();

    if !s2_trimmed.is_empty() {
        s2_trimmed.to_string()
    } else if !s1_trimmed.is_empty() {
        s1_trimmed.to_string()
    } else {
        original_trimmed.to_string()
    }
}

/// 正規化済み件名から SHA-256 で subject_hash を生成（先頭16文字）
pub fn calculate_subject_hash(subject: &str) -> String {
    let normalized = normalize_subject(subject);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let bytes = hasher.finalize();
    let mut hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    hex.truncate(16);
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_multiple_prefixes() {
        assert_eq!(normalize_subject("RE: RE: 【案件】Java開発"), "Java開発");
        assert_eq!(normalize_subject("re: Re: Fwd: 案件紹介"), "案件紹介");
        assert_eq!(normalize_subject("ＦＷ：Java開発"), "Java開発");
    }

    #[test]
    fn normalize_strips_brackets_variants() {
        assert_eq!(normalize_subject("【急募】Python開発"), "Python開発");
        assert_eq!(normalize_subject("[案件] Ruby開発"), "Ruby開発");
        assert_eq!(normalize_subject("【案件】【急募】Java開発"), "Java開発");
        assert_eq!(normalize_subject("[info] [urgent] Ruby案件"), "Ruby案件");
    }

    #[test]
    fn normalize_handles_spaces_and_unicode() {
        assert_eq!(normalize_subject("RE:\t【案件】Java開発"), "Java開発");
        assert_eq!(normalize_subject("FW:　【急募】Ruby案件"), "Ruby案件");
        assert_eq!(
            normalize_subject("RE: 🔥急募🔥 Java案件"),
            "🔥急募🔥 Java案件"
        );
        assert_eq!(normalize_subject("【案件】Ⅰ期開発"), "Ⅰ期開発");
    }

    #[test]
    fn normalize_fallbacks_when_empty_after_strip() {
        assert_eq!(normalize_subject("RE: "), "RE:");
        assert_eq!(normalize_subject("Fwd: [info]"), "[info]");
        assert_eq!(normalize_subject("").as_str(), "");
    }

    #[test]
    fn normalize_subject_hash_matches_expected_prefix() {
        assert_eq!(
            calculate_subject_hash("RE: 【案件】Java開発"),
            "ae5c4b5a8fff1759"
        );
        assert_eq!(calculate_subject_hash("Python案件"), "0ef182a61d9b77a1");
    }
}
