import { Outlet, Link, useLocation } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";

const navItems = [
  { href: "/queue", labelKey: "nav.queue" },
  { href: "/jobs", labelKey: "nav.jobs" },
  { href: "/projects", labelKey: "nav.projects" },
  { href: "/talents", labelKey: "nav.talents" },
] as const;

export function RootLayout() {
  const { t } = useI18n();
  const location = useLocation();
  const { t } = useI18n();

  const isActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(`${href}/`);

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
                  "text-sm font-medium transition-colors hover:text-primary",
                  isActive(item.href) ? "text-primary" : "text-muted-foreground",
                )}
              >
                {t(item.labelKey)}
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
