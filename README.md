# rust-matcher

案件とタレントの情報を正規化し、KO 判定とスコアリングを一元化するマッチングエンジン。

## Goal: 3ヶ月以内に営業が使えるGUIを出す

```
現在地                                              3ヶ月後
   │                                                   │
   ▼                                                   ▼
Phase 2 完了 ───────────────────────────────────► GUI リリース
   │                                                   │
   ├─ 正規化レイヤ ✅                                   ├─ Queue Dashboard
   ├─ KO判定 + スコアリング ✅                          ├─ 案件→候補一覧
   ├─ キュー処理 ✅                                    ├─ 候補詳細 + FB入力
   └─ DB連携 ✅                                        └─ 営業FB → Two-Tower学習データ蓄積
```

---

## 実装ロードマップ

| Phase | Step | 内容 | 状態 |
|-------|------|------|------|
| 3 | 1 | match_results DDL + 保存 | ✅ 完了 |
| 3 | 2 | LLM shadow 10% 比較 | ✅ 完了 |
| 3 | 3 | systemd 本番ループ | ✅ 完了 |
| 3 | 3-A | Two-Tower 骨格 (HashTwoTower) | 🔴 着手予定 |
| 3 | 3-B | interaction_logs DDL | ✅ 完了 |
| 3.5 | A | MatchResponse DTO + MatchConfig | ✅ 完了 |
| 3.5 | B | feedback_events DDL（統一版） | ✅ 完了 |
| 3.5 | C | QueueDashboard DTO | ✅ 完了 |
| 3.5 | D | HTTP API (Axum) | ⏳ 待機 |
| 3.5 | E | **GUI (Next.js)** | 🎯 **ゴール** |
| 4 | - | Two-Tower 学習 | 🔜 将来 |

---

## 現状できること（MVP準備中のサマリ）

- **マッチング結果の契約を確定**: `MatchResponse` / `MatchConfig` / `QueueDashboard` DTO を sr-common に実装済み。フロントはこの契約に沿って開発着手可能。
- **DDL 整備済み**: `match_results` 保存、`interaction_logs` + `feedback_events`（統一版）のテーブルと、Phase4 向け `training_pairs` / `training_stats` ビューまで設計済み。
- **LLM ワーカーの環境変数ドリブン動作**: `LLM_ENABLED` で完全 OFF、`LLM_PROVIDER`/`LLM_MODEL`/`LLM_ENDPOINT`/`LLM_API_KEY` で実プロバイダを差し替え、`LLM_COMPARE_MODE=shadow` + `LLM_SHADOW_*` で 10% サンプリングの影比較ログを出力できる。`LLM_TIMEOUT_SECONDS` などリトライ・タイムアウトも環境変数で制御。
- **systemd 運用準備**: 常駐ループ（アイドルポーリング）とキューのカナリア記録を追加済み。`--exit-on-empty` で単発実行も可能。

---

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────────┐
│                        sr-gui (Next.js)                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │Queue Dashboard│  │案件→候補一覧 │  │候補詳細 + Feedback   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
└─────────┼─────────────────┼─────────────────────┼───────────────┘
          │                 │                     │
          ▼                 ▼                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                      sr-api (Axum REST)                         │
