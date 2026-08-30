# Add Table::contar
# 28/08 19:44

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    /// Quantas linhas vivas estao marcadas como excluidas. Sai do cabecalho."""
novo = """    /// Quantas linhas a visao enxerga, SEM varrer.
    ///
    /// # Por que agora da para contar
    ///
    /// Contar era o item mais caro da tela: mostrar «pagina 3 de 40» custava
    /// percorrer a tabela inteira, e por isso o `total` tinha saido da
    /// resposta. Com o contador de marcadas no cabecalho a conta fecha em
    /// tempo constante, porque os dois numeros de que ela precisa ja estao
    /// la: `registros` sao os slots ocupados, `marcadas` sao os ocupados que
    /// estao escondidos, e a diferenca e o que a lista mostra.
    ///
    /// Numa tabela sem a coluna de sistema nao ha marca: `Excluidas` da zero
    /// e as outras duas dao o total.
    pub fn contar(&self, visao: Visao) -> u64 {
        if self.esquema.coluna_softdeleted().is_none() {
            return match visao {
                Visao::Excluidas => 0,
                _ => self.reg.registros(),
            };
        }
        match visao {
            Visao::Ativas => self.reg.registros().saturating_sub(self.reg.marcadas()),
            Visao::Excluidas => self.reg.marcadas(),
            Visao::Todas => self.reg.registros(),
        }
    }

    /// Quantas linhas vivas estao marcadas como excluidas. Sai do cabecalho."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
