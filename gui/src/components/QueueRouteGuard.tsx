import { type ReactNode } from "react";
import { Navigate, useLocation } from "react-router-dom";
import { useFlags } from "@/lib/auth";

export function QueueRouteGuard({ children }: { children: ReactNode }) {
  const { isQueueAdmin } = useFlags();
  const location = useLocation();

  if (!isQueueAdmin) {
    return (
      <Navigate
        to="/projects"
        replace
        state={{
          from: location.pathname,
          reason: "queue-admins-only",
        }}
      />
    );
  }

  return <>{children}</>;
}
