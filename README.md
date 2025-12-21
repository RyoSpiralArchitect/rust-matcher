# rust-matcher

rust-matcher は、案件とタレントの情報を正規化し、KO 判定とスコアリングを一元化するマッチングエンジンです。Phase1 までで実装された
範囲を中心に、現在の構成と使い方をまとめています。

## 現在の達成状況 (Phase1 → Phase2 着手)
- **正規化レイヤ**: 都道府県/エリア/リモート設定、駅名、スキル、フロー深度・制限など、主要フィールドの補正を実装。
- **KO 判定とスコアリング**: `KoDecision` を単一ソースに、ロケーションやスキル一致度を考慮した KO/スコア算出を実装。Phase2 では `BusinessRulesEngine` で詳細/Prefilter スコア（単価/勤務地/スキル/経験/契約）を加重合成し、NG キーワードや年齢制限も KO 判定に加味。`business_rules_score` をベースに `total_score = business × semantic × historical` の重み付き合成を用意し、MVP では business=1.0 で semantic/historical を 0.0 に固定しています。設定ミスでもスコアが暴れないよう、total_score_weights は内部で正規化し、負の重みは0に丸めた上で 0〜1 にクランプされます。
- **事前フィルタ**: HardKo を弾いた上で prefilter 重みを使う `EnhancedPreFilter` を追加し、候補を上位スコア順に絞り込める状態に。
- **スコアリング連携**: `MatchingEngine` で prefilter → 詳細スコアを一気通貫に計算し、総合スコア順で順位付けできるパイプラインを追加。
- **キュー処理**: `extraction_queue` ワーカーが pending → processing → completed を決定的順序で巡回し、リトライや manual review
  の判定を行う仕組みを実装。`sr-queue-recovery` では processing のまま 10 分以上滞留したジョブのみを pending に戻して
  再試行できるようにしています。message_id/subject_hash での重複投入もガード。
- **DB 連携の足がかり**: Postgres プールを共有し、`sr-extractor` は抽出結果を pending のまま `ses.extraction_queue`
  に UPSERT、`sr-llm-worker` は `FOR UPDATE SKIP LOCKED` で次の pending をロックして処理しつつ
  `match_results` のスナップショットを挿入、連続ジョブをキューが空になるまで（または `--max-jobs` 上限まで）処理できるように拡張、
  `sr-queue-recovery` は DB 上の滞留ジョブを pending に戻す更新クエリまで実装済みです（本番 I/O は今後拡充予定）。
- **抽出パイプライン**: `sr-extractor` が `ses.anken_emails` から未処理メールを取得し、Tier1/2 正規表現で抽出→
  推奨メソッドと priority を付けて queue に投入する最小フローまで組み込み済みです（本文は queue に保存せず DB から都度参照）。
- **価格計算**: 単価関連のパラメータとタレント/案件別の計算ユーティリティを追加。
- **日付正規化**: 受領日時を基準に開始日のテキストを ASAP/日付/四半期/応相談まで丸め、`DatePrecision` と注釈で解像度を明示。
- **エントリポイント**: 3 バイナリを追加（`sr-extractor`・`sr-llm-worker`・`sr-queue-recovery`）。現時点ではスタブ実装で、
  共通ロジックは `sr-common` クレート経由で利用。

詳細な MVP までの計画書は `docs/MVP_PLAN.md` に移動しました。

## Phase2 運用準備メモ
- **技術選定**: DB 接続は `tokio-postgres` + `deadpool-postgres`、CLI は `clap`、ログは `tracing` + `tracing-subscriber`、環境変数は `dotenvy` で吸収。
- **CLI オプション**: `sr-extractor` は `--db-url` と `--dry-run`、`sr-llm-worker` は `--db-url` / `--worker-id` / `--max-jobs`（キューが空になるまで処理がデフォルト）、`sr-queue-recovery` は `--db-url` と `--stale-minutes` (デフォルト 10) を受け取ります。いずれも `.env` の `DATABASE_URL` から自動解決可能。
- **systemd 雛形**: `deploy/` に 5 分間隔の `sr-extractor.timer`、常駐の `sr-llm-worker.service`、10 分間隔の `sr-queue-recovery.timer` を配置。`/etc/sr-matcher.env` で DB URL を注入する想定です。
- **データフロー (MVP)**: `sr-extractor` で `anken_emails` から pending を投入し、`sr-llm-worker` が同一トランザクションで `talents_enum` UPSERT → `match_results` INSERT → queue 完了まで進める方針 (DB I/O は現在スタブ)。
- **match_results テーブル案**: 以下の DDL を基準に、日次スナップショットのユニーク制約とスコア/理由を永続化予定です。

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

CREATE INDEX idx_match_results_talent ON ses.match_results(talent_id, created_at);
CREATE INDEX idx_match_results_project ON ses.match_results(project_id, created_at);
CREATE INDEX idx_match_results_score ON ses.match_results(score_total DESC) WHERE NOT is_knockout;
```

## Phase 3: LLM統合とマッチングパイプライン接続 (Now)

**目標**: 本番で壊れない形で回しつつ、比較ログ（shadow）を溜めて、勝ち筋を確定する

### 実装ロードマップ

| Step | 内容 | 状態 |
|------|------|------|
| **Step 1** | match_results DDL + 保存 | 🔴 着手予定 |
| **Step 2** | LLM shadow 10% | ⏳ 待機 |
| **Step 3** | systemd本番ループ | ⏳ 待機 |
| **Step 4** | GUI（営業FB導線） | 🔜 将来 |

### データ設計（Phase 4 を見据える）

```
ses.match_results          # その時点の判定（再計算可能）
├── talent_snapshot_id     # Two-Tower用キー
├── project_snapshot_id    # Two-Tower用キー
├── match_run_id           # 実行ID（engine_version込み）
├── ko_reasons             # KO理由 JSONB
├── score_breakdown        # スコア内訳 JSONB
└── llm_provider           # どのLLMで処理したか

