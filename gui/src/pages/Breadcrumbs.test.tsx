import { render, screen, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { vi } from "vitest";
import { I18nProvider } from "@/lib/i18n";
import { JobDetailPage } from "./JobDetailPage";
import { ProjectDetailPage } from "./projects/ProjectDetailPage";
import { TalentDetailPage } from "./TalentDetailPage";

const mockJobDetail = {
  job: {
    status: "completed",
    requiresManualReview: false,
    priority: "normal",
    retryCount: 0,
    finalMethod: null,
    updatedAt: "2024-01-01T00:00:00.000Z",
    decisionReason: null,
  },
  entity: {
    type: "talent",
    id: 1,
    talentName: "Alice",
    desiredPriceMin: null,
    availableDate: null,
    summaryText: null,
  },
  pairs: [],
  lastError: null,
  partialFields: null,
  llmLatencyMs: null,
};

const mockProjectDetail = {
  name: "Project A",
  summary: null,
  rateMin: null,
  rateMax: null,
  workStyle: null,
  skills: [],
  matches: [],
};

const mockTalentDetail = {
  talent: {
    id: 5,
    name: "Jane Doe",
    title: "Engineer",
    skills: [],
    availability: null,
    availableFrom: null,
    location: null,
    desiredPriceMin: 100000,
  },
  matches: [],
};

vi.mock("@/api", () => ({
  useJobDetail: () => ({ data: mockJobDetail, isLoading: false, error: null }),
  useRetryJob: () => ({ mutate: vi.fn(), isPending: false }),
  useSendFeedback: () => ({ mutate: vi.fn(), isPending: false }),
  useSendConversion: () => ({ mutate: vi.fn(), isPending: false }),
  useProjectDetail: () => ({ data: mockProjectDetail, isLoading: false, error: null }),
  useProjectFeedback: () => ({ mutate: vi.fn(), isPending: false }),
  useProjectMatches: () => ({ data: mockProjectDetail.matches, isLoading: false, error: null }),
  trackViewedDetail: vi.fn(),
  useTalentDetail: () => ({ data: mockTalentDetail, isLoading: false, error: null }),
  useMatchDecision: () => ({ mutate: vi.fn(), isPending: false }),
}));

function renderWithRouter(ui: ReactElement, path: string, initialEntry: string) {
  return render(
    <I18nProvider locale="en">
      <MemoryRouter initialEntries={[initialEntry]}>
        <Routes>
          <Route path={path} element={ui} />
        </Routes>
      </MemoryRouter>
    </I18nProvider>,
  );
}

describe("detail page breadcrumbs", () => {
  it("links job detail back to jobs list", () => {
    renderWithRouter(<JobDetailPage />, "/jobs/:jobId", "/jobs/123");

    const breadcrumb = screen.getByLabelText("Breadcrumb");

    expect(within(breadcrumb).getByRole("link", { name: "Jobs" })).toHaveAttribute("href", "/jobs");
    expect(within(breadcrumb).getByText("Job #123")).toBeInTheDocument();
  });

  it("links project detail back to projects list", () => {
    renderWithRouter(<ProjectDetailPage />, "/projects/:projectId", "/projects/42");

    const breadcrumb = screen.getByLabelText("Breadcrumb");

    expect(within(breadcrumb).getByRole("link", { name: "Projects" })).toHaveAttribute("href", "/projects");
    expect(within(breadcrumb).getByText("Project A")).toBeInTheDocument();
  });

  it("links talent detail back to talents list", () => {
    renderWithRouter(<TalentDetailPage />, "/talents/:id", "/talents/5");

    const breadcrumb = screen.getByLabelText("Breadcrumb");

    expect(within(breadcrumb).getByRole("link", { name: "Talents" })).toHaveAttribute("href", "/talents");
    expect(within(breadcrumb).getByText("Jane Doe")).toBeInTheDocument();
  });
});
