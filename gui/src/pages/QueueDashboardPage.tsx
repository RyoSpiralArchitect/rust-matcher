import { Link } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useQueueDashboard } from "@/api";
import { LoadingState } from "@/components/LoadingState";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { useI18n } from "@/lib/i18n";

export function QueueDashboardPage() {
  const { data, isLoading, error } = useQueueDashboard();
  const { t } = useI18n();

  if (isLoading) {
    return <LoadingState />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">{t("queue.dashboard.title")}</h1>

      {/* Status Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("status.pending")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {data?.statusCounts.pending ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("status.processing")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {data?.statusCounts.processing ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("status.completed")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">
              {data?.statusCounts.completed ?? "-"}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("queue.dashboard.manualReview")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-yellow-600">
              {data?.manualReviewCount ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("queue.dashboard.errors")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-destructive">
              {data?.errorCount ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {t("queue.dashboard.staleProcessing")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-orange-600">
              {data?.staleProcessingCount ?? "-"}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Quick Links */}
      <div className="flex gap-4">
        <Link
          to="/jobs"
          className="text-sm text-primary hover:underline"
        >
          {t("queue.dashboard.viewAllJobs")}
        </Link>
        <Link
          to="/jobs?status=pending"
          className="text-sm text-primary hover:underline"
        >
          {t("queue.dashboard.viewPendingJobs")}
        </Link>
      </div>
    </div>
  );
}
