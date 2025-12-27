import { useMemo } from "react";

type JwtPayload = {
  role?: string | string[];
  roles?: string | string[];
};

function readEnv(key: string): string {
  const raw = (import.meta.env as Record<string, string | undefined>)[key];
  return (raw ?? "").trim();
}

function decodeJwtPayload(token: string): JwtPayload | null {
  const parts = token.split(".");
  if (parts.length < 2) return null;

  try {
    const base64 = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const json = atob(base64);
    return JSON.parse(json) as JwtPayload;
  } catch (error) {
    console.warn("[auth] Failed to decode JWT payload", error);
    return null;
  }
}

function normalizeRoles(value: unknown): string[] {
  if (!value) return [];

  if (Array.isArray(value)) {
    return value
      .map((role) => (typeof role === "string" ? role.toLowerCase() : null))
      .filter((role): role is string => Boolean(role));
  }

  if (typeof value === "string") {
    return value
      .split(/[ ,]/)
      .map((role) => role.trim().toLowerCase())
      .filter(Boolean);
  }

  return [];
}

function deriveRolesFromEnv(): string[] {
  const token = readEnv("VITE_JWT_TOKEN");
  if (!token) return [];

  const payload = decodeJwtPayload(token);
  if (!payload) return [];

  const rolesFromRoles = normalizeRoles(payload.roles);
  const rolesFromRole = normalizeRoles(payload.role);
  return Array.from(new Set([...rolesFromRoles, ...rolesFromRole]));
}

export function useAuth() {
  return useMemo(() => {
    const roles = deriveRolesFromEnv();

    return {
      roles,
    };
  }, []);
}

export function useFlags() {
  const { roles } = useAuth();

  const isQueueAdminFlag = readEnv("VITE_ENABLE_QUEUE_ADMIN").toLowerCase() === "true";
  const hasQueueRole = roles.some((role) =>
    ["queue_admin", "admin", "ops", "operations"].includes(role)
  );

  const isQueueAdmin = isQueueAdminFlag || hasQueueRole;

  return {
    roles,
    isQueueAdmin,
  };
}
