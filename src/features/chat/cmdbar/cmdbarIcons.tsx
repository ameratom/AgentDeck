const svgProps = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  "aria-hidden": true as const,
};

export function PlusIcon() {
  return (
    <svg {...svgProps}>
      <line
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth={2}
        x1="12"
        x2="12"
        y1="5"
        y2="19"
      />
      <line
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth={2}
        x1="5"
        x2="19"
        y1="12"
        y2="12"
      />
    </svg>
  );
}

export function AgentIcon() {
  return (
    <svg {...svgProps}>
      <rect
        fill="none"
        height="18"
        rx="5"
        stroke="currentColor"
        strokeWidth={1.75}
        width="18"
        x="3"
        y="3"
      />
      <path d="M9 9l6 2.4-2.4.9-.9 2.4z" fill="currentColor" />
    </svg>
  );
}

export function AppsIcon() {
  return (
    <svg {...svgProps} fill="currentColor">
      <circle cx="8" cy="8" r="2.1" />
      <circle cx="16" cy="8" r="2.1" />
      <circle cx="8" cy="16" r="2.1" />
      <circle cx="16" cy="16" r="2.1" />
    </svg>
  );
}

export function ChevronDownIcon({ size = 16 }: { size?: number }) {
  return (
    <svg
      aria-hidden
      fill="none"
      height={size}
      viewBox="0 0 24 24"
      width={size}
    >
      <path
        d="M6 9l6 6 6-6"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
      />
    </svg>
  );
}

export function MicIcon() {
  return (
    <svg height={22} viewBox="0 0 24 24" width={22} aria-hidden fill="none">
      <rect
        height="11"
        rx="3"
        stroke="currentColor"
        strokeWidth={1.8}
        width="6"
        x="9"
        y="3"
      />
      <path
        d="M5 11a7 7 0 0 0 14 0"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth={1.8}
      />
      <line
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth={1.8}
        x1="12"
        x2="12"
        y1="18"
        y2="21"
      />
    </svg>
  );
}

export function VoiceIcon() {
  return (
    <svg height={20} viewBox="0 0 24 24" width={20} aria-hidden fill="#fff">
      <rect height="6" rx="1.4" width="2.5" x="5" y="9" />
      <rect height="12" rx="1.4" width="2.5" x="8.5" y="6" />
      <rect height="16" rx="1.4" width="2.5" x="12" y="4" />
      <rect height="12" rx="1.4" width="2.5" x="15.5" y="6" />
      <rect height="6" rx="1.4" width="2.5" x="19" y="9" />
    </svg>
  );
}

export function ArrowUpIcon() {
  return (
    <svg height={20} viewBox="0 0 24 24" width={20} aria-hidden fill="none">
      <line
        stroke="#fff"
        strokeLinecap="round"
        strokeWidth={2.2}
        x1="12"
        x2="12"
        y1="19"
        y2="6"
      />
      <path
        d="M6 12l6-6 6 6"
        stroke="#fff"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2.2}
      />
    </svg>
  );
}

export function StopIcon() {
  return (
    <svg height={20} viewBox="0 0 24 24" width={20} aria-hidden>
      <rect fill="#fff" height="10" rx="2.5" width="10" x="7" y="7" />
    </svg>
  );
}