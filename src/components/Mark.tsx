/**
 * The app mark — decision 29. A bold, geometric "G" on a small neutral
 * tile: GGallery's own initial, not a picture-of-a-camera cliché. Deliberately
 * not accent-tinted — the accent is a per-session preference, and an
 * identity that changes colour with one is not an identity. Used at 16–20px
 * in the window bar and regenerated as the Windows `.ico` (see
 * `docs/reference/` and the icon generation notes).
 */

export function Mark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      aria-hidden="true"
      className={className}
      xmlns="http://www.w3.org/2000/svg"
    >
      <rect
        x="1"
        y="1"
        width="30"
        height="30"
        rx="7"
        className="fill-raised stroke-line"
        strokeWidth="1.5"
      />
      {/* The ring is a full circle; the mask rect (plate colour) cuts the
          opening on the right, and the bar reads as the G's crossbar
          reaching back in from that opening. */}
      <circle cx="16" cy="16" r="8" className="stroke-fg" fill="none" strokeWidth="5" />
      <rect x="16" y="11" width="12" height="10" className="fill-raised" />
      <rect x="16" y="13.5" width="10" height="5" className="fill-fg" />
    </svg>
  );
}
