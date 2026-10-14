export type AppEnv = {
  MONGODB_URI?: string;
  MONGODB_DB?: string;
  JWT_SECRET?: string;
  ADMIN_EMAIL?: string;
  CORS_ORIGINS?: string;
  APP_ENV?: string;
  API_PROXY_URL?: string;
};

export type Role = "owner" | "editor" | "viewer" | "admin";

export type AuthUser = {
  id: string;
  email: string;
  displayName: string;
  role: Role;
};

export type Project = {
  id: string;
  name: string;
  ownerId: string;
  members: Array<{ userId: string; role: Role }>;
  createdAt: string;
  updatedAt: string;
};

export type ProjectFile = {
  id: string;
  projectId: string;
  name: string;
  extension: ".classic";
  sizeBytes: number;
  storageKey: string;
  createdAt: string;
  updatedAt: string;
};

export type Presence = {
  roomId: string;
  userId: string;
  displayName: string;
  cursor?: { x: number; y: number };
  updatedAt: string;
};
