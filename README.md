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
| 3.5 | A | HTTP API (Axum) — QueueDashboard / MatchResponse / Feedback | ⏳ 待機 |
| 3.5 | B | MatchResponse DTO + MatchConfig | ✅ 完了 |
| 3.5 | C | QueueDashboard DTO | ✅ 完了 |
| 3.5 | D | feedback_events DDL（統一版） | ✅ 完了 |
| 3.5 | E | **GUI (Next.js)** | 🎯 **ゴール** |
| 4 | - | Two-Tower 学習（interaction_logs + feedback_events 蓄積後） | 🔜 将来 |

---

## 現状できること（MVP準備中のサマリ）

- **直人材パースを最優先で確定**: メール本文（将来は PDF も）からタレント情報を正規化し、案件側は「最低限の正規化 + KO/スコア算出」で運用開始できるよう契約を固め済み。
- **マッチング結果の契約を確定**: `MatchResponse` / `MatchConfig` / `QueueDashboard` DTO を sr-common に実装済み。フロントはこの契約に沿って開発着手可能。
- **DDL 整備済み**: `match_results` 保存、`interaction_logs` + `feedback_events`（統一版）のテーブルと、Phase4 向け `training_pairs` / `training_stats` ビューまで設計済み。
- **LLM ワーカーの環境変数ドリブン動作**: `LLM_ENABLED` で完全 OFF、`LLM_PROVIDER`/`LLM_MODEL`/`LLM_ENDPOINT`/`LLM_API_KEY` で実プロバイダを差し替え、`LLM_COMPARE_MODE=shadow` + `LLM_SHADOW_*` で 10% サンプリングの影比較ログを出力できる。`LLM_TIMEOUT_SECONDS` などリトライ・タイムアウトも環境変数で制御。
- **systemd 運用準備**: 常駐ループ（アイドルポーリング）とキューのカナリア記録を追加済み。`--exit-on-empty` で単発実行も可能。

## ここまでの達成とこれからやること（まとめ）

### これまでにやったこと

- **バックエンドの下地を固めた**: sr-common の DTO（`MatchResponse`/`MatchConfig`/`QueueDashboard`）や DB テーブル群（`interaction_logs`・`feedback_events`・`match_results` など）が揃い、Axum ベースの sr-api を新設済み。API キー or JWT の二段構えで認証できる足場を作ってある。
- **ワーカーと運用の準備が整った**: LLM ワーカーは環境変数だけで ON/OFF やプロバイダ切替、影比較（shadow）まで制御でき、systemd 常駐や単発実行フラグで本番運用を想定した動作が確認済み。
- **フロントと話せる契約を明文化**: GUI 側が依存するレスポンス形式（候補一覧の `interaction_id` を含むなど）と、フィードバックの冪等性ルール（同じ interaction に対する重複 POST は無視）を設計・実装しているので、Next.js 側はこの契約を前提に作り込みできる。
- **CORS やトレース設定も用意済み**: 環境変数で CORS 許可オリジンを切り替え、`TraceLayer` でリクエストログも出るため、デプロイ直後から最低限の運用監視が効く。

#### 達成物の詳細（着地イメージ）

- **現在の到達点**
  - REST API の土台は立ち上がっており、`/health` で疎通確認、`/api/*` で API キー経由の認証パスを用意済み。
  - 「候補一覧レスポンスに `interaction_id` を必ず含める」「feedback は同じ人が同じ種類を重複送っても無視する」といった、GUI と運用が安心して使える取り決めをコードに反映済み。
  - LLM ワーカーは「止めておく／1プロバイダで回す／primary+shadow の2系統で比較する」を全部 ENV だけで切り替え可能。試したい人が、コードをいじらず動作モードを変えられる状態。

- **既にインストール不要で試せること**
  - `cargo run -p sr-api -- --help` で起動オプションが一覧でき、PORT や CORS を環境変数で差し替えるだけでローカル起動可能。
  - LLM 側は「LLM_ENABLED=0 で完全停止」「LLM_PROVIDER/LLM_MODEL でプロバイダ切替」「LLM_COMPARE_MODE=shadow で影比較ON」といったスイッチを README のサンプル通りに切り替えれば即座に挙動が変わる。

