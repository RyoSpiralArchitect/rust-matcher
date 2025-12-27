import { Link } from "react-router-dom";

type BreadcrumbItem = {
  label: string;
  href?: string;
  isCurrent?: boolean;
};

interface BreadcrumbsProps {
  items: BreadcrumbItem[];
}

export function Breadcrumbs({ items }: BreadcrumbsProps) {
  const lastIndex = items.length - 1;

  return (
    <nav aria-label="Breadcrumb" className="text-sm text-muted-foreground">
      <ol className="flex items-center gap-2">
        {items.map((item, index) => {
          const isLast = index === lastIndex || item.isCurrent;
          const content = !isLast && item.href ? (
            <Link to={item.href} className="hover:underline">
              {item.label}
            </Link>
          ) : (
            <span className={isLast ? "text-foreground" : undefined}>{item.label}</span>
          );

          return (
            <li key={`${item.label}-${index}`} className="flex items-center gap-2">
              {content}
              {!isLast && <span className="text-muted-foreground">/</span>}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