│  GET /queue/dashboard  GET /projects/:id/candidates  POST /feedback │
└─────────────────────────────┬───────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│ sr-extractor │    │sr-llm-worker │    │sr-queue-recov│
│  (5分timer)  │    │  (常駐)      │    │ (10分timer)  │
└──────┬───────┘    └──────┬───────┘    └──────┬───────┘
       │                   │                   │
       └───────────────────┼───────────────────┘
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                      sr-common (Library)                         │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐   │
│  │ corrections│ │  matching  │ │  two_tower │ │    api     │   │
│  │(正規化)    │ │(KO+Score)  │ │(HashTower) │ │(DTO/Config)│   │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘   │
└─────────────────────────────┬───────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL (ses.*)                          │
│  extraction_queue │ match_results │ interaction_logs │ feedback │
└─────────────────────────────────────────────────────────────────┘
```

---

## データ設計

### テーブル関係

```
interaction_logs (Exposure)          feedback_events (Label)
┌────────────────────────┐           ┌────────────────────────┐
│ id                     │◄──────────│ interaction_id (FK)    │
│ talent_id              │           │ feedback_type          │
│ project_id             │           │   thumbs_up/down       │
│ two_tower_score        │           │   review_ok/ng/pending │
│ business_score         │           │   accepted/rejected    │
│ created_at             │           │ ng_reason_category     │
└────────────────────────┘           │ actor / source         │
         │                           └────────────────────────┘
         │                                      │
         └──────────────┬───────────────────────┘
                        ▼
              training_pairs (VIEW)
              ┌────────────────────────┐
              │ interaction_id         │
              │ two_tower_score        │
              │ label (1.0/0.8/0.0)    │  → Phase 4: Two-Tower 学習
              └────────────────────────┘
```

### フィードバック統一ENUM

| feedback_type | ソース | label |
|---------------|--------|-------|
| `thumbs_up` | GUI | 1.0 |
| `thumbs_down` | GUI | 0.0 |
| `review_ok` | GUI | 1.0 |
| `review_ng` | GUI | 0.0 |
| `accepted` | 営業 | 1.0 |
| `rejected` | 営業 | 0.0 |
| `interview_scheduled` | 営業 | 0.8 |

---

## Two-Tower 設計

Phase 3 で骨格を仕込み、Phase 4 で学習。

```
┌─────────────────┐    ┌─────────────────┐
│  Talent Tower   │    │  Project Tower  │
│  (HashTwoTower) │    │  (HashTwoTower) │
└────────┬────────┘    └────────┬────────┘
         │                      │
         └──────────┬───────────┘
                    ▼
              Cosine Similarity
                    │
                    ▼
         total_score に加重合成
         (MVP: weight=0.0 で無効)
```

**不変条件**:
- HardKo は常に勝つ（Two-Tower スコアが高くても覆らない）
- Two-Tower は順位づけ（KO判定ではない）

---

## GUI 技術スタック

| レイヤー | 選定 |
|----------|------|
| フロントエンド | Next.js 14 + TypeScript |
| UI | shadcn/ui (Tailwind CSS) |
| 状態管理 | TanStack Query |
| バックエンド | Axum (Rust) |
| 認証 | NextAuth.js (Google OAuth) |

---

## クイックスタート

```bash
# テスト実行
cargo test

# ビルド
cargo build --release

# ワーカー起動（開発）
./target/release/sr-llm-worker --db-url $DATABASE_URL

# LLM ワーカー向け環境変数のセット例（export または .env で設定）
export DATABASE_URL=postgres://user:pass@host/db
export LLM_ENABLED=1                          # 0 にすると LLM を完全停止（KO/スコアだけで処理）
export LLM_PROVIDER=deepseek                  # 既定: deepseek（LLM_PRIMARY_PROVIDER が未指定ならこれが primary）
export LLM_MODEL=deepseek-chat                # プロバイダ未指定時の既定モデル。プロバイダ毎の既定は下表を参照
export LLM_ENDPOINT=http://localhost:8000/api/v1/extract # プロバイダ未指定時の既定エンドポイント
export LLM_API_KEY=your-token
export LLM_TIMEOUT_SECONDS=30                 # 既定: 30 秒
export LLM_MAX_RETRIES=3                      # 既定: 3 回
export LLM_RETRY_BACKOFF_SECONDS=5            # 既定: 5 秒
export LLM_COMPARE_MODE=shadow                # none/shadow（既定: none）
export LLM_PRIMARY_PROVIDER=deepseek          # 既定: LLM_PROVIDER と同じ
export LLM_SHADOW_PROVIDER=openai             # 影比較先プロバイダ（既定: openai）
export LLM_SHADOW_API_KEY=shadow-token        # 影比較の API キー（未設定可）
export LLM_SHADOW_SAMPLE_PERCENT=10           # 0-100（既定: 10、100 で常に影比較）
export AUTO_MATCH_THRESHOLD=0.7               # MatchResponse 変換用の自動承認閾値
export TWO_TOWER_ENABLED=false
```

### プロバイダ別の実動エンドポイント/既定モデル

`LLM_PROVIDER` を設定すると、`LLM_MODEL` と `LLM_ENDPOINT` の未指定時は下表の値が自動で入ります（公式 API エンドポイントに準拠）。

| LLM_PROVIDER | 既定モデル | 既定エンドポイント（公式） |
|--------------|------------|---------------------------|
| `openai` | `gpt-4o-mini` | `https://api.openai.com/v1/chat/completions` |
| `anthropic` | `claude-3-5-sonnet-20240620` | `https://api.anthropic.com/v1/messages` |
| `google` / `google-genai` | `gemini-1.5-flash` | `https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent` |
| `mistral` | `mistral-large-latest` | `https://api.mistral.ai/v1/chat/completions` |
| `xai` | `grok-2-latest` | `https://api.x.ai/v1/chat/completions` |
| `huggingface` / `hf` | `meta-llama/Meta-Llama-3-70B-Instruct` | `https://api-inference.huggingface.co/models/meta-llama/Meta-Llama-3-70B-Instruct` |
| `deepseek`（既定） | `deepseek-chat` | `http://localhost:8000/api/v1/extract` |

