import { useMemo, type KeyboardEvent } from "react";
import { useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useProjects, type ProjectListItem } from "@/api";
import { useI18n } from "@/lib/i18n";

function formatBudgetRange(
  min: number | null,
  max: number | null,
  locale: string,
  format: (key: string, values?: Record<string, string | number>) => string,
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
};

function ProjectCard({ project, locale, onOpen }: ProjectCardProps) {
  const { t } = useI18n();
  const projectId = project.id ?? project.projectId;

  const budgetLabel = useMemo(
    () =>
      formatBudgetRange(
        project.monthlyTankaMin,
        project.monthlyTankaMax,
        locale,
        t,
      ),
    [project.monthlyTankaMin, project.monthlyTankaMax, locale, t],
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
      aria-label={
        project.projectName
          ? t("projects.card.ariaLabel", { name: project.projectName })
          : undefined
      }
      className="cursor-pointer transition hover:shadow-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
    >
      <CardHeader>
        <CardTitle>{project.projectName}</CardTitle>
        <CardDescription>{budgetLabel}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap gap-2">
          <Badge variant="secondary">
            {t("projects.counts.matched", {
              count: project.matchedCount ?? 0,
            })}
          </Badge>
          <Badge variant="outline">
            {t("projects.counts.proposed", {
              count: project.proposedCount ?? 0,
            })}
          </Badge>
          <Badge variant="outline">
            {t("projects.counts.interviewing", {
              count: project.interviewingCount ?? 0,
            })}
          </Badge>
        </div>
      </CardContent>
    </Card>
  );
}

export function ProjectsList() {
  const navigate = useNavigate();
  const { t, locale } = useI18n();
  const { data, isLoading, error } = useProjects();

  const projects = useMemo<ProjectListItem[]>(() => {
    if (!data) return [];
    if (Array.isArray(data)) return data;
    return data.items ?? [];
  }, [data]);

  if (isLoading) {
    return <LoadingState message={t("projects.loading")} />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  const handleOpenProject = (project: ProjectListItem) => {
    const projectId = project.id ?? project.projectId;
    if (projectId) {
      navigate(`/projects/${projectId}`);
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">{t("projects.title")}</h1>
        <p className="text-sm text-muted-foreground">
          {t("projects.subtitle", { count: projects.length })}
        </p>
      </div>

      {projects.length === 0 ? (
        <Card>
          <CardContent className="py-10 text-center text-muted-foreground">
            {t("projects.empty")}
          </CardContent>
        </Card>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {projects.map((project) => (
            <ProjectCard
              key={project.id ?? project.projectId}
              project={project}
              onOpen={handleOpenProject}
              locale={locale}
            />
          ))}
        </div>
      )}
    </div>
  );
}
