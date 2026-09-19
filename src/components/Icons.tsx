import type { JSX } from "solid-js";

type IconProps = {
  class?: string;
  size?: number;
};

/**
 * One icon family, one visual language: 24px box, 1.75 stroke, round caps.
 * Filled variants are reserved for "on" states (favourite, playing).
 */
function base(props: IconProps, children: JSX.Element, filled = false) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={props.size ?? 20}
      height={props.size ?? 20}
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      class={props.class}
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

const STAR = "M12 3.6l2.6 5.3 5.8.85-4.2 4.1 1 5.75L12 16.9l-5.2 2.7 1-5.75-4.2-4.1 5.8-.85z";

export const StarOutline = (props: IconProps) => base(props, <path d={STAR} />);
export const StarFilled = (props: IconProps) => base(props, <path d={STAR} />, true);

export const Play = (props: IconProps) => base(props, <path d="M7 4.5l12 7.5-12 7.5z" />, true);

export const Check = (props: IconProps) => base(props, <path d="M4 12l5 5L20 6" />);

export const ChevronLeft = (props: IconProps) => base(props, <path d="M15 5l-7 7 7 7" />);
export const ChevronRight = (props: IconProps) => base(props, <path d="M9 5l7 7-7 7" />);

export const Search = (props: IconProps) =>
  base(
    props,
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="M16 16l4.5 4.5" />
    </>,
  );

export const Tv = (props: IconProps) =>
  base(
    props,
    <>
      <rect x="2.5" y="7" width="19" height="13" rx="2" />
      <path d="M8 3.5l4 3.5 4-3.5" />
    </>,
  );

export const Film = (props: IconProps) =>
  base(
    props,
    <>
      <rect x="2.5" y="4" width="19" height="16" rx="2" />
      <path d="M7.5 4v16M16.5 4v16M2.5 12h19" />
    </>,
  );

export const Stack = (props: IconProps) =>
  base(
    props,
    <>
      <rect x="3" y="7" width="14" height="13" rx="2" />
      <path d="M7 4h11a3 3 0 0 1 3 3v9" />
    </>,
  );

export const Signal = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M4 20V13M9 20V9M14 20V5M19 20v-9" />
    </>,
  );

export const Library = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M3 4h4v16H3zM9 4h4v16H9zM16 5l4 14" />
    </>,
  );

export const Sliders = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
      <circle cx="16" cy="7" r="2" />
      <circle cx="10" cy="17" r="2" />
    </>,
  );

export const Pause = (props: IconProps) =>
  base(props, <path d="M8 5v14M16 5v14" />);

export const Volume = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M4 9v6h4l5 4V5L8 9z" />
      <path d="M17 9.5a3.5 3.5 0 010 5M19.5 7a7 7 0 010 10" />
    </>,
  );

export const VolumeOff = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M4 9v6h4l5 4V5L8 9z" />
      <path d="M17 10l4 4M21 10l-4 4" />
    </>,
  );

export const Fullscreen = (props: IconProps) =>
  base(props, <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />);

export const FullscreenExit = (props: IconProps) =>
  base(props, <path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />);

export const Pip = (props: IconProps) =>
  base(
    props,
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <rect x="12" y="11" width="7" height="6" rx="1" />
    </>,
  );

export const Subtitles = (props: IconProps) =>
  base(
    props,
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="M7 14h4M14 14h3" />
    </>,
  );

export const Gauge = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M4 16a8 8 0 1116 0" />
      <path d="M12 16l4-4" />
    </>,
  );

export const Back10 = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M11 5L6 9l5 4" />
      <path d="M6 9h7a5 5 0 110 10H9" />
    </>,
  );

export const Forward10 = (props: IconProps) =>
  base(
    props,
    <>
      <path d="M13 5l5 4-5 4" />
      <path d="M18 9h-7a5 5 0 100 10h4" />
    </>,
  );
