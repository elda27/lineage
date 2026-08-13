/**
 * エージェント CLI へ配る skill の本文と、その版。
 *
 * 内容を変えたら必ず `LINEAGE_SKILL_VERSION` を上げる。上げ忘れると、既に配った
 * 環境が古い本文のまま「最新」と判定されて更新されない。
 *
 * 版はアプリの VERSION とは別に持つ。skill の中身はアプリの更新とは無関係に
 * 変わる（agentos のコマンドが増えたときだけ変わる）ので、同じ番号にすると
 * 「アプリを上げただけで skill も配り直す」ことになってしまう。
 */

import { SKILL_NAME } from "./AgentSkill";

/** 配布する skill の版。本文を変えたらここを上げる。 */
export const LINEAGE_SKILL_VERSION = "1.0.0";

/** version.json に書く「誰が置いたか」。 */
export const SKILL_INSTALLED_BY = "fullos";

/**
 * skill 本文を組み立てる。
 *
 * `agentos` の場所は環境ごとに違う（配布時は fullos のリソース、開発時は target/）。
 * エージェントに探させると毎回失敗しうるので、判明している絶対パスを本文へ埋め込む。
 */
export function renderLineageSkill(agentosPath: string): string {
  return `---
name: ${SKILL_NAME}
description: Work with the user's Lineage records (memos captured by minos) through the bundled agentos CLI. Use when the user asks about their own captured notes/memos, wants to list or run a Lineage automation rule, or needs the Lineage hash-chain verified.
---

# ${SKILL_NAME}

利用者の記録（minos で入力したメモ）と、その記録に紐づく自動化を扱うための手順。

記録は SQLite に入っており、\`agentos\` という CUI から読める。
このファイルは fullos が自動で配置しているので、\`agentos\` の場所は既に判明している。

\`\`\`
${agentosPath}
\`\`\`

以降このパスを \`agentos\` と書く。空白を含むので、実行するときは必ず引用符で囲む。

## できること

| やりたいこと | コマンド |
| --- | --- |
| 登録されている自動化ルールを見る | \`agentos rules --json\` |
| ある記録に当てられるルールを探す | \`agentos match --memo <id> --json\` |
| ルールを1件実行する | \`agentos run --rule <id> --memo <id> --json\` |
| 送るプロンプトだけ組み立てる | \`agentos render --rule <id> --memo <id>\` |
| 外部で得た結果を記録として確定する | \`agentos record --rule <id> --memo <id> --result-file <path\\|->\` |
| hash-chain を検証する | \`agentos verify --json\` |

共通の引数:

- \`--json\` … 機械が読む形で stdout に出す。解析するときは必ず付ける。
- \`--db <path>\` … 対象の DB。既定は minos と同じ場所なので普段は要らない。
- \`--workspace <id>\` … 既定は \`local\`。

出力の約束:

- 結果は stdout、進行状況は stderr。解析するのは stdout だけでよい。
- \`run\` と \`record\` は「自動化が成功しなかった」とき終了コード 2 を返す。
  実行そのものの失敗（1）とは区別すること。2 のときも結果 JSON は stdout に出ている。

## 守ること

1. **記録の台帳（\`links\`）へ直接書かない。** 記録の追記は hash-chain を伴い、
   その組み立ては \`agentos\` と \`minos\` に一本化されている。SQLite を開いて
   自分で INSERT すると鎖が切れ、\`agentos verify\` が落ちる。
   記録を残したいときは必ず \`agentos record\` を使う。
2. **既存の記録を書き換えない・消さない。** 台帳は append-only。
   訂正が要るなら新しい記録として足す。
3. **API キーを引数で渡さない。** コマンドライン引数は他プロセスから見える。
   資格情報は \`agentos credential set --provider <name>\` が標準入力から読む。
4. **ブラウザ方式のルールはここから実行できない。** \`agentos\` は画面を持たないので
   \`browser_backend_unsupported\` で断られる。その場合は fullos の自動化画面から
   実行するよう利用者に伝える。

## 進め方

1. まず \`agentos rules --json\` で、いま何が登録されているかを確かめる。
2. 記録に対して何かする前に \`agentos match --memo <id> --json\` で、
   当てられるルールがあるかを見る。無いのに \`run\` を叩いても失敗するだけ。
3. 実行したら終了コードを見る。2 なら stdout の JSON に理由が入っている。
4. 台帳に書く操作をしたあとは \`agentos verify --json\` で鎖が繋がっていることを確かめる。
`;
}
