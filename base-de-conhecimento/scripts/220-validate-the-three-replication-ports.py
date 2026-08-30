# Validate the three replication ports
# 27/08 21:48

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
# validacao: cada porta de replicacao contra a de dados, a da web e a outra
velho = s[s.index('        if !self.replicacao.escuta.is_empty() {'):s.index('        if self.replicacao.papel == Papel::Replica')]
novo = '''        // Cada porta de replicacao contra a de dados, a da web e a outra.
        // Duas portas no mesmo endereco nao sobem, e descobrir isso no
        // arranque e melhor do que descobrir com uma delas calada.
        let mut ocupadas = vec![("bind", self.endereco()?)];
        if self.web.ligado {
            ocupadas.push(("web.bind", self.web.endereco()?));
        }
        for (rotulo, texto) in self.replicacao.portas() {
            let alvo = Replicacao::resolver(rotulo, texto)?;
            if let Some((quem, _)) = ocupadas.iter().find(|(_, e)| *e == alvo) {
                return Err(PhxError::Esquema(format!(
                    "replicacao.{rotulo} e {quem} apontam para o mesmo endereco ({alvo})"
                )));
            }
            ocupadas.push((rotulo, alvo));
        }
'''
s = s.replace(velho, novo)
s = s.replace('''            (
                "replicacao_escuta",
                Json::texto_de(if self.replicacao.escuta.is_empty() {
                    "(a porta de dados)"
                } else {
                    &self.replicacao.escuta
                }),
            ),''','''            (
                "replicacao_portas",
                Json::Objeto(
                    self.replicacao
                        .portas()
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), Json::texto_de(v)))
                        .collect(),
                ),
            ),''')
open(p,'w').write(s)
