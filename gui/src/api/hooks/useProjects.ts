import { useQuery, useMutation, useQueryClient, type UseQueryOptions } from "@tanstack/react-query";
import { get, post } from "../client";
import type {
  ProjectDetailResponse,
  FeedbackRequest,
  FeedbackResponse,
  ProjectsListResponse,
} from "../types";

export function useProjects() {
  return useQuery({
    queryKey: ["projects", "list"],
    queryFn: () => get<ProjectsListResponse>("/api/projects"),
  });
}

export function useProjectDetail(projectId: number | string | undefined) {
  return useProjectDetailQuery<ProjectDetailResponse>(projectId);
}

export function useProjectDetailQuery<TData = ProjectDetailResponse>(
  projectId: number | string | undefined,
  options?: Omit<UseQueryOptions<ProjectDetailResponse, unknown, TData>, "queryKey" | "queryFn">,
) {
  return useQuery<ProjectDetailResponse, unknown, TData>({
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
    ...(options ?? {}),
  });
}

export function useProjectMatches(projectId: number | string | undefined) {
  return useProjectDetailQuery(projectId, {
    select: (detail) => detail.matches ?? [],
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
