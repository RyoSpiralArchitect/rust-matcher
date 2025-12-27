import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { get, post } from "../client";
import type {
  ProjectDetailResponse,
  FeedbackRequest,
  FeedbackResponse,
  ProjectsListResponse,
  FeedbackType,
  ProjectMatch,
  ProjectMatchStatus,
} from "../types";

function deriveStatusFromFeedback(feedbackType: FeedbackType | null | undefined): ProjectMatchStatus {
  switch (feedbackType) {
    case "thumbs_up":
    case "review_ok":
      return "proposed";
    case "accepted":
      return "accepted";
    case "thumbs_down":
    case "review_ng":
    case "rejected":
      return "rejected";
    case "interview_scheduled":
      return "interview_scheduled";
    case "no_response":
      return "no_response";
    default:
      return "pending";
  }
}

function normalizeProjectStatus(
  status: ProjectMatch["status"],
  fallbackFeedback?: FeedbackType | null
): ProjectMatchStatus {
  if (status === "proposed" || status === "pending" || status === "rejected" || status === "interview_scheduled" || status === "accepted" || status === "no_response") {
    return status;
  }

  if (status === "interviewing") {
    return "interview_scheduled";
  }

  if (status === "thumbs_up" || status === "review_ok") {
    return "proposed";
  }

  if (status === "thumbs_down" || status === "review_ng" || status === "rejected") {
    return "rejected";
  }

  if (fallbackFeedback) {
    return deriveStatusFromFeedback(fallbackFeedback);
  }

  return "pending";
}

function normalizeProjectMatch(match: ProjectMatch): ProjectMatch {
  const normalizedStatus = normalizeProjectStatus(match.status, match.lastFeedbackType ?? null);

  return {
    ...match,
    keySkills: match.keySkills ?? [],
    status: normalizedStatus,
    lastFeedbackType: match.lastFeedbackType ?? null,
    lastFeedbackAt: match.lastFeedbackAt ?? null,
  };
}

export function useProjects() {
  return useQuery({
    queryKey: ["projects", "list"],
    queryFn: () => get<ProjectsListResponse>("/api/projects"),
  });
}

export function useProjectDetail(projectId: number | string | undefined) {
  return useQuery({
    queryKey: ["projects", "detail", projectId],
    queryFn: async () => {
      const detail = await get<ProjectDetailResponse>(`/api/projects/${projectId}`);
      return {
        ...detail,
        skills: detail.skills ?? [],
        matches: (detail.matches ?? []).map(normalizeProjectMatch),
      };
    },
    enabled: !!projectId,
  });
}

export function useProjectFeedback(projectId: number) {
  const queryClient = useQueryClient();

  return useMutation<FeedbackResponse, unknown, FeedbackRequest>({
    mutationFn: (request) => post<FeedbackResponse>("/api/feedback", request),
    retry: 2,
    retryDelay: (attempt) => Math.min(1000 * attempt, 4000),
    onSuccess: (response, variables) => {
      const nextStatus = deriveStatusFromFeedback(response.feedbackType);
      queryClient.setQueryData<ProjectDetailResponse | undefined>(
        { queryKey: ["projects", "detail", projectId] },
        (previous) => {
          if (!previous) return previous;

          return {
            ...previous,
            matches: previous.matches.map((match) => {
              if (match.interactionId !== variables.interactionId) {
                return match;
              }
              return {
                ...match,
                status: nextStatus,
                lastFeedbackType: response.feedbackType,
                lastFeedbackAt: new Date().toISOString(),
              };
            }),
          };
        }
      );
      queryClient.invalidateQueries({ queryKey: ["projects", "detail", projectId] });
    },
  });
}
