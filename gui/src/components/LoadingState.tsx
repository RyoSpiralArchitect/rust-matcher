import { useI18n } from "@/lib/i18n";

type LoadingStateProps = {
  message?: string;
  className?: string;
};

export function LoadingState({
  message,
  className = "text-muted-foreground",
}: LoadingStateProps) {
  const { t } = useI18n();
  return <div className={className}>{message ?? t("loading.default")}</div>;
}
