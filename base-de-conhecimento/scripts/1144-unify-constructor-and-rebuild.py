# Unify constructor and rebuild
# 29/08 17:22

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()

# Um construtor so: os `let` das tres frentes juntos, a FORMA do das mensagens
# (que precisa do Arc pronto para semear) e os campos de todas.
velho = """        let servidor = Arc::new(Servidor {
            cluster,
            mensagens,
        let papel = config.replicacao.papel;
        let somente_leitura = config.somente_leitura;
        let posicoes_bidi =
            bidirecional::ler_posicoes(&config.base.join("replicacao-posicoes.json"));
        Ok(Arc::new(Servidor {
            janela:"""
novo = """        let papel = config.replicacao.papel;
        let somente_leitura = config.somente_leitura;
        let posicoes_bidi =
            bidirecional::ler_posicoes(&config.base.join("replicacao-posicoes.json"));
        let servidor = Arc::new(Servidor {
            cluster,
            mensagens,
            papel_vivo: AtomicU8::new(papel_para_u8(papel)),
            somente_leitura_vivo: AtomicBool::new(somente_leitura),
            estado_replicacao: Mutex::new(HashMap::new()),
            toques_bidi: Mutex::new(HashMap::new()),
            posicoes_bidi: Mutex::new(posicoes_bidi),
            janela:"""
assert velho in t
t = t.replace(velho, novo, 1)

# O fim: os campos da replicacao ja subiram; some com a cauda duplicada.
velho2 = """        Ok(servidor)
            papel_vivo: AtomicU8::new(papel_para_u8(papel)),
            somente_leitura_vivo: AtomicBool::new(somente_leitura),
            estado_replicacao: Mutex::new(HashMap::new()),
            toques_bidi: Mutex::new(HashMap::new()),
            posicoes_bidi: Mutex::new(posicoes_bidi),
        }))
    }"""
assert velho2 in t
t = t.replace(velho2, """        Ok(servidor)
    }""", 1)
p.write_text(t); print("construtor unificado")
