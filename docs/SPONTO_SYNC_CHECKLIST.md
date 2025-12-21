# sponto-platform 同期チェックリスト

**作成日**: 2025-12-20
**最終更新**: 2025-12-20
**目的**: rust-matcher と sponto-platform 間の抜け・不整合を追跡し、実装者が対応できるようにする

**参照元（sponto-platform）**:
- `scripts/shared/enum_corrections.js` - ENUM補正ロジック
- `apps/business-api/app/core/skill_processor.py` - スキルエイリアス辞書
- `apps/business-api/database/tables/core/projects_enum.sql` - 案件テーブルDDL
- `apps/business-api/database/tables/core/direct_talents.sql` - 直人材テーブルDDL
- `apps/business-api/database/tables/core/bp_talents_enum.sql` - BP人材テーブルDDL
- `apps/business-api/database/tables/core/matching_pairs.sql` - マッチングテーブルDDL

---

## 凡例

- 🔴 **Critical**: KO判定/スコアリングに直接影響。即対応必須
- 🟡 **Important**: マッチング精度に影響。短期対応推奨
- 🟢 **Nice-to-have**: 将来対応可。なくても動作する

---

## 1. ENUM補正ロジックの抜け

### 🔴 ENUM-1: `correct_contract_type()` 案件契約形態補正

**現状**: rust-matcherには人材用の`correct_talent_contract_type()`はあるが、**案件用**の契約形態補正がない

**参照 (enum_corrections.js:163-177)**:
```javascript
function correctContractType(contractString) {
  if (!contractString || typeof contractString !== 'string') {
    return '準委任契約'; // Default
  }
  const trimmed = contractString.trim();
  if (ENUMS.CONTRACT_TYPE.includes(trimmed)) return trimmed;
  if (trimmed.includes('派遣')) return '派遣';
  return '準委任契約'; // Default
}
// ENUMS.CONTRACT_TYPE = ['準委任契約', '派遣']
```

**対応ファイル**: `crates/sr-common/src/corrections/contract_type.rs`

**追加コード**:
```rust
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

#[cfg(test)]
mod contract_type_tests {
    use super::*;

    #[test]
    fn corrects_contract_type_for_projects() {
        assert_eq!(correct_contract_type("準委任契約"), "準委任契約");
        assert_eq!(correct_contract_type("派遣契約"), "派遣");
        assert_eq!(correct_contract_type(""), "準委任契約");
        assert_eq!(correct_contract_type("業務委託"), "準委任契約");
    }
}
```

**Done条件**:
- [x] `correct_contract_type()` が `contract_type.rs` に追加されている
- [x] テストが通る
- [x] `mod.rs` で re-export されている

---

## 2. Project 構造体のフィールド抜け

### 🔴 PROJ-1: `monthly_tanka_min` 単価レンジ下限

**現状**: `Project` に `monthly_tanka_max` はあるが `monthly_tanka_min` がない

**参照 (projects_enum.sql:47-48)**:
```sql
monthly_tanka_min INTEGER
    CONSTRAINT check_monthly_tanka_min_positive
    CHECK (monthly_tanka_min > 0),
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub monthly_tanka_min: Option<u32>,  // 追加
    pub monthly_tanka_max: Option<u32>,
    // ...
}
```

**影響範囲**:
- 単価KO判定（`check_tanka_ko`）で min/max 両方を考慮すべき
- prefilter の単価スコア計算

**Done条件**:
- [x] `Project` に `monthly_tanka_min` フィールドが追加されている
- [x] 単価関連のKO判定・スコアリングで使用されている

---

### 🔴 PROJ-2: `flow_dept` 商流深度

**現状**: `Project` に `jinzai_flow_limit`（人材商流制限）はあるが、案件自体の商流深度 `flow_dept` がない

