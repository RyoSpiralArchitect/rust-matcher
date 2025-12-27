import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { get, post } from "../client";
import type { ProjectDetailResponse, FeedbackRequest, FeedbackResponse } from "../types";

export function useProjectDetail(projectId: number | string | undefined) {
  return useQuery({
    queryKey: ["projects", "detail", projectId],
    queryFn: async () => {
      const detail = await get<ProjectDetailResponse>(`/api/projects/${projectId}`);
      return {
        ...detail,
        skills: detail.skills ?? [],
        matches: (detail.matches ?? []).map((match) => ({
          ...match,
          keySkills: match.keySkills ?? [],
        })),
      };
    },
    enabled: !!projectId,
  });
}

export function useProjectFeedback(projectId: number) {
  const queryClient = useQueryClient();

  return useMutation<FeedbackResponse, unknown, FeedbackRequest>({
    mutationFn: (request) => post<FeedbackResponse>("/api/feedback", request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["projects", "detail", projectId] });
    },
  });
}
