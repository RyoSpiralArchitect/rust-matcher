import { Outlet, Link, useLocation } from "react-router-dom";
import { cn } from "@/lib/utils";
import { useFlags } from "@/lib/auth";

const navItems = [
  { href: "/queue", label: "Dashboard", requiresQueueAdmin: true },
  { href: "/jobs", label: "Jobs", requiresQueueAdmin: true },
  { href: "/projects", label: "Projects", requiresQueueAdmin: false },
];

export function RootLayout() {
  const location = useLocation();
  const { isQueueAdmin } = useFlags();

  const visibleNavItems = navItems.filter(
    (item) => !item.requiresQueueAdmin || isQueueAdmin,
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
            {visibleNavItems.map((item) => (
              <Link
                key={item.href}
                to={item.href}
                className={cn(
                  "text-sm font-medium transition-colors hover:text-primary",
                  location.pathname === item.href ||
                    location.pathname.startsWith(item.href + "/")
                    ? "text-primary"
                    : "text-muted-foreground"
                )}
              >
                {item.label}
                {item.requiresQueueAdmin && isQueueAdmin && (
                  <span className="ml-2 rounded-full bg-muted px-2 py-0.5 text-[10px] font-semibold text-muted-foreground">
                    Ops
                  </span>
                )}
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
