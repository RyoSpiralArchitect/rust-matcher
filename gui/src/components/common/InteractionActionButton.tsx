import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { componentTheme, type InteractionAction } from "@/theme/component-theme";
import { cn } from "@/lib/utils";

type InteractionActionButtonProps = {
  action: InteractionAction;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  isBusy?: boolean;
};

export function InteractionActionButton({
  action,
  label,
  onClick,
  disabled = false,
  isBusy = false,
}: InteractionActionButtonProps) {
  const token = componentTheme.actions[action];
  const Icon = token.icon;

  return (
    <Button
      variant={token.variant}
      size="sm"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={cn(token.className)}
    >
      {isBusy ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Icon className="mr-2 h-4 w-4" />}
      {label}
    </Button>
  );
}
