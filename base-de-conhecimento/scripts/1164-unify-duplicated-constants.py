# Unify duplicated constants
# 29/08 17:51

import pathlib
p = pathlib.Path("crates/phxsql-server/src/idiomas.rs"); t = p.read_text()
velho = '''/// As seis colunas de idioma, na ordem das colunas da tabela.
///
/// Os nomes sao exatamente os nomes das colunas. `Portugues` e o indice 0 de
/// proposito: e o degrau intermediario da resolucao.
pub const IDIOMAS: [&str; 6] = [
    "Portugues",
    "Frances",
    "Ingles",
    "Italiano",
    "Alemao",
    "Espanhol",
];

/// Um database comum: aparece na arvore, abre na grade, obedece a permissao.
pub const DATABASE: &str = "phxsys";
pub const TABELA: &str = "mensagens";'''
novo = '''// As tres constantes moram no `mensagens`, e este modulo as REUSA em vez de
// repetir. Os dois conjuntos de texto (`erro.` do protocolo e `tela.` da
// interface) dividem a MESMA tabela, entao duas listas de idioma seriam duas
// verdades sobre a mesma coisa: quem mudasse uma so deixaria a outra errada
// em silencio, e o esquema da tabela sairia com colunas que um dos lados nao
// enxerga.
pub use crate::mensagens::{DATABASE, IDIOMAS, TABELA};'''
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("constantes unificadas em mensagens.rs")
