/** Sizes, durations, dates and counts. One implementation, used everywhere. */

const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

export function formatDuration(ms: number | null): string {
  if (!ms || ms <= 0) return "";
  const total = Math.round(ms / 1000);
  const seconds = total % 60;
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  const pad = (n: number) => String(n).padStart(2, "0");
  return hours > 0
    ? `${hours}:${pad(minutes)}:${pad(seconds)}`
    : `${minutes}:${pad(seconds)}`;
}

export function formatCount(n: number): string {
  return n.toLocaleString();
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/** Unix seconds to `12 Jun 2024` — what the scrubber shows while dragging.
 *  The month-name and `Jun 2024` helpers went with the scrubber's year
 *  column; nothing labels a timeline by month any more. */
export function formatDate(seconds: number): string {
  const date = new Date(seconds * 1000);
  return `${date.getDate()} ${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
}

/** `8:32 AM` — built by hand rather than through `toLocaleString`, whose
 *  AM/PM casing follows the OS locale and silently comes back lowercase on
 *  some of them. One implementation means it is never inconsistent between
 *  two dates sitting in the same panel. */
function formatTime(date: Date): string {
  const period = date.getHours() >= 12 ? "PM" : "AM";
  const hours = date.getHours() % 12 || 12;
  const minutes = String(date.getMinutes()).padStart(2, "0");
  return `${hours}:${minutes} ${period}`;
}

/** Unix seconds to `12 Jun 2024, 8:32 AM` — `formatDate` plus a time, for the
 *  pane's Created/Added rows, where the hour matters and the OS locale's
 *  am/pm rendering does not. */
export function formatDateTime(seconds: number): string {
  const date = new Date(seconds * 1000);
  return `${formatDate(seconds)}, ${formatTime(date)}`;
}

/** Unix seconds to `"3 days ago"` / `"5 months ago"` — coarse, one unit,
 *  matching docs/DESIGN.md's folder header mockup ("last added: 5 months
 *  ago"). Not meant to be precise; it's a staleness signal. */
export function formatTimeAgo(seconds: number): string {
  const diff = Math.max(0, Date.now() / 1000 - seconds);
  const DAY = 86_400;
  if (diff < DAY) return "today";
  if (diff < 30 * DAY) {
    const days = Math.floor(diff / DAY);
    return `${days} day${days === 1 ? "" : "s"} ago`;
  }
  if (diff < 365 * DAY) {
    const months = Math.floor(diff / (30 * DAY));
    return `${months} month${months === 1 ? "" : "s"} ago`;
  }
  const years = Math.floor(diff / (365 * DAY));
  return `${years} year${years === 1 ? "" : "s"} ago`;
}
