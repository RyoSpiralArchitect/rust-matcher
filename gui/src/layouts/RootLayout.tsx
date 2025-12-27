import { Link, Outlet, useLocation } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { useFlags } from "@/lib/auth";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import type { TranslationKey } from "@/lib/messages";

const navItems: { href: string; labelKey: TranslationKey; adminOnly?: boolean }[] = [
  { href: "/queue", labelKey: "navigation.dashboard", adminOnly: true },
  { href: "/jobs", labelKey: "navigation.jobs", adminOnly: true },
  { href: "/projects", labelKey: "navigation.projects" },
  { href: "/talents", labelKey: "navigation.talents" },
];

export function RootLayout() {
  const { t } = useI18n();
  const { isQueueAdmin } = useFlags();
  const location = useLocation();
  const visibleNavItems = navItems;

  const isActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(`${href}/`);

  const visibleNavItems = navItems.filter(
    (item) => !item.adminOnly || isQueueAdmin,
  );

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
                <span className="inline-flex items-center gap-2">
                  {t(item.labelKey)}
                  {item.adminOnly && (
                    <Badge variant="outline" className="text-[10px] font-semibold">
                      Admin
                    </Badge>
                  )}
                </span>
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
