/** 表示用の日時・サイズ整形。 */

const MINUTE = 60 * 1000;
const HOUR = 60 * MINUTE;
const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

/** 一覧に出す相対時刻（「10分前」「昨日」「8月10日」）。 */
export function relativeTime(iso: string, now: Date = new Date()): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";

  const elapsed = now.getTime() - date.getTime();
  if (elapsed < MINUTE) return "たった今";
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)}分前`;

  const days = calendarDaysBetween(date, now);
  if (days === 0) return `${Math.floor(elapsed / HOUR)}時間前`;
  if (days === 1) return "昨日";
  if (date.getFullYear() === now.getFullYear()) {
    return `${date.getMonth() + 1}月${date.getDate()}日`;
  }
  return absoluteDate(iso);
}

/** 詳細画面に出す絶対日時（「2026年8月12日 10:24」）。 */
export function absoluteDateTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const time = `${date.getHours()}:${String(date.getMinutes()).padStart(2, "0")}`;
  return `${absoluteDate(iso)} ${time}`;
}

/** 「2026年8月12日 水曜日」。 */
export function longDate(date: Date): string {
  const weekday = ["日", "月", "火", "水", "木", "金", "土"][date.getDay()];
  return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日 ${weekday}曜日`;
}

/** ストレージ使用量の表示（「24 MB」「1.4 GB」）。1024 進。 */
export function formatBytes(bytes: number): string {
  let value = Math.max(bytes, 0);
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  // 1桁台のときだけ小数第1位まで見せる（1.4 GB / 24 MB）。
  const rounded = unit > 0 && value < 10 ? Number(value.toFixed(1)) : Math.round(value);
  return `${rounded} ${BYTE_UNITS[unit]}`;
}

function absoluteDate(iso: string): string {
  const date = new Date(iso);
  return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`;
}

/** 時刻を落として「何日前か」を数える（23時→翌1時は「昨日」）。 */
function calendarDaysBetween(from: Date, to: Date): number {
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  return Math.round((startOfDay(to) - startOfDay(from)) / (24 * HOUR));
}
