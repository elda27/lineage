/** Route-level screen. Business behavior remains in feature services/components. */
import { useState } from "react";

import { AutomationSettings } from "@/features/automation/ui/AutomationSettings";
import { AgentSkillSettings } from "@/features/skill/ui/AgentSkillSettings";
import { LineageUpdateSettings } from "@/features/updater/ui/LineageUpdateSettings";
import {
  eyebrow,
  Icon,
  quietButton,
  serifTitle,
  SettingRow,
  standardPage,
  tagChip,
  Toggle,
} from "@/components/base";

export function SettingsPage() {
  const [launch, setLaunch] = useState(true),
    [copy, setCopy] = useState(true),
    [theme, setTheme] = useState("システム");
  const section = "mt-[30px]",
    sectionTitle = "mb-[3px] text-[13px] font-bold",
    sectionDesc = "mb-2.5 text-[10px] text-muted",
    settingsCard = "overflow-hidden rounded-[10px] border border-line bg-white";
  return (
    <div className={standardPage}>
      <div className="mb-8">
        <p className={eyebrow}>PREFERENCES</p>
        <h1 className={`${serifTitle} mb-[9px] text-[34px]`}>設定</h1>
        <p className="text-[13px] text-muted">minos と fullos の動作をカスタマイズします。</p>
      </div>
      <section className={section}>
        <h2 className={sectionTitle}>minos</h2>
        <p className={sectionDesc}>クイック入力の動作</p>
        <div className={settingsCard}>
          <SettingRow title="システム起動時に開始" desc="PCへのログイン時にminosを自動で起動します">
            <Toggle value={launch} setValue={setLaunch} />
          </SettingRow>
          <SettingRow
            title="直前のアプリからテキストを取得"
            desc="呼び出したとき、選択中のテキストを入力欄へコピーします"
          >
            <Toggle value={copy} setValue={setCopy} />
          </SettingRow>
          <SettingRow title="呼び出しショートカット" desc="どこからでもクイック入力を開きます">
            <kbd className="rounded-md border border-b-2 border-[#deded8] bg-[#fafaf8] px-[9px] py-[5px] font-sans text-[11px] font-medium">
              Alt + Space
            </kbd>
          </SettingRow>
        </div>
      </section>
      <section className={section}>
        <h2 className={sectionTitle}>表示</h2>
        <p className={sectionDesc}>fullos の見た目</p>
        <div className={settingsCard}>
          <SettingRow title="テーマ" desc="アプリ全体の表示テーマ">
            <div className="rounded-[7px] bg-[#f1f1ed] p-[3px]">
              {["ライト", "システム", "ダーク"].map((v) => (
                <button
                  className={`cursor-pointer rounded-[5px] border-0 px-2.5 py-[5px] text-[9px] ${theme === v ? "bg-white shadow-[0_1px_3px_#0001]" : "bg-transparent"}`}
                  onClick={() => setTheme(v)}
                  key={v}
                >
                  {v}
                </button>
              ))}
            </div>
          </SettingRow>
          <LineageUpdateSettings />
        </div>
      </section>
      <section className={section}>
        <h2 className={sectionTitle}>メタ情報の短縮入力</h2>
        <p className={sectionDesc}>minos で # に続けて入力する短縮文字</p>
        <div className={settingsCard}>
          {[
            ["タスク", "task", "32回使用"],
            ["仕事", "work", "24回使用"],
            ["アイデア", "idea", "18回使用"],
          ].map((t) => (
            <div
              className="flex h-[55px] items-center gap-3 border-b border-line px-[18px] last:border-b-0"
              key={t[0]}
            >
              <span className={tagChip}>#{t[0]}</span>
              <span className="border-l border-line pl-3 font-mono text-[11px]">{t[1]}</span>
              <small className="ml-auto text-[9px] text-[#999a94]">{t[2]}</small>
              <button className={quietButton}>
                <Icon name="edit" />
              </button>
            </div>
          ))}
        </div>
      </section>
      <AutomationSettings />
      <AgentSkillSettings />
    </div>
  );
}
