import { useParams, Link } from "react-router-dom";
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
import { ThumbsUp, ThumbsDown, Check, X } from "lucide-react";
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
import { LoadingState } from "@/components/LoadingState";

export function JobDetailPage() {
  const { jobId } = useParams<{ jobId: string }>();
  const { data, isLoading, error } = useJobDetail(jobId);
  const retryMutation = useRetryJob();
  const feedbackMutation = useSendFeedback();
  const conversionMutation = useSendConversion();

  if (error) {
    return (
      <div className="text-destructive">
        Error: {error instanceof Error ? error.message : "Unknown error"}
      </div>
    );
  }

  if (isLoading || !data) {
    return <LoadingState />;
  }

  const { job, entity, pairs, lastError, partialFields, llmLatencyMs } = data;

  const handleRetry = () => {
    if (jobId) {
      retryMutation.mutate(jobId, {
        onSuccess: () => {
          toast.success("Job retry initiated");
        },
        onError: (err) => {
          toast.error(`Failed to retry job: ${err.message}`);
        },
      });
    }
  };

  const handleFeedback = (interactionId: number, feedbackType: FeedbackType) => {
    feedbackMutation.mutate(
      { interactionId, feedbackType, source: "gui" },
      {
        onSuccess: () => {
          const label = feedbackType.replace("_", " ");
          toast.success(`Feedback "${label}" submitted`);
        },
        onError: (err) => {
          toast.error(`Failed to submit feedback: ${err.message}`);
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
          const label = stage.replace("_", " ");
          toast.success(`Stage updated to "${label}"`);
        },
        onError: (err) => {
          toast.error(`Failed to update stage: ${err.message}`);
        },
      }
    );
  };

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Link to="/jobs" className="hover:underline">
          Jobs
        </Link>
        <span>/</span>
        <span>{jobId}</span>
      </div>

      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold">Job #{jobId}</h1>
          <StatusBadge status={job.status} />
          {job.requiresManualReview && (
            <Badge variant="secondary">Review Required</Badge>
          )}
        </div>
        <Button
          onClick={handleRetry}
          disabled={retryMutation.isPending || job.status === "processing"}
          variant="outline"
        >
          {retryMutation.isPending ? "Retrying..." : "Retry"}
        </Button>
      </div>

      {/* Summary */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Summary</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <div className="text-muted-foreground">Priority</div>
              <div className="font-medium">{job.priority}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Retry Count</div>
              <div className="font-medium">{job.retryCount}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Final Method</div>
              <div className="font-medium">{job.finalMethod ?? "-"}</div>
            </div>
            <div>
              <div className="text-muted-foreground">LLM Latency</div>
              <div className="font-medium">
                {llmLatencyMs ? `${llmLatencyMs}ms` : "-"}
              </div>
            </div>
            <div className="col-span-2">
              <div className="text-muted-foreground">Decision</div>
              <div className="font-medium">{job.decisionReason ?? "-"}</div>
            </div>
            <div className="col-span-2">
              <div className="text-muted-foreground">Updated</div>
              <div className="font-medium">
                {new Date(job.updatedAt).toLocaleString("ja-JP")}
              </div>
            </div>
          </div>

          {lastError && (
            <div className="mt-4 p-3 bg-destructive/10 rounded-md border border-destructive/20">
              <div className="text-sm font-medium text-destructive">
                Last Error
              </div>
              <pre className="text-xs mt-1 whitespace-pre-wrap text-destructive/80">
                {lastError}
              </pre>
            </div>
          )}

          {partialFields && Object.keys(partialFields).length > 0 && (
            <div className="mt-4">
              <div className="text-sm text-muted-foreground mb-2">
                Extracted Fields
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
            <CardTitle className="text-lg">Entity</CardTitle>
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
            <CardTitle className="text-lg">Match Pairs ({pairs.length})</CardTitle>
          </CardHeader>
          <CardContent>
            <PairsTable
              pairs={pairs}
              onFeedback={handleFeedback}
              onConversion={handleConversion}
              isSubmitting={feedbackMutation.isPending || conversionMutation.isPending}
            />
          </CardContent>
        </Card>
      )}

      {/* Timeline */}
      {pairs && pairs.length > 0 && <TimelineCard pairs={pairs} />}
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

function EntityDisplay({ entity }: { entity: JobEntity }) {
  if (entity.type === "talent") {
    return (
      <div className="space-y-2">
        <Badge>Talent</Badge>
        <div className="grid grid-cols-2 gap-4 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">ID</div>
            <div className="font-medium">{entity.id}</div>
          </div>
          <div>
            <div className="text-muted-foreground">Name</div>
            <div className="font-medium">{entity.talentName ?? "-"}</div>
          </div>
          <div>
            <div className="text-muted-foreground">Desired Price Min</div>
            <div className="font-medium">
              {entity.desiredPriceMin ? `¥${entity.desiredPriceMin.toLocaleString()}` : "-"}
            </div>
          </div>
          <div>
            <div className="text-muted-foreground">Available Date</div>
            <div className="font-medium">{entity.availableDate ?? "-"}</div>
          </div>
          {entity.summaryText && (
            <div className="col-span-2">
              <div className="text-muted-foreground">Summary</div>
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
        <Badge variant="secondary">Project</Badge>
        <div className="grid grid-cols-2 gap-4 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">Code</div>
            <div className="font-medium">{entity.projectCode}</div>
          </div>
          <div>
            <div className="text-muted-foreground">Name</div>
            <div className="font-medium">{entity.projectName}</div>
          </div>
          <div>
            <div className="text-muted-foreground">Price Range</div>
            <div className="font-medium">
              {entity.monthlyTankaMin || entity.monthlyTankaMax
                ? `¥${entity.monthlyTankaMin?.toLocaleString() ?? "?"} - ¥${entity.monthlyTankaMax?.toLocaleString() ?? "?"}`
                : "-"}
            </div>
          </div>
          <div>
            <div className="text-muted-foreground">Start Date</div>
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
        <Badge>Talent</Badge>
        <div className="grid grid-cols-2 gap-3 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">ID</div>
            <div className="font-medium">{entity.talent.id}</div>
          </div>
          <div>
            <div className="text-muted-foreground">Name</div>
            <div className="font-medium">{entity.talent.talentName ?? "-"}</div>
          </div>
          <div className="col-span-2">
            <div className="text-muted-foreground">Desired Price</div>
            <div className="font-medium">
              {entity.talent.desiredPriceMin
                ? `¥${entity.talent.desiredPriceMin.toLocaleString()}`
                : "-"}
            </div>
          </div>
        </div>
      </div>
      <div className="space-y-2">
        <Badge variant="secondary">Project</Badge>
        <div className="grid grid-cols-2 gap-3 text-sm mt-2">
          <div>
            <div className="text-muted-foreground">Code</div>
            <div className="font-medium">{entity.project.projectCode}</div>
          </div>
          <div>
            <div className="text-muted-foreground">Name</div>
            <div className="font-medium">{entity.project.projectName}</div>
          </div>
          <div className="col-span-2">
            <div className="text-muted-foreground">Price Range</div>
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

const CONVERSION_STAGES: { value: ConversionStage; label: string }[] = [
  { value: "contacted", label: "Contacted" },
  { value: "entry", label: "Entry" },
  { value: "interview_scheduled", label: "Interview" },
  { value: "offer", label: "Offer" },
  { value: "contract_signed", label: "Contract" },
  { value: "lost", label: "Lost" },
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
  isSubmitting: boolean;
}

function PairsTable({ pairs, onFeedback, onConversion, isSubmitting }: PairsTableProps) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Talent ID</TableHead>
          <TableHead>Project ID</TableHead>
          <TableHead>Score</TableHead>
          <TableHead>KO</TableHead>
          <TableHead>Feedback</TableHead>
          <TableHead>Stage</TableHead>
          <TableHead>Actions</TableHead>
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
                  <Badge variant="destructive">KO</Badge>
                ) : pair.matchResult.needsManualReview ? (
                  <Badge variant="secondary">Review</Badge>
                ) : (
                  <Badge variant="outline">OK</Badge>
                )}
              </TableCell>
              <TableCell>
                <div className="flex gap-1 flex-wrap">
                  {pair.feedbackEvents.length === 0 ? (
                    <span className="text-muted-foreground text-xs">-</span>
                  ) : (
                    pair.feedbackEvents.map((fb) => (
                      <FeedbackBadge key={fb.id} feedback={fb} />
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
                    <SelectValue placeholder="Stage" />
                  </SelectTrigger>
                  <SelectContent>
                    {CONVERSION_STAGES.map((stage) => (
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
                          title="Review OK"
                        >
                          <Check className="h-3 w-3" />
                        </Button>
                        <Button
                          variant={hasNegativeFb ? "destructive" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "review_ng")}
                          disabled={isSubmitting}
                          title="Review NG"
                        >
                          <X className="h-3 w-3" />
                        </Button>
                      </>
                    ) : (
                      <>
                        <Button
                          variant={hasPositiveFb ? "default" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "thumbs_up")}
                          disabled={isSubmitting}
                          title="Thumbs Up"
                        >
                          <ThumbsUp className="h-3 w-3" />
                        </Button>
                        <Button
                          variant={hasNegativeFb ? "destructive" : "outline"}
                          size="sm"
                          onClick={() => onFeedback(interactionId, "thumbs_down")}
                          disabled={isSubmitting}
                          title="Thumbs Down"
                        >
                          <ThumbsDown className="h-3 w-3" />
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

function FeedbackBadge({ feedback }: { feedback: FeedbackEventRow }) {
  const variant = feedback.feedbackType.includes("up") || feedback.feedbackType.includes("ok")
    ? "default"
    : feedback.feedbackType.includes("down") || feedback.feedbackType.includes("ng")
      ? "destructive"
      : "secondary";

  return (
    <Badge variant={variant} className="text-xs">
      {feedback.feedbackType}
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

function TimelineCard({ pairs }: { pairs: PairDetail[] }) {
  const items: TimelineItem[] = [];

  for (const pair of pairs) {
    for (const fb of pair.feedbackEvents) {
      items.push({
        type: "feedback",
        id: fb.id,
        date: fb.createdAt,
        actor: fb.actor,
        label: fb.feedbackType,
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
        label: ev.eventType,
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
        <CardTitle className="text-lg">Timeline</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {items.slice(0, 20).map((item) => (
            <TimelineRow key={`${item.type}-${item.id}-${item.date}`} item={item} />
          ))}
          {items.length > 20 && (
            <div className="text-sm text-muted-foreground">
              ... and {items.length - 20} more events
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function TimelineRow({ item }: { item: TimelineItem }) {
  const icon = item.type === "feedback" ? "💬" : "👁";
  const formattedDate = new Date(item.date).toLocaleString("ja-JP", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <div className="flex items-start gap-3 text-sm">
      <span className="text-muted-foreground w-24 shrink-0">{formattedDate}</span>
      <span>{icon}</span>
      <div className="flex-1">
        <span className="font-medium">{item.label}</span>
        <span className="text-muted-foreground ml-2">by {item.actor}</span>
      </div>
      <span className="text-xs text-muted-foreground font-mono">
        T{item.talentId}/P{item.projectId}
      </span>
    </div>
  );
}