- **運用時の安全ネット**
  - 認証は `AUTH_MODE=api_key`（既定）と `AUTH_MODE=jwt` のトグル設計で、GUI リリースタイミングに合わせてヘッダーを切り替えるだけ。
  - JWT はデフォルト `JWT_ALGORITHM=hs512`（NextAuth 既定に合わせた対称鍵）で動作し、RSA/EC/EdDSA の公開鍵検証も `JWT_PUBLIC_KEY` を渡せば切り替え可能。対称鍵の場合は `JWT_SECRET`、非対称鍵の場合は公開鍵 PEM だけあれば起動できる。
  - NextAuth 側の JWT は設定次第で JWE（暗号化）や鍵種が変わるため、GUI 連携フェーズでは「NextAuth が出すトークン形式に API 側を合わせる」か「API 側で署名した JWT を Next 側がそのまま送る」方針を事前に決めておくと安全。
  - feedback の INSERT は (interaction_id, feedback_type, actor) の組み合わせで一意。重複は自動でスキップするため、誤って何度送っても DB が壊れない。
  - すべてのリクエストに `TraceLayer` を適用済み。X-Request-Id でログを追えるので、障害調査や遅延調査の初動が簡単。

### これから着手すること（順番の目安つき）

1. **API エンドポイントの Phase 1 を仕上げる（優先）**
   - `/health` は素通し、`/api/queue/dashboard`・`/api/projects/{id}/candidates`・`/api/feedback` を API キー認証付きで仕上げる。
   - DB 層は sr-common の `db/queue_dashboard.rs` / `db/candidates.rs` / `db/feedback.rs` を経由し、HTTP 変換は sr-api 側で行う方針を維持。
2. **GUI 連携のための JWT モードを確認（Phase 2 入口）**
   - `AUTH_MODE=jwt` 時の設定・起動確認を行い、NextAuth 連携の導線を用意する。X-API-Key とのトグル挙動を README に追記予定。
3. **Queue ジョブの詳細系 API を拡張**
   - `/api/queue/jobs` と `/api/queue/jobs/:id`、`/api/queue/retry/:id` を追加し、運用ダッシュボードから再実行できるようにする（Phase 2 以降）。
4. **GUI との結線と動作確認**
   - Next.js 側で Queue Dashboard と候補一覧の画面を立て、`interaction_id` をそのまま feedback POST に渡す流れを end-to-end で確認。CORS/認証設定のデフォルトを README に反映。
5. **Two-Tower 学習の準備と計測**
   - `training_pairs` ビューを元に学習ジョブを試し、影比較ログを活用してモデル差分を計測。フィードバック蓄積量に応じて本番反映タイミングを判断する。

#### 各ステップで「終わった」と言える状態

- **Phase 1 API**: curl か Postman で 3 つのエンドポイントを叩き、API キーが無いと 401、正しいキーだとレスポンスが返ることを全員が確認できる。
- **JWT モード**: AUTH_MODE を `jwt` にした状態で sr-api が起動し、GUI からの Bearer トークンで認証が通るデモが 1 回できる。
- **Queue 拡張**: `/api/queue/jobs` 系で「一覧が返る」「個別 detail が返る」「retry を叩くと実行される」を運用チームがブラウザか curl で確認できる。
- **GUI 結線**: Next.js で Queue Dashboard と候補一覧が表示され、各候補の `interaction_id` をそのまま POST するフィードバック送信が通るデモ動画（もしくはステージング環境）がある。
- **Two-Tower 計測**: `training_pairs` を入力に簡易学習を 1 度回し、影比較ログと合わせて「精度が上がった/変わらない」のコメントを残せる。

### ポイント

- **「今もう動く？」に対する答え**: バックエンドの基盤はほぼ揃っており、API キー認証での MVP 動作は実装着手済み。GUI がそろえば内部利用を開始できる段階。
- **「何を待っている？」の整理**: フロントの画面実装と、API 側の Phase 1 エンドポイントを詰める作業がメイン。運用目線では CORS 設定と環境変数の詰めが残タスク。
- **「安全側の配慮は？」**: 認証トグル（API キー/JWT）、feedback の冪等性（重複は INSERT しない）、X-Request-Id を用いたトレースで、最初のデプロイから最低限の安全性と追跡性を担保。

#### 役割と合流ポイント（誰がいつ関与するか）

