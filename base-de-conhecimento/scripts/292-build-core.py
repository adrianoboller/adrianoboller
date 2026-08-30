# Build core
# 28/08 11:14

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()
v = 'Uuid::de_bytes(leitor.bytes(16)?),'
n = '''Uuid::de_bytes(
                        leitor
                            .bytes(16)?
                            .try_into()
                            .map_err(|_| PhxError::Esquema("id de coluna truncado".into()))?,
                    ),'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
