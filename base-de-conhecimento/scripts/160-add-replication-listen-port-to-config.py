# Add replication listen port to config
# 27/08 20:56

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''pub struct Replicacao {
    pub papel: Papel,''','''pub struct Replicacao {
    pub papel: Papel,
    /// Socket onde o SOURCE serve o fluxo de eventos para as replicas.
    ///
    /// Porta propria, separada da 5000, pelo mesmo motivo da interface web:
    /// quem fala replicacao nao e quem fala consulta, e o firewall precisa
    /// poder tratar as duas de forma diferente. Vazia = usa a porta de dados.
    ///
    /// A volta -- o "ate onde eu ja apliquei" que a replica manda de tempos em
    /// tempos -- vai pela MESMA conexao. Nao ha segundo soquete: quem abriu a
    /// conexao foi a replica, e a resposta volta por onde veio.
    pub escuta: String,''')
s=s.replace('''            papel: Papel::Isolado,
            id_servidor: String::new(),''','''            papel: Papel::Isolado,
            escuta: String::new(),
            id_servidor: String::new(),''')
s=s.replace('''                papel: Papel::de_texto(r.texto_ou("papel", "isolado"))?,
                id_servidor: r.texto_ou("id_servidor", "").to_string(),''','''                papel: Papel::de_texto(r.texto_ou("papel", "isolado"))?,
                escuta: r.texto_ou("escuta", "").trim().to_string(),
                id_servidor: r.texto_ou("id_servidor", "").to_string(),''')

# validacao: a porta da replicacao nao pode colidir com as outras duas
s=s.replace('''        if self.replicacao.papel == Papel::Replica && self.replicacao.origens.is_empty() {''',
'''        if !self.replicacao.escuta.is_empty() {
            let rep = self.replicacao.endereco()?;
            if rep == self.endereco()? {
                return Err(PhxError::Esquema(format!(
                    "replicacao.escuta e bind apontam para o mesmo endereco ({rep})"
                )));
            }
            if self.web.ligado && rep == self.web.endereco()? {
                return Err(PhxError::Esquema(format!(
                    "replicacao.escuta e web.bind apontam para o mesmo endereco ({rep})"
                )));
            }
        }
        if self.replicacao.papel == Papel::Replica && self.replicacao.origens.is_empty() {''')

s=s.replace('''impl Default for Replicacao {''','''impl Replicacao {
    /// Endereco onde o source serve o fluxo. Erro se `escuta` for invalida.
    pub fn endereco(&self) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        self.escuta
            .to_socket_addrs()
            .map_err(|e| {
                PhxError::Esquema(format!(
                    "replicacao.escuta invalida {:?}: {e}",
                    self.escuta
                ))
            })?
            .next()
            .ok_or_else(|| {
                PhxError::Esquema(format!("replicacao.escuta sem endereco: {:?}", self.escuta))
            })
    }
}

impl Default for Replicacao {''')

# para_json mostra a porta
s=s.replace('''            ("papel", Json::texto_de(self.replicacao.papel.nome())),''',
'''            ("papel", Json::texto_de(self.replicacao.papel.nome())),
            (
                "replicacao_escuta",
                Json::texto_de(if self.replicacao.escuta.is_empty() {
                    "(a porta de dados)"
                } else {
                    &self.replicacao.escuta
                }),
            ),''')
open(p,'w').write(s)
