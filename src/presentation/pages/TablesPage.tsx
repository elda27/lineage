import { FormEvent, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useAppClient } from "../AppClientContext";
import { TableAsset } from "../../../core/domain/table/TableAsset";

// /tables — テーブル一覧 + 新規作成
export function TablesPage() {
  const app = useAppClient();
  const [tables, setTables] = useState<TableAsset[]>([]);
  const [name, setName] = useState("");
  const [columnsText, setColumnsText] = useState("date, symbol, pnl");
  const [busy, setBusy] = useState(false);

  async function reload() {
    setTables(await app.listTables());
  }

  useEffect(() => {
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function onCreate(e: FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setBusy(true);
    try {
      const columns = columnsText
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean)
        .map((key) => ({ key, label: key, type: "text" as const }));
      await app.createTable({ name: name.trim(), schema: { columns } });
      setName("");
      await reload();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page">
      <h1>テーブル</h1>

      <form className="card form" onSubmit={onCreate}>
        <h2>新しいテーブル</h2>
        <label>
          名前
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="取引履歴"
          />
        </label>
        <label>
          カラム（カンマ区切り）
          <input
            value={columnsText}
            onChange={(e) => setColumnsText(e.target.value)}
            placeholder="date, symbol, pnl"
          />
        </label>
        <button type="submit" disabled={busy}>
          作成
        </button>
      </form>

      <ul className="list">
        {tables.map((t) => (
          <li key={t.id}>
            <Link to={`/tables/${t.id}`}>{t.name}</Link>
            <span className="muted">
              {t.schema.columns.map((c) => c.key).join(" / ")}
            </span>
          </li>
        ))}
        {tables.length === 0 && <li className="muted">まだテーブルがありません</li>}
      </ul>
    </div>
  );
}
