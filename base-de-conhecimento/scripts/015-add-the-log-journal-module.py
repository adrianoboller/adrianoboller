# Add the .log journal module
# 27/08 18:25

p='crates/phxsql-store/src/util.rs'
s=open(p).read()
s=s.replace('''pub fn ler_exato''','''/// Instante atual em milissegundos desde a epoca Unix.
pub fn agora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn ler_exato''')
open(p,'w').write(s)
