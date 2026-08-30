# Maintain marcadas across mutations
# 28/08 19:41

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

# --- inserir: contar a linha que ja nasce marcada
antigo = """        let payload = self.montar_payload(valores)?;
        let ponteiros = self.ponteiros(&payload)?;
        let rowid = match self.balde_da_linha(valores)? {"""
novo = """        let payload = self.montar_payload(valores)?;
        let ponteiros = self.ponteiros(&payload)?;
        // Linha que ja nasce marcada existe: a importacao traz o campo, e a
        // restauracao de uma lixeira tambem. O contador tem de saber.
        let nasce_marcada = self.marcada_no_payload(&payload)?;
        let rowid = match self.balde_da_linha(valores)? {"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """                let _ = self.reg.excluir(rowid);
                let _ = self.liberar_externos(&ponteiros);
                return Err(e);
            }
        }
        self.log.registrar(Operacao::Inclusao, rowid, 1)?;
        Ok(rowid)
    }"""
novo = """                let _ = self.reg.excluir(rowid);
                let _ = self.liberar_externos(&ponteiros);
                return Err(e);
            }
        }
        if nasce_marcada {
            self.reg.mudar_marcadas(1);
        }
        self.log.registrar(Operacao::Inclusao, rowid, 1)?;
        Ok(rowid)
    }"""
assert antigo in s
s = s.replace(antigo, novo)

# --- atualizar: a marca pode mudar quando vem escrita nos valores
antigo = """        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        let versao = self.reg.atualizar(rowid, &payload)?;"""
novo = """        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        // `completar` herda a marca quando ela nao vem nos valores, mas quem
        // manda a coluna escrita pode virar o valor por aqui.
        let delta = i64::from(self.marcada_no_payload(&payload)?)
            - i64::from(self.marcada_no_payload(&antigo)?);
        let versao = self.reg.atualizar(rowid, &payload)?;
        if delta != 0 {
            self.reg.mudar_marcadas(delta);
        }"""
assert antigo in s
s = s.replace(antigo, novo)

# --- marcar: o funil da exclusao suave
antigo = """        let versao = self.reg.atualizar(rowid, &payload)?;
        for (j, (a, b)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {"""
novo = """        let versao = self.reg.atualizar(rowid, &payload)?;
        self.reg.mudar_marcadas(if valor { 1 } else { -1 });
        for (j, (a, b)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {"""
assert antigo in s
s = s.replace(antigo, novo)

# --- excluir_de_vez: a linha marcada que sai do .reg deixa de contar
antigo = """        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            self.motivos
                .registrar(Tipo::Fisica, rowid, motivo, &identidade)?;"""
novo = """        let estava_marcada = self.marcada_no_payload(&payload)?;
        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            if estava_marcada {
                self.reg.mudar_marcadas(-1);
            }
            self.motivos
                .registrar(Tipo::Fisica, rowid, motivo, &identidade)?;"""
assert antigo in s
s = s.replace(antigo, novo)

p.write_text(s)
print("ok")
