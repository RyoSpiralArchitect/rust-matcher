/**
 * API Client - sr-api との通信を抽象化
 *
 * 認証方式の切り替え（API Key → JWT）に対応できるよう wrapper で隠蔽
 */

const API_ORIGIN = import.meta.env.VITE_API_ORIGIN || "http://localhost:3000";

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
    const error = await response.text();
    throw new ApiError(response.status, error);
  }

  return response.json();
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
    body: JSON.stringify(body),
  });
}

/**
 * Fire-and-forget POST（行動ログ用、失敗しても UX をブロックしない）
 */
export function postFireAndForget(path: string, body: unknown): void {
  apiRequest(path, {
    method: "POST",
    body: JSON.stringify(body),
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

  constructor(status: number, body: string) {
    super(`API Error ${status}: ${body}`);
    this.name = "ApiError";
    this.status = status;
    this.body = body;
  }
}
