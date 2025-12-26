type ErrorDisplayProps = {
  error: unknown;
  fallback?: string;
};

export function ErrorDisplay({ error, fallback = "Unknown error" }: ErrorDisplayProps) {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : fallback;

  return <div className="text-destructive">Error: {message}</div>;
}
