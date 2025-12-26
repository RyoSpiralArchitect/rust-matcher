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
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { LoadingState } from "@/components/LoadingState";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { StatusBadge } from "@/components/StatusBadge";
import { useQueueJobs } from "@/api";

const STATUSES = ["all", "pending", "processing", "completed"] as const;
const PAGE_SIZE = 20;

export function QueueJobsPage() {
  const [searchParams, setSearchParams] = useSearchParams();

  const status = searchParams.get("status") ?? undefined;
  const requiresReview = searchParams.get("review") === "true";
  const rawOffset = Number.parseInt(searchParams.get("offset") ?? "0", 10);
  const offset = Number.isFinite(rawOffset) && rawOffset > 0 ? rawOffset : 0;

  const { data, isLoading, error } = useQueueJobs({
    status: status === "all" ? undefined : status,
    limit: PAGE_SIZE,
    offset,
  });

  const updateFilter = (key: string, value: string | null) => {
    const next = new URLSearchParams(searchParams);
    if (value === null || value === "all" || value === "") {
      next.delete(key);
    } else {
      next.set(key, value);
    }
    next.delete("offset"); // Reset pagination on filter change
    setSearchParams(next);
  };

  const goToPage = (newOffset: number) => {
    const next = new URLSearchParams(searchParams);
    if (newOffset === 0) {
      next.delete("offset");
    } else {
      next.set("offset", String(newOffset));
    }
    setSearchParams(next);
  };

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Queue Jobs</h1>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-4 flex-wrap">
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">Status:</span>
          <Select
            value={status ?? "all"}
            onValueChange={(v) => updateFilter("status", v)}
          >
            <SelectTrigger className="w-[140px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {s === "all" ? "All" : s.charAt(0).toUpperCase() + s.slice(1)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex items-center gap-2">
          <Switch
            id="review-filter"
            checked={requiresReview}
            onCheckedChange={(checked) =>
              updateFilter("review", checked ? "true" : null)
            }
          />
          <label
            htmlFor="review-filter"
            className="text-sm text-muted-foreground cursor-pointer"
          >
            Manual Review Only
          </label>
        </div>

        {(status || requiresReview) && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setSearchParams({})}
          >
            Clear Filters
          </Button>
        )}
      </div>

      {isLoading ? (
        <LoadingState />
      ) : (
        <>
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

          {/* Pagination */}
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Showing {offset + 1} - {offset + (data?.items.length ?? 0)}
            </span>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={offset === 0}
                onClick={() => goToPage(Math.max(0, offset - PAGE_SIZE))}
              >
                ← Prev
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={!data?.hasMore}
                onClick={() => goToPage(offset + PAGE_SIZE)}
              >
                Next →
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
