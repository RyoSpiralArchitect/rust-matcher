import { CalendarClock, MessageCircle, ThumbsDown, ThumbsUp, type LucideIcon } from "lucide-react";

type ButtonVariant = "default" | "destructive" | "outline" | "secondary" | "ghost" | "link";
type BadgeVariant = "default" | "secondary" | "destructive" | "outline";

export type InteractionState =
  | "pending"
  | "proposed"
  | "rejected"
  | "interviewing"
  | "accepted"
  | "in_project"
  | "no_response";

export type InteractionAction = "propose" | "reject" | "interview" | "details";

type StateToken = {
  badgeVariant: BadgeVariant;
  badgeClassName?: string;
  icon?: LucideIcon;
};

type ActionToken = {
  variant: ButtonVariant;
  icon: LucideIcon;
  className?: string;
};

const stateColors = {
  propose:
    "bg-[var(--state-proposed-bg)] text-[var(--state-proposed-fg)] border-[var(--state-proposed-border)]",
  reject:
    "bg-[var(--state-rejected-bg)] text-[var(--state-rejected-fg)] border-[var(--state-rejected-border)]",
  interview:
    "bg-[var(--state-interview-bg)] text-[var(--state-interview-fg)] border-[var(--state-interview-border)]",
  neutral:
    "bg-[var(--state-neutral-bg)] text-[var(--state-neutral-fg)] border-[var(--state-neutral-border)]",
} satisfies Record<string, string>;

export const componentTheme = {
  layout: {
    cardHeader: "flex flex-col gap-3 md:flex-row md:items-start md:justify-between",
    badgeRow: "flex flex-wrap items-center gap-2",
    actionRow: "flex flex-wrap items-center gap-2",
    metaColumn: "space-y-1 text-sm text-muted-foreground",
  },
  badges: {
    state: "gap-1",
    metric: "gap-1",
  },
  states: {
    pending: { badgeVariant: "outline", badgeClassName: stateColors.neutral },
    proposed: { badgeVariant: "default", badgeClassName: stateColors.propose, icon: ThumbsUp },
    rejected: { badgeVariant: "secondary", badgeClassName: stateColors.reject, icon: ThumbsDown },
    interviewing: { badgeVariant: "default", badgeClassName: stateColors.interview, icon: CalendarClock },
    accepted: { badgeVariant: "default", badgeClassName: stateColors.propose, icon: ThumbsUp },
    in_project: { badgeVariant: "secondary", badgeClassName: stateColors.propose, icon: ThumbsUp },
    no_response: { badgeVariant: "outline", badgeClassName: stateColors.neutral },
  } satisfies Record<InteractionState, StateToken>,
  actions: {
    propose: {
      variant: "outline",
      icon: ThumbsUp,
      className: `border-[var(--state-proposed-border)] text-[var(--state-proposed-bg)]`,
    },
    reject: {
      variant: "outline",
      icon: ThumbsDown,
      className: `border-[var(--state-rejected-border)] text-[var(--state-rejected-bg)]`,
    },
    interview: {
      variant: "secondary",
      icon: CalendarClock,
      className: stateColors.interview,
    },
    details: { variant: "ghost", icon: MessageCircle },
  } satisfies Record<InteractionAction, ActionToken>,
  spacing: {
    cardStack: "space-y-4",
    badgeStack: "space-y-2",
  },
};
