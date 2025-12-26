# SR Matcher GUI

マッチング候補の確認とフィードバック用 GUI

## Tech Stack

- Vite 7.x + React 19 + TypeScript
- Tailwind CSS 4 + shadcn/ui
- lucide-react (icons)

## Setup

```bash
# Install dependencies
npm install

# Copy environment file
cp .env.example .env.local

# Start dev server
npm run dev
```

## Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start dev server |
| `npm run build` | Production build |
| `npm run lint` | Run ESLint |
| `npm run preview` | Preview production build |

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `VITE_API_ORIGIN` | sr-api base URL | `http://localhost:3000` |
| `VITE_API_KEY` | API Key (Phase 1) | `your-api-key` |
| `VITE_JWT_TOKEN` | JWT for AUTH_MODE=jwt (set to prefer JWT over API key) | `eyJhbGciOiJIUzUxMiIs...` |

---

## Separation Rules (モノレポ分離ルール)

このフォルダは将来別リポジトリに移植可能なよう設計されている。

### 1. フォルダ完全独立

```
gui/
├── package.json        # 依存は gui 内で完結
├── package-lock.json   # ロックファイルも gui 配下
├── .env.example        # 環境変数も gui 内
└── src/
    └── api/            # sr-api との契約のみ参照
```

### 2. 共有物は「契約」だけ

- GUI が参照していいのは **sr-api の HTTP 契約（DTO/URL/認証方式）** のみ
- Rust 側のソースを直接参照しない
- 生成物（例: OpenAPI から生成した型）も参照しない
- 型定義は `src/api/types.ts` に手動で管理

### 3. 認証の抽象化

```typescript
// src/api/client.ts
// 認証方式の切り替えに対応できるよう wrapper で隠蔽
setApiKey("xxx");     // Phase 1: API Key
setJwtToken("xxx");   // Phase 2: JWT
```

### 4. 移植時のチェックリスト

- [ ] `gui/` フォルダを丸ごとコピー
- [ ] `.env.example` → `.env.local` を設定
- [ ] `npm install && npm run build` が通ることを確認
- [ ] CI を新リポ用に設定