たとえば Google Vertex AI (Gemini) を使う場合は以下のように上書きします。

```bash
export LLM_PROVIDER=google-genai
export LLM_API_KEY=$GOOGLE_API_KEY
# model/endpoint 未指定なら gemini-1.5-flash / Google Generative Language API を自動適用
```

### LLM ワーカーの挙動（環境変数に紐づく動作）

- **停止/バイパス**: `LLM_ENABLED=0` で LLM 呼び出しをスキップし、キューには `LLM_DISABLED` のメッセージだけを残す。
- **プロバイダ切替**: `LLM_PROVIDER` と `LLM_MODEL` でメイン呼び出し先を変更。`LLM_PRIMARY_PROVIDER` を別途指定すると、実呼び出しとログ上の primary ラベルを分離できる。
- **影比較 (shadow)**: `LLM_COMPARE_MODE=shadow` + `LLM_SHADOW_PROVIDER`/`LLM_SHADOW_API_KEY` を設定すると、`LLM_SHADOW_SAMPLE_PERCENT` の割合でカナリアログを記録し、primary/shadow 双方のプロバイダ名を保存する。
- **リトライ/タイムアウト**: `LLM_TIMEOUT_SECONDS`、`LLM_MAX_RETRIES`、`LLM_RETRY_BACKOFF_SECONDS` で REST 呼び出しのタイムアウトとリトライ間隔を細かく調整可能。

---

## ディレクトリ構成

```
crates/
├── sr-common/          # 共通ライブラリ
│   ├── corrections/    # 正規化ロジック
│   ├── matching/       # KO判定 + スコアリング
│   ├── two_tower/      # Two-Tower (HashTwoTower)
│   ├── api/            # DTO (MatchResponse, FeedbackRequest)
│   ├── queue/          # extraction_queue モデル
│   └── db/             # DB操作ヘルパー
├── sr-extractor/       # メール抽出 → キュー投入
├── sr-llm-worker/      # LLM処理ワーカー
├── sr-queue-recovery/  # 滞留ジョブ復旧
├── sr-api/             # HTTP API (Axum) ← NEW
└── sr-gui/             # フロントエンド (Next.js) ← NEW

docs/
└── MVP_PLAN.md         # 詳細設計書（15,000行+）
```

---

## 詳細ドキュメント

すべての詳細設計は `docs/MVP_PLAN.md` に集約:
- DDL定義（match_results, feedback_events, interaction_logs）
- LLM Provider 実装（8プロバイダ対応）
- Two-Tower 詳細設計（TwoTowerEmbedder trait, HashTwoTower）
- GUI 契約層（MatchResponse, FeedbackRequest, QueueDashboard）
- HTTP API エンドポイント一覧
- Done 条件チェックリスト
