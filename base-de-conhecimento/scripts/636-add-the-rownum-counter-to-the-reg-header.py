# Add the rownum counter to the .reg header
# 28/08 18:26

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()

s=s.replace('''const VERSAO: u16 = 2;''','''/// Versao do `.reg`.
///
/// A 3 acrescentou `proximo_rownum` nos bytes 92..100, que estavam reservados.
/// Arquivo da 2 nao abre nesta versao -- o contador nao existiria, e comecar do
/// zero num arquivo que ja tem linhas faria a coluna repetir numero.
const VERSAO: u16 = 3;''',1)

s=s.replace('''    proxima_sequencia: u64,''','''    proxima_sequencia: u64,
    /// Proximo valor da coluna de sistema `rownum`. So o volume 1 manda.
    proximo_rownum: u64,''',1)

s=s.replace('''            proxima_sequencia: 0,''','''            proxima_sequencia: 0,
            proximo_rownum: 1,''',1)

s=s.replace('''        let proxima_sequencia = c.u64(36);''','''        let proxima_sequencia = c.u64(36);
        // Zero num arquivo ja gravado seria "nunca usado", e o primeiro
        // rownum sairia 1 por cima do que existe. O contador comeca em 1.
        let proximo_rownum = c.u64(92).max(1);''',1)

s=s.replace('''            proxima_sequencia,''','''            proxima_sequencia,
            proximo_rownum,''',1)

s=s.replace('''            por_u64(&mut buf, 36, self.proxima_sequencia);
        }''','''            por_u64(&mut buf, 36, self.proxima_sequencia);
            por_u64(&mut buf, 92, self.proximo_rownum);
        }''',1)
io.open(p,'w',encoding='utf-8').write(s)
