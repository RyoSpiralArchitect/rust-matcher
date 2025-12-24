# Two-Tower + 営業FB アルゴリズム概観

> **目的**: 営業フィードバックを学習信号として活用し、マッチング精度を継続的に改善する

---

## 1. なぜ Two-Tower か

### 現状の課題

```
現在のマッチング
┌─────────────────────────────────────────────────┐
│  ビジネスルール（KO判定 + スコアリング）          │
│  ├─ 単価チェック ✅                              │
│  ├─ スキルマッチ ✅                              │
│  ├─ 勤務地マッチ ✅                              │
│  └─ ...                                         │
│                                                 │
│  問題: ルールで表現しにくい「良いマッチ」がある   │
│  例: "この人、スペック的にはOKだけど相性が..."   │
└─────────────────────────────────────────────────┘
```

### Two-Tower が解決すること

1. **暗黙知の学習**: 営業が「良い」と判断したペアから、言語化しにくいパターンを学ぶ
2. **継続的改善**: フィードバックが溜まるほど賢くなる
3. **ルールとの共存**: HardKO は絶対（Two-Tower で覆らない）、順位づけだけを改善

---

## 2. Two-Tower アーキテクチャ

### 基本構造

```
Talent 情報                           Project 情報
    │                                      │
    ▼                                      ▼
┌──────────────┐                    ┌──────────────┐
│ Talent Tower │                    │ Project Tower│
│  (Encoder)   │                    │   (Encoder)  │
└──────┬───────┘                    └──────┬───────┘
       │                                   │
       ▼                                   ▼
   Talent Embedding                 Project Embedding
   (D次元ベクトル)                   (D次元ベクトル)
   ※ D = TwoTowerConfig.dimension（デフォルト256）
       │                                   │
       └───────────────┬───────────────────┘
                       ▼
               Cosine Similarity
                       │
                       ▼
              Two-Tower Score (0.0〜1.0)
```

### なぜ "Two-Tower" か

- **独立したエンコーダ**: Talent と Project を別々にベクトル化
- **オフライン計算可能**: 新しい Project が来たら、既存 Talent の埋め込みと比較するだけ
- **スケーラブル**: O(N×M) の全ペア計算を回避できる（ANN検索と組み合わせ可能）

---

## 3. 営業フィードバックとの統合

### データフロー

```
┌─────────────────────────────────────────────────────────────────┐
│                        運用フェーズ                              │
│                                                                 │
│  マッチング実行                                                  │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────┐    表示    ┌─────────────────┐             │
│  │ interaction_logs│ ────────► │      GUI        │             │
│  │ (露出ログ)      │           │  候補一覧表示    │             │
│  └─────────────────┘           └────────┬────────┘             │
│                                         │                       │
│                                         ▼ 営業がフィードバック   │
│                                ┌─────────────────┐              │
│                                │ feedback_events │              │
│                                │ (ラベル)        │              │
│                                └────────┬────────┘              │
│                                         │                       │
└─────────────────────────────────────────┼───────────────────────┘
                                          │
┌─────────────────────────────────────────┼───────────────────────┐
│                        学習フェーズ      │                       │
│                                         ▼                       │
│                                ┌─────────────────┐              │
│                                │ training_pairs  │              │
│                                │ (VIEW)          │              │
│                                └────────┬────────┘              │
│                                         │                       │
│                                         ▼                       │
│                                ┌─────────────────┐              │
│                                │  Two-Tower 学習 │              │
│                                │  (PyTorch等)    │              │
│                                └────────┬────────┘              │
│                                         │                       │
│                                         ▼                       │
│                                ┌─────────────────┐              │
│                                │ 学習済みモデル   │              │
│                                │ (.onnx)         │              │
│                                └─────────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

### フィードバック → ラベル変換

| feedback_type | ソース | label | 意味 |
|---------------|--------|-------|------|
| `thumbs_up` | GUI | 1.0 | 良いマッチ |
| `thumbs_down` | GUI | 0.0 | 悪いマッチ |
| `review_ok` | GUI | 1.0 | レビューOK |
| `review_ng` | GUI | 0.0 | レビューNG |
| `accepted` | 営業 | 1.0 | 成約 |
| `rejected` | 営業 | 0.0 | 不成立 |
| `interview_scheduled` | 営業 | 0.8 | 面談設定（ほぼ良い） |
| `no_response` | - | 除外 | 学習データから除外 |

### データフロー詳細：feedback → outcome → training

```
feedback_events INSERT          interaction_logs UPDATE
      ↓                              ↓
┌──────────────────┐           ┌──────────────────┐
│ feedback_events  │  ───────► │ interaction_logs │
│ (監査ログ)        │   trigger │ .outcome 更新     │
│ - 何が起きたか    │   or API  │ - 確定ラベル      │
└──────────────────┘           └──────────────────┘
                                      ↓
                               training_pairs VIEW
                               (学習用に参照)
```

**設計方針**:
- `feedback_events` = **監査ログ**（何が起きたか、revoked含む）
- `interaction_logs.outcome` = **現時点の確定ラベル**（最新非revokedのみ反映）
- `training_pairs` = `outcome` を見る（JOIN減って速い・安定）

### outcome の許容値と優先順位

#### 許容値（実質 enum）

| outcome | ソース | 優先度 | 学習時 label |
|---------|--------|--------|--------------|
| `accepted` | 営業 | 1（最強） | 1.0 |
| `rejected` | 営業 | 2 | 0.0 |
| `interview_scheduled` | 営業 | 3 | 0.8 |
| `review_ok` | GUI | 4 | 1.0 |
| `review_ng` | GUI | 4 | 0.0 |
| `thumbs_up` | GUI | 5 | 1.0 |
| `thumbs_down` | GUI | 5 | 0.0 |
| `no_response` | - | 6（最弱） | 除外 |
| `NULL` | 初期値 | - | 除外 |

> **📌 pending は文字列ではなく `outcome = NULL` で表現する**
> 文字列 "pending" は使わない。VIEWの条件がシンプルになり、事故を防げる。

#### 優先順位ルール

```
┌─────────────────────────────────────────────────────────────┐
│  方針:                                                      │
│  1. 営業起因のラベルが GUI より強い                          │
│  2. 同じ優先度なら「最新のイベントが勝つ」（created_at）      │
│                                                             │
│  accepted > rejected > interview_scheduled                  │
│          > review_ok/ng > thumbs_up/down                    │
│          > no_response > NULL                               │
│                                                             │
│  例:                                                        │
│  1. thumbs_up → accepted: outcome = accepted（上書き）      │
│  2. review_ng → interview_scheduled: outcome = interview    │
│  3. accepted → thumbs_down: outcome = accepted（維持）      │
│  4. thumbs_up → thumbs_down: outcome = thumbs_down（同格→最新）│
│  5. review_ok → review_ng: outcome = review_ng（同格→最新） │
└─────────────────────────────────────────────────────────────┘
```

#### revoked の扱い

**実装方針**: revoke は「元行を UPDATE」で表現（append-only ではない）

```
┌─────────────────────────────────────────────────────────────┐
│  feedback_events は基本 append-only（INSERT）               │
│                                                             │
│  ただし revoke だけは例外:                                   │
│  - 元の feedback_events 行を UPDATE                         │
│  - is_revoked = true, revoked_at = NOW(), revoked_by = ... │
│  - 新しい行は INSERT しない                                  │
│                                                             │
│  理由:                                                      │
│  - 「誰がいつ取り消したか」が元行でわかる（監査）            │
│  - interaction_logs.outcome は再計算で更新                  │
└─────────────────────────────────────────────────────────────┘
```

例:
```
t=1: INSERT thumbs_up    → outcome = thumbs_up
t=2: INSERT review_ng    → outcome = review_ng (優先度高い)
t=3: UPDATE t=2 行に is_revoked=true → outcome = thumbs_up (t=1 に戻る)
t=4: INSERT thumbs_down  → outcome = thumbs_down (t=1と同格→最新が勝つ)
```

#### 実装方針

```rust
// feedback_events 変更時のトリガー or API ロジック
fn on_feedback_change(interaction_id: i64) {
    // 常に「正を求めるSQL」で再計算（シンプル・堅牢）
    let correct_outcome = compute_correct_outcome(interaction_id);
    update_interaction_logs(interaction_id, correct_outcome);
}

