use std::collections::HashMap;

use lazy_static::lazy_static;

lazy_static! {
    /// 都道府県の短縮形 → 正式名称
    pub static ref PREFECTURE_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("北海", "北海道"); m.insert("青森", "青森県"); m.insert("岩手", "岩手県");
        m.insert("宮城", "宮城県"); m.insert("秋田", "秋田県"); m.insert("山形", "山形県");
        m.insert("福島", "福島県"); m.insert("茨城", "茨城県"); m.insert("栃木", "栃木県");
        m.insert("群馬", "群馬県"); m.insert("埼玉", "埼玉県"); m.insert("千葉", "千葉県");
        m.insert("東京", "東京都"); m.insert("神奈", "神奈川県"); m.insert("新潟", "新潟県");
        m.insert("富山", "富山県"); m.insert("石川", "石川県"); m.insert("福井", "福井県");
        m.insert("山梨", "山梨県"); m.insert("長野", "長野県"); m.insert("岐阜", "岐阜県");
        m.insert("静岡", "静岡県"); m.insert("愛知", "愛知県"); m.insert("三重", "三重県");
        m.insert("滋賀", "滋賀県"); m.insert("京都", "京都府"); m.insert("大阪", "大阪府");
        m.insert("兵庫", "兵庫県"); m.insert("奈良", "奈良県"); m.insert("和歌", "和歌山県");
        m.insert("鳥取", "鳥取県"); m.insert("島根", "島根県"); m.insert("岡山", "岡山県");
        m.insert("広島", "広島県"); m.insert("山口", "山口県"); m.insert("徳島", "徳島県");
        m.insert("香川", "香川県"); m.insert("愛媛", "愛媛県"); m.insert("高知", "高知県");
        m.insert("福岡", "福岡県"); m.insert("佐賀", "佐賀県"); m.insert("長崎", "長崎県");
        m.insert("熊本", "熊本県"); m.insert("大分", "大分県"); m.insert("宮崎", "宮崎県");
        m.insert("鹿児", "鹿児島県"); m.insert("沖縄", "沖縄県");
        m
    };

    /// エリアENUM値
    pub static ref AREA_ENUMS: Vec<&'static str> = vec![
        "北海道",
        "東北",
        "関東",
        "中部",
        "関西",
        "中国",
        "四国",
        "九州",
    ];

    /// 都道府県 → エリア マッピング
    pub static ref TODOFUKEN_TO_AREA: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        // 北海道 / 東北
        m.insert("北海道", "北海道");
        m.insert("青森", "東北"); m.insert("青森県", "東北");
        m.insert("岩手", "東北"); m.insert("岩手県", "東北");
        m.insert("宮城", "東北"); m.insert("宮城県", "東北");
        m.insert("秋田", "東北"); m.insert("秋田県", "東北");
        m.insert("山形", "東北"); m.insert("山形県", "東北");
        m.insert("福島", "東北"); m.insert("福島県", "東北");
        // 関東
        m.insert("茨城", "関東"); m.insert("茨城県", "関東");
        m.insert("栃木", "関東"); m.insert("栃木県", "関東");
        m.insert("群馬", "関東"); m.insert("群馬県", "関東");
        m.insert("埼玉", "関東"); m.insert("埼玉県", "関東");
        m.insert("千葉", "関東"); m.insert("千葉県", "関東");
        m.insert("東京", "関東"); m.insert("東京都", "関東");
        m.insert("神奈川", "関東"); m.insert("神奈川県", "関東");
        // 中部
        m.insert("新潟", "中部"); m.insert("新潟県", "中部");
        m.insert("富山", "中部"); m.insert("富山県", "中部");
        m.insert("石川", "中部"); m.insert("石川県", "中部");
        m.insert("福井", "中部"); m.insert("福井県", "中部");
        m.insert("山梨", "中部"); m.insert("山梨県", "中部");
        m.insert("長野", "中部"); m.insert("長野県", "中部");
        m.insert("岐阜", "中部"); m.insert("岐阜県", "中部");
        m.insert("静岡", "中部"); m.insert("静岡県", "中部");
        m.insert("愛知", "中部"); m.insert("愛知県", "中部");
        m.insert("三重", "中部"); m.insert("三重県", "中部");
        // 関西
        m.insert("滋賀", "関西"); m.insert("滋賀県", "関西");
        m.insert("京都", "関西"); m.insert("京都府", "関西");
        m.insert("大阪", "関西"); m.insert("大阪府", "関西");
        m.insert("兵庫", "関西"); m.insert("兵庫県", "関西");
        m.insert("奈良", "関西"); m.insert("奈良県", "関西");
        m.insert("和歌山", "関西"); m.insert("和歌山県", "関西");
        // 中国
        m.insert("鳥取", "中国"); m.insert("鳥取県", "中国");
        m.insert("島根", "中国"); m.insert("島根県", "中国");
        m.insert("岡山", "中国"); m.insert("岡山県", "中国");
        m.insert("広島", "中国"); m.insert("広島県", "中国");
        m.insert("山口", "中国"); m.insert("山口県", "中国");
        // 四国
        m.insert("徳島", "四国"); m.insert("徳島県", "四国");
        m.insert("香川", "四国"); m.insert("香川県", "四国");
        m.insert("愛媛", "四国"); m.insert("愛媛県", "四国");
        m.insert("高知", "四国"); m.insert("高知県", "四国");
        // 九州
        m.insert("福岡", "九州"); m.insert("福岡県", "九州");
        m.insert("佐賀", "九州"); m.insert("佐賀県", "九州");
        m.insert("長崎", "九州"); m.insert("長崎県", "九州");
        m.insert("熊本", "九州"); m.insert("熊本県", "九州");
        m.insert("大分", "九州"); m.insert("大分県", "九州");
        m.insert("宮崎", "九州"); m.insert("宮崎県", "九州");
        m.insert("鹿児島", "九州"); m.insert("鹿児島県", "九州");
        m.insert("沖縄", "九州"); m.insert("沖縄県", "九州");
        m
    };
}