**参照 (projects_enum.sql:81)**:
```sql
flow_dept ses.flow_dept_enum, -- Business flow depth
-- ENUM: 'エンド直', '1次請け', '2次請け', '3次請け', '4次請け以上'
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub flow_dept: Option<String>,  // 追加: "エンド直", "1次請け", etc.
    pub jinzai_flow_limit: Option<String>,
    // ...
}
```

**影響範囲**:
- 商流情報の表示・ログ出力
- 将来的な商流ベースのフィルタリング

**Done条件**:
- [x] `Project` に `flow_dept` フィールドが追加されている

---

### 🟡 PROJ-3: `work_station` 最寄駅

**現状**: `Project` に勤務地の最寄駅フィールドがない

**参照 (projects_enum.sql:40)**:
```sql
work_station VARCHAR(255), -- Nearest station (e.g., '新宿駅', '渋谷駅')
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub work_station: Option<String>,  // 追加
    // ...
}
```

**影響範囲**:
- 勤務地マッチングの精度向上（都道府県より細かい粒度）
- Talent の `nearest_station` との比較

**Done条件**:
- [x] `Project` に `work_station` フィールドが追加されている
- [x] `normalize_station()` を通して格納される

---

### 🟡 PROJ-4: `project_type` 案件タイプ

**現状**: `Project` に案件タイプ（PM, SE等）フィールドがない

**参照 (projects_enum.sql:23)**:
```sql
project_type TEXT[], -- Array of project types (e.g., ['PM', 'SE'])
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub project_type: Option<Vec<String>>,  // 追加: ["PM", "SE"] etc.
    // ...
}
```

**影響範囲**:
- 案件タイプベースのマッチング
- Talent の希望案件タイプとの照合

**Done条件**:
- [x] `Project` に `project_type` フィールドが追加されている

---

### 🟢 PROJ-5: `onsite_frequency` 出社頻度

**現状**: `Project` に週何日出社かのフィールドがない

**参照 (projects_enum.sql:42-44)**:
```sql
onsite_frequency REAL
    CONSTRAINT check_onsite_frequency_valid
    CHECK (onsite_frequency >= 0 AND onsite_frequency <= 7),
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub onsite_frequency: Option<f32>,  // 追加: 週あたり出社日数
    // ...
}
```

**Done条件**:
- [x] `Project` に `onsite_frequency` フィールドが追加されている

---

### 🟢 PROJ-6: `settlement_range` 精算幅

**現状**: `Project` に精算幅フィールドがない

**参照 (projects_enum.sql:53)**:
```sql
settlement_range VARCHAR(50), -- e.g., '140h-180h', '固定'
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub settlement_range: Option<String>,  // 追加: "140h-180h" etc.
    // ...
}
```

**Done条件**:
- [x] `Project` に `settlement_range` フィールドが追加されている

---

### 🟢 PROJ-7: `interviews_count` 面談回数

**現状**: `Project` に面談回数フィールドがない

**参照 (projects_enum.sql:71-73)**:
```sql
interviews_count SMALLINT DEFAULT 2
    CONSTRAINT check_interviews_count_positive
    CHECK (interviews_count > 0),
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub interviews_count: Option<i32>,  // 追加: デフォルト2
    // ...
}
```

**Done条件**:
- [x] `Project` に `interviews_count` フィールドが追加されている

---

### 🟢 PROJ-8: `hiring_headcount` 募集人数

**現状**: `Project` に募集人数フィールドがない

**参照 (projects_enum.sql:18-20)**:
```sql
hiring_headcount SMALLINT DEFAULT 1
    CONSTRAINT check_hiring_headcount_positive
    CHECK (hiring_headcount > 0),
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Project 構造体に追加
pub struct Project {
    // ... existing fields ...
    pub hiring_headcount: Option<i32>,  // 追加: デフォルト1
    // ...
}
```

**Done条件**:
- [x] `Project` に `hiring_headcount` フィールドが追加されている

---

## 3. Talent 構造体のフィールド抜け

### 🟡 TAL-1: `gender` 性別

