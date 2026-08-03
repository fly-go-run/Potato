import type { SVGProps } from "react";

/**
 * The small Potato brand mark used where the desktop icon is shown in-app.
 *
 * It intentionally omits the app-icon background so it can sit inside the
 * existing circular control in the sidebar without creating a second tile.
 */
export function PotatoMark({
  size = 18,
  ...props
}: SVGProps<SVGSVGElement> & { size?: number }) {
  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="240 220 560 610"
      fill="none"
      {...props}
    >
      <defs>
        <linearGradient
          id="potato-mark-fill"
          x1="355"
          y1="392"
          x2="671"
          y2="794"
          gradientUnits="userSpaceOnUse"
        >
          <stop stopColor="#E2B270" />
          <stop offset="1" stopColor="#C9874A" />
        </linearGradient>
      </defs>
      <path
        d="M493 367C493 329 474 290 440 258"
        stroke="#66855E"
        strokeWidth="24"
        strokeLinecap="round"
      />
      <path
        d="M488 349C450 320 406 321 376 348C412 369 452 367 488 349Z"
        fill="#7FA276"
        stroke="#4E6E4D"
        strokeWidth="16"
        strokeLinejoin="round"
      />
      <path
        d="M493 333C501 284 542 250 591 260C579 310 545 343 493 333Z"
        fill="#8CAF7C"
        stroke="#4E6E4D"
        strokeWidth="16"
        strokeLinejoin="round"
      />
      <path
        d="M270 578C261 489 322 411 420 394C483 383 527 408 582 398C680 380 758 440 769 532C780 624 739 704 664 756C604 798 518 812 426 794C334 776 279 697 270 578Z"
        fill="url(#potato-mark-fill)"
        stroke="#5C3D25"
        strokeWidth="26"
        strokeLinejoin="round"
      />
      <path
        d="M350 513C379 481 415 464 455 459"
        stroke="#F1C98C"
        strokeWidth="20"
        strokeLinecap="round"
        opacity="0.65"
      />
      <circle cx="401" cy="601" r="14" fill="#9A6339" />
      <circle cx="595" cy="566" r="13" fill="#9A6339" />
      <circle cx="555" cy="688" r="11" fill="#9A6339" />
    </svg>
  );
}