/// 都道府県補正: 短縮形や部分一致から正式名称に変換
pub fn correct_todofuken(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let all_prefs: Vec<_> = PREFECTURE_MAP.values().cloned().collect();
    if all_prefs.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    for (key, value) in PREFECTURE_MAP.iter() {
        if trimmed == *key || trimmed.starts_with(*key) {
            return Some(value.to_string());
        }
    }

    if trimmed.chars().count() >= 2 {
        let prefix: String = trimmed.chars().take(2).collect();
        if let Some(value) = PREFECTURE_MAP.get(prefix.as_str()) {
            return Some(value.to_string());
        }
    }

    None
}

/// エリアENUM補正
pub fn correct_work_area(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if AREA_ENUMS.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    let legacy_mapping = match trimmed {
        "北海道・東北" => Some("北海道"),
        "甲信越" | "北陸" | "東海" | "甲信越・北陸" => Some("中部"),
        "近畿" => Some("関西"),
        "中国・四国" => Some("中国"),
        "九州・沖縄" => Some("九州"),
        "首都圏" => Some("関東"),
        _ => None,
    };
    if let Some(area) = legacy_mapping {
        return Some(area.to_string());
    }

    if let Some(area) = TODOFUKEN_TO_AREA.get(trimmed) {
        return Some(area.to_string());
    }

    for area in AREA_ENUMS.iter() {
        if trimmed.contains(*area) {
            return Some(area.to_string());
        }
    }

    for (pref, area) in TODOFUKEN_TO_AREA.iter() {
        if trimmed.contains(*pref) {
            return Some(area.to_string());
        }
    }

    let lower = trimmed.to_lowercase();
    if lower.contains("リモート")
        || lower.contains("在宅")
        || lower.contains("全国")
        || lower.contains("不問")
    {
        return None;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_prefecture_names() {
        assert_eq!(correct_todofuken("東京"), Some("東京都".to_string()));
        assert_eq!(correct_todofuken("東京都"), Some("東京都".to_string()));
        assert_eq!(correct_todofuken("神奈"), Some("神奈川県".to_string()));
        assert_eq!(correct_todofuken(""), None);
    }

    #[test]
    fn work_area_supports_new_and_legacy() {
        assert_eq!(correct_work_area("関東"), Some("関東".to_string()));
        assert_eq!(correct_work_area("関西"), Some("関西".to_string()));
        assert_eq!(correct_work_area("中部"), Some("中部".to_string()));
        assert_eq!(correct_work_area("甲信越・北陸"), Some("中部".to_string()));
    }

    #[test]
    fn work_area_maps_prefectures() {
        assert_eq!(correct_work_area("東京都"), Some("関東".to_string()));
        assert_eq!(correct_work_area("愛知県"), Some("中部".to_string()));
        assert_eq!(correct_work_area("北海道"), Some("北海道".to_string()));
    }

    #[test]
    fn work_area_handles_remote_and_unknowns() {
        assert_eq!(correct_work_area("フルリモート"), None);
        assert_eq!(correct_work_area("全国可"), None);
        assert_eq!(correct_work_area("未知"), None);
    }
}