**現状**: `Talent` に性別フィールドがない（`correct_gender()` は存在する）

**参照 (direct_talents.sql:41)**:
```sql
gender ses.talent_gender_enum,
-- ENUM: '男性', '女性', 'その他/無回答'
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Talent 構造体に追加
pub struct Talent {
    // ... existing fields ...
    pub gender: Option<String>,  // 追加: "男性", "女性", "その他/無回答"
    // ...
}
```

**影響範囲**:
- 案件の性別制限がある場合のKO判定（稀だが存在）

**Done条件**:
- [x] `Talent` に `gender` フィールドが追加されている
- [x] `correct_gender()` を通して格納される

---

### 🟡 TAL-2: `nearest_station` 最寄駅

**現状**: `Talent` に最寄駅フィールドがない

**参照 (direct_talents.sql:47)**:
```sql
nearest_station VARCHAR(255),
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Talent 構造体に追加
pub struct Talent {
    // ... existing fields ...
    pub nearest_station: Option<String>,  // 追加
    // ...
}
```

**影響範囲**:
- 勤務地マッチングの精度向上
- Project の `work_station` との距離計算

**Done条件**:
- [x] `Talent` に `nearest_station` フィールドが追加されている
- [x] `normalize_station()` を通して格納される

---

### 🟡 TAL-3: `desired_remote_onsite` 希望勤務形態

**現状**: `Talent` に希望するリモート/出社形態フィールドがない

**参照 (direct_talents.sql:48)**:
```sql
desired_remote_onsite ses.remote_onsite_enum,
-- ENUM: 'フル出社', 'リモート併用', 'フルリモート'
```

**対応ファイル**: `crates/sr-common/src/lib.rs`

**追加コード**:
```rust
// Talent 構造体に追加
pub struct Talent {
    // ... existing fields ...
    pub desired_remote_onsite: Option<String>,  // 追加
    // ...
}
```

**影響範囲**:
- 勤務形態マッチング
- フルリモート希望者 vs フル出社案件のKO判定

**Done条件**:
- [x] `Talent` に `desired_remote_onsite` フィールドが追加されている
- [x] `correct_remote_onsite()` を通して格納される

---

## 4. SKILL_ALIASES の確認

### 🟡 SKILL-1: スキルエイリアス辞書の差異確認

**現状**: rust-matcher (`skill_normalizer.rs`) と sponto-platform (`skill_processor.py`) で SKILL_ALIASES が定義されているが、微妙な差異がある

**参照ファイル**:
- rust-matcher: `crates/sr-common/src/skill_normalizer.rs`
- sponto-platform: `apps/business-api/app/core/skill_processor.py`

**差異の例**:

| canonical | rust-matcher | sponto-platform | 差異 |
|-----------|-------------|-----------------|------|
| `nextjs` | `"next.js", "nextjs", "next js"` | `"nextjs", "next js", "next.js"` (canonical: `next.js`) | canonical名が異なる |
| `css` | `"css", "css3", ...` | `"css3", ...` (canonicalなし) | canonical含有有無 |

**方針**: sponto-platformは凍結のため、**rust-matcherを正**とする。差異があってもrust-matcherの定義を優先。

**Done条件**:
- [ ] 差異を把握済み（上記表で確認）
- [x] rust-matcherを正とする方針決定済み

---

## 5. DB スキーマ関連

> **Note**: sponto-platformは凍結のため、rust-matcherが新規テーブルを定義し、既存テーブルとは共存する方針。

### 🟢 DB-1: `match_results` vs `matching_pairs` テーブル

**現状**: rust-matcher は `ses.match_results` を新規定義、sponto-platform は `ses.matching_pairs` を使用

**rust-matcher (schema.rs)**:
```sql
CREATE TABLE ses.match_results (
    id SERIAL PRIMARY KEY,
    talent_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    is_knockout BOOLEAN NOT NULL,
    ko_reasons JSONB,
    needs_manual_review BOOLEAN NOT NULL DEFAULT false,
    score_total FLOAT,
    score_breakdown JSONB,
    engine_version VARCHAR(20),
    rule_version VARCHAR(20),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(talent_id, project_id, created_at::date)
);
```

