import { render, screen } from "@testing-library/react";
import { LoadingState } from "./LoadingState";
import { I18nProvider } from "@/lib/i18n";

function renderWithI18n(ui: React.ReactElement) {
  return render(<I18nProvider locale="en">{ui}</I18nProvider>);
}

describe("LoadingState", () => {
  it("shows the default loading message", () => {
    renderWithI18n(<LoadingState />);
    expect(screen.getByText("Loading...")).toBeInTheDocument();
  });

  it("renders a custom message", () => {
    renderWithI18n(<LoadingState message="Fetching data..." />);
    expect(screen.getByText("Fetching data...")).toBeInTheDocument();
  });
});
