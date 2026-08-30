# Clean warnings
# 28/08 11:25

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
s = s.replace('fn op_sistabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {',
              'fn op_sistabelas(&self, p: &Json) -> Result<Json> {')
s = s.replace('fn op_siscolunas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {',
              'fn op_siscolunas(&self, p: &Json) -> Result<Json> {')
s = s.replace('"sistabelas" | "systables" => self.op_sistabelas(p, sessao),',
              '"sistabelas" | "systables" => self.op_sistabelas(p),')
s = s.replace('"siscolunas" | "syscolumns" => self.op_siscolunas(p, sessao),',
              '"siscolunas" | "syscolumns" => self.op_siscolunas(p),')
s = s.replace('            let mut t = match db.abrir_qualificada(&nome) {', '            let t = match db.abrir_qualificada(&nome) {')
s = s.replace('''                ]));
                    continue;''', '''                ]));
                    continue;''')
s = s.replace('        let _ = sessao;\n        Ok(Json::objeto(vec![\n            ("database", Json::texto_de(database)),\n            ("total", Json::de_u64(linhas.len() as u64)),\n            ("colunas", Json::Lista(linhas)),',
              '        Ok(Json::objeto(vec![\n            ("database", Json::texto_de(database)),\n            ("total", Json::de_u64(linhas.len() as u64)),\n            ("colunas", Json::Lista(linhas)),')
p.write_text(s)
