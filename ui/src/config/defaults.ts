import type { UiConfig } from "./types";

export const defaultConfig: UiConfig = {
  branding: {
    title: "Hadrian Gateway",
    tagline: null,
    logo_url: null,
    logo_dark_url: null,
    favicon_url: null,
    colors: {},
    colors_dark: null,
    fonts: null,
    footer_text: null,
    footer_links: [],
    show_version: false,
    version: null,
    login: null,
  },
  chat: {
    enabled: true,
    default_model: null,
    available_models: [],
    file_uploads_enabled: true,
    max_file_size_bytes: 10 * 1024 * 1024, // 10MB
    allowed_file_types: [], // Empty = allow all filetypes
  },
  admin: {
    enabled: true,
  },
  auth: {
    methods: ["none"], // Default to no auth for easy development
    oidc: null,
  },
};

export function getApiBaseUrl(): string {
  // In development, Vite proxy handles this
  // In production, use the same origin or env variable
  if (import.meta.env.VITE_API_URL) {
    return import.meta.env.VITE_API_URL;
  }
  return window.location.origin;
}
