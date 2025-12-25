import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // 5分間キャッシュ
      staleTime: 5 * 60 * 1000,
      // バックグラウンドでの自動リフェッチを抑制（営業向けなので手動リフレッシュ推奨）
      refetchOnWindowFocus: false,
      // リトライは1回まで
      retry: 1,
    },
    mutations: {
      // FB送信失敗時もリトライは1回まで
      retry: 1,
    },
  },
});
