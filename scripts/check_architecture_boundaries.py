#!/usr/bin/env python3
"""Fail CI when the v0.0.9 architecture dependency rules regress.

This check intentionally enforces only stable, high-level boundaries. It does not
try to infer every domain concept or replace code review.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]
VIOLATIONS: list[str] = []


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def require_directory(path: str) -> None:
    candidate = ROOT / path
    if not candidate.is_dir():
        VIOLATIONS.append(f"required architecture directory is missing: {path}")


def forbid_path(path: str) -> None:
    candidate = ROOT / path
    if candidate.exists():
        VIOLATIONS.append(f"legacy architecture path must not exist: {path}")


def source_files(root: str, suffixes: set[str]) -> Iterable[Path]:
    base = ROOT / root
    if not base.exists():
        return []
    return (
        path
        for path in sorted(base.rglob("*"))
        if path.is_file() and path.suffix in suffixes
    )


def rust_production_source(path: Path) -> str:
    """Ignore a conventional trailing cfg(test) module.

    Unit tests may assemble features with concrete adapters. The production
    dependency rule remains enforced above the first cfg(test) declaration.
    Dedicated test-support files are excluded separately.
    """

    text = path.read_text(encoding="utf-8")
    marker = re.search(r"(?m)^\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", text)
    return text[: marker.start()] if marker else text


def check_rust_boundary(
    root: str,
    forbidden_tokens: tuple[str, ...],
    *,
    exclude_names: set[str] | None = None,
) -> None:
    excluded = exclude_names or set()
    for path in source_files(root, {".rs"}):
        if path.name in excluded:
            continue
        for line_number, line in enumerate(
            rust_production_source(path).splitlines(), start=1
        ):
            code = line.split("//", 1)[0]
            for token in forbidden_tokens:
                if token in code:
                    VIOLATIONS.append(
                        f"{relative(path)}:{line_number}: forbidden dependency `{token}`"
                    )


def import_specifiers(path: Path) -> Iterable[tuple[int, str]]:
    """Yield static TypeScript/JavaScript import module specifiers."""

    text = path.read_text(encoding="utf-8")
    pattern = re.compile(
        r"(?:^|\n)\s*(?:import|export)\s+(?:[^;]*?\s+from\s+)?[\"']([^\"']+)[\"']",
        re.MULTILINE,
    )
    for match in pattern.finditer(text):
        line_number = text.count("\n", 0, match.start()) + 1
        yield line_number, match.group(1)


def check_ts_boundary(root: str, forbidden: tuple[re.Pattern[str], ...]) -> None:
    for path in source_files(root, {".ts", ".tsx", ".js", ".jsx"}):
        for line_number, specifier in import_specifiers(path):
            for pattern in forbidden:
                if pattern.search(specifier):
                    VIOLATIONS.append(
                        f"{relative(path)}:{line_number}: forbidden import `{specifier}`"
                    )


for required in (
    "lineage-core/src/domain",
    "lineage-core/src/features",
    "lineage-core/src/infra",
    "fullos/core/domain",
    "fullos/core/features",
    "fullos/core/infra",
    "fullos/src/app",
    "fullos/src/pages",
    "fullos/src/components/base",
    "fullos/src/features",
):
    require_directory(required)

for legacy in (
    "lineage-core/src/app",
    "fullos/core/app",
    "fullos/core/application",
):
    forbid_path(legacy)

# Shared-kernel domain code is independent of use cases and adapters.
check_rust_boundary(
    "lineage-core/src/domain",
    ("crate::features", "crate::infra", "super::features", "super::infra"),
)

# Production feature code consumes injected ports rather than concrete adapters.
check_rust_boundary(
    "lineage-core/src/features",
    ("crate::infra", "super::infra"),
    exclude_names={"test_support.rs"},
)

# FullOS core domain and use cases remain framework/adapter independent.
framework_or_adapter = (
    re.compile(r"(?:^|/)infra(?:/|$)"),
    re.compile(r"(?:^|/)features(?:/|$)"),
    re.compile(r"^react(?:/|$)"),
    re.compile(r"^@tauri-apps(?:/|$)"),
)
check_ts_boundary("fullos/core/domain", framework_or_adapter)
check_ts_boundary(
    "fullos/core/features",
    (
        re.compile(r"(?:^|/)infra(?:/|$)"),
        re.compile(r"^react(?:/|$)"),
        re.compile(r"^@tauri-apps(?:/|$)"),
    ),
)

# Route pages depend on application-facing APIs, not concrete core adapters.
check_ts_boundary(
    "fullos/src/pages",
    (
        re.compile(r"(?:^|/)core/infra(?:/|$)"),
        re.compile(r"(?:^|/)src/infra(?:/|$)"),
    ),
)

if VIOLATIONS:
    print("Architecture boundary check failed:", file=sys.stderr)
    for violation in VIOLATIONS:
        print(f"  - {violation}", file=sys.stderr)
    raise SystemExit(1)

print("Architecture boundaries are valid.")
