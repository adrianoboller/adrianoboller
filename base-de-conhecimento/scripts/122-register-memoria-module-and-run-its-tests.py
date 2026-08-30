# Register memoria module and run its tests
# 27/08 20:22

p='crates/phxsql-store/src/lib.rs'
s=open(p).read()
s=s.replace('pub mod log;','pub mod log;\npub mod memoria;')
s=s.replace('pub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};',
            'pub use log::{Evento, LogFile, Operacao, EXT_LOG, MAGIC_LOG};\npub use memoria::{Consulta, Filtro, Operador, Ordem, Resultado, TabelaMemoria};')
open(p,'w').write(s)
