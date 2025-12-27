import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
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
import { useInfiniteQueueJobs } from "@/api";
import { cn } from "@/lib/utils";
import { Inbox, Loader2 } from "lucide-react";

const STATUSES = ["all", "pending", "processing", "completed"] as const;
const PAGE_SIZE = 50;
const ROW_HEIGHT = 64;
const COLUMN_LAYOUT =
  "grid grid-cols-[90px_210px_130px_100px_80px_90px_180px] items-center";

export function QueueJobsPage() {
  const [searchParams, setSearchParams] = useSearchParams();

  const status = searchParams.get("status") ?? undefined;
  const requiresReview = searchParams.get("review") === "true";
  const [pendingStatus, setPendingStatus] = useState(status ?? "all");
  const [pendingRequiresReview, setPendingRequiresReview] =
    useState(requiresReview);
  const {
    data,
    isLoading,
    isFetchingNextPage,
    error,
    hasNextPage,
    fetchNextPage,
  } = useInfiniteQueueJobs({
    status: status === "all" ? undefined : status,
    limit: PAGE_SIZE,
    requiresManualReview: requiresReview ? true : undefined,
  });

  const jobs = useMemo(
    () => data?.pages.flatMap((page) => page.items) ?? [],
    [data],
  );

  const scrollParentRef = useRef<HTMLDivElement | null>(null);
  // eslint-disable-next-line react-hooks/incompatible-library
  const rowVirtualizer = useVirtualizer({
    count: jobs.length,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
  });
  const [activeIndex, setActiveIndex] = useState<number | null>(
    jobs.length ? 0 : null,
  );
  const rowRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const activeJobId =
    activeIndex !== null && jobs[activeIndex] ? jobs[activeIndex].id : undefined;

  const virtualItems = rowVirtualizer.getVirtualItems();

  useEffect(() => {
    if (!hasNextPage || isFetchingNextPage) return;
    const lastItem = virtualItems.at(-1);
    if (lastItem && lastItem.index >= jobs.length - 5) {
      fetchNextPage();
    }
  }, [virtualItems, hasNextPage, isFetchingNextPage, jobs.length, fetchNextPage]);

  useEffect(() => {
    if (
      pendingStatus === (status ?? "all") &&
      pendingRequiresReview === requiresReview
    ) {
      return;
    }

    const timeout = window.setTimeout(() => {
      const next = new URLSearchParams(searchParams);
      if (pendingStatus === "all") {
        next.delete("status");
      } else {
        next.set("status", pendingStatus);
      }
      if (pendingRequiresReview) {
        next.set("review", "true");
      } else {
        next.delete("review");
      }

      setSearchParams(next);
      rowVirtualizer.scrollToIndex(0);
    }, 300);

    return () => window.clearTimeout(timeout);
  }, [
    pendingRequiresReview,
    pendingStatus,
    requiresReview,
    rowVirtualizer,
    searchParams,
    setSearchParams,
    status,
  ]);

  useEffect(() => {
    setPendingStatus(status ?? "all");
    setPendingRequiresReview(requiresReview);
  }, [requiresReview, status]);

  useEffect(() => {
    if (jobs.length === 0) {
      setActiveIndex(null);
      return;
    }

    setActiveIndex((current) => {
      if (current === null || current >= jobs.length) {
        return 0;
      }
      return current;
    });
  }, [jobs.length]);

  useEffect(() => {
    if (activeIndex === null) return;
    rowVirtualizer.scrollToIndex(activeIndex);
    const node = rowRefs.current.get(activeIndex);
    node?.focus({ preventScroll: true });
  }, [activeIndex, rowVirtualizer]);

  const setRowRef = (index: number, node: HTMLDivElement | null) => {
    if (node) {
      rowRefs.current.set(index, node);
      rowVirtualizer.measureElement(node);
    } else {
      rowRefs.current.delete(index);
    }
  };

  const handleListKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!jobs.length) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) => {
        const next =
          current === null ? 0 : Math.min(current + 1, jobs.length - 1);
        return next;
      });
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => {
        const next =
          current === null ? 0 : Math.max(current - 1, 0);
        return next;
      });
    }
  };

  const locale =
    typeof navigator !== "undefined" && navigator.language
      ? navigator.language
      : undefined;
  const formatDate = (value: string) =>
    new Intl.DateTimeFormat(locale, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(value));

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
            value={pendingStatus}
            onValueChange={setPendingStatus}
          >
            <SelectTrigger
              aria-label="Filter queue by status"
              className="w-[140px]"
            >
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
            aria-label="Filter to jobs requiring manual review"
            checked={pendingRequiresReview}
            onCheckedChange={setPendingRequiresReview}
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
            aria-label="Clear active filters"
            onClick={() => {
              setSearchParams({});
              setPendingStatus("all");
              setPendingRequiresReview(false);
              rowVirtualizer.scrollToIndex(0);
            }}
          >
            Clear Filters
          </Button>
        )}
      </div>

      {isLoading ? (
        <LoadingState />
      ) : jobs.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-md border border-dashed p-10 text-center">
          <Inbox className="h-10 w-10 text-muted-foreground" />
          <p className="mt-2 text-lg font-semibold">No queue jobs found</p>
          <p className="text-sm text-muted-foreground">
            Try adjusting the filters to see jobs that match your criteria.
          </p>
        </div>
      ) : (
        <div className="rounded-md border">
          <div className="overflow-x-auto">
            <div className={cn(COLUMN_LAYOUT, "min-w-[880px] border-b bg-muted/60 px-3 py-2 text-sm font-semibold")}>
              <span>ID</span>
              <span>Message ID</span>
              <span>Status</span>
              <span>Priority</span>
              <span>Retry</span>
              <span>Review</span>
              <span>Updated</span>
            </div>
            <div
              ref={scrollParentRef}
              className="max-h-[70vh] min-w-[880px] overflow-auto"
              role="listbox"
              aria-label="Queue jobs"
              aria-activedescendant={
                activeJobId ? `queue-row-${activeJobId}` : undefined
              }
              tabIndex={0}
              onKeyDown={handleListKeyDown}
            >
              <div
                style={{
                  height: `${rowVirtualizer.getTotalSize() + (isFetchingNextPage ? ROW_HEIGHT : 0)}px`,
                  position: "relative",
                }}
              >
                {virtualItems.map((virtualRow) => {
                  const job = jobs[virtualRow.index];
                  return (
                    <div
                      key={job.id}
                      id={`queue-row-${job.id}`}
                      data-index={virtualRow.index}
                      ref={(node) => setRowRef(virtualRow.index, node)}
                      className={cn(
                        COLUMN_LAYOUT,
                        "border-b px-3 py-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background"
                      )}
                      role="option"
                      aria-selected={activeIndex === virtualRow.index}
                      tabIndex={activeIndex === virtualRow.index ? 0 : -1}
                      onFocus={() => setActiveIndex(virtualRow.index)}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        transform: `translateY(${virtualRow.start}px)`,
                      }}
                    >
                      <Link
                        to={`/jobs/${job.id}`}
                        aria-label={`View queue job ${job.id}`}
                        className="text-primary hover:underline"
                      >
                        {job.id}
                      </Link>
                      <span className="font-mono text-xs">
                        {job.messageId.slice(0, 20)}...
                      </span>
                      <span>
                        <StatusBadge status={job.status} />
                      </span>
                      <span>{job.priority}</span>
                      <span>{job.retryCount}</span>
                      <span>
                        {job.requiresManualReview && (
                          <Badge variant="secondary">Review</Badge>
                        )}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {formatDate(job.updatedAt)}
                      </span>
                    </div>
                  );
                })}
                {isFetchingNextPage && (
                  <div
                    className="absolute left-0 right-0 flex items-center justify-center gap-2 text-sm text-muted-foreground"
                    style={{ top: rowVirtualizer.getTotalSize() }}
                  >
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Loading more jobs...
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
