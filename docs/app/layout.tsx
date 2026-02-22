import { Provider } from "@/components/provider";
import { Banner } from "fumadocs-ui/components/banner";
import "./global.css";
import { Inter } from "next/font/google";
import type { Metadata } from "next";

const inter = Inter({
  subsets: ["latin"],
});

export const metadata: Metadata = {
  metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL ?? "https://hadriangateway.com"),
  title: {
    default: "Hadrian Documentation",
    template: "%s | Hadrian",
  },
  description:
    "Hadrian is an open-source AI Gateway providing a unified OpenAI-compatible API for routing requests to multiple LLM providers. Single binary, zero dependencies, all enterprise features free.",
  keywords: [
    "AI Gateway",
    "LLM",
    "OpenAI",
    "Anthropic",
    "API",
    "proxy",
    "multi-tenant",
    "open source",
  ],
  authors: [{ name: "Adam Smith" }],
  openGraph: {
    type: "website",
    locale: "en_US",
    siteName: "Hadrian Documentation",
  },
  twitter: {
    card: "summary_large_image",
  },
};

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={inter.className} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <Provider>
          <Banner id="alpha-warning" variant="rainbow">
            Hadrian is experimental alpha software. Do not use in production.
          </Banner>
          {children}
        </Provider>
      </body>
    </html>
  );
}
