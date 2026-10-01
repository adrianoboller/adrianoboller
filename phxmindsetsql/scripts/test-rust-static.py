from pathlib import Path
import tomllib,re,sys
root=Path(__file__).resolve().parents[1]
errors=[]
# Parse every Cargo.toml with stdlib TOML parser.
for p in root.rglob('Cargo.toml'):
    try: tomllib.loads(p.read_text())
    except Exception as e: errors.append(f'{p.relative_to(root)} TOML: {e}')
# Workspace members must exist.
ws=tomllib.loads((root/'Cargo.toml').read_text())
for member in ws['workspace']['members']:
    if not (root/member/'Cargo.toml').exists(): errors.append(f'missing workspace member {member}')
# Rust compiler is unavailable in this environment; do not pretend lexical checks replace cargo check.
# Architectural catalog fingerprints.
expect={
 'crates/phx-introspector-postgresql/src/lib.rs':['information_schema.columns','information_schema.referential_constraints','pg_indexes'],
 'crates/phx-introspector-mysql/src/lib.rs':['information_schema.COLUMNS','information_schema.REFERENTIAL_CONSTRAINTS','information_schema.STATISTICS'],
 'crates/phx-introspector-sqlite/src/lib.rs':['sqlite_schema','PRAGMA table_xinfo','PRAGMA foreign_key_list'],
 'crates/phx-introspector-sqlserver/src/lib.rs':['sys.tables','sys.foreign_key_columns','sys.indexes'],
}
for rel,needles in expect.items():
    text=(root/rel).read_text()
    for n in needles:
        if n not in text: errors.append(f'{rel} missing {n}')
if errors:
    print('\n'.join(errors));sys.exit(1)
print('rust-static: OK — Cargo TOML, workspace members and 4 catalog adapters structurally validated')
