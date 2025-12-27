import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { I18nProvider } from "@/lib/i18n";
import { RootLayout } from "./RootLayout";

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
  it.each([
    { path: "/queue", active: "Dashboard" },
    { path: "/jobs/99", active: "Jobs" },
    { path: "/projects/42", active: "Projects" },
    { path: "/projects/42/candidates", active: "Projects" },
    { path: "/talents/7", active: "Talents" },
  ])("highlights %s for %s routes", ({ path, active }) => {
    renderAtPath(path);

    const navLabels = ["Dashboard", "Jobs", "Projects", "Talents"] as const;

    navLabels.forEach((label) => {
      const link = screen.getByRole("link", { name: label });
      if (label === active) {
        expect(link.className).toContain("text-primary");
      } else {
        expect(link.className).toContain("text-muted-foreground");
      }
    });
  });
});
