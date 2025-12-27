import { useInfiniteQuery } from "@tanstack/react-query";
import { get } from "../client";
import type { TalentListItem, TalentSearchResponse } from "../types";

export interface TalentSearchParams {
  search?: string;
  skill?: string;
  location?: string;
  availability?: string;
  limit?: number;
}

type CombinedTalentPages = {
  pages: TalentSearchResponse[];
  pageParams: number[];
  items: TalentListItem[];
  total: number;
  hasMore: boolean;
  limit: number;
};

function buildSearchParams(params: TalentSearchParams, offset: number) {
  const searchParams = new URLSearchParams();
  const limit = params.limit ?? 20;
  searchParams.set("limit", String(limit));
  searchParams.set("offset", String(offset));

  if (params.search) searchParams.set("search", params.search);
  if (params.skill) searchParams.set("skill", params.skill);
  if (params.location) searchParams.set("location", params.location);
  if (params.availability) searchParams.set("availability", params.availability);

  return searchParams;
}

export function useTalents(params: TalentSearchParams) {
  const limit = params.limit ?? 20;

  return useInfiniteQuery<TalentSearchResponse, unknown, CombinedTalentPages>({
    queryKey: ["talents", params],
    initialPageParam: 0,
    queryFn: ({ pageParam }) => {
      const searchParams = buildSearchParams(params, pageParam);
      const query = searchParams.toString();
      const path = query ? `/api/talents?${query}` : "/api/talents";
      return get<TalentSearchResponse>(path);
    },
    getNextPageParam: (lastPage) =>
      lastPage.hasMore ? lastPage.offset + lastPage.limit : undefined,
    select: (data) => {
      const items = data.pages.flatMap((page) => page.items);
      const total = data.pages.at(-1)?.total ?? data.pages[0]?.total ?? 0;
      const hasMore = data.pages.at(-1)?.hasMore ?? false;

      return {
        ...data,
        items,
        total,
        hasMore,
        limit,
      } satisfies {
        pages: TalentSearchResponse[];
        pageParams: number[];
        items: TalentListItem[];
        total: number;
        hasMore: boolean;
        limit: number;
      };
    },
  });
}
