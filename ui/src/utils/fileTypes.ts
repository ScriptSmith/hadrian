/**
 * Utilities for detecting and validating file types.
 * Handles the common problem of browsers assigning empty or generic MIME types to code/text files.
 */

// Common text-based file extensions that browsers often fail to assign proper MIME types
export const TEXT_FILE_EXTENSIONS = new Set([
  // Plain text
  "txt",
  "md",
  "markdown",
  "rst",
  "log",
  // Code files
  "js",
  "mjs",
  "cjs",
  "ts",
  "mts",
  "cts",
  "jsx",
  "tsx",
  "py",
  "pyw",
  "rs",
  "go",
  "java",
  "c",
  "cpp",
  "cc",
  "cxx",
  "h",
  "hpp",
  "hxx",
  "rb",
  "php",
  "swift",
  "kt",
  "kts",
  "scala",
  "cs",
  "fs",
  "vb",
  "lua",
  "r",
  "pl",
  "pm",
  "sh",
  "bash",
  "zsh",
  "fish",
  "ps1",
  "bat",
  "cmd",
  // Config/data
  "json",
  "yaml",
  "yml",
  "toml",
  "ini",
  "cfg",
  "conf",
  "env",
  "properties",
  // Web
  "html",
  "htm",
  "css",
  "scss",
  "sass",
  "less",
  "svg",
  "xml",
  "xsl",
  "xslt",
  // Other
  "sql",
  "graphql",
  "gql",
  "proto",
  "dockerfile",
  "makefile",
  "cmake",
  "gradle",
]);

/**
 * Check if a file should be considered a text file based on its extension.
 * Browsers often return empty or generic MIME types for code/text files.
 */
export function isTextFileByExtension(filename: string): boolean {
  const ext = filename.split(".").pop()?.toLowerCase();
  if (!ext) return false;
  return TEXT_FILE_EXTENSIONS.has(ext);
}

/**
 * Check if a file type is allowed, with fallback to extension-based detection.
 * Handles the case where browsers return empty MIME types for text files.
 *
 * Special handling: If "text/plain" or "text/*" is in allowedTypes, we treat it as
 * allowing all text-based files (text/markdown, text/x-python, application/javascript, etc.)
 * since users configuring these typically want to allow code/text files.
 */
export function isFileTypeAllowed(file: File, allowedTypes: string[]): boolean {
  // If no restrictions, allow all
  if (allowedTypes.length === 0) return true;

  // Check by exact MIME type match or wildcard
  const mimeAllowed = allowedTypes.some(
    (type) =>
      file.type === type ||
      file.type.startsWith(type.replace("*", "")) ||
      file.name.endsWith(type.replace("*", ""))
  );
  if (mimeAllowed) return true;

  // Check if any text type is allowed (text/plain, text/*, etc.)
  const textAllowed = allowedTypes.some((type) => type === "text/*" || type.startsWith("text/"));

  if (textAllowed) {
    // Allow any text/* MIME type
    if (file.type.startsWith("text/")) return true;

    // Allow JavaScript/TypeScript (browsers report as application/*)
    if (
      file.type === "application/javascript" ||
      file.type === "application/x-javascript" ||
      file.type === "application/typescript"
    )
      return true;

    // Allow files with empty/generic MIME types if they have known text extensions
    if (!file.type || file.type === "application/octet-stream") {
      if (isTextFileByExtension(file.name)) return true;
    }

    // Allow by extension for known text/code files regardless of MIME type
    if (isTextFileByExtension(file.name)) return true;
  }

  return false;
}

/**
 * Build an accept attribute string for file inputs.
 * When text/* types are allowed, extends the attribute with common text file extensions
 * since browsers don't recognize many code files as text/*.
 */
export function buildAcceptAttribute(allowedTypes: string[]): string | undefined {
  if (allowedTypes.length === 0) return undefined;

  const hasTextWildcard = allowedTypes.some(
    (type) => type === "text/*" || type.startsWith("text/")
  );

  if (hasTextWildcard) {
    // Add common text file extensions that browsers don't recognize as text/*
    const textExtensions = Array.from(TEXT_FILE_EXTENSIONS).map((ext) => `.${ext}`);
    return [...allowedTypes, ...textExtensions].join(",");
  }

  return allowedTypes.join(",");
}