**sponto-platform (matching_pairs.sql)**: 別構造（詳細はsponto-platform参照）

**方針**: migration期間中は両テーブルが共存。rust-matcherは`match_results`に書き込み、既存システムは`matching_pairs`を参照。

**Done条件**:
- [ ] `match_results` DDL が本番環境に適用済み
- [ ] 必要に応じて `matching_pairs` への同期処理を実装

---

### 🟢 DB-2: `extraction_queue` テーブル

**現状**: rust-matcher 独自のキューテーブル。sponto-platform には対応テーブルなし

**rust-matcher (schema.rs)**:
```sql
CREATE TABLE ses.extraction_queue (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    email_subject TEXT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    -- ... その他フィールド
);
```

**方針**: rust-matcher専用。sponto-platformとの連携は `message_id` をキーとして行う。

**Done条件**:
- [ ] `extraction_queue` DDL が本番環境に適用済み

---

## 6. BP人材対応（Phase 2 参考情報）

### 🟢 BP-1: `bp_talents_enum` スキーマ差異

**現状**: MVPは `direct_talents`（直人材）のみ対応。Phase 2 で `bp_talents_enum`（BP人材）対応予定。

**BP人材特有のフィールド**:

| フィールド | 型 | 説明 | direct_talents との差異 |
|-----------|---|------|------------------------|
| `company_name` | VARCHAR(255) | BP会社名 | direct_talentsにはない |
| `flow_depth` | ENUM | 商流深度 | NOT NULL（直人材はOptional） |
| `is_shienhi_ok` | BOOLEAN | 支援費OK | direct_talentsにはない |
| `availability_status` | ENUM | 営業ステータス | `eigyo_status` と異なるENUM |
| `message_id` | VARCHAR(255) | メールID | direct_talentsにはない |
| `source_text` | TEXT | 原文 | direct_talentsにはない |

**BP人材用 ENUM 値（参考）**:
```sql
-- talent_availability_status_enum
'営業中', '営業終了'

-- talent_flow_depth_enum
'1社先', '2社先', '3社先以上'
```

**Done条件**:
- [ ] Phase 2 で `BpTalent` 構造体を追加
- [ ] `talent_type` フラグでDirect/BP を区別

---

### 🟢 BP-2: direct_talents 特有フィールド

**BP人材にはないが direct_talents にあるフィールド**:

| フィールド | 説明 | 用途 |
|-----------|------|------|
| `eigyo_status` | SPONTO内部営業ステータス | レビュー待ち/営業中/デモ可能/etc |
| `source` | 獲得元 | LinkedIn, Indeed, etc |
| `email`, `phone`, `linkedin_url` | 連絡先 | 直接連絡用 |
| `internal_resume_url` | 内部履歴書URL | 詳細スキルシート |
| `public_resume_url` | 匿名履歴書URL | クライアント提出用 |
| `email_subject_template` | メール件名テンプレ | 人材配信用 |
| `email_body_template` | メール本文テンプレ | 人材配信用 |

---

## 7. フィールド名の差異（要注意）

以下はDB側と rust-matcher 側で名前が異なるが、意味的には同じフィールド。
実装時に混乱しないよう記録しておく。

| DB (sponto-platform) | rust-matcher | 備考 |
|---------------------|--------------|------|
| `desired_monthly_tanka` | `desired_price_min` | 単価希望（下限） |
| `skill_keywords` | `possessed_skills_keywords` | 保有スキル |
| `total_experience_years` | `min_experience_years` | 経験年数 |

---

## 実装優先順位まとめ

### Phase 1（即対応）- KO判定/スコアリング直接影響
1. 🔴 ENUM-1: `correct_contract_type()` 追加
2. 🔴 PROJ-1: `monthly_tanka_min` 追加
3. 🔴 PROJ-2: `flow_dept` 追加

