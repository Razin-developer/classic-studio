import { AppError } from "./errors.js";

const CONTROL_CHARS = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g;

/** Plain text only: escape markup before it is persisted or rebroadcast to collaborators. */
export function sanitizeText(value: string, maxLength = 4_000) {
  return value
    .replace(CONTROL_CHARS, "")
    .replace(/[<>]/g, "")
    .trim()
    .slice(0, maxLength);
}

export function parseCorsOrigins(value?: string) {
  return (value ?? "").split(",").map((origin) => origin.trim()).filter(Boolean);
}

type RateLimitEntry = { count: number; resetAt: number };

/** Replace with Durable Objects/Redis in a multi-process production deployment. */
export class InMemoryRateLimiter {
  private readonly entries = new Map<string, RateLimitEntry>();

  consume(key: string, limit: number, windowMs: number) {
    const now = Date.now();
    const current = this.entries.get(key);
    const entry = !current || current.resetAt <= now
      ? { count: 0, resetAt: now + windowMs }
      : current;
    entry.count += 1;
    this.entries.set(key, entry);
    if (entry.count > limit) {
      throw new AppError(429, "Too many requests. Try again shortly.", "RATE_LIMITED");
    }
    return { remaining: Math.max(0, limit - entry.count), resetAt: entry.resetAt };
  }
}
