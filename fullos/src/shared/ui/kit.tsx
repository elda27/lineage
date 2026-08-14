/**
 * 画面をまたいで使い回す見た目と部品。
 *
 * シェル（app/）と features/* の両方から使うため、どちらかに置くと循環参照になる。
 */

import type React from "react";

/* Tailwind のクラス列をここに集約する。 */
export const button =
  "inline-flex items-center justify-center gap-2 rounded-lg text-[13px] font-semibold cursor-pointer";
export const primaryLook =
  "border border-[#39383c] bg-[#39383c] text-white shadow-[0_1px_2px_#0002] hover:bg-[#242326]";
export const primaryButton = `${button} px-4 py-2.5 ${primaryLook}`;
export const smallPrimaryButton = `${button} px-[13px] py-2 ${primaryLook}`;
// 旧 .secondary は色指定が無く、feature-card 内では文字が白背景に白で消えていたため text-ink を明示する。
export const secondaryButton = `${button} px-4 py-2.5 border border-[#deded8] bg-white text-ink`;
export const quietButton =
  "grid place-items-center rounded-[7px] p-2 cursor-pointer hover:bg-[#eeeee9]";
export const tagChip =
  "inline-flex rounded-[5px] bg-[#f1f0ed] px-[7px] py-0.5 text-[9px] font-medium text-[#6e706a]";
export const eyebrow =
  "mb-3.5 text-[11px] font-semibold uppercase tracking-[0.13em] text-[#96978f]";
export const cardSurface = "overflow-hidden rounded-[11px] border border-line bg-white";
export const subheading = "mb-[5px] text-[15px] font-bold";
export const serifTitle = "font-serif font-normal tracking-[-0.025em]";
/** 1050px 以下では左右の余白を詰める（旧 @media(max-width:1050px) 相当）。 */
export const heroPadding = "px-[5vw] min-[1051px]:px-[8vw]";
export const pagePadding = "px-[45px] min-[1051px]:px-[clamp(50px,8vw,110px)]";
export const standardPage = `min-h-screen mx-auto max-w-[1180px] py-16 ${pagePadding}`;
export const toggleTrack = "h-5 w-9 rounded-xl border-0 p-0.5 cursor-pointer";
export const toggleKnob =
  "block h-4 w-4 rounded-full bg-white shadow-[0_1px_2px_#0003] transition duration-200";

export type IconName =
  | "home"
  | "search"
  | "sparkles"
  | "settings"
  | "plus"
  | "clock"
  | "check"
  | "file"
  | "arrow"
  | "more"
  | "tag"
  | "trash"
  | "edit"
  | "command"
  | "inbox"
  | "archive"
  | "chevron"
  | "play"
  | "moon";

export function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, React.ReactNode> = {
    home: (
      <>
        <path d="m3 11 9-8 9 8" />
        <path d="M5 10v10h14V10M9 20v-6h6v6" />
      </>
    ),
    search: (
      <>
        <circle cx="11" cy="11" r="7" />
        <path d="m20 20-4-4" />
      </>
    ),
    sparkles: (
      <>
        <path d="m12 3 1.2 3.8L17 8l-3.8 1.2L12 13l-1.2-3.8L7 8l3.8-1.2L12 3Z" />
        <path d="m5 14 .8 2.2L8 17l-2.2.8L5 20l-.8-2.2L2 17l2.2-.8L5 14ZM19 13l.6 1.4L21 15l-1.4.6L19 17l-.6-1.4L17 15l1.4-.6L19 13Z" />
      </>
    ),
    settings: (
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" />
      </>
    ),
    plus: <path d="M12 5v14M5 12h14" />,
    clock: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7v5l3 2" />
      </>
    ),
    check: <path d="m5 12 4 4L19 6" />,
    file: (
      <>
        <path d="M6 3h8l4 4v14H6z" />
        <path d="M14 3v5h5M9 13h6M9 17h5" />
      </>
    ),
    arrow: (
      <>
        <path d="M5 12h14M14 7l5 5-5 5" />
      </>
    ),
    more: (
      <>
        <circle cx="5" cy="12" r="1" fill="currentColor" />
        <circle cx="12" cy="12" r="1" fill="currentColor" />
        <circle cx="19" cy="12" r="1" fill="currentColor" />
      </>
    ),
    tag: (
      <>
        <path d="M20 13 13 20 4 11V4h7z" />
        <circle cx="8.5" cy="8.5" r="1" />
      </>
    ),
    trash: (
      <>
        <path d="M4 7h16M9 7V4h6v3M7 7l1 14h8l1-14M10 11v6M14 11v6" />
      </>
    ),
    edit: (
      <>
        <path d="m14 5 5 5L9 20H4v-5zM12 7l5 5" />
      </>
    ),
    command: (
      <>
        <rect x="4" y="4" width="6" height="6" rx="3" />
        <rect x="14" y="4" width="6" height="6" rx="3" />
        <rect x="4" y="14" width="6" height="6" rx="3" />
        <rect x="14" y="14" width="6" height="6" rx="3" />
        <path d="M10 7v10c0 4 4 4 4 0V7" />
      </>
    ),
    inbox: (
      <>
        <path d="M4 5h16v14H4z" />
        <path d="M4 14h5l2 2h2l2-2h5" />
      </>
    ),
    archive: (
      <>
        <path d="M3 5h18v4H3zM5 9v10h14V9" />
        <path d="M10 13h4" />
      </>
    ),
    chevron: <path d="m9 7 5 5-5 5" />,
    play: <path d="m9 6 9 6-9 6z" />,
    moon: <path d="M20 15.5A8 8 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z" />,
  };
  return (
    <svg
      className="flex-none"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {paths[name]}
    </svg>
  );
}

export function SettingRow({
  title,
  desc,
  children,
}: {
  title: string;
  desc: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex min-h-[69px] items-center border-b border-line px-[18px] py-[15px] last:border-b-0">
      <div className="flex flex-1 flex-col gap-[3px]">
        <b className="text-[12px]">{title}</b>
        <small className="text-[9px] text-[#8f918a]">{desc}</small>
      </div>
      {children}
    </div>
  );
}
export function Toggle({ value, setValue }: { value: boolean; setValue: (v: boolean) => void }) {
  return (
    <button
      role="switch"
      aria-checked={value}
      className={`${toggleTrack} ${value ? "bg-[#7063b6]" : "bg-[#d8d8d2]"}`}
      onClick={() => setValue(!value)}
    >
      <i className={`${toggleKnob} ${value ? "translate-x-4" : ""}`} />
    </button>
  );
}
