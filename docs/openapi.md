# API ドキュメント整備方針

機械可読な API ドキュメントは `docs/openapi.yaml`（OpenAPI 3.1）に集約済みです。日時フィールドはすべて RFC 3339 / ISO 8601 の `date-time` で明示し、ページネーション応答は `limit` / `offset` / `total` と `has_more` を統一的に返します。残タスクはこの仕様を提供するエンドポイントと Swagger UI の公開です。

- `/openapi.json`（もしくは `/openapi.yaml`）でスキーマを配信し、`/docs` に Swagger UI を載せてクライアント開発者がリクエスト/レスポンスの形を確認できるようにする。
- ルートごとに認証ヘッダー（`X-API-Key` または `Authorization: Bearer ...`）を明記し、`/api/v1/queue/retry/:id` やソーステキスト返却のような管理者限定操作には適切なスコープ/ロールを付与する。バージョン接頭辞と合わせて非推奨・破壊的変更の方針を記載し、`/api/*` エイリアスをどの程度維持するかを示す。

サーブ版のドキュメントを用意するまで、このファイルを OpenAPI へ反映すべき内容の正とする:

- **認証**: 既定は `X-API-Key` による API キー認証。設定次第で JWT モードもサポート。管理者限定ルート: キュー再実行、ソーステキスト返却。
- **ヘルスチェック**: `/livez`（プロセス存活）、`/readyz` および `/health`（DB 到達性込み。未準備の場合 503）。
- **Queue**:
  - `GET /api/v1/queue/dashboard` – 認証必須。ダッシュボード集計を返す。
  - `GET /api/v1/queue/jobs` – ページネーション（`limit` / `offset`）とフィルタ（`status`、`final_method`）付き一覧。
  - `GET /api/v1/queue/jobs/{id}` – `include` フラグと `limit`、`days`（上限 365）でマッチ/フィードバック履歴を制御。
  - `POST /api/v1/queue/retry/{id}` – 管理者のみ。
- **Candidates**: `GET /api/v1/projects/{project_id}/candidates`。`limit` / `offset` と `include_softko` オプション。マッチ結果単位で重複排除。
- **Feedback**: `POST /api/v1/feedback`。JSON ボディ（デフォルトのボディサイズ制限あり）を受け付け、認証済みユーザーを actor として扱う。

フォローアップ:
- `docs/openapi.yaml` を常に最新に保つ。
- CI で OpenAPI のバリデーションを追加する。
- `/docs` を公開し、ブラウザでインタラクティブに確認できるようにする。
