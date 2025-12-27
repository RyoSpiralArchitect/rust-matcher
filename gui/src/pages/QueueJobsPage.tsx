import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
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
import { QUEUE_JOB_STATUSES, type QueueJobStatus, useInfiniteQueueJobs } from "@/api";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { Inbox, Loader2 } from "lucide-react";
import { useFlags } from "@/lib/auth";

type StatusFilter = QueueJobStatus | "all";
type SortMode = "status" | "updated";
const PAGE_SIZE = 50;
const ROW_HEIGHT = 64;
const FILTER_DEBOUNCE_MS = 300;
const STATUS_ORDER: QueueJobStatus[] = ["pending", "processing", "completed"];
const COLUMN_LAYOUT =
  "grid grid-cols-[90px_160px_1fr_200px] items-center";

export function QueueJobsPage({ canCreateJob = true }: { canCreateJob?: boolean } = {}) {
  const { t, locale } = useI18n();
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const { isQueueAdmin } = useFlags();

  const statusParam = searchParams.get("status");
  const normalizedStatus = QUEUE_JOB_STATUSES.includes(statusParam as QueueJobStatus)
    ? (statusParam as QueueJobStatus)
    : undefined;
  const requiresReview = searchParams.get("review") === "true";
  const [pendingStatus, setPendingStatus] = useState<StatusFilter>(normalizedStatus ?? "pending");
  const [pendingRequiresReview, setPendingRequiresReview] = useState(requiresReview);
  const [sortMode, setSortMode] = useState<SortMode>("status");
  const {
    data,
    isLoading,
    isFetchingNextPage,
    error,
    hasNextPage,
    fetchNextPage,
  } = useInfiniteQueueJobs({
    status: normalizedStatus,
    limit: PAGE_SIZE,
    requiresManualReview: requiresReview ? true : undefined,
  });

  const rawJobs = useMemo(
    () => data?.pages.flatMap((page) => page.items) ?? [],
    [data],
  );

  const jobs = useMemo(() => {
    const copy = [...rawJobs];
    if (sortMode === "status") {
      copy.sort((a, b) => {
        if (a.requiresManualReview !== b.requiresManualReview) {
          return a.requiresManualReview ? -1 : 1;
        }
        const statusDiff =
          STATUS_ORDER.indexOf(a.status) - STATUS_ORDER.indexOf(b.status);
        if (statusDiff !== 0) return statusDiff;
        return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
      });
      return copy;
    }
    return copy.sort(
      (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
    );
  }, [rawJobs, sortMode]);

  const scrollParentRef = useRef<HTMLDivElement | null>(null);
  const debounceRef = useRef<number | null>(null);
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
      pendingStatus === (normalizedStatus ?? "all") &&
      pendingRequiresReview === requiresReview
    ) {
      return;
    }

    if (typeof window === "undefined") return;

    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current);
    }

    debounceRef.current = window.setTimeout(() => {
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

      setSearchParams(next, { replace: true });
      rowVirtualizer.scrollToIndex(0);
    }, FILTER_DEBOUNCE_MS);

    return () => {
      if (debounceRef.current !== null) {
        window.clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
    };
  }, [
    pendingRequiresReview,
    pendingStatus,
    requiresReview,
    rowVirtualizer,
    searchParams,
    setSearchParams,
    normalizedStatus,
  ]);

  useEffect(() => {
    setPendingStatus(normalizedStatus ?? "pending");
    setPendingRequiresReview(requiresReview);
  }, [requiresReview, normalizedStatus]);

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

    if (event.key === "Home") {
      event.preventDefault();
      setActiveIndex(jobs.length ? 0 : null);
    }

    if (event.key === "End") {
      event.preventDefault();
      setActiveIndex(jobs.length ? jobs.length - 1 : null);
    }

    if ((event.key === "Enter" || event.key === " ") && activeJobId) {
      event.preventDefault();
      navigate(`/jobs/${activeJobId}`);
    }
  };

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
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <h1 className="text-2xl font-bold">{t("queue.jobs.title")}</h1>
        {canCreateJob && (
          <Button asChild>
            {/* TODO: Replace with the actual job creation route or modal trigger once implemented. */}
            <Link to="/jobs/new">
              {t("queue.jobs.ctaCreate")}
            </Link>
          </Button>
        )}
      </div>

      {!isQueueAdmin ? (
        <div className="rounded-md border border-dashed bg-muted/40 p-6 text-center text-sm text-muted-foreground">
          {t("queue.access.noAccess")}
        </div>
      ) : (
        <>
          <div className="space-y-2 rounded-md border bg-muted/30 p-4">
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm text-muted-foreground">
                  {t("queue.jobs.filter.statusLabel")}
                </span>
                {[
                  { value: "pending" as StatusFilter, label: t("queue.jobs.filter.unprocessed") },
                  { value: "processing" as StatusFilter, label: t("queue.jobs.filter.onHold") },
                  { value: "completed" as StatusFilter, label: t("status.completed") },
                  { value: "all" as StatusFilter, label: t("queue.jobs.filter.all") },
                ].map((option) => (
                  <Button
                    key={option.value}
                    size="sm"
                    variant={pendingStatus === option.value ? "default" : "outline"}
                    onClick={() => setPendingStatus(option.value)}
                  >
                    {option.label}
                  </Button>
                ))}
              </div>

              <div className="flex items-center gap-2">
                <Switch
                  id="review-filter"
                  aria-label={t("queue.jobs.filter.reviewOnly")}
                  checked={pendingRequiresReview}
                  onCheckedChange={setPendingRequiresReview}
                />
                <label
                  htmlFor="review-filter"
                  className="text-sm text-muted-foreground cursor-pointer"
                >
                  {t("queue.jobs.filter.reviewOnly")}
                </label>
              </div>

              <div className="flex items-center gap-2">
                <span className="text-sm text-muted-foreground">
                  {t("queue.jobs.filter.sort.label")}
                </span>
                <Select value={sortMode} onValueChange={(value) => setSortMode(value as SortMode)}>
                  <SelectTrigger className="w-[170px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="status">{t("queue.jobs.filter.sort.status")}</SelectItem>
                    <SelectItem value="updated">{t("queue.jobs.filter.sort.updated")}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              {(normalizedStatus || requiresReview) && (
                <Button
                  variant="ghost"
                  size="sm"
                  aria-label={t("queue.jobs.filter.clear")}
                  onClick={() => {
                    setSearchParams({}, { replace: true });
                    setPendingStatus("pending");
                    setPendingRequiresReview(false);
                    rowVirtualizer.scrollToIndex(0);
                  }}
                >
                  {t("queue.jobs.filter.clear")}
                </Button>
              )}
            </div>
            <p className="text-xs text-muted-foreground">
              {t("queue.jobs.filter.statusHint")}
            </p>
          </div>

          {isLoading ? (
            <LoadingState />
          ) : jobs.length === 0 ? (
            <div className="flex flex-col items-center justify-center rounded-md border border-dashed p-10 text-center">
              <Inbox className="h-10 w-10 text-muted-foreground" />
              <p className="mt-2 text-lg font-semibold">{t("queue.jobs.empty.title")}</p>
              <p className="text-sm text-muted-foreground">
                {t("queue.jobs.empty.description")}
              </p>
            </div>
          ) : (
            <div className="rounded-md border">
              <div className="overflow-x-auto">
                <div className={cn(COLUMN_LAYOUT, "min-w-[740px] border-b bg-muted/60 px-3 py-2 text-sm font-semibold")}>
                  <span>{t("queue.jobs.table.id")}</span>
                  <span>{t("queue.jobs.table.status")}</span>
                  <span>{t("queue.jobs.table.meta")}</span>
                  <span>{t("queue.jobs.table.updated")}</span>
                </div>
                <div
                  ref={scrollParentRef}
                  className="max-h-[70vh] min-w-[740px] overflow-auto"
                  role="listbox"
                  aria-label={t("queue.jobs.title")}
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
                    {virtualItems.map((virtualRow: VirtualItem) => {
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
                              aria-label={t("queue.jobs.row.link", { id: job.id })}
                              className="text-primary hover:underline font-semibold"
                            >
                              {job.id}
                            </Link>
                          <div className="flex flex-col gap-1">
                            <div className="flex items-center gap-2">
                              <StatusBadge status={job.status} />
                              {job.requiresManualReview && (
                                <Badge variant="secondary">{t("status.review")}</Badge>
                              )}
                            </div>
                            {job.manualReviewReason && (
                              <p className="text-xs text-muted-foreground line-clamp-1">
                                {job.manualReviewReason}
                              </p>
                            )}
                          </div>
                          <div className="flex flex-col gap-2 text-xs text-muted-foreground">
                            <div className="flex flex-wrap gap-2">
                              <Badge variant="outline">prio {job.priority}</Badge>
                              <Badge variant="outline">retry {job.retryCount}</Badge>
                              {job.finalMethod && (
                                <Badge variant="secondary">{job.finalMethod}</Badge>
                              )}
                            </div>
                            {job.decisionReason && (
                              <p className="line-clamp-1">{job.decisionReason}</p>
                            )}
                          </div>
                          <div className="flex flex-col items-end gap-1">
                            <span className="text-xs text-muted-foreground">
                              {formatDate(job.updatedAt)}
                            </span>
                            <Link
                              to={`/jobs/${job.id}`}
                              className="text-sm text-primary hover:underline"
                            >
                              {t("queue.jobs.row.details")}
                            </Link>
                          </div>
                        </div>
                      );
                    })}
                    {isFetchingNextPage && (
                      <div
                        className="absolute left-0 right-0 flex items-center justify-center gap-2 text-sm text-muted-foreground"
                        style={{ top: rowVirtualizer.getTotalSize() }}
                      >
                        <Loader2 className="h-4 w-4 animate-spin" />
                        {t("queue.jobs.loadingMore")}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
