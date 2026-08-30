# Wire Recursos into Config
# 28/08 13:47

import pathlib
p = pathlib.Path('crates/phxsql-server/src/config.rs')
s = p.read_text()
# o Default
v = '''            conexoes_max: 64,'''
assert s.count(v) == 1
s = s.replace(v, '''            conexoes_max: 64,
            recursos: Recursos::default(),''')
# o de_json
v = '''            conexoes_max: j
                .inteiro_ou("conexoes_max", padrao.conexoes_max as i64)'''
assert s.count(v) == 1
s = s.replace(v, '''            recursos: Recursos::de_json(
                j,
                j.inteiro_ou("conexoes_max", padrao.conexoes_max as i64).max(1) as usize,
            )?,
            conexoes_max: j
                .inteiro_ou("conexoes_max", padrao.conexoes_max as i64)''')
# o campo conhecido
v = '''    "conexoes_max",'''
assert s.count(v) == 1
s = s.replace(v, '''    "conexoes_max",
    "recursos",''')
s = s.replace('const CAMPOS_CONHECIDOS: [&str; 16]', 'const CAMPOS_CONHECIDOS: [&str; 17]')
# e o para_json
v = '''            ("conexoes_max", Json::de_u64(self.conexoes_max as u64)),'''
assert s.count(v) == 1
s = s.replace(v, '''            ("conexoes_max", Json::de_u64(self.conexoes_max as u64)),
            ("recursos", self.recursos.para_json()),''')
p.write_text(s)
print('ok')