- **バックエンド担当**: `/api/*` の 3 本（dashboard / candidates / feedback）を Phase 1 の完了ラインまで実装し、curl サンプルを README に残す。JWT モードの起動確認も担当。
- **フロントエンド担当**: Next.js で Queue Dashboard + 候補一覧 + Feedback 入力画面を組み、`interaction_id` をそのまま送る形で API と結線。API キー/JWT どちらでも動くよう .env.example を準備。
- **営業/運用担当**: feedback 入力の運用ルール（誰がいつ thumbs_up / thumbs_down を押すか、コメント必須か）を定義し、運用手順書を整備。CORS 設定や API キー/JWT 配布の窓口を決める。
- **LLM/ML 担当**: training_pairs ビューを元にした学習・影比較のサンプルノートブックを用意し、Phase 4 に備えて「精度がどこで効いてくるか」を測定できるようにする。

#### すぐ試せるデモ手順
1. `.env.example` をコピーして `SR_API_KEY` と `DATABASE_URL` だけを埋め、`cargo run -p sr-api` を実行（PORT=3001 で起動）。
2. 別ターミナルで `/health` を叩く: `curl http://localhost:3001/health` → `{ "status": "ok" }` が返ればサーバー起動 OK。
3. API キー付きで dashboard を叩く: `curl -H "X-API-Key: $SR_API_KEY" http://localhost:3001/api/queue/dashboard` → JSON が返るか確認。
4. GUI 側は `.env.local` に `NEXT_PUBLIC_API_ORIGIN=http://localhost:3001` と API キー or JWT 設定を入れ、候補一覧表示 → フィードバック送信までひととおりクリックしてみる。
5. ログを追う: サーバー側ログに X-Request-Id が出るので、障害調査や遅延時にその ID を伝えるとバックエンド側で追跡できる。

#### FAQ

- **Q. API キーと JWT、どちらを本番で使う？** → MVP では API キー。GUI と結線するタイミングで JWT へ切り替え。AUTH_MODE で即トグルできる。
- **Q. LLM は止めても大丈夫？** → `LLM_ENABLED=0` にすれば KO/スコア算出のみで動き、既存ロジックだけで運用可。影比較も ENV だけで ON/OFF 切り替え。
- **Q. 重複送信や誤送信のリスクは？** → feedback は (interaction_id, feedback_type, actor) の組み合わせで一意に管理。重複は無視され、DB が汚れない設計。
- **Q. 依存する外部サービスは？** → PostgreSQL と LLM プロバイダ（deepseek 既定）。LLM なし運用も可能なので、データベースさえあれば最低限の機能は動く。
- **Q. ロールアウト手順は？** → ローカルで API + GUI を繋いでデモ → ステージングで CORS/認証設定を確認 → 本番で API キー配布 or JWT 切替を実施、の 3 ステップ。

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

