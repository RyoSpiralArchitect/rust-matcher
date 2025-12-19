# rust-matcher

rust-matcher は、案件とタレントの情報を正規化し、KO 判定とスコアリングを一元化するマッチングエンジンです。Phase1 までで実装された範囲を中心に、現在の構成と使い方をまとめています。

## 現在の達成状況 (Phase1)
- **正規化レイヤ**: 都道府県/エリア/リモート設定、駅名、スキル、フロー深度・制限など、主要フィールドの補正を実装。
- **KO 判定とスコアリング**: `KoDecision` を単一ソースに、ロケーションやスキル一致度を考慮した KO/スコア算出を実装。
- **キュー処理**: `extraction_queue` ワーカーが pending → processing → completed を決定的順序で巡回し、リトライや manual review の判定を行う仕組みを実装。
- **価格計算**: 単価関連のパラメータとタレント/案件別の計算ユーティリティを追加。

詳細な MVP までの計画書は `docs/MVP_PLAN.md` に移動しました。

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
          |
          v
  集計 / 判定
  - KO 収束とスコア合算
          |
          v
  抽出キュー処理
  - extraction_queue worker
  - pending → processing → completed
```

## ディレクトリ構成
- `src/corrections/`: 都道府県・エリア・リモート・駅名・スキルなどの正規化ロジック。
- `src/matching/`: KO 判定 (`ko_unified`)、ロケーション評価、スキルマッチング、重み設定。
- `src/queue/`: `extraction_queue` のモデルとワーカー処理。
- `src/calculation/`: 単価パラメータと計算ユーティリティ。
- `src/date/`: 受領日時の解決など日付関連の補助機能。
- `docs/MVP_PLAN.md`: 旧 README。MVP までの詳細計画を保持。

## 開発のヒント
- テスト実行: `cargo test`
- 正規化や KO 判定の規約は README ではなく `docs/MVP_PLAN.md` にまとまっています。

