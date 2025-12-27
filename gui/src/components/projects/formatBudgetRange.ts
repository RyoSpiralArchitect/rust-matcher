import type { TranslationKey } from "@/lib/messages";

type Translator = (key: TranslationKey, values?: Record<string, string | number>) => string;

export function formatBudgetRange(
  min: number | null,
  max: number | null,
  locale: string,
  format: Translator,
) {
  if (min == null && max == null) {
    return format("projects.budget.unknown");
  }

  const formatter = new Intl.NumberFormat(locale === "ja" ? "ja-JP" : "en-US", {
    style: "currency",
    currency: "JPY",
    maximumFractionDigits: 0,
  });

  if (min != null && max != null) {
    return format("projects.budget.range", {
      min: formatter.format(min),
      max: formatter.format(max),
    });
  }

  if (min != null) {
    return format("projects.budget.min", { min: formatter.format(min) });
  }

  if (max != null) {
    return format("projects.budget.max", { max: formatter.format(max) });
  }

  return format("projects.budget.unknown");
}
