# Fix remaining call sites
# 28/08 11:20

import pathlib
for arq, v, n in [
 ('crates/phxsql-store/src/catalogo.rs',
  '.com_paginacao(phxsql_core::paginacao::Paginacao::nova(2, 99).unwrap());',
  '.com_paginacao(phxsql_core::paginacao::Paginacao::nova(2, 99).unwrap())\n            .unwrap();'),
 ('crates/phxsql-store/src/reg.rs',
  'esquema().com_paginacao(Paginacao::nova(registros, arquivos).unwrap())',
  'esquema()\n            .com_paginacao(Paginacao::nova(registros, arquivos).unwrap())\n            .unwrap()'),
]:
    p = pathlib.Path(arq); s = p.read_text()
    assert s.count(v) == 1, arq
    p.write_text(s.replace(v, n))
print('ok')
