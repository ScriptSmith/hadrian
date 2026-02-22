import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import jsxA11y from "eslint-plugin-jsx-a11y";
import tseslint from "typescript-eslint";
import prettier from "eslint-plugin-prettier";
import prettierConfig from "eslint-config-prettier";
import { defineConfig, globalIgnores } from "eslint/config";
import hadrianPlugin from "./eslint-plugins/require-story.js";

export default defineConfig([
  globalIgnores(["dist", "src/api/generated", "storybook-static"]),
  {
    files: ["src/**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
      jsxA11y.flatConfigs.recommended,
      prettierConfig,
    ],
    plugins: {
      prettier,
      hadrian: hadrianPlugin,
    },
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    rules: {
      "prettier/prettier": "error",
      // autoFocus is used intentionally for inline-edit inputs and search fields
      "jsx-a11y/no-autofocus": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // Allow exporting hooks and helper functions alongside components
      "react-refresh/only-export-components": "off",
      // Disable overly strict React Compiler rules
      "react-hooks/set-state-in-effect": "off",
      "react-hooks/refs": "off",
      "react-hooks/immutability": "off",
      "react-hooks/incompatible-library": "off",
      "react-hooks/purity": "off",
      // Enforce correct React Query invalidation pattern for hey-api generated queries
      "hadrian/no-string-query-key": "error",
    },
  },
  // Require Storybook stories for components
  {
    files: ["src/components/**/*.tsx"],
    ignores: [
      "src/components/**/index.tsx",
      "src/components/**/*.stories.tsx",
      "src/components/**/*.test.tsx",
      // Context providers are not visual components
      "src/components/**/*Provider.tsx",
    ],
    rules: {
      "hadrian/require-story": [
        "warn",
        {
          componentPaths: ["src/components"],
          ignore: [],
        },
      ],
    },
  },
]);
