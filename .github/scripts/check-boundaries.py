"""Guard ADR-0005/0006 ownership without requiring desktop dependencies."""
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[2]
errors = []
core = tomllib.loads((ROOT / "lineage-core/Cargo.toml").read_text(encoding="utf-8"))
# New kernel adapters must be reviewed here instead of silently importing app SDKs.
shared_dependencies = {"anyhow", "chrono", "rusqlite", "serde", "serde_json", "sha2", "uuid"}
for name in core.get("dependencies", {}):
    if name not in shared_dependencies:
        errors.append(f"lineage-core dependency outside the shared kernel: {name}")
for app in ("lineage-core", "minos", "agentos"):
    for path in (ROOT / app / "src/domain").rglob("*.rs"):
        production = path.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
        if re.search(r"(?:crate|lineage_core)::(?:features|infra)::", production):
            errors.append(f"domain depends on a use case or adapter: {path.relative_to(ROOT)}")
for path in (ROOT / "lineage-core/src").rglob("*.rs"):
    production = path.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    # These names identify the app-specific policies removed in #49.
    if re.search(r"\b(?:CaptureContext|CaptureMemo|SOURCE_KIND_MINOS|AUTO_PULL_FOREGROUND_TEXT|InferenceBackend)\b", production):
        errors.append(f"app-specific policy in the shared kernel: {path.relative_to(ROOT)}")
for path in (ROOT / "lineage-core/src/features").rglob("*.rs"):
    if path.name == "tests.rs":
        continue
    production = path.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    if re.search(r"crate::infra::", production):
        errors.append(f"shared use case depends on a concrete adapter: {path.relative_to(ROOT)}")
if errors:
    raise SystemExit("\n".join(errors))
print("Architecture boundaries: OK")
