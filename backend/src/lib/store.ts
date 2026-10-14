import { randomUUID } from "node:crypto";
import { MongoClient, ObjectId } from "mongodb";
import { AppError } from "./errors.js";
import type { AuthUser, Presence, Project, ProjectFile, Role } from "../types.js";

export type StoredUser = AuthUser & { passwordHash: string };

export interface DataStore {
  findUserByEmail(email: string): Promise<StoredUser | undefined>;
  createUser(input: Omit<StoredUser, "id">): Promise<StoredUser>;
  createProject(owner: AuthUser, name: string): Promise<Project>;
  listProjects(user: AuthUser): Promise<Project[]>;
  findProject(id: string): Promise<Project | undefined>;
  saveFile(user: AuthUser, input: Omit<ProjectFile, "id" | "createdAt" | "updatedAt">): Promise<ProjectFile>;
  listFiles(projectId: string): Promise<ProjectFile[]>;
  updatePresence(presence: Presence): Promise<void>;
  listPresence(roomId: string): Promise<Presence[]>;
  close(): Promise<void>;
}

export class MemoryStore implements DataStore {
  private users = new Map<string, StoredUser>();
  private projects = new Map<string, Project>();
  private files = new Map<string, ProjectFile>();
  private presence = new Map<string, Presence>();

  async findUserByEmail(email: string) { return [...this.users.values()].find((u) => u.email === email); }
  async createUser(input: Omit<StoredUser, "id">) {
    const user = { ...input, id: randomUUID() };
    this.users.set(user.id, user);
    return user;
  }
  async createProject(owner: AuthUser, name: string) {
    const now = new Date().toISOString();
    const project = { id: randomUUID(), name, ownerId: owner.id, members: [{ userId: owner.id, role: "owner" as Role }], createdAt: now, updatedAt: now };
    this.projects.set(project.id, project);
    return project;
  }
  async listProjects(user: AuthUser) { return [...this.projects.values()].filter((p) => p.members.some((m) => m.userId === user.id)); }
  async findProject(id: string) { return this.projects.get(id); }
  async saveFile(_user: AuthUser, input: Omit<ProjectFile, "id" | "createdAt" | "updatedAt">) {
    const now = new Date().toISOString();
    const file = { ...input, id: randomUUID(), createdAt: now, updatedAt: now };
    this.files.set(file.id, file);
    return file;
  }
  async listFiles(projectId: string) { return [...this.files.values()].filter((f) => f.projectId === projectId); }
  async updatePresence(presence: Presence) { this.presence.set(`${presence.roomId}:${presence.userId}`, presence); }
  async listPresence(roomId: string) {
    const cutoff = Date.now() - 45_000;
    return [...this.presence.values()].filter((p) => p.roomId === roomId && Date.parse(p.updatedAt) >= cutoff);
  }
  async close() {}
}

/** Node-only MongoDB Atlas store. Cloudflare Workers must call this over HTTPS. */
export class MongoStore implements DataStore {
  private constructor(private readonly client: MongoClient, private readonly dbName: string) {}
  static async connect(uri: string, dbName = "classic") {
    const client = new MongoClient(uri, { serverSelectionTimeoutMS: 10_000, maxPoolSize: 20 });
    await client.connect();
    const store = new MongoStore(client, dbName);
    await store.ensureIndexes();
    return store;
  }
  private get db() { return this.client.db(this.dbName); }
  private async ensureIndexes() {
    await Promise.all([
      this.db.collection("users").createIndex({ email: 1 }, { unique: true }),
      this.db.collection("projects").createIndex({ "members.userId": 1 }),
      this.db.collection("files").createIndex({ projectId: 1 }),
      this.db.collection("presence").createIndex({ updatedAt: 1 }, { expireAfterSeconds: 60 })
    ]);
  }
  async findUserByEmail(email: string) {
    const value = await this.db.collection<StoredUser>("users").findOne({ email });
    return value ? { ...value, id: value.id ?? value._id.toString() } : undefined;
  }
  async createUser(input: Omit<StoredUser, "id">) {
    const user: StoredUser = { ...input, id: randomUUID() };
    try { await this.db.collection<StoredUser>("users").insertOne(user); return user; }
    catch (error: unknown) {
      if ((error as { code?: number }).code === 11000) throw new AppError(409, "An account with that email already exists.", "EMAIL_EXISTS");
      throw error;
    }
  }
  async createProject(owner: AuthUser, name: string) {
    const now = new Date().toISOString();
    const project: Project = { id: randomUUID(), name, ownerId: owner.id, members: [{ userId: owner.id, role: "owner" }], createdAt: now, updatedAt: now };
    await this.db.collection<Project>("projects").insertOne(project);
    return project;
  }
  async listProjects(user: AuthUser) { return this.db.collection<Project>("projects").find({ "members.userId": user.id }).toArray(); }
  async findProject(id: string) { return (await this.db.collection<Project>("projects").findOne({ id })) ?? undefined; }
  async saveFile(_user: AuthUser, input: Omit<ProjectFile, "id" | "createdAt" | "updatedAt">) {
    const now = new Date().toISOString();
    const file: ProjectFile = { ...input, id: randomUUID(), createdAt: now, updatedAt: now };
    await this.db.collection<ProjectFile>("files").insertOne(file);
    return file;
  }
  async listFiles(projectId: string) { return this.db.collection<ProjectFile>("files").find({ projectId }).toArray(); }
  async updatePresence(presence: Presence) { await this.db.collection<Presence>("presence").updateOne({ roomId: presence.roomId, userId: presence.userId }, { $set: presence }, { upsert: true }); }
  async listPresence(roomId: string) { return this.db.collection<Presence>("presence").find({ roomId }).toArray(); }
  async close() { await this.client.close(); }
}

export function assertProjectRole(project: Project, user: AuthUser, allowed: Role[]) {
  const membership = project.members.find((member) => member.userId === user.id);
  if (!membership || !allowed.includes(membership.role)) {
    throw new AppError(403, "You do not have access to this project.", "FORBIDDEN");
  }
}
