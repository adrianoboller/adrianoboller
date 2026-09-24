//! # phxsql-store
//!
//! O motor de armazenamento do PhxSql, no modelo de quatro arquivos do HFSQL(R).
//!
//! ```text
//! cadastroClientes.reg   registros na ordem de digitacao (heap de slots fixos)
//! cadastroClientes.ndx   indices em B+tree, todos no mesmo arquivo
//! cadastroClientes.bin   binarios (imagens, anexos, documentos)
//! cadastroClientes.memo  textos longos
//!
//! .reg + .ndx + .bin + .memo = cadastroClientes
//! ```
//!
//! Cada arquivo se abre sozinho: tem assinatura propria, versao de formato e
//! CRC. O esquema mora dentro do `.reg`, entao o quarteto e auto-descritivo.

#[cfg(test)]
mod apoio_teste;
pub mod backup;
pub mod blob;
pub mod catalogo;
pub mod cofre;
pub mod conferidor_fsync;
pub mod congelamento;
pub mod diario;
pub mod fts;
pub mod integridade;
pub mod ledger;
pub mod leitura;
pub mod lixeira;
pub mod log;
pub mod memoria;
pub mod motivo;
pub mod ndx;
pub mod no;
pub mod pag;
pub mod reg;
pub mod restaurar;
pub mod sincronia;
pub mod table;
pub mod trilha;
mod util;
pub mod volume;

/// O motor da permissao dos arquivos do banco (pedido 542): 0600 em todo
/// arquivo e 0700 em todo diretorio que o banco cria, e o alerta da base
/// antiga. Mora em `util`; o servidor o alcanca por aqui para os arquivos
/// que ELE cria (visoes, rotinas, logs, a lapide do backup) nascerem pela
/// mesma decisao, e nao por uma segunda copia dela.
pub mod permissao {
    pub use crate::util::{
        copiar_do_banco, criar_diretorio_do_banco, escrever_do_banco, opcoes_do_banco,
        permissao_larga, recriar_do_banco, MODO_DO_ARQUIVO, MODO_DO_DIRETORIO,
    };
}

pub use blob::{BlobFile, EstatisticaBlob, MAGIC_BIN, MAGIC_MEMO};
pub use catalogo::{qualificar, separar_qualificado, Aberta, Database, Instancia, Raiz};
pub use ledger::{
    hash_do_bloco, preparar_bloco, topo_da_cadeia, verificar_cadeia, Prova, Verificacao,
};
pub use leitura::{Legivel, TabelaLeitura};
pub use lixeira::{Descartada, LixeiraFile, EXT_TRASH, MAGIC_LIXEIRA};
pub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};
pub use memoria::{Consulta, Filtro, Operador, Ordem, Resultado, TabelaMemoria};
pub use motivo::{Motivo, MotivoFile, EXT_REASON, MAGIC_MOTIVO};
pub use ndx::{DescritorIndice, NdxFile, MAGIC_NDX, PAGINA_PADRAO};
pub use pag::EXT_PAG;
pub use reg::{RegFile, MAGIC_REG};
pub use table::{Linha, Lote, PlanoV10, Relatorio, Salto, SemEscrever, Table, Visao};
pub use trilha::{TrilhaFile, EXT_LGPD, MAGIC_TRILHA};
pub use volume::Volumes;
