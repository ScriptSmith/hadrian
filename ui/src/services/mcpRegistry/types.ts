/**
 * Type definitions for the official MCP Registry v0.1 API.
 *
 * Schema reference: https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json
 *
 * The registry is a community-run catalog of published MCP servers. Entries
 * describe how to connect — either over a remote HTTP transport, or by running
 * a local package that speaks stdio.
 */

export type MCPRegistryRemoteType = "streamable-http" | "sse";

/** A templated header value — may contain `{placeholder}` tokens the user must fill in. */
export interface MCPRegistryHeader {
  name: string;
  value?: string;
  description?: string;
  isRequired?: boolean;
  isSecret?: boolean;
}

export interface MCPRegistryRemote {
  type: MCPRegistryRemoteType;
  url: string;
  headers?: MCPRegistryHeader[];
}

export interface MCPRegistryEnvVar {
  name: string;
  description?: string;
  isRequired?: boolean;
  isSecret?: boolean;
  default?: string;
}

export interface MCPRegistryPackageArgument {
  /** "named" args render as `{name} {value}`; otherwise as just `{value}`. */
  type?: "named" | "positional" | string;
  name?: string;
  value?: string;
  description?: string;
  isRequired?: boolean;
}

export interface MCPRegistryPackageTransport {
  type: string;
  /** Present when the package exposes its own reachable URL (e.g. streamable-http). */
  url?: string;
}

export interface MCPRegistryPackage {
  registryType: string;
  identifier: string;
  version?: string;
  transport?: MCPRegistryPackageTransport;
  environmentVariables?: MCPRegistryEnvVar[];
  /** Arguments passed to the package itself (after the identifier). */
  packageArguments?: MCPRegistryPackageArgument[];
  /** Arguments passed to the runtime (e.g. `docker run <runtimeArgs> image <packageArgs>`). */
  runtimeArguments?: MCPRegistryPackageArgument[];
  runtimeHint?: string;
}

export interface MCPRegistryIcon {
  src: string;
  sizes?: string[];
  mimeType?: string;
}

export interface MCPRegistryRepository {
  url: string;
  source?: string;
  subfolder?: string;
}

export interface MCPRegistryServer {
  $schema?: string;
  name: string;
  title?: string;
  description?: string;
  version?: string;
  websiteUrl?: string;
  repository?: MCPRegistryRepository;
  icons?: MCPRegistryIcon[];
  remotes?: MCPRegistryRemote[];
  packages?: MCPRegistryPackage[];
}

export interface MCPRegistryMeta {
  status?: string;
  statusChangedAt?: string;
  publishedAt?: string;
  updatedAt?: string;
  isLatest?: boolean;
}

export interface MCPRegistryEntry {
  server: MCPRegistryServer;
  _meta?: {
    "io.modelcontextprotocol.registry/official"?: MCPRegistryMeta;
  };
}

export interface MCPRegistrySearchResponse {
  servers: MCPRegistryEntry[];
  metadata?: {
    nextCursor?: string;
    count?: number;
  };
}
