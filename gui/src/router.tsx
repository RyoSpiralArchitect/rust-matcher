import { createBrowserRouter, Navigate } from "react-router-dom";
import { RootLayout } from "./layouts/RootLayout";
import { QueueDashboardPage } from "./pages/QueueDashboardPage";
import { QueueJobsPage } from "./pages/QueueJobsPage";
import { JobDetailPage } from "./pages/JobDetailPage";
import { CandidatesPage } from "./pages/CandidatesPage";

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
        element: <QueueDashboardPage />,
      },
      // Queue Jobs 一覧
      {
        path: "jobs",
        element: <QueueJobsPage />,
      },
      // Job 詳細
      {
        path: "jobs/:jobId",
        element: <JobDetailPage />,
      },
      // 候補一覧（プロジェクト単位）
      {
        path: "projects/:projectId/candidates",
        element: <CandidatesPage />,
      },
    ],
  },
]);
