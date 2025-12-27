import { Suspense, lazy, type ReactNode } from "react";
import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
import { RootLayout } from "./layouts/RootLayout";
import { LoadingState } from "./components/LoadingState";
import { QueueRouteGuard } from "./components/QueueRouteGuard";

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
  import("./pages/projects/ProjectsListPage").then((module) => ({
    default: module.ProjectsListPage,
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
        element: (
          <QueueRouteGuard>
            {withSuspense(<QueueDashboardPage />)}
          </QueueRouteGuard>
        ),
      },
      // Queue Jobs 一覧
      {
        path: "jobs",
        element: (
          <QueueRouteGuard>
            {withSuspense(<QueueJobsPage />)}
          </QueueRouteGuard>
        ),
      },
      // Job 詳細
      {
        path: "jobs/:jobId",
        element: (
          <QueueRouteGuard>
            {withSuspense(<JobDetailPage />)}
          </QueueRouteGuard>
        ),
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
