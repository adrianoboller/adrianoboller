# Split replication into send and return ports
# 27/08 21:48

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''    /// Socket onde o SOURCE serve o fluxo de eventos para as replicas.
    ///
    /// Porta propria, separada da 5000, pelo mesmo motivo da interface web:
    /// quem fala replicacao nao e quem fala consulta, e o firewall precisa
    /// poder tratar as duas de forma diferente. Vazia = usa a porta de dados.
    ///
    /// A volta -- o "ate onde eu ja apliquei" que a replica manda de tempos em
    /// tempos -- vai pela MESMA conexao. Nao ha segundo soquete: quem abriu a
    /// conexao foi a replica, e a resposta volta por onde veio.
    pub escuta: String,''','''    /// Socket por onde o SOURCE ENVIA os eventos para as replicas.
    ///
    /// Porta propria, separada da 5000, pelo mesmo motivo da interface web:
    /// quem fala replicacao nao e quem fala consulta, e o firewall precisa
    /// poder tratar as duas de forma diferente. Vazia = usa a porta de dados.
    pub envio: String,
    /// Socket por onde o SOURCE RECEBE o retorno das replicas.
    ///
    /// O retorno e o "apliquei ate aqui" de cada replica, mais os pedidos de
    /// reenvio. Separado do envio a pedido: com dois soquetes, uma replica
    /// lenta lendo devagar nao segura o canal por onde as confirmacoes das
    /// outras chegam, e o firewall pode abrir so um sentido.
    ///
    /// Vazio = a volta usa a MESMA conexao do envio, que e o desenho mais
    /// simples e o que o MySQL(R) faz.
    pub retorno: String,''')
s=s.replace('''            papel: Papel::Isolado,
            escuta: String::new(),''','''            papel: Papel::Isolado,
            envio: String::new(),
            retorno: String::new(),''')
s=s.replace('''                escuta: r.texto_ou("escuta", "").trim().to_string(),''',
'''                // "escuta" e o nome antigo de "envio". Continua valendo:
                // config que ja existe nao pode parar de subir por renomeacao.
                envio: r
                    .texto_ou("envio", r.texto_ou("escuta", ""))
                    .trim()
                    .to_string(),
                retorno: r.texto_ou("retorno", "").trim().to_string(),''')

# endereco() vira dois
s=s.replace('''    /// Endereco onde o source serve o fluxo. Erro se `escuta` for invalida.
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
    }''','''    /// Resolve um dos enderecos de replicacao.
    fn resolver(rotulo: &str, texto: &str) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        texto
            .to_socket_addrs()
            .map_err(|e| {
                PhxError::Esquema(format!("replicacao.{rotulo} invalida {texto:?}: {e}"))
            })?
            .next()
            .ok_or_else(|| {
                PhxError::Esquema(format!("replicacao.{rotulo} sem endereco: {texto:?}"))
            })
    }

    /// Por onde o source envia os eventos.
    pub fn endereco_envio(&self) -> Result<SocketAddr> {
        Replicacao::resolver("envio", &self.envio)
    }

    /// Por onde o source recebe o retorno das replicas.
    pub fn endereco_retorno(&self) -> Result<SocketAddr> {
        Replicacao::resolver("retorno", &self.retorno)
    }

    /// As portas configuradas, para o arranque e para o `config`.
    pub fn portas(&self) -> Vec<(&'static str, &str)> {
        let mut v = Vec::new();
        if !self.envio.is_empty() {
            v.push(("envio", self.envio.as_str()));
        }
        if !self.retorno.is_empty() {
            v.push(("retorno", self.retorno.as_str()));
        }
        v
    }''')
open(p,'w').write(s)
