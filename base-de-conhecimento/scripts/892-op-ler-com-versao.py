# op_ler com_versao
# 28/08 23:52

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# ---- op_ler: com_versao
alvo = '''    fn op_ler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        match t.ler(rowid)? {
            None => Ok(Json::Nulo),
            Some(linha) => Ok(linha_para_json(&linha, t.esquema())),
        }
    }'''
novo = '''    /// `ler`: uma linha pelo rowid.
    ///
    /// Com `"com_versao": true` a resposta deixa de ser a linha crua e passa
    /// a ser `{linha, rowid, versao}`. A forma muda porque a versao NAO pode
    /// entrar como mais uma chave dentro da linha: ali ela viraria uma coluna
    /// que nao existe no esquema, e todo cliente que percorre as chaves da
    /// resposta comecaria a mandar de volta um campo fantasma. Quem nao pede
    /// continua recebendo exatamente o que sempre recebeu.
    fn op_ler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let com_versao = p.booleano_ou("com_versao", false);
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let linha = match t.ler(rowid)? {
            None => return Ok(Json::Nulo),
            Some(l) => linha_para_json(&l, t.esquema()),
        };
        if !com_versao {
            return Ok(linha);
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("linha", linha),
            ("versao", Json::de_u64(t.versao(rowid)?.unwrap_or(0))),
        ]))
    }'''
assert alvo in s
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("op_ler ok")
