import { Outlet, Link, useLocation } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";

const navItems = [
  { href: "/projects", labelKey: "nav.projects", isAdmin: false },
  { href: "/queue", labelKey: "nav.queueAdmin", isAdmin: true },
  { href: "/jobs", labelKey: "nav.jobsAdmin", isAdmin: true },
] as const;

export function RootLayout() {
  const { t } = useI18n();
  const location = useLocation();
  const { t } = useI18n();

  return (
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="border-b bg-card">
        <div className="container mx-auto px-4 py-3 flex items-center justify-between">
          <Link to="/" className="text-xl font-bold">
            SR Matcher
          </Link>
          <nav className="flex gap-4">
            {navItems.map((item) => (
              <Link
                key={item.href}
                to={item.href}
                className={cn(
                  "inline-flex items-center gap-2 rounded-md px-2 py-1 text-sm font-medium transition-colors hover:text-primary",
                  location.pathname === item.href ||
                    location.pathname.startsWith(item.href + "/")
                    ? "bg-primary/10 text-primary"
                    : "text-muted-foreground"
                )}
              >
                <span>{t(item.labelKey)}</span>
                {item.isAdmin ? (
                  <Badge
                    variant="secondary"
                    className="px-1.5 py-0 text-[10px] font-semibold uppercase tracking-wide"
                  >
                    {t("nav.adminBadge")}
                  </Badge>
                ) : null}
              </Link>
            ))}
          </nav>
        </div>
      </header>

      {/* Main Content */}
      <main className="container mx-auto px-4 py-6">
        <Outlet />
      </main>
    </div>
  );
}
