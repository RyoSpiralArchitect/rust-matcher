import { Link, useSearchParams } from "react-router-dom";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { useQueueJobs } from "@/api";

export function QueueJobsPage() {
  const [searchParams] = useSearchParams();
  const status = searchParams.get("status") ?? undefined;

  const { data, isLoading, error } = useQueueJobs({ status, limit: 50 });

  if (isLoading) {
    return <div className="text-muted-foreground">Loading...</div>;
  }

  if (error) {
    return (
      <div className="text-destructive">
        Error: {error instanceof Error ? error.message : "Unknown error"}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Queue Jobs</h1>
        {status && (
          <Badge variant="outline">Filter: {status}</Badge>
        )}
      </div>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>ID</TableHead>
            <TableHead>Message ID</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Priority</TableHead>
            <TableHead>Retry</TableHead>
            <TableHead>Review</TableHead>
            <TableHead>Updated</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {data?.items.map((job) => (
            <TableRow key={job.id}>
              <TableCell>
                <Link
                  to={`/jobs/${job.id}`}
                  className="text-primary hover:underline"
                >
                  {job.id}
                </Link>
              </TableCell>
              <TableCell className="font-mono text-xs">
                {job.messageId.slice(0, 20)}...
              </TableCell>
              <TableCell>
                <StatusBadge status={job.status} />
              </TableCell>
              <TableCell>{job.priority}</TableCell>
              <TableCell>{job.retryCount}</TableCell>
              <TableCell>
                {job.requiresManualReview && (
                  <Badge variant="secondary">Review</Badge>
                )}
              </TableCell>
              <TableCell className="text-xs text-muted-foreground">
                {new Date(job.updatedAt).toLocaleString("ja-JP")}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      {data?.hasMore && (
        <div className="text-sm text-muted-foreground">
          More jobs available...
        </div>
      )}
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const variant =
    status === "completed"
      ? "default"
      : status === "processing"
        ? "secondary"
        : "outline";

  return <Badge variant={variant}>{status}</Badge>;
}
