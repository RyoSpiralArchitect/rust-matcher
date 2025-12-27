import { useMemo } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useProjects, type ProjectListItem } from "@/api";
import { useI18n } from "@/lib/i18n";
import { ProjectCard } from "@/components/projects/ProjectCard";

export function ProjectsList({ canCreateProject = true }: { canCreateProject?: boolean } = {}) {
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
      <div className="flex items-start justify-between gap-3 flex-wrap">
        <div>
          <h1 className="text-2xl font-bold">{t("projects.title")}</h1>
          <p className="text-sm text-muted-foreground">
            {t("projects.subtitle", { count: projects.length })}
          </p>
        </div>
        {canCreateProject && (
          <Button asChild>
            {/* TODO: Replace with the real project creation route/modal when available. */}
            <Link to="/projects/new">
              {t("projects.list.ctaCreate")}
            </Link>
          </Button>
        )}
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
              formatMessage={t}
            />
          ))}
        </div>
      )}
    </div>
  );
}
