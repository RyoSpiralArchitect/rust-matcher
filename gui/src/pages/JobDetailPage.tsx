import { useMemo } from "react";
import { useParams } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Loader2, ThumbsUp, ThumbsDown, Check, X } from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toast } from "sonner";
import { useJobDetail, useRetryJob, useSendFeedback, useSendConversion } from "@/api";
import type {
  JobEntity,
  PairDetail,
  FeedbackEventRow,
  FeedbackType,
  ConversionStage,
} from "@/api";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { StatusBadge } from "@/components/StatusBadge";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";
import { useFlags } from "@/lib/auth";
import { Breadcrumbs } from "@/components/Breadcrumbs";

export function JobDetailPage() {
  const { jobId } = useParams<{ jobId: string }>();
  const { data, isLoading, error } = useJobDetail(jobId);
  const retryMutation = useRetryJob();
  const feedbackMutation = useSendFeedback();
  const conversionMutation = useSendConversion();
  const { t, locale } = useI18n();
  const { isQueueAdmin } = useFlags();

  const feedbackLabels: Record<string, string> = useMemo(
    () => ({
      thumbs_up: t("feedback.thumbs_up"),
      thumbs_down: t("feedback.thumbs_down"),
      review_ok: t("feedback.review_ok"),
      review_ng: t("feedback.review_ng"),
      review_pending: t("feedback.review_pending"),
      accepted: t("feedback.accepted"),
      rejected: t("feedback.rejected"),
      interview_scheduled: t("feedback.interview_scheduled"),
      no_response: t("feedback.no_response"),
    }),
    [t],
  );

  const conversionStageLabels = useMemo(
    () => ({
      contacted: t("conversionStage.contacted"),
      entry: t("conversionStage.entry"),
      interview_scheduled: t("conversionStage.interview_scheduled"),
      offer: t("conversionStage.offer"),
      contract_signed: t("conversionStage.contract_signed"),
      lost: t("conversionStage.lost"),
    }),
    [t],
  );

  const conversionStages = useMemo(
    () =>
      CONVERSION_STAGE_VALUES.map((stage) => ({
        value: stage,
        label: conversionStageLabels[stage],
      })),
    [conversionStageLabels],
  );

  const formatDateTime = (value: string) =>
    new Intl.DateTimeFormat(locale, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(value));

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  if (isLoading || !data) {
    return <LoadingState />;
  }

  const { job, entity, pairs, lastError, partialFields, llmLatencyMs } = data;

  const handleRetry = () => {
    if (jobId) {
      retryMutation.mutate(jobId, {
        onSuccess: () => {
          toast.success(t("jobDetail.retry.success"));
        },
        onError: (err) => {
          toast.error(t("jobDetail.retry.failed", { message: err.message }));
        },
      });
    }
  };

  const handleFeedback = (interactionId: number, feedbackType: FeedbackType) => {
    const label = feedbackLabels[feedbackType] ?? feedbackType.replace("_", " ");
    feedbackMutation.mutate(
      { interactionId, feedbackType, source: "gui" },
      {
        onSuccess: () => {
          toast.success(t("jobDetail.feedback.submitted", { label }));
        },
        onError: (err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          toast.error(t("jobDetail.feedback.failed", { message }));
        },
      }
    );
  };

  const handleConversion = (
    interactionId: number | undefined,
    talentId: number,
    projectId: number,
    stage: ConversionStage
  ) => {
    conversionMutation.mutate(
      { interactionId, talentId, projectId, stage },
      {
        onSuccess: () => {
          const label = conversionStageLabels[stage] ?? stage.replace("_", " ");
          toast.success(t("jobDetail.conversion.success", { label }));
        },
        onError: (err) => {
          toast.error(t("jobDetail.conversion.failed", { message: err.message }));
        },
      }
    );
  };

  const breadcrumb = (
    <Breadcrumbs
      items={[
        { label: t("navigation.jobs"), href: "/jobs" },
        { label: t("jobDetail.breadcrumb.job", { id: jobId ?? "" }), isCurrent: true },
      ]}
    />
  );

  const header = (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold">{t("jobDetail.title", { id: jobId ?? "" })}</h1>
          <StatusBadge status={job.status} />
          {job.requiresManualReview && (
            <Badge variant="secondary">{t("jobDetail.reviewRequired")}</Badge>
          )}
        </div>
        <Button
          onClick={handleRetry}
          disabled={retryMutation.isPending || job.status === "processing"}
          variant="outline"
        >
          {retryMutation.isPending ? t("jobDetail.retrying") : t("jobDetail.retry")}
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">
        {t("queue.access.note")}
      </p>
    </div>
  );

  if (!isQueueAdmin) {
    return (
      <div className="space-y-6">
        {breadcrumb}
        {header}
        <div className="rounded-md border border-dashed bg-muted/40 p-6 text-center text-sm text-muted-foreground">
          {t("queue.access.noAccess")}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {breadcrumb}
      {header}

      {/* Summary */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t("jobDetail.summary")}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <div className="text-muted-foreground">{t("jobDetail.priority")}</div>
              <div className="font-medium">{job.priority}</div>
            </div>
            <div>
              <div className="text-muted-foreground">{t("jobDetail.retryCount")}</div>
              <div className="font-medium">{job.retryCount}</div>
            </div>
            <div>
              <div className="text-muted-foreground">{t("jobDetail.finalMethod")}</div>
              <div className="font-medium">{job.finalMethod ?? "-"}</div>
            </div>
            <div>
              <div className="text-muted-foreground">{t("jobDetail.llmLatency")}</div>
              <div className="font-medium">
                {llmLatencyMs ? `${llmLatencyMs}ms` : "-"}
              </div>
            </div>
            <div className="col-span-2">
              <div className="text-muted-foreground">{t("jobDetail.decision")}</div>
              <div className="font-medium">{job.decisionReason ?? "-"}</div>
            </div>
            <div className="col-span-2">
              <div className="text-muted-foreground">{t("jobDetail.updated")}</div>
              <div className="font-medium">
                {formatDateTime(job.updatedAt)}
              </div>
            </div>
          </div>

          {lastError && (
            <div className="mt-4 p-3 bg-destructive/10 rounded-md border border-destructive/20">
              <div className="text-sm font-medium text-destructive">
                {t("jobDetail.lastError")}
              </div>
              <pre className="text-xs mt-1 whitespace-pre-wrap text-destructive/80">
                {lastError}
              </pre>
            </div>
          )}

          {partialFields && Object.keys(partialFields).length > 0 && (
            <div className="mt-4">
              <div className="text-sm text-muted-foreground mb-2">
                {t("jobDetail.extractedFields")}
              </div>
              <pre className="text-xs p-3 bg-muted rounded-md overflow-auto">
                {JSON.stringify(partialFields, null, 2)}
              </pre>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Entity */}
      {entity && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">{t("jobDetail.entity")}</CardTitle>
          </CardHeader>
          <CardContent>
            <EntityDisplay entity={entity} />
          </CardContent>
        </Card>
      )}

      {/* Match Pairs */}
      {pairs && pairs.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">
              {t("jobDetail.matchPairs", { count: pairs.length })}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <PairsTable
              pairs={pairs}
              onFeedback={handleFeedback}
              onConversion={handleConversion}
              conversionStages={conversionStages}
              feedbackLabels={feedbackLabels}
              isSubmitting={feedbackMutation.isPending || conversionMutation.isPending}
            />
          </CardContent>
        </Card>
      )}

      {/* Timeline */}
      {pairs && pairs.length > 0 && (
        <TimelineCard
          pairs={pairs}
          feedbackLabels={feedbackLabels}
          formatDateTime={formatDateTime}
        />
      )}
    </div>
  );
}

