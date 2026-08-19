import { useEffect, useMemo, useState } from "react";
import type { TagDefinition } from "@core/domain/tag/TagDefinition";
import { appClient } from "@/shared/api/appClient";
import { primaryButton, secondaryButton } from "@/shared/ui/kit";

export function TagExplorerPage() {
  const [tags, setTags] = useState<TagDefinition[]>([]),
    [selected, setSelected] = useState<TagDefinition | null>(null),
    [query, setQuery] = useState(""),
    [kind, setKind] = useState("all"),
    [sort, setSort] = useState("usage");
  const reload = () =>
    appClient()
      .then((c) => c.listTags())
      .then(setTags);
  useEffect(() => {
    void reload();
  }, []);
  const shown = useMemo(
    () =>
      tags
        .filter(
          (t) =>
            (kind === "all" || t.kind === kind) &&
            t.displayName.toLowerCase().includes(query.toLowerCase()),
        )
        .sort((a, b) =>
          sort === "name"
            ? a.displayName.localeCompare(b.displayName)
            : b.usageCount - a.usageCount,
        ),
    [tags, query, kind, sort],
  );
  return (
    <section className="mx-auto max-w-6xl p-6">
      <div className="mb-5 flex items-center gap-3">
        <h1 className="mr-auto text-2xl font-semibold">Tag Explorer</h1>
        <input
          className="rounded border p-2"
          placeholder="タグを検索"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="all">all kinds</option>
          <option value="user">user</option>
          <option value="builtin">builtin</option>
          <option value="metadata">metadata</option>
        </select>
        <select value={sort} onChange={(e) => setSort(e.target.value)}>
          <option value="usage">usage</option>
          <option value="name">name</option>
        </select>
      </div>
      <div className="overflow-auto rounded-xl border bg-white">
        <table className="w-full text-left text-sm">
          <thead>
            <tr>
              {["Name", "Kind", "Uses", "Last used", "View", "Recipe", "Enabled", "Owner"].map(
                (x) => (
                  <th className="p-3" key={x}>
                    {x}
                  </th>
                ),
              )}
            </tr>
          </thead>
          <tbody>
            {shown.map((t) => (
              <tr
                key={t.id}
                className="cursor-pointer border-t hover:bg-gray-50"
                onClick={() => setSelected(t)}
              >
                <td className="p-3 font-medium">{t.displayName}</td>
                <td>{t.kind}</td>
                <td>{t.usageCount}</td>
                <td>{t.lastUsedAt?.slice(0, 10) ?? "—"}</td>
                <td>{t.view ?? "—"}</td>
                <td>{t.recipe ? "yes" : "—"}</td>
                <td>{t.enabled ? "on" : "off"}</td>
                <td>{t.managed ? "managed" : "external"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {selected && (
        <TagDetail
          tag={selected}
          close={() => setSelected(null)}
          saved={() => {
            setSelected(null);
            void reload();
          }}
        />
      )}
    </section>
  );
}
function TagDetail({
  tag,
  close,
  saved,
}: {
  tag: TagDefinition;
  close: () => void;
  saved: () => void;
}) {
  const [draft, setDraft] = useState(tag);
  const [deleting, setDeleting] = useState(false);
  const save = async () => {
    const c = await appClient();
    await c.updateTag(tag.id, draft);
    saved();
  };
  const remove = async () => {
    if (
      !confirm(`タグ「${tag.displayName}」を削除しますか？過去の記録に付いたタグと来歴は残ります。`)
    )
      return;
    setDeleting(true);
    try {
      await (await appClient()).deleteTag(tag.id);
      saved();
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-30 grid place-items-center bg-black/30">
      <div className="max-h-[90vh] w-[min(560px,94vw)] overflow-y-auto rounded-xl bg-white p-6 shadow-xl">
        <h2 className="text-xl font-semibold">{tag.id}</h2>
        <label className="mt-4 block">
          Display name
          <input
            className="w-full rounded border p-2"
            value={draft.displayName}
            disabled={tag.kind !== "user"}
            onChange={(e) => setDraft({ ...draft, displayName: e.target.value })}
          />
        </label>
        <label className="mt-3 block">
          Shorthand
          <input
            className="w-full rounded border p-2"
            value={draft.shorthand ?? ""}
            onChange={(e) => setDraft({ ...draft, shorthand: e.target.value || null })}
          />
        </label>
        <label className="mt-3 block">
          View
          <input
            className="w-full rounded border p-2"
            value={draft.view ?? ""}
            onChange={(e) => setDraft({ ...draft, view: e.target.value || null })}
          />
        </label>
        <label className="mt-3 block">
          Recipe
          <input
            className="w-full rounded border p-2"
            value={draft.recipe ?? ""}
            onChange={(e) => setDraft({ ...draft, recipe: e.target.value || null })}
          />
        </label>
        <label className="mt-3 flex gap-2">
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })}
          />
          enabled
        </label>
        <div className="mt-5 flex flex-wrap gap-2">
          <button className={primaryButton} onClick={save}>
            Save
          </button>
          <button
            className={secondaryButton}
            onClick={() =>
              alert("Preview/test run uses the bound recipe without saving an artifact.")
            }
          >
            Preview / test run
          </button>
          <button className={secondaryButton} onClick={() => alert("Rebuild queued")}>
            Rebuild
          </button>
          <button
            className={secondaryButton}
            onClick={() => alert("Force rebuild queued (cache ignored)")}
          >
            Force rebuild
          </button>
        </div>
        <div className="mt-6 flex items-center gap-3 border-t border-line pt-5">
          {tag.kind === "user" && (
            <button
              className="inline-flex cursor-pointer items-center justify-center rounded-lg border border-red-300 bg-red-50 px-4 py-2.5 text-[13px] font-semibold text-red-700 hover:bg-red-100 disabled:cursor-wait disabled:opacity-60"
              disabled={deleting}
              onClick={remove}
            >
              {deleting ? "Deleting…" : "Delete tag"}
            </button>
          )}
          <button className={`${secondaryButton} ml-auto`} onClick={close}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
