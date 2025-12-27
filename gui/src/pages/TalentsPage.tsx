import { useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useTalents, type TalentListItem } from "@/api";
import { useI18n } from "@/lib/i18n";

const DEFAULT_LIMIT = 20;

export function TalentsPage() {
  const { t } = useI18n();
  const [searchParams, setSearchParams] = useSearchParams();

  const activeFilters = useMemo(
    () => ({
      search: searchParams.get("q") ?? undefined,
      skill: searchParams.get("skill") ?? undefined,
      location: searchParams.get("location") ?? undefined,
      availability: searchParams.get("availability") ?? undefined,
    }),
    [searchParams],
  );

  const talentsQuery = useTalents({ ...activeFilters, limit: DEFAULT_LIMIT });
  const { data, error, isLoading, isFetchingNextPage, hasNextPage, fetchNextPage } = talentsQuery;

  if (isLoading && !data) {
    return <LoadingState message={t("talents.loading")}/>;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  const items = data?.items ?? [];
  const total = data?.total ?? 0;

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          <h1 className="text-2xl font-bold">{t("talents.title")}</h1>
          <Badge variant="secondary">{t("talents.resultCount", { count: total })}</Badge>
        </div>
        <p className="text-sm text-muted-foreground">
          {t("talents.subtitle")}
        </p>
      </div>

      <FiltersForm
        key={JSON.stringify(activeFilters)}
        activeFilters={activeFilters}
        onApply={(values) => {
          const next = new URLSearchParams();
          if (values.search.trim()) next.set("q", values.search.trim());
          if (values.skill.trim()) next.set("skill", values.skill.trim());
          if (values.location.trim()) next.set("location", values.location.trim());
          if (values.availability.trim()) next.set("availability", values.availability.trim());
          setSearchParams(next, { replace: true });
        }}
      />

      <TalentsList
        talents={items}
        isEmpty={!items.length && !isLoading}
        isLoadingMore={isFetchingNextPage}
        hasMore={hasNextPage}
        onLoadMore={() => fetchNextPage()}
      />
    </div>
  );
}

interface TalentsListProps {
  talents: TalentListItem[];
  isEmpty: boolean;
  isLoadingMore: boolean;
  hasMore: boolean | undefined;
  onLoadMore: () => void;
}

export function TalentsList({ talents, isEmpty, hasMore, isLoadingMore, onLoadMore }: TalentsListProps) {
  const { t } = useI18n();

  return (
    <div className="space-y-4">
      {isEmpty && (
        <Card>
          <CardContent className="py-8 text-center text-muted-foreground">
            {t("talents.empty")}
          </CardContent>
        </Card>
      )}

      {talents.map((talent) => (
        <Card key={talent.id}>
          <CardHeader className="flex flex-row items-start justify-between gap-4">
            <div className="space-y-1">
              <CardTitle>
                <Link to={`/talents/${talent.id}`} className="hover:underline">
                  {talent.name}
                </Link>
              </CardTitle>
              <CardDescription>
                {talent.role || t("talents.roleFallback")}
              </CardDescription>
            </div>
            <div className="flex flex-col items-end gap-2 text-right">
              <Badge variant="secondary">
                {t("talents.score", { score: talent.score?.toFixed(2) ?? "-" })}
              </Badge>
              <span className="text-xs text-muted-foreground">
                {t("talents.locationAvailability", {
                  location: talent.location ?? t("talents.unknown"),
                  availability: talent.availabilityDate ?? t("talents.unknown"),
                })}
              </span>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {talent.experienceHighlights.length > 0 && (
              <div className="space-y-2">
                <div className="text-sm font-medium text-muted-foreground">
                  {t("talents.experience")}
                </div>
                <ul className="list-disc space-y-1 pl-5 text-sm text-muted-foreground">
                  {talent.experienceHighlights.map((item, index) => (
                    <li key={`${talent.id}-exp-${index}`}>{item}</li>
                  ))}
                </ul>
              </div>
            )}
            {talent.skills.length > 0 && (
              <div className="space-y-2">
                <div className="text-sm font-medium text-muted-foreground">
                  {t("talents.skills")}
                </div>
                <div className="flex flex-wrap gap-2">
                  {talent.skills.map((skill) => (
                    <Badge key={`${talent.id}-${skill}`} variant="outline">
                      {skill}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      ))}

      {hasMore && (
        <div className="flex justify-center">
          <Button variant="outline" onClick={onLoadMore} disabled={isLoadingMore}>
            {isLoadingMore ? t("talents.loading") : t("talents.loadMore")}
          </Button>
        </div>
      )}
    </div>
  );
}

interface FiltersFormProps {
  activeFilters: {
    search?: string;
    skill?: string;
    location?: string;
    availability?: string;
  };
  onApply: (values: { search: string; skill: string; location: string; availability: string }) => void;
}

function FiltersForm({ activeFilters, onApply }: FiltersFormProps) {
  const { t } = useI18n();
  const [formState, setFormState] = useState(() => ({
    search: activeFilters.search ?? "",
    skill: activeFilters.skill ?? "",
    location: activeFilters.location ?? "",
    availability: activeFilters.availability ?? "",
  }));

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    onApply({
      search: formState.search,
      skill: formState.skill,
      location: formState.location,
      availability: formState.availability,
    });
  };

  return (
    <Card>
      <form onSubmit={handleSubmit} className="space-y-4">
        <CardHeader>
          <CardTitle className="text-base">{t("talents.filters.title")}</CardTitle>
          <CardDescription>{t("talents.filters.description")}</CardDescription>
        </CardHeader>
        <CardContent className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
          <label className="space-y-2">
            <span className="text-sm font-medium text-muted-foreground">{t("talents.filters.search")}</span>
            <input
              type="text"
              value={formState.search}
              onChange={(e) => setFormState((prev) => ({ ...prev, search: e.target.value }))}
              placeholder={t("talents.filters.searchPlaceholder")}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </label>
          <label className="space-y-2">
            <span className="text-sm font-medium text-muted-foreground">{t("talents.filters.skill")}</span>
            <input
              type="text"
              value={formState.skill}
              onChange={(e) => setFormState((prev) => ({ ...prev, skill: e.target.value }))}
              placeholder={t("talents.filters.skillPlaceholder")}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </label>
          <label className="space-y-2">
            <span className="text-sm font-medium text-muted-foreground">{t("talents.filters.location")}</span>
            <input
              type="text"
              value={formState.location}
              onChange={(e) => setFormState((prev) => ({ ...prev, location: e.target.value }))}
              placeholder={t("talents.filters.locationPlaceholder")}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </label>
          <label className="space-y-2">
            <span className="text-sm font-medium text-muted-foreground">{t("talents.filters.availability")}</span>
            <input
              type="text"
              value={formState.availability}
              onChange={(e) => setFormState((prev) => ({ ...prev, availability: e.target.value }))}
              placeholder={t("talents.filters.availabilityPlaceholder")}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </label>
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button type="submit">{t("talents.filters.apply")}</Button>
        </CardFooter>
      </form>
    </Card>
  );
}
