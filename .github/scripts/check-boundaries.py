"""Enforce a domain-only core and one-way persistence dependencies (#52)."""
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[2]
errors = []
core = tomllib.loads((ROOT / 'lineage-core/Cargo.toml').read_text(encoding='utf-8'))
allowed = {'anyhow', 'serde', 'serde_json'}
for section in [core, *core.get('target', {}).values()]:
    for kind in ('dependencies', 'build-dependencies', 'dev-dependencies'):
        permitted = allowed | ({'sha2'} if kind == 'dev-dependencies' else set())
        for name in section.get(kind, {}):
            if name not in permitted:
                errors.append(f'core dependency outside domain boundary: {name}')
for path in (ROOT / 'lineage-core/src').rglob('*.rs'):
    relative = path.relative_to(ROOT / 'lineage-core/src')
    if relative != Path('lib.rs') and relative.parts[0] != 'domain':
        errors.append(f'non-domain module in core: {relative}')
    source = path.read_text(encoding='utf-8')
    if re.search(r'\b(?:lineage_store|rusqlite|reqwest|tokio|chrono|uuid)::|std::(?:fs|net|process|time)::|(?:crate|super)::(?:features|infra|ports)::', source):
        errors.append(f'I/O or outer-layer dependency in core: {relative}')
    if re.search(r'\b(?:CaptureContext|CaptureMemo|AUTO_PULL_FOREGROUND_TEXT|InferenceBackend|find_active_tag_token|split_completed_tags|CompleteMetaTag)\b', source):
        errors.append(f'app policy in core: {relative}')
for app in ('minos', 'agentos'):
    for path in (ROOT / app / 'src/domain').rglob('*.rs'):
        production = path.read_text(encoding='utf-8').split('#[cfg(test)]')[0]
        if re.search(r'(?:crate|lineage_core)::(?:features|infra)::|lineage_store::', production):
            errors.append(f'domain depends on application/adapter: {path.relative_to(ROOT)}')
for path in (ROOT / 'lineage-store/src/features').rglob('*.rs'):
    if path.name == 'tests.rs':
        continue
    production = path.read_text(encoding='utf-8').split('#[cfg(test)]')[0]
    if re.search(r'crate::infra::', production):
        errors.append(f'use case depends on concrete adapter: {path.relative_to(ROOT)}')
if errors:
    raise SystemExit('\n'.join(errors))
print('Architecture boundaries: OK')
