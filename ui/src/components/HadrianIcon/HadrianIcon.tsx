import { forwardRef, useId, type SVGProps } from "react";

import { cn } from "@/utils/cn";

interface HadrianIconProps extends SVGProps<SVGSVGElement> {
  size?: number | string;
}

export const HadrianIcon = forwardRef<SVGSVGElement, HadrianIconProps>(
  ({ size = 24, className, ...props }, ref) => {
    const id = useId();
    return (
      <svg
        ref={ref}
        width={size}
        height={size}
        viewBox="0 0 512 512"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className={cn(className)}
        {...props}
      >
        <rect width="512" height="512" rx="96" />
        <g transform="translate(256,256) scale(0.88) translate(-256,-256)">
          <defs>
            <mask id={`${id}-upper`}>
              <rect x="108" y="106" width="296" height="98" fill="white" />
              <rect x="122" y="118" width="52" height="78" rx="1" fill="black" />
              <rect x="200" y="118" width="112" height="78" rx="1" fill="black" />
              <rect x="338" y="118" width="52" height="78" rx="1" fill="black" />
            </mask>
            <mask id={`${id}-lower`}>
              <rect x="88" y="232" width="336" height="186" fill="white" />
              <path d="M186,418 L186,326 A70,70 0 0,1 326,326 L326,418 Z" fill="black" />
            </mask>
          </defs>
          <rect x="72" y="418" width="368" height="12" rx="2" fill="currentColor" opacity="0.95" />
          <rect x="68" y="428" width="376" height="8" rx="2" fill="currentColor" opacity="0.7" />
          <rect
            x="88"
            y="232"
            width="336"
            height="186"
            fill="currentColor"
            opacity="0.9"
            mask={`url(#${id}-lower)`}
          />
          <path
            d="M186,418 L186,326 A70,70 0 0,1 326,326 L326,418"
            stroke="currentColor"
            strokeWidth="3"
            fill="none"
            opacity="0.5"
          />
          <path
            d="M192,418 L192,328 A64,64 0 0,1 320,328 L320,418"
            stroke="currentColor"
            strokeWidth="1.5"
            fill="none"
            opacity="0.3"
          />
          <rect x="250" y="254" width="12" height="14" rx="1" fill="currentColor" opacity="0.6" />
          <rect x="172" y="318" width="22" height="10" rx="1" fill="currentColor" opacity="0.7" />
          <rect x="318" y="318" width="22" height="10" rx="1" fill="currentColor" opacity="0.7" />
          <rect x="96" y="248" width="14" height="168" fill="currentColor" opacity="0.6" />
          <rect x="116" y="248" width="14" height="168" fill="currentColor" opacity="0.6" />
          <rect x="93" y="240" width="20" height="10" rx="2" fill="currentColor" opacity="0.7" />
          <rect x="113" y="240" width="20" height="10" rx="2" fill="currentColor" opacity="0.7" />
          <rect x="93" y="414" width="20" height="6" rx="1" fill="currentColor" opacity="0.6" />
          <rect x="113" y="414" width="20" height="6" rx="1" fill="currentColor" opacity="0.6" />
          <rect x="152" y="248" width="12" height="168" fill="currentColor" opacity="0.5" />
          <rect x="149" y="240" width="18" height="10" rx="2" fill="currentColor" opacity="0.6" />
          <rect x="382" y="248" width="14" height="168" fill="currentColor" opacity="0.6" />
          <rect x="402" y="248" width="14" height="168" fill="currentColor" opacity="0.6" />
          <rect x="379" y="240" width="20" height="10" rx="2" fill="currentColor" opacity="0.7" />
          <rect x="399" y="240" width="20" height="10" rx="2" fill="currentColor" opacity="0.7" />
          <rect x="379" y="414" width="20" height="6" rx="1" fill="currentColor" opacity="0.6" />
          <rect x="399" y="414" width="20" height="6" rx="1" fill="currentColor" opacity="0.6" />
          <rect x="348" y="248" width="12" height="168" fill="currentColor" opacity="0.5" />
          <rect x="345" y="240" width="18" height="10" rx="2" fill="currentColor" opacity="0.6" />
          <rect x="78" y="222" width="356" height="8" rx="1" fill="currentColor" />
          <rect x="74" y="212" width="364" height="12" rx="1" fill="currentColor" opacity="0.95" />
          <rect x="70" y="204" width="372" height="10" rx="2" fill="currentColor" />
          <rect
            x="108"
            y="106"
            width="296"
            height="98"
            fill="currentColor"
            opacity="0.85"
            mask={`url(#${id}-upper)`}
          />
          <rect x="114" y="118" width="10" height="78" rx="2" fill="currentColor" />
          <rect x="111" y="112" width="16" height="8" rx="2" fill="currentColor" opacity="0.9" />
          <rect x="111" y="194" width="16" height="5" rx="1" fill="currentColor" opacity="0.8" />
          <rect x="172" y="118" width="10" height="78" rx="2" fill="currentColor" />
          <rect x="169" y="112" width="16" height="8" rx="2" fill="currentColor" opacity="0.9" />
          <rect x="169" y="194" width="16" height="5" rx="1" fill="currentColor" opacity="0.8" />
          <rect x="192" y="118" width="10" height="78" rx="2" fill="currentColor" />
          <rect x="189" y="112" width="16" height="8" rx="2" fill="currentColor" opacity="0.9" />
          <rect x="189" y="194" width="16" height="5" rx="1" fill="currentColor" opacity="0.8" />
          <rect x="310" y="118" width="10" height="78" rx="2" fill="currentColor" />
          <rect x="307" y="112" width="16" height="8" rx="2" fill="currentColor" opacity="0.9" />
          <rect x="307" y="194" width="16" height="5" rx="1" fill="currentColor" opacity="0.8" />
          <rect x="330" y="118" width="10" height="78" rx="2" fill="currentColor" />
          <rect x="327" y="112" width="16" height="8" rx="2" fill="currentColor" opacity="0.9" />
          <rect x="327" y="194" width="16" height="5" rx="1" fill="currentColor" opacity="0.8" />
          <rect x="388" y="118" width="10" height="78" rx="2" fill="currentColor" />
          <rect x="385" y="112" width="16" height="8" rx="2" fill="currentColor" opacity="0.9" />
          <rect x="385" y="194" width="16" height="5" rx="1" fill="currentColor" opacity="0.8" />
          <rect x="106" y="102" width="300" height="6" rx="1" fill="currentColor" opacity="0.95" />
          <rect x="102" y="96" width="308" height="7" rx="1" fill="currentColor" />
          <polygon points="190,96 256,62 322,96" fill="currentColor" opacity="0.95" />
          <polygon
            points="190,96 256,62 322,96"
            stroke="currentColor"
            strokeWidth="2"
            fill="none"
            opacity="0.4"
          />
          <polygon
            points="200,93 256,68 312,93"
            stroke="currentColor"
            strokeWidth="1"
            fill="none"
            opacity="0.3"
          />
          <path d="M252,62 L256,52 L260,62" fill="currentColor" opacity="0.8" />
          <rect x="88" y="232" width="60" height="186" fill="currentColor" opacity="0.08" />
          <rect x="364" y="232" width="60" height="186" fill="currentColor" opacity="0.08" />
        </g>
      </svg>
    );
  }
);

HadrianIcon.displayName = "HadrianIcon";
