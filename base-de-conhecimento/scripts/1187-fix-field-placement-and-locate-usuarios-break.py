# Fix field placement and locate usuarios break
# 29/08 18:40

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
# O campo foi colado depois do `Ok(servidor)`; move para dentro do literal.
t = t.replace("""        Ok(servidor)            telemetria: Arc::new(crate::telemetria::Telemetria::default()),
""", """        Ok(servidor)
""", 1)
t = t.replace("""            max_linhas_vivo: AtomicU64::new(max_linhas),
            espelho_vivo: AtomicBool::new(espelho),
""", """            max_linhas_vivo: AtomicU64::new(max_linhas),
            espelho_vivo: AtomicBool::new(espelho),
            telemetria: Arc::new(crate::telemetria::Telemetria::default()),
""", 1)
p.write_text(t); print("campo telemetria no lugar certo")
