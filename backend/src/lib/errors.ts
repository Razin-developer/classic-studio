import type { Context } from "hono";

export class AppError extends Error {
  constructor(public status: number, message: string, public code = "REQUEST_FAILED") {
    super(message);
  }
}

export function errorResponse(error: unknown, c: Context) {
  if (error instanceof AppError) {
    return c.json({ error: { code: error.code, message: error.message } }, error.status as 400);
  }
  console.error("Unhandled API error", error);
  return c.json({ error: { code: "INTERNAL_ERROR", message: "An unexpected error occurred." } }, 500);
}
