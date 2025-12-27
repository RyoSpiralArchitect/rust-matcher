import { Badge } from "@/components/ui/badge";
import type { QueueJobStatus } from "@/api";
import { useI18n } from "@/lib/i18n";

const STATUS_VARIANTS: Record<QueueJobStatus, "default" | "secondary" | "destructive"> = {
  pending: "secondary",
  processing: "default",
  completed: "default",
};

const STATUS_LABEL_KEYS: Record<QueueJobStatus, "status.pending" | "status.processing" | "status.completed"> = {
  pending: "status.pending",
  processing: "status.processing",
  completed: "status.completed",
};

export function StatusBadge({ status }: { status: QueueJobStatus }) {
  const { t } = useI18n();
  const variant = STATUS_VARIANTS[status] ?? "default";
  const label = t(STATUS_LABEL_KEYS[status]);
  return <Badge variant={variant}>{label}</Badge>;
}
