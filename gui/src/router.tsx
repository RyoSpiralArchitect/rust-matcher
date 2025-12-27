import { Suspense, lazy, type ReactNode } from "react";
import { createBrowserRouter, Navigate } from "react-router-dom";
import { RootLayout } from "./layouts/RootLayout";
import { LoadingState } from "./components/LoadingState";

const QueueDashboardPage = lazy(() =>
  import("./pages/QueueDashboardPage").then((module) => ({
    default: module.QueueDashboardPage,
  })),
);
const QueueJobsPage = lazy(() =>
  import("./pages/QueueJobsPage").then((module) => ({
    default: module.QueueJobsPage,
  })),
);
const JobDetailPage = lazy(() =>
  import("./pages/JobDetailPage").then((module) => ({
    default: module.JobDetailPage,
  })),
);
const CandidatesPage = lazy(() =>
  import("./pages/CandidatesPage").then((module) => ({
    default: module.CandidatesPage,
  })),
);
const TalentsPage = lazy(() =>
  import("./pages/TalentsPage").then((module) => ({
    default: module.TalentsPage,
  })),
);

function withSuspense(element: ReactNode) {
  return <Suspense fallback={<LoadingState />}>{element}</Suspense>;
}

export const router = createBrowserRouter([
  {
    path: "/",
    element: <RootLayout />,
    children: [
      // / → /queue にリダイレクト
      {
        index: true,
        element: <Navigate to="/queue" replace />,
      },
      // Queue Dashboard
      {
        path: "queue",
        element: withSuspense(<QueueDashboardPage />),
      },
      // Queue Jobs 一覧
      {
        path: "jobs",
        element: withSuspense(<QueueJobsPage />),
      },
      // Job 詳細
      {
        path: "jobs/:jobId",
        element: withSuspense(<JobDetailPage />),
      },
      // 候補一覧（プロジェクト単位）
      {
        path: "projects/:projectId/candidates",
        element: withSuspense(<CandidatesPage />),
      },
      // タレント一覧
      {
        path: "talents",
        element: withSuspense(<TalentsPage />),
      },
    ],
  },
]);
