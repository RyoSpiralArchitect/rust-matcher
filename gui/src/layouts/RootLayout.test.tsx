import { beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { I18nProvider } from "@/lib/i18n";
import { RootLayout } from "./RootLayout";

let mockIsAdmin = false;

vi.mock("@/lib/auth", () => ({
  useFlags: () => ({
    roles: [],
    isQueueAdmin: mockIsAdmin,
  }),
}));

function renderAtPath(path: string) {
  const router = createMemoryRouter(
    [
      {
        path: "/",
        element: <RootLayout />,
        children: [
          {
            path: "*",
            element: <div>Page</div>,
          },
        ],
      },
    ],
    { initialEntries: [path] },
  );

  return render(
    <I18nProvider locale="en">
      <RouterProvider router={router} />
    </I18nProvider>,
  );
}

describe("RootLayout navigation", () => {
  beforeEach(() => {
    mockIsAdmin = false;
  });

  it.each([
    { path: "/queue", active: /Dashboard/ },
    { path: "/jobs/99", active: /Jobs/ },
  ])("shows admin links when queue access is enabled for %s", ({ path, active }) => {
    mockIsAdmin = true;
    renderAtPath(path);

    const navLabels = ["Dashboard", "Jobs", "Projects", "Talents"] as const;

    navLabels.forEach((label) => {
      const link = screen.getByRole("link", { name: new RegExp(label) });
      if (active.test(label)) {
        expect(link.className).toContain("text-primary");
      } else {
        expect(link.className).toContain("text-muted-foreground");
      }
    });
  });

  it("hides admin-only links for non-queue users", () => {
    renderAtPath("/talents/7");

    expect(screen.queryByRole("link", { name: /Dashboard/ })).toBeNull();
    expect(screen.queryByRole("link", { name: /Jobs/ })).toBeNull();

    const projectsLink = screen.getByRole("link", { name: "Projects" });
    expect(projectsLink.className).toContain("text-muted-foreground");

    const talentsLink = screen.getByRole("link", { name: "Talents" });
    expect(talentsLink.className).toContain("text-primary");
  });
});
