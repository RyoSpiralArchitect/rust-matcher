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

# 環境変数例
DATABASE_URL=postgres://user:pass@host/db
LLM_PROVIDER=deepseek
AUTO_MATCH_THRESHOLD=0.7
TWO_TOWER_ENABLED=false
```

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
