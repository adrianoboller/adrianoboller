# Extract marcada_no_payload helper
# 28/08 19:41

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

# --- 1. helper: marca lida direto do payload, e refatora visao_aceita_payload
antigo = """    fn visao_aceita_payload(&self, payload: &[u8], visao: Visao) -> Result<bool> {
        if visao == Visao::Todas {
            return Ok(true);
        }
        let Some(i) = self.esquema.coluna_softdeleted() else {
            return Ok(visao != Visao::Excluidas);
        };
        // Nulo no bitmap nao acontece nesta coluna, que e obrigatoria -- mas
        // se acontecer, «nao marcada» e a leitura segura.
        let excluida = if payload[i / 8] & (1 << (i % 8)) != 0 {
            false
        } else {
            let off = self.esquema.offset_coluna(i)?;
            payload[off] != 0
        };
        Ok(visao.aceita(excluida))
    }"""
novo = """    fn visao_aceita_payload(&self, payload: &[u8], visao: Visao) -> Result<bool> {
        if visao == Visao::Todas {
            return Ok(true);
        }
        if self.esquema.coluna_softdeleted().is_none() {
            return Ok(visao != Visao::Excluidas);
        }
        Ok(visao.aceita(self.marcada_no_payload(payload)?))
    }

    /// A linha esta marcada como excluida? Le SO o byte da coluna de sistema.
    ///
    /// Falso tambem quando a tabela nao tem a coluna: ali nao ha marca, e
    /// nenhuma linha esta excluida de forma suave.
    fn marcada_no_payload(&self, payload: &[u8]) -> Result<bool> {
        let Some(i) = self.esquema.coluna_softdeleted() else {
            return Ok(false);
        };
        // Nulo no bitmap nao acontece nesta coluna, que e obrigatoria -- mas
        // se acontecer, «nao marcada» e a leitura segura.
        if payload[i / 8] & (1 << (i % 8)) != 0 {
            return Ok(false);
        }
        let off = self.esquema.offset_coluna(i)?;
        Ok(payload[off] != 0)
    }"""
assert antigo in s
s = s.replace(antigo, novo)

p.write_text(s)
print("ok")
