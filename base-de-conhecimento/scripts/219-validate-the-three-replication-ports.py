# Validate the three replication ports
# 27/08 21:48

p='crates/phxsql-server/src/config.rs'
linhas=open(p).read().split('\n')
# substitui o impl Replicacao antigo (linhas 96..110, indice 95..109)
assert linhas[95].strip()=='impl Replicacao {', linhas[95]
assert linhas[109].strip()=='}', linhas[109]
novo = '''impl Replicacao {
    /// Resolve um dos enderecos de replicacao.
    fn resolver(rotulo: &str, texto: &str) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        texto
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("replicacao.{rotulo} invalida {texto:?}: {e}")))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("replicacao.{rotulo} sem endereco: {texto:?}")))
    }

    /// Por onde o source ENVIA os eventos.
    pub fn endereco_envio(&self) -> Result<SocketAddr> {
        Replicacao::resolver("envio", &self.envio)
    }

    /// Por onde o source RECEBE o retorno das replicas.
    pub fn endereco_retorno(&self) -> Result<SocketAddr> {
        Replicacao::resolver("retorno", &self.retorno)
    }

    /// As portas configuradas, em ordem, para o arranque e para o `config`.
    pub fn portas(&self) -> Vec<(&'static str, &str)> {
        let mut v = Vec::new();
        if !self.envio.is_empty() {
            v.push(("envio", self.envio.as_str()));
        }
        if !self.retorno.is_empty() {
            v.push(("retorno", self.retorno.as_str()));
        }
        v
    }
}'''
linhas[95:110] = novo.split('\n')
open(p,'w').write('\n'.join(linhas))
