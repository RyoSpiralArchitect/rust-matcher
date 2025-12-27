import { Link } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useQueueDashboard } from "@/api";
import { LoadingState } from "@/components/LoadingState";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { useI18n } from "@/lib/i18n";
import { useFlags } from "@/lib/auth";

export function QueueDashboardPage() {
  const { data, isLoading, error } = useQueueDashboard();
  const { t, locale } = useI18n();
  const { isQueueAdmin } = useFlags();

  if (isLoading) {
    return <LoadingState />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-1 md:flex-row md:items-baseline md:justify-between">
        <div className="space-y-1">
          <h1 className="text-2xl font-bold">{t("queue.dashboard.title")}</h1>
          <p className="text-sm text-muted-foreground">
            {t("queue.dashboard.managementSubheading")}
          </p>
        </div>
        {data?.updatedAt && (
          <p className="text-sm text-muted-foreground">
            {t("queue.dashboard.lastUpdated", {
              value: new Intl.DateTimeFormat(locale, {
                dateStyle: "medium",
                timeStyle: "short",
              }).format(new Date(data.updatedAt)),
            })}
          </p>
        )}
      </div>

      {!isQueueAdmin ? (
        <div className="rounded-md border border-dashed bg-muted/40 p-6 text-center text-sm text-muted-foreground">
          {t("queue.access.noAccess")}
        </div>
      ) : (
        <>
          <div className="space-y-4">
            <div className="space-y-2">
              <h2 className="text-lg font-semibold">{t("queue.dashboard.managementHeading")}</h2>
              <p className="text-sm text-muted-foreground">{t("queue.access.note")}</p>
            </div>

            <div className="space-y-3">
              {[
                {
                  key: "pending",
                  title: t("queue.dashboard.rows.pending"),
                  note: t("queue.dashboard.rows.pendingNote"),
                  count: data?.statusCounts.pending ?? 0,
                  href: "/jobs?status=pending",
                  badge: t("status.pending"),
                },
                {
                  key: "on-hold",
                  title: t("queue.dashboard.rows.onHold"),
                  note: t("queue.dashboard.rows.onHoldNote"),
                  count: data?.statusCounts.processing ?? 0,
                  href: "/jobs?status=processing",
                  badge: t("status.processing"),
                  extras: [
                    {
                      label: t("queue.dashboard.staleProcessing"),
                      value: data?.staleProcessingCount ?? 0,
                    },
                    {
                      label: t("queue.dashboard.errors"),
                      value: data?.errorCount ?? 0,
                    },
                  ],
                },
                {
                  key: "review",
                  title: t("queue.dashboard.rows.review"),
                  note: t("queue.dashboard.rows.reviewNote"),
                  count: data?.manualReviewCount ?? 0,
                  href: "/jobs?review=true",
                  badge: t("queue.dashboard.manualReview"),
                },
              ].map((row) => (
                <Card key={row.key}>
                  <CardHeader className="flex flex-row items-center justify-between gap-3 space-y-0">
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <CardTitle className="text-lg">{row.title}</CardTitle>
                        <Badge variant="outline">{row.badge}</Badge>
                      </div>
                      <p className="text-sm text-muted-foreground">{row.note}</p>
                    </div>
                    <div className="text-3xl font-bold">{row.count}</div>
                  </CardHeader>
                  <CardContent className="flex flex-wrap items-center justify-between gap-3">
                    <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                      {row.extras?.map((extra) => (
                        <Badge key={extra.label} variant="secondary">
                          {extra.label}: {extra.value}
                        </Badge>
                      ))}
                    </div>
                    <Link to={row.href} className="text-sm text-primary hover:underline">
                      {t("queue.dashboard.linkToList", { label: row.title })}
                    </Link>
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>

          <div className="flex flex-wrap gap-4 pt-2">
            <Link to="/projects" className="text-sm text-primary hover:underline">
              {t("queue.dashboard.linkProjects")}
            </Link>
            <Link to="/talents" className="text-sm text-primary hover:underline">
              {t("queue.dashboard.linkTalents")}
            </Link>
            <Link to="/jobs" className="text-sm text-primary hover:underline">
              {t("queue.dashboard.viewAllJobs")}
            </Link>
            <Link to="/jobs?status=pending" className="text-sm text-primary hover:underline">
              {t("queue.dashboard.viewPendingJobs")}
            </Link>
          </div>
        </>
      )}
    </div>
  );
}
