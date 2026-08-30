# Guard atualizar/excluir/restaurar
# 28/08 23:52

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# ---- op_atualizar
alvo = '''        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let mut linha = json_para_linha(&valores_json, t.esquema())?;'''
novo = '''        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let mut linha = json_para_linha(&valores_json, t.esquema())?;'''
assert s.count(alvo) == 1, s.count(alvo)
s = s.replace(alvo, novo, 1)

alvo = '''        t.atualizar(rowid, &linha)?;
        self.gravar_de_verdade(&mut t, p)?;
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))
    }'''
novo = '''        t.atualizar(rowid, &linha)?;
        self.gravar_de_verdade(&mut t, p)?;
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        // A versao nova volta na resposta: quem grava duas vezes seguidas
        // continua protegido sem precisar reler a linha inteira no meio.
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("versao", Json::de_u64(t.versao(rowid)?.unwrap_or(0))),
        ]))
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ---- op_excluir
alvo = '''        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let tem_marca = t.esquema().coluna_softdeleted().is_some();'''
novo = '''        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let tem_marca = t.esquema().coluna_softdeleted().is_some();'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ---- op_restaurar
alvo = '''        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let voltou = t.restaurar(rowid, &motivo)?;'''
novo = '''        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let voltou = t.restaurar(rowid, &motivo)?;'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
