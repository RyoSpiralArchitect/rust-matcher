import { Link } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useQueueDashboard } from "@/api";
import { LoadingState } from "@/components/LoadingState";
import { ErrorDisplay } from "@/components/ErrorDisplay";

export function QueueDashboardPage() {
  const { data, isLoading, error } = useQueueDashboard();

  if (isLoading) {
    return <LoadingState />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Queue Dashboard</h1>

      {/* Status Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Pending
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
              Processing
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
              Completed
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
              Manual Review
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
              Errors
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
              Stale Processing
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
          View All Jobs →
        </Link>
        <Link
          to="/jobs?status=pending"
          className="text-sm text-primary hover:underline"
        >
          View Pending Jobs →
        </Link>
      </div>
    </div>
  );
}
