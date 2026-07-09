#!/bin/sh
# Stop hook: minos/ に変更があるのに docs/minos/ が未更新ならリマインドを出す。
# 仕様: .claude/skills/minos-dev/SKILL.md の「ドキュメント更新ルール」を参照。

# 2周目(既にこのフックで一度止めた後)はブロックしない。無限ループ防止。
stdin=$(cat)
case "$stdin" in
  *'"stop_hook_active":true'*|*'"stop_hook_active": true'*) exit 0 ;;
esac

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

# 対象: 未コミットの変更(staged / unstaged / untracked)と、upstream 未プッシュのコミット
changed=$(git status --porcelain 2>/dev/null | cut -c4-)
if upstream=$(git rev-parse --abbrev-ref '@{upstream}' 2>/dev/null); then
  changed="$changed
$(git diff --name-only "$upstream"...HEAD 2>/dev/null)"
fi

minos_changed=$(printf '%s\n' "$changed" | grep '^minos/' | grep -v '^minos/Cargo\.lock$' || true)
docs_changed=$(printf '%s\n' "$changed" | grep '^docs/minos/' || true)

if [ -n "$minos_changed" ] && [ -z "$docs_changed" ]; then
  cat >&2 <<'EOF'
minos/ に変更がありますが docs/minos/ が更新されていません。
.claude/skills/minos-dev/SKILL.md の「ドキュメント更新ルール」の対応表に従い、
docs/minos/(CONCEPT.md / USECASE.md / DESIGN.md)の更新要否を確認してください。
更新が不要な変更(リファクタ・依存更新等)であれば、その旨をユーザーへの報告に明記して続行してください。
EOF
  exit 2
fi

exit 0
