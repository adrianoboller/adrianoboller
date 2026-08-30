# Add digitos to schema JSON
# 28/08 10:32

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''                        ("max_arquivos", Json::de_u64(pag.max_arquivos as u64)),
                        ("capacidade", Json::de_u64(pag.capacidade())),'''
n = '''                        ("max_arquivos", Json::de_u64(pag.max_arquivos as u64)),
                        ("capacidade", Json::de_u64(pag.capacidade())),
                        // A largura do sufixo vai junto porque sem ela nao da
                        // para escrever o nome do volume: `_1` e `_001` sao
                        // arquivos diferentes.
                        ("digitos", Json::de_u64(pag.digitos as u64)),
                        (
                            "bytes_por_arquivo",
                            Json::de_u64(pag.bytes_por_arquivo),
                        ),'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
