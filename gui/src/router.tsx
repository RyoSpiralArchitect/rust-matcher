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
const ProjectsIndexPage = lazy(() =>
  import("./pages/projects/ProjectsIndexPage").then((module) => ({
    default: module.ProjectsIndexPage,
  })),
);
const ProjectDetailPage = lazy(() =>
  import("./pages/projects/ProjectDetailPage").then((module) => ({
    default: module.ProjectDetailPage,
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
      // / → /projects にリダイレクト
      {
        index: true,
        element: <Navigate to="/projects" replace />,
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
      // Project 一覧
      {
        path: "projects",
        element: withSuspense(<ProjectsIndexPage />),
      },
      {
        path: "projects/:projectId",
        element: withSuspense(<ProjectDetailPage />),
      },
      // 候補一覧（プロジェクト単位）
      {
        path: "projects/:projectId/candidates",
        element: withSuspense(<CandidatesPage />),
      },
    ],
  },
]);