- `interaction_logs` = 「誰に何を候補として見せたか」の露出ログ。必ず `match_run_id`（engine_version / config_version を内包）を保存して再現性を確保。
- `feedback_events` = 「どの露出に対して誰がどんなフィードバックをしたか」のラベル。`actor`/`source` を残し、`is_revoked` で取り消し履歴も保持。
- `training_pairs` = 上記 2 テーブルを束ねた View。Phase 4 の Two-Tower 学習ではここから教師データを作る。

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
export LLM_SHADOW_API_KEY=shadow-token        # 影比較の API キー（未設定可、未設定時は影プロバイダ専用の env を自動検索）
export LLM_SHADOW_SAMPLE_PERCENT=10           # 0-100（既定: 10、100 で常に影比較）
export AUTO_MATCH_THRESHOLD=0.7               # MatchResponse 変換用の自動承認閾値
export TWO_TOWER_ENABLED=false
```

### プロバイダ別のデフォルト設定（sr-llm-worker が想定する前提）

- **Mode A: Unified Gateway（推奨）**: `LLM_ENDPOINT` に自前ゲートウェイ（例: `/api/v1/extract` 互換）を立て、プロバイダ差分を吸収する。
- **Mode B: Direct Provider（上級者向け）**: `LLM_PROVIDER`/`LLM_MODEL`/`LLM_ENDPOINT` を直接指定し、sr-llm-worker 内部のプロバイダ別アダプタでリクエストを組み立てる。

`LLM_PROVIDER` を設定すると、`LLM_MODEL` と `LLM_ENDPOINT` の未指定時は下表の値が自動で入ります（リクエスト互換性は上記モード前提）。

| LLM_PROVIDER | 既定モデル | 既定エンドポイント |
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

### プロバイダ別の API キー環境変数解決

`LLM_API_KEY` と `LLM_SHADOW_API_KEY` が未設定の場合でも、以下のプロバイダ固有の環境変数があれば自動で読み込みます。

| プロバイダ | 優先して読む環境変数 |
|------------|----------------------|
| `openai` | `OPENAI_API_KEY` |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `google` / `google-genai` | `GOOGLE_API_KEY` |
| `mistral` | `MISTRAL_API_KEY` |
| `xai` | `XAI_API_KEY` |
| `huggingface` / `hf` | `HUGGINGFACE_API_KEY` → なければ `HF_TOKEN` |

影比較中 (`LLM_COMPARE_MODE=shadow`) は `LLM_SHADOW_PROVIDER` に対応するキーを同様に検索します。`LLM_API_KEY`/`LLM_SHADOW_API_KEY` を明示的に指定すればそれが最優先です。同一プロバイダで shadow を動かす場合は、`LLM_SHADOW_API_KEY` が空でも primary 用のキーを自動で再利用します。

### LLM ワーカーの挙動（環境変数に紐づく動作）

- **停止/バイパス**: `LLM_ENABLED=0` で LLM 呼び出しをスキップし、キューには `LLM_DISABLED` のメッセージだけを残す。
- **プロバイダ切替**: `LLM_PROVIDER` と `LLM_MODEL` でメイン呼び出し先を変更。`LLM_PRIMARY_PROVIDER` を別途指定すると、実呼び出しとログ上の primary ラベルを分離できる。
- **影比較 (shadow)**: `LLM_COMPARE_MODE=shadow` + `LLM_SHADOW_PROVIDER`/`LLM_SHADOW_API_KEY` を設定すると、`LLM_SHADOW_SAMPLE_PERCENT` の割合でカナリアログを記録し、primary/shadow 双方のプロバイダ名を保存する。
- **リトライ/タイムアウト**: `LLM_TIMEOUT_SECONDS`、`LLM_MAX_RETRIES`、`LLM_RETRY_BACKOFF_SECONDS` で REST 呼び出しのタイムアウトとリトライ間隔を細かく調整可能。

### ingestion はプラガブル（n8n / Gmail API）

- 現状は「DB に既にメールが入っている前提」のまま進め、n8n 等のワークフローから `anken_emails` を投入する運用がデフォルト。
- **Google Cloud 直結ルート（DWD + Gmail API）が追加済み**: `sr-gmail-ingestor` がサービスアカウント経由で Gmail をポーリングし、`ses.anken_emails` / `ses.jinzai_emails` に直接投入する。Pub/Sub Watch は後続拡張予定だが、まずは認証がシンプルなポーリングのみで運用開始できる。
  - 必須 env: `DATABASE_URL`、`GWS_SERVICE_ACCOUNT_KEY`（サービスアカウント JSON）、`GWS_IMPERSONATE_USER`（DWD の対象ユーザー）
  - 想定トラフィック: 1 日 1000 通を超えるスパイクにも耐える前提。`max_results=500` で 1 ページ 500 通まで取得し、`GWS_MAX_PAGES_PER_POLL`（デフォルト 20 ページ = 最大 ~10,000 通）で 1 回のポーリング上限を緩めた。負荷を抑えたい環境では env で下げられる。
  - 任意 env: `GWS_POLL_INTERVAL_SECONDS`（デフォルト 60 秒）、`GWS_MAX_PAGES_PER_POLL`（デフォルト 20）、`GWS_ANKEN_QUERY` / `GWS_JINZAI_QUERY`（Gmail の検索クエリ、ラベルや添付有無で切り分け）
  - 起動例: `cargo run -p sr-gmail-ingestor -- --db-url $DATABASE_URL --sa-key-path /etc/sr/gcp-sa.json --impersonate-user ingest@example.com`
- 将来は `sr-extractor` から Gmail API を直接叩いて `anken_emails` を埋める構成にも切り替え可能にする方針。環境変数で n8n ルート／Gmail 直結のどちらも選べる形を維持する。

---

## ディレクトリ構成

Rust workspace crates（Cargo 管理）

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
├── sr-gmail-ingestor/  # Gmail API 直結（Google Cloud / Service Account）
└── sr-api/             # HTTP API (Axum)
```

フロントエンド（別 npm プロジェクト）

```
sr-gui/                 # Next.js 14 + TypeScript
```

ドキュメント

```
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

### セットアップガイド

- [Gmail Ingestor セットアップ](docs/gmail-setup.md) - Google Workspace 連携の設定手順
