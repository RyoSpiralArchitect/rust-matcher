# ログ収集・運用ガイド

このプロジェクトはデフォルトで `tracing` の構造化ログを stdout に出力し、必要に応じて日次ローテーション（ファイル出力）に切り替えられます。本ガイドは、ログを集約基盤へ送る際の設定と、長時間稼働するワーカーが出す業務メトリクスログの扱いをまとめたものです。

## アプリケーションログの転送

- **デフォルト**: stdout/stderr に出力。Fluent Bit / Vector / Filebeat などのコレクタをプロセスの stdout ストリームに向け、`application`・`thread_name`・`location`・`panic_message` フィールドを含める。
- **ファイルローテーション**: `SR_LOG_DIR=/var/log/rust-matcher`（任意の書き込み可能パスで可）を設定するとバイナリごとに日次ローテーション（`<app>.log`）する。ファイルベースで収集したい場合は、そのディレクトリを tail するようコレクタを設定する。
- **ELK**: stdout またはローテーションファイルを Logstash/Filebeat に送る。HTTP ルートには `X-Request-Id` を残し、リクエストトレースに使う。
- **Loki**: stdout を Promtail に渡す。ファイルローテーションを使う場合は `SR_LOG_DIR` を scrape 対象にする。保持すべきラベル: `application`、`worker_id`、`message_id`、`request_id`。
- **CloudWatch**: stdout を CloudWatch Agent / FireLens に流す。ファイルローテーション利用時は Agent 設定の `files` に `SR_LOG_DIR` を追加する。

## パニック時バックトレースの制御

`sr_common::logging::install_tracing_panic_hook` は、ノイズ低減のため本番ログでは Rust 標準のバックトレース出力を抑制します。調査時にバックトレースを出したい場合は次を設定してください:

```
SR_LOG_INCLUDE_BACKTRACE=true
```

これにより構造化されたパニックログを維持しつつ、標準フックによるスタックトレース印字を有効化できます。

## 業務メトリクスログ（LLM ワーカー）

`sr-llm-worker` は一定間隔で運用サマリを `INFO` に出力します:

- `jobs_total`: 起動後に処理したジョブ総数
- `worker_id`: CLI/環境変数で指定したワーカーラベル
- `successes`: 手動確認なしで完了した件数
- `permanent_failures`: 手動確認が必要な完了（致命的エラーなど）
- `retries_scheduled`: リトライキューに戻した件数
- `jobs_per_hour`: 実行時間ベースのスループット
- `success_rate`: `successes / max(1, jobs_total)` で算出した成功率

出力間隔は `SR_LOG_SUMMARY_INTERVAL_SECS`（デフォルト 300 秒）で制御します。Prometheus のメトリクスと合わせ、集約基盤でこれらのログを監視してください。
