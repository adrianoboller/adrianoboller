# Update replication tests
# 27/08 21:49

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
velho = s[s.index('        if self.config.replicacao.papel != crate::config::Papel::Isolado {'):s.index('        self.subir_web();')]
novo = '''        if self.config.replicacao.papel != crate::config::Papel::Isolado {
            let portas = self.config.replicacao.portas();
            eprintln!(
                "replicacao: papel {} | {}",
                self.config.replicacao.papel.nome(),
                if portas.is_empty() {
                    "envio e retorno pela porta de dados".to_string()
                } else {
                    portas
                        .iter()
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            );
            eprintln!(
                "ATENCAO: o transporte de eventos ainda nao esta implementado \\
                 (ver docs/REPLICACAO.md). As portas sao configuracao, nao servico."
            );
        }

'''
s = s.replace(velho, novo)
open(p,'w').write(s)
