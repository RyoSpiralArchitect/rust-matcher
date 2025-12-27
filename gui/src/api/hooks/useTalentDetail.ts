import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { get, post } from "../client";
import type {
  TalentDetailResponse,
  TalentMatchDecisionRequest,
  TalentMatchDecisionResponse,
} from "../types";

export function useTalentDetail(talentId: number | string | undefined) {
  return useQuery({
    queryKey: ["talents", talentId],
    queryFn: () => get<TalentDetailResponse>(`/api/talents/${talentId}`),
    enabled: !!talentId,
  });
}

export function useMatchDecision() {
  const queryClient = useQueryClient();

  return useMutation<TalentMatchDecisionResponse, unknown, TalentMatchDecisionRequest>({
    mutationFn: ({ talentId, projectId, decision }) =>
      post<TalentMatchDecisionResponse>(
        `/api/talents/${talentId}/matches/${projectId}/decision`,
        { decision },
      ),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ["talents", variables.talentId] });
      queryClient.invalidateQueries({ queryKey: ["candidates", variables.projectId] });
    },
  });
}
