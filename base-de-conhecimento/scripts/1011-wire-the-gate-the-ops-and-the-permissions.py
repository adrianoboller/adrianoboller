# Wire the gate, the ops and the permissions
# 29/08 02:55

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# --- simplifica: quem chega aqui ja passou pelo portao, entao "reservada" == "minha"
alvo = '''    /// Esta tabela esta reservada por ESTA ligacao?
    ///
    /// Enquanto estiver, a janela de durabilidade fica aberta: a carga inteira
    /// vira um `fsync` so, no fim.
    fn em_carga_por_mim(&self, p: &Json, sessao: &Sessao) -> bool {
        let (db, tab) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
        if db.is_empty() || tab.is_empty() {
            return false;
        }
        let k = crate::carga::chave(db, tab);
        match self.cargas.lock() {
            Ok(c) => c
                .todas()
                .iter()
                .any(|r| r.ligacao == sessao.ligacao && crate::carga::chave(&r.database, &r.tabela) == k),
            Err(_) => false,
        }
    }'''
novo = '''    /// Esta tabela esta reservada para carga?
    ///
    /// Nao pergunta POR QUEM de proposito: quem chegou ate aqui ja passou pelo
    /// portao, que so deixa o dono escrever numa tabela reservada. Entao
    /// «reservada» e «reservada por mim» sao a mesma coisa neste ponto, e
    /// perguntar de novo pediria a sessao em quarenta lugares.
    ///
    /// Enquanto estiver, a janela de durabilidade fica aberta: a carga inteira
    /// vira um `fsync` so, no fim.
    fn tabela_reservada(&self, p: &Json) -> bool {
        let (db, tab) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
        if db.is_empty() || tab.is_empty() {
            return false;
        }
        let k = crate::carga::chave(db, tab);
        match self.cargas.lock() {
            Ok(c) => c
                .todas()
                .iter()
                .any(|r| crate::carga::chave(&r.database, &r.tabela) == k),
            Err(_) => false,
        }
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# --- a janela fica aberta durante a carga
alvo = '''        if !self.janela.hora_de_gravar() {
            if let Ok(mut s) = self.sujas.lock() {
                s.insert(chave);
            }
            return Ok(());
        }'''
novo = '''        // Durante uma carga a janela NAO fecha: o `BULKINSERT(false)` e quem
        // sincroniza, uma vez, no fim. E o segundo ganho da reserva -- o
        // primeiro e a exclusividade.
        if self.tabela_reservada(p) || !self.janela.hora_de_gravar() {
            if let Ok(mut s) = self.sujas.lock() {
                s.insert(chave);
            }
            return Ok(());
        }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# --- o portao, logo depois do de permissao
alvo = '''        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }'''
novo = '''        // Portao 4 -- a tabela esta reservada para uma carga de outra ligacao?
        //
        // Depois do de permissao, e nao antes: quem nao pode nem ler a tabela
        // nao precisa descobrir que ela esta em carga, e o recado diz QUEM
        // reservou. `bulkinsert` fica de fora para o comando dizer o proprio
        // recado -- e para o administrador conseguir soltar a reserva alheia.
        if op != "bulkinsert" {
            let (db, tab) = (
                pedido.texto_ou("database", ""),
                pedido.texto_ou("tabela", ""),
            );
            if let Some(recado) = self.barrado_por_carga(db, tab, sessao.ligacao) {
                return (op, true, Err(PhxError::EmCarga(recado)));
            }
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# --- despacho das duas ops
alvo = '''            "tabelas" => self.op_tabelas(p, sessao),'''
novo = '''            "tabelas" => self.op_tabelas(p, sessao),
            "bulkinsert" | "carga" => self.op_bulkinsert(p, sessao),
            "cargas" => self.op_cargas(),'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