function EntityDisplay({ entity }: { entity: JobEntity }) {
  const { t } = useI18n();

  if (entity.type === "talent") {
    return (
      <div className="space-y-2">
        <Badge>{t("jobDetail.field.talent")}</Badge>
        <div className="grid grid-cols-2 gap-4 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.id")}</div>
            <div className="font-medium">{entity.id}</div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.name")}</div>
            <div className="font-medium">{entity.talentName ?? "-"}</div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.desiredPriceMin")}</div>
            <div className="font-medium">
              {entity.desiredPriceMin ? `¥${entity.desiredPriceMin.toLocaleString()}` : "-"}
            </div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.availableDate")}</div>
            <div className="font-medium">{entity.availableDate ?? "-"}</div>
          </div>
          {entity.summaryText && (
            <div className="col-span-2">
              <div className="text-muted-foreground">{t("jobDetail.field.summary")}</div>
              <div className="font-medium text-xs">{entity.summaryText}</div>
            </div>
          )}
        </div>
      </div>
    );
  }

  if (entity.type === "project") {
    return (
      <div className="space-y-2">
        <Badge variant="secondary">{t("jobDetail.field.project")}</Badge>
        <div className="grid grid-cols-2 gap-4 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.code")}</div>
            <div className="font-medium">{entity.projectCode}</div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.name")}</div>
            <div className="font-medium">{entity.projectName}</div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.priceRange")}</div>
            <div className="font-medium">
              {entity.monthlyTankaMin || entity.monthlyTankaMax
                ? `¥${entity.monthlyTankaMin?.toLocaleString() ?? "?"} - ¥${entity.monthlyTankaMax?.toLocaleString() ?? "?"}`
                : "-"}
            </div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.startDate")}</div>
            <div className="font-medium">{entity.startDate ?? "-"}</div>
          </div>
        </div>
      </div>
    );
  }

  // Both
  return (
    <div className="grid md:grid-cols-2 gap-6">
      <div className="space-y-2">
        <Badge>{t("jobDetail.field.talent")}</Badge>
        <div className="grid grid-cols-2 gap-3 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.id")}</div>
            <div className="font-medium">{entity.talent.id}</div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.name")}</div>
            <div className="font-medium">{entity.talent.talentName ?? "-"}</div>
          </div>
          <div className="col-span-2">
            <div className="text-muted-foreground">{t("jobDetail.field.desiredPrice")}</div>
            <div className="font-medium">
              {entity.talent.desiredPriceMin
                ? `¥${entity.talent.desiredPriceMin.toLocaleString()}`
                : "-"}
            </div>
          </div>
        </div>
      </div>
      <div className="space-y-2">
        <Badge variant="secondary">{t("jobDetail.field.project")}</Badge>
        <div className="grid grid-cols-2 gap-3 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.code")}</div>
            <div className="font-medium">{entity.project.projectCode}</div>
          </div>
          <div>
            <div className="text-muted-foreground">{t("jobDetail.field.name")}</div>
            <div className="font-medium">{entity.project.projectName}</div>
          </div>
          <div className="col-span-2">
            <div className="text-muted-foreground">{t("jobDetail.field.priceRange")}</div>
            <div className="font-medium">
              {entity.project.monthlyTankaMin || entity.project.monthlyTankaMax
                ? `¥${entity.project.monthlyTankaMin?.toLocaleString() ?? "?"} - ¥${entity.project.monthlyTankaMax?.toLocaleString() ?? "?"}`
                : "-"}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