/// 優先順位 + created_at で勝者を決める
fn should_override(current: Option<&OutcomeState>, new: &OutcomeState) -> bool {
    match current {
        None => true,
        Some(cur) => {
            let p_new = priority(&new.outcome);
            let p_cur = priority(&cur.outcome);
            // 優先度が高い、または同格なら最新が勝つ
            p_new < p_cur || (p_new == p_cur && new.created_at > cur.created_at)
        }
    }
}
```

#### 正を求める SQL（復旧・検証用）

> **📌 バグが起きてもこの SQL で outcome を再計算できる（運用の保険）**

```sql
-- interaction_id に対する「正しい outcome」を1件返す
SELECT feedback_type
FROM ses.feedback_events
WHERE interaction_id = $1
  AND is_revoked = false
ORDER BY
  CASE feedback_type
    WHEN 'accepted' THEN 1
    WHEN 'rejected' THEN 2
    WHEN 'interview_scheduled' THEN 3
    WHEN 'review_ok' THEN 4
    WHEN 'review_ng' THEN 4
    WHEN 'thumbs_up' THEN 5
    WHEN 'thumbs_down' THEN 5
    WHEN 'no_response' THEN 6
    ELSE 100
  END ASC,
  created_at DESC  -- 同格は最新が勝つ
LIMIT 1;

-- この結果を interaction_logs.outcome に書き戻す
-- NULL が返れば outcome = NULL（初期状態に戻る）
```

### training_pairs ビュー

> **📌 正は `schema.rs` の `INTERACTION_LOGS_DDL` 内の VIEW 定義**

```sql
-- interaction_logs.outcome を直接参照（feedback_events JOIN 不要）
CREATE OR REPLACE VIEW ses.training_pairs AS
SELECT
    il.talent_id,
    il.project_id,
    il.two_tower_score,
    il.two_tower_embedder,
    il.two_tower_version,
    il.business_score,
    il.outcome,
    il.variant,  -- A/Bテスト識別
    CASE
        WHEN il.outcome = 'accepted' THEN 1.0
        WHEN il.outcome = 'rejected' THEN 0.0
        WHEN il.outcome = 'thumbs_up' THEN 1.0
        WHEN il.outcome = 'thumbs_down' THEN 0.0
        WHEN il.outcome = 'review_ok' THEN 1.0
        WHEN il.outcome = 'review_ng' THEN 0.0
        WHEN il.outcome = 'interview_scheduled' THEN 0.8
        ELSE NULL
    END AS label,
    il.run_date,   -- JST基準
    il.created_at
FROM ses.interaction_logs il
WHERE il.outcome IS NOT NULL
  AND il.outcome <> 'no_response';
