interface NexoraMarkProps {
  size?: number;
  className?: string;
}

/** Original, single-colour geometric N: two uprights joined by a diagonal. */
export function NexoraMark({ size = 24, className }: NexoraMarkProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-hidden="true"
      focusable="false"
    >
      <path
        d="M9 38V10h5.5L34 30V10h5v28h-5.5L14 18v20H9Z"
        fill="currentColor"
      />
    </svg>
  );
}
