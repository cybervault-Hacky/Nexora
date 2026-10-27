interface ApertureMarkProps {
  /** Rendered size in CSS pixels. */
  size?: number;
  className?: string;
}

/*
 * Six blades spaced 60° apart, each swept 58° so consecutive blades overlap
 * slightly — the same aperture geometry used by the desktop application's
 * mark, drawn here as vector so the website ships no binary brand file and
 * makes no network request for it.
 */
const BLADES = Array.from({ length: 6 }, (_, index) => {
  const start = (index * 60 * Math.PI) / 180;
  const end = ((index * 60 + 58) * Math.PI) / 180;
  const point = (radius: number, angle: number) =>
    `${24 + radius * Math.cos(angle)} ${24 + radius * Math.sin(angle)}`;

  return `M ${point(8, start)} L ${point(18.5, end)}`;
});

/**
 * Nexora aperture mark.
 *
 * Stroked and geometric, inheriting `currentColor` so it adapts to the light
 * theme. Decorative when paired with the wordmark, so it is hidden from
 * assistive technology.
 */
export default function ApertureMark({
  size = 24,
  className,
}: ApertureMarkProps) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      className={className}
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.4"
      strokeLinecap="round"
    >
      <circle cx="24" cy="24" r="19" opacity="0.9" />
      {BLADES.map((path) => (
        <path key={path} d={path} opacity="0.55" />
      ))}
      <circle
        cx="24"
        cy="24"
        r="3"
        fill="currentColor"
        stroke="none"
        opacity="0.9"
      />
    </svg>
  );
}
