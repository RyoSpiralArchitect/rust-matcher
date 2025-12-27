/* eslint-disable react-refresh/only-export-components */
import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import { MESSAGES, type Locale, type TranslationKey } from "./messages";

interface I18nContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, values?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function normalizeLocale(input?: string | null): Locale {
  if (!input) return "en";
  const normalized = input.toLowerCase();
  if (normalized.startsWith("ja")) return "ja";
  return "en";
}

function formatMessage(template: string, values?: Record<string, string | number>) {
  if (!values) return template;
  return Object.entries(values).reduce(
    (result, [key, value]) => result.replaceAll(`{${key}}`, String(value)),
    template,
  );
}

export function I18nProvider({ children, locale }: { children: ReactNode; locale?: string }) {
  const initialLocale =
    typeof navigator !== "undefined" && !locale ? normalizeLocale(navigator.language) : normalizeLocale(locale);
  const [currentLocale, setCurrentLocale] = useState<Locale>(initialLocale);

  const value = useMemo<I18nContextValue>(() => {
    const t = (key: TranslationKey, values?: Record<string, string | number>) => {
      const message =
        MESSAGES[currentLocale]?.[key] ??
        MESSAGES.en[key] ??
        `[missing translation: ${key}]`;
      return formatMessage(message, values);
    };

    return {
      locale: currentLocale,
      setLocale: setCurrentLocale,
      t,
    };
  }, [currentLocale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return ctx;
}
