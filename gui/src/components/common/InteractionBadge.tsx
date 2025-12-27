import { Badge } from "@/components/ui/badge";
import { componentTheme, type InteractionState } from "@/theme/component-theme";
import { cn } from "@/lib/utils";

type InteractionBadgeProps = {
  state: InteractionState;
  label: string;
  className?: string;
};

export function InteractionBadge({ state, label, className }: InteractionBadgeProps) {
  const token = componentTheme.states[state] ?? componentTheme.states.pending;
  const Icon = token.icon;

  return (
    <Badge
      variant={token.badgeVariant}
      className={cn(componentTheme.badges.state, token.badgeClassName, className)}
    >
      {Icon ? <Icon className="size-3.5" aria-hidden /> : null}
      {label}
    </Badge>
  );
}
