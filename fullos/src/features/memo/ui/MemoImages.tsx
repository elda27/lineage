import { useEffect, useState } from "react";

import { convertFileSrc } from "@tauri-apps/api/core";

import type { MemoImage } from "@core/domain/memo/Memo";

export function MemoImages({ images }: { images: MemoImage[] }) {
  const [expanded, setExpanded] = useState<MemoImage | null>(null);

  useEffect(() => {
    if (!expanded) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setExpanded(null);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [expanded]);

  if (images.length === 0) return null;

  return (
    <>
      <section className="border-b border-line py-5" aria-label="添付画像">
        <div className="grid grid-cols-2 gap-2.5">
          {images.map((image) => (
            <button
              key={image.id}
              type="button"
              className="group aspect-[4/3] cursor-zoom-in overflow-hidden rounded-xl border border-line bg-[#f5f4f0] p-0"
              aria-label={`${image.name}を拡大表示`}
              onClick={() => setExpanded(image)}
            >
              <img
                src={convertFileSrc(image.path)}
                alt={image.name}
                loading="lazy"
                className="h-full w-full object-cover transition-transform duration-200 group-hover:scale-[1.03]"
              />
            </button>
          ))}
        </div>
      </section>

      {expanded && (
        <div
          role="dialog"
          aria-modal="true"
          aria-label={`${expanded.name}の拡大表示`}
          className="fixed inset-0 z-40 grid cursor-zoom-out place-items-center bg-[#181914e8] p-5 animate-[fade_0.15s]"
          onMouseDown={(event) => event.target === event.currentTarget && setExpanded(null)}
        >
          <img
            src={convertFileSrc(expanded.path)}
            alt={expanded.name}
            className="max-h-[90vh] max-w-[92vw] cursor-default object-contain shadow-2xl"
          />
          <button
            type="button"
            className="absolute top-4 right-5 cursor-pointer border-0 bg-transparent text-3xl text-white"
            aria-label="拡大表示を閉じる"
            onClick={() => setExpanded(null)}
          >
            ×
          </button>
          <p className="absolute bottom-3 max-w-[80vw] truncate text-xs text-white/80">
            {expanded.name}
          </p>
        </div>
      )}
    </>
  );
}
