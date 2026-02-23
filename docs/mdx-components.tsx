import defaultMdxComponents from "fumadocs-ui/mdx";
import type { MDXComponents } from "mdx/types";
import { APIPage } from "@/components/api-page";
import { StoryEmbed } from "@/components/story-embed";
import { Mermaid } from "@/components/mdx/mermaid";
import { QuickStartSelector } from "@/components/quick-start-selector";

export function getMDXComponents(components?: MDXComponents): MDXComponents {
  return {
    ...defaultMdxComponents,
    APIPage,
    StoryEmbed,
    Mermaid,
    QuickStartSelector,
    ...components,
  };
}
