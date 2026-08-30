# Add the recovery counter field
# 27/08 21:44

import re
p='crates/phxsql-core/src/lib.rs'
s=open(p).read()
s=s.replace('pub const EXT_MEMO: &str = "memo";',
'''pub const EXT_MEMO: &str = "memo";
/// Espelho do `.reg`, quando ligado. Ver `volume::Volumes::com_espelho`.
pub const EXT_BKP: &str = "bkp";''')
open(p,'w').write(s)

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
s=s.replace('''    slot_count: u64,
    live_count: u64,
    criado_em: i64,
}''','''    slot_count: u64,
    live_count: u64,
    criado_em: i64,
    /// Leituras salvas pelo espelho nesta sessao.
    recuperados: u64,
}''')
# todo construtor de RegFile ganha o campo
s = re.sub(r'(\n(\s+)criado_em[,:][^\n]*\n)', lambda m: m.group(1) + m.group(2) + 'recuperados: 0,\n', s)
open(p,'w').write(s)
