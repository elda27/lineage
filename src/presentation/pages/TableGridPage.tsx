import { FormEvent, useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useAppClient } from "../AppClientContext";
import { TableData } from "../../../core/application/GetTableData";

// /tables/:id — Excel 風グリッド。セル編集・行追加。
export function TableGridPage() {
  const app = useAppClient();
  const { id = "" } = useParams();
  const [data, setData] = useState<TableData | null>(null);
  const [draft, setDraft] = useState<Record<string, string>>({});

  const reload = useCallback(async () => {
    setData(await app.getTableData(id));
  }, [app, id]);

  useEffect(() => {
    void reload();
  }, [reload]);

  if (!data) return <div className="state">読み込み中…</div>;
  const columns = data.table.schema.columns;

  function cellValue(rowId: string, key: string): string {
    const row = data!.rows.find((r) => r.row.id === rowId);
    return row?.cells.find((c) => c.columnKey === key)?.rawValue ?? "";
  }

  async function saveCell(rowId: string, key: string, value: string) {
    if (value === cellValue(rowId, key)) return;
    await app.editCells({ rowId, values: { [key]: value } });
    await reload();
  }

  async function addRow(e: FormEvent) {
    e.preventDefault();
    await app.appendRow({ tableId: id, values: draft });
    setDraft({});
    await reload();
  }

  return (
    <div className="page">
      <p>
        <Link to="/tables">← テーブル一覧</Link>
      </p>
      <h1>{data.table.name}</h1>

      <table className="grid">
        <thead>
          <tr>
            <th></th>
            {columns.map((c) => (
              <th key={c.key}>{c.label}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.rows.map(({ row }) => (
            <tr key={row.id}>
              <td className="rownum">
                <Link to={`/rows/${row.id}`}>#{row.rowIndex + 1}</Link>
              </td>
              {columns.map((c) => (
                <td key={c.key}>
                  <input
                    defaultValue={cellValue(row.id, c.key)}
                    onBlur={(e) => saveCell(row.id, c.key, e.target.value)}
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <td className="rownum">＋</td>
            {columns.map((c) => (
              <td key={c.key}>
                <input
                  value={draft[c.key] ?? ""}
                  onChange={(e) =>
                    setDraft((d) => ({ ...d, [c.key]: e.target.value }))
                  }
                  placeholder={c.label}
                />
              </td>
            ))}
          </tr>
        </tfoot>
      </table>

      <form onSubmit={addRow}>
        <button type="submit">行を追加</button>
      </form>
    </div>
  );
}
