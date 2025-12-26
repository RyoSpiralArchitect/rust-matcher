type LoadingStateProps = {
  message?: string;
  className?: string;
};

export function LoadingState({
  message = "Loading...",
  className = "text-muted-foreground",
}: LoadingStateProps) {
  return <div className={className}>{message}</div>;
}
