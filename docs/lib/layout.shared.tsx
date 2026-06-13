import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";
import Image from "next/image";

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <>
          <Image
            src={`${process.env.DOCS_BASE_PATH || ""}/icon.svg`}
            alt=""
            width={24}
            height={24}
            className="rounded-md"
          />
          Hadrian
        </>
      ),
    },
    links: [
      {
        text: "Documentation",
        url: "/docs",
        active: "nested-url",
      },
      {
        text: "GitHub",
        url: "https://github.com/hadriangateway/hadrian",
        external: true,
      },
    ],
  };
}
