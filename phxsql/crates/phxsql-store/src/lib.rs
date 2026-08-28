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

pub mod backup;
pub mod blob;
pub mod catalogo;
pub mod log;
pub mod memoria;
pub mod ndx;
pub mod reg;
pub mod table;
mod util;
pub mod volume;

pub use blob::{BlobFile, EstatisticaBlob, MAGIC_BIN, MAGIC_MEMO};
pub use catalogo::{qualificar, separar_qualificado, Database, Instancia};
pub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};
pub use memoria::{Consulta, Filtro, Operador, Ordem, Resultado, TabelaMemoria};
pub use ndx::{DescritorIndice, NdxFile, MAGIC_NDX, PAGINA_PADRAO};
pub use reg::{RegFile, MAGIC_REG};
pub use table::{Linha, Relatorio, Table};
pub use volume::Volumes;
