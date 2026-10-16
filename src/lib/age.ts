const DAY_MS = 86_400_000;
const MONTH_DAYS = 30.44;

export function daysSince(mtimeSec: number, now = Date.now()): number {
  return Math.max(0, Math.floor((now - mtimeSec * 1000) / DAY_MS));
}

export function monthsSince(mtimeSec: number, now = Date.now()): number {
  return Math.floor(daysSince(mtimeSec, now) / MONTH_DAYS);
}

export function isOlderThan(mtimeSec: number, days: number, now = Date.now()): boolean {
  return mtimeSec > 0 && daysSince(mtimeSec, now) >= days;
}

export function ageText(mtimeSec: number, now = Date.now()): string {
  if (mtimeSec <= 0) return "";
  const days = daysSince(mtimeSec, now);
  if (days < 7) return "Modified this week";
  if (days < 60) return "Modified last month";
  const months = Math.max(2, monthsSince(mtimeSec, now));
  if (months < 12) return `Modified ${months} months ago`;
  const years = Math.floor(months / 12);
  return `Not modified in ${years} year${years > 1 ? "s" : ""}`;
}
