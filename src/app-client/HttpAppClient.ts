import { TableAsset } from "../../core/domain/table/TableAsset";
import { Cell } from "../../core/domain/table/Cell";
import {
  LineageRecord,
  VerifyResult,
} from "../../core/domain/lineage/LineageRecord";
import { LineageListFilter } from "../../core/domain/ports/LineageRepository";
import { AppendRowResult } from "../../core/application/AppendRow";
import { TableData } from "../../core/application/GetTableData";
import { RowDetail } from "../../core/application/GetRowDetail";
import { MemoEntry } from "../../core/application/ListMemos";
import { WriteMemoResult } from "../../core/application/WriteMemo";
import {
  ApplicationPort,
  AppendLinkArgs,
  AppendRowArgs,
  CreateTableArgs,
  EditCellsArgs,
  WriteMemoArgs,
} from "./ApplicationPort";

export interface HttpAppClientConfig {
  baseUrl: string;
  workspaceId: string;
  // クラウド接続時の JWT。スタブ運用では未指定可。
  getToken?: () => string | null | Promise<string | null>;
}

// クラウド接続（HTTP, 認証アリ）の composition root。Worker の REST を fetch する。
// 送受信 JSON は use case の input/result 形（camelCase）にそろえ、Worker 側を薄く保つ。
export function createHttpAppClient(config: HttpAppClientConfig): ApplicationPort {
  const { baseUrl, workspaceId } = config;

  async function req<T>(
    method: string,
    path: string,
    body?: unknown
  ): Promise<T> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    const token = config.getToken ? await config.getToken() : null;
    if (token) headers["Authorization"] = `Bearer ${token}`;

    const res = await fetch(`${baseUrl}${path}`, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`HTTP ${res.status} ${method} ${path}: ${text}`);
    }
    return (await res.json()) as T;
  }

  function qs(params: Record<string, string | undefined>): string {
    const entries = Object.entries(params).filter(
      ([, v]) => v !== undefined && v !== ""
    ) as [string, string][];
    const s = new URLSearchParams(entries).toString();
    return s ? `?${s}` : "";
  }

  return {
    workspaceId,

    createTable: (args: CreateTableArgs) =>
      req<TableAsset>("POST", "/api/tables", { workspaceId, ...args }),
    listTables: () =>
      req<TableAsset[]>("GET", `/api/tables${qs({ workspaceId })}`),
    getTableData: (tableId: string) =>
      req<TableData>("GET", `/api/tables/${tableId}`),

    appendRow: (args: AppendRowArgs) =>
      req<AppendRowResult>("POST", `/api/tables/${args.tableId}/rows`, {
        values: args.values,
      }),
    editCells: (args: EditCellsArgs) =>
      req<Cell[]>("PATCH", `/api/rows/${args.rowId}/cells`, {
        values: args.values,
      }),
    getRowDetail: (rowId: string) =>
      req<RowDetail>("GET", `/api/rows/${rowId}`),

    writeMemo: (args: WriteMemoArgs) =>
      req<WriteMemoResult>("POST", `/api/rows/${args.rowId}/memos`, {
        workspaceId,
        title: args.title,
        bodyText: args.bodyText ?? null,
      }),
    listMemos: (rowId: string) =>
      req<MemoEntry[]>("GET", `/api/rows/${rowId}/memos${qs({ workspaceId })}`),

    appendLink: (args: AppendLinkArgs) =>
      req<LineageRecord>("POST", "/api/links", { workspaceId, ...args }),
    listLinks: (filter?: LineageListFilter) =>
      req<LineageRecord[]>(
        "GET",
        `/api/links${qs({
          workspaceId,
          sourceKind: filter?.sourceKind,
          sourceId: filter?.sourceId,
        })}`
      ),
    verifyLineage: () =>
      req<VerifyResult>(
        "GET",
        `/api/workspaces/${workspaceId}/lineage/verify`
      ),
  };
}
