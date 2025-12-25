import { useMutation, useQueryClient } from "@tanstack/react-query";
import { post } from "../client";
import type { ConversionRequest, ConversionResponse } from "../types";

/**
 * Conversion Stage 送信
 */
export function useSendConversion() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (request: ConversionRequest) =>
      post<ConversionResponse>("/api/conversions", request),
    onSuccess: () => {
      // Job詳細をリフレッシュ
      queryClient.invalidateQueries({ queryKey: ["queue", "job"] });
    },
  });
}
