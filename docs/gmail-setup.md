# Gmail Ingestor セットアップガイド

`sr-gmail-ingestor` は Google Workspace の Gmail API を直接使用してメールを取り込むコンポーネントです。
サービスアカウント + Domain-Wide Delegation (DWD) を利用し、指定ユーザーのメールボックスをポーリングします。

---

## 前提条件

- Google Workspace アカウント（個人 Gmail では DWD が使用不可）
- Google Cloud プロジェクト
- Workspace 管理者権限（DWD 設定用）

---

## 1. Google Cloud プロジェクトの設定

### 1.1 Gmail API の有効化

```bash
# gcloud CLI で有効化
gcloud services enable gmail.googleapis.com --project=YOUR_PROJECT_ID
```

または [Google Cloud Console](https://console.cloud.google.com/apis/library/gmail.googleapis.com) から有効化。

### 1.2 サービスアカウントの作成

```bash
# サービスアカウント作成
gcloud iam service-accounts create sr-gmail-ingestor \
  --display-name="SR Gmail Ingestor" \
  --project=YOUR_PROJECT_ID

# JSON キーの生成
gcloud iam service-accounts keys create /path/to/gcp-sa.json \
  --iam-account=sr-gmail-ingestor@YOUR_PROJECT_ID.iam.gserviceaccount.com
```

**重要**: 生成された JSON ファイルは安全な場所に保管してください。

---

## 2. Domain-Wide Delegation (DWD) の設定

### 2.1 サービスアカウントの Client ID を取得

[IAM & Admin > Service Accounts](https://console.cloud.google.com/iam-admin/serviceaccounts) で作成したサービスアカウントを選択し、**Client ID** (数字のみ) をコピーします。

### 2.2 Google Workspace Admin Console で DWD を許可

1. [admin.google.com](https://admin.google.com) にログイン
2. **セキュリティ** → **アクセスとデータ管理** → **API の制御** → **ドメイン全体の委任を管理**
3. **API クライアントを追加** をクリック
4. 以下を入力:
   - **クライアント ID**: 上記でコピーした数字
   - **OAuth スコープ**:
     ```
     https://www.googleapis.com/auth/gmail.readonly
     ```
5. **承認** をクリック

### スコープの説明

| スコープ | 説明 |
|----------|------|
| `gmail.readonly` | メールの読み取り専用（送信・削除は不可） |

---

## 3. 環境変数の設定

```bash
# 必須
export DATABASE_URL="postgres://user:pass@host:5432/dbname"
export GWS_SERVICE_ACCOUNT_KEY="/path/to/gcp-sa.json"
export GWS_IMPERSONATE_USER="target-user@your-domain.com"

# オプション
export GWS_POLL_INTERVAL_SECONDS=60           # ポーリング間隔（デフォルト: 60秒）
export GWS_ANKEN_QUERY="label:partner -has:attachment"  # 案件メールのクエリ
export GWS_JINZAI_QUERY="label:partner has:attachment"  # 人材メールのクエリ
```

### 環境変数一覧

| 変数名 | 必須 | デフォルト | 説明 |
|--------|------|------------|------|
| `DATABASE_URL` | Yes | - | PostgreSQL 接続文字列 |
| `GWS_SERVICE_ACCOUNT_KEY` | Yes | - | サービスアカウント JSON のパス |
| `GWS_IMPERSONATE_USER` | Yes | - | 委任対象のメールアドレス |
| `GWS_POLL_INTERVAL_SECONDS` | No | `60` | ポーリング間隔（秒） |
| `GWS_ANKEN_QUERY` | No | `label:partner -has:attachment` | 案件メール検索クエリ |
| `GWS_JINZAI_QUERY` | No | `label:partner has:attachment` | 人材メール検索クエリ |

---

## 4. Gmail ラベルの設定

効率的なメール取り込みのため、Gmail 側でラベルを設定することを推奨します。

### 推奨ラベル構成

```
partner/           # BP各社からのメール
├── anken         # 案件紹介（添付なし）
└── jinzai        # 人材紹介（スキルシート添付あり）
```

### フィルタの例

Gmail の設定 → フィルタとブロック中のアドレス → 新しいフィルタを作成:

```
From: *@bp-company.co.jp
→ ラベル "partner" を付ける
```

### クエリの例

| 目的 | クエリ |
|------|--------|
| 添付なしパートナーメール | `label:partner -has:attachment` |
| 添付ありパートナーメール | `label:partner has:attachment` |
| 特定送信者 | `from:specific@bp.co.jp` |
| 日付指定 | `after:2025/01/01` |

---

## 5. 起動方法

### 開発環境

```bash
cargo run -p sr-gmail-ingestor -- \
  --db-url "$DATABASE_URL" \
  --sa-key-path /path/to/gcp-sa.json \
  --impersonate-user ingest@your-domain.com
```

### 本番環境（systemd）

```ini
# /etc/systemd/system/sr-gmail-ingestor.service
[Unit]
Description=SR Gmail Ingestor
After=network.target postgresql.service

[Service]
Type=simple
User=sr
Group=sr
EnvironmentFile=/etc/sr/gmail-ingestor.env
ExecStart=/opt/sr/bin/sr-gmail-ingestor
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
# 環境変数ファイル
# /etc/sr/gmail-ingestor.env
DATABASE_URL=postgres://...
GWS_SERVICE_ACCOUNT_KEY=/etc/sr/gcp-sa.json
GWS_IMPERSONATE_USER=ingest@your-domain.com
GWS_POLL_INTERVAL_SECONDS=60
```

```bash
sudo systemctl enable sr-gmail-ingestor
sudo systemctl start sr-gmail-ingestor
sudo journalctl -u sr-gmail-ingestor -f
```

---

## 6. データフロー

```
Gmail Inbox
    │
    ▼ (Gmail API + DWD)
┌─────────────────────────────────┐
│      sr-gmail-ingestor          │
│  ┌───────────────────────────┐  │
│  │ GWS_ANKEN_QUERY           │──┼──► ses.anken_emails
│  │ (label:partner -has:att)  │  │
│  └───────────────────────────┘  │
│  ┌───────────────────────────┐  │
│  │ GWS_JINZAI_QUERY          │──┼──► ses.jinzai_emails
│  │ (label:partner has:att)   │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
    │
    ▼ (既存フロー)
sr-extractor → extraction_queue → sr-llm-worker
```

---

## 7. トラブルシューティング

### よくあるエラー

#### `failed to load service account key`

```
Error: failed to load service account key: No such file or directory
```

**原因**: `GWS_SERVICE_ACCOUNT_KEY` のパスが間違っている
**対処**: ファイルパスを確認し、アクセス権限があることを確認

#### `Insufficient Permission`

```
Error: gmail api error: Insufficient Permission
```

**原因**: DWD スコープが不足している、または設定されていない
**対処**:
1. Admin Console で DWD の設定を確認
2. スコープに `https://www.googleapis.com/auth/gmail.readonly` があることを確認
3. 設定変更後、最大24時間かかる場合がある

#### `User not found`

```
Error: User not found: target@domain.com
```

**原因**: `GWS_IMPERSONATE_USER` が存在しない、または Workspace ドメインにない
**対処**: 正しいメールアドレスを指定

#### `Connection refused (postgres)`

```
Error: database pool error: Connection refused
```

**原因**: PostgreSQL に接続できない
**対処**: `DATABASE_URL` の接続情報を確認

### デバッグログの有効化

```bash
RUST_LOG=debug cargo run -p sr-gmail-ingestor -- ...
```

---

## 8. セキュリティ考慮事項

- **サービスアカウント JSON は秘密情報として扱う**
  - Git にコミットしない
  - 適切なファイルパーミッション（`chmod 600`）を設定
  - 本番では Secret Manager を使用することを推奨

- **DWD は最小権限の原則に従う**
  - `gmail.readonly` のみ付与（メール送信権限は不要）
  - 必要なユーザーのみ対象にする

- **ネットワーク**
  - PostgreSQL への接続は SSL を使用
  - Gmail API は HTTPS のみ

---

## 参考リンク

- [Gmail API ドキュメント](https://developers.google.com/gmail/api)
- [Domain-Wide Delegation 設定ガイド](https://developers.google.com/identity/protocols/oauth2/service-account#delegatingauthority)
- [Gmail 検索演算子](https://support.google.com/mail/answer/7190)
