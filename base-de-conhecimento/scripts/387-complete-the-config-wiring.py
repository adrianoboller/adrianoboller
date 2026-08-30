# Complete the Config wiring
# 28/08 13:48

import pathlib
p = pathlib.Path('crates/phxsql-server/src/config.rs')
linhas = p.read_text().split('\n')
# a linha 678 (1-based) e a do de_json do Config
assert 'inteiro_ou("conexoes_max", padrao.conexoes_max as i64)' in linhas[677], linhas[677]
linhas.insert(676, '''            recursos: Recursos::de_json(
                j,
                j.inteiro_ou("conexoes_max", padrao.conexoes_max as i64).max(1) as usize,
            )?,''')
# o campo conhecido, na linha 562 (agora deslocada)
i = next(i for i, l in enumerate(linhas) if l.strip() == '"conexoes_max",')
linhas.insert(i + 1, '    "recursos",')
# o para_json do Config, na ultima ocorrencia
j = max(i for i, l in enumerate(linhas) if '("conexoes_max", Json::de_u64(self.conexoes_max as u64)),' in l)
linhas.insert(j + 1, '            ("recursos", self.recursos.para_json()),')
s = '\n'.join(linhas).replace('const CAMPOS_CONHECIDOS: [&str; 16]', 'const CAMPOS_CONHECIDOS: [&str; 17]')
p.write_text(s)
print('ok')
