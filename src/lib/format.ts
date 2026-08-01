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

/** Unix seconds to `Jun 2024`. */
export function formatMonth(seconds: number): string {
  const date = new Date(seconds * 1000);
  return `${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
}

export function monthLabel(month: number): string {
  return MONTHS[month] ?? "";
}

export function formatDate(seconds: number): string {
  const date = new Date(seconds * 1000);
  return `${date.getDate()} ${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
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
