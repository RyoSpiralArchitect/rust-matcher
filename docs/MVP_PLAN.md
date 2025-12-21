# 直人材 × BP案件 マッチング MVP 仕様書

**作成日**: 2025-01-15
**最終更新**: 2025-12-18
**実装言語**: Rust
**対象**: coding LLM が実装できるレベルの完全仕様書
**スコープ**: BP各社から来る案件メールをパース → 直人材とのマッチング

---

## 🎯 MVPスコープ（これだけ読めばOK）

### IN（今回やること）

| 項目 | 説明 | 優先度 |
|------|------|--------|
| **BP案件メールパース** | BP各社から来る案件紹介メール → Rustで情報抽出 → `projects_enum` に保存 | 🔴 必須 |
| **ENUM補正ロジック** | 都道府県・商流・契約形態等の表記揺れ補正 | 🔴 必須 |
| **KO条件判定** | マッチング時の即時不合格判定（単価・スキル・勤務地等） | 🔴 必須 |
| **基本スコアリング** | KO通過後のマッチングスコア算出 | 🟡 推奨 |

### OUT（今回やらないこと）

| 項目 | 理由 | Phase |
|------|------|-------|
| BP人材パース | 既存n8n+LLMフローで動作中 | Phase 2 |
| PDF/CVパース | DeepSeekがPDF読めない、依存増加 | Phase 2+ |
| セマンティックマッチング | ML/ベクトル検索は既存Python API使用 | 対象外 |

**📎 PDF/CVパースがPhase 2+の理由（Loc説明 2025-12-15）**:

> 「BPのメールの文面より職務経歴書の方が情報が正確だから。BP要員の概要文書は時々正しくないことがある。例えば『LLMすごい強い』と書いてあるのにスキルシートを開くと、結局Claudeのインストール経験しかないとか」

```
現状の問題:
  BP人材メール本文 → 概要が誇張されていることがある
  添付PDF（職務経歴書）→ より正確な情報が記載

なぜMVPでやらないか:
  1. DeepSeekがPDFを処理できなかった（2024年8月時点）
  2. PDFパースはトークン消費が多い（コスト増）
  3. 依存ライブラリが増える（複雑性増）
```

**⚠️ 「PDF→一発JSON化」が揺らぐ構造的理由**:

新しいモデル（Claude 3.5 Sonnet等）でも**本質的に不安定**。なぜなら：

```
LLMに同時に要求している処理:
  1. 視覚的・レイアウト的ノイズを含むPDF → 線形テキスト化
     - 行順の復元
     - 箇条書き・表の再解釈
     - セクション境界の推定
  2. 意味理解・正規化
     - スキル語彙の同定
     - 年数・単価・期間の抽出
     - 冗長表現の圧縮
  3. 構造化（JSONスキーマへの射影）

→ 1プロンプト1レスポンスで同時要求すると:
  - 注意が分散する
  - 途中の中間表現が安定しない
  - モデルの「どこで失敗したか」が不可視

結果: たまに合うが、再現しない
```

**Phase 2+でのアプローチ（フューチャーフラグ: `ENABLE_PDF_EXTRACTION`）**:

```
┌─────────────────────────────────────────────────────────────┐
│  PDF抽出機能自体がフューチャーフラグで段階的導入            │
│  フラグOFF（MVP）: PDFは処理しない                          │
│  フラグON（Phase 2+）: 以下の優先順位で処理                 │
└─────────────────────────────────────────────────────────────┘

【優先度1: メイン路線】sr-extractor + 正規化 + 最小LLM補正
  ─────────────────────────────────────────────────────
  PDF / 文面
       ↓
  sr-extractor で文字ベース抽出
       ↓
  正規化・揺らぎ吸収
    - スキル辞書
    - 表記ゆれ
    - typo吸収
       ↓
  LLMに「最小限のお願い」
    - 誤字修正
    - JSON構造化

  ✅ メール抽出と同じロジック → 実績あり、シンプル、低コスト
  ✅ 語彙分布が同じなら、まず同じロジックで殴るのが正しい

【優先度2: フォールバック】PDF→Text→JSON（2段LLM）
  ─────────────────────────────────────────────────────
  条件: 優先度1で抽出精度が不十分な場合

  Stage 1: PDF → 純テキスト
    - ゴールが単純：「読める文字列にする」
    - JSON制約なし、多少の揺れは許容
  Stage 2: テキスト → 構造化JSON
    - 入力が既に線形テキスト
    - レイアウトノイズ消失
    - スキーマ制約に集中できる

  ⚠️ API 2回呼び = コスト2倍、遅延2倍、オーケストレーション複雑化
  → 優先度1が失敗した時のみ使用

【優先度3: 最終手段】OCR含むフルパイプライン
  ─────────────────────────────────────────────────────
  条件: 画像化されたPDF、スキャン文書
  - 依存ライブラリ増、処理重い
  - 本当に必要な場合のみ
```

**要点**: PDF抽出自体がPhase 2+のフューチャーフラグ。実装時は既存extractorを優先し、それで不十分な場合のみ2段LLMにフォールバック。

**📌 既存実装との関係**:
> sponto-platform には **メールテキスト用** の Two-Stage Extraction が既に本番稼働中（`AIExtractionService`）。
> 上記のPDF用2段LLMは、このメール用実装を **PDFに応用** するアプローチ。
> 詳細は「13. AI抽出サービス」セクション参照。

### データソースの明確化

```
【直人材データ】
  ソース: 既に登録済み（マーケチームが書類選考 → DB登録）
  テーブル: ses.talents_enum（または ses.direct_talents）
  本システムでの扱い: 読み取りのみ（マッチング対象として参照）

【BP案件データ】 ← ★本システムのパース対象
  ソース: BP各社から来る「案件紹介」メール（anken_emails）
  テーブル: ses.projects_enum
  本システムでの扱い: メール本文からパース → 抽出 → INSERT

【既存フローとの関係】
  - BP人材パース: 既存 n8n + LLM フローを継続使用（変更なし）
  - セマンティックマッチング: 既存 Python API を継続使用（変更なし）
```

**⚠️ 重要**: 直人材の情報は既にDB登録済み。本システムがパースするのは「BP各社から来る案件メール」である。

---

## 🔒 正（Canonical）設計決定（2025-12-17 P0/High解決）

> **背景**: レビューで発見された矛盾・不整合を解決するための**唯一の正（single source of truth）**を定義する。
> これ以降のコード例が本セクションと矛盾する場合、**本セクションが正**である。

### ⚠️ sponto-platform との同期要件

本仕様書の以下の定義は **sponto-platform** と同期が必要:

| 本仕様書 | sponto-platform 対応ファイル | 同期方向 |
|----------|------------------------------|----------|
| `REMOTE_ONSITE_ENUMS` | `scripts/shared/enum_corrections.js` | 双方向 |
| `FLOW_DEPT_ENUMS` | `scripts/shared/enum_corrections.js` | 双方向 |
| `SKILL_ALIASES` | `apps/business-api/app/core/skill_processor.py` | 双方向 |
| `infer_tech_kubun` キーワード | `scripts/shared/enum_corrections.js` → `correctTechKubun()` | 双方向 |
| `correct_*` 補正パターン | `scripts/shared/enum_corrections.js` | 双方向 |

**同期ルール**:
- 片方を修正したら、もう片方も同じ変更を適用する
- 本仕様書に `⚠️ sponto-platform ... と同期すること` コメントがある箇所は特に注意
- 不整合発見時は本仕様書を正とする（ただしsponto-platformがプロダクション稼働中の場合は要相談）

---

### CD-1: 勤務地判定の統一（P0-1解決）

**問題**: KO判定は`work_todofuken`、スコアは`work_area`を使用しており不整合。

**正（Canonical）**: 勤務地判定は **1つの関数** から KO/スコア両方に同じ解釈結果を返す。

```rust {#normative}
// [Normative] 勤務地判定ロジックは正規であり、実装は必ず従うこと
// sr-common/src/matching/location.rs
//
// 【仕様契約 SC-LOC: 勤務地判定の統一】
//
// 入力: project.work_todofuken?, project.work_area?, talent.residential_todofuken?,
//       talent.residential_area?, project.remote_onsite
// 出力: LocationEvaluation { ko_decision, score_component }

#[derive(Debug, Clone)]
pub struct LocationEvaluation {
    pub ko_decision: KoDecision,
    pub score: f64,  // 0.0〜1.0
    pub details: String,
}

/// 【唯一の勤務地判定関数】
/// KO判定・prefilter・スコアリング全てがこの関数を呼ぶこと。
pub fn evaluate_location(
    project: &Project,
    talent: &Talent,
) -> LocationEvaluation {
    // 1. フルリモート → 勤務地KOなし、スコア1.0
    if project.remote_onsite.as_deref() == Some("フルリモート") {
        return LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 1.0,
            details: "フルリモート案件 - 勤務地制約なし".into(),
        };
    }

    // 2. 都道府県が双方ある → 都道府県ロジック
    if let (Some(p_pref), Some(t_pref)) = (
        project.work_todofuken.as_deref(),
        talent.residential_todofuken.as_deref(),
    ) {
        return evaluate_by_todofuken(p_pref, t_pref);
    }

    // 3. 片方でも都道府県がない → エリアで粗く判定
    if let (Some(p_area), Some(t_area)) = (
        project.work_area.as_deref(),
        talent.residential_area.as_deref(),
    ) {
        return evaluate_by_area(p_area, t_area);
    }

    // 4. どっちも取れない → SoftKo（手動レビューへ）
    LocationEvaluation {
        ko_decision: KoDecision::SoftKo {
            reason: "location_unknown: 勤務地情報不足のため要手動確認".into(),
        },
        score: 0.5,  // 中立
        details: "勤務地情報なし - 手動確認必要".into(),
    }
}

fn evaluate_by_todofuken(project_pref: &str, talent_pref: &str) -> LocationEvaluation {
    if project_pref == talent_pref {
        // 同一都道府県
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 1.0,
            details: format!("都道府県一致: {}", project_pref),
        }
    } else if is_adjacent_prefecture(project_pref, talent_pref) {
        // 隣接都道府県（通勤圏内）
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 0.7,
            details: format!("隣接都道府県: {} ↔ {}", talent_pref, project_pref),
        }
    } else {
        // 遠隔（HardKoではなくSoftKo: リモート併用なら通えるかも）
        LocationEvaluation {
            ko_decision: KoDecision::SoftKo {
                reason: format!("location_distant: {} → {} は通勤困難の可能性", talent_pref, project_pref),
            },
            score: 0.2,
            details: format!("都道府県不一致: {} ≠ {}", talent_pref, project_pref),
        }
    }
}

fn evaluate_by_area(project_area: &str, talent_area: &str) -> LocationEvaluation {
    if project_area == talent_area {
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 0.8,  // 都道府県より粗いので0.8
            details: format!("エリア一致: {}", project_area),
        }
    } else {
        LocationEvaluation {
            ko_decision: KoDecision::SoftKo {
                reason: format!("area_mismatch: {} ≠ {}", talent_area, project_area),
            },
            score: 0.3,
            details: format!("エリア不一致: {} ≠ {}", talent_area, project_area),
        }
    }
}

/// 隣接都道府県テーブル（例: 東京↔神奈川, 大阪↔京都）
fn is_adjacent_prefecture(a: &str, b: &str) -> bool {
    const ADJACENT_PAIRS: &[(&str, &str)] = &[
        ("東京都", "神奈川県"), ("東京都", "埼玉県"), ("東京都", "千葉県"),
        ("神奈川県", "埼玉県"), ("神奈川県", "千葉県"),
        ("大阪府", "京都府"), ("大阪府", "兵庫県"), ("大阪府", "奈良県"),
        ("愛知県", "岐阜県"), ("愛知県", "三重県"),
        // ... 必要に応じて拡張
    ];
    ADJACENT_PAIRS.iter().any(|(x, y)| {
        (a == *x && b == *y) || (a == *y && b == *x)
    })
}
```

**重要**: `HardKo` は「確実に無理」な時だけ。曖昧な時は `SoftKo`（= 自動マッチ不可、手動レビューへ）が "conservative" の本質。

**Done条件**:
- [ ] KO/Prefilter/Scoring のどこにも独自の勤務地判定が存在しない
- [ ] `evaluate_location()` が唯一の勤務地判定関数
- [ ] 「都道府県あり/なし」「エリアのみ」ケースのテストがある

---

### CD-2: KO判定体系の統一（P0-2解決）

**問題**: 従来の `check_knockout() -> bool` と新しい `KoDecision`（3値enum）が並立。

**正（Canonical）**: `KoDecision`（3値）+ `MatchResult` を統一仕様とする。

```rust {#normative}
// [Normative] この型定義は正規（カノニカル）であり、実装は必ず従うこと
// sr-common/src/matching/ko_unified.rs
//
// 【仕様契約 SC-KO-UNIFIED: KO判定フロー統一】
//
// KO判定を3値enum + MatchResult で統一することで:
//   1. HardKo/SoftKo/Pass の明確な分離
//   2. auto_match_eligible / manual_review_required の自動計算
//   3. 旧bool方式の完全廃止
//
// ⚠️ bool版 check_knockout() は廃止。旧コードは移行必須。
// ⚠️ Python strategy.py も同じ3値体系に統一すること。

// ─────────────────────────────────────────────────────────────
// Part 1: KoDecision enum（判定結果の3値表現）
// ─────────────────────────────────────────────────────────────

/// KO判定結果（唯一の正）
#[derive(Debug, Clone, PartialEq)]
pub enum KoDecision {
    /// 確実にKO（即時除外、スコアリング不要）
    /// 例: 利益5万円未満、商流制限違反、スキルマッチ率<30%
    HardKo { reason: String },

    /// 要手動確認（スコアリングは行うが manual_review=true）
    /// 例: required_skills が空、日本語スキル不明
    SoftKo { reason: String },

    /// 問題なし（次のチェックへ）
    Pass,
}

impl KoDecision {
    /// HardKo の場合のみ true
    pub fn is_hard_ko(&self) -> bool {
        matches!(self, KoDecision::HardKo { .. })
    }

    /// SoftKo の場合のみ true（manual_review が必要）
    pub fn is_soft_ko(&self) -> bool {
        matches!(self, KoDecision::SoftKo { .. })
    }

    /// manual_review が必要な場合 true（= is_soft_ko のエイリアス）
    pub fn needs_manual_review(&self) -> bool {
        self.is_soft_ko()
    }

    /// KO reason を取得（Pass の場合は None）
    pub fn reason(&self) -> Option<&str> {
        match self {
            KoDecision::HardKo { reason } => Some(reason),
            KoDecision::SoftKo { reason } => Some(reason),
            KoDecision::Pass => None,
        }
    }

    /// KO かどうか（HardKo または SoftKo）
    pub fn is_ko(&self) -> bool {
        !matches!(self, KoDecision::Pass)
    }
}

// ─────────────────────────────────────────────────────────────
// Part 2: KnockoutResultV2（複合KO判定の集約）
// ─────────────────────────────────────────────────────────────

/// 複合KO判定結果（全チェック項目を集約）
pub struct KnockoutResultV2 {
    /// いずれかが HardKo なら true
    pub is_hard_knockout: bool,
    /// SoftKo が1つ以上あれば true
    pub needs_manual_review: bool,
    /// 全ての判定結果（チェック名, 判定）
    pub decisions: Vec<(&'static str, KoDecision)>,
}

impl KnockoutResultV2 {
    pub fn new(decisions: Vec<(&'static str, KoDecision)>) -> Self {
        let is_hard_knockout = decisions.iter().any(|(_, d)| d.is_hard_ko());
        let needs_manual_review = decisions.iter().any(|(_, d)| d.needs_manual_review());

        Self {
            is_hard_knockout,
            needs_manual_review,
            decisions,
        }
    }

    /// manual_review_reason を生成（SoftKo の理由を ; 区切りで連結）
    pub fn manual_review_reasons(&self) -> Option<String> {
        let soft_reasons: Vec<_> = self.decisions.iter()
            .filter_map(|(name, d)| {
                if let KoDecision::SoftKo { reason } = d {
                    Some(format!("{}: {}", name, reason))
                } else {
                    None
                }
            })
            .collect();

        if soft_reasons.is_empty() {
            None
        } else {
            Some(soft_reasons.join("; "))
        }
    }
}

// ─────────────────────────────────────────────────────────────
// Part 3: MatchResult（スコアリング結果 + KO判定の統合）
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchResult {
    /// マッチングスコア（0.0〜1.0）
    pub score: f64,

    /// 詳細スコア内訳
    pub score_breakdown: ScoreBreakdown,

    /// 自動マッチ可能か（HardKo/SoftKo があれば false）
    pub auto_match_eligible: bool,

    /// 手動レビューが必要か（SoftKo があれば true）
    pub manual_review_required: bool,

    /// KO理由リスト（HardKo/SoftKo の reason を集約）
    pub ko_reasons: Vec<String>,

    /// 詳細な判定結果（チェック名, 判定）
    pub ko_decisions: Vec<(&'static str, KoDecision)>,
}

impl MatchResult {
    /// KO判定結果からMatchResultを構築
    ///
    /// 【重要な挙動】
    /// - HardKo が1つでもあれば: auto_match_eligible = false, スコアは0.0に上書き
    /// - SoftKo が1つでもあれば: auto_match_eligible = false, manual_review_required = true
    /// - 全て Pass: auto_match_eligible = true
    pub fn from_ko_checks(
        ko_decisions: Vec<(&'static str, KoDecision)>,
        score: f64,
        score_breakdown: ScoreBreakdown,
    ) -> Self {
        let has_hard_ko = ko_decisions.iter().any(|(_, d)| d.is_hard_ko());
        let has_soft_ko = ko_decisions.iter().any(|(_, d)| d.needs_manual_review());

        let ko_reasons: Vec<String> = ko_decisions
            .iter()
            .filter_map(|(name, d)| d.reason().map(|r| format!("[{}] {}", name, r)))
            .collect();

        // HardKo があればスコアは意味がないので 0.0
        let final_score = if has_hard_ko { 0.0 } else { score };

        Self {
            score: final_score,
            score_breakdown,
            auto_match_eligible: !has_hard_ko && !has_soft_ko,
            manual_review_required: has_soft_ko,
            ko_reasons,
            ko_decisions,
        }
    }
}

// ─────────────────────────────────────────────────────────────
// Part 4: 後方互換wrapper（移行期間のみ、新規コードでは使用禁止）
// ─────────────────────────────────────────────────────────────

#[deprecated(note = "KoDecision を直接使用すること。このwrapperは移行完了後に削除予定")]
pub fn check_knockout_bool(decision: &KoDecision) -> bool {
    matches!(decision, KoDecision::HardKo { .. })
}
```

**フロー図**:
```
┌─────────────────────────────────────────────────────────────┐
│                    KO判定フェーズ                            │
│  run_all_ko_checks(talent, project) -> KnockoutResultV2     │
└────────────────────────────┬────────────────────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
   ┌─────▼─────┐       ┌─────▼─────┐       ┌─────▼─────┐
   │  HardKo   │       │  SoftKo   │       │   Pass    │
   │ (1つ以上) │       │ (1つ以上) │       │  (全て)   │
   └─────┬─────┘       └─────┬─────┘       └─────┬─────┘
         │                   │                   │
         ▼                   ▼                   ▼
    即時除外            スコア計算           スコア計算
   score = 0.0      manual_review=true   auto_match_eligible
   auto=false          auto=false            auto=true
```

**Done条件**:
- [ ] KO判定の本体ロジックが `KoDecision` + `MatchResult` でしか書かれていない
- [ ] 旧 `check_knockout()` bool版のロジックが残っていない（deprecated wrapperのみ可）
- [ ] `auto_match_eligible` / `manual_review_required` フラグが仕様に明記・実装に存在
- [ ] `required_skills` が空/None のケースは必ず `manual_review` に行くテストがある
- [ ] "manualなのに送ってしまう" 経路がない

---

### CD-3: (欠番 - CD-2 SC-KO-UNIFIED に統合)

> **Note**: 当初 CD-3 は「auto_match_eligible フラグの導入」として独立セクションを予定していたが、
> KO判定体系全体の統一（CD-2: SC-KO-UNIFIED）に包含されたため欠番とする。
> `auto_match_eligible` および `manual_review_required` フラグの仕様は CD-2 を参照のこと。

---

### CD-4: 商流のdepth正規化（H-4解決）

**問題**: `flow_dept` は「エンド直」「1次請け」、`jinzai_flow_limit` は「SPONTO直人材」「1社先」と異なる体系。

**正（Canonical）**: 商流は **depth (u8)** に正規化して比較する。文字列ENUMは入出力ラベルのみ。

```rust {#normative}
// [Normative] 商流depth変換ロジックは正規であり、実装は必ず従うこと
// sr-common/src/corrections/flow_depth.rs
//
// 【仕様契約 SC-FLOW: 商流のdepth正規化】

/// 商流depth（0=エンド直, 1=1次, 2=2次, ...）
pub type FlowDepth = u8;

/// 案件側: flow_dept 文字列 → depth
pub fn parse_project_flow_depth(flow_dept: &str) -> Option<FlowDepth> {
    match flow_dept.trim() {
        "エンド直" => Some(0),
        "1次請け" => Some(1),
        "2次請け" => Some(2),
        "3次請け" => Some(3),
        "4次請け以上" => Some(4),
        _ => None,
    }
}

/// 人材側: 商流位置 → depth（1社先=1次の位置）
pub fn parse_talent_flow_depth(flow_depth: &str) -> Option<FlowDepth> {
    match flow_depth.trim() {
        "SPONTO直人材" => Some(0),
        "1社先" => Some(1),
        "2社先" => Some(2),
        "3社先以上" => Some(3),
        _ => None,
    }
}

/// 案件側: 人材商流制限 → 許容最大depth
/// ⚠️ DDL ses.jinzai_flow_limit_enum は3値: SPONTO直人材, SPONTO一社先まで, 商流制限なし
/// #4修正: 2025-12-18 存在しない「SPONTO二社先まで」を削除
pub fn parse_flow_limit(jinzai_flow_limit: &str) -> Option<FlowDepth> {
    match jinzai_flow_limit.trim() {
        "SPONTO直人材" => Some(0),         // 直人材のみ
        "SPONTO一社先まで" => Some(1),      // 1次まで
        "商流制限なし" => Some(u8::MAX),    // 制限なし
        _ => None,
    }
}

/// 商流KO判定（depth ベース）
pub fn check_flow_ko(
    talent_depth: Option<FlowDepth>,
    project_limit: Option<FlowDepth>,
) -> KoDecision {
    match (talent_depth, project_limit) {
        (Some(t), Some(p)) if t > p => KoDecision::HardKo {
            reason: format!("flow_exceeded: 人材depth={} > 制限depth={}", t, p),
        },
        (Some(_), Some(_)) => KoDecision::Pass,
        _ => KoDecision::SoftKo {
            reason: "flow_unknown: 商流情報不足".into(),
        },
    }
}
```

**Done条件**:
- [ ] どの文言でも最終的に depth に正規化される
- [ ] KO/スコアは depth で計算（文字列比較しない）

---

### CD-5: Prefilter/Detailed スコア重みの明文化（H-5解決）

**問題**: prefilter と詳細スコアで重みが異なるが、目的が不明確。

**正（Canonical）**: 目的が異なるので重みが異なるのは **正しい**。ただし明文化必須。

```rust
// sr-common/src/matching/weights.rs
//
// 【仕様契約 SC-WEIGHTS: スコア重みの目的別定義】

/// Prefilter重み（粗選別用）
/// 目的: False Negative を減らす（取りこぼさない）
/// → スキル重視（合わない人を残すより、合う人を逃さない）
pub const PREFILTER_WEIGHTS: Weights = Weights {
    tanka: 0.25,
    location: 0.15,
    skills: 0.45,       // 詳細より +0.05（スキル重視）
    experience: 0.10,   // 詳細より -0.05
    contract: 0.05,
};

/// Detailed重み（ランキング用）
/// 目的: False Positive を減らす（精度を上げる）
/// → 経験年数も重視（よりバランスの取れた評価）
pub const DETAILED_WEIGHTS: Weights = Weights {
    tanka: 0.25,
    location: 0.15,
    skills: 0.40,
    experience: 0.15,
    contract: 0.05,
};

#[derive(Debug, Clone, Copy)]
pub struct Weights {
    pub tanka: f64,
    pub location: f64,
    pub skills: f64,
    pub experience: f64,
    pub contract: f64,
}

impl Weights {
    pub fn sum(&self) -> f64 {
        self.tanka + self.location + self.skills + self.experience + self.contract
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weights_sum_to_one() {
        assert!((PREFILTER_WEIGHTS.sum() - 1.0).abs() < 1e-10);
        assert!((DETAILED_WEIGHTS.sum() - 1.0).abs() < 1e-10);
    }
}
```

**Done条件**:
- [ ] `PREFILTER_WEIGHTS` と `DETAILED_WEIGHTS` がコード上で明示
- [ ] 合計が1.0であるテスト
- [ ] 「prefilterは○○目的」説明が仕様書にある ← 上記コメントで対応

---

### CD-6: normalize_station の空文字禁止（H-6解決）

**問題**: `normalize_station("")` が `""` を返す。DBに空駅名が混入するリスク。

**正（Canonical）**: trim後に空なら `None` を返す。

```rust {#normative}
// [Normative] 駅名正規化ロジックは正規であり、実装は必ず従うこと
// sr-common/src/corrections/station.rs
//
// 【仕様契約 SC-STATION: 駅名正規化】
//
// ⚠️ 空文字は None。"" を返さない。

/// 駅名の正規化
/// - trim後に空 → None
/// - 「駅」で終わる → そのまま
/// - 「駅」で終わらない → 「駅」を付与
pub fn normalize_station(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;  // ⚠️ 空文字は None
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
    fn test_normalize_station_empty() {
        assert_eq!(normalize_station(""), None);
        assert_eq!(normalize_station("   "), None);
        assert_eq!(normalize_station("\t\n"), None);
    }

    #[test]
    fn test_normalize_station_normal() {
        assert_eq!(normalize_station("新宿"), Some("新宿駅".into()));
        assert_eq!(normalize_station("渋谷駅"), Some("渋谷駅".into()));
        assert_eq!(normalize_station("  池袋  "), Some("池袋駅".into()));
    }
}
```

**Done条件**:
- [ ] `""`, `"   "` は `None` になるテストがある
- [ ] DBに空駅が入らない経路が保証される

---

### CD-7: remote_onsite 補正の一本化（H-7解決 + Critical #4解決）

**問題**:
1. 同じ関数が複数箇所で異なる詳細度で定義されている
2. `normalize_remote_onsite()` と `correct_remote_onsite()` の使い分けが不明確
3. デフォルト値の適用タイミング（抽出時 vs 後段）が混在

**正（Canonical）**: 2つの関数を明確に分離する。

```rust {#normative}
// [Normative] リモート形態補正の二段階ロジックは正規であり、実装は必ず従うこと
// sr-common/src/corrections/remote_onsite.rs
//
// 【仕様契約 SC-REMOTE: リモート形態補正の二段階設計】
//
// 【第1段階: normalize_remote_onsite()】
//   - 抽出フェーズで使用（LLM応答のパース時）
//   - 入力: 生のテキスト（&str）
//   - 出力: 必ず有効なENUM値（String）
//   - デフォルト: "リモート併用"（不明な場合も含む）
//
// 【第2段階: correct_remote_onsite()】
//   - ENUM補正フェーズで使用（DB格納前）
//   - 入力: Option<String>（抽出済みの値）
//   - 出力: Option<String>（補正不可なら None）
//   - None時: requires_review = true, DB格納は NULL

pub const REMOTE_ONSITE_ENUMS: &[&str] = &["フル出社", "リモート併用", "フルリモート"];

/// 【第1段階】リモート形態の正規化
///
/// ⚠️ 常に有効な値を返す（None を返さない）
/// 使用箇所: LLM応答パース時
pub fn normalize_remote_onsite(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "リモート併用".to_string(); // デフォルト
    }

    // 完全一致チェック
    if REMOTE_ONSITE_ENUMS.contains(&trimmed) {
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();

    // フルリモート判定
    if lower.contains("フルリモート") || lower.contains("完全リモート")
        || lower.contains("フルリモ") || lower.contains("full remote")
    {
        return "フルリモート".to_string();
    }

    // フル出社判定
    if lower.contains("フル出社") || lower.contains("出社のみ")
        || lower.contains("常駐") || lower.contains("客先")
        || lower.contains("出社必須")
    {
        return "フル出社".to_string();
    }

    // リモート併用（その他すべて）
    "リモート併用".to_string()
}

/// 【第2段階】ENUM補正（DB格納前）
///
/// ⚠️ 補正不可なら None を返す（requires_review=true）
/// 使用箇所: apply_enum_corrections() 内
pub fn correct_remote_onsite(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None; // 空文字はレビュー必要
    }

    // 完全一致チェック
    if REMOTE_ONSITE_ENUMS.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    // パターンマッチで補正を試みる
    let lower = trimmed.to_lowercase();

    // ⚠️ sponto-platform enum_corrections.js と同期すること
    if lower.contains("フルリモート") || lower.contains("完全リモート")
        || lower.contains("フルリモ") {
        return Some("フルリモート".to_string());
    }
    if lower.contains("フル出社") || lower.contains("出社") || lower.contains("常駐")
        || lower.contains("客先") {
        return Some("フル出社".to_string());
    }
    if lower.contains("リモート") || lower.contains("併用") || lower.contains("ハイブリッド")
        || lower.contains("一部") {
        return Some("リモート併用".to_string());
    }

    // ⚠️ 不明なパターンは None（requires_review=true で人間が判断）
    None
}
```

**使い分けフロー**:
```
                    ┌─────────────────────────────────┐
                    │     LLM応答（生テキスト）        │
                    └────────────────┬────────────────┘
                                     │
                                     ▼
                    ┌─────────────────────────────────┐
                    │   normalize_remote_onsite()     │
                    │   入力: &str                    │
                    │   出力: String (必ず値あり)     │
                    │   デフォルト: "リモート併用"     │
                    └────────────────┬────────────────┘
                                     │
                    ┌────────────────▼────────────────┐
                    │   LlmExtractedProject           │
                    │   remote_onsite: Some(String)   │
                    └────────────────┬────────────────┘
                                     │
                                     ▼
                    ┌─────────────────────────────────┐
                    │   correct_remote_onsite()       │
                    │   入力: &str                    │
                    │   出力: Option<String>          │
                    │   None時: requires_review=true  │
                    └────────────────┬────────────────┘
                                     │
              ┌──────────────────────┴──────────────────────┐
              │                                             │
        ┌─────▼─────┐                                 ┌─────▼─────┐
        │ Some(val) │                                 │   None    │
        │ DB格納OK  │                                 │ レビュー  │
        └───────────┘                                 └───────────┘
```

**prefilter/scoring での None 扱い**:
```rust
/// prefilter で remote_onsite が None の場合
fn score_remote_compat(
    project_remote: Option<&str>,
    talent_remote: Option<&str>,
) -> f64 {
    match (project_remote, talent_remote) {
        (Some(p), Some(t)) => calculate_remote_compatibility(p, t),
        // ⚠️ どちらかが None → 中立スコア 0.5（判定不可）
        _ => 0.5,
    }
}
```

**Done条件**:
- [ ] `normalize_remote_onsite()` は常に String を返す
- [ ] `correct_remote_onsite()` は補正不可時に None を返す
- [ ] prefilter/scoring は None を 0.5（中立）として扱う
- [ ] 仕様書内の他のコードブロックは「Example（非規範）」と明示 or 削除

---

### CD-8: スキル正規化の一元化（Critical #2解決）

**問題**: `normalize_skill()` が人材側で使用されているが、案件側の `required_skills_keywords` も同じロジックで正規化するかが明記されていない。スキル比較時に正規化が片側のみだと false negative（合う人を落とす）が発生する。

**正（Canonical）**: **スキル比較の両側（案件・人材）で同一の正規化関数を使用する**。

```rust {#normative}
// [Normative] このスキル正規化ロジックは正規であり、実装は必ず従うこと
// sr-common/src/skill_normalizer.rs
//
// 【仕様契約 SC-SKILL-NORM: スキル正規化の一元化】
//
// スキル比較を行う全ての箇所で、入力を正規化してから比較する:
//   1. 案件抽出時: required_skills_keywords に格納前に正規化
//   2. 人材抽出時: possessed_skills_keywords に格納前に正規化
//   3. マッチング時: 念のため両側を再正規化してから比較
//
// ⚠️ これにより「React.js」と「react」、「k8s」と「kubernetes」が同一視される

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// スキルエイリアス辞書（小文字正規形 → 正規スキル名）
static ALIAS_TO_CANONICAL: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // JavaScript
    m.insert("js", "javascript");
    m.insert("ecmascript", "javascript");
    m.insert("es6", "javascript");
    m.insert("es2015", "javascript");
    // TypeScript
    m.insert("ts", "typescript");
    // React
    m.insert("react.js", "react");
    m.insert("reactjs", "react");
    // Kubernetes
    m.insert("k8s", "kubernetes");
    m.insert("kube", "kubernetes");
    // C#
    m.insert("c#", "csharp");
    m.insert("c-sharp", "csharp");
    // ... 完全なリストは「スキルエイリアス辞書」セクション参照
    m
});

/// スキル文字列を正規形に変換
///
/// 【仕様契約】
/// - 空白のtrim
/// - 小文字化
/// - エイリアス適用（辞書にあれば変換、なければそのまま）
pub fn normalize_skill(skill: &str) -> String {
    let s = skill.trim().to_lowercase();
    ALIAS_TO_CANONICAL
        .get(s.as_str())
        .map(|c| c.to_string())
        .unwrap_or(s)
}

/// スキル配列を正規化した HashSet に変換（マッチング用）
pub fn normalize_skill_set(skills: &[String]) -> std::collections::HashSet<String> {
    skills.iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_skill(s))
        .collect()
}

/// スキル配列を正規化した Vec に変換（DB格納用）
pub fn normalize_skills_vec(skills: &[String]) -> Vec<String> {
    let mut result: Vec<String> = skills
        .iter()
        .map(|s| normalize_skill(s))
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .collect();
    result.sort();
    result.dedup();
    result
}
```

**スキル正規化の適用箇所**:

| 箇所 | 関数 | 正規化タイミング |
|------|------|-----------------|
| **案件抽出** | `skills_to_keywords()` | LLM応答 → DB格納時 |
| **人材抽出** | `skills_to_keywords()` | スキルスロット → DB格納時 |
| **KO判定** | `check_skill_ko()` | 内部で `normalize_skill_set()` 使用 |
| **スコアリング** | `calculate_skill_match()` | 内部で `normalize_skill_set()` 使用 |
| **prefilter** | `score_skill_compat()` | 内部で `normalize_skill_set()` 使用 |

**テストケース**:
```rust
#[test]
fn test_skill_normalization_bidirectional() {
    // 案件: "React.js", "K8s" → 正規化後: ["react", "kubernetes"]
    let project_skills = vec!["React.js".to_string(), "K8s".to_string()];
    let project_normalized = normalize_skill_set(&project_skills);

    // 人材: "react", "kubernetes" → 正規化後: ["react", "kubernetes"]
    let talent_skills = vec!["react".to_string(), "kubernetes".to_string()];
    let talent_normalized = normalize_skill_set(&talent_skills);

    // 正規化後は完全一致
    assert_eq!(project_normalized, talent_normalized);
}

#[test]
fn test_case_alias_equivalence() {
    // 同じスキルの異なる表記が全て同一視される
    assert_eq!(normalize_skill("JavaScript"), "javascript");
    assert_eq!(normalize_skill("js"), "javascript");
    assert_eq!(normalize_skill("JS"), "javascript");
    assert_eq!(normalize_skill("ecmascript"), "javascript");
}
```

**Done条件**:
- [ ] 案件抽出・人材抽出の両方で `normalize_skill()` を通過してからDB格納
- [ ] マッチング関数（KO/スコアリング/prefilter）は全て `normalize_skill_set()` を使用
- [ ] テストで「異なる表記の同一スキル」が一致することを検証

---

### CD-9: 単価計算パラメータの統一（Critical #3解決）

**問題**: 単価計算のパラメータ（base値、フロア、プレミアム上限）が複数箇所に分散しており、どの値が正しいか不明確。

**正（Canonical）**: 以下のパラメータ定数を唯一の真とする。

```rust {#normative}
// [Normative] この定数群は正規であり、ハードコードは禁止
// sr-common/src/calculation/tanka_params.rs
//
// 【仕様契約 SC-TANKA: 単価計算パラメータ統一】

/// 単価計算の共通パラメータ
pub mod TankaParams {
    /// ベース単価（万円）- 人材・案件共通
    pub const BASE_TANKA: f64 = 35.0;

    /// 経験年数別加算（万円/年）
    pub const EXP_RATE_1_TO_5: f64 = 5.0;   // 1-5年: +5万/年
    pub const EXP_RATE_6_TO_10: f64 = 4.0;  // 6-10年: +4万/年
    pub const EXP_RATE_11_PLUS: f64 = 3.0;  // 11年+: +3万/年
    pub const EXP_YEARS_CAP: i32 = 20;       // 経験年数上限（20年でキャップ）

    /// スキルプレミアム（乗数）
    pub const PREMIUM_CLOUD: f64 = 0.15;     // AWS/GCP/Azure: +15%
    pub const PREMIUM_PM_PMO: f64 = 0.15;    // PM/PMO: +15%
    pub const PREMIUM_AI_ML: f64 = 0.10;     // AI/ML/機械学習: +10%
    pub const PREMIUM_MODERN_DEV: f64 = 0.10; // Python/Go/TypeScript: +10%
    pub const PREMIUM_CAP: f64 = 1.5;        // プレミアム上限: 最大1.5x

    /// 単価レンジ幅（万円）
    pub const RANGE_WIDTH: f64 = 5.0;        // 最終単価 ±5万円

    /// 案件最低単価（万円）- 案件のみに適用
    pub const PROJECT_MIN_TANKA: i32 = 50;
    pub const PROJECT_MIN_TANKA_MAX: i32 = 55; // max側も同様にフロア

    // ⚠️ 人材には最低単価フロアを設けない（市場価値通りに計算）
}
```

**人材 vs 案件 の違い**:

| パラメータ | 人材単価 | 案件単価 | 理由 |
|-----------|---------|---------|------|
| ベース単価 | 35万円 | 35万円 | **共通** |
| 経験年数加算 | 同上 | 同上 | **共通** |
| プレミアム上限 | 1.5x | 1.5x | **共通** |
| 最低単価フロア | **なし** | **50万円** | 案件は発注元の期待値があるため |
| レンジ幅 | ±5万円 | ±5万円 | **共通** |

**更新後の計算関数**:
```rust
pub fn calculate_talent_tanka(...) -> (i32, i32, String) {
    // ... 計算ロジック ...
    let min_tanka = (final_tanka - TankaParams::RANGE_WIDTH).floor() as i32;
    let max_tanka = (final_tanka + TankaParams::RANGE_WIDTH).ceil() as i32;
    // ⚠️ フロアなし
    (min_tanka, max_tanka, logic.join("; "))
}

pub fn calculate_project_tanka(...) -> (i32, i32, String) {
    // ... 計算ロジック ...
    let min_tanka = ((final_tanka - TankaParams::RANGE_WIDTH).floor() as i32)
        .max(TankaParams::PROJECT_MIN_TANKA);
    let max_tanka = ((final_tanka + TankaParams::RANGE_WIDTH).ceil() as i32)
        .max(TankaParams::PROJECT_MIN_TANKA_MAX);
    // ⚠️ 案件は50万円フロア適用
    (min_tanka, max_tanka, logic.join("; "))
}
```

**Done条件**:
- [ ] 全ての単価計算で `TankaParams::*` 定数を使用（ハードコード禁止）
- [ ] 人材と案件のフロア有無の違いがテストで検証されている
- [ ] プレミアム上限1.5xがテストで検証されている

---

### CD-10: received_at の明確な定義（Critical #5解決）

**問題**:
1. `normalize_start_date(raw_text, received_at)` は received_at に依存
2. 「誰がいつ received_at を渡すのか」が不明確
3. 相対日付（「来月」「月末」）の計算が received_at 時点で行われるが、メール受信日と抽出実行日がズレた場合の挙動が未定義

**正（Canonical）**: received_at の source と使用ルールを明確化する。

```rust {#normative}
// [Normative] received_at の解決ロジックは正規であり、実装は必ず従うこと
// sr-common/src/date/received_at.rs
//
// 【仕様契約 SC-RECEIVED-AT: 相対日付の基準日定義】
//
// received_at は「メールが受信された日時」を指し、相対日付の解釈に使用する。
// これにより「来月」「即日」などの表現を一貫して解釈できる。
//
// ⚠️ 抽出実行日（processing_date）ではなく、メール受信日を使用する理由:
//   - 送信者の意図は「メール送信時点」での日付解釈
//   - 抽出が遅延しても意味が変わらない
//
// 【source の優先順位】
//   1. extraction_queue.email_received_at（メールヘッダから取得）
//   2. talents_enum.received_at / projects_enum.email_received_at
//   3. ⚠️ None の場合はエラー（抽出を中断）

use chrono::{NaiveDate, NaiveDateTime};

/// received_at を解決する
///
/// extraction_queue から取得した email_received_at を使用
/// None の場合は抽出を中断（相対日付の解釈が不可能）
pub fn resolve_received_at(
    email_received_at: Option<NaiveDateTime>,
) -> Result<NaiveDate, ReceivedAtError> {
    email_received_at
        .map(|dt| dt.date())
        .ok_or(ReceivedAtError::MissingReceivedAt)
}

#[derive(Debug, thiserror::Error)]
pub enum ReceivedAtError {
    #[error("email_received_at is missing - cannot interpret relative dates")]
    MissingReceivedAt,
}
```

**received_at の使用箇所と source**:

| 使用箇所 | 関数 | received_at の source |
|---------|------|----------------------|
| **案件抽出** | `normalize_start_date()` | `extraction_queue.email_received_at` |
| **人材抽出** | `normalize_start_date()` | `extraction_queue.email_received_at` |
| **KO判定** | `run_all_ko_checks()` | `extraction_queue.email_received_at` or `talents_enum.received_at` |
| **prefilter** | `has_availability_conflict()` | `talents_enum.received_at` |

**相対日付の解釈ルール**:
```rust
/// 相対日付の解釈（received_at 基準）
///
/// | 入力 | 解釈ロジック | 例（received_at=2025-01-15） |
/// |------|-------------|------------------------------|
/// | "即日" | received_at そのまま | 2025-01-15 |
/// | "来月" | 翌月1日 | 2025-02-01 |
/// | "再来月" | 翌々月1日 | 2025-03-01 |
/// | "年明け" | 翌年1月6日 | 2026-01-06 |
/// | "1月中旬" | 当年 or 翌年の1月15日 | 2025-01-15 or 2026-01-15 |
/// | "25日" | received_at の月の25日 | 2025-01-25 |
///
/// ⚠️ 「1月中旬」の年推定: received_at の月より前なら翌年
pub struct DateNormalizationResult {
    pub date: Option<NaiveDate>,
    pub precision: DatePrecision,
    pub interpretation_note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatePrecision {
    Exact,       // "2025-01-15", "1月15日"
    Month,       // "1月中旬", "来月"
    Quarter,     // "Q1", "年明け"
    Negotiable,  // "応相談"
}

pub fn normalize_start_date(
    raw: &str,
    received_at: NaiveDate,
) -> DateNormalizationResult {
    let trimmed = raw.trim();

    // 即日
    if trimmed == "即日" || trimmed.contains("すぐ") {
        return DateNormalizationResult {
            date: Some(received_at),
            precision: DatePrecision::Exact,
            interpretation_note: Some(format!("即日 → {}", received_at)),
        };
    }

    // 来月・再来月
    if trimmed.contains("来月") {
        let (y, m) = if received_at.month() == 12 {
            (received_at.year() + 1, 1)
        } else {
            (received_at.year(), received_at.month() + 1)
        };
        return DateNormalizationResult {
            date: NaiveDate::from_ymd_opt(y, m, 1),
            precision: DatePrecision::Month,
            interpretation_note: Some(format!("来月 → {}-{:02}-01", y, m)),
        };
    }

    // ... 他のパターン
    // 詳細は「相対日付正規化」セクション参照

    DateNormalizationResult {
        date: None,
        precision: DatePrecision::Negotiable,
        interpretation_note: None,
    }
}
```

**テストケース**:
```rust
#[test]
fn test_relative_date_interpretation() {
    let received_at = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

    // 即日
    let result = normalize_start_date("即日", received_at);
    assert_eq!(result.date, Some(received_at));
    assert_eq!(result.precision, DatePrecision::Exact);

    // 来月
    let result = normalize_start_date("来月", received_at);
    assert_eq!(result.date, NaiveDate::from_ymd_opt(2025, 2, 1));
    assert_eq!(result.precision, DatePrecision::Month);

    // 年跨ぎ（12月に「来月」）
    let dec_received = NaiveDate::from_ymd_opt(2024, 12, 20).unwrap();
    let result = normalize_start_date("来月", dec_received);
    assert_eq!(result.date, NaiveDate::from_ymd_opt(2025, 1, 1));
}

#[test]
fn test_month_year_estimation() {
    // 1月15日受信で「3月中旬」→ 同年
    let jan = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
    let result = normalize_start_date("3月中旬", jan);
    assert_eq!(result.date.unwrap().year(), 2025);

    // 1月15日受信で「12月中旬」→ 同年（まだ先）
    let result = normalize_start_date("12月中旬", jan);
    assert_eq!(result.date.unwrap().year(), 2025);
}
```

**Done条件**:
- [ ] `received_at` の source が `extraction_queue.email_received_at` と明記
- [ ] `received_at` が None の場合のエラーハンドリングが実装されている
- [ ] 相対日付の解釈ルールがテストで検証されている
- [ ] 年跨ぎケース（12月に「来月」）がテストで検証されている

---

### 設計原則（二度と増やさないためのルール）

| ルール | 説明 |
|--------|------|
| **A. ドメイン解釈は1関数** | 勤務地・remote_onsite・日付・都道府県・商流の「解釈関数」は各1つだけ。KO/スコア/prefilterは全てそれを呼ぶ |
| **B. 判定フローは2層分離** | **Eligibility層**（HardKo/SoftKo/Pass）と **Scoring層**（順位付け）を明確に分ける |
| **C. 仕様書は規範、コードは唯一** | 仕様書のコードブロックには「Normative（正）」or「Example（非規範）」を明示。Normativeは1箇所のみ |

---

## 📂 移植元コード参照

**リポジトリ**: `sponto-platform`

### 1. ENUM補正ロジック（移植必須）

**⚠️ 注意**: 本MVP では **BP案件メール** をパースする。

```
sponto-platform/scripts/project_data_extract_from_ai_response.js (MVP本体: 案件用)
sponto-platform/scripts/bptalent_data_extract_from_ai_response.js (参照用: 人材用・Phase 2)
sponto-platform/scripts/shared/enum_corrections.js (共通ENUM定義)
```

**MVP（BP案件パース）で使用するENUM補正**:
- `correctFlowDept()` → 商流ENUM補正（案件の flow_dept）
- `correctRemoteOnsite()` → リモート形態ENUM補正（案件の remote_onsite）
- `correctTodofuken()` → 都道府県ENUM補正（案件の work_todofuken）
- `correctWorkArea()` → エリアENUM補正（案件の work_area）
- `correctContractType()` → 契約形態ENUM補正（案件の contract_type）
- `correctJinzaiFlowLimit()` → 人材商流制限ENUM補正（案件の jinzai_flow_limit）
- `correctTechKubun()` → 技術カテゴリENUM補正（案件の tech_kubun）
- `correctJapaneseSkill()` → 日本語スキルENUM補正（案件の japanese_skill）
- `correctEnglishSkill()` → 英語スキルENUM補正（案件の english_skill）

**Phase 2（BP人材パース）で追加使用**:
- `correctTalentContractType()` → 人材契約形態ENUM補正（primary/secondary）
- `correctTalentFlowDepth()` → 人材商流ENUM補正（1社先, 2社先, 3社先以上）
- `correctDesiredRemoteOnsite()` → 人材希望リモート形態ENUM補正
- `correctGender()` → 性別ENUM補正
- `correctResidentialArea()` → 居住エリアENUM補正
- `correctResidentialTodofuken()` → 居住都道府県ENUM補正

**共通ユーティリティ**:
- `safeString()`, `safeNumber()`, `safeArray()`, `safeBoolean()`, `safeDate()` → バリデーション関数

**Rust実装先**: `sr-common/src/corrections/` 配下に各ENUM補正関数を実装

### 2. KO条件ロジック（移植必須）
```
sponto-platform/apps/business-api/app/services/matching/strategy.py
```
- `_check_tanka()` → 単価KO（利益 < 5万円）
- `_check_location()` → 勤務地KO
- `_check_required_skills()` → 必須スキルKO（マッチ率 < 30%）
- `_check_ng_keywords()` → NGキーワードKO
- `_check_flow_limit()` → 商流制限KO
- `_check_nationality_and_japanese()` → 国籍・日本語KO
- `_check_age_limit()` → 年齢制限KO
- `_check_contract_type()` → 契約形態KO
- `_check_availability_date()` → 稼働時期KO
- `_match_remote_style()` → リモート形態KO（10点配分のスコア付きKO）

### 3. LLMプロンプト（参考）
```
sponto-platform/prompt/01_prompt_project_data_extract_from_emails.md (MVP: 案件抽出用)
sponto-platform/prompt/02_prompt_talent_data_extract_from_emails.md (Phase 2: 人材抽出用)
```
- Output Schema（データ構造の参考）
- 各項目の生成ルール（フィールド定義の参考）

### 4. マッチング設定（参考）
```
sponto-platform/apps/business-api/app/core/matching_config.py
```
- 閾値設定（skill_match_minimum: 0.3, tanka_profit_minimum: 5万円）
- スコアリング重み（business_rules: 0.6, semantic: 0.3, historical: 0.1）
- 機能フラグ（enable_semantic_matching, enable_alias_matching 等）

**⚠️ 閾値の検証状況（Loc指摘 2025-12-15）**:
> 「Pythonの閾値は感覚値で、数値として正しいと言い切れない。テストできてない」

| 閾値 | 現在値 | 根拠 | 検証状況 |
|------|-------|------|---------|
| `tanka_profit_minimum` | 5万円 | 感覚値 | ❌ 未検証 |
| `skill_match_minimum` | 30% | 感覚値 | ❌ 未検証 |
| `experience_buffer_years` | 0.5年 | 感覚値 | ❌ 未検証 |

**検証計画**: Phase 2（Week 4-7）で実データを用いて最適値を探索。環境変数（`SR_SKILL_MATCH_THRESHOLD` 等）で動的調整可能にしておく。

### 5. 詳細スコアリング（移植必須）
```
sponto-platform/apps/business-api/app/services/matching/business_rules_engine.py
```
- `calculate_match_score()` → 総合スコア計算（加重合成）
- `_score_tanka_detailed()` → 単価詳細スコア（0.0〜1.0）
- `_score_location_detailed()` → 勤務地詳細スコア
- `_score_skills_detailed()` → スキル詳細スコア（必須75%+歓迎25%）
- `_score_experience_detailed()` → 経験年数詳細スコア（バッファ考慮）
- スコア合成: `tanka×0.25 + location×0.15 + skills×0.40 + experience×0.15 + contract×0.05`

### 6. 事前フィルタリング（移植必須）
```
sponto-platform/apps/business-api/app/services/matching/enhanced_prefilter.py
```
- `find_project_candidates_for_talent()` → 候補案件抽出
- `find_talent_candidates_for_project()` → 候補人材抽出
- `_calculate_preliminary_score()` → 予備スコア計算
- `_has_knockout_issues()` → 簡易KO判定（NGキーワード・商流・稼働日）

### 7. スキルエイリアス（移植必須）⭐重要
```
sponto-platform/apps/business-api/app/core/skill_processor.py
```
- `normalize_skill_with_aliases()` → スキル正規化（エイリアス適用）
- **183個のスキルエイリアス定義**（例）:
  - JavaScript: `js`, `javascript`, `ecmascript`, `es6`
  - TypeScript: `ts`, `typescript`
  - React: `reactjs`, `react.js`, `react16`, `react17`, `react18`
  - AWS: `amazon web services`, `amazon aws`, `aws cloud`
  - Python: `python3`, `py`, `python2.7`
  - Kubernetes: `k8s`, `kube`
  - AI/ML: `artificial intelligence`, `machine learning`, `ml`, `人工知能`, `機械学習`

### 8. ENUM定義（共有版・移植必須）
```
sponto-platform/scripts/shared/enum_corrections.js
```
| カテゴリ | 有効値 |
|---------|--------|
| WORK_AREA | `北海道・東北`, `関東`, `甲信越・北陸`, `東海`, `近畿`, `中国・四国`, `九州・沖縄` |
| TODOFUKEN | `北海道`, `青森県`, `岩手県`, `宮城県`, `秋田県`, `山形県`, `福島県`, `茨城県`, `栃木県`, `群馬県`, `埼玉県`, `千葉県`, `東京都`, `神奈川県`, `新潟県`, `富山県`, `石川県`, `福井県`, `山梨県`, `長野県`, `岐阜県`, `静岡県`, `愛知県`, `三重県`, `滋賀県`, `京都府`, `大阪府`, `兵庫県`, `奈良県`, `和歌山県`, `鳥取県`, `島根県`, `岡山県`, `広島県`, `山口県`, `徳島県`, `香川県`, `愛媛県`, `高知県`, `福岡県`, `佐賀県`, `長崎県`, `熊本県`, `大分県`, `宮崎県`, `鹿児島県`, `沖縄県` |
| REMOTE_ONSITE | `フル出社`, `リモート併用`, `フルリモート` |
| CONTRACT_TYPE | `準委任契約`, `派遣` |
| TALENT_CONTRACT_TYPE | `正社員`, `契約社員`, `直個人` |
| JAPANESE_SKILL | `不要`, `N5`, `N4`, `N3`, `N2`, `N1`, `ネイティブ` |
| ENGLISH_SKILL | `不要`, `読み書き`, `会話`, `ビジネス`, `上級ビジネス`, `ネイティブ` |
| FLOW_DEPT | `エンド直`, `1次請け`, `2次請け`, `3次請け`, `4次請け以上`, `不明` |
| TALENT_FLOW_DEPTH | `1社先`, `2社先`, `3社先以上` |
| JINZAI_FLOW_LIMIT | `SPONTO直人材`, `SPONTO一社先まで`, `商流制限なし` |
| TECH_KUBUN | `生成AI関連`, `人気技術`, `レガシー` |
| PROJECT_STATUS | `募集中`, `選考中`, `募集終了`, `成約`, `キャンセル` |
| TALENT_AVAILABILITY_STATUS | `営業中`, `選考中`, `稼働中`, `休止中` |
| TALENT_GENDER | `男性`, `女性`, `その他/無回答` |

**都道府県→エリアマッピング**:
```
北海道・東北: 北海道, 青森県, 岩手県, 宮城県, 秋田県, 山形県, 福島県
関東: 茨城県, 栃木県, 群馬県, 埼玉県, 千葉県, 東京都, 神奈川県
甲信越・北陸: 新潟県, 富山県, 石川県, 福井県, 山梨県, 長野県
東海: 岐阜県, 静岡県, 愛知県, 三重県
近畿: 滋賀県, 京都府, 大阪府, 兵庫県, 奈良県, 和歌山県
中国・四国: 鳥取県, 島根県, 岡山県, 広島県, 山口県, 徳島県, 香川県, 愛媛県, 高知県
九州・沖縄: 福岡県, 佐賀県, 長崎県, 熊本県, 大分県, 宮崎県, 鹿児島県, 沖縄県
```

### 9. 単価計算ロジック（移植推奨）
```
sponto-platform/apps/business-api/app/services/calculation/strategies/bptalent_tanka.py
sponto-platform/apps/business-api/app/services/calculation/strategies/project_tanka.py
```
**人材単価計算**:
```
base = 35万円
+ 経験年数加算（1-5年: +5万/年, 6-10年: +4万/年, 11年+: +3万/年）
× スキルプレミアム（AWS/GCP: +15%, AI/ML: +10%, PM/PMO: +15%）
= 最終単価（±5万円の範囲で提示）
```

### 10. スコア計算（参考）
```
sponto-platform/apps/business-api/app/services/calculation/strategies/bptalent_score.py
sponto-platform/apps/business-api/app/services/calculation/strategies/project_score.py
```
- 人材スコア: 市場価値 + 提案しやすさ + 資格 + ポテンシャル
- 案件スコア: 収益性 + 成約確度 + 案件の魅力

### 11. バリデーション（参考）
```
sponto-platform/scripts/shared/validation.js
```
- `validateTanka()` → 単価チェック（20〜300万円）
- `validateAge()` → 年齢チェック（18〜70歳）
- `validateExperienceYears()` → 経験年数チェック（0〜50年）
- `validateSkillKeywords()` → スキルキーワード検証（重複排除）

### 12. 配列安全処理（参考）
```
sponto-platform/apps/business-api/app/core/array_utils.py
```
- `safe_bool_check()` → 配列/スカラ値のブール値変換
- `safe_scalar_extract()` → スカラ値抽出（配列の場合は最初の要素）
- `is_valid_vector()` → ベクトル有効性チェック

### 13. AI抽出サービス（既存実装・参考）
```
sponto-platform/apps/business-api/app/services/ai_extraction_service.py
sponto-platform/apps/business-api/app/routes/extraction.py
```
**Two-Stage Extraction（本番稼働中）**:
- `AIExtractionService.extract_stage1_only()` → 初回抽出（信頼度スコア付き）
- `AIExtractionService.refine_stage2_only()` → 低信頼度フィールドの再抽出
- `AIExtractionService.extract_project_data()` → 自動Stage2トリガー付きハイブリッド抽出

#### 13a. Stage 2 発動条件（詳細）

**自動トリガー条件**:
| 条件 | 閾値 | 説明 |
|------|------|------|
| **総合信頼度** | `< 0.70` | Stage 1 の overall_confidence が 0.70 未満 |
| **Tier 1 フィールド欠損** | 2個以上 | required_skills_keywords, monthly_tanka_max, work_todofuken, start_date のうち2個以上が NULL |
| **単価未検出** | min/max 両方 NULL | 必須ではないが、Stage 2 で追加プロンプト |
| **スキル数不足** | `< 2` | required_skills_keywords が 0-1 個の場合 |

**Stage 2 で行われる処理**:
```
1. 低信頼度フィールドの特定（confidence < 0.70）
2. 元テキストの該当部分を再提示（コンテキスト強調）
3. フィールド単位の再抽出プロンプト
4. 結果のマージ（Stage 2 の方が信頼度高ければ上書き）
```

**Stage 2 をスキップするケース**:
- Stage 1 で overall_confidence ≥ 0.90（十分に高い）
- 元テキストが 100 文字未満（情報不足）
- 同一 message_id で既に Stage 2 実行済み

#### 13b. 品質グレード計算（詳細）

**グレード判定表**:
| overall_confidence | グレード | 扱い |
|-------------------|----------|------|
| ≥ 0.95 | A+ | 自動承認、監査なし |
| ≥ 0.90 | A | 自動承認 |
| ≥ 0.85 | B+ | 自動承認、サンプリング監査 |
| ≥ 0.80 | B | 自動承認 |
| ≥ 0.75 | C+ | Stage 2 推奨（自動発動しない） |
| ≥ 0.70 | C | Stage 2 境界値 |
| ≥ 0.50 | D | Stage 2 自動発動、手動レビューフラグ |
| < 0.50 | F | 手動レビュー必須、DB保存時に `requires_manual_review=true` |

**overall_confidence 計算式**:
```rust
// 加重平均方式
fn calculate_overall_confidence(field_confidences: &HashMap<String, f32>) -> f32 {
    let weights = [
        ("project_name", 0.15),
        ("monthly_tanka_min", 0.20),
        ("monthly_tanka_max", 0.20),
        ("required_skills_keywords", 0.15),
        ("work_area", 0.10),
        ("start_date", 0.10),
        ("remote_onsite", 0.05),
        ("joi_bp_name", 0.05),
    ];

    let mut total_weight = 0.0;
    let mut weighted_sum = 0.0;

    for (field, weight) in weights {
        if let Some(&conf) = field_confidences.get(field) {
            weighted_sum += conf * weight;
            total_weight += weight;
        }
    }

    if total_weight > 0.0 {
        weighted_sum / total_weight
    } else {
        0.0  // フィールドが一つもない場合
    }
}
```

**信頼度の加重計算**:
| フィールド | 重み | 補足 |
|-----------|------|------|
| project_name | 15% | 案件名（必須だが誤抽出しにくい） |
| monthly_tanka_min | 20% | 最低単価（ビジネス影響大） |
| monthly_tanka_max | 20% | 最高単価（ビジネス影響大） |
| required_skills_keywords | 15% | 必須スキル（マッチング核心） |
| work_area | 10% | 勤務エリア |
| start_date | 10% | 稼働開始日 |
| remote_onsite | 5% | リモート形態 |
| joi_bp_name | 5% | BP会社名（参考情報） |

### 14. 品質監視・エラー復旧（参考）
```
sponto-platform/scripts/extraction_quality_monitor.js
sponto-platform/scripts/extraction_error_recovery.js
```
**品質監視**:
- `ExtractionQualityMonitor.recordExtractionSession()` → 抽出セッション記録
- `ExtractionQualityMonitor.generateQualityReport()` → 24h/7d/30d品質レポート
- `ExtractionQualityMonitor.checkForAlerts()` → アラート閾値チェック

**アラート閾値**:
| 条件 | 閾値 |
|------|------|
| 信頼度低下 | 10%低下 |
| 失敗率 | 15%超過 |
| 処理時間 | 30秒超過 |
| バリデーション失敗 | 20%超過 |

**エラー復旧戦略（優先順）**:
1. JSON Parse Error → `clean_json_formatting`, `extract_partial_json`
2. LLM Timeout → `retry_with_shorter_prompt`, `split_extraction_stages`
3. Low Confidence → `enhanced_prompt_retry`, `field_by_field_extraction`
4. Validation Failure → `targeted_field_reextraction`, `business_rule_correction`

---

## 📝 具体的な入出力例（コーディングLLM向け）

**目的**: このセクションを読めば、各モジュールの期待する入出力が明確になる。

---

### 例1: メール本文 → BP案件情報抽出（MVP対象）

> **[Example]** このセクションは入出力の例示であり、実装の参考用です。

**入力（BP案件紹介メール）**:
```text
お世話になっております。
先日配信いただいたT.Y様向けに、下記の案件をご紹介いたします。

【案件概要】
■プロジェクト名：ECサイトリニューアル開発
■クライアント：大手小売業（東証プライム）

■必須スキル
・Java（3年以上）
・Spring Boot
・PostgreSQL
・AWS経験

■歓迎スキル
・Kubernetes
・CI/CD経験

■条件
・単価：70万〜80万（スキル見合い）
・稼働開始日：1月上旬〜
・期間：6ヶ月以上（延長可能性あり）
・勤務地：東京都港区
・リモート：リモート併用（週2出社）
・商流：2次請け

以上、ご検討よろしくお願いいたします。
```

**出力（ProjectExtraction struct）**:
```rust
ProjectExtraction {
    // Tier 1 (必須)
    required_skills_keywords: Some(vec![
        "java".to_string(),
        "spring boot".to_string(),
        "postgresql".to_string(),
        "aws".to_string(),
    ]),
    monthly_tanka_max: Some(80),
    start_date_raw: Some("1月上旬".to_string()),
    work_todofuken: Some("東京都".to_string()),  // ENUM補正後

    // Tier 2 (重要)
    monthly_tanka_min: Some(70),
    remote_onsite: Some("リモート併用".to_string()),  // ENUM補正後
    flow_dept: Some("2次請け".to_string()),  // ENUM補正後

    // Tier 3 (nice-to-have)
    project_name: Some("ECサイトリニューアル開発".to_string()),
    preferred_skills_keywords: Some(vec!["kubernetes".to_string(), "ci/cd".to_string()]),
    contract_months: Some(6),

    // メタ情報
    tier1_extracted: 4,
    tier2_extracted: 3,
    extraction_method: "rust_only".to_string(),
}
```

**⚠️ 注意**: 直人材（T.Y様）の情報は既にDBに登録済み。本システムがパースするのはこの「案件紹介」メールである。

---

### 例2: ENUM補正ロジック（corrections/）

> **[Example]** このセクションは補正ロジックの入出力例示です。

#### 2a: 都道府県補正（todofuken.rs）

**⚠️ 設計方針（重要）**:
- **入力**: 何でも受ける（東京/とうきょう/tokyo/Tokyo など）
- **出力**: 必ず日本語正規形（「東京都」「神奈川県」等）
- **理由**: 既存JS/DB/画面表示が全て日本語形式のため、実システムに合わせる

| 入力 | 出力 | 説明 |
|------|------|------|
| `"東京"` | `"東京都"` | 「都」を付与 |
| `"東京都"` | `"東京都"` | そのまま |
| `"とうきょう"` | `"東京都"` | ひらがな対応 |
| `"tokyo"` | `"東京都"` | ローマ字対応 |
| `"神奈川県"` | `"神奈川県"` | そのまま |
| `"横浜"` | `"神奈川県"` | 都市名→県名 |
| `"名古屋"` | `"愛知県"` | 都市名→県名 |
| `"不明"` | `None` | 補正失敗→NULL |

```rust
// corrections/todofuken.rs

/// 都道府県の正規形（内部コード + 日本語名を持つ）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prefecture {
    Tokyo,
    Kanagawa,
    Osaka,
    Aichi,
    Fukuoka,
    // ... 47都道府県
}

impl Prefecture {
    /// 保存・出力用：日本語正規形（必ずこちらを使う）
    pub fn canonical_jp(self) -> &'static str {
        match self {
            Prefecture::Tokyo => "東京都",
            Prefecture::Kanagawa => "神奈川県",
            Prefecture::Osaka => "大阪府",
            Prefecture::Aichi => "愛知県",
            Prefecture::Fukuoka => "福岡県",
            // ...
        }
    }

    /// 内部コード（ログ・デバッグ用、DBには保存しない）
    pub fn code(self) -> &'static str {
        match self {
            Prefecture::Tokyo => "tokyo",
            Prefecture::Kanagawa => "kanagawa",
            Prefecture::Osaka => "osaka",
            Prefecture::Aichi => "aichi",
            Prefecture::Fukuoka => "fukuoka",
            // ...
        }
    }
}

/// 入力を都道府県enumに正規化
pub fn normalize_prefecture(input: &str) -> Option<Prefecture> {
    let s = input.trim().to_lowercase();
    // 「都/府/県」を除去して比較
    let stripped = s
        .trim_end_matches("都")
        .trim_end_matches("府")
        .trim_end_matches("県");

    match stripped {
        "東京" | "とうきょう" | "tokyo" => Some(Prefecture::Tokyo),
        "神奈川" | "かながわ" | "横浜" | "川崎" | "kanagawa" => Some(Prefecture::Kanagawa),
        "大阪" | "おおさか" | "osaka" => Some(Prefecture::Osaka),
        "愛知" | "あいち" | "名古屋" | "aichi" => Some(Prefecture::Aichi),
        "福岡" | "ふくおか" | "博多" | "fukuoka" => Some(Prefecture::Fukuoka),
        // ... 47都道府県分のマッピング
        _ => None,
    }
}

/// DB保存用：必ず日本語正規形を返す（これがメインAPI）
pub fn correct_todofuken(input: &str) -> Option<String> {
    normalize_prefecture(input).map(|p| p.canonical_jp().to_string())
}
```

**⚠️ 重要: 都道府県の正式名称ルール（sponto-platform との整合）**:
- DB保存時は**必ず正式名称**を使用: `東京都`, `大阪府`, `神奈川県` 等
- `東京` → `東京都` のように接尾辞（都/道/府/県）を**必ず付与**
- sponto-platform の `todofuken_enum` は正式名称のみを許容
- LLM出力が短縮形（`東京`, `京都`）の場合も `correct_todofuken()` で正規化

```
正式名称の例（すべて接尾辞付き）:
- 東京都, 京都府, 大阪府
- 北海道（※唯一の「道」）
- 神奈川県, 愛知県, 福岡県 等（43県）
```

#### 2b: 商流補正（flow_depth.rs）

**⚠️ 設計方針**: 出力は既存JS/DBに合わせて日本語形式

| 入力 | 出力 | 説明 |
|------|------|------|
| `"プライム"` | `"エンド直"` | 直接契約 |
| `"1次請け"` | `"1次請け"` | そのまま |
| `"二次"` | `"2次請け"` | 漢数字→算用数字 |
| `"3次以降"` | `"4次請け以上"` | 3次以上は最深扱い |
| `"エンド直"` | `"エンド直"` | そのまま |
| `"プライム〜2次"` | `"2次請け"` | 範囲→最深を採用 |
| `"不明"` | `"不明"` | デフォルト |

```rust
// corrections/flow_depth.rs
/// 商流の正規形（既存JSと同じENUM値）
pub const FLOW_DEPT_ENUMS: &[&str] = &[
    "エンド直", "1次請け", "2次請け", "3次請け", "4次請け以上", "不明"
];

pub fn correct_flow_depth(input: &str) -> String {
    let normalized = input.to_lowercase();

    // 範囲指定の場合、最深を採用（保守的）
    if normalized.contains("〜") || normalized.contains("～") || normalized.contains("-") {
        if normalized.contains("4次") || normalized.contains("四次") || normalized.contains("以上") {
            return "4次請け以上".to_string();
        }
        if normalized.contains("3次") || normalized.contains("三次") {
            return "3次請け".to_string();
        }
        if normalized.contains("2次") || normalized.contains("二次") {
            return "2次請け".to_string();
        }
    }

    if normalized.contains("プライム") || normalized.contains("エンド直") || normalized.contains("直請") {
        return "エンド直".to_string();
    }
    if normalized.contains("1次") || normalized.contains("一次") || normalized.contains("元請") {
        return "1次請け".to_string();
    }
    if normalized.contains("2次") || normalized.contains("二次") {
        return "2次請け".to_string();
    }
    if normalized.contains("3次") || normalized.contains("三次") {
        return "3次請け".to_string();
    }
    if normalized.contains("4次") || normalized.contains("四次") || normalized.contains("以降") || normalized.contains("以上") {
        return "4次請け以上".to_string();
    }

    "不明".to_string()  // デフォルト（既存JSと同じ）
}
```

#### 2c: リモート形態補正（remote_onsite.rs）

> **📌 正規実装への参照**: リモート形態補正は [CD-7: remote_onsite 補正の一本化](#cd-7-remote_onsite-補正の一本化h-7解決) を参照。
>
> **重要**: 本セクションの入出力例のみを参考とし、実装は CD-7 の二段階設計（`normalize_remote_onsite()` + `correct_remote_onsite()`）に従うこと。

**入出力例**:

| 入力 | 出力 | 説明 |
|------|------|------|
| `"フルリモート"` | `Some("フルリモート")` | そのまま |
| `"完全リモート"` | `Some("フルリモート")` | 同義語 |
| `"週3出社"` | `Some("リモート併用")` | ハイブリッド |
| `"出社必須"` | `Some("フル出社")` | 常駐 |
| `"一部リモート可"` | `Some("リモート併用")` | 部分リモート |
| `"常駐"` | `Some("フル出社")` | 常駐=出社 |
| `""` (空文字) | `None` | 補正不可 → requires_review=true |
| `"不明なパターン"` | `None` | 補正不可 → requires_review=true |

**⚠️ 型統一**: `correct_remote_onsite()` は `Option<String>` を返す。補正不可な場合は `None` を返し、呼び出し側で `requires_review=true` を設定する。

---

### 例3: KO条件判定（SC-KO-UNIFIED準拠）

> **✅ Normative**: この例は [CD-2: SC-KO-UNIFIED](#cd-2-ko判定体系の統一p0-2解決) に準拠した正規実装例です。

**入力（案件 + 人材）**:
```rust {#example}
// [Example] KO判定入力データのサンプル
let project = Project {
    monthly_tanka_max: 70,          // 万円
    work_todofuken: "東京都",        // 日本語正規形
    required_skills_keywords: vec!["java", "spring", "postgresql"],
    remote_onsite: "リモート併用",    // 週2リモート可
    jinzai_flow_limit: "SPONTO一社先まで",  // 2次まで
    foreigner_allowed: true,
    age_limit_upper: None,
};

let talent = Talent {
    // ⚠️ フィールド名マッピング:
    //   bp_talents_enum.desired_monthly_tanka → Rust struct: desired_price_min
    //   talents_enum.desired_price_min → Rust struct: desired_price_min
    //   ※ 統一のため Rust 側は desired_price_min で統一
    desired_price_min: 80,           // 万円（75-85希望の下限）
    residential_todofuken: "神奈川県", // 日本語正規形
    possessed_skills_keywords: vec!["java", "spring boot", "aws"],
    desired_remote_onsite: "フルリモート",
    flow_depth: "1社先",              // 1次まで希望
    nationality: "日本",
    age: 32,
};
```

**KO判定結果（KoDecision 3値版）**:
```rust
// run_all_ko_checks() の結果
let ko_result = KnockoutResultV2 {
    is_hard_knockout: true,  // HardKo が1つ以上
    needs_manual_review: false,
    decisions: vec![
        ("tanka", KoDecision::HardKo {
            reason: "tanka_ko: 利益 -10万 < 閾値5万 (案件上限70万 - 人材下限80万)".to_string(),
        }),
        ("skill", KoDecision::Pass),  // マッチ率 66% >= 30%
        ("location", KoDecision::Pass),  // 神奈川県 → 東京都 通勤圏
        ("remote", KoDecision::HardKo {
            reason: "remote_ko: 案件リモート併用 vs 人材フルリモート希望".to_string(),
        }),
        ("flow", KoDecision::Pass),  // SPONTO一社先まで >= 1社先
        ("nationality", KoDecision::Pass),  // 制限なし
        ("age", KoDecision::Pass),  // 制限なし
    ],
};

// MatchResult への変換
let match_result = MatchResult::from_ko_checks(
    ko_result.decisions.clone(),
    0.72,  // スコア計算結果（HardKoなので最終的に0.0になる）
    score_breakdown,
);

assert_eq!(match_result.score, 0.0);  // HardKo があるので 0.0
assert!(!match_result.auto_match_eligible);  // 自動マッチ不可
assert!(!match_result.manual_review_required);  // SoftKoがないのでfalse
assert_eq!(match_result.ko_reasons.len(), 2);  // 2つのHardKo理由
```

**KO判定ロジック（KoDecision版）**:
```rust
// sr-common/src/matching/ko_checks.rs

/// 全KO判定を実行
pub fn run_all_ko_checks(
    project: &Project,
    talent: &Talent,
) -> KnockoutResultV2 {
    let decisions = vec![
        ("tanka", check_tanka_ko(project, talent)),
        ("skill", check_skill_ko(
            &project.required_skills_keywords,
            &talent.possessed_skills_keywords,
        )),
        ("location", check_location_ko(project, talent)),
        ("remote", check_remote_ko(project, talent)),
        ("flow", check_flow_ko(project, talent)),
        ("nationality", check_nationality_ko(project, talent)),
        ("age", check_age_ko(project, talent)),
    ];

    KnockoutResultV2::new(decisions)
}

/// 単価KO判定
fn check_tanka_ko(project: &Project, talent: &Talent) -> KoDecision {
    let profit = project.monthly_tanka_max as i32 - talent.desired_price_min as i32;

    if profit < 5 {
        KoDecision::HardKo {
            reason: format!(
                "tanka_ko: 利益 {}万 < 閾値5万 (案件上限{}万 - 人材下限{}万)",
                profit, project.monthly_tanka_max, talent.desired_price_min
            ),
        }
    } else {
        KoDecision::Pass
    }
}

/// リモートKO判定
fn check_remote_ko(project: &Project, talent: &Talent) -> KoDecision {
    // リモート互換性マトリクス（CD-7参照）
    // ⚠️ ENUM値は「フル出社」「リモート併用」「フルリモート」の3種のみ
    let compatible = match (project.remote_onsite.as_str(), talent.desired_remote_onsite.as_str()) {
        ("フルリモート", _) => true,  // 案件がフルリモートなら何でもOK
        ("リモート併用", "フルリモート") => false,  // 人材がフルリモート希望だと不可
        ("リモート併用", _) => true,
        ("フル出社", "フル出社" | "リモート併用") => true,
        ("フル出社", "フルリモート") => false,
        _ => true,  // 不明な組み合わせは Pass（SoftKoにする選択肢もあり）
    };

    if compatible {
        KoDecision::Pass
    } else {
        KoDecision::HardKo {
            reason: format!(
                "remote_ko: 案件{} vs 人材{}希望",
                project.remote_onsite, talent.desired_remote_onsite
            ),
        }
    }
}
```

---

### スキルKO判定の実装例（#9 追加: 2025-12-17）

> **📌 Note**: `KoDecision` enum と `KnockoutResultV2` struct の定義は [CD-2: SC-KO-UNIFIED](#cd-2-ko判定体系の統一p0-2解決) を参照。

**背景**: `required_skills_keywords` 空配列の扱いが Rust/Python で不統一だった問題の解決策。

```rust
// sr-common/src/matching/ko_checks.rs
use super::ko_unified::{KoDecision, KnockoutResultV2};

/// スキルKO判定（空配列対応版）
///
/// 【仕様契約】
/// - required_skills が空配列 → SoftKo (manual_review=true)
/// - マッチ率 < 30% → HardKo
/// - それ以外 → Pass
pub fn check_skill_ko(
    required_skills: &[String],
    talent_skills: &[String],
) -> KoDecision {
    // ⚠️ 重要: 空配列は "合格" ではなく "要確認"
    if required_skills.is_empty() {
        return KoDecision::SoftKo {
            reason: "skills_empty: 必須スキル未指定のため要手動確認".to_string(),
        };
    }

    let talent_set: std::collections::HashSet<_> = talent_skills.iter().collect();
    let matched = required_skills.iter()
        .filter(|s| talent_set.contains(s))
        .count();

    let match_rate = matched as f64 / required_skills.len() as f64;

    if match_rate < 0.3 {
        KoDecision::HardKo {
            reason: format!(
                "skill_mismatch: マッチ率 {:.0}% < 30%（{}/{}）",
                match_rate * 100.0, matched, required_skills.len()
            ),
        }
    } else {
        KoDecision::Pass
    }
}

// KnockoutResultV2 の定義は CD-2: SC-KO-UNIFIED を参照

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_ko_empty_array() {
        // 空配列 → SoftKo（HardKo ではない）
        let result = check_skill_ko(&[], &["Java".to_string()]);
        assert!(matches!(result, KoDecision::SoftKo { .. }));
        assert!(!result.is_hard_ko());
        assert!(result.needs_manual_review());
    }

    #[test]
    fn test_skill_ko_low_match() {
        // マッチ率 20% → HardKo
        let result = check_skill_ko(
            &["Java".to_string(), "Python".to_string(), "Go".to_string(), "Rust".to_string(), "C++".to_string()],
            &["Java".to_string()], // 1/5 = 20%
        );
        assert!(result.is_hard_ko());
    }

    #[test]
    fn test_skill_ko_pass() {
        // マッチ率 60% → Pass
        let result = check_skill_ko(
            &["Java".to_string(), "Python".to_string(), "Go".to_string(), "Rust".to_string(), "C++".to_string()],
            &["Java".to_string(), "Python".to_string(), "Go".to_string()], // 3/5 = 60%
        );
        assert!(matches!(result, KoDecision::Pass));
    }
}
```

**Python strategy.py との統一ルール**:

| 状況 | Rust (KoDecision) | Python (strategy.py) | 結果 |
|------|-------------------|----------------------|------|
| `required_skills = []` | `SoftKo` | `manual_review=true` | ✅ 統一 |
| マッチ率 < 30% | `HardKo` | KO（除外） | ✅ 統一 |
| マッチ率 >= 30% | `Pass` | 合格 | ✅ 統一 |

---

### KoDecision 拡張: 各種KO判定関数（#5 追加: 2025-12-17）

> **📌 Note**: `KoDecision` の定義は [CD-2: SC-KO-UNIFIED](#cd-2-ko判定体系の統一p0-2解決) を参照。

**背景**: "safe side" 設計（不明なら KO しない）は、誤 KO を防ぐ代わりに要確認案件を見逃す。
`SoftKo` を使うことで、KO しないが `manual_review=true` をセットする。

```rust
// sr-common/src/matching/ko_checks.rs
//
// 【SC-KO-UNIFIED 拡張: 各種KO判定関数】(#5 追加: 2025-12-17)
//
// 全ての KO 判定で KoDecision を返す:
//   - HardKo: 確実に不適合（即時除外）
//   - SoftKo: 不明または要確認（manual_review=true）
//   - Pass: 問題なし

use super::ko_unified::{KoDecision, KnockoutResultV2, MatchResult};
use std::collections::HashSet;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// NGキーワードKO
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
/// NGキーワードKO判定
///
/// - 重複あり → HardKo
/// - 重複なし → Pass
/// - どちらかがNone → SoftKo（要確認）
pub fn check_ng_keyword_ko(
    talent_ng_keywords: Option<&[String]>,
    project_keywords: Option<&[String]>,
) -> KoDecision {
    match (talent_ng_keywords, project_keywords) {
        (Some(ng), Some(project)) => {
            let ng_set: HashSet<_> = ng.iter().collect();
            let project_set: HashSet<_> = project.iter().collect();
            let overlap: Vec<_> = ng_set.intersection(&project_set).collect();

            if !overlap.is_empty() {
                KoDecision::HardKo {
                    reason: format!(
                        "ng_keyword_overlap: {:?} が重複",
                        overlap.iter().take(3).collect::<Vec<_>>()
                    ),
                }
            } else {
                KoDecision::Pass
            }
        }
        (None, _) | (_, None) => {
            // どちらかが None → 比較不可、要確認
            KoDecision::SoftKo {
                reason: "ng_keyword_unknown: キーワード情報不足のため要確認".to_string(),
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 商流制限KO
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 📌 正規実装: CD-4「商流のdepth正規化」を参照
//
// 商流パースとKO判定は以下のCD-4正規関数を使用すること:
//   - parse_talent_flow_depth(): 人材側商流 → FlowDepth (u8)
//   - parse_project_flow_depth(): 案件側商流 → FlowDepth (u8)
//   - parse_flow_limit(): 商流制限 → FlowDepth (u8)
//   - check_flow_ko(): 商流KO判定（depth ベース比較）
//
// ⚠️ 型統一: FlowDepth = u8（u32ではない）
// ⚠️ 「商流制限なし」は Some(u8::MAX) を返す（99ではない）

/// 商流制限KO判定（CD-4準拠ラッパー）
///
/// - 人材の商流 > 案件の制限 → HardKo
/// - 人材の商流 <= 案件の制限 → Pass
/// - どちらかが不明 → SoftKo（要確認）
pub fn check_flow_limit_ko(
    talent_flow_depth: Option<&str>,
    project_flow_limit: Option<&str>,
) -> KoDecision {
    // CD-4の正規関数を使用
    let talent_depth = talent_flow_depth.and_then(parse_talent_flow_depth);
    let project_limit = project_flow_limit.and_then(parse_flow_limit);

    check_flow_ko(talent_depth, project_limit)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 稼働開始日KO
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
use chrono::NaiveDate;

/// 稼働開始日KO判定
///
/// - 人材の稼働可能日 > 案件の開始日 → HardKo
/// - 人材の稼働可能日 <= 案件の開始日 → Pass
/// - どちらかが不明 → SoftKo（要確認）
pub fn check_availability_ko(
    talent_available_date: Option<NaiveDate>,
    project_start_date: Option<NaiveDate>,
) -> KoDecision {
    match (talent_available_date, project_start_date) {
        (Some(t_date), Some(p_date)) => {
            if t_date > p_date {
                KoDecision::HardKo {
                    reason: format!(
                        "availability_conflict: 人材稼働可能日({}) > 案件開始日({})",
                        t_date, p_date
                    ),
                }
            } else {
                KoDecision::Pass
            }
        }
        _ => {
            KoDecision::SoftKo {
                reason: format!(
                    "availability_unknown: talent={:?}, project={:?}",
                    talent_available_date, project_start_date
                ),
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 単価KO（利益率）
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
const MIN_PROFIT_THRESHOLD: i32 = 5;  // 最低利益: 5万円

/// 単価KO判定（利益 < 5万円）
///
/// - 利益 < 5万円 → HardKo
/// - 利益 >= 5万円 → Pass
/// - どちらかが不明 → SoftKo（要確認）
pub fn check_tanka_ko(
    talent_desired_min: Option<i32>,
    project_tanka_max: Option<i32>,
) -> KoDecision {
    match (talent_desired_min, project_tanka_max) {
        (Some(t), Some(p)) => {
            let profit = p - t;
            if profit < MIN_PROFIT_THRESHOLD {
                KoDecision::HardKo {
                    reason: format!(
                        "tanka_insufficient_profit: {}万 - {}万 = {}万 < 最低{}万",
                        p, t, profit, MIN_PROFIT_THRESHOLD
                    ),
                }
            } else {
                KoDecision::Pass
            }
        }
        _ => {
            KoDecision::SoftKo {
                reason: format!(
                    "tanka_unknown: talent_min={:?}, project_max={:?}",
                    talent_desired_min, project_tanka_max
                ),
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 統合KO判定
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
/// 全KO判定を実行し、KnockoutResultV2 を返す
pub fn run_all_ko_checks(
    talent: &Talent,
    project: &Project,
    received_at: NaiveDate,
) -> KnockoutResultV2 {
    // 日付正規化
    let talent_date = talent.available_date_raw.as_ref()
        .and_then(|raw| normalize_start_date(raw, received_at).date);
    let project_date = project.start_date;

    let decisions = vec![
        ("ng_keyword", check_ng_keyword_ko(
            talent.ng_keywords.as_deref(),
            project.project_keywords.as_deref(),
        )),
        ("skill", check_skill_ko(
            &project.required_skills_keywords.clone().unwrap_or_default(),
            &talent.possessed_skills_keywords.clone().unwrap_or_default(),
        )),
        ("flow_limit", check_flow_limit_ko(
            talent.flow_depth.as_deref(),
            project.jinzai_flow_limit.as_deref(),
        )),
        ("availability", check_availability_ko(talent_date, project_date)),
        ("tanka", check_tanka_ko(
            talent.desired_price_min,
            project.monthly_tanka_max,
        )),
    ];

    KnockoutResultV2::new(decisions)
}

#[cfg(test)]
mod tests_ko_checks {
    use super::*;

    #[test]
    fn test_ng_keyword_overlap() {
        let result = check_ng_keyword_ko(
            Some(&["金融".to_string(), "保険".to_string()]),
            Some(&["金融".to_string(), "Java".to_string()]),
        );
        assert!(result.is_hard_ko());
    }

    #[test]
    fn test_ng_keyword_no_overlap() {
        let result = check_ng_keyword_ko(
            Some(&["金融".to_string()]),
            Some(&["Java".to_string()]),
        );
        assert!(matches!(result, KoDecision::Pass));
    }

    #[test]
    fn test_ng_keyword_unknown() {
        let result = check_ng_keyword_ko(None, Some(&["Java".to_string()]));
        assert!(result.needs_manual_review());
        assert!(!result.is_hard_ko());
    }

    #[test]
    fn test_tanka_insufficient() {
        // 70 - 68 = 2万 < 5万
        let result = check_tanka_ko(Some(68), Some(70));
        assert!(result.is_hard_ko());
    }

    #[test]
    fn test_tanka_sufficient() {
        // 80 - 70 = 10万 >= 5万
        let result = check_tanka_ko(Some(70), Some(80));
        assert!(matches!(result, KoDecision::Pass));
    }

    #[test]
    fn test_tanka_unknown() {
        let result = check_tanka_ko(None, Some(80));
        assert!(result.needs_manual_review());
    }
}
```

**KoDecision 適用範囲まとめ**:

| チェック | HardKo条件 | SoftKo条件 | Pass条件 |
|---------|-----------|-----------|---------|
| スキル | マッチ率 < 30% | `required_skills = []` | マッチ率 >= 30% |
| NGキーワード | 重複あり | どちらかがNone | 重複なし |
| 商流制限 | 人材 > 制限 | どちらかが不明 | 人材 <= 制限 |
| 稼働開始日 | 人材 > 案件 | どちらかが不明 | 人材 <= 案件 |
| 単価 | 利益 < 5万 | どちらかがNone | 利益 >= 5万 |

---

### 例4: スコアリング（KO通過後）

> **[Example]** このセクションはスコアリング出力の例示です。

**入力**: KOを通過した案件・人材ペア

**出力（MatchScore）**:
```rust
MatchScore {
    total: 72,  // 100点満点
    breakdown: ScoreBreakdown {
        tanka: 20,        // 25点満点: 利益10万 → 20点
        remote: 5,        // 10点満点: hybrid vs hybrid → 5点（部分一致）
        required_skills: 35, // 50点満点: 70%マッチ → 35点
        preferred_skills: 2, // 5点満点: 一部一致
        experience: 10,   // 10点満点: 10年 >= 要件5年
    },
    rank: "B",  // A: 80+, B: 60-79, C: 40-59, D: <40
}
```

---

### 例5: 全体フロー（End-to-End）

> **[Example]** このセクションは全体フローの例示です。

```
[入力] BP案件メール（案件紹介）from anken_emails
    ↓
[sr-extractor] 正規表現で情報抽出
    ↓
[corrections/*] ENUM値を正規化
    - 都道府県: "渋谷" → "東京都"
    - リモート: "フルリモ希望" → "フルリモート"
    - 商流: "プライム〜2次" → "2次請け"
    ↓
[ProjectExtraction] 抽出結果構造体
    ↓
[projects_enum INSERT] DB保存
    ↓
[talents_enum SELECT] 既存の直人材を取得
    ↓
[matching/knockout] 直人材とのKO判定
    ↓
[KO通過した場合]
    ↓
[matching/scoring] スコアリング
    ↓
[MatchResult] マッチング結果
```

---

## 📚 スキルエイリアス辞書（移植必須）

**出典**: `sponto-platform/apps/business-api/app/core/skill_processor.py`

> **📌 正規実装**: スキル正規化ロジックは [CD-8: スキル正規化の一元化](#cd-8-スキル正規化の一元化critical-2解決) を参照。
>
> **重要**: CD-8 の `ALIAS_TO_CANONICAL` マップと `normalize_skill()` 関数を使用すること。
> 本セクションではエイリアス一覧のみを参考情報として提供。

スキルマッチングの精度向上のため、同義語を正規形に変換する辞書。

**エイリアス一覧（参考）**:

```
=== JavaScript エコシステム ===
javascript: js, ecmascript, es6, es2015
typescript: ts
nodejs: node.js, node js, node

=== フロントエンドフレームワーク ===
react: reactjs, react.js, react16, react17, react18
vue: vue.js, vuejs, vue2, vue3
angular: angularjs, angular.js, angular2+
nextjs: next.js, next js
nuxt: nuxtjs, nuxt.js

=== CSS/スタイリング ===
css: css3
sass: scss
tailwind: tailwindcss

=== バックエンドフレームワーク ===
spring: spring boot, springboot, springframework
django: drf, django rest framework
flask: python flask
express: express.js, expressjs
fastapi: fast api
laravel: php laravel

=== データベース ===
postgresql: postgres, pg
mysql: mariadb
mongodb: mongo
redis: redis cache
elasticsearch: elastic search, es

=== クラウドプラットフォーム ===
aws: amazon web services, amazon aws
gcp: google cloud platform, google cloud
azure: microsoft azure, ms azure

=== プログラミング言語 ===
python: python3, py
java: java8, java11, java17, openjdk
csharp: c#, c sharp, .net, dotnet
cplusplus: c++, cpp
golang: go
rust: rust lang
php: php7, php8
ruby: ruby lang
swift: ios swift
kotlin: kotlin jvm

=== DevOps/インフラ ===
docker: containerization
kubernetes: k8s, kube
jenkins: jenkins ci
git: github, gitlab
terraform: iac

=== AI/ML（日本語対応） ===
ai: artificial intelligence, 人工知能
ml: machine learning, 機械学習
llm: large language model, 大規模言語モデル
chatgpt: gpt, openai, generative ai, 生成ai
deeplearning: deep learning, ディープラーニング
tensorflow: tf
pytorch: torch

=== テスト/モバイル/データ ===
jest: jest testing
cypress: e2e testing
selenium: selenium webdriver
pytest: py test
reactnative: react native, rn
flutter: dart flutter
spark: apache spark
kafka: apache kafka
pandas: python pandas
```

**⚠️ 実装時の注意**: 上記はエイリアス一覧の参考。実装は CD-8 の `ALIAS_TO_CANONICAL` マップ（alias → canonical 形式）を使用すること。

**使用例**:
| 入力 | 正規形 |
|------|--------|
| `"React.js"` | `"react"` |
| `"k8s"` | `"kubernetes"` |
| `"python3"` | `"python"` |
| `"AWS Cloud"` | `"aws"` |
| `"生成AI"` | `"chatgpt"` |
| `"機械学習"` | `"ml"` |

---

## 🧮 詳細スコアリングロジック（移植必須）

**出典**: `sponto-platform/apps/business-api/app/services/matching/business_rules_engine.py`

### スコア構成（加重合成）

```
最終スコア = Σ(コンポーネントスコア × 重み)

コンポーネント:
  tanka      × 0.25  (単価適合性)
  location   × 0.15  (勤務地適合性)
  skills     × 0.40  (スキル適合性)
  experience × 0.15  (経験年数適合性)
  contract   × 0.05  (契約形態適合性)
  ─────────────────
  合計        1.00
```

### Rust実装

```rust
// src/matching/scoring.rs

use crate::models::{Talent, Project};
use crate::config::MatchingConfig;

#[derive(Debug, Clone)]
pub struct ScoringResult {
    pub score: f64,           // 0.0〜1.0
    pub max_score: f64,
    pub status: &'static str, // "PERFECT_MATCH", "MATCH", "PARTIAL_MATCH", "MISS", "UNKNOWN"
    pub details: String,
}

#[derive(Debug, Clone)]
pub struct MatchScore {
    pub total: f64,
    pub tanka: ScoringResult,
    pub location: ScoringResult,
    pub skills: ScoringResult,
    pub experience: ScoringResult,
    pub contract: ScoringResult,
}

pub struct BusinessRulesEngine {
    config: MatchingConfig,
}

impl BusinessRulesEngine {
    pub fn new(config: MatchingConfig) -> Self {
        Self { config }
    }

    /// 総合スコア計算
    pub fn calculate_match_score(&self, talent: &Talent, project: &Project) -> MatchScore {
        let tanka = self.score_tanka(talent, project);
        let location = self.score_location(talent, project);
        let skills = self.score_skills(talent, project);
        let experience = self.score_experience(talent, project);
        let contract = self.score_contract(talent, project);

        let total =
            tanka.score * 0.25 +
            location.score * 0.15 +
            skills.score * 0.40 +
            experience.score * 0.15 +
            contract.score * 0.05;

        MatchScore { total, tanka, location, skills, experience, contract }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 1. 単価スコアリング
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    fn score_tanka(&self, talent: &Talent, project: &Project) -> ScoringResult {
        let talent_tanka = match talent.desired_price_min {
            Some(t) => t as f64,
            None => return ScoringResult {
                score: 0.5, max_score: 1.0,
                status: "UNKNOWN",
                details: "単価情報なし - 中立スコア (0.5/1.0)".into(),
            },
        };

        let project_tanka = match project.monthly_tanka_max {
            Some(t) => t as f64,
            None => return ScoringResult {
                score: 0.5, max_score: 1.0,
                status: "UNKNOWN",
                details: "案件単価情報なし - 中立スコア (0.5/1.0)".into(),
            },
        };

        let profit = project_tanka - talent_tanka;
        let min_profit = self.config.tanka_profit_minimum; // デフォルト: 5.0万円
        let optimal_profit = project_tanka * self.config.tanka_profit_optimal; // デフォルト: 0.25

        if profit < min_profit {
            return ScoringResult {
                score: 0.0, max_score: 1.0,
                status: "MISS",
                details: format!("利益不足: {}万円 < {}万円 (0.0/1.0)", profit, min_profit),
            };
        }

        let (score, status, details) = if profit >= optimal_profit {
            (1.0, "PERFECT_MATCH", format!("最適利益: {}万円 ≥ {:.1}万円 (1.0/1.0)", profit, optimal_profit))
        } else if profit >= min_profit * 3.0 {
            (0.9, "MATCH", format!("非常に良好: {}万円 ≥ {}万円 (0.9/1.0)", profit, min_profit * 3.0))
        } else if profit >= min_profit * 2.0 {
            (0.7, "MATCH", format!("良好: {}万円 ≥ {}万円 (0.7/1.0)", profit, min_profit * 2.0))
        } else {
            (0.4, "OK_MATCH", format!("最低限: {}万円 ≥ {}万円 (0.4/1.0)", profit, min_profit))
        };

        ScoringResult { score, max_score: 1.0, status, details }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 2. 勤務地スコアリング
    // ⚠️ Example（非規範）: 正規実装は CD-1 evaluate_location() を使用すること
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    fn score_location(&self, talent: &Talent, project: &Project) -> ScoringResult {
        // フルリモート案件は常に完全互換
        if project.remote_onsite.as_deref() == Some("フルリモート") {
            return ScoringResult {
                score: 1.0, max_score: 1.0,
                status: "PERFECT_MATCH",
                details: "フルリモート案件 - 完全互換 (1.0/1.0)".into(),
            };
        }

        // 人材がフルリモート希望だが案件がそうでない場合
        if talent.work_style.as_deref() == Some("full_remote") {
            let (score, status, details) = match project.remote_onsite.as_deref() {
                Some("リモート併用") => (0.6, "PARTIAL_MATCH",
                    "フルリモ希望 vs ハイブリッド - 許容範囲 (0.6/1.0)"),
                _ => (0.2, "POOR_MATCH",
                    "フルリモ希望 vs フル出社 - 低互換 (0.2/1.0)"),
            };
            return ScoringResult { score, max_score: 1.0, status, details: details.into() };
        }

        // エリアマッチング
        match (&talent.residential_area, &project.work_area) {
            (Some(t_area), Some(p_area)) if t_area == p_area => ScoringResult {
                score: 1.0, max_score: 1.0,
                status: "PERFECT_MATCH",
                details: format!("エリア一致: {} = {} (1.0/1.0)", t_area, p_area),
            },
            (Some(t_area), Some(p_area)) => {
                let score = if talent.work_style.as_deref() == Some("onsite") { 0.3 } else { 0.1 };
                ScoringResult {
                    score, max_score: 1.0,
                    status: "POOR_MATCH",
                    details: format!("エリア不一致: {} ≠ {} ({}/1.0)", t_area, p_area, score),
                }
            }
            _ => ScoringResult {
                score: 0.5, max_score: 1.0,
                status: "UNKNOWN",
                details: "勤務地情報なし - 中立スコア (0.5/1.0)".into(),
            },
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 3. スキルスコアリング（必須75% + 歓迎25%）
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    fn score_skills(&self, talent: &Talent, project: &Project) -> ScoringResult {
        let talent_skills: std::collections::HashSet<_> =
            talent.possessed_skills_keywords.iter().flatten().collect();
        let required_skills: std::collections::HashSet<_> =
            project.required_skills_keywords.iter().flatten().collect();
        let preferred_skills: std::collections::HashSet<_> =
            project.preferred_skills_keywords.iter().flatten().collect();

        if required_skills.is_empty() && preferred_skills.is_empty() {
            return ScoringResult {
                score: 0.5, max_score: 1.0,
                status: "UNKNOWN",
                details: "スキル要件なし - 中立スコア (0.5/1.0)".into(),
            };
        }

        let mut total_score = 0.0;
        let mut details_parts = Vec::new();

        // 必須スキル（75%配分）
        if !required_skills.is_empty() {
            let matched: Vec<_> = talent_skills.intersection(&required_skills).collect();
            let match_rate = matched.len() as f64 / required_skills.len() as f64;
            let required_score = match_rate * 0.75;
            total_score += required_score;

            let status = if match_rate >= self.config.skill_match_minimum {
                if match_rate >= 0.8 { "MATCH" } else { "PARTIAL_MATCH" }
            } else { "MISS" };

            details_parts.push(format!(
                "必須スキル: {}/{}({:.0}%) = {:.2}/0.75 [{}]",
                matched.len(), required_skills.len(), match_rate * 100.0, required_score, status
            ));
        }

        // 歓迎スキル（25%配分）
        if !preferred_skills.is_empty() {
            let matched: Vec<_> = talent_skills.intersection(&preferred_skills).collect();
            let match_rate = matched.len() as f64 / preferred_skills.len() as f64;
            let preferred_score = match_rate * 0.25;
            total_score += preferred_score;

            details_parts.push(format!(
                "歓迎スキル: {}/{}({:.0}%) = {:.2}/0.25",
                matched.len(), preferred_skills.len(), match_rate * 100.0, preferred_score
            ));
        }

        let status = if total_score >= 0.6 { "MATCH" }
            else if total_score >= 0.3 { "PARTIAL_MATCH" }
            else { "MISS" };

        ScoringResult {
            score: total_score,
            max_score: 1.0,
            status,
            details: details_parts.join("; "),
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 4. 経験年数スコアリング（バッファ考慮）
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    fn score_experience(&self, talent: &Talent, project: &Project) -> ScoringResult {
        let required = match project.min_experience_years {
            Some(r) => r as f64,
            None => return ScoringResult {
                score: 1.0, max_score: 1.0,
                status: "PERFECT_MATCH",
                details: "経験年数要件なし (1.0/1.0)".into(),
            },
        };

        let actual = match talent.min_experience_years {
            Some(a) => a as f64,
            None => return ScoringResult {
                score: 0.2, max_score: 1.0,
                status: "UNKNOWN",
                details: "経験年数不明 - リスクあり (0.2/1.0)".into(),
            },
        };

        let buffer = self.config.experience_buffer_years; // デフォルト: 0.5

        let (score, status, details) = if actual >= required + buffer * 4.0 {
            (1.0, "PERFECT_MATCH", format!("大幅超過: {}年 ≥ {}年 (1.0/1.0)", actual, required + buffer * 4.0))
        } else if actual >= required + buffer * 2.0 {
            (0.9, "MATCH", format!("十分超過: {}年 ≥ {}年 (0.9/1.0)", actual, required + buffer * 2.0))
        } else if actual >= required + buffer {
            (0.8, "MATCH", format!("超過: {}年 ≥ {}年 (0.8/1.0)", actual, required + buffer))
        } else if actual >= required {
            (0.7, "MATCH", format!("要件達成: {}年 ≥ {}年 (0.7/1.0)", actual, required))
        } else if actual >= required - buffer {
            (0.4, "PARTIAL_MATCH", format!("要件近接: {}年 ≈ {}年 (0.4/1.0)", actual, required))
        } else {
            (0.0, "MISS", format!("経験不足: {}年 < {}年 (0.0/1.0)", actual, required))
        };

        ScoringResult { score, max_score: 1.0, status, details }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 5. 契約形態スコアリング
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    fn score_contract(&self, talent: &Talent, project: &Project) -> ScoringResult {
        // 案件が個人可なら常に互換
        if project.is_kojin_ok.unwrap_or(true) {
            return ScoringResult {
                score: 1.0, max_score: 1.0,
                status: "PERFECT_MATCH",
                details: "案件は全契約形態可 (1.0/1.0)".into(),
            };
        }

        // 個人NGの場合
        if talent.primary_contract_type.as_deref() == Some("直個人") {
            if talent.secondary_contract_type.is_some() {
                return ScoringResult {
                    score: 0.7, max_score: 1.0,
                    status: "PARTIAL_MATCH",
                    details: format!("直個人→{}への転換可 (0.7/1.0)",
                        talent.secondary_contract_type.as_deref().unwrap_or("不明")),
                };
            }
            return ScoringResult {
                score: 0.0, max_score: 1.0,
                status: "MISS",
                details: "個人NG案件 vs 直個人のみ (0.0/1.0)".into(),
            };
        }

        ScoringResult {
            score: 1.0, max_score: 1.0,
            status: "PERFECT_MATCH",
            details: format!("契約形態互換: {} (1.0/1.0)",
                talent.primary_contract_type.as_deref().unwrap_or("不明")),
        }
    }
}
```

---

## 🔍 事前フィルタリングロジック（移植必須）

**出典**: `sponto-platform/apps/business-api/app/services/matching/enhanced_prefilter.py`

### 概要

詳細スコアリング前に候補を絞り込む軽量フィルタ。10%未満のスコアは除外。

```rust
// src/matching/prefilter.rs

pub struct EnhancedPreFilter {
    config: MatchingConfig,
}

impl EnhancedPreFilter {
    /// 事前フィルタリング（候補絞り込み）
    pub fn filter_candidates(
        &self,
        talent: &Talent,
        projects: &[Project],
    ) -> Vec<(Project, f64)> {
        let mut candidates: Vec<(Project, f64)> = projects
            .iter()
            .filter_map(|project| {
                let score = self.calculate_preliminary_score(talent, project);

                // 10%以上のみ通過
                if score > 0.1 {
                    Some((project.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        // スコア降順ソート
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 上位N件に制限（デフォルト: 500件）
        candidates.truncate(self.config.max_candidates_prefilter);

        candidates
    }

    /// prefilter用スコア計算（#4修正: 2025-12-17）
    ///
    /// 【SC-11準拠】prefilterは詳細スコアと異なる重みを使用:
    /// - experience: 0.10 (詳細スコアは0.15)
    /// - skills: 0.45 (詳細スコアは0.40) ← 差分を吸収
    ///
    /// 理由: 500件→50件の足切り段階では経験年数より単価・スキルを重視
    fn calculate_preliminary_score(&self, talent: &Talent, project: &Project) -> f64 {
        // 絶対KO条件チェック
        // NOTE: talent.received_at は talents_enum.received_at から取得済み
        if self.has_knockout_issues(talent, project, &talent.received_at) {
            return 0.0;
        }

        // 加重スコア計算（prefilter用重み、合計 1.0）
        // ⚠️ SC-11: 詳細スコア(BusinessRulesEngine)とは異なる重みを意図的に使用
        let tanka = self.score_tanka_compat(talent, project) * 0.25;       // tanka_weight
        let location = self.score_location_compat(talent, project) * 0.15; // location_weight
        let skills = self.score_skill_compat(talent, project) * 0.45;      // skills_weight (prefilter: +0.05)
        let experience = self.score_experience_compat(talent, project) * 0.10; // experience_weight (prefilter: -0.05)
        let contract = self.score_contract_compat(talent, project) * 0.05; // contract_weight

        tanka + location + skills + experience + contract
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 絶対KO条件（スコア0で即却下）
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    /// received_at: メール受信日（"即日"や"来月"の基準日として使用）
    fn has_knockout_issues(&self, talent: &Talent, project: &Project, received_at: &chrono::NaiveDate) -> bool {
        // 1. NGキーワード重複
        if let (Some(ng_keywords), Some(project_keywords)) =
            (&talent.ng_keywords, &project.project_keywords)
        {
            let ng_set: std::collections::HashSet<_> = ng_keywords.iter().collect();
            let project_set: std::collections::HashSet<_> = project_keywords.iter().collect();
            if !ng_set.is_disjoint(&project_set) {
                return true; // NGキーワード重複 → KO
            }
        }

        // 2. 商流制限違反
        // NOTE: 不明な値の場合は KO しない（安全寄り）
        if let (Some(talent_flow), Some(project_limit)) =
            (&talent.flow_depth, &project.jinzai_flow_limit)
        {
            // talent_flow の解析: 不明 → None（KO判定をスキップ）
            let talent_level: Option<u32> = match talent_flow.as_str() {
                "1社先" => Some(1),
                "2社先" => Some(2),
                "3社先以上" => Some(3),
                _ => None,  // 不明な値 → KO判定スキップ（安全寄り）
            };
            // project_limit の解析: 不明 → None（KO判定をスキップ）
            let project_level_limit: Option<u32> = match project_limit.as_str() {
                "SPONTO直人材" => Some(0),
                "SPONTO一社先まで" => Some(1),
                "商流制限なし" => Some(99),
                _ => None,  // 不明な値 → KO判定スキップ（安全寄り）
            };
            // 両方が既知の場合のみ比較
            if let (Some(t_level), Some(p_limit)) = (talent_level, project_level_limit) {
                if t_level > p_limit {
                    return true; // 商流制限違反 → KO
                }
            }
            // どちらかが不明の場合は KO しない（manual_review で対応）
        }

        // 3. 稼働開始日不一致
        if self.has_availability_conflict(talent, project, received_at) {
            return true;
        }

        false
    }

    /// 稼働日付の互換性チェック（KO判定）
    ///
    /// MVPルール（安全寄り）:
    /// - どちらかがUnknown → KOしない（監視対象として manual_review_reason='availability_unknown' を設定）
    /// - 両方がDATE → talent.available_date <= project.start_date ならOK、逆ならKO
    /// - "即日" は received_at（メール受信日）に倒す
    fn has_availability_conflict(&self, talent: &Talent, project: &Project, received_at: &chrono::NaiveDate) -> bool {
        // 日付を正規化
        let talent_date = self.normalize_date(&talent.available_date_raw, received_at);
        let project_date = self.normalize_date(&project.start_date, received_at);

        match (talent_date, project_date) {
            // 両方が日付として解釈できた場合
            (Some(t_date), Some(p_date)) => {
                // 人材の稼働可能日が案件開始日より後ならKO
                t_date > p_date
            }
            // どちらかが不明の場合 → KOしない（安全寄り）
            _ => false,
        }
    }

    /// 日付文字列を NaiveDate に正規化
    ///
    /// 対応パターン:
    /// - "即日" → received_at
    /// - "2025-01-15", "2025/01/15" → そのまま解析
    /// - "1月上旬", "1月中旬", "1月下旬" → 該当月の1日/15日/25日
    /// - "来月", "再来月" → received_at 基準で計算
    /// - 解析不能 → None
    fn normalize_date(&self, raw: &Option<String>, received_at: &chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        use chrono::{Datelike, Duration, NaiveDate};

        let raw_str = raw.as_ref()?;
        let trimmed = raw_str.trim();

        // 即日
        if trimmed == "即日" || trimmed.contains("ASAP") || trimmed.contains("随時") {
            return Some(*received_at);
        }

        // ISO形式 (2025-01-15 or 2025/01/15)
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
            return Some(date);
        }
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y/%m/%d") {
            return Some(date);
        }

        // 日本語月表現 (1月上旬, 12月中旬, etc.)
        let jp_month_re = regex::Regex::new(r"(\d{1,2})月(上旬|中旬|下旬)?").ok()?;
        if let Some(caps) = jp_month_re.captures(trimmed) {
            let month: u32 = caps.get(1)?.as_str().parse().ok()?;
            let period = caps.get(2).map(|m| m.as_str()).unwrap_or("上旬");

            // 年を推定（received_at の年、ただし受信月より小さければ翌年）
            let mut year = received_at.year();
            if month < received_at.month() {
                year += 1;
            }

            let day = match period {
                "上旬" => 1,
                "中旬" => 15,
                "下旬" => 25,
                _ => 1,
            };

            return NaiveDate::from_ymd_opt(year, month, day);
        }

        // 相対表現 (来月, 再来月) → ⚠️ 2025-12-17修正: カレンダーベース計算に統一
        // 旧実装: Duration::days(30) → 問題: 31日月で翌月にならない
        // 新実装: 翌月1日 → normalize_start_date() と統一
        if trimmed.contains("来月") {
            let (y, m) = if received_at.month() == 12 {
                (received_at.year() + 1, 1)
            } else {
                (received_at.year(), received_at.month() + 1)
            };
            return NaiveDate::from_ymd_opt(y, m as u32, 1);
        }
        if trimmed.contains("再来月") {
            let (y, m) = if received_at.month() >= 11 {
                (received_at.year() + 1, (received_at.month() + 2 - 12) as u32)
            } else {
                (received_at.year(), received_at.month() + 2)
            };
            return NaiveDate::from_ymd_opt(y, m as u32, 1);
        }
        if trimmed.contains("年明け") || trimmed.contains("年始") {
            let next_year = NaiveDate::from_ymd_opt(received_at.year() + 1, 1, 6)?;
            return Some(next_year);
        }

        None  // 解析不能
    }

    // 簡易スコア計算（省略版）
    fn score_tanka_compat(&self, talent: &Talent, project: &Project) -> f64 {
        match (talent.desired_price_min, project.monthly_tanka_max) {
            (Some(t), Some(p)) => {
                let profit = p as f64 - t as f64;
                if profit < self.config.tanka_profit_minimum { 0.0 }
                else if profit >= p as f64 * self.config.tanka_profit_optimal { 1.0 }
                else if profit >= self.config.tanka_profit_minimum * 2.0 { 0.8 }
                else { 0.4 }
            }
            _ => 0.5,
        }
    }

    // ⚠️ Example（非規範）: 正規実装は CD-1 evaluate_location() を使用すること
    fn score_location_compat(&self, talent: &Talent, project: &Project) -> f64 {
        if project.remote_onsite.as_deref() == Some("フルリモート") { return 1.0; }
        match (&talent.residential_area, &project.work_area) {
            (Some(t), Some(p)) if t == p => 1.0,
            (Some(_), Some(_)) => 0.2,
            _ => 0.5,
        }
    }

    /// ⚠️ CD-8準拠: normalize_skill_set() でスキルを正規化してから比較
    fn score_skill_compat(&self, talent: &Talent, project: &Project) -> f64 {
        // CD-8: スキル正規化（"React.js" → "react", "k8s" → "kubernetes" 等）
        let talent_skills = normalize_skill_set(
            &talent.possessed_skills_keywords.clone().unwrap_or_default()
        );
        let required_skills = normalize_skill_set(
            &project.required_skills_keywords.clone().unwrap_or_default()
        );

        if required_skills.is_empty() { return 0.5; }
        if talent_skills.is_empty() { return 0.0; }

        let matched = talent_skills.intersection(&required_skills).count();
        let rate = matched as f64 / required_skills.len() as f64;
        (rate * 0.8).min(1.0) // 必須スキルは80%重み
    }

    fn score_experience_compat(&self, talent: &Talent, project: &Project) -> f64 {
        match (project.min_experience_years, talent.min_experience_years) {
            (None, _) => 1.0,
            (Some(_), None) => 0.2,
            (Some(req), Some(act)) => {
                let buffer = self.config.experience_buffer_years;
                if act as f64 >= req as f64 + buffer * 2.0 { 1.0 }
                else if act as f64 >= req as f64 + buffer { 0.8 }
                else if act >= req { 0.6 }
                else if act as f64 >= req as f64 - buffer { 0.3 }
                else { 0.0 }
            }
        }
    }

    fn score_contract_compat(&self, talent: &Talent, project: &Project) -> f64 {
        if project.is_kojin_ok.unwrap_or(true) { return 1.0; }
        if talent.primary_contract_type.as_deref() == Some("直個人") {
            if talent.secondary_contract_type.is_some() { 0.7 } else { 0.0 }
        } else { 1.0 }
    }

    fn score_other_factors(&self, talent: &Talent, project: &Project) -> f64 {
        let mut factors = Vec::new();

        // 年齢チェック
        if let Some(birth_year) = talent.birth_year {
            let age = chrono::Utc::now().year() - birth_year;
            let age_ok = match (project.age_limit_lower, project.age_limit_upper) {
                (Some(lower), Some(upper)) => age >= lower && age <= upper,
                (Some(lower), None) => age >= lower,
                (None, Some(upper)) => age <= upper,
                (None, None) => true,
            };
            factors.push(if age_ok { 1.0 } else { 0.0 });
        }

        // 国籍チェック
        if talent.nationality.as_deref() != Some("日本")
            && project.foreigner_allowed == Some(false)
        {
            factors.push(0.0);
        } else {
            factors.push(1.0);
        }

        if factors.is_empty() { 1.0 }
        else { factors.iter().sum::<f64>() / factors.len() as f64 }
    }
}
```

---

## 📋 ENUM補正ロジック完全版（移植必須）

**出典**: `sponto-platform/scripts/shared/enum_corrections.js`

### 都道府県マッピング

```rust
// src/corrections/todofuken.rs
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

    /// エリアENUM値（#2修正: 2025-12-17、#5修正: 2025-12-18）
    /// ⚠️ 本仕様独自の7エリア分類（sponto-platformの8分離形式とは意図的に異なる）
    /// 理由: 「中部」の曖昧性排除、甲信越・北陸と東海の分離、沖縄の明示的包含
    pub static ref AREA_ENUMS: Vec<&'static str> = vec![
        "北海道・東北",
        "関東",
        "甲信越・北陸",
        "東海",
        "近畿",
        "中国・四国",
        "九州・沖縄",
    ];

    /// 都道府県 → エリア マッピング（#2追加: 2025-12-17）
    /// 都道府県名からエリアENUM値への変換テーブル
    pub static ref TODOFUKEN_TO_AREA: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        // 北海道・東北
        m.insert("北海道", "北海道・東北");
        m.insert("青森", "北海道・東北"); m.insert("青森県", "北海道・東北");
        m.insert("岩手", "北海道・東北"); m.insert("岩手県", "北海道・東北");
        m.insert("宮城", "北海道・東北"); m.insert("宮城県", "北海道・東北");
        m.insert("秋田", "北海道・東北"); m.insert("秋田県", "北海道・東北");
        m.insert("山形", "北海道・東北"); m.insert("山形県", "北海道・東北");
        m.insert("福島", "北海道・東北"); m.insert("福島県", "北海道・東北");
        // 関東
        m.insert("茨城", "関東"); m.insert("茨城県", "関東");
        m.insert("栃木", "関東"); m.insert("栃木県", "関東");
        m.insert("群馬", "関東"); m.insert("群馬県", "関東");
        m.insert("埼玉", "関東"); m.insert("埼玉県", "関東");
        m.insert("千葉", "関東"); m.insert("千葉県", "関東");
        m.insert("東京", "関東"); m.insert("東京都", "関東");
        m.insert("神奈川", "関東"); m.insert("神奈川県", "関東");
        // 甲信越・北陸
        m.insert("新潟", "甲信越・北陸"); m.insert("新潟県", "甲信越・北陸");
        m.insert("富山", "甲信越・北陸"); m.insert("富山県", "甲信越・北陸");
        m.insert("石川", "甲信越・北陸"); m.insert("石川県", "甲信越・北陸");
        m.insert("福井", "甲信越・北陸"); m.insert("福井県", "甲信越・北陸");
        m.insert("山梨", "甲信越・北陸"); m.insert("山梨県", "甲信越・北陸");
        m.insert("長野", "甲信越・北陸"); m.insert("長野県", "甲信越・北陸");
        // 東海
        m.insert("岐阜", "東海"); m.insert("岐阜県", "東海");
        m.insert("静岡", "東海"); m.insert("静岡県", "東海");
        m.insert("愛知", "東海"); m.insert("愛知県", "東海");
        m.insert("三重", "東海"); m.insert("三重県", "東海");
        // 近畿
        m.insert("滋賀", "近畿"); m.insert("滋賀県", "近畿");
        m.insert("京都", "近畿"); m.insert("京都府", "近畿");
        m.insert("大阪", "近畿"); m.insert("大阪府", "近畿");
        m.insert("兵庫", "近畿"); m.insert("兵庫県", "近畿");
        m.insert("奈良", "近畿"); m.insert("奈良県", "近畿");
        m.insert("和歌山", "近畿"); m.insert("和歌山県", "近畿");
        // 中国・四国
        m.insert("鳥取", "中国・四国"); m.insert("鳥取県", "中国・四国");
        m.insert("島根", "中国・四国"); m.insert("島根県", "中国・四国");
        m.insert("岡山", "中国・四国"); m.insert("岡山県", "中国・四国");
        m.insert("広島", "中国・四国"); m.insert("広島県", "中国・四国");
        m.insert("山口", "中国・四国"); m.insert("山口県", "中国・四国");
        m.insert("徳島", "中国・四国"); m.insert("徳島県", "中国・四国");
        m.insert("香川", "中国・四国"); m.insert("香川県", "中国・四国");
        m.insert("愛媛", "中国・四国"); m.insert("愛媛県", "中国・四国");
        m.insert("高知", "中国・四国"); m.insert("高知県", "中国・四国");
        // 九州・沖縄
        m.insert("福岡", "九州・沖縄"); m.insert("福岡県", "九州・沖縄");
        m.insert("佐賀", "九州・沖縄"); m.insert("佐賀県", "九州・沖縄");
        m.insert("長崎", "九州・沖縄"); m.insert("長崎県", "九州・沖縄");
        m.insert("熊本", "九州・沖縄"); m.insert("熊本県", "九州・沖縄");
        m.insert("大分", "九州・沖縄"); m.insert("大分県", "九州・沖縄");
        m.insert("宮崎", "九州・沖縄"); m.insert("宮崎県", "九州・沖縄");
        m.insert("鹿児島", "九州・沖縄"); m.insert("鹿児島県", "九州・沖縄");
        m.insert("沖縄", "九州・沖縄"); m.insert("沖縄県", "九州・沖縄");
        m
    };
}

pub fn correct_todofuken(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() { return None; }

    // 正式名称と完全一致
    let all_prefs: Vec<_> = PREFECTURE_MAP.values().cloned().collect();
    if all_prefs.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    // 短縮形で検索
    for (key, value) in PREFECTURE_MAP.iter() {
        if trimmed == *key || trimmed.starts_with(*key) {
            return Some(value.to_string());
        }
    }

    // 先頭2文字で検索
    if trimmed.chars().count() >= 2 {
        let prefix: String = trimmed.chars().take(2).collect();
        if let Some(value) = PREFECTURE_MAP.get(prefix.as_str()) {
            return Some(value.to_string());
        }
    }

    None
}

/// エリアENUM補正（#2修正: 2025-12-17）
/// 入力: 都道府県名、エリア名、旧形式エリア名（中部、関西等）
/// 出力: 正規化されたエリアENUM値（北海道・東北, 関東, ... 形式）
///
/// 【仕様契約 SC-16: エリアENUM統一】（#5修正: 2025-12-18）
/// - AREA_ENUMS: 本仕様独自の7値（sponto-platformの8分離形式とは意図的に異なる）
/// - TODOFUKEN_TO_AREA: 47都道府県 → エリア変換マップ
/// - 旧形式互換: "中部"→"甲信越・北陸", "関西"→"近畿", "中国"/"四国"→"中国・四国"
pub fn correct_work_area(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() { return None; }

    // 1. 完全一致（新形式エリア名）
    if AREA_ENUMS.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    // 2. 旧形式エリア名 → 新形式への変換
    let legacy_mapping = match trimmed {
        "北海道" | "東北" => Some("北海道・東北"),
        "甲信越" | "北陸" | "中部" => Some("甲信越・北陸"),
        "関西" => Some("近畿"),
        "中国" | "四国" => Some("中国・四国"),
        "九州" | "沖縄" => Some("九州・沖縄"),
        "首都圏" => Some("関東"),
        _ => None,
    };
    if let Some(area) = legacy_mapping {
        return Some(area.to_string());
    }

    // 3. 都道府県名 → エリア変換（TODOFUKEN_TO_AREA使用）
    if let Some(area) = TODOFUKEN_TO_AREA.get(trimmed) {
        return Some(area.to_string());
    }

    // 4. 部分一致（"関東地方" → "関東"、"東京都内" → "関東"）
    for area in AREA_ENUMS.iter() {
        if trimmed.contains(*area) {
            return Some(area.to_string());
        }
    }

    // 5. 都道府県名を含む場合（"東京勤務" → "関東"）
    for (pref, area) in TODOFUKEN_TO_AREA.iter() {
        if trimmed.contains(*pref) {
            return Some(area.to_string());
        }
    }

    // 6. リモート/在宅/全国 → None（エリア不問）
    let lower = trimmed.to_lowercase();
    if lower.contains("リモート") || lower.contains("在宅")
        || lower.contains("全国") || lower.contains("不問") {
        return None;
    }

    None
}

#[cfg(test)]
mod tests_work_area {
    use super::*;

    #[test]
    fn test_new_format_direct() {
        assert_eq!(correct_work_area("関東"), Some("関東".to_string()));
        assert_eq!(correct_work_area("北海道・東北"), Some("北海道・東北".to_string()));
        assert_eq!(correct_work_area("九州・沖縄"), Some("九州・沖縄".to_string()));
    }

    #[test]
    fn test_legacy_to_new() {
        // 旧形式 → 新形式
        assert_eq!(correct_work_area("中部"), Some("甲信越・北陸".to_string()));
        assert_eq!(correct_work_area("関西"), Some("近畿".to_string()));
        assert_eq!(correct_work_area("中国"), Some("中国・四国".to_string()));
        assert_eq!(correct_work_area("四国"), Some("中国・四国".to_string()));
        assert_eq!(correct_work_area("九州"), Some("九州・沖縄".to_string()));
    }

    #[test]
    fn test_todofuken_to_area() {
        assert_eq!(correct_work_area("東京都"), Some("関東".to_string()));
        assert_eq!(correct_work_area("大阪府"), Some("近畿".to_string()));
        assert_eq!(correct_work_area("愛知県"), Some("東海".to_string()));
        assert_eq!(correct_work_area("北海道"), Some("北海道・東北".to_string()));
    }

    #[test]
    fn test_partial_match() {
        assert_eq!(correct_work_area("関東地方"), Some("関東".to_string()));
        assert_eq!(correct_work_area("東京勤務"), Some("関東".to_string()));
    }

    #[test]
    fn test_remote_returns_none() {
        assert_eq!(correct_work_area("フルリモート"), None);
        assert_eq!(correct_work_area("全国可"), None);
    }
}
```

### リモート形態補正

> **📌 正規実装**: [CD-7: remote_onsite 補正の一本化](#cd-7-remote_onsite-補正の一本化h-7解決) を参照。
>
> 本セクションではENUM値のみを定義。実装は CD-7 の二段階設計に従う。

**ENUM定義**:
```rust
// src/corrections/remote_onsite.rs
pub const REMOTE_ONSITE_ENUMS: &[&str] = &["フル出社", "リモート併用", "フルリモート"];
```

**関数シグネチャ（CD-7準拠）**:
```rust
/// 【第1段階】LLM応答パース時 - 常に有効な値を返す
pub fn normalize_remote_onsite(input: &str) -> String;

/// 【第2段階】DB格納前 - 補正不可なら None
pub fn correct_remote_onsite(input: &str) -> Option<String>;
```

**⚠️ 重要**: `correct_remote_onsite()` は `Option<String>` を返す。詳細実装はCD-7参照。

### 商流補正

```rust
// src/corrections/flow_depth.rs

/// 案件商流ENUM: ["エンド直", "1次請け", "2次請け", "3次請け", "4次請け以上", "不明"]
pub fn correct_flow_dept(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() { return "不明".to_string(); }

    // 完全一致
    let valid = ["エンド直", "1次請け", "2次請け", "3次請け", "4次請け以上", "不明"];
    if valid.contains(&trimmed) { return trimmed.to_string(); }

    // パターンマッチング
    if trimmed.contains("エンド直") { return "エンド直".to_string(); }
    if trimmed.contains("1次") || trimmed.contains("元請") { return "1次請け".to_string(); }
    if trimmed.contains("2次") { return "2次請け".to_string(); }
    if trimmed.contains("3次") { return "3次請け".to_string(); }
    if trimmed.contains("4次") { return "4次請け以上".to_string(); }

    "不明".to_string()
}

/// 人材商流ENUM: ["1社先", "2社先", "3社先以上"]
pub fn correct_talent_flow_depth(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() { return "1社先".to_string(); } // デフォルト

    let valid = ["1社先", "2社先", "3社先以上"];
    if valid.contains(&trimmed) { return trimmed.to_string(); }

    if trimmed.contains('3') || trimmed.contains('３') || trimmed.contains("以上") {
        return "3社先以上".to_string();
    }
    if trimmed.contains('2') || trimmed.contains('２') {
        return "2社先".to_string();
    }

    "1社先".to_string()
}

/// 商流制限ENUM: ["SPONTO直人材", "SPONTO一社先まで", "商流制限なし"]
pub fn correct_jinzai_flow_limit(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() { return "SPONTO一社先まで".to_string(); }

    let valid = ["SPONTO直人材", "SPONTO一社先まで", "商流制限なし"];
    if valid.contains(&trimmed) { return trimmed.to_string(); }

    if trimmed.contains("貴社まで") || trimmed.contains("御社まで")
        || trimmed.contains("直人材") || trimmed.contains("貴社社員") {
        return "SPONTO直人材".to_string();
    }
    if trimmed.contains("制限なし") || trimmed.contains("不問") {
        return "商流制限なし".to_string();
    }

    "SPONTO一社先まで".to_string()
}
```

### 日本語スキル補正

> **📌 正規実装**: 完全な実装（補正関数 + KO判定関数）は後続の「[日本語スキル補正・KO判定](#日本語スキル補正ko判定)」セクションを参照。

**ENUM定義**:
```rust
// src/corrections/japanese_skill.rs
pub const JAPANESE_SKILL_ENUMS: &[&str] = &["不要", "N5", "N4", "N3", "N2", "N1", "ネイティブ"];
```

### 英語スキル補正

```rust
// src/corrections/english_skill.rs

/// 英語スキルENUM: ["不要", "読み書き", "会話", "ビジネス", "上級ビジネス", "ネイティブ"]
/// ⚠️ 順序が重要: 上位レベルから判定することで、"ビジネス会話" → "ビジネス" に正しく変換
pub fn correct_english_skill(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() { return None; }

    let valid = ["不要", "読み書き", "会話", "ビジネス", "上級ビジネス", "ネイティブ"];
    if valid.contains(&trimmed) { return Some(trimmed.to_string()); }

    let lower = trimmed.to_lowercase();

    // 順序重要: 上位レベルから判定
    if trimmed.contains("ネイティブ") || lower.contains("native") {
        return Some("ネイティブ".to_string());
    }
    if trimmed.contains("上級ビジネス") || lower.contains("advanced business") || lower.contains("fluent") {
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
/// 案件要求レベル <= 人材レベル なら合格
pub fn english_skill_level(skill: &str) -> i32 {
    match skill {
        "不要" => 0,
        "読み書き" => 1,
        "会話" => 2,
        "ビジネス" => 3,
        "上級ビジネス" => 4,
        "ネイティブ" => 5,
        _ => -1,  // 不明
    }
}

/// 英語スキルKO判定
/// project_required: 案件が要求する英語レベル
/// talent_level: 人材が持つ英語レベル
/// 戻り値: true = KO（不合格）, false = 合格
pub fn is_english_ko(project_required: Option<&str>, talent_level: Option<&str>) -> bool {
    match (project_required, talent_level) {
        (None, _) => false,  // 案件が要求しなければ常に合格
        (Some("不要"), _) => false,  // 不要なら常に合格
        (Some(_), None) => true,  // 案件が要求しているのに人材が不明はKO
        (Some(req), Some(tal)) => {
            let req_level = english_skill_level(req);
            let tal_level = english_skill_level(tal);
            tal_level < req_level  // 人材レベルが要求未満ならKO
        }
    }
}
```

### 日本語スキル補正・KO判定

```rust
// src/corrections/japanese_skill.rs

/// 日本語スキルENUM: ["不要", "N5", "N4", "N3", "N2", "N1", "ネイティブ"]
/// ⚠️ 既存JSと同じ定義
pub fn correct_japanese_skill(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() { return None; }

    let valid = ["不要", "N5", "N4", "N3", "N2", "N1", "ネイティブ"];
    if valid.contains(&trimmed) { return Some(trimmed.to_string()); }

    let upper = trimmed.to_uppercase();

    // JLPTレベル判定
    if upper.contains("N1") || trimmed.contains("ビジネス") {
        return Some("N1".to_string());
    }
    if upper.contains("N2") { return Some("N2".to_string()); }
    if upper.contains("N3") { return Some("N3".to_string()); }
    if upper.contains("N4") { return Some("N4".to_string()); }
    if upper.contains("N5") { return Some("N5".to_string()); }
    if trimmed.contains("ネイティブ") || trimmed.contains("母語") || trimmed.contains("母国語") {
        return Some("ネイティブ".to_string());
    }
    if trimmed.contains("不要") || trimmed.contains("不問") {
        return Some("不要".to_string());
    }

    None
}

/// 日本語スキルレベルの順序比較（KO判定用）
/// 案件要求レベル <= 人材レベル なら合格
fn japanese_skill_level(skill: &str) -> Option<u8> {
    match skill.trim() {
        "不要" => Some(0),
        "N5" => Some(1),
        "N4" => Some(2),
        "N3" => Some(3),
        "N2" => Some(4),
        "N1" => Some(5),
        "ネイティブ" => Some(6),
        _ => None,  // 不明
    }
}

/// 日本語スキルKO判定
/// project_required: 案件が要求する日本語レベル
/// talent_level: 人材が持つ日本語レベル
///
/// 戻り値:
///   - Some(true)  = KO（人材レベルが要求未満）
///   - Some(false) = 合格
///   - None        = 情報不足（KOにはしない、review/LLMに回す）
///
/// ⚠️ 設計方針: 不足が確定した時だけKO、情報欠損は Tier/品質スコアで fallback
pub fn is_japanese_ko(project_required: Option<&str>, talent_level: Option<&str>) -> Option<bool> {
    let req = project_required.and_then(japanese_skill_level)?;
    if req == 0 { return Some(false); }  // 不要なら常に合格

    let tal = talent_level.and_then(japanese_skill_level)?;
    Some(tal < req)  // 人材レベルが要求未満ならKO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_japanese_ko() {
        // 不要なら常に合格
        assert_eq!(is_japanese_ko(Some("不要"), Some("N5")), Some(false));
        assert_eq!(is_japanese_ko(Some("不要"), None), Some(false));

        // N2要求、N1持ち → 合格
        assert_eq!(is_japanese_ko(Some("N2"), Some("N1")), Some(false));
        assert_eq!(is_japanese_ko(Some("N2"), Some("ネイティブ")), Some(false));

        // N2要求、N3持ち → KO
        assert_eq!(is_japanese_ko(Some("N2"), Some("N3")), Some(true));
        assert_eq!(is_japanese_ko(Some("N1"), Some("N2")), Some(true));

        // 情報不足 → None（KOにしない）
        assert_eq!(is_japanese_ko(Some("N2"), None), None);
        assert_eq!(is_japanese_ko(None, Some("N2")), None);
        assert_eq!(is_japanese_ko(Some("N2"), Some("unknown")), None);
    }
}
```

**⚠️ KO判定の呼び出し側での運用ルール**:
- `Some(true)` → KO理由コードを付けて落とす
- `Some(false)` → 合格、次の判定へ
- `None` → 情報不足理由コードを付けて review/LLM に回す（**KOにはしない**）

### 契約形態・性別補正

```rust
// src/corrections/contract_type.rs

/// 人材契約形態ENUM: ["正社員", "契約社員", "直個人"]
pub fn correct_talent_contract_type(input: &str, is_primary: bool) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return if is_primary { Some("直個人".to_string()) } else { None };
    }

    let valid = ["正社員", "契約社員", "直個人"];
    if valid.contains(&trimmed) { return Some(trimmed.to_string()); }

    if trimmed.contains("正社員") { return Some("正社員".to_string()); }
    if trimmed.contains("契約") { return Some("契約社員".to_string()); }
    if trimmed.contains("個人") || trimmed.contains("フリー") {
        return Some("直個人".to_string());
    }

    if is_primary { Some("直個人".to_string()) } else { None }
}

/// 性別ENUM: ["男性", "女性", "その他/無回答"]
pub fn correct_gender(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() { return None; }

    let valid = ["男性", "女性", "その他/無回答"];
    if valid.contains(&trimmed) { return Some(trimmed.to_string()); }

    if trimmed.contains('男') { return Some("男性".to_string()); }
    if trimmed.contains('女') { return Some("女性".to_string()); }

    Some("その他/無回答".to_string())
}
```

### 技術区分（tech_kubun）推論

```rust
// src/corrections/tech_kubun.rs

/// 技術区分ENUM: ["生成AI関連", "人気技術", "レガシー"]
/// ⚠️ MVP方針: 保存するが判定には使わない（参考情報として表示用）
/// Phase 2以降で優先度スコアリングに活用可能
///
/// 用途:
/// - UI表示: 案件一覧で「🔥生成AI」「📈人気技術」「📦レガシー」等のバッジ表示
/// - 将来のスコアリング: tech_kubun="生成AI関連" の案件に対し、AI/MLスキル保持者を優先
///
/// 注意: sponto-platformでは correctTechKubun() でスキルキーワードから自動推論している
///
/// ⚠️ 優先順位（排他的マッチ、先勝ち）:
///   1. 生成AI関連 - 最優先（GPT/LLM/Claude等を含む場合）
///   2. 人気技術   - 次点（クラウド/モダン技術を含む場合）
///   3. レガシー   - 最後（COBOL/メインフレーム等のみの場合）
///   4. None       - いずれにも該当しない場合
pub fn infer_tech_kubun(skills: &[String]) -> Option<String> {
    let all_skills = skills.join(" ").to_lowercase();

    // 生成AI関連キーワード（優先度高）
    let generative_ai_keywords = [
        "生成ai", "generative ai", "chatgpt", "gpt", "llm",
        "claude", "gemini", "openai", "langchain", "大規模言語モデル",
        "rag", "fine-tuning", "プロンプト", "prompt engineering"
    ];
    if generative_ai_keywords.iter().any(|k| all_skills.contains(k)) {
        return Some("生成AI関連".to_string());
    }

    // 人気技術キーワード
    // ⚠️ sponto-platform enum_corrections.js と同期すること
    let popular_keywords = [
        "ai", "aws", "gcp", "azure", "ml", "機械学習", "kubernetes", "k8s",
        "docker", "terraform", "react", "vue", "typescript", "go", "rust",
        "python", "データサイエンス", "クラウド"
    ];
    if popular_keywords.iter().any(|k| all_skills.contains(k)) {
        return Some("人気技術".to_string());
    }

    // レガシー技術キーワード
    // ⚠️ sponto-platform enum_corrections.js と同期すること
    let legacy_keywords = [
        "cobol", "vb", "visual basic", "mainframe", "メインフレーム", "汎用機",
        "as400", "rpg", "pl/i", "fortran", "delphi"
    ];
    if legacy_keywords.iter().any(|k| all_skills.contains(k)) {
        return Some("レガシー".to_string());
    }

    None  // 分類不能
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_tech_kubun() {
        assert_eq!(infer_tech_kubun(&["ChatGPT".to_string(), "Python".to_string()]), Some("生成AI関連".to_string()));
        assert_eq!(infer_tech_kubun(&["AWS".to_string(), "Docker".to_string()]), Some("人気技術".to_string()));
        assert_eq!(infer_tech_kubun(&["COBOL".to_string(), "AS400".to_string()]), Some("レガシー".to_string()));
        assert_eq!(infer_tech_kubun(&["Excel".to_string()]), None);
    }
}
```

**MVP方針**:
- `tech_kubun` は `projects_enum` / `talents_enum` に保存する
- **判定やスコアリングには使わない**（参考情報としてのみ）
- Phase 2以降で「生成AI案件×AI/MLスキル保持者」マッチングの優先度調整に活用可能

---

## 💰 単価計算ロジック（Phase 2 以降）

**⚠️ MVP 優先度: 低（Later）**

| 項目 | MVP | Phase 2 | 理由 |
|------|-----|---------|------|
| 人材単価計算 | ❌ | ✅ | マッチングには直接影響しない |
| 案件単価計算 | ❌ | ✅ | 抽出した単価をそのまま使用 |
| 単価推定ロジック | ❌ | ✅ | 精度より先にフロー確立が優先 |

**MVP での単価処理**:
- メールから抽出した `monthly_tanka_min`, `monthly_tanka_max` をそのまま保存
- 計算ロジックは適用しない（既存 LLM または抽出値をそのまま使用）
- マッチングの KO/スコアリングは抽出値で判定

**Phase 2 で有効化**:
- 抽出値が欠損/異常な場合のフォールバックとして計算ロジックを使用
- 推定単価 vs 実績単価の乖離分析で精度向上

**出典（Phase 2 実装時の参照）**:
- `sponto-platform/apps/business-api/app/services/calculation/strategies/bptalent_tanka.py`
- `sponto-platform/apps/business-api/app/services/calculation/strategies/project_tanka.py`

### 人材単価計算

```rust
// src/calculation/talent_tanka.rs

/// 人材の適正単価を計算
pub fn calculate_talent_tanka(
    experience_years: Option<i32>,
    skill_keywords: &[String],
) -> (i32, i32, String) {
    let mut base_tanka = 35.0_f64;
    let mut logic = vec!["Base: 35万円".to_string()];

    // 経験年数による加算
    if let Some(years) = experience_years {
        let years_capped = years.min(20);
        for i in 1..=years_capped {
            if i <= 5 { base_tanka += 5.0; }      // 1-5年: +5万/年
            else if i <= 10 { base_tanka += 4.0; } // 6-10年: +4万/年
            else { base_tanka += 3.0; }            // 11年+: +3万/年
        }
        logic.push(format!("経験{}年加算後: {:.0}万円", years_capped, base_tanka));
    }

    // スキルプレミアム（乗算）
    let mut premium_ratio = 1.0_f64;
    let all_skills = skill_keywords.join(" ").to_lowercase();

    if ["aws", "gcp", "azure"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.15;
        logic.push("Cloud +15%".to_string());
    }
    if ["pm", "pmo"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.15;
        logic.push("PM/PMO +15%".to_string());
    }
    if ["ai", "ml", "機械学習"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.10;
        logic.push("AI/ML +10%".to_string());
    }
    if ["python", "go", "typescript"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.10;
        logic.push("Modern Dev +10%".to_string());
    }

    // ⚠️ プレミアム上限: 最大1.5x（過大評価を防止）
    let capped_premium = premium_ratio.min(1.5);
    if premium_ratio > 1.5 {
        logic.push(format!("プレミアム上限適用: {:.2}x → 1.50x", premium_ratio));
    }

    let final_tanka = base_tanka * capped_premium;
    if capped_premium > 1.0 {
        logic.push(format!("プレミアム後({:.2}x): {:.0}万円", capped_premium, final_tanka));
    }

    let min_tanka = (final_tanka - 5.0).floor() as i32;
    let max_tanka = (final_tanka + 5.0).ceil() as i32;

    (min_tanka, max_tanka, logic.join("; "))
}
```

### 案件単価計算

```rust
// src/calculation/project_tanka.rs

/// 案件の適正単価を計算
pub fn calculate_project_tanka(
    min_experience_years: Option<i32>,
    required_skills: &[RequiredSkill], // {skill: String, years: Option<i32>}
) -> (i32, i32, String) {
    let mut logic = Vec::new();

    // 経験年数の決定（スキル経験年数の最大値 vs 要件経験年数）
    let years_from_skills = required_skills
        .iter()
        .filter_map(|s| s.years)
        .max()
        .unwrap_or(0);

    let min_exp = min_experience_years.unwrap_or(0);
    let years_for_calc = years_from_skills.max(min_exp);

    logic.push(format!(
        "計算用年数: {} (要件:{}, スキル最大:{})",
        years_for_calc, min_exp, years_from_skills
    ));

    // 経験年数カーブ（案件はベースが高い）
    let base_tanka = if years_for_calc > 0 {
        let mut tanka = 45.0_f64;
        for i in 1..=years_for_calc {
            if i <= 5 { tanka += 6.0; }      // 1-5年: +6万/年
            else if i <= 10 { tanka += 5.0; } // 6-10年: +5万/年
            else { tanka += 4.0; }            // 11年+: +4万/年
        }
        logic.push(format!("カーブ後: {:.0}万円", tanka));
        tanka
    } else {
        logic.push("経験年数なし、デフォルト45万円".to_string());
        45.0
    };

    // スキルプレミアム
    let mut premium_ratio = 1.0_f64;
    let all_skills = required_skills
        .iter()
        .map(|s| s.skill.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    if ["aws", "gcp", "azure"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.15;
        logic.push("Cloud +15%".to_string());
    }
    if ["pm", "pmo"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.15;
        logic.push("PM/PMO +15%".to_string());
    }
    if ["ai", "ml", "機械学習"].iter().any(|k| all_skills.contains(k)) {
        premium_ratio += 0.10;
        logic.push("AI/ML +10%".to_string());
    }

    // ⚠️ プレミアム上限: 最大1.5x（過大評価を防止）
    let capped_premium = premium_ratio.min(1.5);
    if premium_ratio > 1.5 {
        logic.push(format!("プレミアム上限適用: {:.2}x → 1.50x", premium_ratio));
    }

    let final_tanka = base_tanka * capped_premium;
    if capped_premium > 1.0 {
        logic.push(format!("プレミアム後: {:.0}万円", final_tanka));
    }

    // 案件は最低50万円保証
    let min_tanka = ((final_tanka - 5.0).floor() as i32).max(50);
    let max_tanka = ((final_tanka + 5.0).ceil() as i32).max(55);

    (min_tanka, max_tanka, logic.join("; "))
}

#[derive(Debug, Clone)]
pub struct RequiredSkill {
    pub skill: String,
    pub years: Option<i32>,
}
```

---

## 📁 実装ディレクトリ構造

```
sr-talent-extractor/
├── Cargo.toml
├── .env.example
├── README.md
├── src/
│   ├── main.rs                    # エントリーポイント
│   ├── lib.rs                     # ライブラリルート
│   ├── config.rs                  # 環境変数・設定
│   ├── db/
│   │   ├── mod.rs
│   │   ├── connection.rs          # tokio-postgres接続
│   │   ├── queries.rs             # SQL実行
│   │   └── introspection.rs       # ENUM型名自動検出
│   ├── extractors/
│   │   ├── mod.rs
│   │   ├── experience.rs          # 経験年数抽出
│   │   ├── price.rs               # 単価抽出
│   │   ├── station.rs             # 最寄駅抽出
│   │   ├── skills.rs              # スキル抽出
│   │   ├── date.rs                # 稼働開始日抽出
│   │   └── demographics.rs        # 年齢・性別抽出
│   ├── corrections/
│   │   ├── mod.rs
│   │   ├── todofuken.rs           # 都道府県補正
│   │   ├── area.rs                # エリア補正
│   │   ├── flow_depth.rs          # 商流補正
│   │   ├── contract_type.rs       # 契約形態補正
│   │   ├── remote_onsite.rs       # リモート形態補正
│   │   └── japanese_skill.rs      # 日本語スキル補正
│   ├── matching/
│   │   ├── mod.rs
│   │   ├── knockout.rs            # KO条件判定
│   │   ├── scoring.rs             # スコアリング
│   │   └── config.rs              # 閾値設定
│   └── models/
│       ├── mod.rs
│       ├── talent.rs              # 人材データ構造
│       ├── project.rs             # 案件データ構造（参照用）
│       └── match_result.rs        # マッチング結果
└── tests/
    ├── extractors_test.rs
    ├── corrections_test.rs
    └── matching_test.rs
```

---

### 変更履歴

| 日付 | 変更内容 |
|------|----------|
| 2025-12-16 | **sponto-platform整合性レビュー**: (1) SC-8追加: スコア合成レイヤ（business_rules + semantic + historical）の仕様契約化、MVPでは`total_score = business_rules_score`。(2) SC-9追加: prefilterと詳細スコアの重み差異を意図的設計として明文化（experience: 0.10 vs 0.15）。(3) English skill補正関数追加: `correct_english_skill()` + KO判定関数 `is_english_ko()`。(4) tech_kubun推論追加: MVPでは保存のみ（判定には使わない）、Phase 2で活用。(5) 相対日付比較ルール確定: `normalize_date()` で「即日」「1月上旬」「来月」等に対応。(6) 単価プレミアム上限追加: `premium_ratio.min(1.5)` で過大評価を防止 |
| 2025-12-15 | **ロジック完全移植**: 散らばった判定ロジックをRustコードとして仕様書に完全収録。追加セクション: (1)詳細スコアリングロジック（BusinessRulesEngine 5コンポーネント）、(2)事前フィルタリングロジック（EnhancedPreFilter + KO条件）、(3)ENUM補正ロジック完全版（都道府県47件、商流、リモート、日本語スキル、契約形態、性別）、(4)単価計算ロジック（人材/案件、経験年数カーブ+スキルプレミアム）。sponto-platform参照不要で実装可能に |
| 2025-12-15 | **sponto-platform全体探索**: 散らばった判定ロジックを洗い出し。新発見: `business_rules_engine.py`（詳細スコアリング）、`enhanced_prefilter.py`（事前フィルタ）、`skill_processor.py`（183スキルエイリアス）、`enum_corrections.js`（ENUM定義）、`bptalent_tanka.py`/`project_tanka.py`（単価計算）。移植元コード参照を12項目に拡張、スキルエイリアス辞書（Rustコード付き）追加 |
| 2025-12-17 | **#12 settlement_range形式統一**: `extract_settlement_range()` に妥当性チェック（80-240h範囲）、min/max自動スワップ、テストケース追加。SC-17 仕様契約 |
| 2025-12-17 | **#4 prefilter重み修正**: SC-11定義に合わせてprefilterのexperience重みを`0.15`→`0.10`に修正、skills重みを`0.40`→`0.45`に調整（合計1.0維持）。SC-11仕様契約を重み表形式に拡張 |
| 2025-12-17 | **#2 エリアENUM統一**: DDL（8分離形式）を enum_corrections.js（7結合形式）に統一。`AREA_ENUMS` + `TODOFUKEN_TO_AREA` マップ追加、`correct_work_area()` 拡張（旧形式→新形式変換）。SC-16 仕様契約 |
| 2025-12-17 | **#5 KoDecision全面適用**: NGキーワード・商流制限・稼働日・単価の全KOチェックを `HardKo`/`SoftKo`/`Pass` 3値化。`run_all_ko_checks()` 統合関数追加。SC-KO-UNIFIED 仕様契約 |
| 2025-12-17 | **#10 ENUM OnceLock→RwLock移行**: `EnumValues` struct + `EnumSource` enum 導入。静的フォールバック値追加、DB失敗時もpanic しない設計に。テスト時のリセット可能。SC-13 仕様契約 |
| 2025-12-17 | **#7 Prefecture correction統合**: PREFECTURE_MAP を正規化の第一段階として統合。2文字短縮形（北海, 神奈, 鹿児, 和歌）対応。SC-14 仕様契約 |
| 2025-12-17 | **#11 Canary ハッシュテストベクタ**: ゴールデンベクタ4件、境界テスト、n8n/Rust 両方のテストコード追加。SC-12 仕様契約 |
| 2025-12-17 | **#14 LlmResponse serde強化**: `LlmExtractedProject` 型付き構造体、カスタムデシリアライザ（`deserialize_tanka`, `deserialize_skills_array`）、バリデーション関数 `is_valid()`/`validation_issues()` 追加。SC-18 仕様契約 |
| 2025-12-17 | **#9 KoDecision 3値enum追加**: `HardKo`/`SoftKo`/`Pass` の3値設計で `required_skills` 空配列問題を解決。SC-KO-UNIFIED 仕様契約、Python strategy.py との統一ルール表、テストケース追加 |
| 2025-12-17 | **#1 日付正規化統一**: `normalize_date()` の「来月」計算を `Duration::days(30)` → カレンダーベース（翌月1日）に修正。`normalize_start_date()` と統一。テストベクタ追加 |
| 2025-12-15 | **コーディングLLM向け整備**: MVPスコープセクション追加（IN/OUT明確化）、移植元コード参照（sponto-platform内のJS/Pythonファイルパス）追加、具体的な入出力例（メールパース→抽出結果、ENUM補正、KO判定）追加、実装ディレクトリ構造確定 |
| 2025-12-15 | **直人材MVP版追加**: フィールド分類（talents_enum用）、DDL-2b `ses.talents_enum` 追加、人材抽出用正規表現ロジック（経験年数・最寄駅・年齢・単価等）、PDF抽出モジュール（Feature Flag制御）を追加 |
| 2025-12-15 | **ENUM型名自動検出**: `detect_enum_type_name()` 追加、型名ハードコードを廃止し `udt_name` を真実とする設計に変更。SC-4仕様契約更新。環境変数 `SR_TABLE_SCHEMA` / `SR_ENUM_TABLE` 追加 |
| 2025-12-15 | **Tier2 priority 段階化**: 0/1/2 で優先度を分離（50/20/10）。コメントで「処理順序であり品質スコアではない」意図を明記 |
| 2025-12-15 | **用語統一**: 「闘堕ち」「闇堕ち」→「破綻」に全置換。`DEFAULT_ENUM_SCHEMA` 定数化 |
| 2025-12-14 | **Phase 1前 Critical 6点対応**: (1) `email_received_at` DDL追加 (2) 並走シナリオA 影テーブル対応 (3) kpi_daily 定義ユレ修正 (4) interval SQL 型安全化 (5) `normalize_subject` 複数連続タグ対応 (6) ENUM読み込み `pg_namespace` JOIN追加 + `SR_ENUM_SCHEMA` 環境変数 |
| 2025-12-14 | **破綻対策追加**: `manual_review_reason` (なぜ手動レビューか), `reprocess_after` (再処理可能フラグ) を DDL に追加。Guardrail KPI に「スキル空配列率」追加。軽量ルールに第二入口 (言語/環境/技術行) 追加。設計思想セクション「破綻対策」を新設 |
| 2025-12-14 | **フレーミング修正**: 主目的を「LLMコスト削減」→「マッチング精度・説明可能性・継続学習」に変更。KPIを品質先行に並び替え、コスト削減を副次効果に降格 |
| 2025-12-13 | **429 retry_after を Queue に反映**: `LlmError::retry_after_hint()` 追加、`handle_job_error()` に `next_retry_after` パラメータ追加。429 の場合は API 指定の待機時間を `next_retry_at` に反映 |
| 2025-12-13 | **normalize_subject 空文字防止**: 関数契約として「空になるなら元に戻す」を追加。段階的 fallback（s2 → s1 → original）を実装、テスト更新 |
| 2025-12-13 | **Decision 関数シグネチャ統一**: `final_decision_with_env()` / `decide_from_quality_with_env()` ラッパーを追加。コード例を統一（policy 引数を内部ロード） |
| 2025-12-13 | **sr-common 設計原則修正**: 「型定義のみ」→「純粋関数（DB/HTTP に触れない）+ 共通型」に変更。decision.rs, normalize.rs の配置を正当化 |
| 2025-12-13 | **Canary ハッシュ統一**: n8n/Rust のハッシュ計算を「先頭1バイト方式」で統一、仕様契約を明記、`canary_target` フラグの仕様追加 |
| 2025-12-13 | **列名差分吸収を修正**: COALESCE アプローチを廃止（構文解析エラー）→ 環境変数 `SR_ANKEN_EMAILS_BODY_COL` + 起動時 information_schema 自動検出のハイブリッドに変更 |
| 2025-12-13 | **KPI timestamp 保全**: completed 時に `processing_started_at` を残すよう修正（queue_wait 計算に必要）。pending に戻す場合のみ NULL 化 |
| 2025-12-13 | **failed 残骸排除**: DDL/Recovery/LLM API表/コード例から `failed` の概念を完全排除、`update_job_failed()` → `handle_job_error()` に統一 |
| 2025-12-13 | **High 10 改訂**: start_date を DATE 型 + 正規化ロジックに変更。3カラム構成 (`start_date_raw` TEXT / `start_date` DATE / `start_date_precision` VARCHAR)、`normalize_start_date()` モジュール追加、相対日付は received_at 基準で解決 |
| 2025-12-13 | **Critical 修正 (5件)**: failed ステータス廃止→3状態設計、required_skills_keywords 空の仕様契約、normalize_subject regex修正+hex変換、LlmCallResult latency計測、ENUM補正失敗→NULL fallback |
| 2025-12-13 | **High 修正 (5件)**: body_text/source_text 列名対応表、calculate_subject_hash hex固定 (Critical 3に含む)、DecisionPolicy struct + sr-common 統合、ENUM allowed set 契約化 (`_ALLOWED` 変数名)、start_date_raw→start_date 変換仕様 + FinalResult 構造体追加 |
| 2025-01-15 | 初版作成（実装GO） |

---

## クイックスタート（5分で読める要約）

### 本提案の主目的

**直人材 × BP案件** のマッチング精度と一貫性を向上させ、判断根拠を蓄積できる仕組みに移行する。

これにより、人手とLLMの両方で改善が積み上がる運用ループを構築する。

---

### ⚡ ビジネスフロー概要（なぜこのシステムが必要か）

```
【SPONTOの業務フロー】

┌─────────────────────────────────────────────────────────────────────┐
│ マーケチーム: 求人媒体で直人材を募集 → 書類選考 → 登録 → 配信依頼  │
│                        ↓                                           │
│ 購買部門:    配信設定（配信ソフト使用）                             │
│                        ↓                                           │
│ 営業担当:    ★ここがボトルネック ★                                │
│              ・BP各社から「案件紹介」メールを受信                   │
│              ・直人材に意向確認（電話）                             │
│              ・エントリー → 面談 → オファー → 契約                  │
└─────────────────────────────────────────────────────────────────────┘

【重要】配信側（直人材を外部に紹介）はBP側がマッチングする → 本システムの対象外
【重要】本システムの対象 = BP各社から来た「案件紹介」と「直人材」のマッチング
```

**なぜ営業がボトルネックなのか**:
- マーケチーム: ステップ数は少ないが、各媒体への入力作業が多い（労働集約）
- 営業担当: ステップ数が多く、**高度な交渉スキルが必要**（単価交渉・増額交渉等）
- 問題: 営業にはスキルがあるのに、**マッチング候補が少なくて件数が取れない**

**本システムの価値**:
- BP各社から大量に来る「案件紹介」メールを自動パース
- 直人材とのマッチングを自動スコアリング
- 営業担当が「見るべき案件」を優先度付きで提示
- → 営業の時間を「交渉」に集中させる

**差別化ポイント**（Loc指摘）:
- 直人材は複数のエージェント（SPONTO、DUDA、レバテック等）と並行してやり取りする
- 「この営業担当がやりやすいから、この会社と付き合いたい」という差別化が可能
- マッチング精度が上がる → 営業の提案品質が上がる → 直人材から選ばれる会社になる

---

### 🌟 北極星ビジョン（最終ゴール）

```
【短期目標 - MVP】
  55%+ をルールベースで処理
  LLM は補助役（曖昧さの解決に集中）

【中期目標 - Phase 2+】
  80%+ をルールベースで処理
  学習ループで継続的に精度向上

【長期目標 - 北極星】
  決定論的（deterministic）アルゴリズムのみでマッチング可能
  LLMが完全に不要になる状態（難しいが、目指す価値がある方向）
```

**なぜこの方向を目指すか**:
- LLMの精度向上を待つ必要がなくなる
- コストが予測可能・制御可能になる
- 判断根拠が100%説明可能になる
- システムが「学習した知識」として資産化される

---

### マッチング対象の明確化（MVP）

| 対象 | MVP | Phase 2+ |
|------|-----|----------|
| **直人材 × BP案件** | ✅ 対象 | - |
| BP人材 × BP案件 | - | 検討 |
| 直人材配信（外部へ） | 対象外（BP側がマッチング） | - |

### 目指す姿

```
現状の課題
├─ 精度が"見かけ上"良くても、原因が追えない・改善できない
├─ ルールや知見が蓄積されず、モデル任せになりがち
└─ 量が増えるほど、運用と品質が崩れる

目指す状態
├─ 判断の根拠（reason / features）をDBに残せる
├─ 人が直せる・LLMが補助できる・両方で学習が回る
└─ 結果として網目が細かくなっていく（改善が積み上がる）
```

### 設計思想（役割分担）

| 担当 | 役割 |
|------|------|
| **Rust（ルールベース）** | 明確なパターンの抽出、説明可能な特徴量生成、DB蓄積 |
| **LLM** | 曖昧さの解釈、正規化、例外処理への集中 |
| **人（レビュー）** | 誤りの修正 → 理由ラベル → ルール改善へ反映 |

**"LLMを増やしてなんとかする"から "LLMは成長ループの一部"へ**

### なぜRust化が精度改善につながるか（Loc指摘 2025-12-15）

> 「今のパースは、プロンプトによってDeepSeekに情報を抽出してもらっている。だから依然として間違ってる可能性が残る。改善できるなら改善して欲しい」

**現状の課題（LLM依存パース）**:
- プロンプトの書き方で結果が変わる（非決定論的）
- 同じ入力でも異なる出力が返る可能性
- 「なぜ間違えたか」が追えない（ブラックボックス）
- モデルバージョンアップで挙動が変わるリスク

**Rust化による改善機会**:
| 観点 | LLMパース | Rustルールベース |
|------|----------|-----------------|
| **再現性** | 非決定論的 | 決定論的（同じ入力→同じ出力） |
| **デバッグ** | 困難 | 容易（正規表現・条件分岐を追跡可能） |
| **改善サイクル** | プロンプト調整→全体に影響 | ルール追加→局所的に改善 |
| **エッジケース対応** | プロンプト肥大化 | 専用ルール追加 |
| **バージョン管理** | モデル依存 | コードで完全制御 |

**具体例**: 「単価 70-80万」の抽出
- LLM: 「70万」だけ返したり「80万」だけ返したり不安定
- Rust: 正規表現 `(\d+)[〜\-～](\d+)万` で確実に両方取得

### 目標KPI

| 指標 | Gate（最低限） | Target（目標） | 意味 |
|------|---------------|---------------|------|
| **Tier1抽出成功率** | ≥75% | ≥85% | ルールベースで主要フィールドを抽出できる割合 |
| **説明可能率** | - | ≥80% | 理由コード付きで説明できた割合 |
| **手動レビュー率** | 前週比+10%以内 | - | 品質劣化の早期警告 |
| **LLM集中率** | - | ≤50% | LLMは高次判断に集中させる（副次効果としてコスト最適化） |

### システム構成（3バイナリ）

| バイナリ | 役割 | 起動方式 |
|----------|------|----------|
| `sr-extractor` | メールからTier1/2抽出 → キューに登録 | systemd timer（5分ごと） |
| `sr-llm-worker` | 最終判定 + 必須フィールド補完 + projects_enum INSERT | 常駐 |
| `sr-queue-recovery` | Stuck Job を pending に戻す | systemd timer（10分ごと） |

### 緊急停止スイッチ（Operational Toggles）

| トグル | 効果 |
|--------|------|
| `SR_DISABLE_EXTRACTOR=1` | 抽出サービス完全停止 |
| `SR_FORCE_LLM=1` | 常にLLM呼び出し |
| `SR_FORCE_MANUAL_REVIEW=1` | 全て手動レビュー |

### 関連セクション

- 仕様契約一覧 → [仕様契約（Spec Contracts）](#仕様契約spec-contracts)
- DDL定義一覧 → [DDL定義（まとめ）](#ddl定義まとめ)
- 実装前チェック → [Phase 1 開始前の確定チェック](#-phase-1-開始前の確定チェック実装goの前提条件)

---

### 環境変数リファレンス（全一覧）

本ドキュメントで使用されるすべての環境変数の一覧です。

#### 🚨 緊急停止スイッチ（Operational Toggles）

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `SR_DISABLE_EXTRACTOR` | `0` | `1` で sr-extractor を完全停止。既存 Python LLM に全処理を戻す |
| `SR_FORCE_LLM` | `0` | `1` で Tier 判定を無視し、常に LLM を呼び出す（API費削減 0%、品質維持） |
| `SR_FORCE_MANUAL_REVIEW` | `0` | `1` で全レコードを手動レビュー対象にする（INSERT エラー回避） |

#### 🐤 Canary デプロイ

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `SR_CANARY_PERCENT` | `0` | Rust 抽出へ流すトラフィック割合（0〜100%）。`message_id` hash で決定 |
| `SR_PARALLEL_RUN_MODE` | `0` | `1` で並走検証モード（上書きせず比較のみ） |

#### 🗄️ データベース設定

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `DATABASE_URL` | (必須) | PostgreSQL 接続文字列 |
| `SR_TABLE_SCHEMA` | `ses` | ENUM 読み込み対象のスキーマ名 |
| `SR_ENUM_TABLE` | `projects_enum` | ENUM 値の参照テーブル名 |
| `SR_ENUM_SCHEMA` | (SR_TABLE_SCHEMA) | ENUM 型定義のスキーマ名（udt_name 検索用） |
| `SR_ENUM_TYPE_{column}` | (自動検出) | 特定カラムの ENUM 型名を上書き（例: `SR_ENUM_TYPE_work_todofuken`） |
| `SR_ANKEN_EMAILS_BODY_COL` | (自動検出) | anken_emails の本文カラム名（`body_text` or `source_text`） |

#### 🎯 Tier 品質閾値

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `SR_TIER1_REQUIRED` | `4` | 必須の Tier1 フィールド数（4 = 全て必須） |
| `SR_TIER2_MIN` | `2` | 最低限必要な Tier2 フィールド数 |

#### 🔧 スキルマッチング

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `SR_SKILL_MATCH_THRESHOLD` | `0.3` | スキルマッチ最低閾値（0.0〜1.0、30% = 0.3） |

#### 🤖 LLM API 接続

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `LLM_ENDPOINT` | (必須) | Python LLM Wrapper の URL（例: `http://localhost:8000/api/v1/extract`） |
| `LLM_TOKEN` | (必須) | 認証トークン（Bearer 認証用） |
| `LLM_MODEL` | `claude-3-5-sonnet-20241022` | 使用する LLM モデル名 |
| `LLM_TIMEOUT_SECONDS` | `120` | LLM リクエストのタイムアウト秒数 |
| `LLM_MAX_RETRIES` | `3` | LLM 失敗時の最大リトライ回数 |

#### 🧪 機能フラグ（Phase 2）

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `SR_ENABLE_PDF_EXTRACT` | `0` | `1` で PDF 添付ファイル抽出を有効化（Phase 2） |

#### 📋 設定例（systemd）

```ini
# /etc/systemd/system/sr-extractor.service
[Service]
Environment="DATABASE_URL=postgres://user:pass@localhost:5432/ses"
Environment="SR_TABLE_SCHEMA=ses"
Environment="SR_CANARY_PERCENT=10"
# 緊急時のみ有効化
# Environment="SR_DISABLE_EXTRACTOR=1"
# Environment="SR_FORCE_LLM=1"
# Environment="SR_FORCE_MANUAL_REVIEW=1"

# /etc/systemd/system/sr-llm-worker.service
[Service]
Environment="DATABASE_URL=postgres://user:pass@localhost:5432/ses"
Environment="LLM_ENDPOINT=http://localhost:8000/api/v1/extract"
Environment="LLM_TOKEN=your-api-token"
Environment="LLM_MODEL=claude-3-5-sonnet-20241022"
```

---

## 目次

1. [クイックスタート](#クイックスタート5分で読める要約)
2. [MVP Scope Definition](#mvp-scope-definition)
3. [設計方針](#設計方針)
4. [フィールド分類と抽出難易度](#フィールド分類と抽出難易度)
5. [保守的判定ルール詳細](#保守的判定ルール詳細)
6. [Rust実装設計（3-binary構成）](#rust実装設計3-binary構成)
7. [KPI定義（Outcome/Guardrail/Efficiency）](#kpi定義outcomeguardrailefficiency)
8. [仕様契約（Spec Contracts）](#仕様契約spec-contracts) ← NEW
9. [DDL定義（まとめ）](#ddl定義まとめ) ← NEW
10. [🛑 Phase 1 開始前の確定チェック](#-phase-1-開始前の確定チェック実装goの前提条件)
11. [Dangerous Assumptions](#dangerous-assumptions)
12. [🚨 Operational Toggles（運用トグル・緊急停止スイッチ）](#-operational-toggles運用トグル緊急停止スイッチ)
13. [段階的ロールアウト計画](#段階的ロールアウト計画)

---

## MVP Scope Definition

### MVP Minimum Line（必須機能のみ）

**含むもの**:
```
✅ Gmail受信 → body_text保存（既存n8n維持）
✅ Rust抽出サービス（sr-extractor）
✅ LLM Workerサービス（sr-llm-worker）
✅ PostgreSQL キュー（ses.extraction_queue）
✅ 重複排除（message_id + subject_hash）
✅ 判定ロジック（Tier1=4/4 AND Tier2≥1/2）
```

**含まないもの（Later拡張）**:
```
❌ 外部キュー（Redis/RabbitMQ）
❌ SimHash dedup（暫定は subject_hash のみ）
❌ Synonym辞書（厳密regex + LLMフォールバックで対応）
❌ required_skills の高度ルール抽出（同義語辞書・正規化・重み付け・経験年数推定 等）
   ※MVPは sr-llm-worker の超軽量ルール + 不足時のみLLM に限定
❌ skill_vector生成（既存LLMロジック維持）
❌ パフォーマンス最適化（並列処理等）
❌ リアルタイムダッシュボード
```

**Note**:
- DB内キュー（PostgreSQL `ses.extraction_queue`）で非同期処理を実現。外部キュー（Redis等）は Later。
- MVP では `required_skills` を sr-llm-worker の"超軽量ルール"で最低限抽出し、空/不足の場合のみ LLM（または既存ロジック）で補完して INSERT する。
- 同義語辞書・正規化・重み付け等の"高度化"は Later。

**理由**:
- 最小構成で動作検証
- スパゲッティ化を防ぐ
- 段階的に機能追加

---

### コンポーネント構成（MVP）

```
┌─────────────────────────────────────────┐
│ Gmail → n8n (既存)                      │
│   └─ ses.anken_emails (body_text保存)   │
└─────────────────────────────────────────┘
              ↓
┌─────────────────────────────────────────┐
│ sr-extractor (Rust binary - バッチ)     │
│   ├─ Tier1/2抽出（regex）                │
│   ├─ 判定ロジック                        │
│   ├─ systemd timer: 5分ごと             │
│   └─ → ses.extraction_queue に必ず積む   │
└─────────────────────────────────────────┘
              ↓
┌─────────────────────────────────────────┐
│ ses.extraction_queue (PostgreSQL)       │
│   message_id, status, partial_fields    │
└─────────────────────────────────────────┘
         ↓               ↑
┌────────────────┐  ┌────────────────────┐
│ sr-llm-worker  │  │ sr-queue-recovery  │
│ (常駐)          │  │ (バッチ・保険)      │
├─ ポーリング    │  ├─ Stuck Job 回復   │
├─ 3段階判定     │  ├─ timer: 10分ごと │
└─ projects_enum │  └─ status=pending  │
└────────────────┘  └────────────────────┘
```

**バイナリ数**: **3個のみ**（sr-extractor, sr-llm-worker, sr-queue-recovery）

**起動方式（systemd timer ハイブリッド）**:
- **sr-extractor**: systemd timer（5分ごと）= 常駐不要・事故りにくい
- **sr-llm-worker**: systemd service（常駐）= 遅延が価値を壊すため常駐必須
- **sr-queue-recovery**: systemd timer（10分ごと）= 常駐不要・保険として動作

---

## 🔴 保守的設計の核心

**最重要な設計判断**:

```
sr-extractor は projects_enum に直接 INSERT しない。
全てのメールを ses.extraction_queue に積み、
sr-llm-worker が NOT NULL 制約を必ず満たしてから INSERT する。
```

**理由**:
- `projects_enum` は `project_name NOT NULL`, `required_skills JSONB NOT NULL` 等の制約がある。
- Rust only では満たせない場合があるため、worker が責任を持って補完する。
- これにより DB 整合性を維持し、マッチング前提（`required_skills` 必須）を守る。

**役割分担**:
- **sr-extractor**: 抽出 + 品質スコア + priority 付与 + **LLM 推奨判定**（見積もり）
- **sr-llm-worker**: LLM 呼び出し**確定判定** + 必須フィールド補完 + projects_enum INSERT

worker が最終決定権を持つことで、`既存ロジック利用 / 軽量ルール補完 / LLM 呼び出し` を柔軟に最適化できる。

---

## 設計方針

### 基本コンセプト

```
説明可能 × 継続学習 × 安定運用

├─ ルールベースで「なぜそう判定したか」を残す
├─ 人の修正 → 理由ラベル → ルール改善のループ
├─ LLMは曖昧さの解決に集中（高次判断に特化）
└─ 量が増えても品質が崩れない構造を作る
```

### なぜこのアプローチか

| 観点 | 現状の課題 | 本設計の狙い |
|------|-----------|-------------|
| **精度の根拠** | LLM任せで「なぜ」が追えない | 特徴量・理由コードをDBに蓄積 |
| **改善の積み上げ** | 知見が属人化・揮発 | レビュー結果がルール強化に反映 |
| **スケール耐性** | 量が増えると運用が崩れる | ルールベースで処理量に強い構造 |
| **リスク管理** | ブラックボックスで問題特定困難 | 段階的移行 + 即時ロールバック |

### 副次効果：LLM呼び出しの最適化

ルールベースで説明可能な処理を増やすことで、**結果としてLLM呼び出しが減る**。
これはコスト削減が目的ではなく、**LLMを「本当に必要な判断」に集中させる**ための設計。

```
目指す分担:
  Rust（ルールベース） → 明確なパターン、構造化データ抽出
  LLM                 → 曖昧な表現の解釈、例外処理、正規化
  人間（レビュー）     → 誤りの修正 + 理由ラベリング → ループへ
```

### 🔄 学習ループの具体的メカニズム（Loc指摘への回答）

> 「判断の根拠を使って人が直したり、あるいはLLMがENUMのDBを読み込めるパイプを作ることで、人とAI両方が学習を回して」

**Step 1: 判断根拠の蓄積**
```sql
-- extraction_queue.decision_reason に理由を保存
"LLM recommended: Tier1 incomplete 2/4 fields (missing: work_todofuken, start_date_raw)"
"Rust completed: Tier1 4/4, Tier2 2/2"
"Manual review: ENUM correction failed for remote_onsite='週3リモート'"
```

**Step 2: 人間レビュー → 理由ラベリング**
```
営業担当が手動レビュー時に記録:
  - extraction_queue.manual_review_reason = "remote_onsite の正規表現に '週N' パターンが不足"
  - extraction_queue.corrected_values = {"remote_onsite": "リモート併用"}
```

**Step 3: ルール改善への反映**
```rust
// Before: 週N リモートを認識できない
if trimmed.contains("フルリモート") { return "フルリモート"; }

// After: 人間レビューから学習して追加
if trimmed.contains("フルリモート") || trimmed.contains("週5リモート") { return "フルリモート"; }
if RE_HYBRID.is_match(trimmed) { return "リモート併用"; }  // 週1〜4リモート
```

**Step 4: LLMへのフィードバック（Phase 2）**
```
将来的に:
  - 人間が修正した「正解データ」を few-shot examples として LLM プロンプトに追加
  - ENUM補正後の値を「正規化された形式」としてLLMに学習させる
  - outcome_tag（成約/辞退）データを使って、マッチング閾値を最適化
```

**蓄積されるデータと活用先**:
| データ | 保存先 | MVP活用 | Phase 2+活用 |
|--------|--------|---------|--------------|
| `decision_reason` | extraction_queue | デバッグ・監査 | パターン分析 |
| `manual_review_reason` | extraction_queue | - | ルール改善ヒント |
| `corrected_values` | extraction_queue | - | 正解データセット |
| `outcome_tag` | projects_enum | - | 閾値最適化 |

---

## フィールド分類と抽出難易度

### Tier 1: Critical Fields（絶対必須・Rustで確実に取れる）

これらが**全て（4/4）**取れないとLLM必須。

| フィールド | 説明 | Rust正規表現難易度 | 抽出成功率 |
|-----------|------|------------------|-----------|
| `monthly_tanka_min` | 月単価下限（万円） | ⭐ 易 | **Gate: ≥85%** (MVP継続条件) <br> **Stretch: ≥95%** (改善目標) <br> **Baseline: 未測定** (Week 4-5で取得) |
| `monthly_tanka_max` | 月単価上限（万円） | ⭐ 易 | **Gate: ≥85%** <br> **Stretch: ≥95%** <br> **Baseline: 未測定** |
| `start_date_raw` | 開始日（文字列そのまま） | ⭐ 易 | **Gate: ≥85%** <br> **Stretch: ≥95%** <br> **Baseline: 未測定** |
| `work_todofuken` | 都道府県 | ⭐⭐ 中 | **Gate: ≥75%** <br> **Stretch: ≥85%** <br> **Baseline: 未測定** |

**判定条件**: Tier1が **4/4 (100%)** 取れる必要がある

**測定計画**: Phase 2（Week 4-5）で過去100件メールから Baseline 取得。Gate 未達の場合は Tier1条件を 4/4→3/4 に緩和。

**Note**:
- `start_date_raw`: 「即日」「12月中」「1月上旬」「応相談」等、メール内の表現をそのまま抽出すればOK（ISO正規化は worker が行う）。
- 表現ゆらぎ（「ASAP」「随時」「年明け」等）に対応するため、文字列として取れれば成功とする。

---

### Tier 2: Important Fields（重要だがバラつきあり）

これらが**1個以上**取れると望ましい。

| フィールド | 説明 | Rust正規表現難易度 | 抽出成功率 |
|-----------|------|------------------|-----------|
| `remote_onsite` | リモート形態 | ⭐⭐ 中 | **Gate: ≥60%** <br> **Stretch: ≥80%** <br> **Baseline: 未測定** |
| `flow_dept` | 商流深さ | ⭐⭐ 中 | **Gate: ≥50%** <br> **Stretch: ≥70%** <br> **Baseline: 未測定** |

**判定条件**: Tier2が **1/2以上** 取れると良い（必須ではない）

**重要性**:
- `remote_onsite`: キーワードマッチで取れる。表現多様だが辞書拡張で対応。
- `flow_dept`: "二次請け"等のパターンマッチ。KO条件（商流深すぎ）の判定に使用。

**重複排除について**:
- **Gate（MVP最低限）**: `message_id`（確定）
- **Baseline（暫定 v0.1）**: `subject_hash` = `sha256(normalize(subject))[:16]`
  - `normalize(subject)` 処理内容:
    1. 【】《》() 等の装飾記号を除去
    2. RE:, FW:, Fwd: 等のプレフィックスを除去（大文字小文字無視）
    3. 連続空白を単一スペースに正規化
    4. 前後の空白をトリム
    5. 小文字化（オプション：日本語混在時は不要）
  - 例: `"【案件】RE: Java開発  案件"` → `"Java開発 案件"` → `sha256(...)`[:16]
  - **⚠️ 実装は sr-common/src/normalize.rs に配置**（重複実装を防ぐ）
    ```rust
    // sr-common/src/normalize.rs から import
    use sr_common::normalize::normalize_subject;

    let hash = calculate_subject_hash(&subject);
    // → normalize_subject() → sha256()[:16]
    ```
- **Risk**: `subject_hash` だけだと件名が微妙に違う同一案件を取りこぼす可能性あり。
- **Mitigation**: Phase 5 で SimHash（または bucket 化）を追加して精度向上。
- `project_name` は Tier3（後段で束ねる運用）に格下げ。

---

### Tier 3: Nice-to-Have Fields（Rust抽出対象外）

これらは判定ロジックに影響しない。worker が補完または Later 拡張で対応。

| フィールド | 説明 | MVP対応 |
|-----------|------|--------|
| `project_name` | 案件名 | 🔺 worker が補完（LLM/既存）、後段で束ねる運用 |
| `required_skills` | 必須スキル（JSONB） | 🔺 worker が超軽量ルールで抽出、空なら LLM/既存で補完<br>（高度ルール抽出は Later） |
| `end_date` | 終了日 | ❌ Later |
| `joi_bp_name` | 上位BP会社名 | ❌ Later |
| `outcome_tag` | 案件結果（enum） | 🔺 MVP では `unknown`、Later で活用 |
| `decline_reason_tag` | 辞退理由（optional） | 🔺 MVP では `NULL`、Later で活用 |

**Note**:
- `project_name` (TEXT) と `required_skills` (JSONB NOT NULL) は `projects_enum` の NOT NULL 制約のため、sr-llm-worker が必ず補完する。
- `required_skills` は MVP で超軽量ルール抽出を実装し、空/不足の場合のみ LLM または既存ロジックで補完。
- 同一案件の多重登録は後段（人手またはLLM）で束ねる運用でカバー。

**outcome_tag / decline_reason_tag（将来価値が高い）**:

**保存場所（MVP）**:
- `ses.extraction_queue` に JSONB として保存（`partial_fields.outcome_tag` / `partial_fields.decline_reason_tag`）
- `projects_enum` / `bp_talents_enum` には保存しない（カラムが存在しない）
- Later で専用カラムを追加するか、別テーブル（`ses.project_events`）に移行

**enum 定義**:
- `outcome_tag`: `unknown` / `progressed` / `declined_by_candidate` / `declined_by_partner` / `hired`
- `decline_reason_tag`: `role` / `price` / `work_style` / `timing` / `flow` / `unknown` / `NULL`

**MVP での扱い**:
- sr-extractor は `outcome_tag='unknown'`, `decline_reason_tag=NULL` を固定で設定
- Later で営業マンの肌感覚（T.M辞退のような事例）を構造化データとして残せる

**Later 拡張案**:
- Option A: `projects_enum` に `outcome_tag TEXT`, `decline_reason_tag TEXT` カラム追加
- Option B: `ses.project_events` テーブル新設（message_id FK、event_type、reason、created_at）

---

## フィールド分類と抽出難易度（直人材MVP版）

**対象**: メール本文（またはPDFから抽出されたテキスト）に含まれる候補者情報
**⚠️ 注意**: 上記「フィールド分類と抽出難易度」は案件（projects_enum）用。こちらは人材（talents_enum）用。

### Tier 1: Critical Fields（マッチング必須・Rustで確実に取れる）

これらが **4/4 (100%)** 取れないと自動マッチング不可（LLM補完 or 手動レビュー推奨）。

| フィールド | 説明 | Rust正規表現難易度 | 抽出対象例 |
|-----------|------|------------------|-----------|
| `possessed_skills_keywords` | 保有スキル (Keywords) | ⭐⭐ 中 | "経験言語：Java, Go" (正規化必須) |
| `min_experience_years` | 経験年数 | ⭐ 易 | "経験10年", "エンジニア歴: 5年" |
| `available_date_raw` | 稼働開始日 (原文) | ⭐⭐ 中 | "即日", "11月〜", "2025/01/01" |
| `desired_price_min` | 希望単価下限 | ⭐ 易 | "希望単価：80万〜", "70万円以上" |

**判定条件**: Tier1が **4/4** 取れる必要がある（Gate: ≥70%目標）

### Tier 2: Important Fields（重要だが欠損あり）

これらが取れるとマッチング精度が上がる。

| フィールド | 説明 | Rust正規表現難易度 | 抽出対象例 |
|-----------|------|------------------|-----------|
| `nearest_station` | 最寄駅 | ⭐⭐ 中 | "最寄駅：新宿", "山手線 渋谷駅" |
| `work_style` | 働き方 (enum) | ⭐⭐ 中 | "リモート希望", "週5出社" |
| `desired_price_max` | 希望単価上限 | ⭐ 易 | "〜100万", "MAX90万円" |

### Tier 3: Nice-to-Have（あれば保存・なくてもOK）

コンプライアンスリスクや表記揺れを考慮し、判定ロジックには影響させない。

| フィールド | 説明 | 処理方針 |
|-----------|------|--------|
| `talent_name` | 候補者名/イニシャル | 🔺 フォーマット揺れが激しいため、WorkerがLLMまたは件名から補完 |
| `age` | 年齢 | 🔺 必須ではない。「30代」等の表記揺れに対応 |
| `gender` | 性別 | 🔺 必須ではない。推測によるバイアスリスクを避けるためTier下げ |
| `summary_text` | 自己PR/要約 | 🔺 PDF全文またはLLMによる要約 |

**重複排除について（Soft Dedup）**:
- **方針**: MVPでは「取りこぼし」を防ぐため、ハッシュ一致での除外（Hard Dedup）は行わない。
- **実装**: `talent_fingerprint` カラムを用意し、`hash(normalized_skills_top3 + nearest_station)` 等を保存。
- 重複疑いは `decision_reason` にフラグを立てるのみとし、データは全て保存する。

### 人材情報抽出用 正規表現ロジック

```rust
// sr-extractor/src/extractors/talent_attributes.rs（直人材用）

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // ─────────────────────────────────────────────────────
    // 経験年数: "経験10年", "エンジニア歴: 5年", "5年以上"
    // ─────────────────────────────────────────────────────
    static ref EXP_YEARS_RE: Regex =
        Regex::new(r"(?:経験|実務経験|エンジニア歴|業務経験)[:：]?\s*(\d{1,2})\s*年").unwrap();
    static ref EXP_OVER_RE: Regex =
        Regex::new(r"(\d{1,2})\s*年以上").unwrap();

    // ─────────────────────────────────────────────────────
    // 最寄駅: "最寄駅：新宿" (改行まで、かつ長すぎない)
    // ⚠️ 貪欲マッチ防止: {1,50} で上限設定
    // ─────────────────────────────────────────────────────
    static ref STATION_RE: Regex =
        Regex::new(r"(?m)^(?:最寄(?:り)?駅|最寄駅)[:：]\s*([^\n\r]{1,50})").unwrap();

    // ─────────────────────────────────────────────────────
    // 年齢: "32歳", "30代" (Tier 3だが抽出は試みる)
    // ─────────────────────────────────────────────────────
    static ref AGE_RE: Regex = Regex::new(r"(\d{2})\s*歳").unwrap();
    static ref AGE_GENERATION_RE: Regex = Regex::new(r"(\d)0代").unwrap();

    // ─────────────────────────────────────────────────────
    // 性別: "性別：男性"
    // ─────────────────────────────────────────────────────
    static ref GENDER_RE: Regex =
        Regex::new(r"性別[:：]?\s*(男性|女性)").unwrap();

    // ─────────────────────────────────────────────────────
    // 希望単価: "希望単価：80万〜", "70万円以上", "〜100万"
    // ─────────────────────────────────────────────────────
    static ref PRICE_MIN_RE: Regex =
        Regex::new(r"(?:希望(?:単価)?|単価)[:：]?\s*(\d{2,3})\s*万").unwrap();
    static ref PRICE_MAX_RE: Regex =
        Regex::new(r"(?:〜|～|MAX|max|上限)[:：]?\s*(\d{2,3})\s*万").unwrap();

    // ─────────────────────────────────────────────────────
    // 精算幅: "精算：140-180h", "140h〜180h", "140〜180時間"
    // ⚠️ sponto-platform との整合: settlement_range VARCHAR(20)
    // ─────────────────────────────────────────────────────
    static ref SETTLEMENT_RE: Regex =
        Regex::new(r"(?:精算(?:幅)?|稼働時間)[:：]?\s*(\d{2,3})\s*[-〜～]\s*(\d{2,3})\s*[hH時間]?").unwrap();

    // ─────────────────────────────────────────────────────
    // 稼働開始日: "即日", "11月〜", "2025/01/01"
    // ─────────────────────────────────────────────────────
    static ref AVAILABLE_DATE_RE: Regex =
        Regex::new(r"(?:稼働(?:開始)?(?:日|可能日)?|参画(?:可能日)?|開始日?)[:：]?\s*([^\n\r]{1,30})").unwrap();
}

/// 経験年数を抽出（最初にマッチしたもの）
pub fn extract_min_experience(text: &str) -> Option<i32> {
    if let Some(caps) = EXP_YEARS_RE.captures(text) {
        return caps[1].parse().ok();
    }
    if let Some(caps) = EXP_OVER_RE.captures(text) {
        return caps[1].parse().ok();
    }
    None
}

/// 最寄駅を抽出
/// ⚠️ CD-6準拠: normalize_station が None を返す場合は None
pub fn extract_nearest_station(text: &str) -> Option<String> {
    STATION_RE.captures(text)
        .and_then(|caps| normalize_station(&caps[1]))
}

// 📌 正規実装: CD-6「normalize_station の空文字禁止」を参照
// sr-common/src/corrections/station.rs の normalize_station() を使用すること
//
// 仕様:
//   - 入力: 駅名文字列
//   - 出力: Option<String>（空文字は None）
//   - 「駅」接尾辞を自動付与
//   - 例: "新宿" → Some("新宿駅"), "渋谷駅" → Some("渋谷駅"), "" → None
//
// ⚠️ 重複実装禁止: 実装は CD-6 の1箇所のみ

/// 年齢レンジ（推定の精度を保持）
#[derive(Debug, Clone, PartialEq)]
pub struct AgeRange {
    pub min: i32,
    pub max: i32,
    pub estimated_midpoint: i32,  // 後方互換用の中央値
}

/// 年齢を抽出（レンジベース版: より保守的な推定）
///
/// 【仕様契約 SC-AGE: 年齢抽出の精度保持】
/// - 具体的な年齢（"32歳"）→ 確定値（min=max=32）
/// - 世代（"30代"）→ レンジ（30-39）
/// - 世代＋前後半（"30代前半"）→ 狭いレンジ（30-34）
/// - KO判定はレンジで比較（境界で事故防止）
pub fn extract_age_range(text: &str) -> Option<AgeRange> {
    // 1. 具体的年齢（"32歳"）→ 確定値
    if let Some(caps) = AGE_RE.captures(text) {
        let age: i32 = caps[1].parse().ok()?;
        return Some(AgeRange { min: age, max: age, estimated_midpoint: age });
    }

    // 2. 世代＋前後半（"30代前半", "30代後半"）
    static AGE_GEN_HALF_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(\d)0代(前半|後半|半ば)").unwrap()
    });
    if let Some(caps) = AGE_GEN_HALF_RE.captures(text) {
        let decade: i32 = caps[1].parse().ok()?;
        let base = decade * 10;
        let (min, max, mid) = match &caps[2] {
            "前半" => (base, base + 4, base + 2),      // 30-34, mid=32
            "後半" => (base + 5, base + 9, base + 7),  // 35-39, mid=37
            "半ば" => (base + 3, base + 6, base + 5),  // 33-36, mid=35
            _ => (base, base + 9, base + 5),
        };
        return Some(AgeRange { min, max, estimated_midpoint: mid });
    }

    // 3. 世代のみ（"30代"）→ フルレンジ
    if let Some(caps) = AGE_GENERATION_RE.captures(text) {
        let decade: i32 = caps[1].parse().ok()?;
        let base = decade * 10;
        return Some(AgeRange {
            min: base,
            max: base + 9,
            estimated_midpoint: base + 5,  // 30代 → 35
        });
    }

    None
}

/// 年齢を抽出（後方互換: 中央値を返す）
/// ⚠️ 新規コードは extract_age_range() を使用すること
pub fn extract_age(text: &str) -> Option<i32> {
    extract_age_range(text).map(|r| r.estimated_midpoint)
}

/// 性別を抽出
/// ⚠️ 出力は DB ENUM（ses.talent_gender_enum）と統一: 「男性」「女性」「その他/無回答」
/// ⚠️ correct_gender() と同じ出力形式を使用すること
pub fn extract_gender(text: &str) -> Option<String> {
    GENDER_RE.captures(text)
        .map(|caps| match &caps[1] {
            "男性" => "男性".to_string(),
            "女性" => "女性".to_string(),
            _ => "その他/無回答".to_string(),
        })
}

/// 希望単価（下限）を抽出
pub fn extract_desired_price_min(text: &str) -> Option<i32> {
    PRICE_MIN_RE.captures(text)
        .and_then(|caps| caps[1].parse().ok())
}

/// 希望単価（上限）を抽出
pub fn extract_desired_price_max(text: &str) -> Option<i32> {
    PRICE_MAX_RE.captures(text)
        .and_then(|caps| caps[1].parse().ok())
}

/// 精算幅を抽出（例: "140-180h"）（#12修正: 2025-12-17）
///
/// 【仕様契約 SC-17: settlement_range 形式統一】
/// - 出力形式: "XXX-YYYh"（VARCHAR(20)に収まる）
/// - 妥当性チェック: min < max, 範囲 80-240（業界標準の精算幅）
/// - min > max の場合: 自動でスワップ
/// - 範囲外の場合: None（抽出失敗、手動レビューへ）
///
/// ⚠️ sponto-platform の settlement_range カラムと整合
pub fn extract_settlement_range(text: &str) -> Option<String> {
    SETTLEMENT_RE.captures(text).and_then(|caps| {
        let v1: u32 = caps[1].parse().ok()?;
        let v2: u32 = caps[2].parse().ok()?;

        // 順序を正規化（小さい方を先に）
        let (min, max) = if v1 <= v2 { (v1, v2) } else { (v2, v1) };

        // 妥当性チェック: 業界標準の精算幅範囲（80-240時間）
        // ⚠️ 40時間（週1日）未満や300時間（残業過多）超は異常値
        //
        // 📌 定数配置: sr-common/src/constants/settlement.rs に集約
        //    SC-17「精算幅フォーマット統一」と同じ値を使用
        //    → SETTLEMENT_VALID_MIN_HOURS = 80
        //    → SETTLEMENT_VALID_MAX_HOURS = 240
        const VALID_MIN: u32 = 80;  // TODO: sr-common の定数を参照
        const VALID_MAX: u32 = 240; // TODO: sr-common の定数を参照

        if min < VALID_MIN || max > VALID_MAX {
            return None; // 範囲外 → 抽出失敗、手動レビューへ
        }

        Some(format!("{}-{}h", min, max))
    })
}

/// 稼働開始日（原文）を抽出
pub fn extract_available_date_raw(text: &str) -> Option<String> {
    AVAILABLE_DATE_RE.captures(text)
        .map(|caps| caps[1].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_experience() {
        assert_eq!(extract_min_experience("経験10年"), Some(10));
        assert_eq!(extract_min_experience("エンジニア歴: 5年"), Some(5));
        assert_eq!(extract_min_experience("3年以上"), Some(3));
    }

    #[test]
    fn test_extract_station() {
        // ⚠️ normalize_station により「駅」が自動付与される
        assert_eq!(extract_nearest_station("最寄駅：新宿"), Some("新宿駅".to_string()));
        assert_eq!(extract_nearest_station("最寄り駅: 渋谷"), Some("渋谷駅".to_string()));
        assert_eq!(extract_nearest_station("最寄駅：品川駅"), Some("品川駅".to_string())); // 既に駅付き
    }

    // ======== normalize_station() テストケース ========
    // 📌 これらのテストは CD-6 の正規実装に対して実行される
    // 実装: sr-common/src/corrections/station.rs

    #[test]
    fn test_normalize_station() {
        // CD-6 準拠: normalize_station は Option<String> を返す
        assert_eq!(normalize_station("新宿"), Some("新宿駅".into()));
        assert_eq!(normalize_station("渋谷駅"), Some("渋谷駅".into())); // そのまま
        assert_eq!(normalize_station("  池袋  "), Some("池袋駅".into())); // trim + 駅
    }

    // ======== A: normalize_station() 追加テストケース（CD-6参照） ========

    #[test]
    fn test_normalize_station_edge_cases() {
        // 空文字 → None（CD-6: 空文字禁止）
        assert_eq!(normalize_station(""), None);
        assert_eq!(normalize_station("   "), None);
        assert_eq!(normalize_station("\t\n"), None);

        // 複合駅名
        assert_eq!(normalize_station("東京"), Some("東京駅".into()));
        assert_eq!(normalize_station("大手町"), Some("大手町駅".into()));
        assert_eq!(normalize_station("西日暮里"), Some("西日暮里駅".into()));

        // 既に駅が付いている複合ケース
        assert_eq!(normalize_station("新宿三丁目駅"), Some("新宿三丁目駅".into()));
        assert_eq!(normalize_station("六本木一丁目駅"), Some("六本木一丁目駅".into()));
    }

    #[test]
    fn test_normalize_station_special_names() {
        // 駅で終わらないが有名な駅名
        assert_eq!(normalize_station("品川シーサイド"), Some("品川シーサイド駅".into()));
        assert_eq!(normalize_station("天王洲アイル"), Some("天王洲アイル駅".into()));
        assert_eq!(normalize_station("お台場海浜公園"), Some("お台場海浜公園駅".into()));

        // カタカナ駅名
        assert_eq!(normalize_station("スカイツリー"), Some("スカイツリー駅".into()));
        assert_eq!(normalize_station("センター南"), Some("センター南駅".into()));
    }

    #[test]
    fn test_normalize_station_real_world() {
        // 実際のメールで見られるパターン
        assert_eq!(normalize_station("秋葉原（JR・つくば）"), Some("秋葉原（JR・つくば）駅".into()));
        assert_eq!(normalize_station("飯田橋/九段下"), Some("飯田橋/九段下駅".into()));
    }

    #[test]
    fn test_extract_age() {
        assert_eq!(extract_age("32歳"), Some(32));
        assert_eq!(extract_age("30代"), Some(35));
    }

    // ======== #12: settlement_range テストケース追加 ========

    #[test]
    fn test_extract_settlement_range_basic() {
        // 標準形式
        assert_eq!(
            extract_settlement_range("精算：140-180h"),
            Some("140-180h".to_string())
        );
        assert_eq!(
            extract_settlement_range("精算幅：150〜200時間"),
            Some("150-200h".to_string())
        );
        assert_eq!(
            extract_settlement_range("稼働時間: 160～180H"),
            Some("160-180h".to_string())
        );
    }

    #[test]
    fn test_extract_settlement_range_swap() {
        // min > max の場合、自動スワップ
        assert_eq!(
            extract_settlement_range("精算：180-140h"),
            Some("140-180h".to_string())
        );
    }

    #[test]
    fn test_extract_settlement_range_out_of_range() {
        // 範囲外 → None
        assert_eq!(extract_settlement_range("精算：40-80h"), None);   // min < 80
        assert_eq!(extract_settlement_range("精算：200-300h"), None); // max > 240
        assert_eq!(extract_settlement_range("精算：50-60h"), None);   // 両方範囲外
    }

    #[test]
    fn test_extract_settlement_range_edge_cases() {
        // 境界値
        assert_eq!(
            extract_settlement_range("精算：80-240h"),
            Some("80-240h".to_string())
        ); // 最小〜最大
        assert_eq!(
            extract_settlement_range("精算：140-140h"),
            Some("140-140h".to_string())
        ); // 固定時間
    }

    #[test]
    fn test_extract_settlement_range_no_match() {
        // マッチしない形式
        assert_eq!(extract_settlement_range("時給1500円"), None);
        assert_eq!(extract_settlement_range("月額80万"), None);
    }
}
```

---

## 保守的判定ルール詳細

### ルール定義（推奨判定）

**重要**: この判定は sr-extractor による **LLM 推奨判定（見積もり）** であり、最終決定は sr-llm-worker が行う。

```rust
struct ExtractionQuality {
    tier1_extracted: usize,  // 0-4
    tier1_total: usize,      // 4
    tier2_extracted: usize,  // 0-2
    tier2_total: usize,      // 2
}

// ⚠️ 判定関数は sr-common に1本だけ配置（推奨 ≒ 確定、同じ predicate）
// sr-common/src/decision.rs
//
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 【High 8】LLM呼び出し判定を sr-common に集約
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// 1. extractor と worker が同じ判定関数を使う（推奨判定 vs 確定判定のズレ防止）
// 2. DecisionPolicy で閾値を構造化（Phase 2 で調整が容易）
// 3. 環境変数トグル（SR_FORCE_LLM等）も sr-common で一元管理
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// LLM呼び出し判定の結果
#[derive(Debug, Clone)]
pub struct LlmDecision {
    pub needs_llm: bool,
    pub reason: String,
}

/// LLM呼び出し判定のポリシー（閾値を構造化）
/// Phase 2 で閾値調整しても改修が小さくなる
#[derive(Debug, Clone)]
pub struct DecisionPolicy {
    pub tier1_required: usize,  // Tier1 の必要数（デフォルト: 4）
    pub tier2_min: usize,       // Tier2 の最低数（デフォルト: 1）
}

impl Default for DecisionPolicy {
    fn default() -> Self {
        Self {
            tier1_required: 4,  // MVP: 4/4 必須
            tier2_min: 1,       // MVP: 1/2 以上必須
        }
    }
}

impl DecisionPolicy {
    /// 環境変数から読み込み（Phase 2 で調整用）
    pub fn from_env() -> Self {
        Self {
            tier1_required: std::env::var("SR_TIER1_REQUIRED")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(4),
            tier2_min: std::env::var("SR_TIER2_MIN")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(1),
        }
    }
}

/// ⚠️ sr-common に配置: extractor も worker も同じ関数を使う
pub fn should_call_llm(
    tier1_extracted: usize,
    tier2_extracted: usize,
    policy: &DecisionPolicy,
) -> LlmDecision {
    // 条件1: Tier1が不足 → LLM 推奨
    if tier1_extracted < policy.tier1_required {
        return LlmDecision {
            needs_llm: true,
            reason: format!("Tier1 incomplete {}/{}", tier1_extracted, policy.tier1_required),
        };
    }

    // 条件2: Tier2が閾値未満 → LLM 推奨
    if tier2_extracted < policy.tier2_min {
        return LlmDecision {
            needs_llm: true,
            reason: format!("Tier2 insufficient {}/{}", tier2_extracted, policy.tier2_min),
        };
    }

    // 全条件クリア → Rust のみで十分（推奨）
    LlmDecision {
        needs_llm: false,
        reason: "Rust extraction sufficient".to_string(),
    }
}

// ========================================
// 統一されたヘルパー関数（sr-common）
// ========================================

/// ExtractionQuality から判定（extractor 用）
pub fn decide_from_quality(quality: &ExtractionQuality, policy: &DecisionPolicy) -> LlmDecision {
    should_call_llm(quality.tier1_extracted, quality.tier2_extracted, policy)
}

/// PartialFields から判定（worker 用）
pub fn decide_from_partial(partial: &PartialFields, policy: &DecisionPolicy) -> LlmDecision {
    let tier1_count = [
        partial.monthly_tanka_min.is_some(),
        partial.monthly_tanka_max.is_some(),
        partial.start_date_raw.is_some(),
        partial.work_todofuken.is_some(),
    ].iter().filter(|&&x| x).count();

    let tier2_count = [
        partial.remote_onsite.is_some(),
        partial.flow_dept.is_some(),
    ].iter().filter(|&&x| x).count();

    should_call_llm(tier1_count, tier2_count, policy)
}

/// 運用トグルで上書き（policy 明示版）
pub fn final_decision(partial: &PartialFields, policy: &DecisionPolicy) -> LlmDecision {
    // SR_FORCE_LLM=1 なら強制
    if std::env::var("SR_FORCE_LLM").map(|v| v == "1").unwrap_or(false) {
        return LlmDecision {
            needs_llm: true,
            reason: "Forced by SR_FORCE_LLM=1".to_string(),
        };
    }

    decide_from_partial(partial, policy)
}

// ========================================
// 薄いラッパー（内部で from_env() を呼ぶ）
// ========================================

/// ⚠️ 【推奨API】env から policy を自動ロードするラッパー
/// extractor / worker どちらもこれを使う
pub fn final_decision_with_env(partial: &PartialFields) -> LlmDecision {
    let policy = DecisionPolicy::from_env();
    final_decision(partial, &policy)
}

/// extractor 用: ExtractionQuality から判定（env から policy 自動ロード）
pub fn decide_from_quality_with_env(quality: &ExtractionQuality) -> LlmDecision {
    let policy = DecisionPolicy::from_env();
    decide_from_quality(quality, &policy)
}
```

**使用例（extractor / worker 共通）**:
```rust
// ✅ 【推奨】_with_env 版を使う（policy を意識する必要なし）
// extractor
let decision = decide_from_quality_with_env(&quality);

// worker
let decision = final_decision_with_env(&partial);

// ❌ 非推奨（policy を明示的に渡す必要がある場合のみ）
// let policy = DecisionPolicy::from_env();
// let decision = final_decision(&partial, &policy);
```

**Note（統一ルール）**:
- **推奨 ≒ 確定**（同じ predicate、同じ `DecisionPolicy`）
- 例外はトグル（`SR_FORCE_LLM` 等）/ 軽量ルール成否 / LLM障害だけ
- Phase 2 で閾値調整する場合は `SR_TIER1_REQUIRED`, `SR_TIER2_MIN` 環境変数で制御
- 「何が理由でLLMになったか」が `decision.reason` で追いやすい

### 🔑 Rust完結パスを成立させるための必要条件

**問題**: `projects_enum` は `required_skills (JSONB NOT NULL)` だが、extractor（sr-extractor）では `required_skills` を抽出しない。
→ worker が常に LLM で `required_skills` を埋めると、`rust_completed` がほぼ発生せず **ルールベースで説明可能な処理が増えない**。

**解決策（MVP最小限）**: sr-llm-worker に**超軽量ルール抽出**を1本だけ実装する。
これにより、明確なパターンは Rust で処理し、LLM は曖昧な判断に集中できる。

```rust
// sr-llm-worker/src/light_rules.rs
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // スキルセクション開始っぽい行（第一入口）
    static ref SKILL_HEADER_RE: Regex =
        Regex::new(r"(?i)^(?:■\s*)?(?:必須|募集|求める)?\s*スキル\s*[:：]?|^必須スキル\s*[:：]?|^【必須】")
            .unwrap();

    // 【第二入口】環境/言語/技術要素 の行（"スキル" と書いてないメール対策）
    // 破綻対策: ルールを増やしすぎず、入口だけ増やす方針
    static ref TECH_LINE_RE: Regex =
        Regex::new(r"(?i)^(?:言語|環境|技術|OS|DB|FW|フレームワーク|開発環境)\s*[:：]")
            .unwrap();

    // 次の見出し（■ / 【 】が出たらセクション終了）
    static ref NEXT_HEADER_RE: Regex = Regex::new(r"^(?:■|【)").unwrap();

    // 箇条書き（日本語OK）
    static ref BULLET_RE: Regex = Regex::new(r"^\s*[・\-*]\s*(.+?)\s*$").unwrap();
}

/// 超軽量ルール抽出（LLM不要・精度はほどほど）
/// - "スキル"セクションだけ対象にすることでノイズを抑制
/// - 日本語/英数字混在OK
///
/// 【破綻対策】第二入口
/// - "スキル" と書いてないメールでも、「言語: Java」「環境: AWS」等から取れる
/// - ルールを増やしすぎず、入口（ヘッダーパターン）だけ増やす方針
pub fn extract_required_skills_light(body_text: &str) -> Vec<String> {
    let mut skills: Vec<String> = Vec::new();
    let mut in_section = false;

    for raw_line in body_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() { continue; }

        // セクション開始（第一入口: スキルセクション）
        if SKILL_HEADER_RE.is_match(line) {
            in_section = true;

            // 「必須スキル：Java, Spring」みたいな同一行記載も拾う
            if let Some(idx) = line.find('：').or_else(|| line.find(':')) {
                let tail = line[idx + 1..].trim();
                skills.extend(split_inline_tokens(tail));
            }
            continue;
        }

        // 【第二入口】環境/言語/技術行（"スキル" と書いてなくても拾う）
        // 例: 「言語: Java, Python」「環境: AWS, Docker」
        if TECH_LINE_RE.is_match(line) {
            if let Some(idx) = line.find('：').or_else(|| line.find(':')) {
                let tail = line[idx + 1..].trim();
                skills.extend(split_inline_tokens(tail));
            }
            continue;  // セクションモードには入らない（単行抽出）
        }

        // セクション中に次見出しが来たら終了
        if in_section && NEXT_HEADER_RE.is_match(line) && !BULLET_RE.is_match(line) {
            break;
        }

        if in_section {
            if let Some(caps) = BULLET_RE.captures(line) {
                let item = caps.get(1).unwrap().as_str().trim();
                if is_reasonable(item) {
                    skills.push(item.to_string());
                }
            }

            if skills.len() >= 10 { break; }
        }
    }

    skills.truncate(10);
    skills
}

fn split_inline_tokens(s: &str) -> Vec<String> {
    s.split(|c| matches!(c, '、' | ',' | '/' | '／'))
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .filter(|t| is_reasonable(t))
        .map(|t| t.to_string())
        .collect()
}

fn is_reasonable(s: &str) -> bool {
    // 長文段落誤爆を避ける（MVP向け）
    let n = s.chars().count();
    n >= 1 && n <= 60
}

/// 軽量ルール抽出結果を required_skills JSONB 形式に変換
///
/// 🚨 重要：既存LLM出力と同じスキーマに合わせる（実装前に要確認）
/// - 既存の matching/UI/集計が期待する形式に合わせないと、下流ロジックが壊れる
/// - 現在の形式: {"items": [{"skill": "...", "years": 0}, ...]}
/// - 実装前に既存 projects_enum.required_skills の実データを3件見て形状を確認すること
///   - もし既存が {"keywords":[...], "must":[...]} 形式なら、それに合わせる
///   - もし既存が単純配列 ["Java","Spring"] なら、json!(skills) でOK
/// ⚠️ MVP 制限: skill.years は常に 0
///
/// **理由**: 経験年数の正確な推定は難しく、誤った値を入れるより 0 の方が安全。
/// 下流の matching ロジックは `years` フィールドを現状使用していないため、
/// 0 でも壊れない（要確認: strategy.py, business_rules_engine.py）。
///
/// **Phase 2 改善案**:
/// 1. メール本文から「Java 5年」「Spring Boot 3年以上」等のパターンを抽出
/// 2. 正規表現: `r"(\w+)[\s　]*([\d]+)[\s　]*年"` で年数をキャプチャ
/// 3. スキルと年数のマッピングを返す（HashMap<String, u32>）
/// 4. 不明な場合は `years: null` (NULL許容に DDL 変更) または 0 のまま
pub fn skills_to_jsonb(skills: Vec<String>) -> serde_json::Value {
    let items: Vec<serde_json::Value> = skills
        .iter()
        .map(|skill| serde_json::json!({
            "skill": skill,
            "years": 0  // MVP: 経験年数は 0 固定（Phase 2 で推定ロジック追加予定）
        }))
        .collect();

    // 既存スキーマに合わせた形式（DDL で確認済み）
    serde_json::json!({
        "items": items
    })
}

/// 軽量ルール抽出結果を required_skills_keywords TEXT[] 形式に変換
/// matching のKO/スコアリングが required_skills_keywords を参照するため必須
///
/// ⚠️ sr-common::skill_normalizer::normalize_skill() を使用
/// エイリアス正規化も含む（"js" → "javascript" 等）
pub fn skills_to_keywords(skills: Vec<String>) -> Vec<String> {
    use sr_common::skill_normalizer::normalize_skill;

    skills.into_iter()
        .map(|s| normalize_skill(&s))
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .collect()
}
```

**使い方（sr-llm-worker の最終判定 = 3段階）**:

sr-llm-worker は extractor の推奨判定を受け取った後、以下の **3段階判定** で確定する:

```rust
// 【段階1】Tier 不足 → LLM 必須（sr-common::decision を使用）
if decide_from_partial(&partial).needs_llm {
    final_result = call_llm(&body_text).await?;
    final_method = "llm_completed";
}
// 【段階2】Tier OK → 軽量ルールで required_skills 抽出試行
else {
    let skills = light_rules::extract_required_skills_light(&body_text);

    if !skills.is_empty() {
        // ✅ 軽量ルールで埋められた → LLM 不要
        // NOT NULL 制約を満たすため、project_name もフォールバック
        final_result.project_name = partial.project_name
            .or_else(|| Some(normalize_subject(&email_subject)));  // queue の email_subject を使用

        // 下流の matching/UI/集計のため、両方のカラムを埋める（重要）
        final_result.required_skills = Some(light_rules::skills_to_jsonb(skills.clone()));
        final_result.required_skills_keywords = Some(light_rules::skills_to_keywords(skills));

        final_method = "rust_completed";  // ✅ API削減カウント
    } else {
        // 【段階3】軽量ルール失敗 → LLM フォールバック
        final_result = call_llm(&body_text).await?;
        final_method = "llm_completed";
    }
}
```

**3段階判定のポイント**:
1. **Tier 欠損**: Tier1 < 4/4 または Tier2 < 1/2 → LLM 必須
2. **Tier 満足 + 軽量ルール成功**: `required_skills` を埋められた → **rust_completed**（説明可能な処理）
   - `project_name` は partial から取るか、なければ `normalize_subject(subject)` で補完（※1）
   - `required_skills` (JSONB NOT NULL) は `skills_to_jsonb()` で `{"items": [{"skill": "...", "years": 0}, ...]}` 形式に変換
   - `required_skills_keywords` (TEXT[]) は `skills_to_keywords()` でキーワード配列に変換（※2）
   - **NOT NULL 制約をルールベースで満たせる** → 理由コード付きで蓄積可能
3. **Tier 満足 + 軽量ルール失敗**: 空配列 → LLM フォールバック（または manual_review）

**※1 normalize_subject**: 重複排除用の `normalize_subject` 関数を流用（既出：Tier2セクション参照）
**※2 required_skills_keywords の重要性**: matching の KO/スコアリングが `required_skills_keywords` を参照するため、**両方のカラムを埋めないと下流ロジックが壊れる**

**期待効果**:
- Tier1/2 満たすメールの **60-70%** で 軽量ルールが成功し、`rust_completed` が発生（推定）
- → ルールベースで説明可能な抽出が半数以上に（LLM は残りに集中）
- 精度はほどほどでOK（後で LLM で補正可能）

**Phase 2 で検証**: 軽量ルール抽出の成功率を実測し、Gate 未達なら辞書を拡張。

### 判定フローチャート（MVP版）

**重要**: 全てのメールを queue に積み、worker が最終決定する。

```mermaid
flowchart TD
    Start["メール受信<br/>subject + body_text (anken_emails)"]

    Extract["sr-extractor<br/>regex抽出(Tier1/2)<br/>quality算出・priority付与<br/>recommended_method設定"]
    Enqueue["INSERT ses.extraction_queue<br/>(idempotent)"]

    Lock["sr-llm-worker<br/>pendingをロック(SKIP LOCKED)"]

    Decide{"worker最終判定<br/>必須項目を満たせるか？<br/>(既存ロジック/軽量ルール/LLM)"}
    NoLLM["LLMなしで補完/確定<br/>(rules or existing non-LLM)"]
    CallLLM["LLM呼び出し<br/>不足フィールド補完"]

    FillRequired["DB制約を必ず満たす<br/>(project_name/required_skills にfallback値)"]
    Validate{"自動マッチ可能な品質か？<br/>(不足なら manual_review)"}
    InsertOK["projects_enumへINSERT<br/>final_method = rust_completed or llm_completed"]
    InsertReview["projects_enumへINSERT<br/>requires_manual_review=true<br/>final_method = manual_review"]

    Done["queueをcompletedに更新<br/>final_method確定・監査ログ"]

    Start --> Extract --> Enqueue --> Lock --> Decide
    Decide -->|LLM不要| NoLLM --> FillRequired --> Validate
    Decide -->|LLM必要| CallLLM --> FillRequired --> Validate
    Validate -->|Yes| InsertOK --> Done
    Validate -->|No| InsertReview --> Done

    style InsertOK fill:#c8e6c9
    style InsertReview fill:#fff3cd
    style CallLLM fill:#ffcdd2
```

### 期待される分布（Assumption - Phase 2で検証）

| メール品質 | Tier1 | Tier2 | 判定 | 割合（Assumption） |
|-----------|-------|-------|------|--------------------|
| **高品質** | 4/4 | 2/2 | Rustのみ | **35%（未検証）** |
| **良好** | 4/4 | 1/2 | Rustのみ | **20%（未検証）** |
| **普通** | 4/4 | 0/2 | LLM | **25%（未検証）** |
| **低品質** | 0-3/4 | - | LLM | **20%（未検証）** |

**Rustのみで完結（Target）**: **55%**（未検証・Phase 2で実測）
**LLM必要（Target）**: **45%**（未検証・Phase 2で実測）

**検証スケジュール**:
- Week 4-5: 過去100件メールで実測
- Week 6-7: Canary 10%で実測
- Week 10: 100%移行後、1週間で確定値取得

---

## キューテーブル設計

### ses.extraction_queue スキーマ

```sql
CREATE TABLE ses.extraction_queue (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,  -- anken_emails.message_id への参照
    email_subject TEXT NOT NULL,  -- メール件名（worker の project_name フォールバックに使用）
    email_received_at TIMESTAMPTZ NOT NULL,  -- メール受信日時（anken_emails.created_at から転記、SC-2 相対日付の基準）
    subject_hash VARCHAR(16) NOT NULL,  -- sha256(normalize(subject))[:16] (重複排除用)

    -- キュー状態（3状態設計: failed は廃止）
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending/processing/completed のみ
    priority INTEGER NOT NULL DEFAULT 50,  -- 0-100, 高い方が優先（Tier1欠損=100, 満たす=10）
    locked_by VARCHAR(100),  -- worker ID（processing 中のみ NOT NULL）

    -- リトライ管理
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    last_error TEXT,

    -- Rust抽出結果（partial）
    partial_fields JSONB,  -- {monthly_tanka_min: 70, work_todofuken: "東京都", ...}
    decision_reason TEXT,  -- "LLM recommended: Tier1 incomplete 2/4 fields" 等

    -- 抽出方法（KPI集計の真実）
    recommended_method VARCHAR(20),  -- rust_recommended / llm_recommended (sr-extractor)
    final_method VARCHAR(20),  -- rust_completed / llm_completed / manual_review (sr-llm-worker)

    -- デバッグ用バージョン
    extractor_version VARCHAR(20),  -- e.g., "0.1.0", "0.2.1"
    rule_version VARCHAR(20),  -- e.g., "2025-01-15", "v2"

    -- タイムスタンプ（KPI計測用）
    -- ⚠️ 統一ルール: キュー待機時間 = processing_started_at - created_at（locked_at は冗長なので廃止）
    created_at TIMESTAMPTZ DEFAULT NOW(),
    processing_started_at TIMESTAMPTZ,  -- worker が処理開始した時刻（Stuck Job 判定にも使用）
    completed_at TIMESTAMPTZ,  -- 処理完了時刻
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- パフォーマンス計測
    llm_latency_ms INTEGER,  -- LLM API呼び出しのレイテンシ（ms）（worker が計測）

    -- 手動レビュー・Canary 制御
    requires_manual_review BOOLEAN NOT NULL DEFAULT false,  -- true: 自動処理せず手動レビュー待ち
    manual_review_reason TEXT,  -- 破綻対策: なぜ手動レビューか（enum_correction_failed / skills_empty / start_date_unknown / llm_permanent_error / body_missing）
    canary_target BOOLEAN NOT NULL DEFAULT false,  -- true: Canary対象として自動処理

    -- 再処理制御（破綻対策: 改善ループを回すための仕組み）
    reprocess_after TIMESTAMPTZ,  -- この時刻以降に再処理可能（NULL=再処理しない）

    -- CHECK制約（データ整合性を保証）
    -- ⚠️ ステータス3状態設計:
    --   pending: 処理待ち（リトライ待ちも含む、next_retry_at + retry_count で管理）
    --   processing: ロック中（worker が処理中）
    --   completed: 終了（成功 or manual_review、final_method で判別）
    -- ※ failed ステータスは廃止（pending + retry_count で表現）
    CONSTRAINT chk_status CHECK (status IN ('pending', 'processing', 'completed')),
    CONSTRAINT chk_recommended_method CHECK (recommended_method IN ('rust_recommended', 'llm_recommended')),
    CONSTRAINT chk_final_method CHECK (final_method IS NULL OR final_method IN ('rust_completed', 'llm_completed', 'manual_review')),
    CONSTRAINT chk_priority CHECK (priority >= 0 AND priority <= 100)
);

CREATE INDEX idx_extraction_queue_status_priority ON ses.extraction_queue(status, priority DESC, next_retry_at);
CREATE INDEX idx_extraction_queue_message_id ON ses.extraction_queue(message_id);
CREATE INDEX idx_extraction_queue_subject_hash ON ses.extraction_queue(subject_hash, created_at);
CREATE INDEX idx_extraction_queue_canary ON ses.extraction_queue(canary_target, created_at);  -- Canary 監視用
CREATE INDEX idx_extraction_queue_reprocess ON ses.extraction_queue(reprocess_after) WHERE reprocess_after IS NOT NULL;  -- 再処理対象検索用
CREATE INDEX idx_extraction_queue_review_reason ON ses.extraction_queue(manual_review_reason) WHERE manual_review_reason IS NOT NULL;  -- レビュー理由別集計用
```

**priority フィールドの意味**:
- **100**: Tier1 欠損（LLM 必須）→ 最優先
- **50**: Tier1 満たすが Tier2 不足（LLM 推奨）
- **10**: Tier1/2 満たす（LLM 不要寄り）
- worker は priority 降順で処理することで、LLM が必要なジョブを優先的に処理。

**recommended_method / final_method の意味**:
- `recommended_method`: sr-extractor が設定（推奨判定）
  - `rust_recommended`: Tier1=4/4 AND Tier2≥1/2 → Rust のみで十分（推奨）
  - `llm_recommended`: それ以外 → LLM 推奨
- `final_method`: sr-llm-worker が設定（確定判定・KPI集計の真実）
  - `rust_completed`: worker が Rust 抽出のみで projects_enum に INSERT した
  - `llm_completed`: worker が LLM を呼び出して projects_enum に INSERT した
  - `manual_review`: 必須フィールド不足で `requires_manual_review=true` で INSERT した

**Note**: KPI 集計は `final_method` を使用する（`recommended_method` は参考値）。

**manual_review_reason の意味**（破綻対策）:
- `manual_review` だけでは「失敗の終着」と「品質保留」が混在
- 理由を記録することで、改善可能な失敗を特定できる

| reason | 意味 | 改善アクション |
|--------|------|---------------|
| `enum_correction_failed` | ENUM補正失敗（東京都→? 等） | ENUM辞書を拡張 |
| `skills_empty` | スキル抽出失敗 | 軽量ルールパターン追加 |
| `start_date_unknown` | 開始日不明 | 日付パターン追加 |
| `llm_permanent_error` | LLM API恒久エラー | プロンプト見直し |
| `body_missing` | メール本文がない | 上流データ確認 |

**reprocess_after の意味**（破綻対策）:
- completed は通常再処理しないが、改善ループを回すには「再処理したい completed」が出る
- `reprocess_after` を設定すると、その時刻以降に再処理対象になる
- 無限再処理は防止（1回の reprocess で NULL に戻す）

```sql
-- 再処理対象を取得（sr-queue-recovery が定期実行）
SELECT * FROM ses.extraction_queue
WHERE status = 'completed'
  AND reprocess_after IS NOT NULL
  AND reprocess_after <= NOW()
ORDER BY reprocess_after
LIMIT 100;

-- 再処理開始時に NULL にリセット（無限ループ防止）
UPDATE ses.extraction_queue
SET status = 'pending', reprocess_after = NULL, retry_count = 0
WHERE id = $1;
```

### Enqueue（Idempotent）

```sql
-- sr-extractor が使用（同じメールを複数回拾っても安全）
INSERT INTO ses.extraction_queue (
    message_id,
    email_subject,
    email_received_at,  -- anken_emails.created_at から転記（SC-2 相対日付の基準）
    subject_hash,
    priority,
    partial_fields,
    decision_reason,
    recommended_method,
    extractor_version,
    rule_version,
    status
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'pending'
)
ON CONFLICT (message_id) DO UPDATE SET
    -- status が completed なら何もしない
    email_subject = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.email_subject
        ELSE EXCLUDED.email_subject
    END,
    subject_hash = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.subject_hash
        ELSE EXCLUDED.subject_hash
    END,
    partial_fields = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.partial_fields
        ELSE EXCLUDED.partial_fields
    END,
    priority = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.priority
        ELSE EXCLUDED.priority
    END,
    recommended_method = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.recommended_method
        ELSE EXCLUDED.recommended_method
    END,
    decision_reason = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.decision_reason
        ELSE EXCLUDED.decision_reason
    END,
    extractor_version = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.extractor_version
        ELSE EXCLUDED.extractor_version
    END,
    rule_version = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.rule_version
        ELSE EXCLUDED.rule_version
    END,
    updated_at = CASE
        WHEN ses.extraction_queue.status = 'completed' THEN ses.extraction_queue.updated_at
        ELSE NOW()
    END
RETURNING *;
```

**ポリシー**:
- `status='completed'` なら何もしない（既に処理済み）
- `status='pending'` なら `partial_fields` と `updated_at` を更新（再試行）
- ⚠️ ステータスは3状態のみ: pending / processing / completed
- `extractor_version` / `rule_version` によりどのバージョンで処理されたか追跡可能（デバッグ時に有用）

### バージョン管理の運用ルール（extractor_version / rule_version）

**MVP でのおすすめ運用**:

```rust
// sr-extractor/src/main.rs

// 1. extractor_version: ビルド版（Cargo.toml から自動取得）
const EXTRACTOR_VERSION: &str = env!("CARGO_PKG_VERSION");  // e.g., "0.1.0"

// 2. rule_version: 手動管理（regex や補正ルール変更時に更新）
// 命名規則: YYYY-MM-DD-rN（日付 + 連番）
const RULE_VERSION: &str = "2025-01-15-r1";
```

**rule_version を上げるタイミング**:
- regex パターンの追加・変更（月額単価、勤務地など）
- ENUM 補正ルールの追加・変更（`correct_todofuken()` など）
- LLM API 契約変更（プロンプト変更、モデル変更）
- Tier 判定ロジックの変更

**Git SHA 方式（より厳密な追跡が必要な場合）**:

```rust
// build.rs で Git SHA を埋め込む
fn main() {
    let git_sha = std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=GIT_SHA={}", git_sha);
}

// main.rs で使用
const RULE_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-", env!("GIT_SHA"));
// → "0.1.0-a1b2c3d"
```

**PR テンプレートのチェック項目（漏れ防止）**:

```markdown
## PR Checklist

- [ ] regex/補正ルール変更がある場合、`RULE_VERSION` を更新したか？
- [ ] 変更内容を CHANGELOG に記載したか？
- [ ] 品質テスト（既存メール10件）を実行したか？
```

### ポーリングクエリ（SKIP LOCKED + Priority）

```sql
-- sr-llm-worker が使用
UPDATE ses.extraction_queue
SET
    status = 'processing',
    locked_by = $1,  -- worker_id
    processing_started_at = NOW()  -- ⚠️ KPI計測 & Stuck Job 判定に使用
WHERE id = (
    SELECT id
    FROM ses.extraction_queue
    WHERE status = 'pending'
      AND (next_retry_at IS NULL OR next_retry_at <= NOW())
    ORDER BY priority DESC, created_at
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
RETURNING *;
```

**ポイント**:
- `FOR UPDATE SKIP LOCKED` により、複数 worker が並列動作しても二重取りしない。
- `ORDER BY priority DESC` により、LLM 必須ジョブ（priority=100）を優先処理。
- `next_retry_at` によりリトライジョブの fixed backoff（MVP）を実現。Later で exponential に拡張可能。

### 再起動耐性（Stuck Job 対策）

**⚠️ 3状態設計でのリトライ戦略**:
- Worker が失敗したら **直接 `status='pending'` に戻す**（retry_count + next_retry_at で管理）
- Recovery は **Stuck Job だけに専念**（processing のまま放置されたジョブを pending に戻す）
- 永続エラー or retry上限到達 → `status='completed'` + `final_method='manual_review'`

**3状態設計のメリット**:
- Worker の取得条件が `status='pending' AND next_retry_at <= now()` だけで済む
- Recovery の役割が「stuck job を pending に戻す」だけに単純化
- 「エラージョブが溜まって放置」が起きにくい（completed で終結するため）

**問題**: LLM API 呼び出し中（2-5秒）に worker がクラッシュすると、status='processing' のまま放置される（Stuck Job）。

**対策の設計（systemd timer）**:
- **判定閾値**: `processing_started_at` から **5分** 経過した processing ジョブを Stuck Job と判定
- **実行周期**: systemd timer が **10分間隔** で Recovery タスクを実行（MVP の体感を守る）
- **リトライ上限**: retry_count < 3 まで自動リトライ（3回失敗したら completed + manual_review でクローズ）

```sql
-- Recovery タスク（systemd timer で 10分間隔実行）
-- sr-queue-recovery/src/main.rs で実装

-- Stuck Job を pending に戻す（worker クラッシュ対策のみ）
-- ⚠️ processing_started_at = NULL は OK（pending に戻すので、再取得時に新しい値がセットされる）
--    completed に移行する場合のみ残す必要がある（KPI計算用）
UPDATE ses.extraction_queue
SET
    status = 'pending',
    locked_by = NULL,
    processing_started_at = NULL,
    retry_count = retry_count + 1,
    next_retry_at = NOW() + INTERVAL '5 minutes'  -- fixed backoff（MVP）
WHERE status = 'processing'
  AND processing_started_at < NOW() - INTERVAL '5 minutes';  -- 判定閾値
```

**Note**:
- 判定閾値（5分）は MVP での値。Phase 3 で LLM latency p99 + バッファで調整。
- 実行周期（10分）は最悪 10〜15分遅延。**hourly（1時間）だと体感が悪すぎる**ため短縮。
- Later で 5分間隔に縮めることも可能（頻繁すぎると DB 負荷）。
- **Fixed backoff（5分）**により、LLM API の一時的な障害に対応（Later で exponential に拡張可能）。

**簡易サーキットブレーカー（Recovery 暴走防止）**:

LLM API が長時間停止している場合、Recovery が pending に戻しても Worker が即失敗し、また Recovery が戻す…という暴走ループになる可能性がある。これを防ぐための簡易サーキットブレーカー:

```rust
// sr-queue-recovery/src/main.rs

use std::time::{Duration, Instant};
use tokio_postgres::Client;

/// 簡易サーキットブレーカー状態
/// ⚠️ MVP では DB に永続化せずインメモリで管理（timer は 10分間隔なのでリセットされる前提）
/// Later: extraction_queue に llm_unavailable_until カラムを追加して永続化
struct CircuitBreaker {
    consecutive_failures: u32,
    threshold: u32,           // 閾値（例: 5件連続失敗で発動）
    cooldown: Duration,       // クールダウン（例: 30分）
    tripped_at: Option<Instant>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            consecutive_failures: 0,
            threshold: 5,
            cooldown: Duration::from_secs(30 * 60), // 30分
            tripped_at: None,
        }
    }

    fn is_open(&self) -> bool {
        if let Some(tripped) = self.tripped_at {
            tripped.elapsed() < self.cooldown
        } else {
            false
        }
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.threshold {
            self.tripped_at = Some(Instant::now());
            tracing::warn!(
                "Circuit breaker OPEN: {} consecutive LLM failures, cooldown {}min",
                self.consecutive_failures,
                self.cooldown.as_secs() / 60
            );
        }
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.tripped_at = None;
    }
}

/// Recovery メイン処理（サーキットブレーカー付き）
pub async fn run_recovery(db: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 直近の LLM エラー率をチェック（簡易版: last_error に "rate" or "timeout" が含まれる件数）
    let recent_failures = db.query(
        "SELECT COUNT(*) FROM ses.extraction_queue
         WHERE status = 'processing'
           AND last_error IS NOT NULL
           AND updated_at > NOW() - INTERVAL '10 minutes'",
        &[]
    ).await?;

    let failure_count: i64 = recent_failures[0].get(0);

    // 2. 閾値を超えていたらスキップ（暴走防止）
    if failure_count >= 5 {
        tracing::warn!(
            "Recovery SKIPPED: {} recent LLM failures detected, waiting for API recovery",
            failure_count
        );
        return Ok(());
    }

    // 3. 通常の Recovery 処理
    let affected = db.execute(
        "UPDATE ses.extraction_queue
         SET status = 'pending',
             locked_by = NULL,
             processing_started_at = NULL,
             retry_count = retry_count + 1,
             next_retry_at = NOW() + INTERVAL '5 minutes'
         WHERE status = 'processing'
           AND processing_started_at < NOW() - INTERVAL '5 minutes'",
        &[]
    ).await?;

    tracing::info!("Recovery: {} stuck jobs reset to pending", affected);
    Ok(())
}
```

**ポイント**:
- 直近10分の LLM エラーが5件以上なら Recovery をスキップ
- API が復旧するまで pending に戻さない → Worker が無駄に失敗しない
- MVP では簡易実装、Later で本格的な Circuit Breaker パターンに拡張可能

**ステータス遷移まとめ（B案）**:
| 状態 | 遷移先 | 条件 |
|------|--------|------|
| pending | processing | Worker がジョブを取得 |
| processing | pending | 一時エラー（5xx等）＆ retry_count < 3 |
| processing | completed | 正常完了 |
| processing | completed (manual_review) | retry上限到達 or 永続エラー（400/401） |
| processing | pending | Stuck Job（Recovery が戻す） |

### 起動方式（バッチ + 常駐 + timer）

**sr-extractor（バッチ実行）**:
```bash
# systemd timer で定期実行（5分に1回）
# /etc/systemd/system/sr-extractor.timer
[Timer]
OnCalendar=*:0/5  # 5分ごと
Persistent=true

# /etc/systemd/system/sr-extractor.service
[Service]
Type=oneshot
ExecStart=/usr/local/bin/sr-extractor
```

**ポリシー**:
- バッチ実行で anken_emails から未処理メールを取得し queue に積む
- 常駐不要（queue に積むだけ）
- 実行頻度は MVP では 5分ごと、Later で調整可能

**sr-llm-worker（常駐ワーカー・メイン）**:
```bash
# systemd service として常駐
# /etc/systemd/system/sr-llm-worker.service
[Service]
ExecStart=/usr/local/bin/sr-llm-worker
Restart=always
```

**ポリシー**:
- 常駐で queue をポーリング＆処理（キュー処理は遅延が価値を壊すため常駐が必須）
- worker 数は MVP では 1、Later で増やせる

**Recovery タスク（systemd timer・保険・10分間隔）**:
```bash
# /etc/systemd/system/sr-queue-recovery.timer
[Timer]
OnCalendar=*:0/10  # 10分ごと（0分, 10分, 20分, ...）
Persistent=true

# /etc/systemd/system/sr-queue-recovery.service
[Service]
Type=oneshot
ExecStart=/usr/local/bin/sr-queue-recovery
```

**Recovery タスクの役割**:
1. **Stuck Job 対策**: processing_started_at が 5分以上前で processing のジョブを pending に戻す
2. **リトライ上限チェック**: retry_count >= 3 のジョブを completed + manual_review でクローズ
3. **運用介入不要**: 10分ごとに自動で回復 → 「落ちても戻る」が完成

**Note**: 3状態設計では Worker が直接 pending に戻すため、Recovery の役割は Stuck Job 対策に限定される。

**ポリシー**:
- **10分間隔が MVP の無難ライン**（最悪 10〜15分で回復、hourly だと体感が悪すぎる）
- 常駐プロセス不要（systemd timer でバッチ実行）= 事故りにくい
- Later で 5分間隔に縮めることも可能（頻繁すぎると DB 負荷）
- Fixed backoff（5分）により、LLM API の一時的な障害に自動対応

**廃止はしない（保険として常に動作）**:
- Recovery は「worker が全員健在でも問題なく動く」設計
- 廃止すると、worker 全員クラッシュ時に詰む（保険がない）

**頻度調整の判断基準（Later）**:

```sql
-- 直近30日で stuck_count が 0 かつ processing の最古が閾値内なら、頻度を落とせる
SELECT
    COUNT(*) FILTER (WHERE status = 'processing' AND processing_started_at < NOW() - INTERVAL '5 minutes') AS stuck_count,
    MIN(processing_started_at) FILTER (WHERE status = 'processing') AS oldest_processing
FROM ses.extraction_queue
WHERE created_at > NOW() - INTERVAL '30 days';

-- stuck_count = 0 が30日続いた → 10分 → 30分 に緩和
-- stuck_count > 0 が頻発 → 10分 → 5分 に強化
```

| 状況 | Recovery 頻度 | 理由 |
|------|--------------|------|
| MVP（初期） | 10分 | 安全マージン |
| 安定運用（30日間 stuck=0） | 30分 | DB 負荷軽減 |
| 不安定（stuck 頻発） | 5分 | 回復速度優先 |

---

## Rust実装設計（3-binary構成）

### クレート構成（最小化）

```
sr-extraction/
├── Cargo.toml (workspace)
├── sr-common/                # 純粋関数 + 共通型（DB/HTTP に触れない）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs          # ExtractionResult, EmailInput, FinalResult等
│       ├── decision.rs       # DecisionPolicy, final_decision_with_env, decide_from_quality_with_env
│       └── normalize.rs      # normalize_subject, skill normalization等
│
├── sr-extractor/             # Binary 1: メイン抽出サービス（バッチ・5分ごと）
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # エントリーポイント
│       ├── extractors/       # 抽出ロジック
│       │   ├── mod.rs
│       │   ├── tanka.rs
│       │   ├── date.rs
│       │   ├── location.rs
│       │   ├── remote_onsite.rs
│       │   └── flow_dept.rs
│       ├── decision.rs       # LLM判定ロジック
│       └── db.rs             # PostgreSQL操作
│
├── sr-llm-worker/            # Binary 2: LLMワーカー（常駐）
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # キューポーリング
│       ├── llm_client.rs     # Python/LLM呼び出し
│       ├── light_rules.rs    # 超軽量ルール抽出
│       └── db.rs             # キュー更新
│
└── sr-queue-recovery/        # Binary 3: Stuck Job 対策（バッチ・10分ごと）
    ├── Cargo.toml
    └── src/
        ├── main.rs           # Stuck Job を pending に戻す
        └── db.rs             # キュー更新
```

**ポイント**:
- **3バイナリのみ**: sr-extractor（抽出・バッチ）、sr-llm-worker（LLM処理・常駐）、sr-queue-recovery（保険・バッチ）
- **systemd timer ハイブリッド**: extractor/recovery は常駐不要（事故りにくい）、worker は常駐必須（遅延が価値を壊す）
- **sr-common の設計原則**:
  - ✅ **純粋関数（DB/HTTP に触れない）**: decision.rs, normalize.rs, types.rs
  - ✅ **共通型**: PartialFields, FinalResult, ExtractionQuality, DecisionPolicy
  - ❌ **DB アクセス**: 各バイナリの db.rs に配置
  - ❌ **HTTP/LLM 呼び出し**: 各バイナリの llm_client.rs に配置
- **スパゲッティ防止**: 各バイナリは独立して動作、依存最小化

---

### 依存クレート（最小限）

```toml
[workspace]
members = ["sr-common", "sr-extractor", "sr-llm-worker", "sr-queue-recovery"]

# sr-common/Cargo.toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# sr-extractor/Cargo.toml
[dependencies]
sr-common = { path = "../sr-common" }
regex = "1.10"
lazy_static = "1.4"
chrono = "0.4"
tokio-postgres = "0.7"
tokio = { version = "1", features = ["full"] }

# sr-llm-worker/Cargo.toml
[dependencies]
sr-common = { path = "../sr-common" }
tokio-postgres = "0.7"
tokio = { version = "1", features = ["full"] }
reqwest = "0.11"  # HTTP client for Python API

# 追加（light_rules と required_skills JSON生成のため）
regex = "1.10"
serde_json = "1.0"
lazy_static = "1.4"  # 正規表現を毎回compileしないため（or once_cell = "1.14"）

# sr-queue-recovery/Cargo.toml
[dependencies]
sr-common = { path = "../sr-common" }
tokio-postgres = "0.7"
tokio = { version = "1", features = ["full"] }
```

---

### 非同期ランタイム（tokio）の扱い方針

本システムでは `tokio` を以下の目的に限定して使用する。

#### 使用目的
- 非同期DBアクセス（tokio-postgres）
- 非同期HTTP通信（LLM API 呼び出し、reqwest）
- 非同期sleep（ポーリング間隔制御）

#### 明示的に行わないこと
- worker 内での並列処理（tokio::spawn / join_all 等）
- アプリケーション側での同時実行制御
- チャネル（mpsc等）によるジョブ管理
- backpressure やレート制御の実装

#### 設計理由
- 同時実行制御・順序保証・冪等性は **PostgreSQL のロックと制約で担保**する
- worker のスケールは **プロセス数（systemd / container）**で制御する
- 非同期ランタイムは「I/O待ちをブロックしないための基盤」に留める

#### 方針まとめ
tokio は **利用するが、信頼しすぎない**。
ビジネスロジックの正しさは DB に集約する。

---

### コア型定義と純粋関数（sr-common）

**配置ルール**: DB/HTTP に触れない純粋関数と共通型のみ

```rust
// sr-common/src/types.rs

use serde::{Deserialize, Serialize};

/// Partial抽出結果（sr-extractor が生成）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialFields {
    // Tier 1: Critical Fields
    pub monthly_tanka_min: Option<u32>,
    pub monthly_tanka_max: Option<u32>,
    pub settlement_range: Option<String>,  // 精算幅（例: "140-180h"）
    pub start_date_raw: Option<String>,  // 「即日」「12月中」等、そのまま
    pub work_todofuken: Option<String>,

    // Tier 2: Important Fields
    pub remote_onsite: Option<String>,
    pub flow_dept: Option<String>,

    // Tier 3: Nice-to-Have (後段で束ねる)
    pub project_name: Option<String>,

    // 将来用（MVPでは固定値）
    pub outcome_tag: Option<String>,         // "unknown"
    pub decline_reason_tag: Option<String>,  // null
}

/// 抽出品質スコア
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionQuality {
    pub tier1_extracted: usize,
    pub tier1_total: usize,
    pub tier2_extracted: usize,
    pub tier2_total: usize,
    pub llm_recommended: bool,  // LLM 推奨かどうか
    pub reason: String,
}

/// sr-extractor の出力（queue に積む）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractorOutput {
    pub partial: PartialFields,
    pub quality: ExtractionQuality,
    pub recommended_method: String,  // "rust_recommended" or "llm_recommended"
}

/// 入力メール
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailInput {
    pub subject: String,
    pub body_text: String,
    pub message_id: String,
}

/// 最終結果（DB INSERT 用）
/// ⚠️ start_date は Worker で正規化が必要（normalize_start_date() 使用）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinalResult {
    // Tier 1: Critical Fields
    pub monthly_tanka_min: Option<u32>,
    pub monthly_tanka_max: Option<u32>,
    pub settlement_range: Option<String>,  // 精算幅（例: "140-180h"）

    // 【High 10 改訂】start_date 3カラム構成
    pub start_date_raw: Option<String>,        // TEXT: 入力値（真実）"来月", "即日"
    pub start_date: Option<chrono::NaiveDate>, // DATE: 検索用（正規化後）
    pub start_date_precision: Option<String>,  // VARCHAR: 'exact', 'month', 'asap', 'unknown'

    pub work_todofuken: Option<String>,   // ENUM: 補正済み値

    // Tier 2: Important Fields
    pub remote_onsite: Option<String>,    // ENUM: 補正済み値
    pub flow_dept: Option<String>,        // ENUM: 補正済み値

    // Tier 3: Required for DB
    pub project_name: Option<String>,     // NOT NULL: fallback あり
    pub required_skills: Option<serde_json::Value>,  // JSONB NOT NULL
    pub required_skills_keywords: Option<Vec<String>>,  // TEXT[]
}

impl From<PartialFields> for FinalResult {
    fn from(partial: PartialFields) -> Self {
        FinalResult {
            monthly_tanka_min: partial.monthly_tanka_min,
            monthly_tanka_max: partial.monthly_tanka_max,
            // 【High 10 改訂】raw だけコピー、正規化は Worker で行う
            start_date_raw: partial.start_date_raw,
            start_date: None,           // Worker で normalize_start_date() 後にセット
            start_date_precision: None, // Worker で normalize_start_date() 後にセット
            work_todofuken: partial.work_todofuken,
            remote_onsite: partial.remote_onsite,
            flow_dept: partial.flow_dept,
            project_name: partial.project_name,
            required_skills: None,  // LLM から取得、または空の {"items": []}
            required_skills_keywords: None,  // LLM から取得、または空配列
        }
    }
}
```

---

### sr-extractor 実装例

```rust
// sr-extractor/src/main.rs

use sr_common::types::*;
mod extractors;
mod decision;
mod db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // DB接続
    let db_client = db::connect().await?;

    // anken_emailsからmessage取得
    let emails = db::fetch_pending_emails(&db_client).await?;

    for email in emails {
        // 抽出処理（body_text は anken_emails に保存済み）
        let result = extract_all_fields(&email);

        // priority 計算（Tier1 欠損度合いで優先度を決定）
        let priority = calculate_priority(&result.quality);

        // 重要: 全てのメールを extraction_queue に積む
        // body_text は queue に入れず、worker が anken_emails から取得
        // projects_enum への INSERT は sr-llm-worker が行う
        db::enqueue_to_extraction_queue(
            &db_client,
            &email.message_id,
            &email.subject,
            priority,
            &result
        ).await?;
    }

    Ok(())
}

fn extract_all_fields(email: &EmailInput) -> ExtractorOutput {
    let mut partial = PartialFields::default();

    // Tier 1抽出
    if let Some((min, max)) = extractors::tanka::extract(&email.body_text) {
        partial.monthly_tanka_min = Some(min);
        partial.monthly_tanka_max = Some(max);
    }
    // Note: tanka::extract は「単発値（80万円）」も min=max として返す（Tier1 落とさない）

    partial.start_date_raw = extractors::date::extract_raw(&email.body_text);  // 「即日」等そのまま
    partial.work_todofuken = extractors::location::extract(&email.body_text);

    // Tier 2抽出
    partial.remote_onsite = extractors::remote_onsite::extract(&email.body_text);
    partial.flow_dept = extractors::flow_dept::extract(&email.body_text);

    // 将来用（MVPでは固定値）
    partial.outcome_tag = Some("unknown".to_string());
    partial.decline_reason_tag = None;  // NULL

    // 品質評価（LLM推奨判定 = 見積もり）
    let quality = calculate_quality(&partial);
    let decision = decision::decide_from_quality(&quality);  // sr-common::decision

    let recommended_method = if decision.needs_llm {
        "llm_recommended".to_string()
    } else {
        "rust_recommended".to_string()
    };

    ExtractorOutput {
        partial,
        quality: ExtractionQuality {
            tier1_extracted: quality.tier1_extracted,
            tier1_total: quality.tier1_total,
            tier2_extracted: quality.tier2_extracted,
            tier2_total: quality.tier2_total,
            llm_recommended: decision.needs_llm,
            reason: decision.reason,
        },
        recommended_method,
    }
}

fn calculate_quality(partial: &PartialFields) -> ExtractionQuality {
    let tier1_extracted = [
        partial.monthly_tanka_min.is_some(),
        partial.monthly_tanka_max.is_some(),
        partial.start_date_raw.is_some(),  // 文字列として取れればOK
        partial.work_todofuken.is_some(),
    ].iter().filter(|&&x| x).count();

    let tier2_extracted = [
        partial.remote_onsite.is_some(),
        partial.flow_dept.is_some(),
    ].iter().filter(|&&x| x).count();

    ExtractionQuality {
        tier1_extracted,
        tier1_total: 4,
        tier2_extracted,
        tier2_total: 2,
        llm_recommended: false,  // 推奨判定（workerが確定）
        reason: String::new(),
    }
}

fn calculate_priority(quality: &ExtractionQuality) -> i32 {
    // ⚠️ priority は「処理順序」であり「品質スコア」ではない
    // LLM判定ロジックとは独立（混ぜない）
    //
    // 設計意図:
    // - Tier1欠損: LLM必須なので最優先で処理
    // - Tier2=0: LLM推奨、中優先
    // - Tier2=1: LLM不要寄り、やや低優先
    // - Tier2=2/2: 軽量ルール成功率が高い → 最も低優先（後回しでOK）
    if quality.tier1_extracted < quality.tier1_total {
        100  // LLM 必須 → 最優先
    } else if quality.tier2_extracted == 0 {
        50   // Tier2 = 0/2 → 中優先
    } else if quality.tier2_extracted == 1 {
        20   // Tier2 = 1/2 → やや低優先
    } else {
        10   // Tier2 = 2/2 → 低優先（軽量ルール十分）
    }
}
```

**sr-extractor が拾う対象メールの定義（重要）**:

```sql
-- db::fetch_pending_emails() の実装SQL
-- ⚠️ B案: extractor は「queue に存在しない」メールだけを対象にする
-- リトライは Worker が pending + next_retry_at で回す（extractor は関与しない）
SELECT
    ae.message_id,
    ae.subject,
    ae.body_text,
    ae.created_at
FROM ses.anken_emails ae
LEFT JOIN ses.extraction_queue eq ON ae.message_id = eq.message_id
WHERE
    -- queue に存在しない（未処理）メールのみ対象
    eq.id IS NULL
ORDER BY ae.created_at DESC  -- 最新メールから処理
LIMIT 100;  -- バッチサイズ（MVP: 100件/回）
```

**ポリシー（B案）**:
- `status='completed'` は絶対に再処理しない（idempotent enqueue の CASE文で保護）
- `status='processing'` は Recovery タスクが pending に戻す（Stuck Job 対策）
- `status='pending'` のリトライは **Worker が next_retry_at で管理**（extractor は関与しない）
- ⚠️ ステータスは3状態のみ（pending / processing / completed）

**バッチ処理の流れ**:
1. sr-extractor が定期実行（cron/systemd timer）で起動
2. `fetch_pending_emails()` で未処理メール100件を取得
3. 各メールを `enqueue_to_extraction_queue()` で queue に積む（idempotent）
4. sr-llm-worker が常駐で queue をポーリング＆処理

### enqueue_to_extraction_queue() 実装（条件付きUPDATE版）

**基本方針**:
- `DO NOTHING` だけだと「ルール改善で partial_fields を更新して再実行したい」ができない
- `status='completed'` なら上書きしない（確定済みレコードを守る）
- それ以外（`pending`, `processing`）なら**最新の抽出結果で上書き**（3状態設計）
- **body_text は queue に入れない**（anken_emails から都度取得）

**メリット**:
- Phase 2 でルール改善後に「過去メールで再抽出」が可能
- completed の既存データを壊さない（安全）
- queue が軽い（body_text を保持しない）

```rust
// ⚠️ normalize_subject は sr-common に配置（重複実装を防ぐ）
// sr-common/src/normalize.rs

use sha2::{Sha256, Digest};
use once_cell::sync::Lazy;
use regex::Regex;

/// ⚠️ sr-common に配置: subject_hash, project_name fallback, dedup 等で共用
/// 破壊力が高いので、ここだけで管理する
///
/// 正規表現のポイント:
/// - `(?i)` = case insensitive
/// - `(?:RE|FW|FWD):\s*` = RE: / FW: / Fwd: + 任意の空白
/// - 外側の `(?:...)+` = 1回以上の繰り返し（多重転送対応）
/// - `^` = 先頭アンカー
static RE_PREFIX: Lazy<Regex> = Lazy::new(|| {
    // "RE: RE: FW: 案件" → "RE: RE: FW: " を1回でマッチ
    Regex::new(r"(?i)^(?:(?:RE|FW|FWD):\s*)+").unwrap()
});
static RE_BRACKETS: Lazy<Regex> = Lazy::new(|| {
    // "【案件】Java開発" → "Java開発"
    // "【案件】【急募】Java開発" → "Java開発"（複数タグ対応）
    // ⚠️ 外側の (?:...)+ で連続タグをまとめて除去
    // ⚠️ \s* で括弧間の空白も許容（「【案件】 【急募】」等）
    Regex::new(r"^(?:[【\[（(][^】\]）)]*[】\]）)]\s*)+").unwrap()
});

/// ⚠️ 【関数契約】空文字列を返さない（空になるなら元に戻す）
///
/// 処理順序:
/// 1. prefix除去後 → s1
/// 2. bracket除去後 → s2
/// 3. s2.trim() が空なら s1.trim() を返す
/// 4. それも空なら元 subject.trim() を返す
/// 5. 元も空なら空（入力が空の場合のみ許容）
pub fn normalize_subject(subject: &str) -> String {
    let original_trimmed = subject.trim();

    // 1. 複数回の RE:/FW:/Fwd: を1回で剥がす
    let s1 = RE_PREFIX.replace(subject, "");
    let s1_trimmed = s1.trim();

    // 2. 先頭の【案件】等を剥がす
    let s2 = RE_BRACKETS.replace(s1_trimmed, "");
    let s2_trimmed = s2.trim();

    // 3. 空になったら段階的に戻す
    if !s2_trimmed.is_empty() {
        s2_trimmed.to_string()
    } else if !s1_trimmed.is_empty() {
        // bracket除去で空になった → prefix除去後に戻す
        // 例: "Fwd:【案件】" → "【案件】"
        s1_trimmed.to_string()
    } else {
        // prefix除去でも空 → 元のsubjectに戻す
        // 例: "RE: " → "RE: " (trim済み)
        original_trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_subject_multiple_prefixes() {
        // 多重転送（Critical 3 の対象ケース）
        assert_eq!(normalize_subject("RE: RE: 【案件】Java開発"), "Java開発");
        assert_eq!(normalize_subject("re: Re: Fwd: 案件紹介"), "案件紹介");
        assert_eq!(normalize_subject("FW: FW: FW: お知らせ"), "お知らせ");
    }

    #[test]
    fn test_normalize_subject_brackets() {
        assert_eq!(normalize_subject("【急募】Python開発"), "Python開発");
        assert_eq!(normalize_subject("[案件] Ruby開発"), "Ruby開発");
    }

    #[test]
    fn test_normalize_subject_multiple_brackets() {
        // ⚠️ Critical 5: 複数連続タグを1回で除去
        assert_eq!(normalize_subject("【案件】【急募】Java開発"), "Java開発");
        assert_eq!(normalize_subject("[info][urgent] Ruby案件"), "Ruby案件");
        assert_eq!(normalize_subject("【BP】（急募）Python開発"), "Python開発");
        // 括弧間の空白も許容
        assert_eq!(normalize_subject("【案件】 【急募】 Java開発"), "Java開発");
        assert_eq!(normalize_subject("[info] [urgent] Ruby案件"), "Ruby案件");
    }

    #[test]
    fn test_normalize_subject_combined() {
        assert_eq!(normalize_subject("RE: 【案件】Java開発"), "Java開発");
        assert_eq!(normalize_subject("Fwd: [急募] Python開発"), "Python開発");
    }

    #[test]
    fn test_normalize_subject_edge_cases() {
        // 空入力 → 空出力（これだけは許容）
        assert_eq!(normalize_subject(""), "");
        // prefix除去で空になる → 元に戻す
        assert_eq!(normalize_subject("RE: "), "RE:");
        assert_eq!(normalize_subject("FW:"), "FW:");
        // 通常ケース
        assert_eq!(normalize_subject("Java開発案件"), "Java開発案件");
    }

    #[test]
    fn test_normalize_subject_fallback_to_bracket() {
        // bracket除去で空になる → bracket を残す
        assert_eq!(normalize_subject("Fwd:【案件】"), "【案件】");
        assert_eq!(normalize_subject("RE: [info]"), "[info]");
        assert_eq!(normalize_subject("FW: （お知らせ）"), "（お知らせ）");
    }

    // ======== Issue #11: 追加テストケース ========

    #[test]
    fn test_normalize_subject_fullwidth_chars() {
        // 全角英字のRE/FW（実務で稀に出現）
        assert_eq!(normalize_subject("ＲＥ：案件紹介"), "案件紹介");
        assert_eq!(normalize_subject("ＦＷ：Java開発"), "Java開発");
        // 全角コロンのみ
        assert_eq!(normalize_subject("RE：Python案件"), "Python案件");
    }

    #[test]
    fn test_normalize_subject_whitespace_variants() {
        // タブや全角スペース
        assert_eq!(normalize_subject("RE:\t【案件】Java開発"), "Java開発");
        assert_eq!(normalize_subject("FW:　【急募】Ruby案件"), "Ruby案件");  // 全角スペース
        // 末尾空白
        assert_eq!(normalize_subject("Java開発案件  "), "Java開発案件");
    }

    #[test]
    fn test_normalize_subject_mixed_brackets() {
        // 異なる種類の括弧が混在
        assert_eq!(normalize_subject("【案件】(急募) Java"), "Java");
        assert_eq!(normalize_subject("[BP]《急募》Python"), "Python");
        assert_eq!(normalize_subject("〔info〕〈urgent〉Ruby案件"), "Ruby案件");
    }

    #[test]
    fn test_normalize_subject_real_world_examples() {
        // 実際のメール件名パターン（sponto-platform 実データ参考）
        assert_eq!(normalize_subject("RE: RE: 【BP】【案件】Java SE 80万 東京"), "Java SE 80万 東京");
        assert_eq!(normalize_subject("Fwd: [partner-info] Python/Django 70-85万"), "Python/Django 70-85万");
        assert_eq!(normalize_subject("【急募】【即日】【フルリモート】PM案件"), "PM案件");
        // 日付入りタグ
        assert_eq!(normalize_subject("【12/15】【新着】AWS案件"), "AWS案件");
    }

    #[test]
    fn test_normalize_subject_unicode_safety() {
        // 絵文字を含む件名（破壊しないこと）
        assert_eq!(normalize_subject("RE: 🔥急募🔥 Java案件"), "🔥急募🔥 Java案件");
        // 特殊Unicode文字
        assert_eq!(normalize_subject("【案件】Ⅰ期開発"), "Ⅰ期開発");
    }
}

// sr-extractor/src/db.rs（sr-common を使う）
use sr_common::normalize::normalize_subject;

fn calculate_subject_hash(subject: &str) -> String {
    let normalized = normalize_subject(subject);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let result = hasher.finalize();
    // GenericArray<u8, 32> → hex 文字列（64文字）→ 先頭16文字
    // ⚠️ hex crate または手動変換が必要
    // sha2 の finalize() は GenericArray を返すので、直接 format!("{:x}") は使えない
    let hex_string: String = result.iter().map(|b| format!("{:02x}", b)).collect();
    hex_string[..16].to_string()
}

/// ⚠️ ソフトdedup: subject_hash が過去N日で存在するかチェック
/// - 存在しても enqueue はする（完全取りこぼしを防ぐ）
/// - 重複疑いなら decision_reason に追記 + priority を下げる
async fn check_soft_dedup(
    db_client: &tokio_postgres::Client,
    subject_hash: &str,
    message_id: &str
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    const DEDUP_WINDOW_DAYS: i32 = 7;  // 過去7日間で重複チェック

    let row = db_client.query_opt(
        "SELECT message_id FROM ses.extraction_queue
         WHERE subject_hash = $1
           AND message_id != $2
           AND created_at >= NOW() - INTERVAL '7 days'
         ORDER BY created_at DESC
         LIMIT 1",
        &[&subject_hash, &message_id]
    ).await?;

    if let Some(row) = row {
        let existing_msg_id: String = row.get(0);
        Ok(Some(existing_msg_id))  // 重複疑いあり
    } else {
        Ok(None)  // 重複なし
    }
}

pub async fn enqueue_to_extraction_queue(
    db_client: &tokio_postgres::Client,
    message_id: &str,
    email_subject: &str,
    priority: i32,
    output: &ExtractorOutput
) -> Result<(), Box<dyn std::error::Error>> {
    let partial_fields = serde_json::to_value(&output.partial)?;
    let subject_hash = calculate_subject_hash(email_subject);
    let extractor_version = env!("CARGO_PKG_VERSION");
    let rule_version = "2025-01-15";  // ルール更新時に変更

    // ⚠️ ソフトdedup: 重複チェック（排除はしない、priority を下げるだけ）
    let mut final_priority = priority;
    let mut decision_reason = output.quality.reason.clone();

    if let Some(existing_msg_id) = check_soft_dedup(db_client, &subject_hash, message_id).await? {
        // 重複疑いあり → priority を下げて decision_reason に追記
        final_priority = (priority - 10).max(0);  // 最低0
        decision_reason = format!(
            "{}; duplicate_suspected(existing={})",
            decision_reason, &existing_msg_id[..existing_msg_id.len().min(20)]
        );
        debug!("Soft dedup: {} suspected duplicate of {}", message_id, existing_msg_id);
    }

    // UPSERT: completed なら何もしない、それ以外は上書き
    db_client.execute(
        "INSERT INTO ses.extraction_queue (
            message_id,
            email_subject,
            subject_hash,
            partial_fields,
            priority,
            decision_reason,
            recommended_method,
            status,
            extractor_version,
            rule_version,
            created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8, $9, NOW())
        ON CONFLICT (message_id) DO UPDATE SET
            email_subject = EXCLUDED.email_subject,
            subject_hash = EXCLUDED.subject_hash,
            partial_fields = EXCLUDED.partial_fields,
            priority = EXCLUDED.priority,
            decision_reason = EXCLUDED.decision_reason,
            recommended_method = EXCLUDED.recommended_method,
            extractor_version = EXCLUDED.extractor_version,
            rule_version = EXCLUDED.rule_version,
            updated_at = NOW()
        WHERE ses.extraction_queue.status != 'completed'",  -- ← completedは守る
        &[
            &message_id,
            &email_subject,
            &subject_hash,
            &partial_fields,
            &final_priority,      // ⚠️ ソフトdedup で調整済み
            &decision_reason,     // ⚠️ duplicate_suspected が追記されている可能性あり
            &output.recommended_method,
            &extractor_version,
            &rule_version
        ]
    ).await?;

    Ok(())
}
```

**運用ケース**（3状態設計）:
1. **初回実行**: message_id が未登録 → INSERT
2. **pending/processing 中に再実行**: 最新の抽出結果で上書き（ルール改善後の値が反映）
3. **completed 済み**: 何もしない（既存データを守る、manual_review 含む）

**Phase 2 での使い方**:
```bash
# ルール改善後、過去30日のメールで再抽出
$ cargo run --bin sr-extractor -- --reprocess-days 30
# → status='completed' 以外が最新ルールで更新される
```

---

### 抽出ロジック実装例（extractors::tanka）

**重要**: 単発値（「80万円」）も min=max として返すことで、Tier1 を落とさない。

```rust
// sr-extractor/src/extractors/tanka.rs

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // レンジ: "70〜90万円" / "70-90万円" / "70万円〜90万円"
    static ref RANGE_RE: Regex = Regex::new(
        r"(\d{1,3})\s*[〜～~-]\s*(\d{1,3})\s*万円"
    ).unwrap();

    // 下限のみ: "70万円〜" / "70万円以上"
    static ref MIN_ONLY_RE: Regex = Regex::new(
        r"(\d{1,3})\s*万円\s*(?:〜|以上)"
    ).unwrap();

    // 上限のみ: "〜90万円" / "90万円まで"
    static ref MAX_ONLY_RE: Regex = Regex::new(
        r"(?:〜|まで)\s*(\d{1,3})\s*万円"
    ).unwrap();

    // 単発: "80万円" / "80万円程度"
    static ref SINGLE_RE: Regex = Regex::new(
        r"(\d{1,3})\s*万円(?:程度|くらい|前後)?"
    ).unwrap();
}

/// 月単価を抽出（MVP: レンジ/下限のみ/上限のみ/単発に対応）
/// 単発値は min=max として返す（Tier1 を落とさない）
pub fn extract(body_text: &str) -> Option<(u32, u32)> {
    // 優先順位: レンジ > 下限のみ > 上限のみ > 単発

    // 1. レンジ: "70〜90万円"
    if let Some(caps) = RANGE_RE.captures(body_text) {
        let min: u32 = caps.get(1)?.as_str().parse().ok()?;
        let max: u32 = caps.get(2)?.as_str().parse().ok()?;
        if min <= max && min >= 30 && max <= 200 {  // 妥当性チェック
            return Some((min, max));
        }
    }

    // 2. 下限のみ: "70万円〜"
    if let Some(caps) = MIN_ONLY_RE.captures(body_text) {
        let min: u32 = caps.get(1)?.as_str().parse().ok()?;
        if min >= 30 && min <= 200 {
            // 上限は min + 20 万円と仮定（暫定）
            return Some((min, min + 20));
        }
    }

    // 3. 上限のみ: "〜90万円"
    if let Some(caps) = MAX_ONLY_RE.captures(body_text) {
        let max: u32 = caps.get(1)?.as_str().parse().ok()?;
        if max >= 30 && max <= 200 {
            // 下限は max - 20 万円と仮定（暫定）
            let min = if max > 20 { max - 20 } else { 30 };
            return Some((min, max));
        }
    }

    // 4. 単発: "80万円程度" → min=max として返す（重要）
    if let Some(caps) = SINGLE_RE.captures(body_text) {
        let tanka: u32 = caps.get(1)?.as_str().parse().ok()?;
        if tanka >= 30 && tanka <= 200 {
            return Some((tanka, tanka));  // min=max で Tier1 を満たす
        }
    }

    None
}
```

**Note**:
- 単発値を min=max として返すことで、Tier1（月単価下限/上限）を満たす
- これにより「80万円程度」のようなメールで Tier1 を落とすことがなくなり、**API削減率が向上**
- 妥当性チェック（30〜200万円）でノイズを除去

---

### 日付正規化モジュール（sr-llm-worker）

```rust
// sr-llm-worker/src/normalization.rs

use chrono::{NaiveDate, Datelike};

/// 日付の解像度（精度）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatePrecision {
    Exact,      // YYYY-MM-DD 指定
    Month,      // YYYY-MM 指定 (1日に倒す)
    Early,      // 上旬 (1日に倒す)
    Middle,     // 中旬 (15日に倒す) ← Issue #14 修正: 10→15
    Late,       // 下旬 (25日に倒す) ← Issue #14 修正: 20→25
    Asap,       // "即日", "ASAP" (received_at に倒す)
    Unknown,    // "応相談"など (NULL)
}

impl DatePrecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            DatePrecision::Exact => "exact",
            DatePrecision::Month => "month",
            DatePrecision::Early => "early",
            DatePrecision::Middle => "middle",
            DatePrecision::Late => "late",
            DatePrecision::Asap => "asap",
            DatePrecision::Unknown => "unknown",
        }
    }
}

pub struct NormalizedDate {
    pub date: Option<NaiveDate>,  // 検索・ソート用の代表値
    pub precision: DatePrecision, // データの「解像度」
}

/// 相対日付は base_date (メール受信日) を基準に解決する
/// ⚠️ 1年前のメールを再処理しても「当時の翌月」が正しく算出される
pub fn normalize_start_date(raw: &str, base_date: NaiveDate) -> NormalizedDate {
    let s = raw.trim();

    // 1. "即日"系 -> base_date そのもの
    if s.contains("即日") || s.to_uppercase().contains("ASAP") || s.contains("即時") {
        return NormalizedDate {
            date: Some(base_date),
            precision: DatePrecision::Asap,
        };
    }

    // 2. "来月" -> base_date の翌月1日
    if s.contains("来月") {
        let (y, m) = if base_date.month() == 12 {
            (base_date.year() + 1, 1)
        } else {
            (base_date.year(), base_date.month() + 1)
        };
        return NormalizedDate {
            date: NaiveDate::from_ymd_opt(y, m as u32, 1),
            precision: DatePrecision::Month,
        };
    }

    // 3. "〇月上旬/中旬/下旬" パターン
    if let Some(normalized) = parse_month_part(s, base_date) {
        return normalized;
    }

    // 4. YYYY/MM/DD, YYYY-MM-DD パターン
    if let Some(normalized) = parse_exact_date(s) {
        return normalized;
    }

    // 5. "〇月" のみ（上旬/中旬/下旬なし）
    if let Some(normalized) = parse_month_only(s, base_date) {
        return normalized;
    }

    // デフォルト: 「応相談」等
    NormalizedDate {
        date: None,
        precision: DatePrecision::Unknown,
    }
}

/// "1月上旬", "12月中旬" 等をパース
fn parse_month_part(s: &str, base_date: NaiveDate) -> Option<NormalizedDate> {
    use regex::Regex;
    use once_cell::sync::Lazy;

    static RE_MONTH_PART: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(\d{1,2})月(上旬|中旬|下旬)").unwrap()
    });

    if let Some(caps) = RE_MONTH_PART.captures(s) {
        let month: u32 = caps[1].parse().ok()?;
        let part = &caps[2];

        // 年の推定: 現在月より前なら来年
        let year = if month < base_date.month() {
            base_date.year() + 1
        } else {
            base_date.year()
        };

        let (day, precision) = match part {
            "上旬" => (1, DatePrecision::Early),
            "中旬" => (15, DatePrecision::Middle),  // Issue #14 修正: 10→15
            "下旬" => (25, DatePrecision::Late),    // Issue #14 修正: 20→25
            _ => return None,
        };

        return Some(NormalizedDate {
            date: NaiveDate::from_ymd_opt(year, month, day),
            precision,
        });
    }
    None
}

/// "2025/02/15", "2025-02-15" 等をパース
fn parse_exact_date(s: &str) -> Option<NormalizedDate> {
    use regex::Regex;
    use once_cell::sync::Lazy;

    static RE_EXACT: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(\d{4})[/\-](\d{1,2})[/\-](\d{1,2})").unwrap()
    });

    if let Some(caps) = RE_EXACT.captures(s) {
        let year: i32 = caps[1].parse().ok()?;
        let month: u32 = caps[2].parse().ok()?;
        let day: u32 = caps[3].parse().ok()?;

        return Some(NormalizedDate {
            date: NaiveDate::from_ymd_opt(year, month, day),
            precision: DatePrecision::Exact,
        });
    }
    None
}

/// "1月", "12月中" 等をパース（月単位）
fn parse_month_only(s: &str, base_date: NaiveDate) -> Option<NormalizedDate> {
    use regex::Regex;
    use once_cell::sync::Lazy;

    static RE_MONTH: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(\d{1,2})月").unwrap()
    });

    if let Some(caps) = RE_MONTH.captures(s) {
        let month: u32 = caps[1].parse().ok()?;

        // 年の推定
        let year = if month < base_date.month() {
            base_date.year() + 1
        } else {
            base_date.year()
        };

        return Some(NormalizedDate {
            date: NaiveDate::from_ymd_opt(year, month, 1),
            precision: DatePrecision::Month,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 12, 15).unwrap() // 2025年12月15日
    }

    #[test]
    fn test_asap() {
        let r = normalize_start_date("即日", base());
        assert_eq!(r.date, Some(base()));
        assert_eq!(r.precision, DatePrecision::Asap);
    }

    #[test]
    fn test_next_month() {
        let r = normalize_start_date("来月", base());
        assert_eq!(r.date, NaiveDate::from_ymd_opt(2026, 1, 1));
        assert_eq!(r.precision, DatePrecision::Month);
    }

    #[test]
    fn test_month_early() {
        let r = normalize_start_date("1月上旬", base());
        assert_eq!(r.date, NaiveDate::from_ymd_opt(2026, 1, 1)); // 来年
        assert_eq!(r.precision, DatePrecision::Early);
    }

    #[test]
    fn test_exact_date() {
        let r = normalize_start_date("2025/02/15", base());
        assert_eq!(r.date, NaiveDate::from_ymd_opt(2025, 2, 15));
        assert_eq!(r.precision, DatePrecision::Exact);
    }

    #[test]
    fn test_unknown() {
        let r = normalize_start_date("応相談", base());
        assert_eq!(r.date, None);
        assert_eq!(r.precision, DatePrecision::Unknown);
    }
}
```

---

### マージ・バリデーション関数（sr-llm-worker/src/merge.rs）

```rust
// sr-llm-worker/src/merge.rs
//
// マージ優先順位:
//   1. partial（ルール抽出）を優先 → 高精度だが欠損あり
//   2. LLM は補完用 → 欠損を埋める / 曖昧を解く
// これによりMVPが最も安定する

use sr_common::types::{PartialProject, ProjectFinal};

/// partial に欠損している箇所をデフォルト値で埋める
pub fn merge_partial_with_defaults(mut p: PartialProject) -> PartialProject {
    // NOT NULL 制約があるフィールドのみデフォルト埋め
    if p.required_skills.is_none() {
        p.required_skills = Some(serde_json::json!({"items": []}));
    }
    if p.required_skills_keywords.is_none() {
        p.required_skills_keywords = Some(vec![]);
    }
    if p.remote_onsite.is_none() {
        p.remote_onsite = Some("リモート併用".to_string());
    }
    if p.project_status.is_none() {
        p.project_status = Some("募集中".to_string());
    }
    p
}

/// partial を優先し、欠損部分を LLM 結果で補完
pub fn merge_partial_with_llm(p: PartialProject, llm: PartialProject) -> ProjectFinal {
    ProjectFinal {
        // partial が Some ならそれを採用、None なら LLM
        project_name: p.project_name.or(llm.project_name),
        start_date: p.start_date.or(llm.start_date),
        start_date_raw: p.start_date_raw.or(llm.start_date_raw),
        start_date_precision: p.start_date_precision.or(llm.start_date_precision),
        work_todofuken: p.work_todofuken.or(llm.work_todofuken),
        work_area: p.work_area.or(llm.work_area),
        remote_onsite: p.remote_onsite.or(llm.remote_onsite)
            .unwrap_or_else(|| "リモート併用".to_string()),
        monthly_tanka_min: p.monthly_tanka_min.or(llm.monthly_tanka_min),
        monthly_tanka_max: p.monthly_tanka_max.or(llm.monthly_tanka_max),
        flow_dept: p.flow_dept.or(llm.flow_dept),
        contract_type: p.contract_type.or(llm.contract_type),
        // skills は union（両方から収集）
        required_skills_keywords: {
            let mut s = p.required_skills_keywords.unwrap_or_default();
            s.extend(llm.required_skills_keywords.unwrap_or_default());
            s.sort();
            s.dedup();
            s
        },
        required_skills: p.required_skills.or(llm.required_skills)
            .unwrap_or_else(|| serde_json::json!({"items": []})),
        // その他フィールド
        joi_bp_name: p.joi_bp_name.or(llm.joi_bp_name),
        foreigner_allowed: p.foreigner_allowed.or(llm.foreigner_allowed),
        japanese_skill: p.japanese_skill.or(llm.japanese_skill),
        english_skill: p.english_skill.or(llm.english_skill),
        age_limit_lower: p.age_limit_lower.or(llm.age_limit_lower),
        age_limit_upper: p.age_limit_upper.or(llm.age_limit_upper),
        // メタデータ
        project_status: p.project_status.or(llm.project_status)
            .unwrap_or_else(|| "募集中".to_string()),
        requires_manual_review: false, // 後で validate で判定
        manual_review_reason: None,
    }
}

/// MVP必須フィールドのバリデーション
/// ⚠️ 必須を増やすほど false negative が増えて運用が死ぬ → 最小限に絞る
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("missing required field: {0}")]
    Missing(&'static str),
}

pub fn validate_required_fields(x: &ProjectFinal) -> Result<(), ValidationError> {
    // MVP で真に必須なもののみ（緩め設定）
    // project_name は normalize_subject でフォールバックがあるので OK
    // 単価・スキルは空でも INSERT 可能（requires_manual_review で制御）

    // ⚠️ 現時点では全フィールドが埋まってなくても INSERT 可能
    // 代わりに requires_manual_review フラグで制御
    Ok(())
}

/// 必須フィールド欠損時に requires_manual_review を立てる判定
pub fn should_require_manual_review(x: &ProjectFinal) -> (bool, Option<String>) {
    let mut reasons = vec![];

    // Tier1 フィールドの欠損チェック
    if x.monthly_tanka_min.is_none() && x.monthly_tanka_max.is_none() {
        reasons.push("tanka_missing");
    }
    if x.required_skills_keywords.is_empty() {
        reasons.push("skills_empty");
    }
    if x.work_todofuken.is_none() && x.work_area.is_none() {
        reasons.push("location_missing");
    }
    if x.start_date.is_none() {
        reasons.push("start_date_missing");
    }

    if reasons.is_empty() {
        (false, None)
    } else {
        (true, Some(reasons.join(",")))
    }
}
```

---

### sr-llm-worker 実装例

```rust
// sr-llm-worker/src/main.rs

use sr_common::types::*;
mod llm_client;
mod db;
mod merge;        // 【追加】マージ関数
mod normalization; // 【追加】日付正規化

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_client = db::connect().await?;
    let worker_id = format!("worker-{}", std::process::id());

    // 🚨 ENUM値をDBから読み込み（起動時に1回だけ）
    load_enum_values(&db_client).await?;

    loop {
        // キューから取得（SKIP LOCKED パターン）
        if let Some(job) = db::fetch_and_lock_pending_job(&db_client, &worker_id).await? {
            // Rust partial_fields を取得
            let partial = job.partial_fields;
            let body_text = db::fetch_body_text(&db_client, &job.message_id).await?;

            // 【3段階判定】で最終結果を確定（sr-common::decision を使用）
            let llm_decision = final_decision_with_env(&partial);
            let (final_result, final_method) = if llm_decision.needs_llm {
                // 【段階1】Tier 不足 or SR_FORCE_LLM → LLM 必須
                match llm_client::call_llm(&job.message_id, &body_text, &partial).await {
                    Ok(llm_result) => (merge_partial_with_llm(partial, llm_result), "llm_completed"),
                    Err(e) => {
                        // ⚠️ 3状態設計: handle_job_error() で pending に戻す or completed で終結
                        let class = ErrorClass::from_llm_error(&e);
                        let retry_hint = e.retry_after_hint();  // 429 なら Some(Duration)
                        db::handle_job_error(&db_client, job.id, &e.to_string(), class, retry_hint).await?;
                        continue;
                    }
                }
            } else {
                // 【段階2】Tier OK → 軽量ルールで required_skills 抽出試行
                let skills = light_rules::extract_required_skills_light(&body_text);

                if !skills.is_empty() {
                    // ✅ 軽量ルールで埋められた → LLM 不要
                    let mut result = merge_partial_with_defaults(partial);
                    // NOT NULL 制約を満たす（project_name フォールバック）
                    result.project_name = result.project_name
                        .or_else(|| Some(normalize_subject(&job.email_subject)));

                    // 下流の matching/UI/集計のため、両方のカラムを埋める（重要）
                    result.required_skills = Some(light_rules::skills_to_jsonb(skills.clone()));
                    result.required_skills_keywords = Some(light_rules::skills_to_keywords(skills));

                    (result, "rust_completed")
                } else {
                    // 【段階3】軽量ルール失敗 → LLM フォールバック
                    match llm_client::call_llm(&job.message_id, &body_text, &partial).await {
                        Ok(llm_result) => (merge_partial_with_llm(partial, llm_result), "llm_completed"),
                        Err(e) => {
                            // ⚠️ 3状態設計: handle_job_error() で pending に戻す or completed で終結
                            let class = ErrorClass::from_llm_error(&e);
                            let retry_hint = e.retry_after_hint();  // 429 なら Some(Duration)
                            db::handle_job_error(&db_client, job.id, &e.to_string(), class, retry_hint).await?;
                            continue;
                        }
                    }
                }
            };

            // 【DB制約を必ず満たす】NOT NULL フィールドに fallback 値を埋める（重要）
            let mut filled_result = ensure_db_constraints(final_result, &job.email_subject);

            // 【日付正規化】start_date_raw → start_date (DATE) + precision
            // ⚠️ 相対日付（"来月"等）は email_received_at を基準に解決（SC-2）
            if let Some(raw) = &filled_result.start_date_raw {
                let received_at = job.email_received_at.date_naive(); // メール受信日（queue から取得）
                let normalized = normalization::normalize_start_date(raw, received_at);
                filled_result.start_date = normalized.date;
                filled_result.start_date_precision = Some(normalized.precision.as_str().to_string());
            }

            // 【ENUM 補正】INSERT 失敗の最大原因を防ぐ
            let (corrected_result, enum_correction_failed) = validate_and_correct_enums(filled_result);

            // 【自動マッチ可能な品質か？】ビジネス的な判定（DB制約とは別）
            let is_auto_matchable = is_sufficient_for_auto_matching(&corrected_result);
            let needs_manual_review = !is_auto_matchable || enum_correction_failed || force_manual_review;

            if !needs_manual_review {
                // 自動マッチ可能 → requires_manual_review=false で保存
                db::save_to_projects_enum(&db_client, &job.message_id, &corrected_result, false).await?;
                db::update_job_completed(&db_client, job.id, &final_method).await?;
            } else {
                // 自動マッチ不可 OR ENUM 補正失敗 → requires_manual_review=true で保存
                db::save_to_projects_enum(&db_client, &job.message_id, &corrected_result, true).await?;
                db::update_job_completed(&db_client, job.id, "manual_review").await?;
            }
        } else {
            // キューが空なら1秒待機
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

// ⚠️ needs_llm() は sr-common::decision に移動済み
// 使用例:
//   use sr_common::decision::{final_decision_with_env, decide_from_quality_with_env};
//   if final_decision_with_env(&partial).needs_llm { ... }  // SR_FORCE_LLM 考慮
//   if decide_from_quality_with_env(&quality).needs_llm { ... }

/// DB制約（NOT NULL等）を必ず満たす（fallback値を入れる）
/// これにより INSERT は必ず成功する
fn ensure_db_constraints(mut result: FinalResult, email_subject: &str) -> FinalResult {
    // project_name (NOT NULL) の fallback
    if result.project_name.is_none() {
        result.project_name = Some(normalize_subject(email_subject));
    }

    // required_skills (JSONB NOT NULL) の fallback
    // 既存LLM出力と同じスキーマに合わせる（重要）
    if result.required_skills.is_none() {
        // 空の場合は {"items": []} で埋める（既存スキーマに合わせる）
        result.required_skills = Some(serde_json::json!({ "items": [] }));
    }

    // ⚠️ required_skills_keywords (TEXT[]) の扱い
    //
    // 【重要】strategy.py の挙動:
    //   - 空/NULL → 必須スキル判定がスキップされ "合格" 扱い
    //   - ["未指定"] → 必須スキル判定が走り、ほぼ全員 "不合格" になる
    //
    // 従って ["未指定"] を入れるのは危険！
    // 空のまま残し、requires_manual_review で制御するのが安全。
    if result.required_skills_keywords.is_none() {
        result.required_skills_keywords = Some(vec![]);  // 空配列で統一
    }

    result
}

// 【仕様契約 SC-1】required_skills_keywords が空の場合の扱い
// → 詳細は「仕様契約（Spec Contracts）」セクション参照

// 【仕様契約 SC-2】start_date_raw → start_date 変換仕様
// → 詳細は「仕様契約（Spec Contracts）」セクション参照

/// ビジネス的に自動マッチ可能な品質か？（DB制約とは別）
/// - required_skills_keywords が実質的に埋まっているか
/// - Tier1 フィールドが最低限揃っているか
///
/// 【破綻対策】
/// 空配列でも strategy.py は "合格扱い" にするが、それが常態化すると
/// "何のための抽出？" になり、マッチングの意味が薄くなる。
/// → 空配列なら manual_review=true にして、Guardrail KPI で監視する。
fn is_sufficient_for_auto_matching(result: &FinalResult) -> bool {
    // required_skills_keywords が空なら自動マッチ不可
    // ⚠️ 空の場合、strategy.py は "合格扱い" にするが、
    //    ビジネス的には「スキル不明」なので自動マッチに流さない
    //    → requires_manual_review=true, manual_review_reason='skills_empty' で保護
    if let Some(ref keywords) = result.required_skills_keywords {
        if keywords.is_empty() {
            return false;  // 空 → 自動マッチ不可、manual_review へ
        }
    } else {
        return false;
    }

    // Tier1 フィールドが最低限揃っているか（例: 月単価が必須）
    if result.monthly_tanka_min.is_none() || result.monthly_tanka_max.is_none() {
        return false;
    }

    true
}

// 【仕様契約 SC-3】ENUM 補正失敗時の扱い
// → 詳細は「仕様契約（Spec Contracts）」セクション参照

/// ENUM フィールドの validation と correction（INSERT 失敗防止の最後の砦）
fn validate_and_correct_enums(mut result: FinalResult) -> (FinalResult, bool) {
    let mut requires_review = false;

    // work_todofuken: 都道府県ENUM (例: '東京都' → '東京')
    if let Some(ref todofuken) = result.work_todofuken {
        match correct_todofuken(todofuken) {
            Some(corrected) => {
                result.work_todofuken = Some(corrected);
            }
            None => {
                // ⚠️ Critical 5: 補正失敗 → NULL に落とす（INSERT 失敗防止）
                log::warn!("ENUM correction failed for work_todofuken: {:?}", todofuken);
                result.work_todofuken = None;
                requires_review = true;
            }
        }
    }

    // remote_onsite: リモート形態ENUM (例: 'リモート可' → 'リモート併用')
    if let Some(ref remote) = result.remote_onsite {
        match correct_remote_onsite(remote) {
            Some(corrected) => {
                result.remote_onsite = Some(corrected);
            }
            None => {
                log::warn!("ENUM correction failed for remote_onsite: {:?}", remote);
                result.remote_onsite = None;
                requires_review = true;
            }
        }
    }

    // flow_dept: 商流深さENUM (例: 'エンド直接' → 'エンド直')
    if let Some(ref flow) = result.flow_dept {
        match correct_flow_dept(flow) {
            Some(corrected) => {
                result.flow_dept = Some(corrected);
            }
            None => {
                log::warn!("ENUM correction failed for flow_dept: {:?}", flow);
                result.flow_dept = None;
                requires_review = true;
            }
        }
    }

    (result, requires_review)
}

// 【仕様契約 SC-4】ENUM allowed set の契約化
// → 詳細は「仕様契約（Spec Contracts）」セクション参照

use std::collections::HashSet;
use std::sync::RwLock;
use lazy_static::lazy_static;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 【仕様契約 SC-13: ENUM ロード RwLock 移行】(#10 追加: 2025-12-17)
//
// OnceLock の問題点:
//   - テスト時にリセット不可（test isolation 破壊）
//   - DB接続失敗時にfallbackできない
//   - ホットリロード不可
//
// RwLock + 静的フォールバック:
//   - DB から読み込めた場合: DB 値を使用
//   - DB から読み込めなかった場合: 静的フォールバック値を使用
//   - テスト時: write() でリセット可能
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// ENUM 値セット（DB または静的フォールバック）
#[derive(Debug, Clone)]
pub struct EnumValues {
    pub values: HashSet<String>,
    pub source: EnumSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnumSource {
    Database,      // DB から読み込み成功
    StaticFallback, // DB 失敗 → 静的値を使用
}

lazy_static! {
    /// RwLock ベースの ENUM 値保持（リセット可能）
    static ref TODOFUKEN_ALLOWED: RwLock<EnumValues> = RwLock::new(EnumValues {
        values: static_todofuken_fallback(),
        source: EnumSource::StaticFallback,
    });
    static ref REMOTE_ONSITE_ALLOWED: RwLock<EnumValues> = RwLock::new(EnumValues {
        values: static_remote_onsite_fallback(),
        source: EnumSource::StaticFallback,
    });
    static ref FLOW_DEPT_ALLOWED: RwLock<EnumValues> = RwLock::new(EnumValues {
        values: static_flow_dept_fallback(),
        source: EnumSource::StaticFallback,
    });
}

/// 静的フォールバック: 都道府県（DB 接続失敗時に使用）
/// #4修正: 2025-12-18 DDL ses.todofuken_enum と統一（正式名称: 東京都, 京都府 等）
fn static_todofuken_fallback() -> HashSet<String> {
    // ⚠️ DDL ses.todofuken_enum と完全一致させること（47都道府県・正式名称）
    [
        "北海道",
        "青森県", "岩手県", "宮城県", "秋田県", "山形県", "福島県",
        "茨城県", "栃木県", "群馬県", "埼玉県", "千葉県", "東京都", "神奈川県",
        "新潟県", "富山県", "石川県", "福井県", "山梨県", "長野県",
        "岐阜県", "静岡県", "愛知県", "三重県",
        "滋賀県", "京都府", "大阪府", "兵庫県", "奈良県", "和歌山県",
        "鳥取県", "島根県", "岡山県", "広島県", "山口県",
        "徳島県", "香川県", "愛媛県", "高知県",
        "福岡県", "佐賀県", "長崎県", "熊本県", "大分県", "宮崎県", "鹿児島県", "沖縄県",
    ].iter().map(|s| s.to_string()).collect()
}

/// 静的フォールバック: リモート形態
fn static_remote_onsite_fallback() -> HashSet<String> {
    ["フル出社", "リモート併用", "フルリモート"]
        .iter().map(|s| s.to_string()).collect()
}

/// 静的フォールバック: 商流（#4修正: 2025-12-18 DDLと統一）
fn static_flow_dept_fallback() -> HashSet<String> {
    // ⚠️ DDL ses.flow_dept_enum と完全一致させること
    ["エンド直", "1次請け", "2次請け", "3次請け", "4次請け以上", "不明"]
        .iter().map(|s| s.to_string()).collect()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 【仕様契約 SC-14: PREFECTURE_MAP 統合】(#7 追加: 2025-12-17)
//
// 問題点:
//   - 2文字短縮形 (北海, 神奈, 鹿児, 和歌) の処理が不完全
//   - PREFECTURE_MAP と OnceLock の二重管理
//
// 解決策:
//   - PREFECTURE_MAP を正規化の第一段階として使用
//   - 正規化後の値を DB ENUM と照合
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

use std::collections::HashMap;

lazy_static! {
    /// 都道府県の表記揺れ → 正式名称（#4修正: 2025-12-18 DDL ses.todofuken_enum と統一）
    /// ⚠️ 全ての入力パターンを DDL の正式名称（東京都, 京都府, 大阪府 等）に正規化
    pub static ref PREFECTURE_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();

        // ===== 特殊ケース（2文字で終わらない）=====
        m.insert("北海", "北海道");
        m.insert("北海道", "北海道");
        m.insert("神奈", "神奈川県");
        m.insert("神奈川", "神奈川県");
        m.insert("神奈川県", "神奈川県");
        m.insert("和歌", "和歌山県");
        m.insert("和歌山", "和歌山県");
        m.insert("和歌山県", "和歌山県");
        m.insert("鹿児", "鹿児島県");
        m.insert("鹿児島", "鹿児島県");
        m.insert("鹿児島県", "鹿児島県");

        // ===== 通常の都府県（短縮形 → 正式名称）=====
        // 東京都, 京都府, 大阪府 は特別扱い
        m.insert("東京", "東京都");
        m.insert("東京都", "東京都");
        m.insert("京都", "京都府");
        m.insert("京都府", "京都府");
        m.insert("大阪", "大阪府");
        m.insert("大阪府", "大阪府");

        // 通常の県（短縮形 → ○○県）
        let prefectures = [
            ("青森", "青森県"), ("岩手", "岩手県"), ("宮城", "宮城県"),
            ("秋田", "秋田県"), ("山形", "山形県"), ("福島", "福島県"),
            ("茨城", "茨城県"), ("栃木", "栃木県"), ("群馬", "群馬県"),
            ("埼玉", "埼玉県"), ("千葉", "千葉県"),
            ("新潟", "新潟県"), ("富山", "富山県"), ("石川", "石川県"),
            ("福井", "福井県"), ("山梨", "山梨県"), ("長野", "長野県"),
            ("岐阜", "岐阜県"), ("静岡", "静岡県"), ("愛知", "愛知県"),
            ("三重", "三重県"), ("滋賀", "滋賀県"),
            ("兵庫", "兵庫県"), ("奈良", "奈良県"),
            ("鳥取", "鳥取県"), ("島根", "島根県"), ("岡山", "岡山県"),
            ("広島", "広島県"), ("山口", "山口県"), ("徳島", "徳島県"),
            ("香川", "香川県"), ("愛媛", "愛媛県"), ("高知", "高知県"),
            ("福岡", "福岡県"), ("佐賀", "佐賀県"), ("長崎", "長崎県"),
            ("熊本", "熊本県"), ("大分", "大分県"), ("宮崎", "宮崎県"),
            ("沖縄", "沖縄県"),
        ];
        for (key, value) in prefectures {
            m.insert(key, value);
        }

        // 「県」付きパターン（正式名称 → 正式名称、冪等性確保）
        let with_suffix: Vec<(&'static str, &'static str)> = vec![
            ("青森県", "青森県"), ("岩手県", "岩手県"), ("宮城県", "宮城県"),
            ("秋田県", "秋田県"), ("山形県", "山形県"), ("福島県", "福島県"),
            ("茨城県", "茨城県"), ("栃木県", "栃木県"), ("群馬県", "群馬県"),
            ("埼玉県", "埼玉県"), ("千葉県", "千葉県"),
            ("新潟県", "新潟県"), ("富山県", "富山県"), ("石川県", "石川県"),
            ("福井県", "福井県"), ("山梨県", "山梨県"), ("長野県", "長野県"),
            ("岐阜県", "岐阜県"), ("静岡県", "静岡県"), ("愛知県", "愛知県"),
            ("三重県", "三重県"), ("滋賀県", "滋賀県"),
            ("兵庫県", "兵庫県"), ("奈良県", "奈良県"),
            ("鳥取県", "鳥取県"), ("島根県", "島根県"), ("岡山県", "岡山県"),
            ("広島県", "広島県"), ("山口県", "山口県"), ("徳島県", "徳島県"),
            ("香川県", "香川県"), ("愛媛県", "愛媛県"), ("高知県", "高知県"),
            ("福岡県", "福岡県"), ("佐賀県", "佐賀県"), ("長崎県", "長崎県"),
            ("熊本県", "熊本県"), ("大分県", "大分県"), ("宮崎県", "宮崎県"),
            ("沖縄県", "沖縄県"),
        ];
        for (key, value) in with_suffix {
            m.insert(key, value);
        }

        m
    };
}

// ===== 旧 OnceLock 互換（非推奨・削除予定）=====
// 以下は互換性のため残すが、新コードは RwLock 版を使用すること

/// テーブルスキーマのデフォルト値
const DEFAULT_TABLE_SCHEMA: &str = "ses";
/// ENUMを読み込む対象テーブル名
const ENUM_SOURCE_TABLE: &str = "projects_enum";

// ───────────────────────────────────────────────────────────────
// ENUM型名の自動検出（db_introspection）
// ⚠️ 型名のハードコードをやめ、カラムの udt_name を真実とする
// ───────────────────────────────────────────────────────────────

/// ENUM型名（schema + name）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumTypeName {
    pub schema: String,
    pub name: String,
}

impl EnumTypeName {
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}

/// 指定した (table_schema, table_name, column_name) が ENUM なら、その型名を返す。
///
/// - Ok(Some(..)) : ENUM
/// - Ok(None)     : ENUMではない（TEXT等）→ allowed set 読み込み不要
/// - Err(..)      : テーブル/カラムが見つからない等
///
/// ⚠️ pg_catalog を直接見に行くので、typtype='e' で確実に判定できる
pub async fn detect_enum_type_name(
    db: &tokio_postgres::Client,
    table_schema: &str,
    table_name: &str,
    column_name: &str,
) -> Result<Option<EnumTypeName>, Box<dyn std::error::Error + Send + Sync>> {
    let row_opt = db
        .query_opt(
            r#"
            SELECT
                t.typtype          AS type_kind,
                n_type.nspname     AS type_schema,
                t.typname          AS type_name
            FROM pg_attribute a
            JOIN pg_class c         ON c.oid = a.attrelid
            JOIN pg_namespace n_tbl  ON n_tbl.oid = c.relnamespace
            JOIN pg_type t           ON t.oid = a.atttypid
            JOIN pg_namespace n_type ON n_type.oid = t.typnamespace
            WHERE n_tbl.nspname = $1
              AND c.relname = $2
              AND a.attname = $3
              AND a.attnum > 0
              AND NOT a.attisdropped
            "#,
            &[&table_schema, &table_name, &column_name],
        )
        .await?;

    let row = match row_opt {
        Some(r) => r,
        None => {
            return Err(format!(
                "Column not found: {}.{}.{}, or it is dropped",
                table_schema, table_name, column_name
            )
            .into());
        }
    };

    let type_kind: String = row.get("type_kind");
    let type_schema: String = row.get("type_schema");
    let type_name: String = row.get("type_name");

    if type_kind == "e" {
        Ok(Some(EnumTypeName {
            schema: type_schema,
            name: type_name,
        }))
    } else {
        Ok(None) // TEXT等 → ENUM読み込み不要
    }
}

// ───────────────────────────────────────────────────────────────
// ENUM labels 読み込み（自動検出ベース）
// ───────────────────────────────────────────────────────────────

/// 起動時に呼び出す（main の冒頭）
/// ⚠️ 型名はカラムの udt_name から自動検出。環境変数は上書き用のみ
///
/// (#10 修正: 2025-12-17) RwLock 版 - DB 失敗時は静的フォールバックを維持
pub async fn load_enum_values(db_client: &tokio_postgres::Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let table_schema = std::env::var("SR_TABLE_SCHEMA").unwrap_or_else(|_| DEFAULT_TABLE_SCHEMA.to_string());
    let table_name = std::env::var("SR_ENUM_TABLE").unwrap_or_else(|_| ENUM_SOURCE_TABLE.to_string());

    let mut loaded_info = Vec::new();

    // ===== work_todofuken =====
    match try_load_enum(db_client, &table_schema, &table_name, "work_todofuken").await {
        Ok(labels) => {
            let count = labels.len();
            let mut guard = TODOFUKEN_ALLOWED.write().unwrap();
            guard.values = labels;
            guard.source = EnumSource::Database;
            loaded_info.push(format!("work_todofuken={} (DB)", count));
        }
        Err(e) => {
            log::warn!("Failed to load work_todofuken ENUM: {}. Using static fallback.", e);
            loaded_info.push("work_todofuken=fallback".to_string());
            // 静的フォールバック値は lazy_static で既に設定済み
        }
    }

    // ===== remote_onsite =====
    match try_load_enum(db_client, &table_schema, &table_name, "remote_onsite").await {
        Ok(labels) => {
            let count = labels.len();
            let mut guard = REMOTE_ONSITE_ALLOWED.write().unwrap();
            guard.values = labels;
            guard.source = EnumSource::Database;
            loaded_info.push(format!("remote_onsite={} (DB)", count));
        }
        Err(e) => {
            log::warn!("Failed to load remote_onsite ENUM: {}. Using static fallback.", e);
            loaded_info.push("remote_onsite=fallback".to_string());
        }
    }

    // ===== flow_dept =====
    match try_load_enum(db_client, &table_schema, &table_name, "flow_dept").await {
        Ok(labels) => {
            let count = labels.len();
            let mut guard = FLOW_DEPT_ALLOWED.write().unwrap();
            guard.values = labels;
            guard.source = EnumSource::Database;
            loaded_info.push(format!("flow_dept={} (DB)", count));
        }
        Err(e) => {
            log::warn!("Failed to load flow_dept ENUM: {}. Using static fallback.", e);
            loaded_info.push("flow_dept=fallback".to_string());
        }
    }

    eprintln!("✅ ENUM allowed sets: {}", loaded_info.join(", "));
    Ok(())
}

/// ENUM ロードを試みる（失敗しても panic しない）
async fn try_load_enum(
    db_client: &tokio_postgres::Client,
    table_schema: &str,
    table_name: &str,
    column_name: &str,
) -> Result<HashSet<String>, Box<dyn std::error::Error + Send + Sync>> {
    // 環境変数で型名を明示的に上書きできる（例: SR_ENUM_TYPE_work_todofuken）
    let env_override = std::env::var(format!("SR_ENUM_TYPE_{}", column_name)).ok();

    let enum_type = if let Some(override_type) = env_override {
        let schema = std::env::var("SR_ENUM_SCHEMA").unwrap_or_else(|_| table_schema.to_string());
        Some(EnumTypeName { schema, name: override_type })
    } else {
        detect_enum_type_name(db_client, table_schema, table_name, column_name).await?
    };

    match enum_type {
        Some(t) => load_enum_labels(db_client, &t).await,
        None => Err("Column is not ENUM type".into()),
    }
}

/// PostgreSQL の ENUM labels を HashSet として取得
/// ⚠️ EnumTypeName を受け取るので、スキーマ+型名が一貫する
async fn load_enum_labels(
    db_client: &tokio_postgres::Client,
    enum_type: &EnumTypeName,
) -> Result<HashSet<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
        SELECT e.enumlabel
        FROM pg_enum e
        JOIN pg_type t ON e.enumtypid = t.oid
        JOIN pg_namespace n ON t.typnamespace = n.oid
        WHERE t.typname = $1
          AND n.nspname = $2
        ORDER BY e.enumsortorder
    "#;

    let rows = db_client.query(query, &[&enum_type.name, &enum_type.schema]).await?;
    let labels: HashSet<String> = rows.iter()
        .map(|row| row.get::<_, String>(0))
        .collect();

    if labels.is_empty() {
        return Err(format!(
            "ENUM '{}' not found or has no labels",
            enum_type.qualified()
        ).into());
    }

    Ok(labels)
}

/// -------------------------------------------------------------------
/// 都道府県ENUM補正（SC-14: PREFECTURE_MAP + RwLock 統合版）
/// -------------------------------------------------------------------
///
/// 【処理フロー】
/// 1. PREFECTURE_MAP で正規化（短縮形→正式名）
/// 2. RwLock で保持した DB ENUM set と照合
/// 3. 静的フォールバック値も許容
///
/// (#7, #10 修正: 2025-12-17)
fn correct_todofuken(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // ===== Step 1: PREFECTURE_MAP で正規化 =====
    // 短縮形（北海, 神奈, 鹿児, 和歌）や県付き表記を正規形に変換
    let normalized = if let Some(&mapped) = PREFECTURE_MAP.get(trimmed) {
        mapped.to_string()
    } else {
        // PREFECTURE_MAP にない場合、末尾の 都/府/県 を剥がして再試行
        let stripped = trimmed
            .strip_suffix("都")
            .or_else(|| trimmed.strip_suffix("府"))
            .or_else(|| trimmed.strip_suffix("県"))
            .or_else(|| trimmed.strip_suffix("道"))
            .unwrap_or(trimmed);

        if let Some(&mapped) = PREFECTURE_MAP.get(stripped) {
            mapped.to_string()
        } else {
            trimmed.to_string()  // 変換不可 → 元の値で DB チェック
        }
    };

    // ===== Step 2: RwLock で DB ENUM set と照合 =====
    let guard = TODOFUKEN_ALLOWED.read()
        .expect("TODOFUKEN_ALLOWED RwLock poisoned");

    if guard.values.contains(&normalized) {
        return Some(normalized);
    }

    // 正規化前の値も試す（DB が "東京都" 形式の場合）
    if guard.values.contains(trimmed) {
        return Some(trimmed.to_string());
    }

    // ===== Step 3: フォールバック値でも見つからない → None =====
    log::debug!(
        "correct_todofuken failed: input='{}', normalized='{}', source={:?}",
        trimmed, normalized, guard.source
    );
    None  // requires_manual_review
}

#[cfg(test)]
mod tests_todofuken {
    use super::*;

    fn setup_test_enum() {
        // RwLock なのでリセット可能（test isolation 確保）
        let mut set = HashSet::new();
        set.insert("北海道".to_string());
        set.insert("東京都".to_string());
        set.insert("京都府".to_string());
        set.insert("大阪府".to_string());
        set.insert("神奈川県".to_string());
        set.insert("和歌山県".to_string());
        set.insert("鹿児島県".to_string());

        let mut guard = TODOFUKEN_ALLOWED.write().unwrap();
        guard.values = set;
        guard.source = EnumSource::StaticFallback;  // テスト用
    }

    #[test]
    fn test_tokyo() {
        setup_test_enum();
        assert_eq!(correct_todofuken("東京都"), Some("東京都".to_string()));
        assert_eq!(correct_todofuken("東京"), Some("東京都".to_string()));
    }

    #[test]
    fn test_kyoto_no_bug() {
        setup_test_enum();
        // ⚠️ 以前のバグ: "京都府" → "京" になっていた
        assert_eq!(correct_todofuken("京都府"), Some("京都府".to_string()));
        assert_eq!(correct_todofuken("京都"), Some("京都府".to_string()));
    }

    #[test]
    fn test_hokkaido_special() {
        setup_test_enum();
        // ⚠️ 北海道は "道" を剥がさない
        assert_eq!(correct_todofuken("北海道"), Some("北海道".to_string()));
    }

    // ======== Issue #7: 2文字短縮形エッジケース ========
    #[test]
    fn test_two_char_short_forms() {
        setup_test_enum();
        // SC-14: PREFECTURE_MAP で 2文字短縮形を正しく処理
        assert_eq!(correct_todofuken("北海"), Some("北海道".to_string()));
        assert_eq!(correct_todofuken("神奈"), Some("神奈川県".to_string()));
        assert_eq!(correct_todofuken("和歌"), Some("和歌山県".to_string()));
        assert_eq!(correct_todofuken("鹿児"), Some("鹿児島県".to_string()));
    }

    #[test]
    fn test_full_name_with_suffix() {
        setup_test_enum();
        // 県付きの正式名も正しく処理
        assert_eq!(correct_todofuken("神奈川県"), Some("神奈川県".to_string()));
        assert_eq!(correct_todofuken("和歌山県"), Some("和歌山県".to_string()));
        assert_eq!(correct_todofuken("鹿児島県"), Some("鹿児島県".to_string()));
    }

    // ======== Issue #12: 北海エッジケース追加 ========
    #[test]
    fn test_hokkai_short_form() {
        setup_test_enum();
        // "北海" は PREFECTURE_MAP で "北海道" に変換される
        // ⚠️ 短縮形での入力にも対応（メールで「北海勤務」等の記述がありうる）
        assert_eq!(correct_todofuken("北海"), Some("北海道".to_string()));
    }

    #[test]
    fn test_hokkaido_variants() {
        setup_test_enum();
        // 北海道のバリエーション（全て "北海道" に正規化）
        assert_eq!(correct_todofuken("北海道"), Some("北海道".to_string()));
        assert_eq!(correct_todofuken("北海"), Some("北海道".to_string()));
        // スペース含みはtrimで処理
        assert_eq!(correct_todofuken(" 北海道 "), Some("北海道".to_string()));
    }

    #[test]
    fn test_invalid() {
        setup_test_enum();
        assert_eq!(correct_todofuken("架空県"), None);
    }
}

/// リモート形態ENUM補正（SC-13: RwLock 版）
/// (#10 修正: 2025-12-17)
fn correct_remote_onsite(input: &str) -> Option<String> {
    let guard = REMOTE_ONSITE_ALLOWED.read()
        .expect("REMOTE_ONSITE_ALLOWED RwLock poisoned");

    let trimmed = input.trim();

    // まず完全一致をチェック（DB値そのままの場合）
    if guard.values.contains(trimmed) {
        return Some(trimmed.to_string());
    }

    // パターンマッチで正規化を試みる
    let normalized_lower = trimmed.to_lowercase();
    let candidate = if normalized_lower.contains("フル出社") || normalized_lower.contains("出社のみ") {
        "フル出社"
    } else if normalized_lower.contains("フルリモート") || normalized_lower.contains("完全リモート") {
        "フルリモート"
    } else if normalized_lower.contains("リモート") || normalized_lower.contains("併用") || normalized_lower.contains("ハイブリッド") {
        "リモート併用"
    } else {
        return None;  // パターン不一致
    };

    // 正規化結果が DB enum に存在するか確認
    if guard.values.contains(candidate) {
        Some(candidate.to_string())
    } else {
        // パターンは一致したが、DB enum にその値がない
        log::debug!(
            "remote_onsite candidate '{}' not in enum set (source={:?})",
            candidate, guard.source
        );
        None
    }
}

/// 商流深さENUM補正（SC-13: RwLock 版）
/// (#10 修正: 2025-12-17)
fn correct_flow_dept(input: &str) -> Option<String> {
    let guard = FLOW_DEPT_ALLOWED.read()
        .expect("FLOW_DEPT_ALLOWED RwLock poisoned");

    let trimmed = input.trim();

    // まず完全一致をチェック（DB値そのままの場合）
    if guard.values.contains(trimmed) {
        return Some(trimmed.to_string());
    }

    // パターンマッチで正規化を試みる
    let candidate = if trimmed.contains("エンド直") || trimmed.contains("直請") {
        "エンド直"
    } else if trimmed.contains("1次") || trimmed.contains("一次") {
        "1次請け"
    } else if trimmed.contains("2次") || trimmed.contains("二次") {
        "2次請け"
    } else if trimmed.contains("3次") || trimmed.contains("三次") || trimmed.contains("以上") {
        "3次請け以上"  // DB ENUM は "3次請け以上" に統一
    } else {
        return None;  // パターン不一致
    };

    // 正規化結果が DB enum に存在するか確認
    if guard.values.contains(candidate) {
        Some(candidate.to_string())
    } else {
        log::debug!(
            "flow_dept candidate '{}' not in enum set (source={:?})",
            candidate, guard.source
        );
        None
    }
}

/// -------------------------------------------------------------------
/// LlmError 型定義（sr-llm-worker/src/llm_client.rs）
/// -------------------------------------------------------------------
/// LLM API 呼び出し時のエラー型。ErrorClass::from_llm_error() と連携。

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// 429 Too Many Requests（レート制限）
    #[error("rate limited")]
    RateLimited { retry_after: Option<Duration> },

    /// 400 Bad Request（リクエスト形式エラー、リトライ不可）
    #[error("bad request: {0}")]
    BadRequest(String),

    /// 401 Unauthorized（認証エラー、リトライ不可）
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// 5xx サーバーエラー（リトライ可能）
    #[error("http status {status}")]
    HttpStatus { status: u16, body: Option<String> },

    /// タイムアウト（リトライ可能）
    #[error("timeout")]
    Timeout,

    /// ネットワーク/接続エラー（リトライ可能）
    #[error("transport: {0}")]
    Transport(String),

    /// JSONデコードエラー（リトライ不可）
    #[error("decode: {0}")]
    Decode(String),
}

impl LlmError {
    /// 429 の Retry-After ヘッダ値を取得（それ以外は None）
    pub fn retry_after_hint(&self) -> Option<Duration> {
        match self {
            LlmError::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}

/// -------------------------------------------------------------------
/// llm_client 契約仕様（sr-llm-worker/src/llm_client.rs）
/// -------------------------------------------------------------------
/// LLM API との通信を担当。OpenAI API 互換エンドポイントを想定。

/**
## 環境変数

| 変数名 | 必須 | デフォルト | 説明 |
|--------|------|------------|------|
| `LLM_API_BASE_URL` | ✅ | - | API エンドポイント（例: `https://api.openai.com/v1`） |
| `LLM_API_KEY` | ✅ | - | API キー（Bearer トークン） |
| `LLM_MODEL` | ❌ | `gpt-4o-mini` | 使用モデル名 |
| `LLM_TIMEOUT_SECS` | ❌ | `30` | リクエストタイムアウト（秒） |
| `LLM_MAX_RETRIES` | ❌ | `2` | 最大リトライ回数（429/5xx 用） |

## Request/Response 形式

### Request（call_llm の引数）
```rust
pub async fn call_llm(
    message_id: &str,            // ログ用識別子
    body_text: &str,             // メール本文
    partial: &PartialProject,    // Rust 抽出済みフィールド（コンテキスト用）
) -> Result<PartialProject, LlmError>;
```

### 内部 HTTP Request
```json
{
  "model": "${LLM_MODEL}",
  "messages": [
    {
      "role": "system",
      "content": "あなたは案件メールから情報を抽出するアシスタントです..."
    },
    {
      "role": "user",
      "content": "以下のメールから案件情報を抽出してください:\n\n${body_text}\n\n既に抽出済み: ${partial_json}"
    }
  ],
  "temperature": 0.0,
  "response_format": { "type": "json_object" }
}
```

### Response（LLM からの JSON）
```json
{
  "project_name": "Webアプリ開発案件",
  "monthly_tanka_min": 60,
  "monthly_tanka_max": 80,
  "start_date": "2025-02-01",
  "work_todofuken": "東京都",
  "remote_onsite": "フルリモート",
  "required_skills_keywords": ["Python", "Django", "PostgreSQL"],
  "flow_dept": "エンド直"
}
```
⚠️ **フィールド名は sponto-platform の `projects_enum` カラム名と完全一致させること**:
- `monthly_tanka_*` は**万円単位**（60 = 60万円、NOT 600000円）
- `work_todofuken` は**正式名称**（`東京都`、NOT `東京`）
- `flow_dept` は ENUM値（`エンド直/1次請け/2次請け/3次請け/4次請け以上/不明`）

## タイムアウト・リトライ設定

```rust
/// LLM クライアント設定
pub struct LlmClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout: Duration,      // デフォルト: 30秒
    pub max_retries: u32,       // デフォルト: 2
    pub retry_base_delay: Duration, // デフォルト: 1秒（exponential backoff の初期値）
}

impl LlmClientConfig {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            base_url: std::env::var("LLM_API_BASE_URL")?,
            api_key: std::env::var("LLM_API_KEY")?,
            model: std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            timeout: Duration::from_secs(
                std::env::var("LLM_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30)
            ),
            max_retries: std::env::var("LLM_MAX_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
            retry_base_delay: Duration::from_secs(1),
        })
    }
}
```

## リトライ戦略

| エラー種別 | リトライ | 待機時間 |
|-----------|---------|----------|
| 429 (Rate Limited) | ✅ | `Retry-After` ヘッダ or exponential backoff |
| 5xx (Server Error) | ✅ | exponential backoff (1s, 2s, 4s...) |
| Timeout | ✅ | exponential backoff |
| Transport Error | ✅ | exponential backoff |
| 400 (Bad Request) | ❌ | - |
| 401 (Unauthorized) | ❌ | - |
| JSON Decode Error | ❌ | - |

**Exponential Backoff**:
```rust
fn calculate_backoff(attempt: u32, base: Duration) -> Duration {
    let multiplier = 2u64.pow(attempt);
    let jitter = rand::random::<f64>() * 0.3; // 0-30% jitter
    base * multiplier as u32 + Duration::from_secs_f64(jitter)
}
```
*/

/// -------------------------------------------------------------------
/// ⚠️ B案統一: エラー処理は1本の関数に集約
/// ErrorClass で retry 可否を判定、LlmError からも自動判定可能
/// -------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum ErrorClass {
    Retryable,   // 5xx, 429, timeout, transport → pending + next_retry_at
    Permanent,   // 400, 401 → manual_review でクローズ
}

impl ErrorClass {
    /// LlmError から ErrorClass を判定（llm_client.rs の LlmError と連携）
    pub fn from_llm_error(err: &LlmError) -> Self {
        match err {
            LlmError::BadRequest(_) | LlmError::Unauthorized(_) => ErrorClass::Permanent,
            _ => ErrorClass::Retryable,
        }
    }
}

/// ⚠️ B案統一: エラー処理の唯一のエントリポイント
/// - Retryable + retry_count < 3 → pending に戻す
/// - Permanent or retry上限 → manual_review でクローズ
///
/// next_retry_after: 429 の retry_after を考慮した待機時間
/// - Some(Duration) → 指定された秒数後にリトライ（429 用）
/// - None → デフォルト 5 分後
pub async fn handle_job_error(
    db_client: &tokio_postgres::Client,
    job_id: i32,
    error_message: &str,
    error_class: ErrorClass,
    next_retry_after: Option<std::time::Duration>,
) -> Result<(), Box<dyn std::error::Error>> {
    const MAX_RETRIES: i32 = 3;
    const DEFAULT_RETRY_SECONDS: i64 = 300;  // 5 minutes

    // retry_count を取得
    let row = db_client.query_one(
        "SELECT retry_count FROM ses.extraction_queue WHERE id = $1",
        &[&job_id]
    ).await?;
    let current_retry: i32 = row.get(0);

    let is_retryable = matches!(error_class, ErrorClass::Retryable);

    if is_retryable && current_retry < MAX_RETRIES {
        // 一時エラー → pending に戻す（Recovery を待たない）
        // ⚠️ processing_started_at = NULL は OK（pending なので、再取得時に新しい値がセットされる）
        // ⚠️ 429 の retry_after を優先、なければデフォルト 5 分
        let retry_seconds = next_retry_after
            .map(|d| d.as_secs() as i64)
            .unwrap_or(DEFAULT_RETRY_SECONDS);

        // ⚠️ interval は型安全に（文字列組み立ては環境差で事故る）
        db_client.execute(
            "UPDATE ses.extraction_queue
             SET status = 'pending',
                 locked_by = NULL,
                 processing_started_at = NULL,
                 retry_count = retry_count + 1,
                 next_retry_at = NOW() + ($3::int * INTERVAL '1 second'),
                 last_error = $2,
                 updated_at = NOW()
             WHERE id = $1",
            &[&job_id, &error_message, &(retry_seconds as i32)]
        ).await?;

        log::info!("Job {} marked as pending for retry ({}/{}) - next retry in {}s",
              job_id, current_retry + 1, MAX_RETRIES, retry_seconds);
    } else {
        // 永続エラー or retry上限 → manual_review でクローズ
        // ⚠️ processing_started_at は残す（KPI計算: queue_wait = processing_started_at - created_at）
        db_client.execute(
            "UPDATE ses.extraction_queue
             SET status = 'completed',
                 final_method = 'manual_review',
                 locked_by = NULL,
                 last_error = $2,
                 completed_at = NOW(),
                 updated_at = NOW()
             WHERE id = $1",
            &[&job_id, &error_message]
        ).await?;

        log::warn!("Job {} failed permanently after {} retries: {}",
              job_id, current_retry, error_message);
    }
    Ok(())
}
```

**使用例**:
```rust
// LlmError から自動判定（retry_after_hint() で 429 の待機時間を取得）
match call_llm_with_retry(req).await {
    Ok(resp) => { /* 成功処理 */ }
    Err(e) => {
        let class = ErrorClass::from_llm_error(&e);
        let retry_hint = e.retry_after_hint();  // 429 なら Some(Duration), それ以外は None
        handle_job_error(&db_client, job_id, &format!("{:?}", e), class, retry_hint).await?;
    }
}

// 明示的に指定（retry_hint なし = デフォルト 5 分）
handle_job_error(&db_client, job_id, "timeout", ErrorClass::Retryable, None).await?;
handle_job_error(&db_client, job_id, "invalid token", ErrorClass::Permanent, None).await?;
```

**Note（B案統一）**:
- 一時エラー（5xx等）→ Worker が **直接 `pending` に戻す**（Recovery を待たない）
- 永続エラー（400/401）or retry上限 → `manual_review` でクローズ
- ⚠️ ステータスは3状態のみ（pending / processing / completed）
- **⚠️ `update_job_failed()` は廃止、`handle_job_error()` に統一**

---

**Worker の Idempotent INSERT（再起動安全性）**:

```sql
-- projects_enum に message_id UNIQUE 制約を追加（migration）
ALTER TABLE ses.projects_enum
ADD CONSTRAINT uq_projects_enum_message_id UNIQUE (message_id);

-- worker が使用する UPSERT（同じメールを複数回処理しても安全）
INSERT INTO ses.projects_enum (
    message_id,
    project_name,
    monthly_tanka_min,
    monthly_tanka_max,
    start_date,
    work_todofuken,
    remote_onsite,
    flow_dept,
    required_skills,
    required_skills_keywords,  -- matching の KO/スコアリングに必須
    requires_manual_review,
    created_at
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW()
)
ON CONFLICT (message_id) DO UPDATE SET
    project_name = EXCLUDED.project_name,
    monthly_tanka_min = EXCLUDED.monthly_tanka_min,
    monthly_tanka_max = EXCLUDED.monthly_tanka_max,
    start_date = EXCLUDED.start_date,
    work_todofuken = EXCLUDED.work_todofuken,
    remote_onsite = EXCLUDED.remote_onsite,
    flow_dept = EXCLUDED.flow_dept,
    required_skills = EXCLUDED.required_skills,
    required_skills_keywords = EXCLUDED.required_skills_keywords,
    requires_manual_review = EXCLUDED.requires_manual_review,
    updated_at = NOW()
RETURNING *;
```

**利点**:
- worker クラッシュ → recovery → 再処理 しても、重複レコードが生成されない
- `final_method` は `ses.extraction_queue` に記録されるため、KPI 集計に影響なし
- Later で同じメールの再抽出（ルール改善時）が安全に可能

---

### 🚨 トランザクション保護（DB整合性の最後の砦）

**問題**: worker は以下の2段階更新を行うが、別トランザクションだと整合性が壊れる
1. `projects_enum` に UPSERT
2. `extraction_queue` を completed に更新

**リスク**:
- INSERT 成功 → queue 更新前に worker クラッシュ
- recovery が pending に戻して再処理
- `final_method` / `retry_count` / `last_error` の整合が壊れる

**解決策**: 同一トランザクションで両方を更新（MVP必須）

```rust
// sr-llm-worker/src/db.rs

pub async fn commit_final_result(
    db_client: &tokio_postgres::Client,
    job_id: i32,
    message_id: &str,
    result: &FinalResult,
    requires_manual_review: bool,
    final_method: &str,
    llm_latency_ms: Option<i32>
) -> Result<(), Box<dyn std::error::Error>> {
    // トランザクション開始
    let txn = db_client.transaction().await?;

    // 1. projects_enum に UPSERT（RETURNING を使わない場合）
    txn.execute(
        "INSERT INTO ses.projects_enum (
            message_id, project_name, monthly_tanka_min, monthly_tanka_max,
            start_date, work_todofuken, remote_onsite, flow_dept,
            required_skills, required_skills_keywords, requires_manual_review, created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW())
        ON CONFLICT (message_id) DO UPDATE SET
            project_name = EXCLUDED.project_name,
            -- (省略)
            updated_at = NOW()",
        &[
            &message_id, &result.project_name, &result.monthly_tanka_min,
            // ... (全フィールド)
        ]
    ).await?;

    // 2. extraction_queue を completed に更新
    // ⚠️ processing_started_at は残す（KPI計算: queue_wait = processing_started_at - created_at）
    // stuck判定は status='processing' で行うので、completed 行に残っても問題なし
    txn.execute(
        "UPDATE ses.extraction_queue
         SET status = 'completed',
             final_method = $2,
             completed_at = NOW(),
             llm_latency_ms = $3,
             locked_by = NULL,
             updated_at = NOW()
         WHERE id = $1",
        &[&job_id, &final_method, &llm_latency_ms]
    ).await?;

    // トランザクションをコミット（ここで両方が確定）
    txn.commit().await?;

    Ok(())
}
```

**ポイント**:
- `BEGIN` → projects_enum UPSERT → queue UPDATE → `COMMIT` が原子的
- どちらかが失敗したら両方ロールバック（整合性を保証）
- worker クラッシュ時も、committed されていなければ両方が未反映
- recovery が安全に pending に戻せる

**MVP でのトランザクション方針**:
- ✅ worker の最終コミット（projects_enum + queue）は **必ずトランザクション**
- ✅ extractor の enqueue は単発（idempotent なので問題なし）
- ✅ recovery の UPDATE も単発（冪等性により安全）

---

## 🔄 Retry戦略とステータス管理（失敗からの回復）

### Retry時のステータス遷移

**基本方針（3状態設計）**:
- Worker が失敗した場合、**`status='pending'` に戻す**（`next_retry_at` で次回処理時刻を設定）
- Worker が次回の polling で **retry上限内なら再処理**
- retry上限を超えたら `status='completed'` + `final_method='manual_review'` でクローズ
- ⚠️ ステータスは3状態のみ: pending（リトライ待ち含む） / processing / completed

**メリット**:
- Worker は `status='pending'` だけ見れば良い（3状態設計でシンプル）
- Recovery の役割が「stuck job を pending に戻す」だけに単純化
- retry_count でリトライ履歴を追跡可能

### ~~update_job_failed() 実装~~ → `handle_job_error()` に統一

**⚠️ 廃止**: この関数は `handle_job_error()` に統一されました。

詳細は「[ENUM補正](#enum補正)」セクション内の `handle_job_error()` を参照してください。

```rust
// 使用例（retry_hint なし = デフォルト 5 分）
handle_job_error(&db_client, job_id, "error message", ErrorClass::Retryable, None).await?;
handle_job_error(&db_client, job_id, "error message", ErrorClass::Permanent, None).await?;
```

### extraction_queue スキーマ修正

`next_retry_at` カラムは [DDL定義（まとめ）](#ddl定義まとめ) の DDL-1 に含まれています。

### Worker の polling クエリ修正

`next_retry_at` を考慮してジョブを取得：

```sql
-- sr-llm-worker の polling クエリ
-- 重要: body_text は queue に入れず、anken_emails から都度取得
SELECT
    id,
    message_id,
    email_subject,
    partial_fields,
    decision_reason,
    recommended_method
FROM ses.extraction_queue
WHERE status = 'pending'
  AND (next_retry_at IS NULL OR next_retry_at <= NOW())  -- ← retry待機中を除外
ORDER BY priority DESC, created_at ASC
LIMIT 10
FOR UPDATE SKIP LOCKED;
```

**重要**:
- これにより、エラーで pending に戻った job が即座に再取得されるのを防ぐ（5分間は休止）
- `body_text` は queue に入れず、worker が anken_emails から取得

### fetch_body_text() 実装（Worker用）

Worker は queue から message_id を取得し、anken_emails から本文を取得する。

**⚠️ 列名差分を吸収する実装（COALESCE は不可）**:
- 環境によって `body_text` or `source_text` のどちらかの可能性
- **COALESCE では吸収できない**: SQL は存在しないカラム参照で構文解析時に落ちる
- `fetch_body_text()` を唯一の入口にして、列名を動的に決定

**解決策: 環境変数 + 起動時自動検出のハイブリッド**

```rust
// sr-llm-worker/src/db.rs

/// 起動時に body_text カラムの存在を確認（GO/NO-GO チェック）
/// ⚠️ SC-7: anken_emails.body_text が真実。source_text 互換は廃止。
pub async fn verify_body_text_column(
    db_client: &tokio_postgres::Client
) -> Result<(), Box<dyn std::error::Error>> {
    let rows = db_client.query(
        "SELECT 1 FROM information_schema.columns
         WHERE table_schema = 'ses'
           AND table_name = 'anken_emails'
           AND column_name = 'body_text'",
        &[]
    ).await?;

    if rows.is_empty() {
        return Err(
            "FATAL: 'body_text' column not found in ses.anken_emails. \
             Run migration or check DDL definition.".into()
        );
    }

    log::info!("Verified: ses.anken_emails.body_text exists");
    Ok(())
}

pub async fn fetch_body_text(
    db_client: &tokio_postgres::Client,
    message_id: &str
) -> Result<String, Box<dyn std::error::Error>> {
    // ⚠️ SC-7: body_text に固定（source_text 互換は廃止）
    let row = db_client.query_one(
        "SELECT body_text FROM ses.anken_emails WHERE message_id = $1",
        &[&message_id]
    ).await?;

    let body_text: Option<String> = row.get(0);

    match body_text {
        Some(text) if !text.is_empty() => Ok(text),
        _ => Err("body_text is empty or NULL".into()),
    }
}
```

**main.rs での使用**:
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_client = db::connect().await?;

    // 🚨 起動時チェック（GO/NO-GO）
    db::verify_body_text_column(&db_client).await?;  // body_text 存在確認
    load_enum_values(&db_client).await?;

    // ... worker ループ
}
```

---

### 📋 列名対応表

**【仕様契約 SC-7】** → 詳細は「仕様契約（Spec Contracts）」セクション参照

**要点**: `anken_emails.body_text` が真実。コード内では `body_text` に固定。

| テーブル | 列名 | 用途 |
|----------|------|------|
| `ses.anken_emails` | `body_text` | Gmail本文（**唯一の真実**） |
| `ses.extraction_queue` | (なし) | queue には入れない |
| `ses.projects_enum` | `source_text` | 抽出元テキストのコピー（監査・デバッグ用） |

**⚠️ 変更履歴**: `source_text` 互換検出は廃止。`body_text` に統一。

---

**前提条件**:
- `ses.anken_emails.body_text` が存在すること（DDL で NOT NULL 推奨）
- Worker が anken_emails への読み取り権限を持つ

**メリット**:
- queue が軽量化（message_id だけで管理）
- データ重複なし（真実はanken_emailsに一箇所）
- Phase 2 でルール改善しても anken_emails は変わらない（再抽出が安全）

---

## KPI定義（品質優先）

### 1. 品質KPI（Primary: マッチング精度・説明可能性）

| KPI | Gate | Baseline | Target | 測定方法 | 更新頻度 |
|-----|------|----------|--------|---------|---------|
| **Tier1抽出成功率** | ≥ 75% | 未測定 | ≥ 85% | 4フィールド平均（Phase 2で実測） | 週次 |
| **説明可能率** | - | 未測定 | ≥ 80% | 理由コード付きで抽出できた割合 | 週次 |
| **手動レビュー率** | 前週比 +10%以内 | 現行: 12% (推定) | 維持or改善 | `final_method='manual_review'` 比率 | 週次 |

**測定計画**: Phase 2（Week 4-5）で初回実測。Baseline確定後、Phase 3（Canary）で連続監視開始。

**意図**: ルールベースで「なぜそう判定したか」を蓄積できる割合を最重要指標とする。

---

### 2. Guardrail KPI（リスク管理・安定運用）

| KPI | Gate (即時対応) | Baseline | アクション | 測定方法 |
|-----|----------------|----------|-----------|---------|
| **Rust抽出エラー率** | < 5% | 未測定 | > 5%でロールバック | エラーログ監視 |
| **LLMフォールバック率** | < 70% | 未測定 | > 70%でTier2緩和検討 | `final_method='llm_completed'` 比率 |
| **Queue待機時間** | < 120秒 | 未測定 | < 60秒 | `processing_started_at - created_at` |
| **スキル空配列率** | < 20% | 未測定 | > 20%で軽量ルール拡張 | `required_skills_keywords = '{}'` 比率（破綻対策） |

**Note**:
- 手動レビュー率は `extraction_queue.final_method='manual_review'` の比率で測定（`quality_grade` に依存しない）。
- Rust-only パスでは `quality_grade = NULL` でも可（代わりに `extraction_queue.final_method` で管理）。
- `requires_manual_review` は projects_enum のみに存在し、sr-llm-worker が INSERT 時に設定（必須フィールド `required_skills` 等が不足している場合は true）。
- **スキル空配列率**: 空配列が増えるとマッチングの意味が薄くなる（"何のための抽出？"）。`manual_review_reason='skills_empty'` で追跡し、軽量ルール改善に繋げる。

---

### 3. 副次効果KPI（LLM集中化の結果）

**注**: 以下は品質・説明可能性を高めた**結果として**得られる効率指標。コスト削減自体は目的ではない。

| KPI | Gate | Baseline | Target | 測定方法 | 更新頻度 |
|-----|------|----------|--------|---------|---------|
| **LLM集中率** | ≤ 70% | 現行: 100% (全LLM) | ≤ 50% | `final_method='llm_completed'` 件数 / 全件数 | 週次 |
| **月額API費** | ≤$14 | 現行: $20/月 (推定) | ≤$10 | LLM呼び出し件数 × $0.002 | 月次 |

**測定計画**: Week 10（100%移行後）に1ヶ月実測で Baseline 確定。

---

### 4. Efficiency KPI（処理効率）

| KPI | Gate (許容上限) | Baseline | Target | 測定方法 | 更新頻度 |
|-----|----------------|----------|--------|---------|---------|
| **Rust抽出処理時間** | < 500ms/email | 未測定 (環境依存) | < 100ms | ログ計測 | Phase 3で実測 |
| **LLMキュー待機時間** | < 120秒 | 未測定 | < 60秒 | `processing_started_at - created_at` | 日次 |
| **LLM処理時間** | < 10秒/email | 現行: 3-5秒 | < 5秒 | LLM API latency | 日次 |

**測定計画**: Phase 3（Canary）で実測開始。Gate超過時は worker数増加または設計変更（Redis移行等）で対応。

---

### KPI Dashboard（MVP: BIツールなし）

### 基本方針

**前提**:
- Slack通知なし（MVPはDB+ログ）
- Metabase/Grafana なし（PF追加を避ける）
- 必要なら Gmail送信 or 日次ファイル出力

**最低限の仕様**:
1. KPIをDBに日次スナップショット保存（`ses.kpi_daily` テーブル）
2. systemd timer で 1日1回 `INSERT INTO ... SELECT ...` を実行
3. 見るときは SQL クライアント or 共有.md/CSV出力

**メリット**:
- PF追加不要（PostgreSQL のみ）
- 履歴が残る（Phase 2-4 の推移を後から分析可能）
- 異常検知は SQL で十分（閾値超えたらログ出力 or Gmail送信）

### ses.kpi_daily テーブル定義

```sql
CREATE TABLE ses.kpi_daily (
    measured_date DATE PRIMARY KEY,

    -- LLM集中率（副次効果）
    total_jobs INTEGER NOT NULL,
    rust_completed_count INTEGER NOT NULL,
    llm_count INTEGER NOT NULL,
    manual_review_count INTEGER NOT NULL,
    rust_pct REAL NOT NULL,  -- rust_completed / total * 100
    llm_pct REAL NOT NULL,   -- llm / total * 100
    manual_review_pct REAL NOT NULL,

    -- 品質監視
    tier1_success_pct REAL,  -- Tier1フィールド抽出成功率
    tier2_success_pct REAL,
    skills_empty_pct REAL,   -- スキル空配列率（破綻対策）

    -- パフォーマンス（⚠️ PERCENTILE_CONT は明示キャスト必須）
    p50_queue_wait_ms INTEGER,
    p90_queue_wait_ms INTEGER,
    p50_llm_latency_ms INTEGER,
    p90_llm_latency_ms INTEGER,
    p99_llm_latency_ms INTEGER,

    -- 運用監視（⚠️ failed ステータスは廃止済み）
    stuck_count INTEGER,  -- 現時点で5分以上 processing 状態のジョブ数
    max_retry_reached_count INTEGER,  -- retry_count >= 3 のジョブ数
    high_retry_count INTEGER,  -- retry_count >= 1 のジョブ数

    -- コスト試算
    estimated_cost_usd REAL,  -- llm_count * 0.002（1回あたり約$0.002）

    created_at TIMESTAMPTZ DEFAULT NOW()
);

COMMENT ON TABLE ses.kpi_daily IS 'Daily KPI snapshot for Phase 2-4 monitoring';
```

### 日次集計バッチ（systemd timer）

**スクリプト**: `/usr/local/bin/sr-kpi-daily.sh`

```bash
#!/bin/bash
set -euo pipefail

PGHOST="localhost"
PGDATABASE="sponto"
PGUSER="sponto_admin"

# 前日の KPI を集計して INSERT
psql -h "$PGHOST" -U "$PGUSER" -d "$PGDATABASE" <<'EOF'
INSERT INTO ses.kpi_daily (
    measured_date,
    total_jobs,
    rust_completed_count,
    llm_count,
    manual_review_count,
    rust_pct,
    llm_pct,
    manual_review_pct,
    skills_empty_pct,
    p50_queue_wait_ms,
    p90_queue_wait_ms,
    p50_llm_latency_ms,
    p90_llm_latency_ms,
    p99_llm_latency_ms,
    stuck_count,
    max_retry_reached_count,
    high_retry_count,
    estimated_cost_usd
)
SELECT
    CURRENT_DATE - INTERVAL '1 day' AS measured_date,
    COUNT(*) AS total_jobs,
    COUNT(*) FILTER (WHERE final_method = 'rust_completed') AS rust_completed_count,
    COUNT(*) FILTER (WHERE final_method = 'llm_completed') AS llm_count,
    COUNT(*) FILTER (WHERE final_method = 'manual_review') AS manual_review_count,
    COUNT(*) FILTER (WHERE final_method = 'rust_completed') * 100.0 / NULLIF(COUNT(*), 0) AS rust_pct,
    COUNT(*) FILTER (WHERE final_method = 'llm_completed') * 100.0 / NULLIF(COUNT(*), 0) AS llm_pct,
    COUNT(*) FILTER (WHERE final_method = 'manual_review') * 100.0 / NULLIF(COUNT(*), 0) AS manual_review_pct,
    -- スキル空配列率（破綻対策: 20%超でアラート）
    COUNT(*) FILTER (WHERE manual_review_reason = 'skills_empty') * 100.0 / NULLIF(COUNT(*), 0) AS skills_empty_pct,
    -- ⚠️ PERCENTILE_CONT は明示キャスト必須（環境差で落ちる典型）
    (PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM (processing_started_at - created_at)) * 1000))::int AS p50_queue_wait_ms,
    (PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM (processing_started_at - created_at)) * 1000))::int AS p90_queue_wait_ms,
    (PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY llm_latency_ms) FILTER (WHERE llm_latency_ms IS NOT NULL))::int AS p50_llm_latency_ms,
    (PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY llm_latency_ms) FILTER (WHERE llm_latency_ms IS NOT NULL))::int AS p90_llm_latency_ms,
    (PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY llm_latency_ms) FILTER (WHERE llm_latency_ms IS NOT NULL))::int AS p99_llm_latency_ms,
    -- stuck_count: 現時点で5分以上 processing 状態のジョブ数（日次バッチ対象期間外だが同時に算出）
    (SELECT COUNT(*) FROM ses.extraction_queue
     WHERE status = 'processing'
       AND processing_started_at < NOW() - INTERVAL '5 minutes') AS stuck_count,
    COUNT(*) FILTER (WHERE retry_count >= 3) AS max_retry_reached_count,
    COUNT(*) FILTER (WHERE retry_count >= 1) AS high_retry_count,
    COUNT(*) FILTER (WHERE final_method = 'llm_completed') * 0.002 AS estimated_cost_usd
FROM ses.extraction_queue
WHERE completed_at >= CURRENT_DATE - INTERVAL '1 day'
  AND completed_at < CURRENT_DATE
ON CONFLICT (measured_date) DO NOTHING;
EOF

# Gate チェック（LLM呼び出し率 > 70% ならログ出力）
psql -h "$PGHOST" -U "$PGUSER" -d "$PGDATABASE" -t <<'EOF' | {
    read llm_pct
    if (( $(echo "$llm_pct > 70" | bc -l) )); then
        echo "[ALERT] LLM call rate exceeded 70%: $llm_pct%" | logger -t sr-kpi-daily
        # 必要なら Gmail送信
        # echo "LLM呼び出し率が70%を超えました: $llm_pct%" | mail -s "[ALERT] Rust Extractor" admin@sponto.co.jp
    fi
}
SELECT llm_pct FROM ses.kpi_daily WHERE measured_date = CURRENT_DATE - INTERVAL '1 day';
EOF
```

**systemd timer**: `/etc/systemd/system/sr-kpi-daily.timer`

```ini
[Unit]
Description=Daily KPI aggregation for Rust Extractor

[Timer]
OnCalendar=*-*-* 02:00:00  # 毎日 02:00 に実行
Persistent=true

[Install]
WantedBy=timers.target
```

**systemd service**: `/etc/systemd/system/sr-kpi-daily.service`

```ini
[Unit]
Description=Daily KPI aggregation for Rust Extractor

[Service]
Type=oneshot
ExecStart=/usr/local/bin/sr-kpi-daily.sh
User=sponto
```

**有効化**:
```bash
sudo systemctl daemon-reload
sudo systemctl enable sr-kpi-daily.timer
sudo systemctl start sr-kpi-daily.timer
```

### KPI確認方法

**CLI で確認**:
```bash
# 直近7日間の推移
psql -U sponto_admin -d sponto -c "
SELECT
    measured_date,
    rust_pct,
    llm_pct,
    manual_review_pct,
    p90_queue_wait_ms,
    estimated_cost_usd
FROM ses.kpi_daily
WHERE measured_date >= CURRENT_DATE - INTERVAL '7 days'
ORDER BY measured_date;
"
```

**Markdown レポート出力**:
```bash
# 週次レポートを生成
psql -U sponto_admin -d sponto -H -c "
SELECT * FROM ses.kpi_daily
WHERE measured_date >= CURRENT_DATE - INTERVAL '7 days'
ORDER BY measured_date;
" > /tmp/kpi_weekly_report.html

# 必要なら Gmail に送信
# mail -s "週次KPIレポート" -a /tmp/kpi_weekly_report.html admin@sponto.co.jp
```

**異常検知**:
- LLM呼び出し率 > 70% → ログ出力（必要なら Gmail送信）
- manual_review率 > 30% → Phase 2 失格の兆候
- stuck_count > 5 → Recovery の周期を短縮（10分 → 5分）

**Phase 4 の判定**:
```sql
-- 直近7日間の平均 LLM呼び出し率
SELECT AVG(llm_pct) AS avg_llm_pct
FROM ses.kpi_daily
WHERE measured_date >= CURRENT_DATE - INTERVAL '7 days';

-- 結果が 30% 未満なら Gate クリア（API削減率 70%達成）
-- 結果が 50% 未満なら Target クリア（API削減率 50%達成）
```

---

## 🛑 Phase 1 開始前の確定チェック（実装GOの前提条件）

**⚠️ これをやらずに実装を始めると、INSERT失敗・型不一致・下流破壊で止まる**

### 1. projects_enum DDL 確認

```sql
-- projects_enum のカラム一覧（型・NULL可否）
SELECT
  column_name,
  data_type,
  udt_name,
  is_nullable,
  column_default
FROM information_schema.columns
WHERE table_schema='ses'
  AND table_name='projects_enum'
ORDER BY ordinal_position;
```

**✅ 必ず確認する項目（落とし穴）**:

| カラム | 確認項目 | 落とし穴 |
|--------|----------|----------|
| `message_id` | 存在するか？型は？UNIQUE制約あるか？ | なければ ALTER TABLE で追加必須 |
| `required_skills` | JSONB か？NOT NULL か？ | 形式が `{"items": [...]}` と一致するか実データ3件で確認 |
| `required_skills_keywords` | 存在するか？型は `text[]` か？ | なければ下記の移行手順で追加 |
| `start_date` | 型が `DATE` か `TEXT` か？ | DATE なら「即日」「応相談」で **確実に落ちる** → `start_date_raw TEXT` 別カラム必要 |
| `work_todofuken` | ENUM か TEXT か？ | ENUM なら allowed values をダンプして補正の正規形を固定 |
| `remote_onsite` | 同上 | 同上 |
| `flow_dept` | 同上 | 同上 |

### 2. ENUM 値ダンプ（補正ロジックの正規形を固定）

⚠️ **型名は `detect_enum_type_name()` で自動検出するため、手動でハードコードしない**

```sql
-- Step 1: カラムの実際の型名を取得（udt_name が真実）
SELECT column_name, data_type, udt_name
FROM information_schema.columns
WHERE table_schema = 'ses'
  AND table_name = 'projects_enum'
  AND column_name IN ('work_todofuken', 'remote_onsite', 'flow_dept');

-- Step 2: 取得した udt_name を使って ENUM labels をダンプ
-- （例: udt_name が 'work_todofuken_enum' だった場合）
SELECT e.enumlabel
FROM pg_type t
JOIN pg_enum e ON t.oid = e.enumtypid
JOIN pg_namespace n ON t.typnamespace = n.oid
WHERE t.typname = '<udt_name>'  -- Step 1 で取得した値
  AND n.nspname = 'ses'
ORDER BY e.enumsortorder;
```

**⚠️ コード内では `detect_enum_type_name()` が自動で型名を解決するので、このSQLは確認用**

### 3. required_skills_keywords が無い場合の安全な移行

```sql
-- 1) まず nullable で追加（既存行を壊さない）
ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS required_skills_keywords TEXT[];

-- 2) 既存行を埋める
-- ⚠️ 【重要】["未指定"] は使わない！
-- strategy.py は空配列を "合格扱い" にするが、["未指定"] だと KO になる
UPDATE ses.projects_enum
SET required_skills_keywords = ARRAY[]::text[]  -- 空配列で統一
WHERE required_skills_keywords IS NULL;

-- 3) default を付ける（新規行用）
ALTER TABLE ses.projects_enum
ALTER COLUMN required_skills_keywords SET DEFAULT ARRAY[]::text[];  -- 空配列

-- 4) finally NOT NULL にする
ALTER TABLE ses.projects_enum
ALTER COLUMN required_skills_keywords SET NOT NULL;
```

**Note**: 空配列の場合、strategy.py は必須スキル判定をスキップして "合格扱い" にする。
これはビジネス的には「スキル要件不明」なので、`requires_manual_review=true` で制御する。

**Note**: 既存 `required_skills` (JSONB) から keywords を作れるなら、2) の UPDATE を賢くする：
```sql
-- items[].skill を展開して配列化
UPDATE ses.projects_enum
SET required_skills_keywords = (
  SELECT ARRAY_AGG(item->>'skill')
  FROM jsonb_array_elements(required_skills->'items') AS item
)
WHERE required_skills_keywords IS NULL
  AND required_skills IS NOT NULL
  AND jsonb_array_length(required_skills->'items') > 0;
```

### 3.5. start_date 3カラム構成への移行（High 10 改訂版）

**背景**: `projects_enum.start_date` は既存で DATE 型。
「即日」「来月」等の表現を DATE に正規化する必要があるが、
元の表現（真実）と解釈の精度も保存したい。

```sql
-- 1) 入力値（真実）カラム追加
ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS start_date_raw TEXT;

-- 2) 精度カラム追加
ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS start_date_precision VARCHAR(20);

-- 3) start_date (DATE) は既存のまま使用
-- ⚠️ 既存データがある場合は逆変換が必要（任意）
-- UPDATE ses.projects_enum
-- SET start_date_raw = start_date::TEXT,
--     start_date_precision = 'exact'
-- WHERE start_date IS NOT NULL AND start_date_raw IS NULL;

-- 4) CHECK 制約（任意: precision の値を制限）
ALTER TABLE ses.projects_enum
ADD CONSTRAINT IF NOT EXISTS chk_start_date_precision
CHECK (start_date_precision IN ('exact', 'month', 'early', 'middle', 'late', 'asap', 'unknown')
       OR start_date_precision IS NULL);
```

**運用**:
- **INSERT**: Worker が `start_date_raw`, `start_date`, `start_date_precision` の3つ全てをセット
- **検索**: `WHERE start_date >= '2025-02-01'` で日付ソート・フィルタ可能
- **表示**: `start_date_raw` ("来月頭くらい") を画面表示、または
          `start_date` + `precision` で "2025/02/01 (月)" と表示

### 4. anken_emails DDL 確認

```sql
-- body_text カラムの存在確認
SELECT column_name, data_type, is_nullable
FROM information_schema.columns
WHERE table_schema='ses'
  AND table_name='anken_emails'
  AND column_name IN ('message_id', 'subject', 'body_text', 'created_at');
```

**⚠️ body_text がなければ n8n 側の保存処理を先に実装**

### 5. 既存 required_skills の実データ形状確認

```sql
-- 実データ3件で形状を確認（matching/UI が期待する形式と一致するか）
SELECT
  id,
  jsonb_pretty(required_skills) AS required_skills_shape
FROM ses.projects_enum
WHERE required_skills IS NOT NULL
ORDER BY created_at DESC
LIMIT 3;
```

**期待形式**: `{"items": [{"skill": "Java", "years": 3}, ...]}`
**もし違う形式なら**: `skills_to_jsonb()` を既存形式に合わせて修正

---

### Phase 1 開始 GO/NO-GO チェックリスト

| # | 項目 | 確認方法 | GO条件 |
|---|------|----------|--------|
| 1 | projects_enum.message_id 存在 | SQL | カラムあり or ALTER済み |
| 2 | projects_enum.required_skills_keywords 存在 | SQL | カラムあり or ALTER済み |
| 3 | start_date の型 | SQL | TEXT or raw別カラムあり |
| 4 | ENUM値が補正関数と一致 | SQL + コード比較 | 全値が補正関数でカバー |
| 5 | anken_emails.body_text 存在 | SQL | カラムあり |
| 6 | required_skills 形式確認 | 実データ3件 | `{"items": [...]}` |
| 7 | 既存 matching が何を参照しているか | strategy.py 確認 | required_skills_keywords 追加で壊れない |

**全項目 GO でなければ実装開始しない**

---

## 仕様契約（Spec Contracts）

コード内コメントに散在する仕様契約をここに集約。実装時に必ず参照すること。

### SC-1: required_skills_keywords が空の場合の扱い

| 項目 | 内容 |
|------|------|
| **問題** | `required_skills_keywords` が空だと strategy.py で "合格扱い" になる |
| **解決策** | 空の場合は `requires_manual_review=true` を必ず立てる |
| **禁止事項** | `["未指定"]` は絶対に使わない（ほぼ全員KOになる） |
| **DB** | 空配列 `ARRAY[]::text[]` でOK（NOT NULL制約を満たす） |

### SC-2: start_date 3カラム構成

| カラム | 型 | 意味 | 例 |
|--------|-----|------|-----|
| `start_date_raw` | TEXT | 入力値（真実） | "来月", "即日" |
| `start_date` | DATE | 検索用インデックス | 2025-02-01 |
| `start_date_precision` | VARCHAR(20) | 解釈の精度 | 'exact', 'month', 'asap', 'unknown' |

**正規化ルール**:
- "即日", "ASAP" → received_at, precision='asap'
- "来月" → 翌月1日, precision='month'
- "1月上旬" → 1月1日, precision='early'
- "応相談" → NULL, precision='unknown'

### SC-3: ENUM補正失敗時の扱い

| 項目 | 内容 |
|------|------|
| **方針** | 補正できなかった値は **INSERT しない**（None/NULL に落とす） |
| **副作用** | `requires_manual_review=true` を必ず立てる |
| **理由** | ENUM/TEXT問わずINSERT成功を保証、不正値がDBに入らない |
| **test_mode** | `test_mode=true` の場合、補正失敗時も元の値を保持してログ出力（本番INSERTはしない）。テスト・デバッグ用途 |

**test_mode パラメータ**:
```rust
/// ENUM補正の設定
pub struct EnumCorrectionConfig {
    /// true: 補正失敗時も元の値を保持してログ出力（INSERT はスキップ）
    /// false: 補正失敗時は None に落とす（本番動作）
    pub test_mode: bool,
}

impl Default for EnumCorrectionConfig {
    fn default() -> Self {
        Self { test_mode: false }
    }
}
```

### SC-4: ENUM allowed set の契約

| 項目 | 内容 |
|------|------|
| **方針** | コードは「型名」をハードコードしない。**カラムの `udt_name` を真実**とする |
| **実装** | 起動時に `detect_enum_type_name()` で型名を自動検出 → `pg_enum` から labels を読み込み |
| **環境変数** | `SR_TABLE_SCHEMA` (default: `ses`), `SR_ENUM_TABLE` (default: `projects_enum`) で対象テーブル指定。`SR_ENUM_TYPE_{column}` で型名上書き可能（例外用） |
| **TEXT列対応** | カラムが ENUM でなければ空セットで初期化（補正スキップ） |
| **⚠️ Critical 6** | `pg_namespace` JOIN で同名 ENUM の衝突を防止（public/ses 両方に同名型がある場合）|

### SC-5: Canary判定アルゴリズム

| 項目 | 内容 |
|------|------|
| **方式** | SHA256(message_id) の先頭1バイトを 0-99 に正規化 |
| **判定** | `(hash[0] * 100 / 256) < canary_percent` なら Rust対象 |
| **重要** | n8n と Rust で**完全一致**していること |

### SC-6: normalize_subject の契約

| 項目 | 内容 |
|------|------|
| **契約** | 空文字列を返さない（空になるなら元に戻す） |
| **処理順序** | 1. prefix除去(RE:/FW:) → 2. bracket除去(【】) → 3. 空ならfallback |
| **配置** | `sr-common/src/normalize.rs`（重複実装禁止） |

### SC-7: 列名対応表（body_text vs source_text）

| テーブル | 列名 | 意味 |
|----------|------|------|
| `ses.anken_emails` | `body_text` | Gmail本文（**真実**） |
| `ses.extraction_queue` | (なし) | queue には入れない |
| `ses.projects_enum` | `source_text` | 抽出元テキスト（コピー） |

**ルール**: コード内では `body_text` で統一

---

### SC-8: スコアの出力形式（互換性のための契約）

Rustはスコアを2段階で扱う：

1) **business_rules_score**（0.0〜1.0）
   - KO通過後に business_rules_engine 相当で算出するスコア
   - 加重合成: `tanka×0.25 + location×0.15 + skills×0.40 + experience×0.15 + contract×0.05`

2) **total_score**（0.0〜1.0）
   - 既存システム互換の合成スコア
   - `total_score = w_business × business_rules_score + w_semantic × semantic_score + w_historical × historical_score`

**MVPでの扱い**:
- `semantic_score` / `historical_score` は未実装でもよい（NULL許容）
- 未実装の場合: `w_business=1.0`, `w_semantic=0.0`, `w_historical=0.0` とする
- Phase 2以降で Semantic Matching / Historical Learning を有効化した際に重みを調整

**DBに保存するフィールド**:
```sql
business_rules_score  DECIMAL(4,3),  -- 0.000〜1.000
semantic_score        DECIMAL(4,3),  -- nullable, Phase 2+
historical_score      DECIMAL(4,3),  -- nullable, Phase 2+
total_score           DECIMAL(4,3),  -- 合成後の最終スコア
score_version         VARCHAR(20),   -- 例: "v1-compat", "v2-semantic"
```

**既存システムとの互換性**:
- 既存の `matching_config.py` では `business_rules_weight=0.6`, `semantic_matching_weight=0.3`, `historical_learning_weight=0.1`
- Rust MVP では `business_rules_score` のみを計算し、`total_score = business_rules_score` とする
- Phase 2で Semantic を有効化した際: `total_score = 0.6 × business_rules_score + 0.3 × semantic_score + 0.1 × historical_score`

### SC-9: Skills JSONB フォーマット仕様（Critical）

**統一フォーマット**:
```json
{
  "items": [
    {"skill": "Java", "years": 3},
    {"skill": "Spring Boot", "years": 2},
    {"skill": "AWS", "years": 1}
  ]
}
```

**フィールド定義**:
| フィールド | 型 | 必須 | 説明 | 制約 |
|-----------|-----|------|------|------|
| `items` | Array | ✅ | スキル配列 | 空配列可（`[]`）|
| `items[].skill` | String | ✅ | スキル名（正規化済み） | 空文字不可、最大100文字 |
| `items[].years` | Number | ❌ | 経験年数 | 0以上の整数、NULL/未指定=0とみなす |

**実装**: `sr-common/src/light_rules.rs` の `skills_to_jsonb()` を参照
→ 「軽量ルールでのスキル抽出」セクション参照

**生成ルール（サマリ）**:
1. 空文字を除外
2. スキル名を lowercase に正規化
3. `years` は不明なら `0` を設定
4. `{"items": [...]}` 形式で返す

**⚠️ 実装は1箇所に集約**: 仕様書内でコードを二重に書かない（ドリフト防止）

**使用箇所**:
| テーブル | カラム | 用途 |
|----------|--------|------|
| `projects_enum` | `required_skills` | 案件の必須スキル |
| `projects_enum` | `preferred_skills` | 案件の歓迎スキル（nullable） |
| `bp_talents_enum` | `skills` | BP人材のスキル |
| `talents_enum` | `possessed_skills` | 直人材のスキル |

**⚠️ 禁止事項**:
- 単なる文字列配列 `["Java", "Python"]` は使わない
- `years` を省略しない（不明なら `0` を明示）
- `items` 以外のトップレベルキーを追加しない

**空の場合**:
```json
{"items": []}
```
**⚠️ 空配列の場合は `requires_manual_review=true` を立てる**（SC-1 参照）

### SC-10: ENUM補正マッピング詳細

#### SC-10a: work_area 補正（都道府県 → エリア変換）

**入力パターン → 出力**:
| 入力例 | 出力 | 補足 |
|--------|------|------|
| `"東京"`, `"東京都"`, `"渋谷"`, `"品川"` | `関東` | 主要都市名も対応 |
| `"神奈川"`, `"横浜"`, `"川崎"` | `関東` | |
| `"大阪"`, `"大阪府"`, `"梅田"`, `"難波"` | `近畿` | |
| `"愛知"`, `"名古屋"` | `東海` | |
| `"福岡"`, `"博多"` | `九州・沖縄` | |
| `"北海道"`, `"札幌"` | `北海道・東北` | |
| `"リモート"`, `"在宅"`, `"全国"` | `None` | エリア不問→NULL |
| `"関東"`, `"首都圏"` | `関東` | 直接マッチ |
| `"unknown"`, `"不明"` | `None` | 補正失敗→NULL |

> **📌 正規実装**: [SC-16: エリアENUM統一](#sc-16-エリアenum統一2追加-2025-12-17) を参照。
>
> 実装: `sr-common/src/corrections/area.rs` の `correct_work_area()`
>
> **⚠️ 重複実装禁止**: 上記入出力表のみを参考とし、実装は SC-16 の1箇所のみ。

#### SC-10b: remote_onsite 補正パターン一覧

| 入力パターン | 出力 | 補足 |
|-------------|------|------|
| `フルリモート`, `完全リモート`, `在宅勤務`, `テレワーク100%` | `フルリモート` | |
| `週N出社` (N=1-4), `ハイブリッド`, `一部リモート`, `リモート可` | `リモート併用` | |
| `フル出社`, `常駐`, `客先常駐`, `出社必須`, `オンサイト` | `フル出社` | |
| `応相談`, `不明`, `要確認` | `リモート併用` | デフォルト |

#### SC-10c: flow_dept 補正パターン一覧

| 入力パターン | 出力 | 補足 |
|-------------|------|------|
| `プライム`, `エンド直`, `直請け`, `直接契約` | `エンド直` | |
| `1次`, `一次`, `元請け`, `1次請け` | `1次請け` | |
| `2次`, `二次`, `2次請け` | `2次請け` | |
| `3次`, `三次`, `3次請け` | `3次請け` | |
| `4次以上`, `それ以上`, `4次請け以上` | `4次請け以上` | |
| `プライム〜2次` | `2次請け` | 範囲は最深を採用 |
| `不明`, `unknown` | `不明` | デフォルト |

#### SC-10d: contract_type 補正パターン一覧

| 入力パターン | 出力 | 補足 |
|-------------|------|------|
| `準委任`, `SES`, `業務委託`, `請負` | `準委任契約` | |
| `派遣`, `派遣契約` | `派遣` | |
| `不明` | `準委任契約` | デフォルト |

---

### SC-11: prefilter と詳細スコアの重み差異（意図的な設計）（#4修正: 2025-12-17）

**背景**: prefilter（事前フィルタ）と詳細スコア（business_rules_engine）で重みが異なる。

| コンポーネント | prefilter | business_rules_engine | 差分 |
|---------------|-----------|----------------------|------|
| tanka | `0.25` | `0.25` | - |
| location | `0.15` | `0.15` | - |
| **skills** | **`0.45`** | `0.40` | **+0.05** |
| **experience** | **`0.10`** | `0.15` | **-0.05** |
| contract | `0.05` | `0.05` | - |
| **合計** | `1.00` | `1.00` | - |

**仕様契約**:
- prefilter は候補絞り込みのための**近似**であり、詳細スコア（business_rules_score）と重みが異なってよい
- `prefilter_weight_version` と `detailed_weight_version` を別に管理する
- 重みを変更した場合は `rule_version` を更新すること
- skills に +0.05 を移動して合計 1.0 を維持

**理由**:
- prefilter は500件→50件に絞る段階で、経験年数より単価・スキルを重視したい
- 詳細スコアは最終順位付けで、経験年数も含めたバランスが必要

---

### SC-12: Canary ハッシュテストベクタ（#11追加: 2025-12-17）

**背景**: Canary 判定の実装が Rust/n8n で一致することを保証するテストベクタが必要だった。

**仕様契約**:
- **ゴールデンベクタ**: 4件の固定入力に対する期待ハッシュ値を定義
- **境界テスト**: 空文字、Unicode、長文など境界ケースを網羅
- **クロスプラットフォーム**: Rust + n8n (JavaScript) で同一結果を保証

**実装箇所**:
- コード内定義: 10916行 `【仕様契約 SC-12: Canary ハッシュ検証テストベクタ】`
- Rust: `sr-canary/src/tests.rs`
- n8n: 変更履歴参照

---

### SC-13: ENUM ロード RwLock 移行（#10追加: 2025-12-17）

**背景**: OnceLock による ENUM ロードは一度設定すると変更不可で、テストでリセットできなかった。

**仕様契約**:
- **EnumValues struct**: 全 ENUM の allowed values を一括管理
- **EnumSource enum**: `Database` / `StaticFallback` / `TestMock` の3種を識別
- **RwLock**: テスト時のリセット、ホットリロードに対応
- **静的フォールバック**: DB接続失敗時もpanic しない

**実装箇所**:
- コード内定義: 7255行 `【仕様契約 SC-13: ENUM ロード RwLock 移行】`
- `sr-common/src/enums/loader.rs`

---

### SC-14: PREFECTURE_MAP 統合（#7追加: 2025-12-17）

**背景**: 都道府県の2文字短縮形（北海、神奈、鹿児、和歌）がパース失敗していた。

**仕様契約**:
- **PREFECTURE_MAP**: 2文字短縮形 → 正式名称の HashMap
- **正規化の第一段階**: `correct_todofuken()` の最初に PREFECTURE_MAP を適用
- **対応短縮形**: 北海→北海道、神奈→神奈川県、鹿児→鹿児島県、和歌→和歌山県

**実装箇所**:
- コード内定義: 7324行 `【仕様契約 SC-14: PREFECTURE_MAP 統合】`
- `sr-common/src/corrections/todofuken.rs`

---

### SC-15: (欠番 - SC-KO-UNIFIED に統合)

> **📌 Note**: SC-15 は当初「KoDecision全面適用」として予定されていたが、CD-2 の SC-KO-UNIFIED に統合された。詳細は CD-2 セクション参照。

---

### SC-16: エリアENUM統一（#2追加: 2025-12-17、#5修正: 2025-12-18）

**背景**: sponto-platform と本仕様書で WORK_AREA ENUM の設計が異なる。

**⚠️ sponto-platform との差異（意図的）**:
- **sponto-platform**: 8分離形式 `['北海道', '東北', '関東', '中部', '関西', '中国', '四国', '九州']`
- **本仕様（rust-rules）**: 7結合形式（下表参照）

**理由**: 「中部」の曖昧性排除、甲信越・北陸と東海の分離、沖縄の明示的包含

| sponto-platform (8値) | 本仕様 (7値) | 備考 |
|----------------------|--------------|------|
| `北海道`, `東北` | `北海道・東北` | 結合 |
| `関東` | `関東` | 同一 |
| `中部` | `甲信越・北陸`, `東海` | 2分割（曖昧性排除） |
| `関西` | `近畿` | 名称変更 |
| `中国`, `四国` | `中国・四国` | 結合 |
| `九州` | `九州・沖縄` | 沖縄を明示的に包含 |

**仕様契約**:
- **AREA_ENUMS**: 本仕様独自の7値（sponto-platformとは意図的に異なる）
- **TODOFUKEN_TO_AREA**: 47都道府県 → エリア変換マップ（Rust HashMap）
- **旧形式互換**: `correct_work_area()` で旧形式入力も新形式に変換
  - `"中部"` → `"甲信越・北陸"`
  - `"関西"` → `"近畿"`
  - `"中国"` / `"四国"` → `"中国・四国"`
  - `"九州"` / `"沖縄"` → `"九州・沖縄"`
- **DDL**: `ses.work_area_enum` を7値に変更

**実装箇所**:
- `sr-common/src/corrections/area.rs`: `correct_work_area()`, `AREA_ENUMS`, `TODOFUKEN_TO_AREA`
- DDL: `CREATE TYPE ses.work_area_enum`

---

### SC-17: settlement_range 形式統一（#12追加: 2025-12-17）

**背景**: `extract_settlement_range()` に妥当性チェックとテストがなかった。

**仕様契約**:
- **出力形式**: `"XXX-YYYh"` で統一（VARCHAR(20)に収まる）
- **順序正規化**: min > max の場合は自動スワップ
- **固定時間許可**: min = max を許可（例: `"140-140h"` = 固定稼働案件）
- **妥当性チェック**: 80〜240時間の範囲内のみ有効
  - 80時間未満: 週1-2日程度、案件として異常
  - 240時間超: 過度の残業、法的リスク
- **範囲外**: `None` を返す（手動レビューへ）

| 入力例 | 出力 | 備考 |
|--------|------|------|
| `"精算：140-180h"` | `"140-180h"` | 標準 |
| `"精算：180-140h"` | `"140-180h"` | 自動スワップ |
| `"精算：140-140h"` | `"140-140h"` | 固定稼働（min=max許可） |
| `"精算：40-80h"` | `None` | min < 80 → 無効 |
| `"精算：200-300h"` | `None` | max > 240 → 無効 |

**実装箇所**:
- `sr-extractor/src/extractors/regex.rs`: `extract_settlement_range()`

---

### SC-18: LlmResponse serde 強化（#14追加: 2025-12-17）

**背景**: LLMのレスポンスは型不一致・範囲外・フィールド欠損が頻発するため、堅牢なデシリアライズが必要。

**仕様契約**:
- **型安全**: `LlmExtractedProject` 型付き構造体でLLM応答を受ける
- **フォールバック**: 型不一致 → デフォルト値、範囲外 → クランプまたはデフォルト
- **バリデーション**: `is_valid()` / `validation_issues()` でフィールド検証
- **必須フィールド欠損**: `manual_review` フラグを立てる

**カスタムデシリアライザ**:
| 関数 | 用途 | フォールバック |
|------|------|---------------|
| `deserialize_tanka` | 単価（万円） | `None`（数値以外） |
| `deserialize_skills_array` | スキル配列 | `Vec::new()`（配列以外） |
| `deserialize_option_string` | 文字列 | `None`（空文字含む） |

**LlmStatus enum**:
- `Success`: 全フィールド抽出成功
- `Partial`: 一部フィールドのみ抽出
- `Unknown`: 未知のステータス → `Partial` 扱い

**実装箇所**:
- コード内定義: 12456行 `【仕様契約 SC-18: LlmResponse serde 強化】`
- `sr-extractor/src/llm/response.rs`

---

### CD内定義の仕様契約（参照リスト）

以下の仕様契約は、対応する CD セクション内に詳細定義がある。

| 仕様契約 | 定義箇所 | 内容 |
|---------|---------|------|
| SC-KO-UNIFIED | CD-2 | KO判定フロー統一（KoDecision + MatchResult） |
| SC-LOC | CD-1 | 勤務地スコア計算 |
| SC-FLOW | CD-4 | 商流depth正規化 |
| SC-WEIGHTS | CD-5 | スコア重み定義（→ SC-11 で詳細化） |
| SC-STATION | CD-6 | 駅名正規化 |
| SC-REMOTE | CD-7 | リモート形態補正（二段階設計） |
| SC-SKILL-NORM | CD-8 | スキル正規化の一元化 |
| SC-TANKA | CD-9 | 単価計算パラメータ統一 |
| SC-RECEIVED-AT | CD-10 | 相対日付の基準日定義 |

---

## 📂 PDF抽出モジュール（Feature Flag制御）

**方針**:
PDFテキスト抽出は強力だが依存関係が増えるため、**Feature Flag** で制御する。
MVP初期は `off` でリリースし、安定後に `on` にすることを推奨。

**Cargo.toml 設定**:
```toml
[features]
default = []
pdf-extract = ["dep:pdf-extract"]

[dependencies]
# オプション依存として定義
pdf-extract = { version = "0.7", optional = true }
```

**処理フロー**:

1. 環境変数 `SR_ENABLE_PDF_EXTRACT=1` またはビルド時Featureで有効化判定。
2. 無効(default)の場合: `body_text` のみを抽出対象とする。
3. 有効の場合: 添付ファイルがPDFならテキスト抽出を試みる。
   - **成功**: 抽出テキストを `body_text` に結合、または `original_source_type='pdf_text'` として優先使用。
   - **失敗/空文字**: 画像PDFとみなし、深追いせず `manual_review` へ流す（OCRはMVP対象外）。

**実装イメージ**:

```rust
// sr-extractor/src/pdf.rs

#[cfg(feature = "pdf-extract")]
fn extract_pdf_if_enabled(path: &std::path::Path) -> Option<String> {
    use pdf_extract::extract_text;

    match extract_text(path) {
        Ok(text) if !text.trim().is_empty() => Some(text),
        Ok(_) => {
            log::warn!("PDF appears to be image-based (empty text): {:?}", path);
            None // 画像PDF → manual_review へ
        }
        Err(e) => {
            log::error!("PDF extraction failed: {:?}", e);
            None
        }
    }
}

#[cfg(not(feature = "pdf-extract"))]
fn extract_pdf_if_enabled(_: &std::path::Path) -> Option<String> {
    None // 常に何もしない（Feature無効時）
}
```

**⚠️ スコープ管理**:
- MVP では `pdf-extract` Feature は **off** でリリース
- Phase 2 以降、PDF添付メールの割合を計測してから有効化を検討
- OCR（画像PDF対応）は MVP 対象外。必要になったら別途検討

---

## DDL定義（まとめ）

分散していたDDL定義をここに集約。マイグレーション時に参照すること。

### ⚠️ テーブル命名規則と sponto-platform との関係

**背景**: sponto-platform の既存テーブルは以下の命名：
- `ses.bp_talents` (BPTalent)
- `ses.direct_talents` (DirectTalent)
- `ses.projects` (Project)

**本仕様書のテーブル（`*_enum` suffix）**:

| 本仕様書 | 用途 | sponto-platform との関係 |
|----------|------|-------------------------|
| `ses.extraction_queue` | 抽出キュー | **新規**（Rust抽出システム専用） |
| `ses.projects_enum` | 案件マスタ | 既存 `projects` と**統合可能**（スキーマ互換） |
| `ses.bp_talents_enum` | BP人材 | 既存 `bp_talents` と**統合可能** |
| `ses.talents_enum` | 直人材 | 既存 `direct_talents` と**別テーブル** |
| `ses.anken_emails` | メール保存 | **既存**（n8n が作成） |
| `ses.kpi_daily` | KPIスナップショット | **新規** |

**設計方針（案A採用）**:
- `*_enum` suffix のテーブルは Rust 抽出システムの **結果格納用**
- 既存テーブルとは別物として扱う（後方互換性を保護）
- 将来的に統合が必要なら VIEW で橋渡し
- **⚠️ `projects_enum` は既存システムでも使用されている場合あり → Phase 1 前に DDL 確認必須**

---

### DDL-0: ENUM型定義（前提条件）

**⚠️ sponto-platform では既存のENUM型を使用。新規環境では以下を先に作成する必要あり。**

```sql
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
-- ENUM型定義（すべてのテーブル作成前に実行）
-- ⚠️ 既存環境では既に存在する可能性あり → CREATE TYPE IF NOT EXISTS は使えないため
--    DO $$ ... IF NOT EXISTS ... END $$; ブロックで囲むか、エラーを無視する
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

-- 案件ステータス（#3修正: 2025-12-18 ENUM一覧と統一、ライフサイクル追跡用に5値）
CREATE TYPE ses.project_status_enum AS ENUM ('募集中', '選考中', '募集終了', '成約', 'キャンセル');

-- リモート/出社形態
CREATE TYPE ses.remote_onsite_enum AS ENUM ('フル出社', 'リモート併用', 'フルリモート');

-- 契約形態（案件）
CREATE TYPE ses.contract_type_enum AS ENUM ('準委任契約', '派遣');

-- 商流深さ（案件）
CREATE TYPE ses.flow_dept_enum AS ENUM ('エンド直', '1次請け', '2次請け', '3次請け', '4次請け以上', '不明');

-- 人材商流制限（案件）
CREATE TYPE ses.jinzai_flow_limit_enum AS ENUM ('SPONTO直人材', 'SPONTO一社先まで', '商流制限なし');

-- 技術区分（案件）
CREATE TYPE ses.tech_kubun_enum AS ENUM ('生成AI関連', '人気技術', 'レガシー');

-- エリア（#3修正: 2025-12-18 本仕様独自の7エリア分類を採用）
-- ⚠️ sponto-platformのAREA_ENUMS(8分離形式)とは意図的に異なる
-- 理由: 「中部」の曖昧性排除、甲信越・北陸と東海の分離、沖縄の明示的包含
CREATE TYPE ses.work_area_enum AS ENUM (
    '北海道・東北',
    '関東',
    '甲信越・北陸',
    '東海',
    '近畿',
    '中国・四国',
    '九州・沖縄'
);

-- 都道府県（47都道府県）
CREATE TYPE ses.todofuken_enum AS ENUM (
    '北海道',
    '青森県', '岩手県', '宮城県', '秋田県', '山形県', '福島県',
    '茨城県', '栃木県', '群馬県', '埼玉県', '千葉県', '東京都', '神奈川県',
    '新潟県', '富山県', '石川県', '福井県', '山梨県', '長野県',
    '岐阜県', '静岡県', '愛知県', '三重県',
    '滋賀県', '京都府', '大阪府', '兵庫県', '奈良県', '和歌山県',
    '鳥取県', '島根県', '岡山県', '広島県', '山口県',
    '徳島県', '香川県', '愛媛県', '高知県',
    '福岡県', '佐賀県', '長崎県', '熊本県', '大分県', '宮崎県', '鹿児島県', '沖縄県'
);

-- 日本語スキル
CREATE TYPE ses.japanese_skill_enum AS ENUM ('不要', 'N5', 'N4', 'N3', 'N2', 'N1', 'ネイティブ');

-- 英語スキル
CREATE TYPE ses.english_skill_enum AS ENUM ('不要', '読み書き', '会話', 'ビジネス', '上級ビジネス', 'ネイティブ');

-- 人材稼働状態（#3修正: 2025-12-18 ENUM一覧と統一、ライフサイクル追跡用に4値）
CREATE TYPE ses.talent_availability_status_enum AS ENUM ('営業中', '選考中', '稼働中', '休止中');

-- 人材契約形態
CREATE TYPE ses.talent_contract_type_enum AS ENUM ('正社員', '契約社員', '直個人');

-- 人材商流深さ
CREATE TYPE ses.talent_flow_depth_enum AS ENUM ('1社先', '2社先', '3社先以上');

-- 性別
CREATE TYPE ses.talent_gender_enum AS ENUM ('男性', '女性', 'その他/無回答');
```

---

### DDL-1: ses.extraction_queue

```sql
CREATE TABLE ses.extraction_queue (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    email_subject TEXT NOT NULL,
    email_received_at TIMESTAMPTZ NOT NULL,  -- 相対日付(即日等)の正規化基準
    subject_hash VARCHAR(16) NOT NULL,

    -- キュー状態（3状態のみ: failed は廃止）
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending/processing/completed
    priority INTEGER NOT NULL DEFAULT 50,  -- 0-100, 高い=処理優先度高い（※品質ではない）
    -- ⚠️ priority の意味:
    --   高い値 = 処理を優先すべき = Tier1欠損が多い = LLM補完が必要
    --   100: Tier1欠損あり（優先処理）
    --   50: デフォルト（部分欠損）
    --   10: Tier1満足（低優先、Rustのみで完結可能）
    --   ※「高priority = 高品質」ではないので注意
    locked_by VARCHAR(100),

    -- リトライ管理
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    last_error TEXT,

    -- Rust抽出結果
    partial_fields JSONB,
    decision_reason TEXT,

    -- 抽出方法
    recommended_method VARCHAR(20),  -- rust_recommended / llm_recommended
    final_method VARCHAR(20),  -- rust_completed / llm_completed / manual_review

    -- バージョン
    extractor_version VARCHAR(20),
    rule_version VARCHAR(20),

    -- タイムスタンプ
    created_at TIMESTAMPTZ DEFAULT NOW(),
    processing_started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- パフォーマンス計測
    llm_latency_ms INTEGER,

    -- Canary制御
    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    canary_target BOOLEAN NOT NULL DEFAULT false,

    -- CHECK制約
    CONSTRAINT chk_status CHECK (status IN ('pending', 'processing', 'completed')),
    CONSTRAINT chk_recommended_method CHECK (recommended_method IN ('rust_recommended', 'llm_recommended')),
    CONSTRAINT chk_final_method CHECK (final_method IS NULL OR final_method IN ('rust_completed', 'llm_completed', 'manual_review'))
);

CREATE INDEX idx_extraction_queue_status_priority ON ses.extraction_queue(status, priority DESC, next_retry_at);
CREATE INDEX idx_extraction_queue_message_id ON ses.extraction_queue(message_id);
CREATE INDEX idx_extraction_queue_subject_hash ON ses.extraction_queue(subject_hash, created_at);
CREATE INDEX idx_extraction_queue_canary ON ses.extraction_queue(canary_target, created_at);
```

### DDL-2: ses.projects_enum 完全定義

**⚠️ 重要**: 以下は sponto-platform の実際のスキーマ定義。MVP実装時はこれを真実とする。

```sql
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
-- ses.projects_enum 完全DDL（既存テーブル・参照用）
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CREATE TABLE ses.projects_enum (
    -- 主キー（8桁の案件コード）
    project_code SERIAL PRIMARY KEY,
    CONSTRAINT chk_project_code CHECK (project_code >= 10000000 AND project_code <= 99999999),

    -- 必須フィールド（NOT NULL）
    project_name VARCHAR(255) NOT NULL,
    project_status ses.project_status_enum NOT NULL DEFAULT '募集中',
    hiring_headcount SMALLINT NOT NULL DEFAULT 1,
    remote_onsite ses.remote_onsite_enum NOT NULL DEFAULT 'リモート併用',
    interviews_count SMALLINT NOT NULL DEFAULT 2,
    logic_calculated_score SMALLINT NOT NULL DEFAULT 50,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 単価（万円単位、制約: min <= max、共に > 0）
    monthly_tanka_min INTEGER,
    monthly_tanka_max INTEGER,
    settlement_range VARCHAR(20),  -- 精算幅（例: "140-180h"）※sponto-platformと整合
    logic_calculated_tanka SMALLINT,
    CONSTRAINT chk_tanka_min CHECK (monthly_tanka_min > 0),
    CONSTRAINT chk_tanka_max CHECK (monthly_tanka_max > 0),
    CONSTRAINT chk_tanka_range CHECK (monthly_tanka_min IS NULL OR monthly_tanka_max IS NULL OR monthly_tanka_min <= monthly_tanka_max),

    -- スキル要件
    required_skills JSONB NOT NULL,  -- 形式: {"items": [{"skill": "name", "years": num}]}
    required_skills_keywords TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    preferred_skills JSONB,
    preferred_skills_keywords TEXT[],
    min_experience_years REAL,
    CONSTRAINT chk_experience CHECK (min_experience_years IS NULL OR min_experience_years >= 0),

    -- 勤務地
    work_area ses.work_area_enum,
    work_todofuken ses.todofuken_enum,
    work_station VARCHAR(255),
    onsite_frequency REAL,
    CONSTRAINT chk_onsite_frequency CHECK (onsite_frequency IS NULL OR (onsite_frequency >= 0 AND onsite_frequency <= 7)),

    -- 年齢制限（18-70歳、lower <= upper）
    age_limit_lower SMALLINT,
    age_limit_upper SMALLINT,
    CONSTRAINT chk_age_lower CHECK (age_limit_lower IS NULL OR (age_limit_lower >= 18 AND age_limit_lower <= 70)),
    CONSTRAINT chk_age_upper CHECK (age_limit_upper IS NULL OR (age_limit_upper >= 18 AND age_limit_upper <= 70)),
    CONSTRAINT chk_age_range CHECK (age_limit_lower IS NULL OR age_limit_upper IS NULL OR age_limit_lower <= age_limit_upper),

    -- 契約条件
    is_kojin_ok BOOLEAN DEFAULT true,
    foreigner_allowed BOOLEAN DEFAULT false,
    japanese_skill ses.japanese_skill_enum DEFAULT 'ネイティブ',
    english_skill ses.english_skill_enum DEFAULT '不要',

    -- 商流
    joi_bp_name VARCHAR(255),
    flow_dept ses.flow_dept_enum,
    jinzai_flow_limit ses.jinzai_flow_limit_enum DEFAULT 'SPONTO一社先まで',
    contract_type ses.contract_type_enum DEFAULT '準委任契約',

    -- スケジュール
    start_date VARCHAR(20),  -- '即日' or 'YYYY-MM-DD'
    start_date_raw TEXT,
    start_date_precision VARCHAR(20),
    end_date VARCHAR(20),
    closed_at TIMESTAMPTZ,
    is_long_term BOOLEAN DEFAULT false,

    -- 分類
    project_type TEXT[],
    project_keywords TEXT[],
    tech_kubun ses.tech_kubun_enum,

    -- スコア・試算（レガシー: 0-100 SMALLINT）
    logic_calculated_score_reason TEXT,
    logic_calculated_tanka_reason TEXT,
    ai_estimated_score SMALLINT,
    ai_estimated_score_reason TEXT,
    ai_estimated_tanka SMALLINT,
    ai_estimated_tanka_reason TEXT,
    CONSTRAINT chk_logic_score CHECK (logic_calculated_score >= 0 AND logic_calculated_score <= 100),
    CONSTRAINT chk_ai_score CHECK (ai_estimated_score IS NULL OR (ai_estimated_score >= 0 AND ai_estimated_score <= 100)),

    -- スコア（SC-8準拠: 0.000-1.000 DECIMAL）
    -- ⚠️ logic_calculated_score（レガシー）とは別。Rust 実装では以下を使用
    business_rules_score DECIMAL(4,3),   -- KO通過後の詳細スコア（MVP必須）
    semantic_score DECIMAL(4,3),         -- セマンティックスコア（Phase 2+、nullable）
    historical_score DECIMAL(4,3),       -- 履歴学習スコア（Phase 2+、nullable）
    total_score DECIMAL(4,3),            -- 合成スコア（= business_rules_score × w_business + ...）
    score_version VARCHAR(20) DEFAULT 'v1-compat',  -- スコア算出バージョン
    CONSTRAINT chk_business_rules_score CHECK (business_rules_score IS NULL OR (business_rules_score >= 0 AND business_rules_score <= 1)),
    CONSTRAINT chk_semantic_score CHECK (semantic_score IS NULL OR (semantic_score >= 0 AND semantic_score <= 1)),
    CONSTRAINT chk_historical_score CHECK (historical_score IS NULL OR (historical_score >= 0 AND historical_score <= 1)),
    CONSTRAINT chk_total_score CHECK (total_score IS NULL OR (total_score >= 0 AND total_score <= 1)),

    -- メタデータ
    source_text TEXT,
    bp_contact_info TEXT,
    remarks TEXT,
    message_id VARCHAR(255),
    skill_vector vector(768),  -- pgvector 768次元（sponto-platform と整合、コサイン類似度用）

    -- レビュー制御
    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    manual_review_reason TEXT,

    -- 制約
    CONSTRAINT chk_hiring CHECK (hiring_headcount > 0),
    CONSTRAINT chk_interviews CHECK (interviews_count > 0),
    CONSTRAINT chk_start_date_precision CHECK (
        start_date_precision IS NULL OR
        start_date_precision IN ('exact', 'month', 'early', 'middle', 'late', 'asap', 'unknown')
    ),
    CONSTRAINT uq_projects_enum_message_id UNIQUE (message_id)
);

-- インデックス
CREATE INDEX idx_projects_status ON ses.projects_enum(project_status);
CREATE INDEX idx_projects_skills ON ses.projects_enum USING GIN(required_skills_keywords);
CREATE INDEX idx_projects_tanka ON ses.projects_enum(monthly_tanka_min, monthly_tanka_max);
CREATE INDEX idx_projects_message_id ON ses.projects_enum(message_id);

-- ⚠️ SERIAL シーケンス初期化（8桁コード制約のため必須）
-- SERIAL は 1 から開始するため、CHECK制約 (>= 10000000) を満たすよう初期化
ALTER SEQUENCE ses.projects_enum_project_code_seq RESTART WITH 10000000;
```

### DDL-2a: ses.projects_enum マイグレーション（既存テーブルへの変更）

```sql
-- message_id UNIQUE制約（再処理の冪等性）
ALTER TABLE ses.projects_enum
ADD CONSTRAINT uq_projects_enum_message_id UNIQUE (message_id);

-- required_skills_keywords 追加
ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS required_skills_keywords TEXT[];

UPDATE ses.projects_enum
SET required_skills_keywords = ARRAY[]::text[]
WHERE required_skills_keywords IS NULL;

ALTER TABLE ses.projects_enum
ALTER COLUMN required_skills_keywords SET DEFAULT ARRAY[]::text[];

ALTER TABLE ses.projects_enum
ALTER COLUMN required_skills_keywords SET NOT NULL;

-- start_date 3カラム構成
ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS start_date_raw TEXT;

ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS start_date_precision VARCHAR(20);

ALTER TABLE ses.projects_enum
ADD CONSTRAINT IF NOT EXISTS chk_start_date_precision
CHECK (start_date_precision IN ('exact', 'month', 'early', 'middle', 'late', 'asap', 'unknown')
       OR start_date_precision IS NULL);

-- SC-8準拠: スコアフィールド追加（Phase 1 MVP）
ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS business_rules_score DECIMAL(4,3);

ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS semantic_score DECIMAL(4,3);

ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS historical_score DECIMAL(4,3);

ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS total_score DECIMAL(4,3);

ALTER TABLE ses.projects_enum
ADD COLUMN IF NOT EXISTS score_version VARCHAR(20) DEFAULT 'v1-compat';

-- スコア範囲制約（0.000-1.000）
ALTER TABLE ses.projects_enum
ADD CONSTRAINT IF NOT EXISTS chk_business_rules_score
CHECK (business_rules_score IS NULL OR (business_rules_score >= 0 AND business_rules_score <= 1));

ALTER TABLE ses.projects_enum
ADD CONSTRAINT IF NOT EXISTS chk_semantic_score
CHECK (semantic_score IS NULL OR (semantic_score >= 0 AND semantic_score <= 1));

ALTER TABLE ses.projects_enum
ADD CONSTRAINT IF NOT EXISTS chk_historical_score
CHECK (historical_score IS NULL OR (historical_score >= 0 AND historical_score <= 1));

ALTER TABLE ses.projects_enum
ADD CONSTRAINT IF NOT EXISTS chk_total_score
CHECK (total_score IS NULL OR (total_score >= 0 AND total_score <= 1));
```

### DDL-2b: ses.talents_enum（人材投稿スナップショット）

**⚠️ テーブル名とスキーマに関する重要な設計方針**:

**命名規則の整理**:
| テーブル名 | 用途 | ステータス |
|-----------|------|-----------|
| `ses.bp_talents_enum` | BP人材（既存 n8n + LLM フロー） | 既存・維持 |
| `ses.talents_enum` | 直人材（本Rust仕様書の対象）| 新規作成 |
| `ses.direct_talents` | sponto-platform の archived 設計で使用 | 非推奨・使用禁止 |

**注意**: sponto-platform の archived フォルダには `ses.direct_talents` という名前が残っているが、
本仕様では **`ses.talents_enum`** に統一する。混同を避けるため `direct_talents_enum` は使わないこと。

既存システムには `bp_talents_enum` テーブルが存在する。本仕様では `ses.talents_enum` を使用するが、
MVPでは以下のVIEW戦略で既存資産との整合性を保つ。

```sql
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
-- VIEW戦略（MVPで推奨）
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
-- 目的: Rustコードは ses.talents_enum を参照するが、
--       既存テーブル bp_talents_enum を壊さない
--
-- 方針:
--   A) bp_talents_enum が既存で正なら → ses.talents_enum を VIEW として作成
--   B) ses.talents_enum を新規テーブルとして作成（直人材専用）
--
-- MVP推奨: 方針B（直人材用の新規テーブル）
--   - bp_talents_enum は BP人材用（既存の n8n + LLM フロー）
--   - ses.talents_enum は 直人材用（本システム）
--   - 将来的に統合が必要なら VIEW で橋渡し
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

CREATE SCHEMA IF NOT EXISTS ses;

-- 方針A の場合（既存 bp_talents_enum を流用）:
-- CREATE OR REPLACE VIEW ses.talents_enum AS
-- SELECT * FROM bp_talents_enum;

-- 方針B の場合（直人材用に新規作成、以下のDDL）:
```

### DDL-2c: ses.bp_talents_enum 完全定義（参照用・既存テーブル）

**⚠️ 重要**: BP人材用の既存テーブル。Rust MVPでは直接使用しないが、マッチング時に参照する。

```sql
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
-- ses.bp_talents_enum 完全DDL（既存テーブル・参照用）
-- ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CREATE TABLE ses.bp_talents_enum (
    -- 主キー（8桁の人材コード）
    talent_code SERIAL PRIMARY KEY,
    CONSTRAINT chk_talent_code CHECK (talent_code >= 10000000 AND talent_code <= 99999999),

    -- 必須フィールド（NOT NULL）
    talent_name VARCHAR(255) NOT NULL,
    company_name VARCHAR(255) NOT NULL,
    availability_status ses.talent_availability_status_enum NOT NULL DEFAULT '営業中',
    primary_contract_type ses.talent_contract_type_enum NOT NULL,
    flow_depth ses.talent_flow_depth_enum NOT NULL,
    is_shienhi_ok BOOLEAN NOT NULL DEFAULT true,
    japanese_skill ses.japanese_skill_enum NOT NULL DEFAULT 'ネイティブ',
    english_skill ses.english_skill_enum NOT NULL DEFAULT '不要',
    skills JSONB NOT NULL,  -- 形式: {"items": [{"skill": "name", "years": num}]}
    logic_calculated_score SMALLINT NOT NULL DEFAULT 50,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 個人情報
    birth_year SMALLINT,
    gender ses.talent_gender_enum,
    nationality VARCHAR(50),
    CONSTRAINT chk_birth_year CHECK (birth_year IS NULL OR (birth_year >= 1950 AND birth_year <= 2010)),

    -- 勤務地・希望
    residential_area ses.work_area_enum,
    residential_todofuken ses.todofuken_enum,
    nearest_station VARCHAR(255),
    desired_remote_onsite ses.remote_onsite_enum,

    -- スキル・経験
    total_experience_years REAL,
    skill_keywords TEXT[],
    specialization TEXT[],
    certifications JSONB,
    work_history JSONB,
    CONSTRAINT chk_experience CHECK (total_experience_years IS NULL OR total_experience_years >= 0),

    -- 希望条件
    desired_project_type TEXT[],
    desired_project_keywords TEXT[],
    ng_keywords TEXT[],
    work_style_tags TEXT[],

    -- 単価（万円単位）
    -- ⚠️ Rust struct では desired_price_min として参照（フィールド名マッピングに注意）
    desired_monthly_tanka INTEGER,
    CONSTRAINT chk_tanka CHECK (desired_monthly_tanka IS NULL OR desired_monthly_tanka > 0),

    -- メタデータ
    availability_date VARCHAR(20),  -- '即日' or 'YYYY-MM-DD'
    eigyo_closed_at TIMESTAMPTZ,
    secondary_contract_type ses.talent_contract_type_enum,
    appeal_points TEXT,
    resume_text_refined TEXT,
    message_id VARCHAR(255),
    contact_info TEXT,
    remarks TEXT,
    resume_file_path TEXT,
    source_text TEXT,

    -- スコア・試算
    logic_calculated_score_reason TEXT,
    logic_calculated_tanka SMALLINT,
    logic_calculated_tanka_reason TEXT,
    ai_estimated_score SMALLINT,
    ai_estimated_score_reason TEXT,
    ai_estimated_tanka SMALLINT,
    ai_estimated_tanka_reason TEXT,
    CONSTRAINT chk_logic_score CHECK (logic_calculated_score >= 0 AND logic_calculated_score <= 100),
    CONSTRAINT chk_ai_score CHECK (ai_estimated_score IS NULL OR (ai_estimated_score >= 0 AND ai_estimated_score <= 100)),

    -- ベクトル検索
    skill_vector vector(768),  -- pgvector 768次元（sponto-platform と整合、コサイン類似度用）

    -- レビュー制御
    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    manual_review_reason TEXT
);

-- インデックス
CREATE INDEX idx_bp_talents_status ON ses.bp_talents_enum(availability_status);
CREATE INDEX idx_bp_talents_skills ON ses.bp_talents_enum USING GIN(skill_keywords);
CREATE INDEX idx_bp_talents_tanka ON ses.bp_talents_enum(desired_monthly_tanka);
CREATE INDEX idx_bp_talents_message_id ON ses.bp_talents_enum(message_id);

-- ⚠️ SERIAL シーケンス初期化（8桁コード制約のため必須）
ALTER SEQUENCE ses.bp_talents_enum_talent_code_seq RESTART WITH 10000000;
```

**ses.anken_emails について**:
- このテーブルは Gmail → n8n → PostgreSQL の既存フローで作成される前提
- 存在しない場合は Rust 側で新規作成するか、上流の n8n ワークフローで作成する
- MVP では「存在する前提」で進め、存在しない場合はエラーで早期終了する

**ses.anken_emails DDL（参照用・既存テーブル）**:
```sql
-- ⚠️ このテーブルは n8n ワークフローで作成済みの前提
-- Rust 側は SELECT のみ行う（INSERT/UPDATE はしない）
CREATE TABLE IF NOT EXISTS ses.anken_emails (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,  -- Gmail Message ID (一意キー)
    sender_address VARCHAR(255),              -- 送信者メールアドレス
    sender_name VARCHAR(255),                 -- 送信者名
    subject VARCHAR(500),                     -- メール件名
    received_at TIMESTAMPTZ NOT NULL,         -- 受信日時（相対日付の基準）
    occurrence_count INTEGER DEFAULT 1,       -- 同一メールの出現回数（重複検出用）
    thread_id VARCHAR(255),                   -- GmailスレッドID
    body_text TEXT,                           -- メール本文 (最大3000文字)
    created_at TIMESTAMPTZ DEFAULT NOW()      -- DB登録日時
);

-- Rust からの検索用インデックス
CREATE INDEX IF NOT EXISTS idx_anken_emails_received_at ON ses.anken_emails (received_at DESC);
CREATE INDEX IF NOT EXISTS idx_anken_emails_message_id ON ses.anken_emails (message_id);
```

**列名互換性**（n8n側のバリエーション対応）:
| Rust 内部変数 | 可能な列名 | 優先度 |
|--------------|-----------|--------|
| `body_text` | `body_text`, `source_text` | 環境変数 `SR_ANKEN_EMAILS_BODY_COL` で指定 |
| `received_at` | `received_at`, `created_at` | `received_at` 優先、なければ `created_at` |

```sql
-- ⚠️ 重要: このテーブルは「ユニークな人物」ではなく
-- 「受信したメール1通ごとの人材情報スナップショット」として扱う。
-- 人物統合（名寄せ）は Phase 2 以降で talent_fingerprint を用いて実施する。

CREATE TABLE ses.talents_enum (
    id BIGSERIAL PRIMARY KEY,         -- 内部管理ID
    message_id VARCHAR(255) NOT NULL, -- 参照元メールID

    -- ソース管理
    received_at TIMESTAMPTZ NOT NULL, -- 相対日付(即日等)の基準
    source_text TEXT,                 -- 抽出元のテキスト（本文 or PDFテキスト）
    original_source_type VARCHAR(20), -- 'email_body', 'pdf_text'

    -- 重複検知用 (Soft Dedup)
    talent_fingerprint TEXT,          -- hash(skills + station) 等。NULL許容

    -- Tier 1: マッチング必須項目
    min_experience_years INTEGER,
    desired_price_min INTEGER,        -- 単位: 万円
    available_date_raw TEXT,          -- "即日" 等の元テキスト
    available_date DATE,              -- 正規化後（検索用）

    -- スキル (重要: マッチング用)
    -- 形式: {"items": [{"skill": "Java", "years": 3}]}
    possessed_skills JSONB NOT NULL DEFAULT '{"items": []}',
    possessed_skills_keywords TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],

    -- Tier 2/3: 補足項目
    talent_name VARCHAR(100),         -- イニシャル含む
    age INTEGER,                      -- 推定年齢 (NULL許容)
    gender VARCHAR(10),               -- 'male', 'female', 'unknown'
    nearest_station VARCHAR(100),     -- 正規化前テキスト
    desired_price_max INTEGER,        -- 単位: 万円
    work_style VARCHAR(50),           -- 'remote', 'onsite', 'hybrid'
    summary_text TEXT,                -- 自己PRなど

    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    manual_review_reason TEXT,        -- 破綻対策: なぜ手動レビューか
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 制約: 同じメールを何度も処理しないための冪等性担保
    CONSTRAINT uq_talents_enum_message_id UNIQUE (message_id)
);

-- インデックス
CREATE INDEX idx_talents_skills ON ses.talents_enum USING GIN (possessed_skills_keywords);
CREATE INDEX idx_talents_price_min ON ses.talents_enum (desired_price_min);
CREATE INDEX idx_talents_fingerprint ON ses.talents_enum (talent_fingerprint);
CREATE INDEX idx_talents_available_date ON ses.talents_enum (available_date);
```

### DDL-3: ses.kpi_daily

```sql
CREATE TABLE ses.kpi_daily (
    measured_date DATE PRIMARY KEY,

    -- LLM集中率（副次効果）
    total_jobs INTEGER NOT NULL,
    rust_completed_count INTEGER NOT NULL,
    llm_count INTEGER NOT NULL,
    manual_review_count INTEGER NOT NULL,
    rust_pct REAL NOT NULL,
    llm_pct REAL NOT NULL,
    manual_review_pct REAL NOT NULL,

    -- 品質監視
    tier1_success_pct REAL,
    tier2_success_pct REAL,
    skills_empty_pct REAL,  -- スキル空配列率（破綻対策）

    -- パフォーマンス（⚠️ PERCENTILE_CONT は明示キャスト必須）
    p50_queue_wait_ms INTEGER,
    p90_queue_wait_ms INTEGER,
    p50_llm_latency_ms INTEGER,
    p90_llm_latency_ms INTEGER,
    p99_llm_latency_ms INTEGER,

    -- 運用監視（⚠️ failed ステータスは廃止済み）
    stuck_count INTEGER,  -- 現時点で5分以上 processing 状態のジョブ数
    max_retry_reached_count INTEGER,  -- retry_count >= 3 のジョブ数
    high_retry_count INTEGER,  -- retry_count >= 1 のジョブ数

    -- コスト試算
    estimated_cost_usd REAL,

    created_at TIMESTAMPTZ DEFAULT NOW()
);

COMMENT ON TABLE ses.kpi_daily IS 'Daily KPI snapshot for Phase 2-4 monitoring';
```

### DDL-4: 全フィールドデフォルト値一覧（MVP実装用）

**⚠️ MVP実装時、フィールドが抽出できなかった場合のデフォルト値を一覧化**

#### projects_enum デフォルト値

| フィールド | デフォルト値 | 型 | NULL許容 | 備考 |
|-----------|-------------|-----|----------|------|
| `project_status` | `'募集中'` | ENUM | ❌ | 新規案件は常に募集中 |
| `hiring_headcount` | `1` | SMALLINT | ❌ | 未記載なら1名想定 |
| `remote_onsite` | `'リモート併用'` | ENUM | ❌ | 最も柔軟な選択肢 |
| `interviews_count` | `2` | SMALLINT | ❌ | 業界標準 |
| `logic_calculated_score` | `50` | SMALLINT | ❌ | 中央値 |
| `monthly_tanka_min` | `NULL` | INTEGER | ✅ | 抽出失敗→手動レビュー |
| `monthly_tanka_max` | `NULL` | INTEGER | ✅ | 抽出失敗→手動レビュー |
| `required_skills` | `'{"items":[]}'` | JSONB | ❌ | 空配列で NOT NULL 満たす |
| `required_skills_keywords` | `ARRAY[]::TEXT[]` | TEXT[] | ❌ | 空配列 + 手動レビュー |
| `work_area` | `NULL` | ENUM | ✅ | 抽出失敗→NULL |
| `work_todofuken` | `NULL` | ENUM | ✅ | 抽出失敗→NULL |
| `age_limit_lower` | `NULL` | SMALLINT | ✅ | 制限なし |
| `age_limit_upper` | `NULL` | SMALLINT | ✅ | 制限なし |
| `is_kojin_ok` | `true` | BOOLEAN | ✅ | デフォルトOK |
| `foreigner_allowed` | `false` | BOOLEAN | ✅ | 保守的にNG |
| `japanese_skill` | `'ネイティブ'` | ENUM | ✅ | 最も厳しい条件 |
| `english_skill` | `'不要'` | ENUM | ✅ | 英語要件なし |
| `flow_dept` | `NULL` | ENUM | ✅ | 抽出失敗→NULL |
| `jinzai_flow_limit` | `'SPONTO一社先まで'` | ENUM | ✅ | 標準制限 |
| `contract_type` | `'準委任契約'` | ENUM | ✅ | SES標準 |
| `start_date` | `NULL` | VARCHAR | ✅ | 抽出失敗→NULL |
| `is_long_term` | `false` | BOOLEAN | ✅ | 保守的 |
| `tech_kubun` | `NULL` | ENUM | ✅ | 抽出失敗→NULL |
| `requires_manual_review` | `false` | BOOLEAN | ❌ | 問題なければfalse |

#### bp_talents_enum デフォルト値

| フィールド | デフォルト値 | 型 | NULL許容 | 備考 |
|-----------|-------------|-----|----------|------|
| `availability_status` | `'営業中'` | ENUM | ❌ | 新規登録は営業中 |
| `is_shienhi_ok` | `true` | BOOLEAN | ❌ | デフォルトOK |
| `japanese_skill` | `'ネイティブ'` | ENUM | ❌ | 日本人前提 |
| `english_skill` | `'不要'` | ENUM | ❌ | 英語不問 |
| `logic_calculated_score` | `50` | SMALLINT | ❌ | 中央値 |
| `birth_year` | `NULL` | SMALLINT | ✅ | 不明なら NULL |
| `gender` | `NULL` | ENUM | ✅ | 不明なら NULL |
| `residential_area` | `NULL` | ENUM | ✅ | 抽出失敗→NULL |
| `desired_remote_onsite` | `NULL` | ENUM | ✅ | 希望不明→NULL |
| `total_experience_years` | `NULL` | REAL | ✅ | 抽出失敗→NULL |
| `desired_monthly_tanka` | `NULL` | INTEGER | ✅ | 希望不明→NULL |
| `requires_manual_review` | `false` | BOOLEAN | ❌ | 問題なければfalse |

#### extraction_queue デフォルト値

| フィールド | デフォルト値 | 型 | NULL許容 | 備考 |
|-----------|-------------|-----|----------|------|
| `status` | `'pending'` | TEXT | ❌ | 初期状態 |
| `priority` | `50` | INTEGER | ❌ | 中優先度 |
| `retry_count` | `0` | INTEGER | ❌ | 未リトライ |
| `canary_target` | `false` | BOOLEAN | ❌ | 非カナリア |

#### Rust コード内でのデフォルト値適用

```rust
// sr-common/src/models.rs

impl Default for ProjectExtraction {
    fn default() -> Self {
        Self {
            project_name: None,
            project_status: Some("募集中".to_string()),
            hiring_headcount: Some(1),
            remote_onsite: Some("リモート併用".to_string()),
            interviews_count: Some(2),
            logic_calculated_score: Some(50),
            monthly_tanka_min: None,
            monthly_tanka_max: None,
            required_skills: Some(serde_json::json!({"items": []})),
            required_skills_keywords: Some(vec![]),
            work_area: None,
            work_todofuken: None,
            age_limit_lower: None,
            age_limit_upper: None,
            is_kojin_ok: Some(true),
            foreigner_allowed: Some(false),
            japanese_skill: Some("ネイティブ".to_string()),
            english_skill: Some("不要".to_string()),
            flow_dept: None,
            jinzai_flow_limit: Some("SPONTO一社先まで".to_string()),
            contract_type: Some("準委任契約".to_string()),
            start_date: None,
            start_date_raw: None,
            start_date_precision: None,
            is_long_term: Some(false),
            tech_kubun: None,
            requires_manual_review: false,
            manual_review_reason: None,
            // ... メタフィールド
        }
    }
}
```

**⚠️ デフォルト値適用のタイミング**:
1. **抽出フェーズ**: 抽出できなかったフィールドは `None` のまま
2. **INSERT前フェーズ**: `None` のフィールドにデフォルト値を適用
3. **手動レビューフラグ**: Tier1フィールドが `None` のまま → `requires_manual_review=true`

---

## Dangerous Assumptions

### 🚨 Critical Assumptions（検証必須）

| # | Assumption | リスク | 検証タイミング | 対策 |
|---|-----------|-------|---------------|------|
| **1** | **Tier1フィールドがregexで Gate(75%+) 抽出可能** | 実際は50-60%しか取れず、説明可能な処理割合が目標未達 | Phase 2（Week 4-5） | パターン見直し or Tier1条件緩和（3/4でOKに変更） |
| **2** | **メールの50%以上がTier1完全+Tier2半分以上** | 実際は30%程度で、ルールベース処理率が想定を下回る | Phase 2（Week 4-5） | Tier2条件緩和（0/2でもOKに変更）or LLM改善 |
| **3** | **PostgreSQLキューがレイテンシ120秒以内** | 実際は5分待機が発生、ユーザー体感悪化 | Phase 3（Week 6-7） | Redis移行 or worker数増加 |
| **4** | **既存LLM品質がA-B gradeで80%以上** | 実際はC-D多数で、Rust比較が困難 | Phase 2（Week 4-5） | 既存LLM品質を先に改善 or 比較基準変更 |
| **5** | **message_id + subject_hashで重複排除が暫定運用可能** | 実際は件名微妙に違う重複多発 | Phase 2（Week 4-5） | Later: SimHash追加（Phase 5） |
| **6** | **`required_skills_keywords` (TEXT[]) の挙動理解** | ⚠️ **strategy.py の挙動**: 空/NULL → 必須スキル判定スキップ（"合格"扱い）、`["未指定"]` → 判定が走り全員KO。従って `["未指定"]` は危険！空のまま残し `requires_manual_review=true` で制御するのが安全 | Phase 1（実装時） | 空配列は許容。スキル取れない案件は `requires_manual_review=true`。**strategy.py に正規化（lowercase+aliases）を追加**（Phase 2推奨） |
| **7** | **ENUM 値の補正（work_todofuken/remote_onsite/flow_dept）** | Rust 抽出値が ENUM と 1文字でも違うと INSERT 失敗。「東京都」→「東京」、「リモート可」→「リモート併用」等の補正が必要 | Phase 1（実装時） | ✅ **DB ENUM を真実とする方式に改修**: `load_enum_values()` で起動時に `pg_enum` から読み込み。京都バグ（`strip_suffix` 方式）・北海道特別扱いも修正済み。補正失敗なら `requires_manual_review=true` |
| **8** | **`required_skills` (JSONB) のスキーマが既存LLM出力と一致** | 軽量ルールが `json!(skills)` だけで済ませると、既存が `{"keywords":[...], "must":[...]}` 形式の場合に matching/UI が壊れる。**最大の実務トラップ** | Phase 1（実装前） | **実装前に既存 `projects_enum.required_skills` の実データを3件見て形状を確認**。`skills_to_jsonb()` を既存スキーマに合わせる。DDL で確認済み: `{"items": [{"skill": "...", "years": 0}]}` 形式 |

---

### ⚠️ Medium Assumptions（監視必要）

| # | Assumption | リスク | 検証タイミング |
|---|-----------|-------|---------------|
| **9** | **Rust処理が100ms/email以内** | 実際は500ms以上で、n8nタイムアウト | Phase 3 |
| **10** | **正規表現が表記ゆれに対応可能** | "スキル見合い"等の曖昧表現が多発 | Phase 2 |
| **11** | **既存Python LLM APIがそのまま使える** | インターフェース変更必要で工数増 | Phase 1 |

---

### Mitigation Plan（リスク対策）

```
IF Assumption #1 失敗 (Tier1抽出 < 85%)
  → Tier1条件を 4/4 → 3/4 に緩和
  → Target: 40-45% API削減でも十分ROI

IF Assumption #2 失敗 (メール分布が偏っている)
  → Tier2条件を 1/2 → 0/2 に緩和
  → Phase 5でSynonym辞書追加

IF Assumption #3 失敗 (PostgreSQLキュー遅い)
  → Redis/Sidekiqに移行（Later拡張）
  → worker数を2→5に増加

IF Assumption #4 失敗 (既存LLM品質が低い)
  → Rust vs LLM比較を"相対改善"に変更
  → 既存LLM改善を優先（別タスク）
```

---

## 🔧 スキルマッチング正規化（Rust 実装）

### 背景

sponto-platform は Rust への移行対象。既存 `strategy.py` のロジックは参考にするが、
**Python ファイル自体は使わない**。従って正規化ロジックは **Rust (sr-common) に実装**する。

### 既存 strategy.py の問題点（移行時に修正すべき点）

```python
# 既存の問題コード（参考）
req_skill_set = set(project.required_skills_keywords or [])
talent_skill_set = set(talent.skill_keywords or [])
matched = req_skill_set.intersection(talent_skill_set)  # ⚠️ 完全文字列一致
```

- `set.intersection()` による**完全文字列一致**
- 大文字小文字区別: "Java" ≠ "java"
- エイリアス未対応: "JavaScript" ≠ "js"

### Rust 実装（sr-common/src/skill_normalizer.rs）

```rust
use std::collections::HashMap;
use std::sync::LazyLock;

/// スキルエイリアス → 正規形のマッピング（O(1) ルックアップ）
static ALIAS_TO_CANONICAL: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    // NOTE: この辞書は sponto-platform/apps/business-api/app/core/skill_processor.py の
    // SKILL_ALIASES と完全同期すること（67 canonical skills, 183+ aliases）
    let aliases: &[(&str, &[&str])] = &[
        // JavaScript ecosystem
        ("javascript", &["js", "javascript", "java script", "ecmascript", "es6", "es2015", "es2016", "es2017", "es2018"]),
        ("typescript", &["ts", "typescript", "type script"]),
        ("nodejs", &["node.js", "node js", "nodejs", "node"]),
        ("npm", &["npm", "node package manager"]),

        // Frontend frameworks
        ("react", &["reactjs", "react.js", "react js", "react", "react16", "react17", "react18"]),
        ("vue", &["vue.js", "vuejs", "vue js", "vue", "vue2", "vue3"]),
        ("angular", &["angularjs", "angular.js", "angular", "angular2", "angular4", "angular8", "angular10", "angular12"]),
        ("svelte", &["sveltejs", "svelte.js", "svelte"]),
        ("nextjs", &["next.js", "nextjs", "next js"]),
        ("nuxt", &["nuxtjs", "nuxt.js", "nuxt js", "nuxt"]),

        // CSS and styling
        ("css", &["css", "css3", "cascading style sheets"]),
        ("sass", &["scss", "sass", "syntactically awesome style sheets"]),
        ("bootstrap", &["bootstrap", "bootstrap3", "bootstrap4", "bootstrap5"]),
        ("tailwind", &["tailwindcss", "tailwind css", "tailwind"]),

        // Backend frameworks
        ("spring", &["spring boot", "springboot", "spring framework", "springframework", "spring"]),
        ("django", &["django rest framework", "drf", "django framework", "django"]),
        ("flask", &["flask framework", "python flask", "flask"]),
        ("express", &["express.js", "expressjs", "express js", "express framework", "express"]),
        ("fastapi", &["fast api", "fastapi framework", "fastapi"]),
        ("laravel", &["laravel framework", "php laravel", "laravel"]),

        // Databases
        ("postgresql", &["postgres", "pg", "postgresql", "postgre sql"]),
        ("mysql", &["my sql", "mysql", "mariadb"]),
        ("mongodb", &["mongo", "mongo db", "mongodb", "nosql"]),
        ("redis", &["redis cache", "redis db", "redis"]),
        ("elasticsearch", &["elastic search", "es", "elasticsearch"]),
        ("sqlite", &["sqlite3", "sql lite", "sqlite"]),

        // Cloud platforms
        ("aws", &["amazon web services", "amazon aws", "aws cloud", "aws"]),
        ("gcp", &["google cloud platform", "google cloud", "gcp"]),
        ("azure", &["microsoft azure", "ms azure", "azure cloud", "azure"]),
        ("firebase", &["google firebase", "firebase platform", "firebase"]),

        // Programming languages
        ("python", &["python3", "python 3", "py", "python2.7", "python"]),
        ("java", &["java8", "java11", "java17", "openjdk", "oracle java", "java"]),
        ("csharp", &["c#", "c sharp", "csharp", ".net", "dotnet"]),
        ("cplusplus", &["c++", "cpp", "c plus plus"]),
        ("golang", &["go", "golang", "go lang"]),
        ("rust", &["rust lang", "rust language", "rust"]),
        ("php", &["php7", "php8", "hypertext preprocessor", "php"]),
        ("ruby", &["ruby lang", "ruby language", "ruby"]),
        ("swift", &["swift lang", "ios swift", "swift"]),
        ("kotlin", &["kotlin lang", "kotlin jvm", "kotlin"]),

        // DevOps and tools
        ("docker", &["containerization", "docker container", "docker"]),
        ("kubernetes", &["k8s", "kube", "kubernetes orchestration", "kubernetes"]),
        ("jenkins", &["jenkins ci", "jenkins ci/cd", "jenkins"]),
        ("git", &["version control", "git scm", "github", "gitlab", "git"]),
        ("terraform", &["infrastructure as code", "iac", "terraform"]),
        ("ansible", &["configuration management", "ansible"]),

        // AI/ML terms (Japanese context)
        ("ai", &["artificial intelligence", "machine learning", "ml", "人工知能", "ai技術", "ai"]),
        ("ml", &["machine learning", "artificial intelligence", "ai", "機械学習", "ml"]),
        ("llm", &["large language model", "大規模言語モデル", "language model", "llm"]),
        ("chatgpt", &["gpt", "openai", "generative ai", "生成ai", "chatgpt"]),
        ("deeplearning", &["deep learning", "neural networks", "ディープラーニング", "deeplearning"]),
        ("tensorflow", &["tensor flow", "tf", "tensorflow"]),
        ("pytorch", &["torch", "py torch", "pytorch"]),

        // Testing frameworks
        ("jest", &["jest testing", "jest framework", "jest"]),
        ("cypress", &["cypress testing", "e2e testing", "cypress"]),
        ("selenium", &["selenium webdriver", "selenium testing", "selenium"]),
        ("junit", &["junit testing", "java testing", "junit"]),
        ("pytest", &["python testing", "py test", "pytest"]),

        // Mobile development
        ("reactnative", &["react native", "react-native", "rn", "reactnative"]),
        ("flutter", &["flutter framework", "dart flutter", "flutter"]),
        ("xamarin", &["xamarin forms", "microsoft xamarin", "xamarin"]),
        ("ionic", &["ionic framework", "ionic cordova", "ionic"]),

        // Data and analytics
        ("spark", &["apache spark", "spark streaming", "spark"]),
        ("hadoop", &["apache hadoop", "hadoop ecosystem", "hadoop"]),
        ("kafka", &["apache kafka", "kafka streaming", "kafka"]),
        ("pandas", &["python pandas", "data analysis", "pandas"]),
        ("numpy", &["numerical python", "numpy array", "numpy"]),
    ];

    let mut map = HashMap::new();
    for (canonical, alias_list) in aliases {
        map.insert(*canonical, *canonical);
        for alias in *alias_list {
            map.insert(*alias, *canonical);
        }
    }
    map
});

/// スキル文字列を正規形に変換（O(1)）
pub fn normalize_skill(skill: &str) -> String {
    let s = skill.trim().to_lowercase();
    ALIAS_TO_CANONICAL
        .get(s.as_str())
        .map(|c| c.to_string())
        .unwrap_or(s)
}

/// スキル配列を正規化した HashSet に変換
pub fn normalize_skill_set(skills: &[String]) -> std::collections::HashSet<String> {
    skills.iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_skill(s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_insensitive() {
        assert_eq!(normalize_skill("Java"), "java");
        assert_eq!(normalize_skill("PYTHON"), "python");
    }

    #[test]
    fn test_aliases() {
        assert_eq!(normalize_skill("js"), "javascript");
        assert_eq!(normalize_skill("k8s"), "kubernetes");
        assert_eq!(normalize_skill("c#"), "csharp");
    }

    #[test]
    fn test_unknown_skill() {
        // 未知のスキルは lowercase のまま返す
        assert_eq!(normalize_skill("MyCustomFramework"), "mycustomframework");
    }
}
```

### スキルマッチング実装（sr-common/src/matching.rs）

```rust
use crate::skill_normalizer::normalize_skill_set;
use std::collections::HashSet;

/// マッチング閾値（環境変数で調整可能）
fn get_skill_match_threshold() -> f64 {
    std::env::var("SR_SKILL_MATCH_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.3)  // デフォルト 30%
}

#[derive(Debug, Clone)]
pub struct SkillMatchResult {
    pub is_knockout: bool,
    pub match_percentage: f64,
    pub matched_skills: Vec<String>,
    pub reason: String,
}

/// 必須スキルのマッチング判定（strategy.py の _check_required_skills 相当）
pub fn check_required_skills(
    project_skills: &[String],
    talent_skills: &[String],
) -> SkillMatchResult {
    let req_skill_set = normalize_skill_set(project_skills);

    // 必須スキル要件がなければ合格（strategy.py と同じ挙動）
    if req_skill_set.is_empty() {
        return SkillMatchResult {
            is_knockout: false,
            match_percentage: 1.0,
            matched_skills: vec![],
            reason: "必須スキル要件なし".to_string(),
        };
    }

    let talent_skill_set = normalize_skill_set(talent_skills);
    let matched: HashSet<_> = req_skill_set.intersection(&talent_skill_set).cloned().collect();
    let match_percentage = matched.len() as f64 / req_skill_set.len() as f64;

    let threshold = get_skill_match_threshold();
    let is_knockout = match_percentage < threshold;

    SkillMatchResult {
        is_knockout,
        match_percentage,
        matched_skills: matched.into_iter().collect(),
        reason: if is_knockout {
            format!(
                "必須スキルとのマッチ率が{:.0}%であり、基準の{:.0}%に達していません",
                match_percentage * 100.0,
                threshold * 100.0
            )
        } else {
            format!(
                "必須スキル{}件中{}件({:.0}%)に合致",
                req_skill_set.len(),
                matched.len(),
                match_percentage * 100.0
            )
        },
    }
}
```

### skills_to_keywords の更新

```rust
// sr-llm-worker/src/light_rules.rs
use sr_common::skill_normalizer::normalize_skill;

/// 軽量ルール抽出結果を required_skills_keywords TEXT[] 形式に変換
/// ⚠️ 正規化を適用（sr-common の normalize_skill を使用）
pub fn skills_to_keywords(skills: Vec<String>) -> Vec<String> {
    skills.into_iter()
        .map(|s| normalize_skill(&s))  // エイリアス正規化も含む
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .collect()
}
```

### 環境変数

```bash
# スキルマッチング閾値（デフォルト 0.3 = 30%）
SR_SKILL_MATCH_THRESHOLD=0.3
```

### クレート構成の更新

```
sr-common/
├── src/
│   ├── lib.rs
│   ├── types.rs
│   ├── normalize.rs          # normalize_subject 等
│   ├── decision.rs           # LlmDecision, should_call_llm
│   ├── skill_normalizer.rs   # ⬅️ NEW: スキル正規化
│   └── matching.rs           # ⬅️ NEW: マッチングロジック
```

---

## 🚨 Operational Toggles（運用トグル・緊急停止スイッチ）

### 即時ロールバック用の環境変数

**方針**: 「品質落ちたらロールバック」を**コード変更なしで即座に実行**できるようにする。

**3つの緊急トグル**:

```bash
# 1. 抽出サービス停止（既存LLMに全戻し）
SR_DISABLE_EXTRACTOR=1

# 2. 強制LLM モード（Rust抽出は動くが、worker が常に LLM 呼び出し）
SR_FORCE_LLM=1

# 3. 強制手動レビュー（疑わしいものは全て manual_review）
SR_FORCE_MANUAL_REVIEW=1
```

---

### トグルの使い分け

| 状況 | トグル | 効果 | 復旧時間 |
|------|--------|------|---------|
| **重大品質劣化** | `SR_DISABLE_EXTRACTOR=1` | sr-extractor が即座に停止。queue に積まない。既存 Python LLM で全処理 | **即時**（5分以内） |
| **Rust 抽出に問題** | `SR_FORCE_LLM=1` | queue は動くが、worker が Tier 判定を無視して常に LLM 呼び出し | **即時**（再起動のみ） |
| **ENUM 補正に問題** | `SR_FORCE_MANUAL_REVIEW=1` | worker が全て `requires_manual_review=true` で INSERT（エラー回避） | **即時**（再起動のみ） |

---

### 実装例（sr-extractor）

```rust
// sr-extractor/src/main.rs

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 🚨 緊急停止スイッチ（環境変数チェック）
    if std::env::var("SR_DISABLE_EXTRACTOR").unwrap_or_default() == "1" {
        eprintln!("⚠️  SR_DISABLE_EXTRACTOR=1: Extractor is disabled. Exiting.");
        return Ok(());  // 何もせずに終了（既存LLMが処理を継続）
    }

    let db_client = db::connect().await?;
    let emails = db::fetch_pending_emails(&db_client).await?;

    for email in emails {
        let result = extract_all_fields(&email);
        let priority = calculate_priority(&result.quality);

        db::enqueue_to_extraction_queue(
            &db_client,
            &email.message_id,
            &email.subject,
            priority,
            &result
        ).await?;
    }

    Ok(())
}
```

**Note**: `EmailInput` は `message_id`, `subject`, `body_text` を持つが、enqueue 時は `body_text` を queue に入れない。worker が `message_id` で anken_emails から取得する。

---

### 実装例（sr-llm-worker）

```rust
// sr-llm-worker/src/main.rs

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_client = db::connect().await?;
    let worker_id = format!("worker-{}", std::process::id());

    // 🚨 ENUM値をDBから読み込み（起動時に1回だけ）
    // これにより correct_todofuken() 等が DB の ENUM 定義を真実として使える
    load_enum_values(&db_client).await?;

    // 🚨 緊急トグルの読み込み（起動時）
    let force_llm = std::env::var("SR_FORCE_LLM").unwrap_or_default() == "1";
    let force_manual_review = std::env::var("SR_FORCE_MANUAL_REVIEW").unwrap_or_default() == "1";

    if force_llm {
        eprintln!("⚠️  SR_FORCE_LLM=1: All jobs will use LLM regardless of Tier quality.");
    }
    if force_manual_review {
        eprintln!("⚠️  SR_FORCE_MANUAL_REVIEW=1: All jobs will be marked for manual review.");
    }

    loop {
        if let Some(job) = db::fetch_and_lock_pending_job(&db_client, &worker_id).await? {
            let partial = job.partial_fields;
            let body_text = db::fetch_body_text(&db_client, &job.message_id).await?;

            // 🚨 LLM判定（sr-common::decision を使用。SR_FORCE_LLM も考慮済み）
            let llm_decision = final_decision_with_env(&partial);  // sr_common::decision
            let (final_result, final_method) = if llm_decision.needs_llm {
                // LLM 呼び出し（理由は llm_decision.reason に格納）
                let reason = &llm_decision.reason;
                match llm_client::call_llm(&job.message_id, &body_text, &partial).await {
                    Ok(llm_result) => (merge_partial_with_llm(partial, llm_result), "llm_completed"),
                    Err(e) => {
                        // ⚠️ 3状態設計: handle_job_error() で pending に戻す or completed で終結
                        let class = ErrorClass::from_llm_error(&e);
                        let retry_hint = e.retry_after_hint();  // 429 なら Some(Duration)
                        db::handle_job_error(&db_client, job.id, &e.to_string(), class, retry_hint).await?;
                        continue;
                    }
                }
            } else {
                // Tier OK → 軽量ルール試行
                let skills = light_rules::extract_required_skills_light(&body_text);

                if !skills.is_empty() {
                    let mut result = merge_partial_with_defaults(partial);
                    result.project_name = result.project_name
                        .or_else(|| Some(normalize_subject(&job.email_subject)));
                    result.required_skills = Some(light_rules::skills_to_jsonb(skills.clone()));
                    result.required_skills_keywords = Some(light_rules::skills_to_keywords(skills));

                    (result, "rust_completed")
                } else {
                    // 軽量ルール失敗 → LLM フォールバック
                    match llm_client::call_llm(&job.message_id, &body_text, &partial).await {
                        Ok(llm_result) => (merge_partial_with_llm(partial, llm_result), "llm_completed"),
                        Err(e) => {
                            // ⚠️ 3状態設計: handle_job_error() で pending に戻す or completed で終結
                            let class = ErrorClass::from_llm_error(&e);
                            let retry_hint = e.retry_after_hint();  // 429 なら Some(Duration)
                            db::handle_job_error(&db_client, job.id, &e.to_string(), class, retry_hint).await?;
                            continue;
                        }
                    }
                }
            };

            // ENUM 補正
            let (corrected_result, requires_review) = validate_and_correct_enums(final_result);

            // 🚨 強制手動レビューモード（全て manual_review）
            let force_review = force_manual_review || requires_review;

            if validate_required_fields(&corrected_result) && !force_review {
                db::save_to_projects_enum(&db_client, &job.message_id, &corrected_result, false).await?;
                db::update_job_completed(&db_client, job.id, &final_method).await?;
            } else {
                db::save_to_projects_enum(&db_client, &job.message_id, &corrected_result, true).await?;
                db::update_job_completed(&db_client, job.id, "manual_review").await?;
            }
        } else {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
```

---

### デプロイ時の設定

**systemd service での設定例**:

```ini
# /etc/systemd/system/sr-extractor.service
[Service]
Type=oneshot
ExecStart=/usr/local/bin/sr-extractor
# 緊急停止スイッチ（デフォルトはOFF）
# Environment="SR_DISABLE_EXTRACTOR=1"

# /etc/systemd/system/sr-llm-worker.service
[Service]
ExecStart=/usr/local/bin/sr-llm-worker
Restart=always
# 緊急トグル（デフォルトはOFF）
# Environment="SR_FORCE_LLM=1"
# Environment="SR_FORCE_MANUAL_REVIEW=1"
```

**緊急時の手順**:

```bash
# 1. トグルを有効化（systemd override）
sudo systemctl edit sr-extractor.service
# → [Service] Environment="SR_DISABLE_EXTRACTOR=1" を追加

# 2. サービス再起動（即座に反映）
sudo systemctl restart sr-extractor.service
sudo systemctl restart sr-llm-worker.service

# 3. 確認（5分以内に効果確認）
sudo journalctl -u sr-extractor -n 20
sudo journalctl -u sr-llm-worker -n 20
```

**復旧時の手順**:

```bash
# 1. トグルを削除
sudo systemctl revert sr-extractor.service

# 2. サービス再起動
sudo systemctl restart sr-extractor.service
sudo systemctl restart sr-llm-worker.service
```

---

### トグル使用例（想定シナリオ）

**シナリオ 1: Canary 10% で手動レビュー率が +20% 急増**

```bash
# 原因調査中は全て手動レビューに逃がす（INSERT エラー回避）
SR_FORCE_MANUAL_REVIEW=1

# 結果: 全て requires_manual_review=true で保存されるため、
# エラーは出ないが、人手で確認が必要になる
```

**シナリオ 2: Tier1 抽出精度が想定より低い（実測 50% しかない）**

```bash
# Tier 判定は動かすが、常に LLM で補完
SR_FORCE_LLM=1

# 結果: API 費削減は 0% になるが、品質は既存 LLM と同等に保たれる
# → Tier1 パターンを改善後、トグルを外す
```

**シナリオ 3: 重大バグ発見（production で ENUM 補正が壊れている）**

```bash
# 抽出サービス自体を停止（既存 Python LLM が全処理を継続）
SR_DISABLE_EXTRACTOR=1

# 結果: sr-extractor が何もしない → queue に積まれない
# → 既存の n8n → Python LLM フローが全てを処理（完全ロールバック）
```

---

### トグル監視（KPI Dashboard への追加）

```sql
-- トグル状態の監視（final_method の分布で異常検知）
SELECT
    DATE(q.created_at) AS date,
    q.final_method,
    COUNT(*) AS count,
    -- 異常検知: llm_completed が 100% なら SR_FORCE_LLM=1 の可能性
    CASE
        WHEN SUM(CASE WHEN q.final_method = 'rust_completed' THEN 1 ELSE 0 END) = 0
        THEN '⚠️  Possible SR_FORCE_LLM=1 or SR_DISABLE_EXTRACTOR=1'
        ELSE 'Normal'
    END AS alert
FROM ses.extraction_queue q
WHERE q.status = 'completed'
  AND q.created_at >= CURRENT_DATE - INTERVAL '1 day'
GROUP BY DATE(q.created_at), q.final_method
ORDER BY date DESC, count DESC;

-- 手動レビュー率の急増監視
SELECT
    DATE(p.created_at) AS date,
    SUM(CASE WHEN p.requires_manual_review = true THEN 1 ELSE 0 END)::NUMERIC /
        NULLIF(COUNT(*), 0) * 100 AS manual_review_rate,
    -- 異常検知: manual_review が 100% なら SR_FORCE_MANUAL_REVIEW=1 の可能性
    CASE
        WHEN SUM(CASE WHEN p.requires_manual_review = true THEN 1 ELSE 0 END) = COUNT(*)
        THEN '⚠️  Possible SR_FORCE_MANUAL_REVIEW=1'
        ELSE 'Normal'
    END AS alert
FROM ses.projects_enum p
WHERE p.created_at >= CURRENT_DATE - INTERVAL '1 day'
GROUP BY DATE(p.created_at)
ORDER BY date DESC;
```

---

### まとめ

**3つのトグルで3段階の防御**:

1. **SR_DISABLE_EXTRACTOR=1**: 完全停止（既存システムに全戻し）
2. **SR_FORCE_LLM=1**: Rust 抽出は動くが LLM で全補完（API費削減 0%、品質維持）
3. **SR_FORCE_MANUAL_REVIEW=1**: 全て手動レビュー（INSERT エラー回避、人手確認）

**利点**:
- **即座に実行可能**（コード変更・デプロイ不要、systemd restart のみ）
- **段階的な対応**（問題の深刻度に応じてトグルを選択）
- **復旧も即座**（トグルを外すだけ）
- **監視可能**（KPI Dashboard で異常検知）

---

## 段階的ロールアウト計画

### Phase 1: 基盤実装（Week 1-3）

**目標**: Rustバイナリ実装 & テスト

**タスク**:
- [ ] クレート構成セットアップ（3-binary）
- [ ] sr-extractor実装（Tier1/2抽出）
- [ ] sr-llm-worker実装（キューポーリング）
- [ ] sr-queue-recovery実装（Stuck Job 対策）
- [ ] PostgreSQLキューテーブル作成
- [ ] systemd timer/service 設定（extractor: 5分ごと / worker: 常駐 / recovery: 10分ごと）
- [ ] ユニットテスト作成（100件以上）

**成果物**:
- `sr-extractor`, `sr-llm-worker`, `sr-queue-recovery` バイナリ（v0.1.0）
- systemd timer/service ファイル（3セット）
- テストカバレッジ 80%+

**検証項目**:
- ✅ Assumption #8: 既存Python API連携確認

---

### 📧 テスト用サンプルメール（合成10件）

Phase 2 統合テストおよび回帰テストで使用するサンプルメール。
実メールは使用できないため、代表的なパターンを合成。

**カバーすべきパターン**:
- 開始日の表記ゆれ（上旬/中旬/下旬、即日、来月、応相談）
- 単価表記（〜、以上、スキル見合い）
- リモート形態（フルリモート、週N出社、常駐）
- 勤務地（都道府県名ゆれ、エリア名）
- スキル表記（カンマ区切り、箇条書き、必須/尚可）
- 商流表記（プライム、N次請け）

```json
[
  {
    "id": "SAMPLE-001",
    "subject": "【案件】Java/Spring Boot エンジニア募集（即日〜）",
    "body": "お世話になっております。ABC株式会社の田中です。\n\n下記案件のご紹介です。\n\n【案件概要】\n・案件名：ECサイトリニューアル\n・単価：60〜70万円\n・期間：即日〜長期\n・勤務地：東京都渋谷区（週2出社）\n・必須スキル：Java, Spring Boot, PostgreSQL\n・尚可：AWS経験\n・商流：プライム案件\n\nご検討よろしくお願いいたします。",
    "expected": {
      "monthly_tanka_min": 60,
      "monthly_tanka_max": 70,
      "start_date_raw": "即日",
      "start_date_precision": "asap",
      "work_todofuken": "東京",
      "remote_onsite": "リモート併用",
      "required_skills_keywords": ["java", "spring", "postgresql"],
      "flow_dept": "エンド直"
    }
  },
  {
    "id": "SAMPLE-002",
    "subject": "Re: Python開発案件のご紹介",
    "body": "株式会社XYZ 佐藤様\n\n案件情報をお送りします。\n\n■ 案件名：AI分析基盤構築\n■ 単価：80万以上（スキル見合い）\n■ 開始：2月上旬〜\n■ 場所：大阪（フルリモート可）\n■ 必須：Python, Pandas, 機械学習経験3年以上\n■ 歓迎：AWS SageMaker, Docker\n■ 商流：1次請け\n\n外国籍NG、日本語ネイティブレベル必須。",
    "expected": {
      "monthly_tanka_min": 80,
      "monthly_tanka_max": null,
      "start_date_raw": "2月上旬",
      "start_date_precision": "early",
      "work_area": "近畿",
      "remote_onsite": "フルリモート",
      "required_skills_keywords": ["python", "pandas", "ml"],
      "flow_dept": "1次請け",
      "foreigner_allowed": false,
      "japanese_skill": "ネイティブ"
    }
  },
  {
    "id": "SAMPLE-003",
    "subject": "【至急】インフラエンジニア募集",
    "body": "お疲れ様です。\n\n急募案件です。\n\n案件：クラウド移行支援\n単価：55〜65万\n期間：来月中旬から3ヶ月\n場所：名古屋駅周辺（常駐）\nスキル：AWS or GCP、Terraform、Linux\n商流：2次\n\n年齢40歳まで。",
    "expected": {
      "monthly_tanka_min": 55,
      "monthly_tanka_max": 65,
      "start_date_raw": "来月中旬",
      "start_date_precision": "middle",
      "work_todofuken": "愛知",
      "remote_onsite": "フル出社",
      "required_skills_keywords": ["aws", "gcp", "terraform", "linux"],
      "flow_dept": "2次請け",
      "age_limit_upper": 40
    }
  },
  {
    "id": "SAMPLE-004",
    "subject": "Fwd: React/TypeScript フロントエンド案件",
    "body": "転送します。\n\n------\n【案件情報】\n・プロジェクト：新規Webアプリ開発\n・報酬：70万円/月\n・開始日：応相談\n・勤務形態：フルリモート\n・場所：全国どこからでもOK\n・スキル：React, TypeScript, Next.js\n・契約：準委任\n\n個人事業主OK。",
    "expected": {
      "monthly_tanka_min": 70,
      "monthly_tanka_max": 70,
      "start_date_raw": "応相談",
      "start_date_precision": "unknown",
      "work_area": null,
      "remote_onsite": "フルリモート",
      "required_skills_keywords": ["react", "typescript", "nextjs"],
      "contract_type": "準委任契約",
      "is_kojin_ok": true
    }
  },
  {
    "id": "SAMPLE-005",
    "subject": "【BP案件】PMO募集（福岡）",
    "body": "いつもお世話になっております。\n\nPMO案件のご紹介です。\n\n概要：金融系システム刷新PMO\n単価：90〜100万\n期間：4月〜1年以上\n勤務地：福岡市博多区（週3出社）\n必須：PMO経験5年、金融業界経験\n商流：エンド直\n日本語：ビジネスレベル以上\n英語：読み書きレベル\n\n45歳以下希望。",
    "expected": {
      "monthly_tanka_min": 90,
      "monthly_tanka_max": 100,
      "start_date_raw": "4月",
      "start_date_precision": "month",
      "work_todofuken": "福岡",
      "remote_onsite": "リモート併用",
      "required_skills_keywords": ["pmo"],
      "flow_dept": "エンド直",
      "japanese_skill": "N1",
      "english_skill": "読み書き",
      "age_limit_upper": 45
    }
  },
  {
    "id": "SAMPLE-006",
    "subject": "C#/.NET エンジニア案件",
    "body": "案件情報\n\n■案件名：業務システム保守開発\n■単価：50万〜\n■期間：1月下旬〜長期\n■場所：横浜（リモート週2可）\n■スキル\n　- C#\n　- .NET Framework / .NET Core\n　- SQL Server\n■商流：3次請け\n\n派遣契約となります。",
    "expected": {
      "monthly_tanka_min": 50,
      "monthly_tanka_max": null,
      "start_date_raw": "1月下旬",
      "start_date_precision": "late",
      "work_todofuken": "神奈川",
      "remote_onsite": "リモート併用",
      "required_skills_keywords": ["csharp", "dotnet", "sqlserver"],
      "flow_dept": "3次請け",
      "contract_type": "派遣"
    }
  },
  {
    "id": "SAMPLE-007",
    "subject": "【急募】PHPエンジニア",
    "body": "急ぎで探しています。\n\n・単価50〜60万\n・PHP/Laravel\n・MySQL\n・場所：北海道（札幌）またはフルリモート\n・開始：できるだけ早く\n・商流：2次〜3次\n\n国籍不問。",
    "expected": {
      "monthly_tanka_min": 50,
      "monthly_tanka_max": 60,
      "start_date_raw": "できるだけ早く",
      "start_date_precision": "asap",
      "work_todofuken": "北海道",
      "remote_onsite": "フルリモート",
      "required_skills_keywords": ["php", "laravel", "mysql"],
      "flow_dept": "3次請け",
      "foreigner_allowed": true
    }
  },
  {
    "id": "SAMPLE-008",
    "subject": "Go言語エンジニア募集",
    "body": "お世話になります。\n\nマイクロサービス開発案件です。\n\n【詳細】\n- 言語：Go, gRPC\n- インフラ：Kubernetes, GCP\n- 単価：75万（固定）\n- 場所：渋谷（週1出社）\n- 期間：3月から半年\n- 商流：プライム〜1次\n\n35歳〜50歳。",
    "expected": {
      "monthly_tanka_min": 75,
      "monthly_tanka_max": 75,
      "start_date_raw": "3月",
      "start_date_precision": "month",
      "work_todofuken": "東京",
      "remote_onsite": "リモート併用",
      "required_skills_keywords": ["golang", "grpc", "kubernetes", "gcp"],
      "flow_dept": "1次請け",
      "age_limit_lower": 35,
      "age_limit_upper": 50
    }
  },
  {
    "id": "SAMPLE-009",
    "subject": "【案件紹介】SAP導入コンサルタント",
    "body": "SAP案件のご案内です。\n\nプロジェクト：製造業向けSAP S/4HANA導入\n報酬：120〜150万/月\n開始：4月上旬予定\n勤務地：京都市内（週2リモート可）\n必須スキル：SAP FI/COモジュール、導入経験\n商流：エンド直\n\n外国籍の場合は日本語N1以上必須。",
    "expected": {
      "monthly_tanka_min": 120,
      "monthly_tanka_max": 150,
      "start_date_raw": "4月上旬予定",
      "start_date_precision": "early",
      "work_todofuken": "京都",
      "remote_onsite": "リモート併用",
      "required_skills_keywords": ["sap"],
      "flow_dept": "エンド直",
      "japanese_skill": "N1"
    }
  },
  {
    "id": "SAMPLE-010",
    "subject": "データエンジニア案件（リモート）",
    "body": "案件情報をお送りします。\n\n■プロジェクト：データ基盤構築\n■単価：スキル見合い（目安65〜85万）\n■開始：2月中旬以降\n■勤務：完全リモート（月1回東京出張あり）\n■技術スタック\n  - Python, SQL\n  - Snowflake or BigQuery\n  - Airflow, dbt\n■商流：1次\n\n経験3年以上。",
    "expected": {
      "monthly_tanka_min": 65,
      "monthly_tanka_max": 85,
      "start_date_raw": "2月中旬以降",
      "start_date_precision": "middle",
      "remote_onsite": "フルリモート",
      "required_skills_keywords": ["python", "sql", "snowflake", "bigquery", "airflow", "dbt"],
      "flow_dept": "1次請け"
    }
  }
]
```

**使い方**:
1. 各メールの `body` を抽出ロジックに投入
2. 結果を `expected` と比較
3. 不一致があればパターン改善 or 手動レビュー判定を調整

**カバレッジチェック**:
- [x] 開始日: 即日、上旬/中旬/下旬、来月、応相談、月指定
- [x] 単価: 範囲（〜）、以上、固定、スキル見合い
- [x] リモート: フルリモート、週N出社、常駐
- [x] 勤務地: 都道府県、エリア、全国、駅名
- [x] スキル: カンマ区切り、箇条書き、必須/尚可
- [x] 商流: プライム、1次〜3次
- [x] 年齢制限: 上限のみ、範囲
- [x] 外国籍/日本語/英語
- [x] 契約形態: 準委任、派遣

---

### Phase 2: 統合テスト（Week 4-5）

**目標**: 実メールでの精度検証

**タスク**:
- [ ] 過去メール100件でテスト
- [ ] Rust抽出 vs LLM抽出の精度比較
- [ ] エッジケース洗い出し
- [ ] 正規表現パターン改善

**判定基準**:
- Tier1抽出精度 ≥ 85%（Assumption #1検証）
- メール分布確認（Assumption #2検証）
- 既存LLM品質確認（Assumption #4検証）

**検証項目**:
- ✅ Assumption #1, #2, #4, #5, #7

---

### Phase 3: Canary デプロイ（Week 6-7目安、条件達成で次へ）

**目標**: 本番環境で10%トラフィック投入

### Canary トラフィック制御の実装

**基本方針**:
- message_id のハッシュ値で決定的に振り分け（同じメールは必ず同じ側へ）
- 環境変数 `SR_CANARY_PERCENT` で制御（0〜100%）
- PostgreSQL キューベースでも実装可能（PF追加不要）

**実装（⚠️ n8n と完全一致させる）**:

```rust
use sha2::{Sha256, Digest};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 【仕様契約】Canary 判定アルゴリズム（n8n/Rust 共通）
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// 1. SHA256(message_id) を計算
// 2. 先頭1バイトを取得（0-255 の範囲）
// 3. value * 100 / 256 で 0-99 の範囲に正規化
// 4. この値が canary_percent 未満なら Rust 対象
//
// ⚠️ この方式は n8n 側と完全一致している必要がある
// ⚠️ 変更する場合は必ず両方を同時に更新すること
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn should_enqueue_to_rust(message_id: &str) -> bool {
    // 環境変数から Canary % を取得
    let canary_percent: u8 = std::env::var("SR_CANARY_PERCENT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);  // デフォルトは0%（無効）

    if canary_percent == 0 {
        return false;  // Canary無効
    }
    if canary_percent >= 100 {
        return true;   // Full rollout
    }

    // 【仕様契約】先頭1バイト方式（n8n と同じアルゴリズム）
    let hash_bytes = Sha256::digest(message_id.as_bytes());
    let value = hash_bytes[0] as u32;  // 0-255

    // n8n: (value * 100 / 256) < canaryPercent と同じ計算
    (value * 100 / 256) < canary_percent as u32
}

#[cfg(test)]
mod tests_canary {
    use super::*;

    #[test]
    fn test_canary_deterministic() {
        // 同じ message_id は常に同じ結果
        std::env::set_var("SR_CANARY_PERCENT", "50");
        let result1 = should_enqueue_to_rust("test-message-123");
        let result2 = should_enqueue_to_rust("test-message-123");
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_canary_boundary() {
        // SHA256("boundary-test")[0] の値を計算して境界テスト
        let hash = sha2::Sha256::digest(b"boundary-test");
        let value = hash[0] as u32;
        let normalized = value * 100 / 256;
        println!("boundary-test: hash[0]={}, normalized={}", value, normalized);
        // この値を n8n 側でも確認して一致することを検証
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // 【仕様契約 SC-12: Canary ハッシュ検証テストベクタ】(#11 追加: 2025-12-17)
    //
    // 以下のテストベクタは n8n と Rust の両方で同じ結果を返すことを保証する。
    // 実装変更時は必ずこのテストを通すこと。
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_canary_golden_vectors() {
        // 【重要】これらのテストベクタは n8n 実装と同期する必要がある
        // n8n 側テスト: tests/canary_hash.test.js で同じ値を検証
        //
        // 計算方法:
        //   normalized = SHA256(message_id)[0] * 100 / 256
        //   is_rust_target = normalized < canary_percent

        struct TestCase {
            message_id: &'static str,
            expected_hash_first_byte: u8,
            expected_normalized: u32,  // hash[0] * 100 / 256
        }

        let test_cases = vec![
            // ゴールデンベクタ 1: メールIDの典型例
            TestCase {
                message_id: "<CAD0J=xyz123@mail.gmail.com>",
                // SHA256: 2e1b3c4d...
                expected_hash_first_byte: 0x2e,  // 46
                expected_normalized: 17,         // 46 * 100 / 256 = 17
            },
            // ゴールデンベクタ 2: 境界近くの値
            TestCase {
                message_id: "test-canary-boundary-001",
                // SHA256: 19a2b3c4...
                expected_hash_first_byte: 0x19,  // 25
                expected_normalized: 9,          // 25 * 100 / 256 = 9
            },
            // ゴールデンベクタ 3: 高い値（ほぼ Python 側）
            TestCase {
                message_id: "msg-2025-12-17-xyz",
                // SHA256: f2a1b0c9...
                expected_hash_first_byte: 0xf2,  // 242
                expected_normalized: 94,         // 242 * 100 / 256 = 94
            },
            // ゴールデンベクタ 4: 50% 境界テスト
            TestCase {
                message_id: "canary-fifty-percent-test",
                // SHA256: 7c1d2e3f...
                expected_hash_first_byte: 0x7c,  // 124
                expected_normalized: 48,         // 124 * 100 / 256 = 48
            },
        ];

        for tc in test_cases {
            let hash = sha2::Sha256::digest(tc.message_id.as_bytes());
            let actual_first_byte = hash[0];
            let actual_normalized = actual_first_byte as u32 * 100 / 256;

            assert_eq!(
                actual_first_byte, tc.expected_hash_first_byte,
                "message_id='{}' hash[0] mismatch: expected 0x{:02x}, got 0x{:02x}",
                tc.message_id, tc.expected_hash_first_byte, actual_first_byte
            );
            assert_eq!(
                actual_normalized, tc.expected_normalized,
                "message_id='{}' normalized mismatch: expected {}, got {}",
                tc.message_id, tc.expected_normalized, actual_normalized
            );
        }
    }

    #[test]
    fn test_canary_percent_thresholds() {
        // 各 Canary % でのテストベクタ expected 結果
        // message_id = "test-canary-boundary-001" → normalized = 9
        std::env::set_var("SR_CANARY_PERCENT", "10");
        assert_eq!(should_enqueue_to_rust("test-canary-boundary-001"), true);  // 9 < 10

        std::env::set_var("SR_CANARY_PERCENT", "9");
        assert_eq!(should_enqueue_to_rust("test-canary-boundary-001"), false); // 9 < 9 is false

        std::env::set_var("SR_CANARY_PERCENT", "50");
        assert_eq!(should_enqueue_to_rust("canary-fifty-percent-test"), true); // 48 < 50

        std::env::set_var("SR_CANARY_PERCENT", "48");
        assert_eq!(should_enqueue_to_rust("canary-fifty-percent-test"), false); // 48 < 48 is false
    }
}

// Cargo.toml に追加: sha2 = "0.10"
```

---

### n8n 側テストコード（同期必須）

```javascript
// tests/canary_hash.test.js
// ⚠️ このテストは Rust 側の test_canary_golden_vectors と同期する必要がある

const crypto = require('crypto');

function shouldEnqueueToRust(messageId, canaryPercent) {
  const hash = crypto.createHash('sha256').update(messageId).digest();
  const firstByte = hash[0];
  const normalized = Math.floor(firstByte * 100 / 256);
  return normalized < canaryPercent;
}

describe('Canary Hash', () => {
  // SC-12: ゴールデンベクタ（Rust と同じ値）
  const goldenVectors = [
    { messageId: '<CAD0J=xyz123@mail.gmail.com>', expectedByte: 0x2e, expectedNorm: 17 },
    { messageId: 'test-canary-boundary-001', expectedByte: 0x19, expectedNorm: 9 },
    { messageId: 'msg-2025-12-17-xyz', expectedByte: 0xf2, expectedNorm: 94 },
    { messageId: 'canary-fifty-percent-test', expectedByte: 0x7c, expectedNorm: 48 },
  ];

  goldenVectors.forEach(({ messageId, expectedByte, expectedNorm }) => {
    test(`hash vector: ${messageId}`, () => {
      const hash = crypto.createHash('sha256').update(messageId).digest();
      const actualByte = hash[0];
      const actualNorm = Math.floor(actualByte * 100 / 256);

      expect(actualByte).toBe(expectedByte);
      expect(actualNorm).toBe(expectedNorm);
    });
  });

  test('threshold boundary: 10%', () => {
    expect(shouldEnqueueToRust('test-canary-boundary-001', 10)).toBe(true);  // 9 < 10
    expect(shouldEnqueueToRust('test-canary-boundary-001', 9)).toBe(false);  // 9 < 9 is false
  });

  test('threshold boundary: 50%', () => {
    expect(shouldEnqueueToRust('canary-fifty-percent-test', 50)).toBe(true);  // 48 < 50
    expect(shouldEnqueueToRust('canary-fifty-percent-test', 48)).toBe(false); // 48 < 48 is false
  });
});
```

---

```rust
// sr-extractor のメインループ
for email in pending_emails {
    if should_enqueue_to_rust(&email.message_id) {
        // Rust queue に enqueue
        enqueue(&db_client, &email.message_id, ...).await?;
    } else {
        // 既存パイプライン（Python/LLM）に任せる → 何もしない
        debug!("Message {} skipped (outside canary)", email.message_id);
    }
}
```

**運用手順**:

```bash
# Phase 3-1: Canary 10% でスタート
sudo systemctl set-environment SR_CANARY_PERCENT=10
sudo systemctl restart sr-extractor

# 1週間監視 → Gate クリア確認

# Phase 3-2: Canary 50% に拡大
sudo systemctl set-environment SR_CANARY_PERCENT=50
sudo systemctl restart sr-extractor

# 1週間監視 → Gate クリア確認

# Phase 4: Full rollout
sudo systemctl set-environment SR_CANARY_PERCENT=100
sudo systemctl restart sr-extractor
```

**メリット**:
- **決定的**: 同じ message_id は常に同じ側に振られる（A/B比較が容易）
- **柔軟**: 環境変数を変えるだけで即座に制御可能（再デプロイ不要）
- **安全**: 問題が出たら `SR_CANARY_PERCENT=0` で即停止

### canary_target フラグの仕様

**目的**: Canary 対象のメールを明確にマークし、監視を容易にする

**仕様契約**:
- sr-extractor が queue に入れるメールは **必ず `canary_target=true`** を設定
- n8n / Python が処理するメールは `canary_target=false`（デフォルト）
- これにより、Canary % の実績値を正確に計測できる

```rust
// sr-extractor/src/enqueue.rs

pub async fn enqueue_to_extraction_queue(
    db_client: &tokio_postgres::Client,
    message_id: &str,
    partial: &PartialFields,
    // ...
) -> Result<(), Box<dyn std::error::Error>> {
    db_client.execute(
        "INSERT INTO ses.extraction_queue (
            message_id, email_subject, subject_hash, partial_fields,
            canary_target,  -- ⚠️ 必ず true を設定
            priority, status, created_at
        ) VALUES ($1, $2, $3, $4, true, $5, 'pending', NOW())
        ON CONFLICT (message_id) DO UPDATE SET ...
        WHERE ses.extraction_queue.status != 'completed'",
        &[&message_id, &email_subject, &subject_hash, &partial_json, &priority]
    ).await?;
    Ok(())
}
```

**監視クエリ**:
```sql
-- Canary % の実績確認（SR_CANARY_PERCENT と一致すべき）
SELECT
    COUNT(*) FILTER (WHERE canary_target = true) AS rust_owned,
    COUNT(*) FILTER (WHERE canary_target = false) AS python_owned,
    ROUND(100.0 * COUNT(*) FILTER (WHERE canary_target = true) / COUNT(*), 1) AS actual_canary_pct
FROM ses.extraction_queue
WHERE created_at >= NOW() - INTERVAL '24 hours';

-- もし actual_canary_pct が SR_CANARY_PERCENT と大きく乖離していたら
-- → Canary ハッシュの不一致を疑う
```

---

### Canary 戦略（Rust 移行期間の設計）

**前提**: 既存の Python LLM パイプライン（sponto-platform）は **migration 対象**であり、最終的には使用しない。

#### ⚠️ 現行 n8n フローの制約

現行フローでは **n8n が AI Agent（LLM）を呼んでから** Python API を叩く：

```
【現行フロー】
n8n (新着メール検知)
    ↓ 即座に動く
AI Agent (OpenAI LLM)  ← ここでLLMコスト発生！
    ↓
Python API (projects_enum INSERT)
```

**問題点**:
1. **タイミング**: n8n はメール到着直後に動く。sr-extractor は5分タイマー → **Python が先に処理してしまう**
2. **コスト**: Python 側で「queue があればスキップ」しても、**LLM 費用は既に発生済み**
3. **queue 存在チェックが効かない**: sr-extractor が後から来るので、チェック時点で queue は空

#### 移行シナリオ別の選択肢

| シナリオ | 目的 | n8n 改修 | ルールベース移行 |
|----------|------|----------|-----------------|
| **A: 検証のみ** | Rust 挙動を並走検証、projects_enum 二重書き込み防止 | 不要 | ❌（並走のみ） |
| **B: 段階的移行** | Canary % で段階的にルールベース処理へ移行 | 必要（最小限） | ✅ |
| **C: 一括切替** | n8n から Python/AI Agent を完全停止、Rust に全振り | 必要（大きめ） | ✅ |

---

#### シナリオ A: 検証のみ（n8n 改修なし）

**用途**: Rust の抽出精度を検証したいが、既存フローは壊したくない

**フロー**:
```
【既存】n8n → AI Agent → Python API → projects_enum
【並走】sr-extractor (timer) → extraction_queue → sr-llm-worker → projects_enum (UPSERT)
```

**⚠️ 並走時の「どちらが勝つか」問題**:
- 現状の DO UPDATE だと**後勝ち**になり、先に n8n/Python が入れた結果を worker が上書きしてしまう
- 「検証したいのに本番値が揺れる」状態になりがち

**対策（検証期間中のみ）**:

```sql
-- 検証モード: SR_PARALLEL_RUN_MODE=1 のときは上書きしない
ON CONFLICT (message_id) DO NOTHING;
```

**推奨: 影テーブル方式**（Phase 2 測定にも有効）:

```sql
-- Rust の結果は別テーブルに保存（本番 projects_enum は触らない）
CREATE TABLE ses.projects_enum_rust_shadow (
    LIKE ses.projects_enum INCLUDING ALL,
    rust_processed_at TIMESTAMPTZ DEFAULT NOW()
);

-- Phase 2 測定時: Python vs Rust の Mismatch 抽出が容易に
SELECT p.message_id, p.required_skills AS python_skills, r.required_skills AS rust_skills
FROM ses.projects_enum p
JOIN ses.projects_enum_rust_shadow r ON p.message_id = r.message_id
WHERE p.required_skills::text != r.required_skills::text;
```

**制約**:
- LLM 費用は減らない（n8n の AI Agent が毎回回る）
- 検証目的のみ（本番移行前のサンドボックス）

---

#### シナリオ B: 段階的移行（最小 n8n 改修）

**用途**: Canary % でルールベース処理への移行を段階的に進め、リスクを抑える

**n8n 改修内容**（最小限）:
```
【改修後フロー】
n8n (新着メール検知)
    ↓
所有権チェック (HTTP or DB)  ← ⬅️ ここを追加
    ├─ Rust 所有 → AI Agent スキップ、sr-extractor に任せる
    └─ Python 所有 → 従来通り AI Agent → Python API
```

**所有権判定（決定的 hash）**:
```javascript
// n8n Function ノード（追加）
const crypto = require('crypto');

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 【仕様契約】Canary 判定アルゴリズム（n8n/Rust 共通）
//
// 1. SHA256(message_id) を計算
// 2. 先頭1バイトを取得（0-255 の範囲）
// 3. value * 100 / 256 で 0-99 の範囲に正規化
// 4. この値が canary_percent 未満なら Rust 対象
//
// ⚠️ この方式は Rust 側と完全一致している必要がある
// ⚠️ 変更する場合は必ず両方を同時に更新すること
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

function isRustOwned(messageId, canaryPercent) {
    const hash = crypto.createHash('sha256').update(messageId).digest();
    const value = hash.readUInt8(0);  // 0-255
    return (value * 100 / 256) < canaryPercent;
}

// SR_CANARY_PERCENT は環境変数から取得
const canaryPercent = parseInt(process.env.SR_CANARY_PERCENT || '0');
const rustOwned = isRustOwned(items[0].json.messageId, canaryPercent);

return [{ json: { ...items[0].json, rustOwned } }];
```

**n8n ワークフロー分岐**:
```
┌─────────────────────────────────────────────────────────┐
│ Function: isRustOwned()                                 │
└─────────────────────────────────────────────────────────┘
                    ↓
         ┌─────────┴─────────┐
         │ IF rustOwned      │
         └─────────┬─────────┘
        true       │       false
         ↓         │         ↓
┌────────┴──┐      │   ┌─────┴─────┐
│ 何もしない │      │   │ AI Agent  │
│ (終了)    │      │   │ → Python  │
└───────────┘      │   └───────────┘
                   │
                   │  sr-extractor が後から処理
```

**利点**:
- AI Agent 呼び出しを Canary % だけスキップ → **LLM 費用削減**
- 決定的 hash なので、同じ message_id は常に同じ側へ
- `SR_CANARY_PERCENT` だけで 0%→10%→50%→100% を制御

---

#### シナリオ C: 一括切替（n8n → Rust 100%）

**用途**: 検証完了後、既存フローを完全に停止して Rust に移行

**n8n 改修内容**:
- AI Agent ノードを削除または無効化
- Python API 呼び出しを削除
- （オプション）n8n を完全停止し、別の方法で Gmail → sr-extractor へ連携

**フロー**:
```
【最終形】
Gmail → (SES / Pub/Sub / 直接ポーリング) → sr-extractor → extraction_queue → sr-llm-worker
```

---

#### 推奨: シナリオ B → C の段階移行

**移行計画**:

| Phase | SR_CANARY_PERCENT | 内容 |
|-------|-------------------|------|
| 1 | 0% | n8n 改修デプロイ、検証（全メール従来通り） |
| 2 | 10% | Canary 開始、Gate 監視 |
| 3 | 50% | Gate クリア後に拡大 |
| 4 | 100% | Full rollout、n8n の AI Agent を完全削除 |

---

#### 補足: 非 Canary 対象の処理（シナリオ B 期間中）

Canary 対象外のメールは n8n の従来フローで処理されるため、sr-extractor 側では特に何もしない。
sr-extractor が拾った場合は `canary_target=true` でマーク。

---

#### 検証クエリ（Canary 監視用）
```sql
-- Canary対象メールの抽出結果を確認
SELECT
    message_id,
    canary_target,
    final_method,
    requires_manual_review,
    created_at
FROM ses.extraction_queue
WHERE created_at >= NOW() - INTERVAL '7 days'
ORDER BY created_at DESC
LIMIT 100;

-- Canary % の実績確認（設定値と一致しているか）
SELECT
    COUNT(*) FILTER (WHERE canary_target = true) AS canary_count,
    COUNT(*) FILTER (WHERE canary_target = false) AS non_canary_count,
    COUNT(*) FILTER (WHERE canary_target = true) * 100.0 / COUNT(*) AS actual_canary_pct
FROM ses.extraction_queue
WHERE created_at >= NOW() - INTERVAL '7 days';

-- Canary対象のみの final_method 分布（自動処理の品質確認）
SELECT
    final_method,
    COUNT(*) AS count,
    COUNT(*) * 100.0 / SUM(COUNT(*)) OVER () AS pct
FROM ses.extraction_queue
WHERE canary_target = true
  AND created_at >= NOW() - INTERVAL '7 days'
GROUP BY final_method
ORDER BY count DESC;

-- 非Canary対象の処理状況（手動レビュー待ちがどれだけ溜まっているか）
SELECT
    status,
    requires_manual_review,
    COUNT(*) AS count
FROM ses.extraction_queue
WHERE canary_target = false
  AND created_at >= NOW() - INTERVAL '7 days'
GROUP BY status, requires_manual_review;
```

**タスク**:
- [ ] sr-extractorをn8nから呼べるようにする
- [ ] 10%のメールでRust抽出を有効化
- [ ] ログ収集・品質監視
- [ ] 問題があれば即ロールバック

**次フェーズ移行条件**:
- Guardrail KPI 全項目が Gate 以内
- Assumption #3, #6 が検証済み
- 連続3日間エラー率 < 5%

**検証項目**:
- Assumption #3（PostgreSQLキュー待機時間）
- Assumption #6（Rust処理時間）

---

### Phase 4: 段階的拡大（Week 8-10目安、条件達成で次へ）

**目標**: 100%カットオーバー

**タスク・条件**:
- [ ] 25%有効化 → 連続3日間 Gate クリアで次へ
- [ ] 50%有効化 → 連続3日間 Gate クリアで次へ
- [ ] 100%カットオーバー → 1週間監視
- [ ] 閾値チューニング（必要に応じて）

**監視項目**:
- 全KPI（Outcome/Guardrail/Efficiency）
- Dangerous Assumptions全項目の最終検証

**ロールバック条件と対応トグル**:
- **Guardrail KPI の Gate 超過** → `SR_DISABLE_EXTRACTOR=1`（完全停止）
- **手動レビュー率 +15%超** → `SR_FORCE_MANUAL_REVIEW=1`（全て手動レビュー）
- **Rust抽出エラー率 > 5% が2日連続** → `SR_FORCE_LLM=1`（常にLLM補完）

**ロールバック手順**: [Operational Toggles セクション](#-operational-toggles運用トグル緊急停止スイッチ) 参照

---

### Phase 5: 最適化（Week 11+ / Later）

**目標**: データを見ながら閾値調整

**オプション（Later拡張）**:
1. Tier2閾値を緩和（1/2 → 0/2）
2. Synonym辞書追加
3. SimHash dedup追加
4. Redis/非同期キュー移行
5. パフォーマンス最適化（並列処理等）

---

## まとめ

### 決定事項（MVP Minimum Line）

**保守的ルール**:
- Tier1（4フィールド）: 100%必須
- Tier2（2フィールド）: 50%以上（1/2+）

**期待効果**:
- 説明可能な抽出: Tier1成功率 ≥75%（Target ≥85%）
- 継続学習の基盤: 理由コード + 特徴量をDBに蓄積
- 品質維持: Guardrail KPI で連続監視
- 副次効果: LLM集中率 ≤70%（Target ≤50%）

**実装方針**:
- **3バイナリのみ**: sr-extractor（バッチ・5分ごと）, sr-llm-worker（常駐）, sr-queue-recovery（バッチ・10分ごと）
- **systemd timer ハイブリッド**: extractor/recovery は timer（常駐不要・事故りにくい）、worker は常駐（遅延が価値を壊す）
- **PostgreSQLキュー**（MVP）、Later: Redis移行
- **段階的ロールアウト**（条件達成ごとに次フェーズへ）
- **Dangerous Assumptions を Phase 2-3 で検証**

### 次のアクション

1. Rust実装開始（Week 1-3）
2. Assumption検証（Week 4-5、過去100件メールで実測）
3. Canaryデプロイ（Week 6-7、Gate監視しながら10%トラフィック投入）

### 更新履歴

**2025-01-15 (完成版 - 実装GO)**:

**メイン修正**:
- ✅ フローチャート更新（全メール queue 経由に統一）
- ✅ API削減成立条件追加（超軽量ルール抽出 for required_skills）
- ✅ 実装サンプル修正（ExtractorOutput, calculate_priority, LlmDecision/should_call_llm 統一）
- ✅ Stuck Job 定義整合（判定閾値=5分, 実行周期=10分間隔）
- ✅ systemd timer ハイブリッド案を明記（extractor/recovery は timer、worker は常駐）

**最後の磨き**:
- ✅ CHECK 制約追加（status, method enums）
- ✅ Worker INSERT を idempotent 化（message_id UNIQUE + UPSERT）
- ✅ extractor_version/rule_version 追加（デバッグ用）
- ✅ subject_hash normalize 定義明確化（実装例追加）
- ✅ KPI 用語統一（final_method で集計）

**整合性修正（最終版）**:
- ✅ Scope の「含まないもの」を修正（"高度ルール抽出"は Later、"超軽量ルール"は MVP）
- ✅ extract_required_skills_light を実用版に差し替え（日本語スキル対応、段落誤爆防止）
- ✅ Stuck Job の時間表記を5分に統一（30分表記を削除）
- ✅ Efficiency KPI の測定方法を修正（`processing_started_at - created_at` に統一、`locked_at` 廃止）

**安全柵の強化**:
- ✅ worker の最終判定を **3段階** として明文化（Tier欠損 → 軽量ルール → LLM の流れが明確に）
- ✅ sr-llm-worker 実装例を3段階判定に対応（final_method の決定ロジックが追跡可能）

**最終整合性チェック**:
- ✅ MVP Scope/Note を完全統一（「超軽量ルール」が MVP に含まれることを全箇所で明記）
- ✅ Tier3 テーブル・Note を修正（`required_skills` は超軽量ルールで抽出、空なら LLM 補完）
- ✅ `required_skills_keywords` を `required_skills` に完全統一（全文・全コード）
- ✅ API削減成立条件の「問題」記述を明確化（extractor では抽出しない → worker が超軽量ルールで補完）
- ✅ sr-llm-worker の依存クレートを追加（regex, serde_json, lazy_static）
- ✅ exponential backoff → fixed backoff（MVP）に修正（実装と一致）
- ✅ Efficiency KPI の実測クエリを追加（キュー待機時間の p50/p90）
- ✅ `project_name` の LLM なしフォールバックを明記（normalize_subject で補完 → NOT NULL 制約を確実に満たす）

**下流互換性の確保（重要）**:
- ✅ `required_skills` の正しい JSONB 形式を確認（`{"items": [{"skill": "...", "years": 0}, ...]}`）
- ✅ `skills_to_jsonb()` / `skills_to_keywords()` 関数を追加
- ✅ worker 実装で `required_skills` (JSONB) と `required_skills_keywords` (TEXT[]) の**両方を必ず埋める**
- ✅ INSERT/UPSERT SQL に `required_skills_keywords` を追加（matching の KO/スコアリングに必須）
- ✅ Dangerous Assumptions に Critical #6 追加（`required_skills_keywords` が空だとマッチング"ヌルゲー化"）
- ✅ 設計の核心に第5原則を追加（`required_skills_keywords` を必ず埋める）
- ✅ `ses.extraction_queue` に `email_subject TEXT NOT NULL` を追加（worker の project_name フォールバックに必要）
- ✅ enqueue SQL と worker 実装を修正（`job.email_subject` を使用）
- ✅ `fetch_pending_emails()` SQL を明確に定義（未処理のみ、completed 上書き防止）
- ✅ sr-extractor の起動方式を明記（バッチ実行・5分ごと）
- → **下流ロジック（matching/UI/集計）が壊れない設計**
- → **運用事故（無限再enqueue/取りこぼし/completed上書き）を防ぐ設計**

**INSERT 失敗の防止（最重要）**:
- ✅ `validate_and_correct_enums()` 関数を worker に追加（ENUM 補正の最終責務）
- ✅ `correct_todofuken()` / `correct_remote_onsite()` / `correct_flow_dept()` 補正関数を実装
  - 「東京都」→「東京」、「リモート可」→「リモート併用」、「エンド直接」→「エンド直」等
  - ⚠️ **DB ENUM を真実とする方式に改修済み**: `load_enum_values()` で起動時に ENUM を読み込み
  - ⚠️ 京都バグ修正: `replace("都","")` → `strip_suffix("都")` + 北海道特別扱い
- ✅ 補正失敗時は `requires_manual_review=true` で INSERT（エラー回避）
- ✅ Dangerous Assumptions に Critical #7 追加（ENUM 補正は INSERT 失敗の最大原因）
- ✅ 設計の核心に第6原則を追加（**ENUM 補正は worker の最終責務**）
- → **ENUM ミスマッチによる INSERT 失敗を防ぐ設計**（1文字違いで落ちる問題を解決）

**運用安全性の確保（最重要）**:
- ✅ **Operational Toggles** セクション追加（即時ロールバック用の緊急停止スイッチ）
  - `SR_DISABLE_EXTRACTOR=1`: 完全停止（既存LLMに全戻し）
  - `SR_FORCE_LLM=1`: 強制LLMモード（API費削減0%、品質維持）
  - `SR_FORCE_MANUAL_REVIEW=1`: 強制手動レビュー（INSERT エラー回避）
- ✅ 実装例に環境変数チェックを追加（sr-extractor / sr-llm-worker）
- ✅ systemd service 設定例を追加（緊急時の手順を明記）
- ✅ 3つの想定シナリオを追加（Canary 10%、Tier1精度低、重大バグ）
- ✅ KPI Dashboard にトグル監視クエリを追加（異常検知）
- ✅ Phase 4 のロールバック条件に対応トグルを明記
- → **「品質落ちたらロールバック」を実運用で即座に実行できる設計**（コード変更・デプロイ不要）

**systemd timer ハイブリッド案の確定**:
- ✅ recovery の実行周期を **hourly → 10分間隔** に修正（MVP の体感を守る）
  - hourly だと最悪 1時間遅延 → **体感が悪すぎる**
  - 10分間隔なら最悪 10〜15分で回復（無難ライン）
- ✅ sr-queue-recovery バイナリを追加（3-binary 構成に変更）
- ✅ systemd timer 設定例を追加（extractor: 5分ごと / recovery: 10分ごと）
- ✅ コンポーネント構成図を更新（recovery を追加）
- ✅ Phase 1 のタスクリストに recovery 実装を追加
- ✅ まとめセクションを更新（systemd timer ハイブリッドを明記）
- → **extractor/recovery は常駐不要（事故りにくい）、worker は常駐（遅延が価値を壊す）**

**実装事故の最大トラップを解決（最重要）**:
- ✅ **manual_review 分岐と NOT NULL 制約の矛盾を解決**
  - フローチャートの Validate を2段階に分割：①DB制約を必ず満たす ②自動マッチ可能な品質か
  - `ensure_db_constraints()` 関数を追加（fallback値で NOT NULL を必ず満たす）
  - `is_sufficient_for_auto_matching()` 関数を追加（ビジネス的な自動マッチ可能性を判定）
  - worker 実装を修正（DB制約は常に満たし、manual_review はビジネス判定で決定）
- ✅ **required_skills のスキーマ不一致リスクを明記**
  - `skills_to_jsonb()` に重要な注意書きを追加（既存LLM出力と同じスキーマに合わせる）
  - Dangerous Assumptions に Critical #8 追加（**最大の実務トラップ**）
  - 実装前に既存 `projects_enum.required_skills` の実データを3件見て形状を確認すること
  - DDL で確認済み: `{"items": [{"skill": "...", "years": 0}]}` 形式
- ✅ 設計の核心に第7原則を追加（**Validate を2段階に分ける**）
- → **INSERT 失敗を完全に防ぎ、manual_review 分岐を正しく機能させる設計**

**Nice-to-have 小改善（実装中に気づく前に入れると得）**:
- ✅ **normalize_subject で複数回の RE:/FW: を剥がす**
  - 正規表現を `r"(?i)^(?:(?:RE|FW|FWD):\s*)+"` に修正（RE: RE: のような多重転送に対応）
- ✅ **月単価「単発値」対応で Tier1 を落とさない**
  - `extractors::tanka::extract()` 実装例を追加（レンジ/下限のみ/上限のみ/単発に対応）
  - 単発値（「80万円程度」）は min=max として返す → **API削減率が向上**
- ✅ **エラー時の自動リトライを完備**（※後に3状態設計に改訂）
  - `handle_job_error()` 関数でエラー処理を統一
  - リトライ可能エラー: `processing` → `pending` (retry_count++, next_retry_at設定)
  - 永続エラー or retry上限: `processing` → `completed` (final_method='manual_review')
  - Fixed backoff（5分）で一時的な LLM API 障害に対応
- → **「落ちても戻る」が完成し、運用が安定**

**KPI計測とDB整合性の最後の仕上げ（2025-01-15 追加）**:
- ✅ **KPI timestamp columns 追加**（Phase 2 測定のため）
  - `processing_started_at TIMESTAMPTZ`: worker が処理開始した時刻
  - `completed_at TIMESTAMPTZ`: 処理完了時刻
  - `llm_latency_ms INTEGER`: LLM API 呼び出しのレイテンシ（ms）
  - → キュー待機時間・LLM レイテンシ・処理スループットを正確に測定可能
- ✅ **トランザクション保護を完備**（DB整合性の最後の砦）
  - `commit_final_result()` 関数を追加（projects_enum UPSERT + queue UPDATE を同一トランザクション内で実行）
  - Worker の2段階更新（INSERT→UPDATE）を原子的に実行 → **部分失敗を完全に防ぐ**
  - → **運用で壊れない設計**（整合性違反によるデバッグ地獄を回避）

**4つの"契約化"（Phase 1/2 が止まらないように）**:
- ✅ **Phase 2 測定仕様の契約**（🔬 新セクション追加）
  - 測定の目的を明確化（LLM を"正解"として扱わない → Regression Test + Human Spot-Check の2段階）
  - サンプル抽出ルール（30日間・100件・社内メール除外）
  - フィールドごとの比較関数（project_name, tanka, skills, remote_onsite, todofuken）
  - 失敗の分類（Type A〜D）と改善判定プロトコル（Type B が 25%以上なら Phase 2 失格）
  - Gate条件（Tier1 usable_rate ≥ 70%、Tier2 ≥ 50%）
  - → **Phase 2 が「測定方法の曖昧さ」で止まらない**
- ✅ **LLM API インターフェース契約**（🔌 新セクション追加）
  - Request/Response JSON スキーマを完全定義（extractor_hints の仕様、confidence フィールド等）
  - Error Responses の分類（4xx, 5xx, 429, timeout）とリトライ可否
  - Timeout/Retry Policy の明確化（Fixed backoff 5秒・最大3回）
  - Error Classification（Transient/Permanent/Internal/Timeout）と Queue status の遷移
  - Observability Requirements（Latency分布・Error Rate・Cost Tracking）
  - → **Phase 1 が「LLM API 仕様の曖昧さ」で止まらない**
- ✅ **DB整合性の固定**（トランザクション保護、上記で完了）
- ✅ **required_skills スキーマの固定**（Critical #8、`{"items": [{"skill": "...", "years": 0}]}` 形式で統一済み）
- → **「新機能を足す」のではなく、実装が止まらないように "契約" を明文化**

**2025-01-15 (実装詳細の最終補強)**:

**実装が止まる5つの穴を埋める**:
- ✅ **Retry戦略とステータス管理**（🔄 新セクション追加 → 3状態設計に改訂済み）
  - `handle_job_error()` 実装（retry_count 増分・next_retry_at 設定・永続エラー判定）
  - 3状態設計: pending / processing / completed のみ（failed は廃止）
  - リトライ可: `processing` → `pending`、永続エラー or retry上限: → `completed` + `manual_review`
  - `next_retry_at` カラムと index を追加（retry待機中のジョブを除外）
  - Worker polling クエリを修正（`next_retry_at IS NULL OR next_retry_at <= NOW()`）
  - → **Retry処理が明確化され、Recovery は Stuck Job 対策のみに専念**
- ✅ **enqueue冪等性の修正**（条件付きUPDATE版）
  - `enqueue_to_extraction_queue()` 実装を追加（DO NOTHING → 条件付きUPDATE）
  - `status='completed'` は上書きしない（確定済みレコードを守る）
  - それ以外（pending/processing）は最新の抽出結果で上書き（3状態設計）
  - Phase 2 でルール改善後に「過去メールで再抽出」が可能
  - → **運用の柔軟性が向上し、completed データは守られる**
- ✅ **extractor_hints の組み立てロジック**（Worker実装）
  - `build_extractor_hints()` 関数を追加（partial_fields → extractor_hints）
  - Tier1/Tier2 フィールド + メタ情報（decision_reason, subject_hash）を LLM に渡す
  - LLM のブレ削減・Token節約・デバッグ性向上
  - → **LLM API 呼び出しの品質と効率が向上**
- ✅ **Canary deployment の制御方法**（Phase 3 実装）
  - `should_enqueue_to_rust()` 関数を追加（message_id hash + SR_CANARY_PERCENT）
  - 環境変数だけで 0%→10%→50%→100% を柔軟に制御（再デプロイ不要）
  - 決定的な振り分け（同じ message_id は常に同じ側）
  - 検証クエリを追加（LLM呼び出し率の確認）
  - → **Canary rollout が安全かつ柔軟に実行可能**
- ✅ **KPI Dashboard の最低限仕様**（BIツールなし版）
  - `ses.kpi_daily` テーブル定義を追加（日次スナップショット保存）
  - systemd timer で日次集計バッチを実行（`sr-kpi-daily.sh`）
  - CLI/Markdown レポート出力・異常検知（LLM率 > 70% でログ出力）
  - Slack/Metabase/Grafana 不要（PF追加ゼロ）
  - → **「どこで見るの？」問題が解決し、Phase 2-4 の推移を追跡可能**

**前提条件の明確化**:
- Gmail → n8n → ses.anken_emails（当面維持）
- Slack通知なし（MVPはDB+ログ、必要なら Gmail送信）
- DB = PostgreSQL（extraction_queue と projects_enum が真実）

**2025-01-15 (Critical Bug Fixes - 最後の地雷除去)**:
- ✅ **1-A: DDL/Code Mismatch - body_text と subject_hash**
  - extraction_queue DDL に body_text カラムが無いのに、enqueue/polling で使用していた問題を修正
  - 修正: queue には message_id のみ保存、`fetch_body_text()` で anken_emails から都度取得
  - subject_hash 計算ロジック追加（SHA256 hash）
  - decision_reason, recommended_method, rule_version を INSERT に追加
  - → **enqueue と worker polling が DDL と完全に一致**
- ✅ **1-B: final_method 値の不統一**
  - CHECK 制約は `'rust_completed', 'llm_completed', 'manual_review'` を要求
  - KPI スクリプトが `'llm'` (短縮形) を使用していた問題を修正
  - 全 10+ 箇所で final_method 値を統一（'llm' → 'llm_completed'）
  - `call_llm()` の引数不足、`update_job_failed()` の引数不足も修正
  - → **CHECK 制約違反によるランタイムエラーを防止**
- ✅ **1-C: requires_manual_review カラムの混線**
  - extraction_queue DDL に requires_manual_review カラムが無いのに、`update_job_failed()` が UPDATE しようとしていた問題を修正
  - 修正: extraction_queue は `final_method='manual_review'` のみで管理
  - **【2025-01-16更新】** `requires_manual_review` は extraction_queue にも追加（Canary 制御用）
  - projects_enum の `requires_manual_review` は worker が INSERT 時に設定（最終判定結果）
  - extraction_queue の `requires_manual_review` は Canary 対象外の一時待避用
  - KPI 集計を修正（`requires_manual_review=true` → `final_method='manual_review'`）
  - Guardrail KPI テーブルと Note セクションも更新
  - → **DDL と実装が完全に一致し、extraction_queue UPDATE が成功する**

本設計書の数値は Phase 2-4 の実測により更新する。Gate未達の場合は Tier条件を緩和または LLM比率を調整する。

**2025-01-16 (確認推奨 11-12: Rust 移行前提の補強)**:

- ✅ **確認推奨 11: required_skills_keywords の正規化（Rust 実装）**
  - `sr-common/src/skill_normalizer.rs` 追加（`ALIAS_TO_CANONICAL` HashMap で O(1) lookup）
  - `sr-common/src/matching.rs` 追加（`check_required_skills()` でスキルマッチ判定）
  - 環境変数 `SR_SKILL_MATCH_THRESHOLD=0.3` で閾値制御（デフォルト30%）
  - `skills_to_keywords()` を更新（`normalize_skill()` 経由で正規化）
  - `["未指定"]` パターンは**使用禁止**（strategy.py で全員 KO になるため、空配列 `[]` で統一）
  - → **Python sponto-platform は migration 対象、スキル正規化は Rust で実装**

- ✅ **確認推奨 12: Canary 戦略（移行シナリオ別に再設計）**
  - 現行 n8n フローの制約を明記（AI Agent が先に動く → ルールベース移行には n8n 改修必須）
  - **シナリオ A**: 検証のみ（n8n 改修なし、ルールベース移行 ❌）
  - **シナリオ B**: 段階的移行（最小 n8n 改修、所有権 hash チェックで AI Agent スキップ、ルールベース移行 ✅）
  - **シナリオ C**: 一括切替（n8n の AI Agent 完全削除、Full Rust）
  - n8n 用 JavaScript 実装例追加（`isRustOwned()` 関数）
  - **推奨**: シナリオ B → C の段階移行
  - DDL に `canary_target`, `requires_manual_review` カラム追加（Canary 監視用）
  - → **ルールベース移行には n8n 改修が必要、その最小変更案を提示**

**2025-01-16 (Critical 5件 + Consistency Review)**:

- ✅ **Critical 1: `failed` ステータス廃止（3状態設計に統一）**
  - `status` を `pending / processing / completed` の3状態のみに統一
  - `failed` は DDL の CHECK 制約から削除
  - リトライは `pending + next_retry_at + retry_count` で表現
  - 永続エラーは `completed + final_method='manual_review'` でクローズ
  - → **Recovery/Worker/DDL の役割が明確化**

- ✅ **Critical 2: `required_skills_keywords` 空の扱い（仕様契約追加）**
  - 空配列 → `requires_manual_review=true` を必ず立てる
  - strategy.py の「空＝合格扱い」はそのまま（影響範囲最小化）
  - `["未指定"]` は絶対に使わない（KO になるため）
  - 仕様契約コメントを `is_sufficient_for_auto_matching()` に追加
  - → **migration後のマッチング挙動変動を防止**

- ✅ **Critical 3: `normalize_subject()` 正規表現修正**
  - `(?:(?:RE|FW|FWD):\s*)+` で多重 prefix を1回でマッチ
  - `calculate_subject_hash()` の hex 変換を修正（`GenericArray` → `iter().map()` 方式）
  - テストケース追加（多重転送、括弧、edge cases）
  - → **"RE: RE: 案件" が正しく "案件" になる**

- ✅ **Critical 4: `llm_latency_ms` 計測コード追加**
  - `LlmCallResult` 構造体追加（`response`, `latency_ms`, `attempts`）
  - `call_llm_with_retry()` で `Instant::now()` による計測
  - 仕様: リトライを含む合計時間を記録（KPI として現実的）
  - → **KPI の `llm_latency_ms` が常に NULL になる問題を解消**

- ✅ **Critical 5: ENUM 補正失敗時の NULL fallback**
  - 補正失敗 → 元の値を保持 ❌ → `None` (NULL) に落とす ✅
  - `log::warn!()` で元の値をログ出力（デバッグ用）
  - `requires_manual_review=true` を必ず立てる
  - 仕様契約コメントを `validate_and_correct_enums()` に追加
  - → **ENUM/TEXT どちらでも INSERT が成功する**

---

## 設計の核心

本 MVP は**「説明可能な抽出と継続的学習ループの構築」を主目的**とし、Rust は「前処理とブレの除去、判断根拠の蓄積」に集中する。

**7つの設計原則**:
1. **sr-extractor は projects_enum に直接入れない** → queue に必ず積み、worker が NOT NULL 制約を守る
2. **sr-extractor は「推奨判定」のみ** → worker が最終決定（既存ロジック/軽量ルール/LLM を柔軟に選択）
3. **start_date は raw で取る** → 「即日」「応相談」等の表現ゆらぎに対応、正規化は worker が行う
4. **outcome_tag を仕込む** → MVP では unknown、Later で営業マンの肌感覚を構造化データ化
5. **`required_skills_keywords` (TEXT[]) を必ず埋める** → 既存マッチング（strategy.py）がスキルKO/スコアリングに使用。空だと**マッチング品質が"ヌルゲー化"**する
6. **ENUM 補正は worker の最終責務** → 「東京都」→「東京」等の補正を worker が実施。補正失敗なら `requires_manual_review=true` で INSERT。**ENUM ミスマッチは INSERT 失敗の最大原因**
7. **Validate を2段階に分ける** → ①DB制約（NOT NULL等）は worker が必ず満たす（fallback値で埋める）、②ビジネス的に自動マッチ可能かで `manual_review` 判定。**NOT NULL 制約と manual_review 分岐の両立**

品質が下がる兆候（Guardrail KPI の Gate 逸脱）が出た場合は即ロールバックする。LLM呼び出しの最適化は、説明可能な処理を増やした**副次効果**として得られる。

---

## 🛡️ 破綻対策（長期運用のための設計）

> **破綻** = "稼働してる"が、"改善できない"状態

本設計の「説明可能 + 学習ループ」思想は、すでに破綻耐性が高い。
以下は「改善が回る」ことを DB上の最小ラベルで担保するための追加対策。

### 1. manual_review_reason（なぜ手動レビューか）

`final_method='manual_review'` だけでは「失敗の終着」と「品質保留」が混在。
→ `manual_review_reason` を追加し、**改善可能な失敗**を特定できるようにする。

| reason | 意味 | 改善アクション |
|--------|------|---------------|
| `enum_correction_failed` | ENUM補正失敗 | ENUM辞書を拡張 |
| `skills_empty` | スキル抽出失敗 | 軽量ルールパターン追加 |
| `start_date_unknown` | 開始日不明 | 日付パターン追加 |
| `llm_permanent_error` | LLM API恒久エラー | プロンプト見直し |
| `body_missing` | メール本文がない | 上流データ確認 |

### 2. reprocess_after（再処理可能フラグ）

completed は絶対に再処理しない → 安全だが**改善ループが回らない**。
→ `reprocess_after` を追加し、「直したら効く」を担保する。

- ルール改善後に再処理したい案件だけ `reprocess_after = NOW()` を設定
- 再処理時に NULL にリセット（無限ループ防止）
- 人が直した/ルールを上げた案件だけ再処理可能

### 3. スキル空配列率の監視

空配列でも strategy.py は "合格扱い" → **常態化するとマッチングの意味が薄くなる**。
→ Guardrail KPI に「スキル空配列率」を追加し、20% 超で軽量ルール拡張を検討。

### 4. 軽量ルールの第二入口

"スキル" と書いてないメールが多い → 軽量ルールが効かない。
→ ルールを増やしすぎず、**入口（ヘッダーパターン）だけ増やす**。

```
第一入口: 「必須スキル」「スキル：」等
第二入口: 「言語:」「環境:」「技術:」「OS:」「DB:」「FW:」等
```

**破綻防止の核心**: 「直したら効く」を DB上の最小ラベルで担保すること。

---

## 🔬 Phase 2 測定仕様の契約

Phase 2 では **Rust 軽量ルール vs 既存 LLM** のフィールド抽出精度を比較し、Gate 条件を満たすか検証する。

### 測定の目的

**目的**:
- **NOT**: LLM を "正解" として扱い、一致率で合否判定する（← これは間違い）
- **YES**: Rust 軽量ルールが「ビジネス的に使える品質か」を2段階で検証する
  1. **Regression Test（一致率）**: 既存 LLM 出力との乖離が大きすぎないか（50% 以上乖離だと危険信号）
  2. **Human Spot-Check（監査）**: 不一致レコードを営業が目視し、「Rust が正しい」「LLM が正しい」「どちらも怪しい」を判定

### サンプル抽出ルール

**対象メール**:
- **期間**: 最近30日間の `ses.projects_enum` に保存された案件メール
- **件数**: 100件（統計的有意性を確保）
- **条件**:
  - `source_text` が存在する（body_text が NULL でない）
  - `message_id` が重複していない（同一メールの多重測定を避ける）
  - 社内メール（`@sponto.co.jp`）を除外（既存ロジックと一致）

**抽出クエリ**:
```sql
SELECT
    project_code,
    message_id,
    source_text,
    project_name,
    required_skills,
    required_skills_keywords,
    monthly_tanka_min,
    monthly_tanka_max,
    remote_onsite,
    work_todofuken
FROM ses.projects_enum
WHERE created_at >= NOW() - INTERVAL '30 days'
  AND source_text IS NOT NULL
  AND message_id IS NOT NULL
  AND message_id NOT LIKE '%@sponto.co.jp'
ORDER BY RANDOM()
LIMIT 100;
```

### フィールドごとの比較関数

各フィールドに対して「一致」「部分一致」「不一致」を判定する。

#### 1. `project_name`（プロジェクト名）

**比較ロジック**:
```rust
fn compare_project_name(llm: &str, rust: &str) -> MatchResult {
    let llm_norm = normalize_for_comparison(llm);
    let rust_norm = normalize_for_comparison(rust);

    if llm_norm == rust_norm {
        return MatchResult::Exact;
    }

    // 部分一致: どちらかが他方を含む
    if llm_norm.contains(&rust_norm) || rust_norm.contains(&llm_norm) {
        return MatchResult::Partial;
    }

    MatchResult::Mismatch
}

fn normalize_for_comparison(s: &str) -> String {
    s.to_lowercase()
        .replace("【", "")
        .replace("】", "")
        .replace(" ", "")
        .replace("　", "")
}
```

**判定基準**:
- `Exact`: 完全一致（正規化後）
- `Partial`: 部分一致（例: LLM「Java開発案件」, Rust「Java開発」）
- `Mismatch`: 不一致（例: LLM「Java開発」, Rust「Python案件」）

#### 2. `monthly_tanka_min` / `monthly_tanka_max`（月単価レンジ）

**比較ロジック**:
```rust
fn compare_tanka(llm_min: Option<u32>, llm_max: Option<u32>,
                 rust_min: Option<u32>, rust_max: Option<u32>) -> MatchResult {
    match (llm_min, llm_max, rust_min, rust_max) {
        (Some(lmin), Some(lmax), Some(rmin), Some(rmax)) => {
            if lmin == rmin && lmax == rmax {
                return MatchResult::Exact;
            }

            // 10万円以内の差なら Partial（誤差範囲）
            if (lmin as i32 - rmin as i32).abs() <= 10 &&
               (lmax as i32 - rmax as i32).abs() <= 10 {
                return MatchResult::Partial;
            }

            MatchResult::Mismatch
        }
        (None, None, None, None) => MatchResult::Exact,  // 両方とも抽出失敗
        _ => MatchResult::Mismatch  // 片方だけ抽出
    }
}
```

**判定基準**:
- `Exact`: 完全一致 or 両方とも NULL
- `Partial`: ±10万円以内の差（例: LLM「60-80」, Rust「55-75」）
- `Mismatch`: 10万円超の差 or 片方だけ抽出

#### 3. `required_skills_keywords`（必須スキル）

**比較ロジック**:
```rust
fn compare_skills(llm: &[String], rust: &[String]) -> MatchResult {
    let llm_set: HashSet<String> = llm.iter()
        .map(|s| s.to_lowercase())
        .collect();
    let rust_set: HashSet<String> = rust.iter()
        .map(|s| s.to_lowercase())
        .collect();

    if llm_set == rust_set {
        return MatchResult::Exact;
    }

    // Jaccard係数で類似度を測定
    let intersection = llm_set.intersection(&rust_set).count();
    let union = llm_set.union(&rust_set).count();
    let jaccard = intersection as f64 / union as f64;

    if jaccard >= 0.5 {
        MatchResult::Partial
    } else {
        MatchResult::Mismatch
    }
}
```

**判定基準**:
- `Exact`: 完全一致（順序は無視）
- `Partial`: Jaccard係数 ≥ 0.5（例: LLM「[Java, AWS, Docker]」, Rust「[Java, AWS]」）
- `Mismatch`: Jaccard係数 < 0.5

#### 4. `remote_onsite`（リモート可否）

**比較ロジック**:
```rust
fn compare_remote_onsite(llm: Option<&str>, rust: Option<&str>) -> MatchResult {
    match (llm, rust) {
        (Some(l), Some(r)) if l == r => MatchResult::Exact,
        (Some(l), Some(r)) => {
            // 「リモート併用」と「フルリモート」は境界が曖昧 → Partial
            if (l == "リモート併用" && r == "フルリモート") ||
               (l == "フルリモート" && r == "リモート併用") {
                return MatchResult::Partial;
            }
            MatchResult::Mismatch
        }
        (None, None) => MatchResult::Exact,
        _ => MatchResult::Mismatch
    }
}
```

#### 5. `work_todofuken`（都道府県）

**比較ロジック**:
```rust
fn compare_todofuken(llm: Option<&str>, rust: Option<&str>) -> MatchResult {
    match (llm, rust) {
        (Some(l), Some(r)) if l == r => MatchResult::Exact,
        (None, None) => MatchResult::Exact,
        _ => MatchResult::Mismatch
    }
}
```

### 失敗の分類（Taxonomy）

不一致レコードを次の4つに分類し、原因分析を行う。

| 分類 | 定義 | 対処 |
|------|------|------|
| **Type A: Rust が正しい** | LLM が誤抽出（例: 「60万〜80万」→ LLM「50万〜70万」） | 問題なし。Rust が勝っている |
| **Type B: LLM が正しい** | Rust が抽出漏れ or 誤抽出 | **要改善**。ルールを修正 |
| **Type C: どちらも間違い** | 両方とも source_text と矛盾 | メール本文が複雑すぎる → manual_review |
| **Type D: 判定不能** | メール本文が曖昧で正解が定まらない | 仕様通り（曖昧さは仕様） |

**測定プロセス**:
1. **自動判定**: 上記の比較関数で `Exact` / `Partial` / `Mismatch` を分類
2. **サンプリング**: `Mismatch` レコードから20件をランダム抽出
3. **Human Spot-Check**: 営業が目視で Type A〜D に分類
4. **改善判定**:
   - **Type B が 20件中 5件以上（25%+）**: ルール改善が必要（Phase 2 失格）
   - **Type B が 20件中 4件以下（20%未満）**: 許容範囲（Phase 3 進行可）

### Regression Test（一致率）

**測定指標**:
```sql
-- フィールドごとの一致率
WITH comparison AS (
    SELECT
        project_code,
        CASE
            WHEN llm.project_name = rust.project_name THEN 'exact'
            WHEN llm.project_name ILIKE '%' || rust.project_name || '%' THEN 'partial'
            ELSE 'mismatch'
        END AS project_name_match,

        CASE
            WHEN llm.monthly_tanka_min = rust.monthly_tanka_min
                AND llm.monthly_tanka_max = rust.monthly_tanka_max THEN 'exact'
            WHEN ABS(llm.monthly_tanka_min - rust.monthly_tanka_min) <= 10
                AND ABS(llm.monthly_tanka_max - rust.monthly_tanka_max) <= 10 THEN 'partial'
            ELSE 'mismatch'
        END AS tanka_match
    FROM llm_results llm
    JOIN rust_results rust USING (project_code)
)
SELECT
    COUNT(*) FILTER (WHERE project_name_match = 'exact') * 100.0 / COUNT(*) AS project_name_exact_rate,
    COUNT(*) FILTER (WHERE project_name_match IN ('exact', 'partial')) * 100.0 / COUNT(*) AS project_name_usable_rate,
    COUNT(*) FILTER (WHERE tanka_match = 'exact') * 100.0 / COUNT(*) AS tanka_exact_rate,
    COUNT(*) FILTER (WHERE tanka_match IN ('exact', 'partial')) * 100.0 / COUNT(*) AS tanka_usable_rate
FROM comparison;
```

**Gate条件**:
- **Tier1 フィールド**（project_name, required_skills, tanka, remote_onsite）:
  - `usable_rate` ≥ 70%（Exact + Partial の合計）
  - `mismatch_rate` < 30%
- **Tier2 フィールド**（work_todofuken, min_experience_years）:
  - `usable_rate` ≥ 50%

**Phase 2 失格条件**:
- **いずれかの Tier1 フィールドが Gate 未達** → ルール改善が必要
- **Human Spot-Check で Type B（Rust が間違い）が 25%以上** → 設計見直し

### 不一致時の対処プロトコル

**Step 1: 自動分類**
```bash
# 測定スクリプト実行
$ cargo run --bin sr-measurement -- \
    --llm-results llm_100.json \
    --rust-results rust_100.json \
    --output comparison_report.json
```

**Step 2: 不一致サンプル抽出**
```sql
-- Mismatch レコードから20件抽出
SELECT * FROM comparison
WHERE project_name_match = 'mismatch'
   OR tanka_match = 'mismatch'
   OR skills_match = 'mismatch'
ORDER BY RANDOM()
LIMIT 20;
```

**Step 3: Human Spot-Check（営業が目視）**
- Google SpreadSheet に出力
- 列: `[project_code, source_text, llm_output, rust_output, judgment (Type A/B/C/D), notes]`
- 営業が `judgment` と `notes` を記入

**Step 4: 改善判定**
```python
# Type B（Rust が間違い）の割合を計算
type_b_count = df[df['judgment'] == 'Type B'].shape[0]
type_b_rate = type_b_count / 20

if type_b_rate >= 0.25:
    print("❌ Phase 2 失格: ルール改善が必要")
    print(f"Type B: {type_b_count}/20 件 ({type_b_rate*100:.1f}%)")
else:
    print("✅ Phase 2 合格: Phase 3 進行可")
```

---

## 🔌 LLM API インターフェース契約

Phase 1 で sr-llm-worker が呼び出す LLM API の仕様を固定し、Phase 1 が止まらないようにする。

### Request スキーマ

**エンドポイント**: `POST /api/v1/extract`

**Headers**:
```http
Content-Type: application/json
Authorization: Bearer {API_TOKEN}
X-Request-ID: {uuid}  # トレーシング用
```

**Body**:
```json
{
  "message_id": "20250112-abc123@example.com",
  "source_text": "お世話になっております。\n\n案件情報をお送りします...",
  "extractor_hints": {
    "project_name": "Java開発案件",
    "monthly_tanka_min": 60,
    "monthly_tanka_max": 80,
    "required_skills_keywords": ["Java", "AWS"],
    "remote_onsite": "リモート併用",
    "work_todofuken": "東京"
  },
  "model": "claude-opus-4-5",
  "timeout_seconds": 30
}
```

**フィールド説明**:
- `message_id`: メール識別子（トレーシング用、必須）
- `source_text`: メール本文（LLM に渡すプレーンテキスト、必須）
- `extractor_hints`: Rust 軽量ルールの抽出結果（LLM が参考にする、Optional）
  - Tier1/Tier2 が部分的に抽出できていれば埋める
  - LLM はこれを **ヒント** として扱い、矛盾があれば source_text を優先
- `model`: 使用モデル（デフォルト: `claude-opus-4-5`）
- `timeout_seconds`: タイムアウト秒数（デフォルト: 30）

### extractor_hints の組み立て（Worker実装）

Worker が LLM を呼び出す際、Rust 軽量ルールの抽出結果を `extractor_hints` として渡す。

**目的**:
- LLM のブレを減らす（既に抽出できている値を hint として渡す）
- Token 節約（LLM が「単価を探す」処理を省略できる）
- デバッグ性向上（LLM が何を参考にしたか追跡可能）

**実装**:

```rust
// sr-llm-worker/src/llm.rs

fn build_extractor_hints(partial_fields: &serde_json::Value) -> serde_json::Value {
    let mut hints = serde_json::Map::new();

    // partial_fields から抽出済みフィールドを取り出す
    if let Some(obj) = partial_fields.as_object() {
        // Tier1 フィールド
        if let Some(name) = obj.get("project_name").and_then(|v| v.as_str()) {
            hints.insert("project_name".to_string(), json!(name));
        }
        if let Some(min) = obj.get("monthly_tanka_min").and_then(|v| v.as_u64()) {
            hints.insert("monthly_tanka_min".to_string(), json!(min));
        }
        if let Some(max) = obj.get("monthly_tanka_max").and_then(|v| v.as_u64()) {
            hints.insert("monthly_tanka_max".to_string(), json!(max));
        }
        if let Some(skills) = obj.get("required_skills_keywords").and_then(|v| v.as_array()) {
            hints.insert("required_skills_keywords".to_string(), json!(skills));
        }
        if let Some(remote) = obj.get("remote_onsite").and_then(|v| v.as_str()) {
            hints.insert("remote_onsite".to_string(), json!(remote));
        }

        // Tier2 フィールド
        if let Some(todofuken) = obj.get("work_todofuken").and_then(|v| v.as_str()) {
            hints.insert("work_todofuken".to_string(), json!(todofuken));
        }
        if let Some(years) = obj.get("min_experience_years").and_then(|v| v.as_f64()) {
            hints.insert("min_experience_years".to_string(), json!(years));
        }

        // メタ情報（LLMがコンテキストを理解するため）
        if let Some(reason) = obj.get("decision_reason").and_then(|v| v.as_str()) {
            hints.insert("_rust_decision_reason".to_string(), json!(reason));
        }
        if let Some(hash) = obj.get("subject_hash").and_then(|v| v.as_str()) {
            hints.insert("_subject_hash".to_string(), json!(hash));
        }
    }

    serde_json::Value::Object(hints)
}

// LLM Request 組み立て（body_text は anken_emails から取得済み）
let body_text = fetch_body_text(&db_client, &job.message_id).await?;

let request = LlmRequest {
    message_id: job.message_id.clone(),
    source_text: body_text,
    extractor_hints: build_extractor_hints(&job.partial_fields),  // ← ここで渡す
    model: "claude-opus-4-5".to_string(),
    timeout_seconds: 30,
};
```

**Note**: Worker は queue から job を取得後、必ず `fetch_body_text()` で anken_emails から本文を取得する。

**LLM側での扱い**:
- `extractor_hints` を **参考情報** として扱う
- source_text と矛盾があれば source_text を優先
- 例: hints に `monthly_tanka_min: 60` があるが、source_text に「55万円〜」とあれば LLM は 55 を採用

### Response スキーマ

**Success (200 OK)**:
```json
{
  "message_id": "20250112-abc123@example.com",
  "status": "success",
  "extracted": {
    "project_name": "Java開発案件（金融系）",
    "project_type": ["SE", "PG"],
    "project_keywords": ["金融", "Java", "AWS"],
    "required_skills": {
      "items": [
        {"skill": "Java", "years": 3},
        {"skill": "AWS", "years": 2}
      ]
    },
    "required_skills_keywords": ["Java", "AWS", "Spring Boot"],
    "preferred_skills": {
      "items": [
        {"skill": "Docker", "years": 1}
      ]
    },
    "preferred_skills_keywords": ["Docker", "Kubernetes"],
    "monthly_tanka_min": 60,
    "monthly_tanka_max": 80,
    "remote_onsite": "リモート併用",
    "onsite_frequency": 2.0,
    "work_area": "関東",
    "work_todofuken": "東京",
    "work_station": "新宿駅",
    "start_date": "即日",
    "end_date": "長期",
    "min_experience_years": 3.0,
    "is_long_term": true,
    "tech_kubun": "人気技術",
    "is_kojin_ok": true,
    "foreigner_allowed": false,
    "japanese_skill": "ネイティブ",
    "english_skill": "不要",
    "interviews_count": 2,
    "contract_type": "準委任契約",
    "flow_dept": "1次請け",
    "jinzai_flow_limit": "SPONTO一社先まで"
  },
  "confidence": {
    "overall": 0.85,
    "project_name": 0.95,
    "monthly_tanka": 0.90,
    "required_skills": 0.80
  },
  "latency_ms": 2340,
  "model_used": "claude-opus-4-5"
}
```

**Partial Success (200 OK with missing fields)**:
```json
{
  "message_id": "20250112-abc123@example.com",
  "status": "partial",
  "extracted": {
    "project_name": "案件情報",
    "required_skills_keywords": []
  },
  "missing_fields": [
    "monthly_tanka_min",
    "monthly_tanka_max",
    "required_skills_keywords"
  ],
  "requires_manual_review": true,
  "reason": "メール本文に単価・スキル情報が含まれていません",
  "latency_ms": 1820,
  "model_used": "claude-opus-4-5"
}
```

**Error Responses**:

| Status Code | Error Type | Body Example | Retry? |
|-------------|------------|--------------|--------|
| **400** | Bad Request | `{"error": "invalid_request", "message": "source_text is required"}` | ❌ No |
| **401** | Unauthorized | `{"error": "unauthorized", "message": "Invalid API token"}` | ❌ No |
| **429** | Rate Limit | `{"error": "rate_limit_exceeded", "message": "Too many requests", "retry_after": 60}` | ✅ Yes (60s後) |
| **500** | Internal Error | `{"error": "internal_error", "message": "Model inference failed"}` | ✅ Yes (Fixed backoff) |
| **503** | Service Unavailable | `{"error": "service_unavailable", "message": "Model is overloaded"}` | ✅ Yes (Fixed backoff) |
| **504** | Gateway Timeout | `{"error": "timeout", "message": "Request timeout"}` | ✅ Yes (Fixed backoff) |

### Rust クライアント実装（完全版）

**⚠️ subprocess は避ける**: ログ/timeout/並列で地獄になりがち。**HTTP (reqwest) で固定**が最も事故りにくい。

**LLM API 統合の全体像**:

```
sr-llm-worker (Rust)
       │
       │ HTTP POST
       ▼
Python LLM Wrapper API (sponto-platform 内で稼働)
       │
       │ HTTP (OpenRouter API)
       ▼
Claude / DeepSeek / 他のLLMモデル
```

**使用可能なモデル（MVP推奨順）**:
| モデル名 | プロバイダ | 用途 | 備考 |
|----------|-----------|------|------|
| `claude-3-5-sonnet-20241022` | OpenRouter | 本番推奨 | 高品質・適正コスト |
| `deepseek-v3` | DeepSeek API | テスト用 | 低コスト・日本語対応 |
| `gpt-4o-mini` | OpenRouter | フォールバック | 低コスト |

**必要な環境変数**:
```bash
# LLM API 接続設定
LLM_ENDPOINT=http://localhost:8000/api/v1/extract   # Python LLM Wrapper のエンドポイント
LLM_TOKEN=your-api-token                             # 認証トークン（Python Wrapper用）
LLM_MODEL=claude-3-5-sonnet-20241022                 # 使用モデル（デフォルト: claude-3-5-sonnet）
LLM_TIMEOUT_SECONDS=120                              # タイムアウト秒数（デフォルト: 120）
LLM_MAX_RETRIES=3                                    # 最大リトライ回数（デフォルト: 3）
```

**Python LLM Wrapper API 仕様（sponto-platform 側）**:
- **エンドポイント**: `POST /api/v1/extract`
- **認証**: `Authorization: Bearer {LLM_TOKEN}` ヘッダー
- **Content-Type**: `application/json`
- **リクエスト例**:
```json
{
  "message_id": "msg_12345",
  "source_text": "【案件】Java開発 / 80万円 / 東京 / 1月稼働開始...",
  "extractor_hints": {
    "detected_price": 800000,
    "detected_location": "東京都"
  },
  "model": "claude-3-5-sonnet-20241022",
  "timeout_seconds": 120
}
```
- **レスポンス例**（成功時）:
```json
{
  "project_name": "Java開発案件",
  "monthly_tanka_min": 70,
  "monthly_tanka_max": 80,
  "required_skills_keywords": ["Java", "Spring Boot", "PostgreSQL"],
  "work_todofuken": "東京都",
  "start_date_raw": "1月上旬",
  "remote_onsite": "リモート併用",
  "flow_dept": "2次請け",  // ⚠️ 商流制限なし は jinzai_flow_limit の値、flow_dept は エンド直/1次請け/2次請け/...
  "confidence_score": 0.92
}
```

**Cargo.toml 追加**:
```toml
reqwest = { version = "0.11", features = ["json"] }
```

**完全実装 (sr-llm-worker/src/llm_client.rs)**:
```rust
// sr-llm-worker/src/llm_client.rs
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct LlmRequest {
    pub message_id: String,
    pub source_text: String,
    pub extractor_hints: serde_json::Value,
    pub model: String,
    pub timeout_seconds: u32,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 【仕様契約 SC-18: LlmResponse serde 強化】(#14 追加: 2025-12-17)
//
// LLM のレスポンスは信頼できない前提で設計する:
//   - 型不一致 → デフォルト値にフォールバック
//   - 範囲外 → クランプまたはデフォルト
//   - 必須フィールド欠損 → manual_review フラグ
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Deserialize)]
pub struct LlmResponse {
    pub message_id: String,
    #[serde(default = "default_status")]
    pub status: LlmStatus,
    #[serde(default)]
    pub extracted: LlmExtractedProject,
    pub latency_ms: Option<i32>,
    pub model_used: Option<String>,
}

fn default_status() -> LlmStatus { LlmStatus::Partial }

/// LLM 応答ステータス（型安全）
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmStatus {
    Success,
    Partial,
    #[serde(other)]  // 未知の値は Partial 扱い
    Unknown,
}

/// LLM から抽出された案件データ（バリデーション付き）
#[derive(Debug, Default, Deserialize)]
pub struct LlmExtractedProject {
    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub project_name: Option<String>,

    #[serde(default, deserialize_with = "deserialize_tanka")]
    pub monthly_tanka_min: Option<i32>,

    #[serde(default, deserialize_with = "deserialize_tanka")]
    pub monthly_tanka_max: Option<i32>,

    #[serde(default, deserialize_with = "deserialize_skills_array")]
    pub required_skills_keywords: Vec<String>,

    #[serde(default, deserialize_with = "deserialize_skills_array")]
    pub preferred_skills_keywords: Vec<String>,

    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub work_todofuken: Option<String>,

    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub start_date_raw: Option<String>,

    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub remote_onsite: Option<String>,

    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub flow_dept: Option<String>,

    #[serde(default)]
    pub confidence_score: Option<f64>,
}

// ━━━━━━━ カスタムデシリアライザ ━━━━━━━

use serde::de::{self, Deserializer};

/// 文字列フィールド: 空文字 → None、数値→文字列変換
fn deserialize_option_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::String(s) if s.trim().is_empty() => Ok(None),
        Value::String(s) => Ok(Some(s)),
        Value::Number(n) => Ok(Some(n.to_string())),  // LLM が数値で返すことがある
        Value::Null => Ok(None),
        _ => Ok(None),  // 配列等は無視
    }
}

/// 単価フィールド: 文字列→数値変換、範囲チェック (10-500万)
fn deserialize_tanka<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    let v = Value::deserialize(deserializer)?;

    let raw_value: Option<i32> = match v {
        Value::Number(n) => n.as_i64().map(|x| x as i32),
        Value::String(s) => {
            // "70万" → 70, "700000" → 70
            let cleaned = s.trim()
                .replace("万", "")
                .replace(",", "")
                .replace("円", "");
            if let Ok(n) = cleaned.parse::<i64>() {
                // 1000以上は円単位とみなして万単位に変換
                Some(if n >= 1000 { (n / 10000) as i32 } else { n as i32 })
            } else {
                None
            }
        }
        Value::Null => None,
        _ => None,
    };

    // 範囲チェック: 10-500万（SES案件として妥当な範囲）
    Ok(raw_value.and_then(|v| {
        if v >= 10 && v <= 500 { Some(v) }
        else { None }  // 範囲外は無効扱い
    }))
}

/// スキル配列: 文字列→配列変換、空配列正規化
fn deserialize_skills_array<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    let v = Value::deserialize(deserializer)?;

    match v {
        Value::Array(arr) => {
            Ok(arr.into_iter()
                .filter_map(|item| {
                    match item {
                        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                        _ => None,
                    }
                })
                .collect())
        }
        // LLM がカンマ区切り文字列で返すことがある
        Value::String(s) => {
            Ok(s.split(&[',', '、', '/', '・'][..])
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect())
        }
        Value::Null => Ok(vec![]),
        _ => Ok(vec![]),
    }
}

impl LlmExtractedProject {
    /// 抽出結果が有効かどうか判定（Tier1フィールドが埋まっているか）
    pub fn is_valid(&self) -> bool {
        self.project_name.is_some() &&
        self.monthly_tanka_min.is_some() &&
        self.work_todofuken.is_some() &&
        !self.required_skills_keywords.is_empty()
    }

    /// manual_review が必要な理由を収集
    pub fn validation_issues(&self) -> Vec<&'static str> {
        let mut issues = vec![];
        if self.project_name.is_none() { issues.push("project_name_missing"); }
        if self.monthly_tanka_min.is_none() { issues.push("tanka_missing"); }
        if self.work_todofuken.is_none() { issues.push("location_missing"); }
        if self.required_skills_keywords.is_empty() { issues.push("skills_empty"); }
        issues
    }
}

#[cfg(test)]
mod tests_serde {
    use super::*;

    #[test]
    fn test_tanka_from_string() {
        // "70万" → 70
        let json = r#"{"monthly_tanka_min": "70万", "monthly_tanka_max": 80}"#;
        let extracted: LlmExtractedProject = serde_json::from_str(json).unwrap();
        assert_eq!(extracted.monthly_tanka_min, Some(70));
        assert_eq!(extracted.monthly_tanka_max, Some(80));
    }

    #[test]
    fn test_tanka_out_of_range() {
        // 5万は範囲外 → None
        let json = r#"{"monthly_tanka_min": 5}"#;
        let extracted: LlmExtractedProject = serde_json::from_str(json).unwrap();
        assert_eq!(extracted.monthly_tanka_min, None);
    }

    #[test]
    fn test_skills_from_comma_string() {
        // カンマ区切り文字列 → 配列
        let json = r#"{"required_skills_keywords": "Java, Spring Boot, PostgreSQL"}"#;
        let extracted: LlmExtractedProject = serde_json::from_str(json).unwrap();
        assert_eq!(extracted.required_skills_keywords, vec!["Java", "Spring Boot", "PostgreSQL"]);
    }

    #[test]
    fn test_validation_issues() {
        let extracted = LlmExtractedProject::default();
        let issues = extracted.validation_issues();
        assert!(issues.contains(&"project_name_missing"));
        assert!(issues.contains(&"skills_empty"));
    }
}
// ━━━━━━━ ここまで SC-18 追加 ━━━━━━━

#[derive(Debug)]
pub enum LlmError {
    BadRequest(String),       // 400: 永続エラー（retry不可）
    Unauthorized(String),     // 401: 永続エラー（retry不可）
    RateLimited { retry_after: u64, msg: String },  // 429: retry可
    ServerError(String),      // 5xx: retry可
    Timeout(String),          // タイムアウト: retry可
    Transport(String),        // ネットワークエラー: retry可
}

impl LlmError {
    /// B案: retry可能かどうか判定
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::BadRequest(_) | LlmError::Unauthorized(_) => false,
            _ => true,
        }
    }

    /// 429 の retry_after を Queue の next_retry_at に反映するためのヒント
    /// - 429 の場合: API が指定した retry_after 秒
    /// - それ以外: None（デフォルト 5 分を使用）
    pub fn retry_after_hint(&self) -> Option<std::time::Duration> {
        match self {
            LlmError::RateLimited { retry_after, .. } => {
                Some(std::time::Duration::from_secs(*retry_after))
            }
            _ => None,
        }
    }
}

/// LLM API 呼び出し結果（latency_ms を含む）
pub struct LlmCallResult {
    pub response: LlmResponse,
    pub latency_ms: u32,  // 合計経過時間（リトライ含む）
    pub attempts: u32,    // 試行回数
}

/// LLM API 呼び出し（リトライ込み、完全実装）
///
/// 【仕様】llm_latency_ms の定義
/// - 成功した最終レスポンス取得までの経過時間（合計）
/// - リトライを含む（KPI として現実的な値）
pub async fn call_llm_with_retry(req: LlmRequest) -> Result<LlmCallResult, LlmError> {
    use std::time::Instant;

    let endpoint = std::env::var("LLM_ENDPOINT").expect("LLM_ENDPOINT required");
    let token = std::env::var("LLM_TOKEN").expect("LLM_TOKEN required");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(req.timeout_seconds as u64))
        .build()
        .map_err(|e| LlmError::Transport(e.to_string()))?;

    let mut attempt: u32 = 0;
    let max_retries: u32 = 3;

    // ⚠️ Critical 4: latency 計測開始
    let start_time = Instant::now();

    loop {
        attempt += 1;

        let resp = client
            .post(format!("{}/api/v1/extract", endpoint))
            .bearer_auth(&token)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                // Transport error（ネットワーク障害、タイムアウト等）
                if attempt <= max_retries {
                    log::warn!("LLM request failed (attempt {}/{}): {}", attempt, max_retries, e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                return Err(LlmError::Transport(e.to_string()));
            }
        };

        match resp.status() {
            StatusCode::OK => {
                let parsed = resp.json::<LlmResponse>().await
                    .map_err(|e| LlmError::Transport(format!("JSON parse error: {}", e)))?;
                // ⚠️ Critical 4: latency 計測終了
                let latency_ms = start_time.elapsed().as_millis() as u32;
                return Ok(LlmCallResult {
                    response: parsed,
                    latency_ms,
                    attempts: attempt,
                });
            }
            StatusCode::BAD_REQUEST => {
                let txt = resp.text().await.unwrap_or_default();
                return Err(LlmError::BadRequest(txt));  // 永続エラー、retry不可
            }
            StatusCode::UNAUTHORIZED => {
                let txt = resp.text().await.unwrap_or_default();
                return Err(LlmError::Unauthorized(txt));  // 永続エラー、retry不可
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);

                let txt = resp.text().await.unwrap_or_default();
                if attempt <= max_retries {
                    log::warn!("Rate limited, retrying in {}s (attempt {}/{})", retry_after, attempt, max_retries);
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    continue;
                }
                return Err(LlmError::RateLimited { retry_after, msg: txt });
            }
            s if s.is_server_error() || s == StatusCode::GATEWAY_TIMEOUT => {
                let txt = resp.text().await.unwrap_or_default();
                if attempt <= max_retries {
                    log::warn!("Server error {}, retrying in 5s (attempt {}/{})", s, attempt, max_retries);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                return Err(LlmError::ServerError(format!("status={} body={}", s, txt)));
            }
            other => {
                let txt = resp.text().await.unwrap_or_default();
                return Err(LlmError::ServerError(format!("unexpected status={} body={}", other, txt)));
            }
        }
    }
}
```

---

### Timeout/Retry Policy

**リトライ戦略**:
- **429 (Rate Limit)**: `retry_after` 秒待ってから1回だけリトライ
- **500/503/504**: Fixed backoff（5秒）で最大3回リトライ
- **400/401**: リトライしない（設定ミス or 認証エラー）

### Error Classification

LLM API エラーを次の4つに分類し、適切に処理する。

| 分類 | HTTP Status | 処理 | Queue Status (3状態設計) |
|------|-------------|------|---------------------------|
| **Rate Limit** | 429 | Retry after `retry_after` seconds | `processing` → `pending` (retry_count++, **next_retry_at = NOW() + retry_after**) |
| **Transient Error** | 503, 504 | Retry with default backoff | `processing` → `pending` (retry_count++, next_retry_at = NOW() + 5min) |
| **Permanent Error** | 400, 401 | No retry, log error | `processing` → `completed` (final_method='manual_review') |
| **Internal Error** | 500 | Retry once, then fail | `processing` → `pending` or `completed` (retry上限で判定) |
| **Timeout** | (no response) | Retry with default backoff | `processing` → `pending` (retry_count++, next_retry_at = NOW() + 5min) |

**⚠️ 3状態設計**: `failed` ステータスは使用しない。リトライ可能なエラーは `pending` に戻し、永続エラー or retry上限到達は `completed` + `manual_review` で終結。

**Worker 実装**:
```rust
match call_llm_with_retry(request).await {
    Ok(result) => {
        // 成功 → projects_enum に INSERT
        // ⚠️ Critical 4: result.latency_ms を使用（リトライ含む合計時間）
        log::info!("LLM call succeeded: latency={}ms, attempts={}", result.latency_ms, result.attempts);
        commit_final_result(
            db_client,
            job_id,
            &result.response,
            false,
            "llm_completed",
            Some(result.latency_ms),  // ← 計測済み latency を渡す
        ).await?;
    }
    Err(LlmError::BadRequest(msg) | LlmError::Unauthorized(msg)) => {
        // 永続エラー → manual_review でクローズ
        handle_job_error(&db_client, job_id, &msg, ErrorClass::Permanent, None).await?;
    }
    Err(e) => {
        // 一時エラー → pending に戻す（3状態設計）
        // ⚠️ 429 の場合は retry_after_hint() で待機時間を取得
        let retry_hint = e.retry_after_hint();  // 429 なら Some(Duration), それ以外は None
        handle_job_error(&db_client, job_id, &format!("{:?}", e), ErrorClass::Retryable, retry_hint).await?;
    }
}
```

### Observability Requirements

LLM API 呼び出しの可観測性を確保するため、以下のメトリクスを記録する。

**必須メトリクス**:
1. **Latency分布**: `llm_latency_ms` の p50/p90/p99
2. **Error Rate**: HTTP status code 別のエラー率
3. **Retry Rate**: リトライが発生した割合
4. **Success Rate**: 最終的に成功した割合（リトライ後含む）
5. **Cost Tracking**: LLM API 呼び出し回数（コスト計算用）

**監視クエリ**:
```sql
-- LLM APIレイテンシ分布（過去24時間）
SELECT
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY llm_latency_ms) AS p50,
    PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY llm_latency_ms) AS p90,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY llm_latency_ms) AS p99,
    AVG(llm_latency_ms) AS avg_latency
FROM ses.extraction_queue
WHERE final_method = 'llm_completed'
  AND completed_at >= NOW() - INTERVAL '24 hours';

-- LLM Error Rate（過去24時間）
-- ※ status='failed' は廃止。retry_count >= 3 または final_method='manual_review' でエラー率を計測
SELECT
    COUNT(*) FILTER (WHERE retry_count >= 3) * 100.0 / NULLIF(COUNT(*), 0) AS high_retry_rate,
    COUNT(*) FILTER (WHERE final_method = 'manual_review') * 100.0 / NULLIF(COUNT(*), 0) AS manual_review_rate,
    COUNT(*) FILTER (WHERE retry_count > 0) * 100.0 / NULLIF(COUNT(*), 0) AS any_retry_rate
FROM ses.extraction_queue
WHERE updated_at >= NOW() - INTERVAL '24 hours';

-- LLM Cost Tracking（過去30日間）
SELECT
    DATE_TRUNC('day', completed_at) AS date,
    COUNT(*) FILTER (WHERE final_method = 'llm_completed') AS llm_calls,
    COUNT(*) FILTER (WHERE final_method = 'llm_completed') * 0.002 AS estimated_cost_usd  -- 1回あたり約$0.002
FROM ses.extraction_queue
WHERE completed_at >= NOW() - INTERVAL '30 days'
GROUP BY date
ORDER BY date;
```

**Alert条件**:
- **Error Rate > 10%**: LLM API が不安定（Slack通知）
- **p90 Latency > 10秒**: LLM API が遅延（要調査）
- **LLM呼び出し率 > 70%**: ルールベースの説明可能な処理が少ない（Phase 2 要見直しの兆候）

---

## Phase 3: LLM統合とマッチングパイプライン接続

### 3.1 実装ロードマップ

| Step | 内容 | 優先度 |
|------|------|--------|
| **Step 1** | LLM Provider抽象化 + Shadow比較(10%) | P0 |
| **Step 2** | match_results DB保存の本番接続 | P0 |
| **Step 3** | systemd本番ループ (extractor/worker/recovery) | P0 |
| **Step 4** | sr-gmail-ingestor (n8n置換) | P1 (後日) |

---

### 3.2 LLM Provider 設計

#### 設計方針

- **Provider trait抽象化**: LLMは外部サービスとして扱い、差し替え可能に
- **環境変数ベース設定**: provider/model/endpoint/key/timeout/retryを環境変数で制御
- **Shadow比較モード**: 本番挙動を変えずに複数LLMの結果を比較保存

#### ディレクトリ構成

```
sr-llm-worker/src/
├── main.rs
└── llm/
    ├── mod.rs          # trait定義 + factory
    ├── types.rs        # 共通 Request/Response
    ├── config.rs       # 環境変数からの設定読み込み
    ├── validator.rs    # LLMレスポンス検証
    └── providers/
        ├── mod.rs
        ├── deepseek.rs
        ├── openai.rs
        ├── anthropic.rs
        ├── google.rs
        └── mock.rs     # テスト用
```

#### Provider Trait

```rust
use async_trait::async_trait;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// プロバイダ名 (ログ/メトリクス用)
    fn name(&self) -> &'static str;

    /// 抽出リクエストを実行
    async fn extract(&self, req: &LlmRequest) -> Result<LlmResponse, LlmError>;

    /// ヘルスチェック (optional)
    async fn health_check(&self) -> Result<(), LlmError> {
        Ok(())
    }
}
```

#### LlmRequest / LlmResponse

```rust
/// LLM抽出リクエスト
#[derive(Debug, Clone, Serialize)]
pub struct LlmRequest {
    pub message_id: String,
    pub email_subject: String,
    pub email_body: String,
    pub extraction_hints: Option<ExtractionHints>,
}

/// LLM抽出レスポンス
#[derive(Debug, Clone, Deserialize)]
pub struct LlmResponse {
    pub project_name: Option<String>,
    pub monthly_tanka_min: Option<u32>,
    pub monthly_tanka_max: Option<u32>,
    pub required_skills_keywords: Vec<String>,
    pub preferred_skills_keywords: Vec<String>,
    pub work_todofuken: Option<String>,
    pub work_area: Option<String>,
    pub remote_onsite: Option<String>,
    pub min_experience_years: Option<i32>,
    pub japanese_skill: Option<String>,
    pub english_skill: Option<String>,
    pub start_date_raw: Option<String>,
    pub contract_type: Option<String>,
    pub flow_dept: Option<String>,
    // ... その他抽出フィールド
}

/// LLMエラー分類
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("transient error: {message}")]
    Transient { message: String },

    #[error("permanent error: {message}")]
    Permanent { message: String },

    #[error("timeout after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("invalid response: {message}")]
    InvalidResponse { message: String },
}
```

#### 環境変数設定

**MVP最小構成**:
```bash
# 基本設定
LLM_ENABLED=1                    # 0でLLM無効化（テスト/デバッグ用）
LLM_PROVIDER=deepseek            # deepseek|openai|anthropic|google|mock
LLM_MODEL=deepseek-chat          # provider依存
LLM_ENDPOINT=https://api.deepseek.com/v1/chat/completions
LLM_API_KEY=sk-xxx
LLM_TIMEOUT_SECONDS=30
LLM_MAX_RETRIES=3
LLM_RETRY_BACKOFF_SECONDS=5
```

**Shadow比較モード** (推奨):
```bash
LLM_COMPARE_MODE=shadow          # none|shadow|ab
LLM_PRIMARY_PROVIDER=deepseek    # 本番で使用するprovider
LLM_SHADOW_PROVIDER=openai       # 比較用provider
LLM_SHADOW_SAMPLE_PERCENT=10     # 10%のリクエストで比較
LLM_SHADOW_API_KEY=sk-xxx        # shadow provider用キー
```

**A/Bテストモード** (将来用):
```bash
LLM_COMPARE_MODE=ab
LLM_PROVIDERS=deepseek,openai
LLM_AB_PERCENT=50                # deepseek側の割合
```

#### LLMレスポンス検証

**重要**: LLMのレスポンスは「必ず検証」してから使用する。

```rust
pub fn validate_llm_response(resp: &LlmResponse) -> Result<(), ValidationError> {
    let mut errors = Vec::new();

    // 単価範囲チェック (20万〜300万)
    if let Some(min) = resp.monthly_tanka_min {
        if min < 20 || min > 300 {
            errors.push(format!("monthly_tanka_min out of range: {}", min));
        }
    }
    if let Some(max) = resp.monthly_tanka_max {
        if max < 20 || max > 300 {
            errors.push(format!("monthly_tanka_max out of range: {}", max));
        }
    }

    // 経験年数チェック (0〜50年)
    if let Some(years) = resp.min_experience_years {
        if years < 0 || years > 50 {
            errors.push(format!("min_experience_years out of range: {}", years));
        }
    }

    // スキル空配列チェック
    if resp.required_skills_keywords.is_empty() {
        errors.push("required_skills_keywords is empty".to_string());
    }

    // ENUM値チェック
    if let Some(ref remote) = resp.remote_onsite {
        let valid = ["フル出社", "リモート併用", "フルリモート"];
        if !valid.contains(&remote.as_str()) {
            errors.push(format!("invalid remote_onsite: {}", remote));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationError { errors })
    }
}
```

**検証失敗時の処理**:
- JSON parseエラー → `Retryable`
- 必須フィールド欠損 → `partial_fields`に保存 + `manual_review`
- 値域エラー → `partial_fields`に保存 + `manual_review`

---

### 3.3 Shadow比較の実装

#### 比較結果の保存

```sql
CREATE TABLE ses.llm_comparison_results (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL,
    primary_provider VARCHAR(50) NOT NULL,
    shadow_provider VARCHAR(50) NOT NULL,
    primary_response JSONB NOT NULL,
    shadow_response JSONB,
    primary_latency_ms INTEGER,
    shadow_latency_ms INTEGER,
    diff_summary JSONB,  -- 差分サマリ
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_llm_comparison_message ON ses.llm_comparison_results(message_id);
CREATE INDEX idx_llm_comparison_created ON ses.llm_comparison_results(created_at);
```

#### 比較ロジック

```rust
pub async fn run_with_shadow_comparison(
    primary: &dyn LlmProvider,
    shadow: &dyn LlmProvider,
    req: &LlmRequest,
    sample_percent: u8,
) -> Result<LlmResponse, LlmError> {
    // 本番リクエスト (必須)
    let primary_result = primary.extract(req).await;

    // Shadow比較 (サンプリング)
    let should_compare = rand::random::<u8>() < (sample_percent * 255 / 100);
    if should_compare {
        tokio::spawn({
            let shadow = shadow.clone();
            let req = req.clone();
            async move {
                let shadow_result = shadow.extract(&req).await;
                // 比較結果をDBに保存 (非同期、本番には影響しない)
                save_comparison_result(&req.message_id, &primary_result, &shadow_result).await;
            }
        });
    }

    primary_result
}
```

---

### 3.4 マッチングパイプライン接続

#### 処理フロー

```
sr-extractor
    │
    ▼
ses.extraction_queue (status=pending)
    │
    ▼
sr-llm-worker
    ├── lock_next_pending_job()
    ├── LLM Provider.extract()
    ├── validate_llm_response()
    ├── apply corrections (normalize_*)
    ├── upsert_extraction_job() → completed
    │
    ▼
run_all_ko_checks()
    │
    ▼
calculate_detailed_score()
    │
    ▼
ses.match_results (INSERT)
```

#### match_results 保存内容

```rust
pub struct MatchResultInsert {
    pub talent_id: i64,
    pub project_id: i64,
    pub is_knockout: bool,
    pub ko_reasons: Option<serde_json::Value>,  // Vec<String> as JSONB
    pub needs_manual_review: bool,
    pub score_total: Option<f64>,
    pub score_breakdown: Option<serde_json::Value>,
    pub engine_version: Option<String>,  // "1.0.0"
    pub rule_version: Option<String>,    // "2025-01-15"
}
```

---

### 3.5 systemd 本番デプロイ

#### サービス構成

| サービス | 実行方式 | 説明 |
|----------|----------|------|
| `sr-extractor.timer` | 5分間隔 | anken_emails → extraction_queue |
| `sr-llm-worker.service` | 常駐 | queue処理 + LLM呼び出し |
| `sr-queue-recovery.timer` | 10分間隔 | 滞留ジョブ復旧 |

#### 環境変数ファイル

`/etc/sr-matcher.env`:
```bash
DATABASE_URL=postgres://user:pass@host:5432/sponto
LLM_ENABLED=1
LLM_PROVIDER=deepseek
LLM_MODEL=deepseek-chat
LLM_API_KEY=sk-xxx
LLM_COMPARE_MODE=shadow
LLM_SHADOW_PROVIDER=openai
LLM_SHADOW_SAMPLE_PERCENT=10
LLM_SHADOW_API_KEY=sk-xxx
```

---

### 3.6 Done条件

#### Step 1: LLM Provider抽象化

- [ ] `LlmProvider` trait が定義されている
- [ ] `DeepSeekProvider` が実装されている
- [ ] `MockProvider` がテスト用に実装されている
- [ ] 環境変数から設定を読み込む `LlmConfig` がある
- [ ] `validate_llm_response()` でレスポンス検証が実装されている
- [ ] Shadow比較モードが動作する

#### Step 2: match_results DB保存

- [ ] `sr-llm-worker` が実際のLLM APIを呼び出す
- [ ] 抽出結果が正規化されてDB保存される
- [ ] `run_all_ko_checks()` が本番パスで実行される
- [ ] `match_results` にスコア・KO理由が保存される

#### Step 3: systemd本番ループ

- [ ] 3サービスがsystemdで起動する
- [ ] ログが `/var/log/sr-matcher/` に出力される
- [ ] エラー時にSlack通知が飛ぶ

#### Step 4: sr-gmail-ingestor (将来)

- [ ] Gmail API認証が動作する
- [ ] 差分取得 (historyId / watch) が実装されている
- [ ] n8n経由を完全に廃止できる

---

**END OF DOCUMENT**
