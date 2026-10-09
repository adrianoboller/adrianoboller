//! O `config.json` do agente PhxClaw: o catalogo unico das chaves (`catalogo`), a carga
//! com precedencia e origem (`carga`) e os artefatos gerados dele (`gerar`).
//!
//! Mora no config-runtime, e nao num crate novo, porque a gravacao com revisao, o
//! historico e a troca atomica ja estavam aqui (`gravar_revisado`): o `UnifiedConfig` de
//! antes e o `config.json` do agente gravam pelo mesmo caminho.

pub mod carga;
pub mod catalogo;
pub mod gerar;

pub use carga::{carregar, definir, Configuracao, Efetivo, Erro, Origem, Recusa};
pub use catalogo::{
    catalogo, por_chave, por_variavel, Alcance, Chave, Natureza, Referencia, Tipo, SO_DO_OPERADOR,
};
