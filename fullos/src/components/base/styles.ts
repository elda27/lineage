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
export const heroPadding = "px-5 sm:px-[5vw] min-[1051px]:px-[8vw]";
export const pagePadding = "px-5 sm:px-[45px] min-[1051px]:px-[clamp(50px,8vw,110px)]";
export const standardPage = `min-h-screen mx-auto max-w-[1180px] py-9 sm:py-16 ${pagePadding}`;
export const toggleTrack = "h-5 w-9 rounded-xl border-0 p-0.5 cursor-pointer";
export const toggleKnob =
  "block h-4 w-4 rounded-full bg-white shadow-[0_1px_2px_#0003] transition duration-200";
