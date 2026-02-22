import fs from "fs";
import path from "path";

/**
 * Custom ESLint rule that requires React components to have Storybook stories.
 *
 * This rule checks that:
 * 1. Component files in src/components/ have a corresponding .stories.tsx file
 * 2. Component files in src/pages/admin/ have a corresponding .stories.tsx file (optional)
 *
 * Files that are excluded:
 * - index.ts/tsx files (barrel exports)
 * - Files already ending in .stories.tsx
 * - Type definition files (.d.ts)
 * - Test files (.test.tsx, .spec.tsx)
 * - Non-component files (hooks, utils, types, etc.)
 */

const rule = {
  meta: {
    type: "suggestion",
    docs: {
      description: "Require React components to have Storybook stories",
      category: "Best Practices",
      recommended: false,
    },
    schema: [
      {
        type: "object",
        properties: {
          componentPaths: {
            type: "array",
            items: { type: "string" },
            description: "Glob patterns for component directories to check",
          },
          ignore: {
            type: "array",
            items: { type: "string" },
            description: "Patterns for files to ignore",
          },
        },
        additionalProperties: false,
      },
    ],
    messages: {
      missingStory:
        "Component '{{componentName}}' is missing a Storybook story. Create {{storyPath}}",
    },
  },

  create(context) {
    const options = context.options[0] || {};
    const componentPaths = options.componentPaths || ["src/components"];
    const ignorePatterns = options.ignore || [];

    const filename = context.filename || context.getFilename();
    const normalizedPath = filename.replace(/\\/g, "/");

    // Check if file is in a component directory
    const isInComponentDir = componentPaths.some((dir) => normalizedPath.includes(`/${dir}/`));

    if (!isInComponentDir) {
      return {};
    }

    // Skip files that shouldn't have stories
    const basename = path.basename(filename);

    // Skip index files (barrel exports)
    if (basename === "index.ts" || basename === "index.tsx") {
      return {};
    }

    // Skip story files themselves
    if (basename.endsWith(".stories.tsx") || basename.endsWith(".stories.ts")) {
      return {};
    }

    // Skip test files
    if (
      basename.endsWith(".test.tsx") ||
      basename.endsWith(".test.ts") ||
      basename.endsWith(".spec.tsx") ||
      basename.endsWith(".spec.ts")
    ) {
      return {};
    }

    // Skip type definition files
    if (basename.endsWith(".d.ts")) {
      return {};
    }

    // Skip non-TSX files (likely utilities, hooks, types)
    if (!basename.endsWith(".tsx")) {
      return {};
    }

    // Check ignore patterns
    if (ignorePatterns.some((pattern) => normalizedPath.includes(pattern))) {
      return {};
    }

    // Construct expected story file path
    const storyPath = filename.replace(/\.tsx$/, ".stories.tsx");

    // Check if story file exists
    const storyExists = fs.existsSync(storyPath);

    if (storyExists) {
      return {};
    }

    // Get component name from filename
    const componentName = basename.replace(/\.tsx$/, "");

    // Calculate relative story path for error message
    const cwd = context.cwd || process.cwd();
    const relativeStoryPath = path.relative(cwd, storyPath).replace(/\\/g, "/");

    return {
      Program(node) {
        // Only report if the file exports a component (has JSX)
        // We check at the Program level to report once per file
        const sourceCode = context.sourceCode || context.getSourceCode();
        const text = sourceCode.getText();

        // Simple heuristic: if the file contains JSX syntax, it's likely a component
        // Look for JSX elements or fragments
        const hasJSX = /<[A-Z][A-Za-z]*|<>|<\/>/g.test(text);

        // Also check for React.createElement calls
        const hasCreateElement = /React\.createElement|createElement\(/g.test(text);

        if (hasJSX || hasCreateElement) {
          context.report({
            node,
            messageId: "missingStory",
            data: {
              componentName,
              storyPath: relativeStoryPath,
            },
          });
        }
      },
    };
  },
};

/**
 * Custom ESLint rule that enforces correct React Query invalidation patterns.
 *
 * The @hey-api/openapi-ts generated query keys use an object structure:
 *   [{ _id: "queryName", baseUrl: "...", path: { ... } }]
 *
 * Using simple string arrays like ["queryName"] won't match due to type mismatch.
 * This rule catches the broken pattern and suggests the correct fix.
 */
const invalidateQueriesRule = {
  meta: {
    type: "problem",
    docs: {
      description: "Enforce correct React Query invalidation pattern for hey-api generated queries",
      category: "Possible Errors",
      recommended: true,
    },
    fixable: "code",
    schema: [],
    messages: {
      incorrectQueryKey:
        'Query key ["{{queryName}}"] will not match hey-api generated keys. Use [{ _id: "{{queryName}}" }] instead.',
    },
  },

  create(context) {
    return {
      CallExpression(node) {
        // Check if this is a call to invalidateQueries
        if (
          node.callee.type !== "MemberExpression" ||
          node.callee.property.name !== "invalidateQueries"
        ) {
          return;
        }

        // Get the first argument (options object)
        const optionsArg = node.arguments[0];
        if (!optionsArg || optionsArg.type !== "ObjectExpression") {
          return;
        }

        // Find the queryKey property
        const queryKeyProp = optionsArg.properties.find(
          (prop) => prop.type === "Property" && prop.key.name === "queryKey"
        );

        if (!queryKeyProp || queryKeyProp.value.type !== "ArrayExpression") {
          return;
        }

        const arrayElements = queryKeyProp.value.elements;

        // Check if first element is a string literal (the broken pattern)
        if (
          arrayElements.length >= 1 &&
          arrayElements[0] &&
          arrayElements[0].type === "Literal" &&
          typeof arrayElements[0].value === "string"
        ) {
          const queryName = arrayElements[0].value;

          context.report({
            node: queryKeyProp.value,
            messageId: "incorrectQueryKey",
            data: { queryName },
            fix(fixer) {
              return fixer.replaceText(queryKeyProp.value, `[{ _id: "${queryName}" }]`);
            },
          });
        }
      },
    };
  },
};

const plugin = {
  meta: {
    name: "eslint-plugin-hadrian",
    version: "1.0.0",
  },
  rules: {
    "require-story": rule,
    "no-string-query-key": invalidateQueriesRule,
  },
};

export default plugin;
