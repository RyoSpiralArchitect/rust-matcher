import { useQuery } from "@tanstack/react-query";
import { get } from "../client";
import type { ProjectListItem, ProjectListResponse } from "../types";

/**
 * プロジェクト一覧取得
 */
export function useProjects() {
  return useQuery({
    queryKey: ["projects", "list"],
    queryFn: () =>
      get<ProjectListResponse | ProjectListItem[]>("/api/projects"),
  });
}
