# Add Salto enum and build
# 28/08 19:43

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """/// Resultado de uma verificacao de integridade da tabela."""
novo = """/// Como a pagina por posicao chegou ao inicio dela.
///
/// Sai na resposta do protocolo porque a diferenca entre os dois nao e de
/// estilo: num milhao de linhas sao vinte leituras contra um milhao de
/// passos. Quem esta montando uma tela grande precisa saber qual dos dois
/// esta pagando, e o que fazer com a tabela para pagar o outro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Salto {
    /// Busca binaria pelo `rownum`. O inicio da pagina custa `log2 N`.
    Bissecao,
    /// Andou ate a posicao, uma linha por vez. Sempre certo, sempre caro.
    Passo,
}

impl Salto {
    pub fn nome(self) -> &'static str {
        match self {
            Salto::Bissecao => "bisseccao",
            Salto::Passo => "passo",
        }
    }
}

/// Resultado de uma verificacao de integridade da tabela."""
assert antigo in s
s = s.replace(antigo, novo, 1)
p.write_text(s)

p = pathlib.Path("crates/phxsql-store/src/lib.rs")
s = p.read_text()
s = s.replace("pub use table::{Linha, Lote, Relatorio, Table, Visao};",
              "pub use table::{Linha, Lote, Relatorio, Salto, Table, Visao};")
p.write_text(s)
print("ok")
