import { useI18n } from "@/lib/i18n";

type ErrorDisplayProps = {
  error: unknown;
  fallback?: string;
};

export function ErrorDisplay({ error, fallback }: ErrorDisplayProps) {
  const { t } = useI18n();
  const defaultMessage = fallback ?? t("error.unknown");
  const message =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : defaultMessage;

  return <div className="text-destructive">{t("error.prefix")}: {message}</div>;
}
