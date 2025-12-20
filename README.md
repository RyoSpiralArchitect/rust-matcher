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
- **価格計算**: 単価関連のパラメータとタレント/案件別の計算ユーティリティを追加。
- **日付正規化**: 受領日時を基準に開始日のテキストを ASAP/日付/四半期/応相談まで丸め、`DatePrecision` と注釈で解像度を明示。
- **エントリポイント**: 3 バイナリを追加（`sr-extractor`・`sr-llm-worker`・`sr-queue-recovery`）。現時点ではスタブ実装で、
  共通ロジックは `sr-common` クレート経由で利用。

詳細な MVP までの計画書は `docs/MVP_PLAN.md` に移動しました。

## 未達成・スタブ・今後やること
MVP_PLAN.md の進行表に沿って、以下は未着手/スタブまたは今後深掘りする項目です。

- **抽出系バイナリの実装充実**: `sr-extractor` / `sr-llm-worker` / `sr-queue-recovery` は最小限のスタブフロー。実サービス I/O や
  エラーハンドリング、LLM 呼び出しなどは未実装。
- **永続化まわり**: キュー/スキーマはインメモリ想定で、外部ストレージやマイグレーションは未実装。DDL の index など
  物理設計は `docs/MVP_PLAN.md` の方針に従って今後適用。
- **スコアリングの運用統合**: `MatchingEngine` で prefilter+詳細スコアの一貫処理は実装済み。実際の I/O やバイナリからの呼び出し、
  E2E テストの拡充や閾値チューニングはこれから。
- **抽出ルールの拡充**: regex ベースの抽出は主要項目のみ。MVP_PLAN の残り（例: 契約形態詳細、追加スキル群など）は順次追加予定。
- **運用フローの整備**: Queue の監視/リカバリ、決定理由のログ出力、Cargo.lock の扱いなど、運用面の方針は `docs/MVP_PLAN.md`
  へ寄せつつ README でも適宜アップデート予定。

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
