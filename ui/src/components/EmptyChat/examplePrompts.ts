import {
  Lightbulb,
  Code,
  Search,
  BarChart3,
  PenLine,
  Palette,
  type LucideIcon,
} from "lucide-react";

export interface ExamplePrompt {
  /** Short title for the prompt card */
  title: string;
  /** Full prompt text to insert into chat */
  prompt: string;
}

export interface PromptCategory {
  /** Category identifier */
  id: string;
  /** Display name */
  name: string;
  /** Icon component */
  icon: LucideIcon;
  /** Tailwind color classes for the category */
  color: string;
  /** Example prompts in this category */
  prompts: ExamplePrompt[];
}

export const EXAMPLE_PROMPT_CATEGORIES: PromptCategory[] = [
  {
    id: "general",
    name: "General",
    icon: Lightbulb,
    color: "text-amber-500",
    prompts: [
      {
        title: "Explain a concept",
        prompt:
          "Explain the concept of [topic] in simple terms, then provide a more technical explanation for someone with domain expertise.",
      },
      {
        title: "Compare options",
        prompt:
          "Compare and contrast [option A] vs [option B] for [use case]. Include pros, cons, and a recommendation based on different scenarios.",
      },
      {
        title: "Summarize document",
        prompt:
          "Summarize the key points from the following text, highlighting the main arguments, supporting evidence, and conclusions:\n\n[paste text here]",
      },
      {
        title: "Decision framework",
        prompt:
          "Create a decision framework for [decision]. Include criteria to evaluate, weighted scoring approach, and risk assessment for each option.",
      },
      {
        title: "Learn new skill",
        prompt:
          "Create a structured learning plan for [skill/topic]. Include prerequisites, key concepts to master, practice exercises, and milestones to track progress.",
      },
      {
        title: "Troubleshoot problem",
        prompt:
          "Help me troubleshoot [problem]. Walk through diagnostic steps systematically, identify likely root causes, and suggest solutions in order of probability.",
      },
    ],
  },
  {
    id: "coding",
    name: "Coding",
    icon: Code,
    color: "text-blue-500",
    prompts: [
      {
        title: "Debug this code",
        prompt:
          "Debug the following code. Identify the bug, explain why it occurs, and provide a corrected version:\n\n```\n[paste code here]\n```",
      },
      {
        title: "Code review",
        prompt:
          "Review this code for bugs, performance issues, security vulnerabilities, and adherence to best practices. Suggest improvements:\n\n```\n[paste code here]\n```",
      },
      {
        title: "Implement feature",
        prompt:
          "Implement a [feature description] in [language/framework]. Include error handling, tests, and documentation. Consider edge cases.",
      },
      {
        title: "Optimize performance",
        prompt:
          "Analyze this code for performance bottlenecks and suggest optimizations. Explain the time/space complexity before and after:\n\n```\n[paste code here]\n```",
      },
      {
        title: "Write tests",
        prompt:
          "Write comprehensive tests for this code. Include unit tests, edge cases, and integration tests where appropriate. Use [testing framework]:\n\n```\n[paste code here]\n```",
      },
      {
        title: "Refactor code",
        prompt:
          "Refactor this code to improve readability, maintainability, and adherence to [language/framework] best practices. Explain each change:\n\n```\n[paste code here]\n```",
      },
    ],
  },
  {
    id: "research",
    name: "Research",
    icon: Search,
    color: "text-green-500",
    prompts: [
      {
        title: "Literature review",
        prompt:
          "Provide an overview of the current state of research on [topic]. Include key findings, methodologies, debates, and gaps in the literature.",
      },
      {
        title: "Fact check",
        prompt:
          "Evaluate the following claim for accuracy. Provide supporting or contradicting evidence, note any nuances, and rate confidence level:\n\n[claim]",
      },
      {
        title: "Research methodology",
        prompt:
          "Design a research methodology to investigate [research question]. Include data collection methods, analysis approach, and potential limitations.",
      },
      {
        title: "Synthesize sources",
        prompt:
          "Synthesize these sources into a coherent analysis. Identify common themes, contradictions, and gaps. Provide citations for key claims:\n\n[paste sources or summaries]",
      },
      {
        title: "Competitive analysis",
        prompt:
          "Conduct a competitive analysis of [company/product] vs [competitors]. Compare features, pricing, market positioning, strengths, and weaknesses.",
      },
      {
        title: "Technology assessment",
        prompt:
          "Assess [technology/tool] for [use case]. Evaluate maturity, ecosystem, learning curve, performance characteristics, and long-term viability.",
      },
    ],
  },
  {
    id: "data",
    name: "Data Analysis",
    icon: BarChart3,
    color: "text-purple-500",
    prompts: [
      {
        title: "Analyze dataset",
        prompt:
          "Analyze this dataset and provide insights. Include summary statistics, identify patterns and anomalies, and suggest visualizations:\n\n[paste data or describe dataset]",
      },
      {
        title: "SQL query",
        prompt:
          "Write a SQL query to [describe what you need]. The database has these tables:\n\n[describe schema]\n\nOptimize for performance and explain the query logic.",
      },
      {
        title: "Statistical test",
        prompt:
          "Recommend and explain the appropriate statistical test for [research question] with [data description]. Include assumptions to check and how to interpret results.",
      },
      {
        title: "Data pipeline",
        prompt:
          "Design a data pipeline for [use case]. Include data sources, transformation steps, validation rules, and destination schema. Consider error handling and monitoring.",
      },
      {
        title: "Dashboard design",
        prompt:
          "Design a dashboard for [audience] to track [metrics/KPIs]. Include chart types, layout, drill-down capabilities, and refresh frequency recommendations.",
      },
      {
        title: "Data quality audit",
        prompt:
          "Create a data quality audit checklist for [dataset/system]. Include completeness, accuracy, consistency, timeliness, and validity checks with remediation steps.",
      },
    ],
  },
  {
    id: "writing",
    name: "Writing",
    icon: PenLine,
    color: "text-rose-500",
    prompts: [
      {
        title: "Draft document",
        prompt:
          "Draft a [document type] for [purpose/audience]. The tone should be [formal/casual/technical]. Key points to cover:\n\n[list points]",
      },
      {
        title: "Edit for clarity",
        prompt:
          "Edit the following text for clarity, conciseness, and impact. Preserve the original meaning while improving readability:\n\n[paste text]",
      },
      {
        title: "Technical documentation",
        prompt:
          "Write technical documentation for [feature/API/system]. Include overview, usage examples, parameters, return values, and error handling.",
      },
      {
        title: "Email draft",
        prompt:
          "Draft a professional email to [recipient] about [topic]. The goal is to [desired outcome]. Tone should be [formal/friendly/urgent]. Include subject line.",
      },
      {
        title: "Presentation outline",
        prompt:
          "Create an outline for a [duration] presentation on [topic] for [audience]. Include key points, supporting data to gather, and speaker notes for transitions.",
      },
      {
        title: "Meeting summary",
        prompt:
          "Summarize these meeting notes into a structured format with: key decisions made, action items with owners, open questions, and next steps:\n\n[paste notes]",
      },
    ],
  },
  {
    id: "creative",
    name: "Creative",
    icon: Palette,
    color: "text-cyan-500",
    prompts: [
      {
        title: "Brainstorm ideas",
        prompt:
          "Generate 10 creative ideas for [project/problem]. For each idea, provide a brief description, potential benefits, and implementation considerations.",
      },
      {
        title: "Design system",
        prompt:
          "Design a [type] system for [use case]. Consider user experience, scalability, and maintainability. Provide component breakdown and interaction patterns.",
      },
      {
        title: "Problem reframe",
        prompt:
          "Reframe this problem from multiple perspectives: [describe problem]. Consider it from user, business, technical, and ethical viewpoints. Suggest novel approaches.",
      },
      {
        title: "Name generator",
        prompt:
          "Generate creative names for [product/project/company] in the [industry/domain]. For each suggestion, explain the meaning, check domain availability considerations, and note potential issues.",
      },
      {
        title: "User personas",
        prompt:
          "Create detailed user personas for [product/service]. Include demographics, goals, pain points, behaviors, and scenarios. Base on [target market description].",
      },
      {
        title: "Workshop agenda",
        prompt:
          "Design a [duration] workshop agenda for [goal] with [number] participants. Include activities, timing, materials needed, and facilitation tips for each section.",
      },
    ],
  },
];
