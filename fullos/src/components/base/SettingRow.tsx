import type React from "react";

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