const CONVERSION_STAGE_VALUES: ConversionStage[] = [
  "contacted",
  "entry",
  "interview_scheduled",
  "offer",
  "contract_signed",
  "lost",
];

interface PairsTableProps {
  pairs: PairDetail[];
  onFeedback: (interactionId: number, feedbackType: FeedbackType) => void;
  onConversion: (
    interactionId: number | undefined,
    talentId: number,
    projectId: number,
    stage: ConversionStage
  ) => void;
  conversionStages: { value: ConversionStage; label: string }[];
  feedbackLabels: Record<string, string>;
  isSubmitting: boolean;
}

function PairsTable({
  pairs,
  onFeedback,
  onConversion,
  conversionStages,
  feedbackLabels,
  isSubmitting,
}: PairsTableProps) {
  const { t } = useI18n();
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{t("jobDetail.table.talentId")}</TableHead>
          <TableHead>{t("jobDetail.table.projectId")}</TableHead>
          <TableHead>{t("jobDetail.table.score")}</TableHead>
          <TableHead>{t("jobDetail.table.ko")}</TableHead>
          <TableHead>{t("jobDetail.table.feedback")}</TableHead>
          <TableHead>{t("jobDetail.table.stage")}</TableHead>
          <TableHead>{t("jobDetail.table.actions")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {pairs.map((pair) => {
          const interactionId = pair.latestInteraction?.id;
          const hasPositiveFb = pair.feedbackEvents.some(
            (fb) => fb.feedbackType === "thumbs_up" || fb.feedbackType === "review_ok"
          );
          const hasNegativeFb = pair.feedbackEvents.some(
            (fb) => fb.feedbackType === "thumbs_down" || fb.feedbackType === "review_ng"
          );

          return (
            <TableRow key={pair.matchResult.id}>
              <TableCell className="font-mono">{pair.matchResult.talentId}</TableCell>
              <TableCell className="font-mono">{pair.matchResult.projectId}</TableCell>
              <TableCell>
                {pair.matchResult.scoreTotal?.toFixed(2) ?? "-"}
              </TableCell>
              <TableCell>
                {pair.matchResult.isKnockout ? (
                  <Badge variant="destructive">{t("jobDetail.table.ko")}</Badge>
                ) : pair.matchResult.needsManualReview ? (
                  <Badge variant="secondary">{t("jobDetail.table.reviewLabel")}</Badge>
                ) : (
                  <Badge variant="outline">{t("jobDetail.table.okLabel")}</Badge>
                )}
              </TableCell>
              <TableCell>
                <div className="flex gap-1 flex-wrap">
                  {pair.feedbackEvents.length === 0 ? (
                    <span className="text-muted-foreground text-xs">-</span>
                  ) : (
                    pair.feedbackEvents.map((fb) => (
                      <FeedbackBadge
                        key={fb.id}
                        feedback={fb}
                        label={feedbackLabels[fb.feedbackType] ?? fb.feedbackType}
                      />
                    ))
                  )}
                </div>
              </TableCell>
              <TableCell>
                <Select
                  onValueChange={(value) =>
                    onConversion(
                      interactionId,
                      pair.matchResult.talentId,
                      pair.matchResult.projectId,
                      value as ConversionStage
                    )
                  }
                  disabled={isSubmitting}
                >
                  <SelectTrigger size="sm" className="w-28">
                    <SelectValue placeholder={t("jobDetail.table.stage")} />
                  </SelectTrigger>
                  <SelectContent>
                    {conversionStages.map((stage) => (
                      <SelectItem key={stage.value} value={stage.value}>
                        {stage.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </TableCell>
              <TableCell>
                {interactionId ? (
                  <div className="flex gap-1">
                    {pair.matchResult.needsManualReview ? (
                      <>
                        <Button
                          variant={hasPositiveFb ? "default" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "review_ok")}
                          disabled={isSubmitting}
                          title={t("jobDetail.table.reviewOk")}
                        >
                          {isSubmitting ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <Check className="h-3 w-3" />
                          )}
                        </Button>
                        <Button
                          variant={hasNegativeFb ? "destructive" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "review_ng")}
                          disabled={isSubmitting}
                          title={t("jobDetail.table.reviewNg")}
                        >
                          {isSubmitting ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <X className="h-3 w-3" />
                          )}
                        </Button>
                      </>
                    ) : (
                      <>
                        <Button
                          variant={hasPositiveFb ? "default" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "thumbs_up")}
                          disabled={isSubmitting}
                          title={t("jobDetail.table.thumbsUp")}
                        >
                          {isSubmitting ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <ThumbsUp className="h-3 w-3" />
                          )}
                        </Button>
                        <Button
                          variant={hasNegativeFb ? "destructive" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "thumbs_down")}
                          disabled={isSubmitting}
                          title={t("jobDetail.table.thumbsDown")}
                        >
                          {isSubmitting ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <ThumbsDown className="h-3 w-3" />
                          )}
                        </Button>
                      </>
                    )}
                  </div>
                ) : (
                  <span className="text-xs text-muted-foreground">-</span>
                )}
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}

function FeedbackBadge({ feedback, label }: { feedback: FeedbackEventRow; label: string }) {
  const variant = feedback.feedbackType.includes("up") || feedback.feedbackType.includes("ok")
    ? "default"
    : feedback.feedbackType.includes("down") || feedback.feedbackType.includes("ng")
      ? "destructive"
      : "secondary";

  return (
    <Badge variant={variant} className="text-xs">
      {label}
    </Badge>
  );
}

interface TimelineItem {
  type: "feedback" | "event";
  id: number;
  date: string;
  actor: string;
  label: string;
  talentId: number;
  projectId: number;
}

function TimelineCard({
  pairs,
  feedbackLabels,
  formatDateTime,
}: {
  pairs: PairDetail[];
  feedbackLabels: Record<string, string>;
  formatDateTime: (value: string) => string;
}) {
  const { t } = useI18n();
  const eventLabels: Record<string, string> = useMemo(
    () => ({
      viewed_candidate_detail: t("event.viewed_candidate_detail"),
      copied_template: t("event.copied_template"),
      clicked_contact: t("event.clicked_contact"),
      shortlisted: t("event.shortlisted"),
    }),
    [t],
  );

  const items: TimelineItem[] = [];

  for (const pair of pairs) {
    for (const fb of pair.feedbackEvents) {
      items.push({
        type: "feedback",
        id: fb.id,
        date: fb.createdAt,
        actor: fb.actor,
        label: feedbackLabels[fb.feedbackType] ?? fb.feedbackType,
        talentId: fb.talentId,
        projectId: fb.projectId,
      });
    }
    for (const ev of pair.interactionEvents) {
      items.push({
        type: "event",
        id: ev.id,
        date: ev.createdAt,
        actor: ev.actor,
        label: eventLabels[ev.eventType] ?? ev.eventType,
        talentId: pair.matchResult.talentId,
        projectId: pair.matchResult.projectId,
      });
    }
  }

  items.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());

  if (items.length === 0) {
    return null;
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-lg">{t("jobDetail.timeline")}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {items.slice(0, 20).map((item) => (
            <TimelineRow
              key={`${item.type}-${item.id}-${item.date}`}
              item={item}
              formatDateTime={formatDateTime}
            />
          ))}
          {items.length > 20 && (
            <div className="text-sm text-muted-foreground">
              {t("jobDetail.timeline.more", { count: items.length - 20 })}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function TimelineRow({
  item,
  formatDateTime,
}: {
  item: TimelineItem;
  formatDateTime: (value: string) => string;
}) {
  const { t } = useI18n();
  const icon = item.type === "feedback" ? "💬" : "👁";
  const formattedDate = formatDateTime(item.date);

  return (
    <div className="flex items-start gap-3 text-sm">
      <span className="text-muted-foreground w-24 shrink-0">{formattedDate}</span>
      <span>{icon}</span>
      <div className="flex-1">
        <span className="font-medium">{item.label}</span>
        <span className="text-muted-foreground ml-2">
          {t("jobDetail.timeline.byActor", { actor: item.actor })}
        </span>
      </div>
      <span className="text-xs text-muted-foreground font-mono">
        T{item.talentId}/P{item.projectId}
      </span>
    </div>
  );
}