### Phase 2（短期対応）- マッチング精度向上
4. 🟡 PROJ-3: `work_station` 追加
5. 🟡 PROJ-4: `project_type` 追加
6. 🟡 TAL-1: `gender` 追加
7. 🟡 TAL-2: `nearest_station` 追加
8. 🟡 TAL-3: `desired_remote_onsite` 追加
9. 🟡 SKILL-1: スキルエイリアス辞書の同期確認

### Phase 3（将来対応）- Nice-to-have
10. 🟢 PROJ-5〜8: `onsite_frequency`, `settlement_range`, `interviews_count`, `hiring_headcount`
11. 🟢 DB-1, DB-2: DBスキーマ適用
12. 🟢 BP-1, BP-2: BP人材対応（Phase 2以降）

---

## クイックリファレンス

### ENUM値一覧（DB定義との整合確認用）

| ENUM名 | 値 |
|--------|---|
| `remote_onsite_enum` | `フル出社`, `リモート併用`, `フルリモート` |
| `contract_type_enum` | `準委任契約`, `派遣` |
| `talent_contract_type_enum` | `正社員`, `契約社員`, `直個人` |
| `japanese_skill_enum` | `不要`, `N5`, `N4`, `N3`, `N2`, `N1`, `ネイティブ` |
| `english_skill_enum` | `不要`, `読み書き`, `会話`, `ビジネス`, `上級ビジネス`, `ネイティブ` |
| `flow_dept_enum` | `エンド直`, `1次請け`, `2次請け`, `3次請け`, `4次請け以上` |
| `talent_flow_depth_enum` | `1社先`, `2社先`, `3社先以上` |
| `jinzai_flow_limit_enum` | `SPONTO直人材`, `SPONTO一社先まで`, `商流制限なし` |
| `tech_kubun_enum` | `生成AI関連`, `人気技術`, `レガシー` |
| `talent_gender_enum` | `男性`, `女性`, `その他/無回答` |
| `work_area_enum` | `北海道`, `東北`, `関東`, `中部`, `関西`, `中国`, `四国`, `九州` |

### 参照ファイルパス（sponto-platform）

| 種類 | パス |
|------|------|
| ENUM補正JS | `scripts/shared/enum_corrections.js` |
| スキル処理 | `apps/business-api/app/core/skill_processor.py` |
| 案件DDL | `apps/business-api/database/tables/core/projects_enum.sql` |
| 直人材DDL | `apps/business-api/database/tables/core/direct_talents.sql` |
| BP人材DDL | `apps/business-api/database/tables/core/bp_talents_enum.sql` |
| マッチングDDL | `apps/business-api/database/tables/core/matching_pairs.sql` |

---

## 更新履歴

| 日付 | 更新者 | 内容 |
|------|--------|------|
| 2025-12-20 | Claude | 初版作成。enum_corrections.js, projects_enum.sql, direct_talents.sql との比較結果を記録 |
| 2025-12-20 | Claude | SKILL_ALIASES同期、DBスキーマ関連、BP人材参考情報、ENUM値一覧を追加 |
| 2025-12-20 | Claude | sponto-platform凍結に伴い、逆同期項目（SYNC-1: tech_kubun）を削除。rust-matcherを正とする方針に更新 |
| 2025-12-20 | Claude | 勤務地判定で最寄駅一致/不一致を考慮し、駅レベルでのPass/SoftKo分岐を追加 |
| 2025-12-21 | Claude | リモート希望と案件リモート形態の整合チェックを追加し、未設定時はSoftKo、フルリモート希望×フル出社案件はHardKoに降格するよう調整 |
| 2025-12-21 | Claude | 案件リモート未設定時の挙動を精緻化し、フル出社希望は減点なし、リモート併用希望は軽微なSoftKo、フルリモート希望は従来通りのSoftKoに分岐 |
