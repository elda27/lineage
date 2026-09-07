import { toggleKnob, toggleTrack } from "./styles";

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
