import { useMemo, type KeyboardEvent } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import type { ProjectListItem } from "@/api";
import type { TranslationKey } from "@/lib/messages";
import { componentTheme } from "@/theme/component-theme";

type Translator = (key: TranslationKey, values?: Record<string, string | number>) => string;

export function formatBudgetRange(
  min: number | null,
  max: number | null,
  locale: string,
  format: Translator,
) {
  if (min == null && max == null) {
    return format("projects.budget.unknown");
  }

  const formatter = new Intl.NumberFormat(locale === "ja" ? "ja-JP" : "en-US", {
    style: "currency",
    currency: "JPY",
    maximumFractionDigits: 0,
  });

  if (min != null && max != null) {
    return format("projects.budget.range", {
      min: formatter.format(min),
      max: formatter.format(max),
    });
  }

  if (min != null) {
    return format("projects.budget.min", { min: formatter.format(min) });
  }

  if (max != null) {
    return format("projects.budget.max", { max: formatter.format(max) });
  }

  return format("projects.budget.unknown");
}

type ProjectCardProps = {
  project: ProjectListItem;
  locale: string;
  onOpen: (project: ProjectListItem) => void;
  formatMessage: Translator;
};

export function ProjectCard({ project, locale, onOpen, formatMessage }: ProjectCardProps) {
  const projectId = project.id ?? project.projectId;
  const displayName =
    project.projectName ??
    formatMessage("projectDetail.title.fallback", { id: projectId ?? "" });

  const budgetLabel = useMemo(
    () =>
      formatBudgetRange(
        project.monthlyTankaMin,
        project.monthlyTankaMax,
        locale,
        formatMessage,
      ),
    [project.monthlyTankaMin, project.monthlyTankaMax, locale, formatMessage],
  );

  const handleOpen = () => {
    if (!projectId) return;
    onOpen(project);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      handleOpen();
    }
  };

  return (
    <Card
      role="button"
      tabIndex={0}
      onClick={handleOpen}
      onKeyDown={handleKeyDown}
      aria-label={formatMessage("projects.card.ariaLabel", { name: displayName })}
      className="cursor-pointer transition hover:shadow-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
    >
      <CardHeader>
        <CardTitle>{displayName}</CardTitle>
        <CardDescription>{budgetLabel}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className={componentTheme.layout.badgeRow}>
          <Badge variant="secondary">
            {formatMessage("projects.counts.matched", {
              count: project.matchedCount ?? 0,
            })}
          </Badge>
          <Badge variant="outline">
            {formatMessage("projects.counts.proposed", {
              count: project.proposedCount ?? 0,
            })}
          </Badge>
          <Badge variant="outline">
            {formatMessage("projects.counts.interviewing", {
              count: project.interviewingCount ?? 0,
            })}
          </Badge>
        </div>
      </CardContent>
    </Card>
  );
}
