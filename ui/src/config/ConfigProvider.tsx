import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import type { UiConfig, ColorPalette, FontsConfig, CustomFont } from "./types";
import { defaultConfig, getApiBaseUrl } from "./defaults";

interface ConfigContextValue {
  config: UiConfig;
  isLoading: boolean;
  error: Error | null;
  apiBaseUrl: string;
}

const ConfigContext = createContext<ConfigContextValue | null>(null);

const BRANDING_STYLE_ID = "hadrian-branding-colors";
const BRANDING_FONTS_STYLE_ID = "hadrian-branding-fonts";

/**
 * Generates CSS variable overrides from a color palette
 */
function generateColorCss(colors: ColorPalette, selector: string): string {
  const rules: string[] = [];

  if (colors.primary) {
    rules.push(`--color-primary: ${colors.primary};`);
    rules.push(`--color-ring: ${colors.primary};`);
    // Set accent-foreground to primary color for consistent branding on selected items
    rules.push(`--color-accent-foreground: ${colors.primary};`);
  }
  if (colors.primary_foreground) {
    rules.push(`--color-primary-foreground: ${colors.primary_foreground};`);
  } else if (colors.primary) {
    // Default to white if primary is set but primary_foreground is not
    rules.push(`--color-primary-foreground: #ffffff;`);
  }
  if (colors.secondary) {
    rules.push(`--color-secondary: ${colors.secondary};`);
  }
  if (colors.secondary_foreground) {
    rules.push(`--color-secondary-foreground: ${colors.secondary_foreground};`);
  }
  if (colors.accent) {
    rules.push(`--color-accent: ${colors.accent};`);
  }
  if (colors.background) {
    rules.push(`--color-background: ${colors.background};`);
  }
  if (colors.foreground) {
    rules.push(`--color-foreground: ${colors.foreground};`);
  }
  if (colors.muted) {
    rules.push(`--color-muted: ${colors.muted};`);
  }
  if (colors.border) {
    rules.push(`--color-border: ${colors.border};`);
    rules.push(`--color-input: ${colors.border};`);
  }

  if (rules.length === 0) return "";
  return `${selector} { ${rules.join(" ")} }`;
}

/**
 * Injects branding colors as CSS custom properties
 */
function injectBrandingColors(colors: ColorPalette, colorsDark: ColorPalette | null): void {
  // Remove existing branding style if present
  const existing = document.getElementById(BRANDING_STYLE_ID);
  if (existing) {
    existing.remove();
  }

  const lightCss = generateColorCss(colors, ":root");
  const darkCss = colorsDark ? generateColorCss(colorsDark, ".dark") : "";

  const css = [lightCss, darkCss].filter(Boolean).join("\n");
  if (!css) return;

  const style = document.createElement("style");
  style.id = BRANDING_STYLE_ID;
  style.textContent = css;
  document.head.appendChild(style);
}

/**
 * Generates @font-face rules for custom fonts
 */
function generateFontFaceRules(customFonts: CustomFont[]): string {
  return customFonts
    .map(
      (font) => `@font-face {
  font-family: "${font.name}";
  src: url("${font.url}");
  font-weight: ${font.weight};
  font-style: ${font.style};
  font-display: swap;
}`
    )
    .join("\n\n");
}

/**
 * Generates CSS variable overrides for font families
 */
function generateFontCss(fonts: FontsConfig): string {
  const rules: string[] = [];

  // Build font stacks with fallbacks
  const sansStack =
    'ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';
  const monoStack =
    'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Monaco, Consolas, "Liberation Mono", monospace';

  if (fonts.body) {
    rules.push(`--font-sans: "${fonts.body}", ${sansStack};`);
  }
  if (fonts.heading) {
    rules.push(`--font-heading: "${fonts.heading}", ${sansStack};`);
  }
  if (fonts.mono) {
    rules.push(`--font-mono: "${fonts.mono}", ${monoStack};`);
  }

  if (rules.length === 0) return "";
  return `:root { ${rules.join(" ")} }`;
}

/**
 * Injects branding fonts as @font-face rules and CSS custom properties
 */
function injectBrandingFonts(fonts: FontsConfig | null): void {
  // Remove existing font style if present
  const existing = document.getElementById(BRANDING_FONTS_STYLE_ID);
  if (existing) {
    existing.remove();
  }

  if (!fonts) return;

  const fontFaceRules = fonts.custom ? generateFontFaceRules(fonts.custom) : "";
  const fontVariables = generateFontCss(fonts);

  const css = [fontFaceRules, fontVariables].filter(Boolean).join("\n\n");
  if (!css) return;

  const style = document.createElement("style");
  style.id = BRANDING_FONTS_STYLE_ID;
  style.textContent = css;
  document.head.appendChild(style);
}

interface ConfigProviderProps {
  children: ReactNode;
}

export function ConfigProvider({ children }: ConfigProviderProps) {
  const [config, setConfig] = useState<UiConfig>(defaultConfig);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const apiBaseUrl = getApiBaseUrl();

  useEffect(() => {
    async function fetchConfig() {
      try {
        const response = await fetch(`${apiBaseUrl}/admin/v1/ui/config`);
        if (response.ok) {
          const data = (await response.json()) as UiConfig;
          setConfig(data);
        } else {
          // Use defaults if endpoint is not available
          console.warn("UI config endpoint not available, using defaults");
        }
      } catch (err) {
        console.warn("Failed to fetch UI config, using defaults:", err);
        setError(err instanceof Error ? err : new Error("Failed to fetch config"));
      } finally {
        setIsLoading(false);
      }
    }

    fetchConfig();
  }, [apiBaseUrl]);

  // Update document title, favicon, colors, and fonts based on config
  useEffect(() => {
    document.title = config.branding.title;
    if (config.branding.favicon_url) {
      const favicon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
      if (favicon) {
        favicon.href = config.branding.favicon_url;
      }
    }
    // Inject branding colors as CSS custom properties
    injectBrandingColors(config.branding.colors, config.branding.colors_dark);
    // Inject branding fonts as @font-face rules and CSS custom properties
    injectBrandingFonts(config.branding.fonts);
  }, [config.branding]);

  return (
    <ConfigContext.Provider value={{ config, isLoading, error, apiBaseUrl }}>
      {children}
    </ConfigContext.Provider>
  );
}

export function useConfig(): ConfigContextValue {
  const context = useContext(ConfigContext);
  if (!context) {
    throw new Error("useConfig must be used within a ConfigProvider");
  }
  return context;
}
