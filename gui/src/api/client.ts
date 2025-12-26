/**
 * API Client - sr-api との通信を抽象化
 *
 * 認証方式の切り替え（API Key → JWT）に対応できるよう wrapper で隠蔽
 */

/**
 * snake_case を camelCase に変換（再帰的）
 */
function snakeToCamel(obj: unknown): unknown {
  if (obj === null || obj === undefined) return obj;
  if (Array.isArray(obj)) return obj.map(snakeToCamel);
  if (typeof obj !== "object") return obj;

  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj as Record<string, unknown>)) {
    const camelKey = key.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
    result[camelKey] = snakeToCamel(value);
  }
  return result;
}

function camelToSnake(obj: unknown): unknown {
  if (obj === null || obj === undefined) return obj;
  if (Array.isArray(obj)) return obj.map(camelToSnake);
  if (typeof obj !== "object") return obj;

  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj as Record<string, unknown>)) {
    const snakeKey = key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
    result[snakeKey] = camelToSnake(value);
  }
  return result;
}

// 開発時: Vite proxy が /api/* を sr-api に転送
// 本番時: VITE_API_ORIGIN を設定するか、同一オリジンにデプロイ
const API_ORIGIN = import.meta.env.VITE_API_ORIGIN || "";

type AuthConfig =
  | { type: "api-key"; key: string }
  | { type: "jwt"; token: string }
  | { type: "none" };

let authConfig: AuthConfig = { type: "none" };

/**
 * 認証を設定（アプリ起動時に呼ぶ）
 */
export function setAuth(config: AuthConfig): void {
  authConfig = config;
}

/**
 * API Key 認証を設定（Phase 1）
 */
export function setApiKey(key: string): void {
  authConfig = { type: "api-key", key };
}

/**
 * JWT 認証を設定（Phase 2）
 */
export function setJwtToken(token: string): void {
  authConfig = { type: "jwt", token };
}

function readEnv(key: string): string {
  const raw = (import.meta.env as Record<string, string | undefined>)[key];
  return (raw ?? "").trim();
}

/**
 * 環境変数から初期認証情報をセット（VITE_JWT_TOKEN が優先）
 */
export function initializeAuthFromEnv(): void {
  const jwtToken = readEnv("VITE_JWT_TOKEN");
  if (jwtToken) {
    setJwtToken(jwtToken);
    return;
  }

  const apiKey = readEnv("VITE_API_KEY");
  if (apiKey && apiKey !== "your-api-key-here") {
    setApiKey(apiKey);
  }
}

/**
 * 認証ヘッダーを取得
 */
function getAuthHeaders(): Record<string, string> {
  switch (authConfig.type) {
    case "api-key":
      return { "X-API-Key": authConfig.key };
    case "jwt":
      return { Authorization: `Bearer ${authConfig.token}` };
    case "none":
      return {};
  }
}

/**
 * API リクエストを送信
 */
export async function apiRequest<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const url = `${API_ORIGIN}${path}`;

  const response = await fetch(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...getAuthHeaders(),
      ...options.headers,
    },
  });

  if (!response.ok) {
    const contentType = response.headers.get("content-type") || "";
    if (contentType.includes("application/json")) {
      const payload = await response.json().catch(() => null);
      throw new ApiError(
        response.status,
        payload?.message ?? "request failed",
        payload?.code,
        payload?.request_id ?? payload?.requestId
      );
    }
    const error = await response.text();
    throw new ApiError(response.status, error);
  }

  const json = await response.json();
  return snakeToCamel(json) as T;
}

/**
 * GET リクエスト
 */
export function get<T>(path: string): Promise<T> {
  return apiRequest<T>(path, { method: "GET" });
}

/**
 * POST リクエスト
 */
export function post<T>(path: string, body: unknown): Promise<T> {
  return apiRequest<T>(path, {
    method: "POST",
    body: JSON.stringify(camelToSnake(body)),
  });
}

/**
 * Fire-and-forget POST（行動ログ用、失敗しても UX をブロックしない）
 */
export function postFireAndForget(path: string, body: unknown): void {
  apiRequest(path, {
    method: "POST",
    body: JSON.stringify(camelToSnake(body)),
  }).catch((error) => {
    console.warn("[API] Fire-and-forget failed:", error);
  });
}

/**
 * API エラー
 */
export class ApiError extends Error {
  status: number;
  body: string;
  code?: string;
  requestId?: string;

  constructor(status: number, body: string, code?: string, requestId?: string) {
    super(`API Error ${status}: ${body}`);
    this.name = "ApiError";
    this.status = status;
    this.body = body;
    this.code = code;
    this.requestId = requestId;
  }
}
