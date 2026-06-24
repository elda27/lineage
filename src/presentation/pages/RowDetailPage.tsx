import { FormEvent, useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useAppClient } from "../AppClientContext";
import { RowDetail } from "../../../core/application/GetRowDetail";
import { MemoEntry } from "../../../core/application/ListMemos";
import { LineageRecord, VerifyResult } from "../../../core/domain/lineage/LineageRecord";

// /rows/:id — 行詳細（セル / メモ / その行の lineage / 検証）。スマホ主画面。
export function RowDetailPage() {
  const app = useAppClient();
  const { id = "" } = useParams();
  const [detail, setDetail] = useState<RowDetail | null>(null);
  const [memos, setMemos] = useState<MemoEntry[]>([]);
  const [links, setLinks] = useState<LineageRecord[]>([]);
  const [verify, setVerify] = useState<VerifyResult | null>(null);

  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    setDetail(await app.getRowDetail(id));
    setMemos(await app.listMemos(id));
    setLinks(await app.listLinks({ sourceKind: "row", sourceId: id }));
  }, [app, id]);

  useEffect(() => {
    void reload();
  }, [reload]);

  async function onSaveMemo(e: FormEvent) {
    e.preventDefault();
    if (!body.trim() && !title.trim()) return;
    setBusy(true);
    try {
      await app.writeMemo({
        rowId: id,
        title: title.trim() || "memo",
        bodyText: body,
      });
      setTitle("");
      setBody("");
      await reload();
    } finally {
      setBusy(false);
    }
  }

  async function onVerify() {
    setVerify(await app.verifyLineage());
  }

  if (!detail) return <div className="state">読み込み中…</div>;

  return (
    <div className="page">
      <p>
        {detail.table ? (
          <Link to={`/tables/${detail.table.id}`}>← {detail.table.name}</Link>
        ) : (
          <Link to="/tables">← テーブル一覧</Link>
        )}
      </p>
      <h1>行 #{detail.row.rowIndex + 1}</h1>

      <section className="card">
        <h2>値</h2>
        <dl className="kv">
          {detail.cells.map((c) => (
            <div key={c.id}>
              <dt>{c.columnKey}</dt>
              <dd>{c.rawValue ?? "—"}</dd>
            </div>
          ))}
          {detail.cells.length === 0 && <span className="muted">セルなし</span>}
        </dl>
      </section>

      <section className="card form">
        <h2>記録を残す</h2>
        <form onSubmit={onSaveMemo}>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="タイトル（例: SOXL損切り）"
          />
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder="気付き・判断・改善案…"
            rows={4}
          />
          <button type="submit" disabled={busy}>
            保存
          </button>
        </form>
      </section>

      <section className="card">
        <h2>記録履歴</h2>
        <ul className="list">
          {memos.map((m) => (
            <li key={m.document.id}>
              <strong>{m.document.title}</strong>
              <div>{m.document.bodyText}</div>
              <span className="muted">{m.document.createdAt}</span>
            </li>
          ))}
          {memos.length === 0 && <li className="muted">まだ記録がありません</li>}
        </ul>
      </section>

      <section className="card">
        <h2>Lineage（この行の関係）</h2>
        <ul className="list">
          {links.map((l) => (
            <li key={l.id}>
              <span className="muted">#{l.seq}</span> {l.relationType} →{" "}
              {l.targetKind} <code className="hash">{l.contentHash.slice(0, 12)}…</code>
            </li>
          ))}
          {links.length === 0 && <li className="muted">関係なし</li>}
        </ul>
        <button onClick={onVerify}>真正性を検証</button>
        {verify && (
          <p className={verify.ok ? "ok" : "ng"}>
            {verify.ok
              ? `OK（${verify.length} 件の鎖が整合）`
              : `改ざん検知: seq ${verify.brokenAt} で不整合`}
          </p>
        )}
      </section>
    </div>
  );
}
