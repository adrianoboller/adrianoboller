# Add replication config tests and honest startup notice
# 27/08 20:56

# o arranque diz o que esta ligado e o que ainda nao transporta evento
p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''        eprintln!("log de acessos: {}", self.config.log_acessos.display());''',
'''        eprintln!("log de acessos: {}", self.config.log_acessos.display());
        if self.config.replicacao.papel != crate::config::Papel::Isolado {
            eprintln!(
                "replicacao: papel {} | escuta {} | ATENCAO: o transporte de eventos \\
                 ainda nao esta implementado (ver docs/REPLICACAO.md)",
                self.config.replicacao.papel.nome(),
                if self.config.replicacao.escuta.is_empty() {
                    "(a porta de dados)"
                } else {
                    &self.config.replicacao.escuta
                }
            );
        }''')
open(p,'w').write(s)
