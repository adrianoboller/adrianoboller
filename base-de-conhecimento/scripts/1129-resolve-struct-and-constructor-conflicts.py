# Resolve struct and constructor conflicts
# 29/08 17:14

import pathlib
p = pathlib.Path("phxsql/crates/phxsql-server/src/servidor.rs")
t = p.read_text()

# 1) Campos do struct: as duas frentes acrescentaram campo proprio -- ficam os dois.
t = t.replace("""<<<<<<< HEAD
    /// O estado vivo do cluster -- `None` quando o `config.json` nao traz o
    /// bloco `cluster`, e ai NADA disto existe: nenhuma thread, nenhum portao.
    cluster: Option<Arc<crate::cluster::EstadoCluster>>,
=======
    /// As mensagens que o servidor devolve, resolvidas pela tabela
    /// `phxsys.mensagens` quando ela existe. Ver `mensagens.rs`.
    mensagens: Mensagens,
>>>>>>> worktree-agent-af10b6f797860b6a7
""", """    /// O estado vivo do cluster -- `None` quando o `config.json` nao traz o
    /// bloco `cluster`, e ai NADA disto existe: nenhuma thread, nenhum portao.
    cluster: Option<Arc<crate::cluster::EstadoCluster>>,
    /// As mensagens que o servidor devolve, resolvidas pela tabela
    /// `phxsys.mensagens` quando ela existe. Ver `mensagens.rs`.
    mensagens: Mensagens,
""")

# 2) Arranque: a frente das mensagens trocou a FORMA do retorno (precisa do
#    Arc pronto para semear). Fica a forma dela, com as pecas das outras duas.
t = t.replace("""<<<<<<< HEAD
        let cluster = config.cluster.clone().map(|c| {
            Arc::new(crate::cluster::EstadoCluster::novo(
                c,
                &config.base,
                config.replicacao.papel,
            ))
        });
        let rotinas = crate::rotinas::Rotinas::carregar(&config.base)?;
        let ha_gatilhos = AtomicBool::new(rotinas.ha_gatilhos());
        Ok(Arc::new(Servidor {
            cluster,
=======
        let mensagens = Mensagens::nova(&config.idioma, &config.base);
        let servidor = Arc::new(Servidor {
            mensagens,
>>>>>>> worktree-agent-af10b6f797860b6a7
""", """        let cluster = config.cluster.clone().map(|c| {
            Arc::new(crate::cluster::EstadoCluster::novo(
                c,
                &config.base,
                config.replicacao.papel,
            ))
        });
        let rotinas = crate::rotinas::Rotinas::carregar(&config.base)?;
        let ha_gatilhos = AtomicBool::new(rotinas.ha_gatilhos());
        let mensagens = Mensagens::nova(&config.idioma, &config.base);
        let servidor = Arc::new(Servidor {
            cluster,
            mensagens,
""")

# 3) Fim do arranque: o `Ok(servidor)` da frente das mensagens engloba os
#    campos das rotinas -- semear exige o Arc ja montado.
t = t.replace("""<<<<<<< HEAD
            rotinas: Mutex::new(rotinas),
            ha_gatilhos,
        }))
=======
        });""", """            rotinas: Mutex::new(rotinas),
            ha_gatilhos,
        });""")
t = t.replace("""        Ok(servidor)
>>>>>>> worktree-agent-af10b6f797860b6a7
""", """        Ok(servidor)
""")
p.write_text(t)
print("marcas restantes no arranque:", t.count("<<<<<<<"))
