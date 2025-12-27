import { Suspense, lazy, type ReactNode } from "react";
import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
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
const ProjectsListPage = lazy(() =>
  import("./pages/projects/ProjectsList").then((module) => ({
    default: module.ProjectsList,
  })),
);
const ProjectDetailPage = lazy(() =>
  import("./pages/projects/ProjectDetailPage").then((module) => ({
    default: module.ProjectDetailPage,
  })),
);
const TalentsPage = lazy(() =>
  import("./pages/TalentsPage").then((module) => ({
    default: module.TalentsPage,
  })),
);
const TalentDetailPage = lazy(() =>
  import("./pages/TalentDetailPage").then((module) => ({
    default: module.TalentDetailPage,
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
      // Projects セクション
      {
        path: "projects",
        element: <Outlet />,
        children: [
          {
            index: true,
            element: withSuspense(<ProjectsListPage />),
          },
          {
            path: ":projectId",
            element: withSuspense(<ProjectDetailPage />),
          },
          // 候補一覧（プロジェクト単位）
          {
            path: ":projectId/candidates",
            element: withSuspense(<CandidatesPage />),
          },
        ],
      },
      // Talents セクション
      {
        path: "talents",
        element: <Outlet />,
        children: [
          {
            index: true,
            element: withSuspense(<TalentsPage />),
          },
          {
            path: ":id",
            element: withSuspense(<TalentDetailPage />),
          },
        ],
      },
    ],
  },
]);
