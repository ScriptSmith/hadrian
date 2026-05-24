/**
 * Local types for the Containers API.
 *
 * The generated client (`ui/src/api/generated`) types the list/get/file
 * response bodies as `unknown` (the backend `#[utoipa::path]` annotations
 * don't carry a body schema for these endpoints yet). These mirror the
 * wire shapes serialized by `src/routes/api/containers.rs` — `WireContainer`,
 * `WireContainerFile`, and `WireList<T>` — so the page code is typed.
 *
 * All timestamps are Unix **seconds** (multiply by 1000 for `Date`).
 */

export type ContainerStatus = "active" | "expired" | "deleted";

export interface ContainerExpiresAfter {
  anchor: string;
  minutes: number;
}

export interface Container {
  id: string;
  object: "container";
  status: ContainerStatus;
  created_at: number;
  last_active_at: number;
  /** Forward-looking expiry estimate for active rows; transition time otherwise. */
  expires_at: number;
  idle_ttl_secs: number;
  runtime: string;
  name?: string;
  memory_limit?: string;
  memory_limit_mb?: number;
  expires_after?: ContainerExpiresAfter;
  network_policy?: unknown;
  skill_ids?: string[];
  source_response_id?: string;
}

export type ContainerFileSource = "user" | "assistant";

export interface ContainerFile {
  id: string;
  object: "container.file";
  container_id: string;
  /** Absolute path inside the container, always under `/mnt/data/`. */
  path: string;
  filename: string;
  bytes: number;
  source: ContainerFileSource;
  content_type?: string;
  created_at: number;
}

export interface ContainerList {
  object: "list";
  data: Container[];
  has_more: boolean;
  first_id?: string;
  last_id?: string;
}

export interface ContainerFileList {
  object: "list";
  data: ContainerFile[];
  has_more: boolean;
  first_id?: string;
  last_id?: string;
}