ses.feedback_events        # 営業の現場真実（不可逆）
├── match_result_id        # FK → match_results
├── feedback_type          # accepted/rejected/no_response
└── created_by             # 営業担当者

ses.llm_comparison_results # Shadow比較ログ
├── primary_provider       # 本番LLM
├── shadow_provider        # 比較LLM
└── diff_summary           # 差分サマリ
```

### LLM Provider 設計

```
sr-llm-worker/src/llm/
├── mod.rs          # LlmProvider trait + factory
├── types.rs        # LlmRequest / LlmResponse
├── validator.rs    # レスポンス検証
└── providers/
    ├── deepseek.rs   # Primary (本番)
    ├── openai.rs     # Shadow比較
    ├── anthropic.rs  # Shadow比較 (Claude)
    ├── google.rs     # Shadow比較 (Gemini)
    ├── mistral.rs    # Shadow比較
    ├── huggingface.rs # 実験用
    ├── n8n_hook.rs   # 既存n8n経由 (移行期間)
    └── mock.rs       # テスト
```

**対応プロバイダ**:

| Provider | Model例 | 用途 |
|----------|---------|------|
| DeepSeek | `deepseek-chat` | Primary (本番) |
| OpenAI | `gpt-4o-mini` | Shadow比較 |
| Anthropic | `claude-3-5-sonnet` | Shadow比較 |
| Google | `gemini-1.5-pro` | Shadow比較 |
| Mistral | `mistral-large` | Shadow比較 |
| HuggingFace | 任意 | 実験用 |
| n8n_hook | (経由) | 移行期間 |

### 接続方式

| 方式 | 説明 | 状態 |
|------|------|------|
| **n8n経由** | 既存ワークフロー活用 | ✅ 現行 |
| **直接API** | rust-matcherから各LLM直接呼び出し | 🔴 着手予定 |
| **GWS直結** | Gmail API直接接続（n8n不要） | 🔜 将来 |

```bash
# 環境変数例
LLM_PROVIDER=deepseek
LLM_SHADOW_PROVIDERS=openai,anthropic
LLM_SHADOW_SAMPLE_PERCENT=10
```

詳細は `docs/MVP_PLAN.md` Phase 3 セクションを参照。

---

## Phase 4: Two-Tower モデル育成 (Preview)

**目標**: 営業FBを学習信号としてマッチング精度を継続的に改善

```
┌─────────────────┐    ┌─────────────────┐
│  Talent Tower   │    │  Project Tower  │
└────────┬────────┘    └────────┬────────┘
         └──────────┬───────────┘
              Cosine Similarity → Match Score
```

- **入力**: `training_pairs` ビュー（match_results + feedback_events JOIN）
- **学習要件**: 1,000+ ペア、3ヶ月以上のFB蓄積
- **統合**: rule-based (0.8) + two-tower (0.2) の加重平均から開始

---

## 未達成・スタブ・今後やること

- **Step 1**: match_results / feedback_events DDL の本番適用
- **Step 2**: LLM Provider trait 実装 + shadow 10% 比較
- **Step 3**: systemd サービス化（落ちても復帰）
- **Step 4**: GUI で「なぜこのスコア？」を可視化、営業FB入力

## アーキテクチャ概要
```
[入力 (案件/タレント)]
          |
          v
  正規化ステップ
  - normalize_for_matching
  - スキル/駅名/フロー補正
          |
          v
  マッチング評価
  - evaluate_location (KoDecision + score)
  - skill matching helper
  - business rules scoring (tanka/location/skills/experience/contract)
          |
          v
  集計 / 判定
  - KO 収束とスコア合算
          |
          v
  抽出キュー処理
  - extraction_queue worker
  - pending → processing → completed
  - sr-extractor / sr-llm-worker / sr-queue-recovery
```

## ディレクトリ構成
- `crates/sr-common/src/corrections/`: 都道府県・エリア・リモート・駅名・スキルなどの正規化ロジック。
- `crates/sr-common/src/matching/`: KO 判定 (`ko_unified`)、ロケーション評価、スキルマッチング、重み設定。
- `crates/sr-common/src/queue/`: `extraction_queue` のモデルとワーカー処理。
- `crates/sr-common/src/calculation/`: 単価パラメータと計算ユーティリティ。
- `crates/sr-common/src/date/`: 受領日時の解決など日付関連の補助機能。
- `crates/sr-extractor/`: 抽出キューへの投入と軽量抽出を担うエントリポイント（スタブ）。
- `crates/sr-llm-worker/`: LLM 推奨ジョブを処理する常駐ワーカー（スタブ）。
- `crates/sr-queue-recovery/`: processing で滞留したジョブを復旧させるタイマーワーカー（スタブ）。
- `docs/MVP_PLAN.md`: 旧 README。MVP までの詳細計画を保持。

## 開発のヒント
- テスト実行: `cargo test`
- 正規化や KO 判定の規約は README ではなく `docs/MVP_PLAN.md` にまとまっています。
