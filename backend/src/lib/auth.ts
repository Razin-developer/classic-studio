import bcrypt from "bcryptjs";
import { SignJWT, jwtVerify } from "jose";
import { AppError } from "./errors.js";
import type { AuthUser, Role } from "../types.js";

const encoder = new TextEncoder();

function secret(value?: string) {
  if (!value || value.length < 32) {
    throw new AppError(503, "Authentication is not configured.", "AUTH_NOT_CONFIGURED");
  }
  return encoder.encode(value);
}

export async function hashPassword(password: string) {
  return bcrypt.hash(password, 12);
}

export async function verifyPassword(password: string, hash: string) {
  return bcrypt.compare(password, hash);
}

export async function issueToken(user: AuthUser, jwtSecret?: string) {
  return new SignJWT({ email: user.email, name: user.displayName, role: user.role })
    .setProtectedHeader({ alg: "HS256" })
    .setSubject(user.id)
    .setIssuedAt()
    .setExpirationTime("12h")
    .sign(secret(jwtSecret));
}

export async function authenticate(header: string | undefined, jwtSecret?: string): Promise<AuthUser> {
  const token = header?.startsWith("Bearer ") ? header.slice(7) : undefined;
  if (!token) throw new AppError(401, "A bearer token is required.", "UNAUTHENTICATED");
  try {
    const { payload } = await jwtVerify(token, secret(jwtSecret));
    const role = payload.role;
    if (typeof payload.sub !== "string" || typeof payload.email !== "string" || typeof payload.name !== "string" ||
      !["owner", "editor", "viewer", "admin"].includes(String(role))) {
      throw new Error("Invalid token payload");
    }
    return { id: payload.sub, email: payload.email, displayName: payload.name, role: role as Role };
  } catch {
    throw new AppError(401, "The access token is invalid or expired.", "UNAUTHENTICATED");
  }
}