-- ※ 'pending' は NULL で表現するため、outcome IS NOT NULL で除外済み
```

---

## 3.5 3層シグナル設計（行動ログ・FB・CV）

### なぜ3層か

```
┌─────────────────────────────────────────────────────────────┐
│  問題:                                                      │
│  1. 営業が「いける！」と思って即連絡→FBなしで良いマッチ見逃す │
│  2. 実際のCV（面談化/成約）がスコア改善に反映されない        │
│                                                             │
│  解決:                                                      │
│  (A) 行動ログ: FBなしでも残る「良い兆候」                    │
│  (B) CVログ: 面談化/成約を必ず反映                          │
│  (C) 学習ラベル: 優先順位で統合（強い証拠が勝つ）            │
└─────────────────────────────────────────────────────────────┘
```

### レイヤー構成

```
┌──────────────────────────────────────────────────────────────┐
│ 強                                                   弱     │
│ ◄────────────────────────────────────────────────────────►  │
│                                                              │
│ [CV]                  [FB]                    [行動]         │
│ contract_signed 1.0   accepted 1.0            shortlisted 0.3│
│ offer           0.9   thumbs_up 1.0           clicked     0.3│
│ interview       0.8   review_ok 1.0           copied      0.2│
│ entry           0.7   interview_sched 0.8     viewed      0.1│
│ contacted       0.4   rejected 0.0                           │
│ lost            0.0   thumbs_down 0.0                        │
│                       review_ng 0.0                          │
│                                                              │
│ NULL = 未知（学習には使わない、負例扱いしない）               │
│                                                              │
│ 📌 accepted = 1.0 (training_pairs と同一スケール)            │
│    → CVがないなら accepted が最終CV級の意味を持つ            │
└──────────────────────────────────────────────────────────────┘
```

### 行動ログ: interaction_events

> **📌 正は `schema.rs` の `INTERACTION_EVENTS_DDL`**

```sql
CREATE TABLE ses.interaction_events (
    id BIGSERIAL PRIMARY KEY,
    interaction_id BIGINT NOT NULL REFERENCES ses.interaction_logs(id),

    -- Phase 1 event_type:
    -- viewed_candidate_detail, copied_template, clicked_contact, shortlisted
    event_type TEXT NOT NULL,

    actor TEXT NOT NULL,          -- JWTならsub
    source TEXT NOT NULL DEFAULT 'gui',
    idempotency_key TEXT NOT NULL UNIQUE,  -- グローバル冪等性
    meta JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- shortlisted は 1回だけ（トグルなら meta.active で状態管理）
CREATE UNIQUE INDEX uniq_interaction_shortlist_once
    ON ses.interaction_events(interaction_id, actor)
    WHERE event_type = 'shortlisted';
```

**ユニーク戦略**:
| event_type | 回数 | 制約 |
|------------|------|------|
| `shortlisted` | 1回だけ | partial unique index |
| `viewed_detail` | 複数回OK | 閲覧回数に価値がある |
| `clicked_contact` | 複数回OK | 再連絡など |
| `copied_template` | 複数回OK | 複数回コピーOK |

**GUI導線**:
- 「詳細」クリック → `viewed_candidate_detail`
- 「コピー」→ `copied_template`
- 「連絡する」→ `clicked_contact`（その後 gmail compose を開く）
- ⭐ → `shortlisted`

**ポイント**:
- イベント送信は失敗してもUXを止めない（fire-and-forget）
- `idempotency_key` で再送時の重複を防止

### CVログ: conversion_events

> **📌 正は `schema.rs` の `CONVERSION_EVENTS_DDL`**

```sql
CREATE TABLE ses.conversion_events (
    id BIGSERIAL PRIMARY KEY,
    interaction_id BIGINT REFERENCES ses.interaction_logs(id),
    talent_id BIGINT NOT NULL,
    project_id BIGINT NOT NULL,

    -- stage: contacted → entry → interview_scheduled → offer → contract_signed
    -- 離脱: lost
    stage TEXT NOT NULL,

    actor TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'gui',  -- gui / crm / import
    meta JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**GUI導線**:
- 候補詳細に「ステージ更新」ドロップダウン
- 最初は「面談化」だけでも十分価値が出る

**CRM連携（Phase 2）**:
- GUIで手入力 → 後からCRMで自動流入

### 統合学習ラベル: training_labels VIEW

> **📌 正は `schema.rs` の `INTERACTION_LOGS_DDL` 内の VIEW 定義**
>
> `training_pairs` は後方互換のため残す。新規学習はこちらを推奨。

**設計原則**:

| 項目 | 決定 | 理由 |
|------|------|------|
| **best_stage 採用** | ✅ | 外部要因で `lost` になっても "マッチング品質" まで否定しない |
| **FBラベル = training_pairs と同一** | ✅ | accepted = 1.0（CVがないなら最終CV級）|
| **Phase 1** | interaction_id ありのCVのみ学習 | 安全に始める |
| **Phase 2** | CRM import → mapping 作成 | interaction_id への紐付け |

> **📌 final_stage（最後のステージ）が欲しい分析は別VIEWで作る:**
> ```sql
> ORDER BY interaction_id, created_at DESC
> ```

```sql
CREATE OR REPLACE VIEW ses.training_labels AS
WITH cv_best AS (
    -- interaction_id ごとに最強のCVステージを取得（best_stage方式）
    SELECT DISTINCT ON (interaction_id)
        interaction_id, stage AS cv_stage,
        CASE stage
            WHEN 'contract_signed'     THEN 1.0
            WHEN 'offer'               THEN 0.9
            WHEN 'interview_scheduled' THEN 0.8
            WHEN 'entry'               THEN 0.7
            WHEN 'contacted'           THEN 0.4
            WHEN 'lost'                THEN 0.0
            ELSE NULL
        END AS cv_label
    FROM ses.conversion_events
    WHERE interaction_id IS NOT NULL  -- Phase 1: interaction_id ありのみ
    ORDER BY interaction_id,
        CASE stage
            WHEN 'contract_signed'     THEN 1
            WHEN 'offer'               THEN 2
            WHEN 'interview_scheduled' THEN 3
            WHEN 'entry'               THEN 4
            WHEN 'contacted'           THEN 5
            WHEN 'lost'                THEN 6
            ELSE 999
        END ASC,
        created_at DESC
),
behavior_best AS (
    -- 最強の行動イベントを取得
    SELECT DISTINCT ON (interaction_id)
        interaction_id, event_type AS behavior_type,
        CASE event_type
            WHEN 'shortlisted'             THEN 0.3
            WHEN 'clicked_contact'         THEN 0.3
            WHEN 'copied_template'         THEN 0.2
            WHEN 'viewed_candidate_detail' THEN 0.1
            ELSE NULL
        END AS behavior_label
    FROM ses.interaction_events
    ORDER BY interaction_id,
        CASE event_type
            WHEN 'shortlisted'             THEN 1
            WHEN 'clicked_contact'         THEN 2
            WHEN 'copied_template'         THEN 3
            WHEN 'viewed_candidate_detail' THEN 4
            ELSE 999
        END ASC,
        created_at DESC
),
fb AS (
    -- FBラベル: training_pairs と同じスケール（accepted = 1.0）
    SELECT
        id AS interaction_id,
        CASE
            WHEN outcome = 'accepted'            THEN 1.0
            WHEN outcome = 'rejected'            THEN 0.0
            WHEN outcome = 'thumbs_up'           THEN 1.0
            WHEN outcome = 'thumbs_down'         THEN 0.0
            WHEN outcome = 'review_ok'           THEN 1.0
            WHEN outcome = 'review_ng'           THEN 0.0
            WHEN outcome = 'interview_scheduled' THEN 0.8
            ELSE NULL
        END AS fb_label
    FROM ses.interaction_logs
    WHERE outcome IS NOT NULL
      AND outcome <> 'no_response'
)
SELECT
    il.id AS interaction_id, il.talent_id, il.project_id,
    il.two_tower_score, il.business_score, il.variant, il.run_date,

    -- デバッグ用
    cv.cv_stage, il.outcome AS fb_outcome, bb.behavior_type,

    -- signal_source: どのソースからラベルが来たか
    CASE
        WHEN cv.cv_label IS NOT NULL THEN 'conversion'
        WHEN fb.fb_label IS NOT NULL THEN 'feedback'
        WHEN bb.behavior_label IS NOT NULL THEN 'behavior'
        ELSE NULL
    END AS signal_source,

    -- label: 優先順位で統合（CV > FB > 行動ログ）
    COALESCE(cv.cv_label, fb.fb_label, bb.behavior_label) AS label

FROM ses.interaction_logs il
LEFT JOIN cv_best cv       ON cv.interaction_id = il.id
LEFT JOIN fb               ON fb.interaction_id = il.id
LEFT JOIN behavior_best bb ON bb.interaction_id = il.id
WHERE cv.cv_label IS NOT NULL
   OR fb.fb_label IS NOT NULL
   OR bb.behavior_label IS NOT NULL;
```

### API設計

```
POST /api/interactions/:interaction_id/events

Request:
{
  "event_type": "clicked_contact",
  "idempotency_key": "8f6c6c1a-....",
  "meta": { "method": "gmail_compose" }
}

Response:
{ "id": 123, "status": "created" }
or
{ "id": null, "status": "already_exists" }
```

- **actor**: サーバが AuthUser から決定（リクエストに含めない）
- **source**: 当面 `"gui"` 固定

---

## 4. 実装の3段階

### Phase 3-A: HashTwoTower（決定論的、学習不要）

```
目的: 骨格を仕込む。weight=0.0 で無効化しておく

┌─────────────────────────────────────────────┐
│  HashTwoTower                               │
│                                             │
│  特徴量抽出                                  │
│  ├─ skills: ["Java", "Python", "AWS"]       │
│  ├─ tanka: 800000                           │
│  └─ location: "東京都"                       │
│           │                                 │
│           ▼                                 │
│  Feature Hashing (SipHasher13)              │
│  "skill:java" → hash % D → index 23         │
│  "skill:python" → hash % D → index 7        │
│           │                                 │
│           ▼                                 │
│  D次元スパースベクトル（D=256）               │
│  [0, 0, ..., 1, 0, ..., 1, 0, ...]          │
│           │                                 │
│           ▼                                 │
│  L2正規化 → 単位ベクトル                     │
└─────────────────────────────────────────────┘

利点:
- 学習不要、決定論的
- 同じ入力 → 同じ出力（デバッグしやすい）
- ベースライン指標として使える
```

### Phase 3-B: interaction_logs への記録

```
目的: Two-Tower スコアをログに残す

INSERT INTO ses.interaction_logs (
    talent_id,
    project_id,
    match_run_id,
    two_tower_score,      -- ← HashTwoTower の出力
    two_tower_embedder,   -- ← "hash"
    two_tower_version,    -- ← "v2"
    business_score,
    ...
)
```

### Phase 3-C: Embedder切替インターフェース固定

```
目的: hash / onnx / candle を環境変数で切り替え可能にする

┌─────────────────────────────────────────────┐
│  create_embedder(name, config)              │
│                                             │
│  "hash"   → HashTwoTower                    │
│  "onnx"   → OnnxTwoTower (stub → 本番)      │
│  "candle" → CandleTwoTower (stub → 本番)    │
└─────────────────────────────────────────────┘

環境変数:
  TWO_TOWER_EMBEDDER=hash|onnx|candle
  TWO_TOWER_DIMENSION=256
  TWO_TOWER_WEIGHT=0.0
  TWO_TOWER_ENABLED=false
```

**Done条件**:
- OnnxTwoTower / CandleTwoTower スタブが実装されている
- `create_embedder("onnx", ...)` がコンパイル通る
- 環境変数で切り替えできる

### Phase 4: OnnxTwoTower（学習済みモデル）

```
目的: training_pairs から学習したモデルで推論

┌─────────────────────────────────────────────┐
│  学習パイプライン (Python)                   │
│                                             │
│  1. training_pairs を PostgreSQL から取得   │
│  2. PyTorch で Two-Tower を学習             │
│     - Talent Encoder: MLP or Transformer    │
│     - Project Encoder: MLP or Transformer   │
│     - Loss: Contrastive or BCE              │
│  3. ONNX にエクスポート                      │
└─────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────┐
│  推論 (Rust)                                │
│                                             │
│  OnnxTwoTower                               │
│  ├─ ort (ONNX Runtime) でモデル読み込み     │
│  ├─ Talent/Project を forward              │
│  └─ cosine similarity で Two-Tower Score   │
└─────────────────────────────────────────────┘
```

---

## 5. TwoTowerEmbedder Trait 設計

### 設計方針

1. **実装交換可能**: Hash / ONNX / Candle を同じインターフェースで
2. **Send + Sync**: マルチスレッド環境で共有可能
3. **バージョン追跡**: どの実装・バージョンで計算したかを記録

### Trait 定義

```rust
/// Two-Tower 埋め込みの抽象インターフェース
///
/// interaction_logs には name() と version() の値が記録される。
/// これにより「どの実装・バージョンで計算したか」を追跡可能。
pub trait TwoTowerEmbedder: Send + Sync {
    /// 実装名（"hash", "onnx", "candle"）
    /// → interaction_logs.two_tower_embedder
    fn name(&self) -> &'static str;

    /// バージョン情報（モデルの世代管理用）
    /// → interaction_logs.two_tower_version
    fn version(&self) -> &str;

    /// 埋め込み次元数
    fn dimension(&self) -> usize;

    /// Project を埋め込みベクトルに変換
    fn embed_project(&self, project: &Project) -> Embedding;

    /// Talent を埋め込みベクトルに変換
    fn embed_talent(&self, talent: &Talent) -> Embedding;

    /// 2つの埋め込みの類似度を計算（デフォルト: cosine similarity）
    fn similarity(&self, a: &Embedding, b: &Embedding) -> f32 {
        cosine_similarity(&a.vector, &b.vector)
    }

    /// 複数の Talent を一括で埋め込み（デフォルト実装: ループ）
    fn embed_talents(&self, talents: &[Talent]) -> Vec<Embedding> {
        talents.iter().map(|t| self.embed_talent(t)).collect()
    }
}

/// 埋め込みベクトル
/// 📌 正は crates/sr-common/src/two_tower/embedding.rs
#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,           // D次元ベクトル
    pub source: EmbeddingSource,    // 出自
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 埋め込みの出自
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingSource {
    Talent,
    Project,
}
```

### 実装一覧

| 実装 | 用途 | 依存 | Phase |
|------|------|------|-------|
| `HashTwoTower` | ベースライン、学習不要 | なし | 3-A |
| `OnnxTwoTower` | 学習済みモデル推論 | ort | 4 |
| `CandleTwoTower` | Rust-native推論 | candle | 4+ |

---

## 6. スコアリングへの統合

### 現在のスコア計算

```rust
total_score = business_score
            × semantic_score    // 将来: 意味的類似度
            × historical_score  // 将来: 過去実績
```

### Two-Tower 統合後

```rust
total_score = (
    base_score                              // 既存の乗算スコア
    + two_tower_weight × two_tower_score    // Two-Tower 加算
) / (1.0 + two_tower_weight)                // 正規化

// where
base_score = business_score × semantic_score × historical_score
```

### なぜ Two-Tower は加算なのか

既存スコア要素は**乗算**だが、Two-Tower は**加算**で統合する理由：

| 比較 | 乗算（既存） | 加算（Two-Tower） |
|------|--------------|-------------------|
| スケール感度 | 0付近で一発壊れる | 安定 |
| 校正ズレ | 致命的 | 許容 |
| 解釈 | 「全部満たすとOK」 | 「順位調整シグナル」 |
| 例 | KOで0、Passで1 | 0.3〜0.7 が主 |

```
Two-Tower は「順位を微調整するシグナル」なので:
- 0.0 に近くても base_score が生きる
- 校正が多少ズレても順位が大崩れしない
- w=0 なら完全に従来通り（安全に導入）
```

### 正規化の意味

```
final = (base + w × tt) / (1 + w)
```

- `w = 0` → `final = base`（Two-Tower無効）
- `w = 1` → `final = (base + tt) / 2`（半々ブレンド）
- `w` を上げても `0.0〜1.0` を保つ（監視しやすい）

### 重み調整の段階

| Phase | two_tower_weight | 意味 |
|-------|------------------|------|
| 3-A | 0.0 | 無効（ログだけ） |
| 4-初期 | 0.1 | 控えめに導入 |
| 4-安定 | 0.2〜0.3 | 効果確認後に上げる |
| 4-成熟 | 0.3〜0.5 | データ蓄積後 |

---

## 7. 不変条件（絶対に守ること）

```
┌─────────────────────────────────────────────────────────────┐
│  1. HardKO は Two-Tower で覆らない                          │
│     Two-Tower スコアが 1.0 でも、単価NGなら候補に出さない    │
│                                                             │
│  2. Two-Tower は順位づけ、KO判定ではない                     │
│     Pass した候補の中で「どれを上に出すか」を決める          │
│                                                             │
│  3. 学習データは interaction_logs 経由                       │
│     GUI に表示されたペアだけが学習対象                       │
│     （表示されなかったペアは学習しない）                     │
│                                                             │
│  4. フィードバックなしは学習しない                           │
│     no_response は除外。確実なラベルだけ使う                 │
│                                                             │
│  5. match_run_id で再現性を担保                              │
│     「この結果はどのモデル・設定で出したか」を追跡可能に     │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. 実装ロードマップ

```
現在地                                                     将来
   │                                                          │
   ▼                                                          ▼
Phase 3-A ──► Phase 3-B ──► Phase 3-C ─────────────────► Phase 4
   │              │              │                            │
   │              │              │                            │
   ├─ trait定義   ├─ ログ記録   ├─ embedder切替            ├─ 学習パイプライン
   ├─ Hash実装    ├─ training   │  インターフェース        ├─ ONNX本番化
   ├─ score統合   │  _pairs     ├─ 環境変数対応           ├─ weight調整
   └─ w=0.0      └─ VIEW       └─ stub完備                └─ A/Bテスト
```

### チェックリスト

**Phase 3-A（HashTwoTower基盤）**
- [ ] `TwoTowerEmbedder` trait 定義（name/version/dimension）
- [ ] `Embedding` / `WeightedToken` 構造体
- [ ] `HashTwoTower` 実装（共通トークン+重み方式）
- [ ] `TwoTowerConfig` 設定
- [ ] スコアリング統合（weight=0.0）
- [ ] 単体テスト（required > preferred）

**Phase 3-B（データ収集）**
- [ ] `interaction_logs` に two_tower_* 記録
- [ ] `training_pairs` VIEW 作成
- [ ] feedback → outcome 更新フロー

**Phase 3-C（切替インターフェース）**
- [ ] `OnnxTwoTower` スタブ実装
- [ ] `CandleTwoTower` スタブ実装
- [ ] `create_embedder()` ファクトリ
- [ ] 環境変数からの設定読み込み

**Phase 4（学習モデル導入）**
- [ ] 学習パイプライン（Python）
- [ ] OnnxTwoTower 本番化
- [ ] weight > 0 で効果測定
- [ ] A/Bテスト

---

## 9. FAQ

### Q. なぜ最初から学習モデルを使わないの？
**A.** 学習には「正解ラベル」が必要。フィードバックが溜まるまでは HashTwoTower でログだけ取る。

### Q. HashTwoTower でも精度出るの？
**A.** Feature Hashing は意外と強い。ベースラインとして十分使える。

### Q. 学習データはどれくらい必要？
**A.** 目安として 1,000〜10,000 ペア。最初は少なくてもいいが、多いほど良い。

### Q. Two-Tower と既存ルールが矛盾したら？
**A.** 既存ルール（特にHardKO）が優先。Two-Tower は「候補の中での順位」だけ。

### Q. モデル更新の頻度は？
**A.** 最初は月1回。データが溜まったら週1回。リアルタイム学習はしない。

---

## 10. 運用戦略

### 10.1 Cold Start（データ不足時）

```
┌─────────────────────────────────────────────────────────────┐
│  ラベル数に応じた段階的導入                                   │
│                                                             │
│  ラベル < 500        → weight = 0.0（従来ロジックのみ）      │
│  500 ≤ ラベル < 1000 → weight = 0.05（微弱シグナル）         │
│  1000 ≤ ラベル       → weight = 0.1〜0.3（本格運用）         │
└─────────────────────────────────────────────────────────────┘
```

**Phase 3 では**:
- HashTwoTower は学習不要なので、ラベル数に関係なく動作する
- ただし weight=0.0 でログだけ取り、効果測定に備える

**Phase 4 移行時**:
- `training_stats.labeled_count` で学習可能ラベル数を監視
- しきい値を超えたら学習モデルに切り替え

### 10.2 Negative Sampling

学習には positive/negative ペアが必要。

| 戦略 | 説明 | Phase |
|------|------|-------|
| Explicit negatives | `rejected` ラベル | 3〜4 |
| In-batch negatives | バッチ内の他ペアを負例に | 4 |
| Hard negatives | スコアが高いのに不成立 | 4+ |

**Phase 4 の最小形**:
```python
# in-batch negatives（実装が簡単で効果的）
for (talent, project, label) in batch:
    positive_sim = model(talent, project)
    negative_sims = [model(talent, other_project) for other_project in batch if other_project != project]
    loss = contrastive_loss(positive_sim, negative_sims, label)
```

**`no_response` の扱い**:
- 最初は除外（確実なラベルのみ）
- データが溜まったら弱い負例（weight=0.3）として追加検討

### 10.3 Embedding キャッシュ

| 実装 | キャッシュ | 理由 |
|------|-----------|------|
| HashTwoTower | 不要 | 高速（<1ms） |
| OnnxTwoTower | Talent のみ | 推論コスト高い |
| CandleTwoTower | Talent のみ | 同上 |

**キャッシュ設計（Phase 4）**:
```sql
CREATE TABLE ses.talent_embeddings (
    talent_id BIGINT PRIMARY KEY,
    embedder VARCHAR(50) NOT NULL,
    version VARCHAR(20) NOT NULL,
    embedding FLOAT8[] NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    FOREIGN KEY (talent_id) REFERENCES talents(id)
);

-- インデックス（embedder+version で絞り込み）
CREATE INDEX ON ses.talent_embeddings (embedder, version);
```

**更新トリガー**:
- Talent 更新時（profile変更）
- モデル更新時（version変更で全件再計算）
- 日次バッチ（差分更新）

### 10.4 バッチ推論

`embed_talents()` のデフォルト実装はループだが、ONNX/Candle では真価を発揮:

```rust
// ONNX バッチ推論の例
impl TwoTowerEmbedder for OnnxTwoTower {
    fn embed_talents(&self, talents: &[Talent]) -> Vec<Embedding> {
        // 全 Talent を1回の forward で処理
        let inputs: Vec<_> = talents.iter()
            .map(|t| tokenize_talent(t))
            .collect();

        let batch_output = self.session.run(inputs)?;

        batch_output.into_iter()
            .map(|vec| Embedding { vector: vec, ... })
            .collect()
    }
}
```

**効果**: N件の Talent を 1/N の時間で処理（GPU利用時は特に顕著）

### 10.5 A/B テスト設計

```
┌─────────────────────────────────────────────────────────────┐
│  Step 1: Shadow（ログのみ）                                  │
│  - Two-Tower スコアを計算・記録するが、ランキングに使わない  │
│  - 既存ロジックとの相関を見る                                │
│                                                             │
│  Step 2: 10% Serving                                        │
│  - project_id % 10 == 0 のみ Two-Tower を適用               │
│  - interaction_logs.variant = 'two_tower_10pct'             │
│                                                             │
│  Step 3: 段階的拡大                                          │
│  - 効果確認後、20% → 50% → 100%                              │
└─────────────────────────────────────────────────────────────┘
```

**評価指標**:
- `thumbs_up_rate`: 👍率（即時フィードバック）
- `accepted_rate`: 成約率（最終結果）
- `interview_scheduled_rate`: 面談設定率（中間指標）

**割り当て単位**: `project_id`（同じ案件で順位がブレない）

**注意**: `project_id % 10` は決定論的だが、IDの発番順によっては偏りが出る可能性がある。
より均一な分割が必要なら `hash(project_id) % 10` を検討。

**ログ拡張**:
```sql
ALTER TABLE ses.interaction_logs
ADD COLUMN variant VARCHAR(50);  -- 'control', 'two_tower_10pct', ...
```

---

## 11. 参考資料

- [Google: Sampling-Bias-Corrected Neural Modeling](https://research.google/pubs/pub48840/)
- [Facebook: Embedding-based Retrieval in Facebook Search](https://arxiv.org/abs/2006.11632)
- [Airbnb: Real-time Personalization using Embeddings](https://medium.com/airbnb-engineering/listing-embeddings-for-similar-listing-recommendations-and-real-time-personalization-in-search-601172f7603e)

---

## 付録A: ディレクトリ構成（予定）

```
crates/sr-common/src/two_tower/
├── mod.rs              # TwoTowerEmbedder trait + factory
├── config.rs           # TwoTowerConfig
├── embedding.rs        # Embedding, EmbeddingSource
├── similarity.rs       # cosine_similarity など
├── tokenizer.rs        # Token 生成ロジック
├── hash_tower.rs       # HashTwoTower 実装
├── onnx_tower.rs       # OnnxTwoTower 実装（Phase 4）
└── candle_tower.rs     # CandleTwoTower 実装（Phase 4+）
```

---

## 付録B: 実装詳細（Rust コード）

### B.1 ドメインモデルの前提

Two-Tower で `rank_talents()` を使うには、`Project` / `Talent` に `id` フィールドが必要：

```rust
// crates/sr-common/src/lib.rs
// ✅ 既に実装済み

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    pub id: Option<i64>,  // DB主キー or 外部ID
    pub work_todofuken: Option<String>,
    // ... 既存フィールド
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Talent {
    pub id: Option<i64>,  // DB主キー or 外部ID
    pub residential_todofuken: Option<String>,
    // ... 既存フィールド
}
```

> **Note**: `Option<i64>` にしているのは、テスト時やリテラル構築時に id を省略できるようにするため。

---

### B.2 TwoTowerEmbedder Trait 完全定義

```rust
// crates/sr-common/src/two_tower/mod.rs

use crate::{Project, Talent};

/// Two-Tower モデルの抽象インターフェース
///
/// 実装例:
/// - HashTwoTower: Feature Hashing（決定論的、学習不要）
/// - OnnxTwoTower: ONNX Runtime（学習済みモデル読み込み）
/// - CandleTwoTower: Candle（Rust-native推論）
///
/// interaction_logs には name() と version() が記録される。
pub trait TwoTowerEmbedder: Send + Sync {
    /// 実装名（"hash", "onnx", "candle"）
    /// → interaction_logs.two_tower_embedder
    fn name(&self) -> &'static str;

    /// バージョン情報（モデルの世代管理用）
    /// → interaction_logs.two_tower_version
    /// 例: "v1", "20241215", "hash-v2"
    fn version(&self) -> &str;

    /// 埋め込み次元数
    fn dimension(&self) -> usize;

    /// 案件を埋め込みベクトルに変換
    fn embed_project(&self, project: &Project) -> Embedding;

    /// 人材を埋め込みベクトルに変換
    fn embed_talent(&self, talent: &Talent) -> Embedding;

    /// 複数の人材を一括で埋め込み（デフォルト実装: ループ）
    /// ONNX/Candle ではバッチ推論でオーバーライド推奨
    fn embed_talents(&self, talents: &[Talent]) -> Vec<Embedding> {
        talents.iter().map(|t| self.embed_talent(t)).collect()
    }

    /// 2つの埋め込みベクトルの類似度（0.0〜1.0）
    fn similarity(&self, a: &Embedding, b: &Embedding) -> f32 {
        cosine_similarity(&a.vector, &b.vector)
    }

    /// 複数の人材を案件に対してランキング
    /// バッチ推論（embed_talents）を使用して高速化
    ///
    /// **注意**: talent.id が None の場合は 0 を返す。
    /// 0 が有効な talent_id でないことを前提としている。
    fn rank_talents(&self, project: &Project, talents: &[Talent]) -> Vec<(i64, f32)> {
        let project_emb = self.embed_project(project);

        // バッチで全Talentを埋め込み（ONNX/Candleで効果的）
        let talent_embs = self.embed_talents(talents);

        // スコア計算
        let mut scores: Vec<_> = talents
            .iter()
            .zip(talent_embs.iter())
            .map(|(t, emb)| {
                let sim = self.similarity(&project_emb, emb);
                (t.id.unwrap_or(0), sim)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

/// 埋め込みベクトル
#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,
    pub source: EmbeddingSource,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingSource {
    Project,
    Talent,
}

/// コサイン類似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "embedding dimension mismatch");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    // Clamp to [0, 1] for normalized similarity
    ((dot / (norm_a * norm_b)) + 1.0) / 2.0
}
```

---

### B.3 Tokenizer（共通トークン＋重み方式）

**設計方針**:
- **共通トークンを使う**: Project と Talent で同じトークン（`skill:java`）を出力
- **重みで強調**: 必須スキルは weight=2.0、優遇スキルは weight=1.0 など
- **HashTwoTower で機能させる**: トークンが重なることで cosine 類似度が上がる

```
旧方式（非対称、HashTwoTowerで効かない）:
  Project: skill:req:java     ←─ 別のトークン
  Talent:  skill:have:java    ←─

新方式（対称、HashTwoTowerで効く）:
  Project: skill:java (weight=2.0)  ←─ 同じトークン
  Talent:  skill:java (weight=1.0)  ←─
```

```rust
// crates/sr-common/src/two_tower/tokenizer.rs

use crate::{Project, Talent};

/// 重み付きトークン
#[derive(Debug, Clone)]
pub struct WeightedToken {
    pub token: String,
    pub weight: f32,
}

impl WeightedToken {
    pub fn new(token: impl Into<String>, weight: f32) -> Self {
        Self { token: token.into(), weight }
    }
}

/// トークン形式（v2: 共通トークン方式）:
/// - skill:<normalized>        (スキル - Project/Talent 共通)
/// - loc:<todofuken>           (都道府県 - 共通)
/// - loc:area:<area>           (エリア)
/// - loc:station:<station>     (駅)
/// - remote:<type>             (リモート形態)
/// - exp:<bucket>              (経験年数バケット)
/// - contract:<type>           (契約形態)
/// - tanka:<bucket>            (単価バケット)
/// - lang:ja:<level>           (日本語レベル)
/// - lang:en:<level>           (英語レベル)

pub fn tokenize_project(project: &Project) -> Vec<WeightedToken> {
    let mut tokens = Vec::new();

    // スキル（共通トークン、重みで必須/優遇を区別）
    for skill in &project.required_skills_keywords {
        tokens.push(WeightedToken::new(
            format!("skill:{}", skill.to_lowercase()),
            2.0,  // 必須は強く
        ));
    }
    for skill in &project.preferred_skills_keywords {
        tokens.push(WeightedToken::new(
            format!("skill:{}", skill.to_lowercase()),
            1.0,  // 優遇は普通
        ));
    }

    // 勤務地（共通トークン）
    if let Some(ref pref) = project.work_todofuken {
        tokens.push(WeightedToken::new(format!("loc:{}", pref), 1.5));
    }
    if let Some(ref area) = project.work_area {
        tokens.push(WeightedToken::new(format!("loc:area:{}", area), 1.0));
    }
    if let Some(ref station) = project.work_station {
        tokens.push(WeightedToken::new(format!("loc:station:{}", station), 0.5));
    }

    // リモート
    if let Some(ref remote) = project.remote_onsite {
        tokens.push(WeightedToken::new(format!("remote:{}", remote), 1.5));
    }

    // 経験年数
    if let Some(years) = project.min_experience_years {
        tokens.push(WeightedToken::new(
            format!("exp:{}", exp_years_bucket(years)),
            1.0,
        ));
    }

    // 契約形態
    if let Some(ref contract) = project.contract_type {
        tokens.push(WeightedToken::new(format!("contract:{}", contract), 1.0));
    }

    // 単価（バケット化して共通トークン）
    if let Some(max_tanka) = project.monthly_tanka_max {
        tokens.push(WeightedToken::new(
            format!("tanka:{}", tanka_bucket(max_tanka)),
            1.0,
        ));
    }

    // 言語
    if let Some(ref ja) = project.japanese_skill {
        tokens.push(WeightedToken::new(format!("lang:ja:{}", ja), 1.0));
    }
    if let Some(ref en) = project.english_skill {
        tokens.push(WeightedToken::new(format!("lang:en:{}", en), 1.0));
    }

    tokens
}

pub fn tokenize_talent(talent: &Talent) -> Vec<WeightedToken> {
    let mut tokens = Vec::new();

    // スキル（共通トークン）
    for skill in &talent.possessed_skills_keywords {
        tokens.push(WeightedToken::new(
            format!("skill:{}", skill.to_lowercase()),
            1.0,
        ));
    }

    // 居住地（共通トークン）
    if let Some(ref pref) = talent.residential_todofuken {
        tokens.push(WeightedToken::new(format!("loc:{}", pref), 1.5));
    }
    if let Some(ref area) = talent.residential_area {
        tokens.push(WeightedToken::new(format!("loc:area:{}", area), 1.0));
    }
    if let Some(ref station) = talent.nearest_station {
        tokens.push(WeightedToken::new(format!("loc:station:{}", station), 0.5));
    }

    // 希望リモート
    if let Some(ref remote) = talent.desired_remote_onsite {
        tokens.push(WeightedToken::new(format!("remote:{}", remote), 1.5));
    }

    // 経験年数
    if let Some(years) = talent.min_experience_years {
        tokens.push(WeightedToken::new(
            format!("exp:{}", exp_years_bucket(years)),
            1.0,
        ));
    }

    // 契約形態（primary のみ共通トークン）
    if let Some(ref contract) = talent.primary_contract_type {
        tokens.push(WeightedToken::new(format!("contract:{}", contract), 1.0));
    }

    // 希望単価（バケット化して共通トークン）
    if let Some(min_price) = talent.desired_price_min {
        tokens.push(WeightedToken::new(
            format!("tanka:{}", tanka_bucket(min_price)),
            1.0,
        ));
    }

    // 言語
    if let Some(ref ja) = talent.japanese_skill {
        tokens.push(WeightedToken::new(format!("lang:ja:{}", ja), 1.0));
    }
    if let Some(ref en) = talent.english_skill {
        tokens.push(WeightedToken::new(format!("lang:en:{}", en), 1.0));
    }

    tokens
}

/// 経験年数バケット: 0-2, 3-5, 6-10, 11+
fn exp_years_bucket(years: i32) -> &'static str {
    match years {
        0..=2 => "0-2",
        3..=5 => "3-5",
        6..=10 => "6-10",
        _ => "11+",
    }
}

/// 単価バケット: 30以下, 30-50, 50-70, 70-100, 100+（万円）
fn tanka_bucket(tanka: u32) -> &'static str {
    match tanka {
        0..=299999 => "under30",
        300000..=499999 => "30-50",
        500000..=699999 => "50-70",
        700000..=999999 => "70-100",
        _ => "100+",
    }
}
```

---

### B.4 HashTwoTower 実装（Feature Hashing + 重み）

```rust
// crates/sr-common/src/two_tower/hash_tower.rs

use super::{Embedding, EmbeddingSource, TwoTowerEmbedder};
use super::tokenizer::WeightedToken;
use crate::{Project, Talent};
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

/// 固定 seed（決定論的 hash のため）
/// ⚠️ この値を変更すると全 embedding が変わる → two_tower_version を上げること
const HASH_SEED_K0: u64 = 0x0123456789abcdef;
const HASH_SEED_K1: u64 = 0xfedcba9876543210;

/// Feature Hashing を用いた決定論的 Two-Tower
///
/// - 学習不要（固定ハッシュ関数）
/// - 高速（O(n) where n = token count）
/// - 重み付きトークンで必須/優遇を区別
/// - SipHash13 + 固定 seed で Rust バージョン間の安定性を保証
pub struct HashTwoTower {
    pub config: TwoTowerConfig,
}

#[derive(Debug, Clone)]
pub struct TwoTowerConfig {
    /// 埋め込み次元数（2のべき乗推奨: 256, 512, 1024）
    pub dimension: usize,
    /// Two-Tower スコアの重み（total_score 計算時）
    pub weight: f32,
    /// 有効/無効フラグ
    pub enabled: bool,
}

impl Default for TwoTowerConfig {
    fn default() -> Self {
        Self {
            dimension: 256,
            weight: 0.0, // Phase 3 では無効
            enabled: false,
        }
    }
}

impl HashTwoTower {
    pub fn new(config: TwoTowerConfig) -> Self {
        Self { config }
    }

    /// トークンをハッシュして次元インデックスに変換
    /// SipHash13 + 固定 seed で決定論的に計算
    fn hash_token(&self, token: &str) -> usize {
        let mut hasher = SipHasher13::new_with_keys(HASH_SEED_K0, HASH_SEED_K1);
        token.hash(&mut hasher);
        (hasher.finish() as usize) % self.config.dimension
    }

    /// 重み付きトークン列を埋め込みベクトルに変換
    fn tokens_to_embedding(
        &self,
        tokens: Vec<WeightedToken>,
        source: EmbeddingSource,
    ) -> Embedding {
        let mut vector = vec![0.0f32; self.config.dimension];

        for wt in &tokens {
            let idx = self.hash_token(&wt.token);
            // Sign hashing: 偶数ハッシュ → +weight, 奇数ハッシュ → -weight
            let sign = if self.hash_token(&format!("{}_sign", wt.token)) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            vector[idx] += sign * wt.weight;  // 重みを掛ける
        }

        // L2正規化
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }

        Embedding {
            vector,
            source: source,
            created_at: chrono::Utc::now(),
        }
    }
}

impl TwoTowerEmbedder for HashTwoTower {
    fn name(&self) -> &'static str {
        "hash"
    }

    fn version(&self) -> &str {
        // トークン設計やハッシュ関数が変わったらバージョンを上げる
        "v2"  // v2: 共通トークン + 重み方式
    }

    fn dimension(&self) -> usize {
        self.config.dimension
    }

    fn embed_project(&self, project: &Project) -> Embedding {
        let tokens = super::tokenizer::tokenize_project(project);
        self.tokens_to_embedding(tokens, EmbeddingSource::Project)
    }

    fn embed_talent(&self, talent: &Talent) -> Embedding {
        let tokens = super::tokenizer::tokenize_talent(talent);
        self.tokens_to_embedding(tokens, EmbeddingSource::Talent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_tower_produces_normalized_vectors() {
        let tower = HashTwoTower::new(TwoTowerConfig::default());

        let project = Project {
            required_skills_keywords: vec!["rust".into(), "python".into()],
            work_todofuken: Some("東京都".into()),
            ..Default::default()
        };

        let emb = tower.embed_project(&project);

        // L2ノルムが1.0であることを確認
        let norm: f32 = emb.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "L2 norm should be 1.0, got {}", norm);
    }

    #[test]
    fn similar_inputs_have_higher_similarity() {
        let tower = HashTwoTower::new(TwoTowerConfig::default());

        // 共通トークン方式: skill:rust, skill:aws, loc:東京都 が共通
        let project = Project {
            required_skills_keywords: vec!["rust".into(), "aws".into()],
            work_todofuken: Some("東京都".into()),
            ..Default::default()
        };

        let similar_talent = Talent {
            possessed_skills_keywords: vec!["rust".into(), "aws".into(), "docker".into()],
            residential_todofuken: Some("東京都".into()),
            ..Default::default()
        };

        let different_talent = Talent {
            possessed_skills_keywords: vec!["cobol".into(), "oracle".into()],
            residential_todofuken: Some("北海道".into()),
            ..Default::default()
        };

        let proj_emb = tower.embed_project(&project);
        let similar_emb = tower.embed_talent(&similar_talent);
        let different_emb = tower.embed_talent(&different_talent);

        let similar_score = tower.similarity(&proj_emb, &similar_emb);
        let different_score = tower.similarity(&proj_emb, &different_emb);

        assert!(
            similar_score > different_score,
            "Similar talent should have higher score: {} vs {}",
            similar_score,
            different_score
        );
    }

    #[test]
    fn required_skill_match_beats_preferred_skill_match() {
        let tower = HashTwoTower::new(TwoTowerConfig::default());

        // rust は必須 (weight=2.0)、python は優遇 (weight=1.0)
        let project = Project {
            required_skills_keywords: vec!["rust".into()],
            preferred_skills_keywords: vec!["python".into()],
            ..Default::default()
        };

        // rust持ち（必須一致）
        let required_match = Talent {
            possessed_skills_keywords: vec!["rust".into()],
            ..Default::default()
        };

        // python持ち（優遇一致）
        let preferred_match = Talent {
            possessed_skills_keywords: vec!["python".into()],
            ..Default::default()
        };

        let proj_emb = tower.embed_project(&project);
        let req_emb = tower.embed_talent(&required_match);
        let pref_emb = tower.embed_talent(&preferred_match);

        let req_score = tower.similarity(&proj_emb, &req_emb);
        let pref_score = tower.similarity(&proj_emb, &pref_emb);

        assert!(
            req_score > pref_score,
            "Required skill match should beat preferred: {} vs {}",
            req_score,
            pref_score
        );
    }
}
```

---

### B.5 OnnxTwoTower スタブ（Phase 4 準備）

```rust
// crates/sr-common/src/two_tower/onnx_tower.rs

use super::{Embedding, EmbeddingSource, TwoTowerEmbedder};
use crate::{Project, Talent};

/// ONNX Runtime を使用した Two-Tower
///
/// Phase 4 で学習済みモデルを読み込む
pub struct OnnxTwoTower {
    // session: ort::Session, // Phase 4 で有効化
    model_path: String,
    dimension: usize,
}

impl OnnxTwoTower {
    pub fn new(model_path: &str, dimension: usize) -> Result<Self, String> {
        // Phase 4: ONNX ランタイム初期化
        // let session = ort::Session::new(model_path)?;

        Ok(Self {
            model_path: model_path.to_string(),
            dimension,
        })
    }
}

impl TwoTowerEmbedder for OnnxTwoTower {
    fn name(&self) -> &'static str {
        "onnx"
    }

    fn version(&self) -> &str {
        // モデルファイル名やメタデータからバージョンを取得
        // 例: "20241215" (学習日) or "v3.2"
        "v1"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_project(&self, _project: &Project) -> Embedding {
        // Phase 4: ONNX 推論
        Embedding {
            vector: vec![0.0; self.dimension],
            source: EmbeddingSource::Project,
            created_at: chrono::Utc::now(),
        }
    }

    fn embed_talent(&self, _talent: &Talent) -> Embedding {
        // Phase 4: ONNX 推論
        Embedding {
            vector: vec![0.0; self.dimension],
            source: EmbeddingSource::Talent,
            created_at: chrono::Utc::now(),
        }
    }
}
```

---

### B.6 CandleTwoTower スタブ（Phase 4+）

```rust
// crates/sr-common/src/two_tower/candle_tower.rs

use super::{Embedding, EmbeddingSource, TwoTowerEmbedder};
use crate::{Project, Talent};

/// Candle (Rust-native) を使用した Two-Tower
///
/// Phase 4+ でPyTorchモデルをRustに移植
pub struct CandleTwoTower {
    // model: candle::Model, // Phase 4+ で有効化
    dimension: usize,
}

impl CandleTwoTower {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl TwoTowerEmbedder for CandleTwoTower {
    fn name(&self) -> &'static str {
        "candle"
    }

    fn version(&self) -> &str {
        "v1"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_project(&self, _project: &Project) -> Embedding {
        // Phase 4+: Candle 推論
        Embedding {
            vector: vec![0.0; self.dimension],
            source: EmbeddingSource::Project,
            created_at: chrono::Utc::now(),
        }
    }

    fn embed_talent(&self, _talent: &Talent) -> Embedding {
        // Phase 4+: Candle 推論
        Embedding {
            vector: vec![0.0; self.dimension],
            source: EmbeddingSource::Talent,
            created_at: chrono::Utc::now(),
        }
    }
}
```

---

### B.7 Two-Tower と既存スコアリングの統合

```rust
// crates/sr-common/src/matching/scoring.rs への追加

use crate::two_tower::{TwoTowerConfig, TwoTowerEmbedder, HashTwoTower};

/// 総合スコア計算（Two-Tower 込み）
pub fn calculate_total_score_with_two_tower(
    business_score: f32,
    semantic_score: f32,
    historical_score: f32,
    two_tower_score: Option<f32>,
    weights: &TotalScoreWeights,
    two_tower_config: &TwoTowerConfig,
) -> f32 {
    // 既存の 3要素スコア
    let base_score = calculate_total_score(
        business_score,
        semantic_score,
        historical_score,
        weights,
    );

    // Two-Tower が無効または未計算の場合はそのまま返す
    if !two_tower_config.enabled {
        return base_score;
    }

    let tt_score = two_tower_score.unwrap_or(0.5); // デフォルト: 中立

    // 重み付き合成
    // Phase 3: two_tower_weight = 0.0 なので影響なし
    let total_weight = 1.0 + two_tower_config.weight;
    let combined = (base_score + two_tower_config.weight * tt_score) / total_weight;

    combined.clamp(0.0, 1.0)
}
```

---

### B.8 Two-Tower ファクトリ

```rust
// crates/sr-common/src/two_tower/mod.rs への追加

/// Two-Tower 実装のファクトリ
pub fn create_embedder(name: &str, config: TwoTowerConfig) -> Box<dyn TwoTowerEmbedder> {
    match name {
        "hash" => Box::new(HashTwoTower::new(config)),
        "onnx" => {
            // Phase 4: モデルパスを設定から読み込み
            let model_path = std::env::var("TWO_TOWER_ONNX_PATH")
                .unwrap_or_else(|_| "models/two_tower.onnx".into());
            Box::new(OnnxTwoTower::new(&model_path, config.dimension).unwrap())
        }
        "candle" => Box::new(CandleTwoTower::new(config.dimension)),
        _ => Box::new(HashTwoTower::new(config)), // デフォルト
    }
}

/// 環境変数から Two-Tower 設定を読み込み
pub fn load_config_from_env() -> TwoTowerConfig {
    TwoTowerConfig {
        dimension: std::env::var("TWO_TOWER_DIMENSION")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(256),
        weight: std::env::var("TWO_TOWER_WEIGHT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        enabled: std::env::var("TWO_TOWER_ENABLED")
            .ok()
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false),
    }
}
```

---

## 付録C: interaction_logs DDL（学習データ収集）

> **📌 正は `crates/sr-common/src/schema.rs` の `INTERACTION_LOGS_DDL`**
>
> 以下は参照用。実装時は schema.rs を参照すること。

```sql
-- 予測とFBのペアを記録（Two-Tower学習用）
CREATE TABLE ses.interaction_logs (
    id BIGSERIAL PRIMARY KEY,

    -- マッチング情報
    match_result_id BIGINT REFERENCES ses.match_results(id),
    talent_id BIGINT NOT NULL,
    project_id BIGINT NOT NULL,
    match_run_id VARCHAR(64) NOT NULL,  -- 実行インスタンスID（ULID）
    engine_version VARCHAR(20),
    config_version VARCHAR(20),

    -- Two-Tower 予測
    two_tower_score DOUBLE PRECISION,   -- 予測スコア
    two_tower_embedder VARCHAR(50),     -- hash / onnx / candle
    two_tower_version VARCHAR(20),      -- モデルバージョン

    -- ビジネスルールスコア（比較用）
    business_score DOUBLE PRECISION,

    -- 結果（後から更新）
    -- 許容値: accepted, rejected, interview_scheduled, review_ok, review_ng,
    --         thumbs_up, thumbs_down, no_response, NULL（初期値）
    -- ※ 'pending' 文字列は使わない（初期状態 = NULL）
    outcome VARCHAR(20),
    feedback_at TIMESTAMPTZ,

    -- A/Bテスト
    variant VARCHAR(50),  -- 'control', 'two_tower_10pct', ...

    -- メタデータ
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- JST基準の日付（自動算出）
    run_date DATE GENERATED ALWAYS AS (
        (created_at AT TIME ZONE 'Asia/Tokyo')::date
    ) STORED,

    -- 制約
    CONSTRAINT interaction_logs_unique_run_pair
        UNIQUE (match_run_id, talent_id, project_id)
);

-- Phase 4 学習用のビュー
-- ※ 正は schema.rs の INTERACTION_LOGS_DDL
CREATE OR REPLACE VIEW ses.training_pairs AS
SELECT
    il.talent_id,
    il.project_id,
    il.two_tower_score,
    il.two_tower_embedder,
    il.two_tower_version,
    il.business_score,
    il.outcome,
    il.variant,
    CASE
        WHEN il.outcome = 'accepted' THEN 1.0
        WHEN il.outcome = 'rejected' THEN 0.0
        WHEN il.outcome = 'thumbs_up' THEN 1.0
        WHEN il.outcome = 'thumbs_down' THEN 0.0
        WHEN il.outcome = 'review_ok' THEN 1.0
        WHEN il.outcome = 'review_ng' THEN 0.0
        WHEN il.outcome = 'interview_scheduled' THEN 0.8
        ELSE NULL
    END AS label,
    il.run_date,
    il.created_at
FROM ses.interaction_logs il
WHERE il.outcome IS NOT NULL
  AND il.outcome <> 'no_response';
-- ※ 'pending' は NULL で表現するため、outcome IS NOT NULL で除外済み

-- 学習データ統計
CREATE OR REPLACE VIEW ses.training_stats AS
SELECT
    COUNT(*) FILTER (WHERE outcome = 'accepted') AS accepted_count,
    COUNT(*) FILTER (WHERE outcome = 'rejected') AS rejected_count,
    COUNT(*) FILTER (WHERE outcome IS NULL) AS pending_count,
    -- Cold Start判定用: training_pairsで使えるラベル総数
    -- ※ 'pending' は NULL で表現するため、outcome IS NOT NULL で除外済み
    COUNT(*) FILTER (WHERE outcome IS NOT NULL AND outcome <> 'no_response') AS labeled_count,
    MIN(created_at) AS first_log_at,
    MAX(created_at) AS last_log_at,
    COUNT(DISTINCT run_date) AS active_days  -- JST基準
FROM ses.interaction_logs;
```

---

## 付録D: 実装チェックリスト（詳細版）

### Phase 3-A Done 条件

- [ ] `TwoTowerEmbedder` trait が定義されている
- [ ] `HashTwoTower` が実装されている
- [ ] `tokenize_project()` / `tokenize_talent()` が動作する
- [ ] `cargo test` で類似度テストが通る

### Phase 3-B Done 条件

- [ ] `interaction_logs` DDL が本番DBに適用されている
- [ ] マッチング実行時に `interaction_logs` にINSERTされる
- [ ] `training_pairs` ビューが動作する

### Phase 3-C Done 条件

- [ ] `OnnxTwoTower` / `CandleTwoTower` のスタブが実装されている
- [ ] `create_embedder("onnx", ...)` / `create_embedder("candle", ...)` がコンパイル通る
- [ ] 環境変数 `TWO_TOWER_EMBEDDER` で切り替え可能

---

## 付録E: 学習パイプライン詳細（Phase 4）

### E.1 モデルアーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│  Two-Tower Model (PyTorch)                                  │
│                                                             │
│  Talent Tower              Project Tower                    │
│  ┌─────────────┐           ┌─────────────┐                  │
│  │ Tokenizer   │           │ Tokenizer   │                  │
│  │ (共通)      │           │ (共通)      │                  │
│  └──────┬──────┘           └──────┬──────┘                  │
│         ▼                         ▼                         │
│  ┌─────────────┐           ┌─────────────┐                  │
│  │ EmbeddingBag│           │ EmbeddingBag│                  │
│  │ (vocab_size │           │ (vocab_size │                  │
│  │  → 128dim)  │           │  → 128dim)  │                  │
│  └──────┬──────┘           └──────┬──────┘                  │
│         ▼                         ▼                         │
│  ┌─────────────┐           ┌─────────────┐                  │
│  │ MLP         │           │ MLP         │                  │
│  │ 128→256→256 │           │ 128→256→256 │                  │
│  └──────┬──────┘           └──────┬──────┘                  │
│         ▼                         ▼                         │
│  ┌─────────────┐           ┌─────────────┐                  │
│  │ L2 Normalize│           │ L2 Normalize│                  │
│  │ → D次元     │           │ → D次元     │                  │
│  └──────┬──────┘           └──────┬──────┘                  │
│         │                         │                         │
│         └───────────┬─────────────┘                         │
│                     ▼                                       │
│              Cosine Similarity                              │
└─────────────────────────────────────────────────────────────┘
```

### E.2 損失関数

```python
def contrastive_loss(positive_sim, negative_sims, margin=0.2):
    """
    In-batch negatives を使った contrastive loss

    positive_sim: 正例ペアの類似度
    negative_sims: 負例ペアの類似度リスト
    """
    losses = []
    for neg_sim in negative_sims:
        # margin-based: positive > negative + margin
        loss = torch.relu(margin - positive_sim + neg_sim)
        losses.append(loss)
    return torch.mean(torch.stack(losses))
```

**代替**: BCE (Binary Cross Entropy) on similarity

```python
def bce_loss(sim, label):
    return F.binary_cross_entropy_with_logits(sim, label)
```

### E.3 評価指標

| 指標 | 計算 | 目標 |
|------|------|------|
| AUC | ROC曲線下面積 | > 0.75 |
| Recall@10 | 正解がTop10に入る率 | > 0.5 |
| MRR | 平均逆順位 | > 0.3 |

```python
def evaluate(model, test_pairs):
    scores = []
    for talent, project, label in test_pairs:
        sim = model.similarity(talent, project)
        scores.append((sim, label))

    # AUC計算
    y_true = [s[1] for s in scores]
    y_pred = [s[0] for s in scores]
    auc = roc_auc_score(y_true, y_pred)

    return {"auc": auc, ...}
```

### E.4 学習スケジュール

```
┌─────────────────────────────────────────────────────────────┐
│  学習パイプライン                                            │
│                                                             │
│  トリガー:                                                   │
│  - 週次 cron（毎週日曜 03:00）                               │
│  - ラベル数が 500件 増加したとき                             │
│                                                             │
│  ステップ:                                                   │
│  1. training_pairs を PostgreSQL から取得                   │
│  2. 80/20 で train/valid 分割                               │
│  3. 100 epochs、early stopping (patience=10)                │
│  4. valid AUC > 0.7 なら ONNX エクスポート                   │
│  5. models/two_tower_YYYYMMDD.onnx に保存                   │
│  6. TWO_TOWER_ONNX_PATH を更新                               │
│                                                             │
│  失敗時:                                                     │
│  - アラート送信                                              │
│  - 既存モデルを維持（ロールバック不要）                       │
└─────────────────────────────────────────────────────────────┘
```

### E.5 ONNX エクスポート

```python
import torch.onnx

def export_to_onnx(model, output_path, dimension=256):
    # ダミー入力
    dummy_talent_tokens = torch.tensor([[1, 2, 3]])  # token IDs
    dummy_project_tokens = torch.tensor([[4, 5, 6]])

    # エクスポート
    torch.onnx.export(
        model,
        (dummy_talent_tokens, dummy_project_tokens),
        output_path,
        input_names=["talent_tokens", "project_tokens"],
        output_names=["similarity"],
        dynamic_axes={
            "talent_tokens": {0: "batch", 1: "seq"},
            "project_tokens": {0: "batch", 1: "seq"},
        },
        opset_version=14,
    )

    print(f"Exported to {output_path}")
```

### E.6 デプロイフロー

```
1. 学習完了
   └─ models/two_tower_20241215.onnx 生成

2. 検証
   └─ AUC > 0.7 確認

3. 環境変数更新
   └─ TWO_TOWER_ONNX_PATH=models/two_tower_20241215.onnx

4. プロセス再起動（or hot reload）
   └─ OnnxTwoTower が新モデルをロード

5. interaction_logs に two_tower_version="20241215" が記録される
```
