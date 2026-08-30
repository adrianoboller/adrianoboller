# Polish the message and the duration unit
# 29/08 02:59

import pathlib
p = pathlib.Path("crates/phxsql-server/src/carga.rs")
s = p.read_text()
s = s.replace('''    let quem = if r.usuario.is_empty() {
        format!("a ligacao {}", r.ligacao)
    } else {
        format!("{} (ligacao {})", r.usuario, r.ligacao)
    };
    format!(
        "{}.{} esta reservada para carga por {quem} desde {}, ha {ha}s; \\
         tente de novo quando ela terminar",''',
'''    let quem = if r.usuario.is_empty() {
        format!("pela ligacao {}", r.ligacao)
    } else {
        format!("por {} (ligacao {})", r.usuario, r.ligacao)
    };
    format!(
        "{}.{} esta reservada para carga {quem} desde {}, ha {ha}s; \\
         tente de novo quando ela terminar",''',1)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace('''            (
                "durou_s",
                Json::de_u64(((agora - r.desde_ms).max(0) / 1000) as u64),
            ),''','''            // Em milissegundos, e nao em segundos: uma carga de 300 ms
            // aparecia como "durou 0s", que e um numero que nao ajuda ninguem.
            ("durou_ms", Json::de_u64((agora - r.desde_ms).max(0) as u64)),''',1)
p.write_text(s)
