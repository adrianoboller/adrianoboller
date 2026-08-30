# Add the Remoto connection type
# 27/08 20:34

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# conexao remota pendurada na sessao
s=s.replace('''    /// Tabelas residentes em RAM, por "database/tabela". Nada entra aqui
    /// sozinho: so o que alguem pediu para carregar.
    residentes: Mutex<HashMap<String, TabelaMemoria>>,''','''    /// Tabelas residentes em RAM, por "database/tabela". Nada entra aqui
    /// sozinho: so o que alguem pediu para carregar.
    residentes: Mutex<HashMap<String, TabelaMemoria>>,
    /// Conexoes abertas para outros PhxSql, uma por sessao do navegador.
    ///
    /// Ficam abertas de proposito: o protocolo da porta 5000 autentica uma vez
    /// por CONEXAO, entao manter o soquete e o que faz o PBKDF2 do servidor
    /// remoto rodar uma vez por login e nao a cada clique.
    remotos: Mutex<HashMap<String, Arc<Mutex<Remoto>>>>,''')
s=s.replace('            residentes: Mutex::new(HashMap::new()),',
            '            residentes: Mutex::new(HashMap::new()),\n            remotos: Mutex::new(HashMap::new()),')

# struct Remoto
s=s.replace('''pub struct Servidor {''','''/// Uma conexao viva para outro PhxSql, do lado de ca da interface.
pub struct Remoto {
    pub destino: String,
    leitor: BufReader<TcpStream>,
    escrita: TcpStream,
}

impl Remoto {
    /// Abre a conexao. Nao autentica -- quem autentica e o pedido de login,
    /// que segue por aqui igual a qualquer outro.
    pub fn abrir(destino: &str, timeout_s: u64) -> Result<Remoto> {
        use std::net::ToSocketAddrs;
        let endereco = destino
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("destino {destino:?} nao resolve: {e}")))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("destino {destino:?} sem endereco")))?;
        let fluxo = TcpStream::connect_timeout(&endereco, Duration::from_secs(timeout_s.min(10)))
            .map_err(|e| PhxError::Esquema(format!("nao consegui falar com {destino}: {e}")))?;
        fluxo.set_read_timeout(Some(Duration::from_secs(timeout_s)))?;
        let escrita = fluxo.try_clone()?;
        Ok(Remoto {
            destino: destino.to_string(),
            leitor: BufReader::new(fluxo),
            escrita,
        })
    }

    /// Manda uma linha e devolve a resposta, crua.
    ///
    /// Crua de proposito: o que o servidor remoto respondeu e o que o
    /// navegador recebe. Reescrever no meio do caminho seria mentir sobre
    /// quem respondeu o que.
    pub fn conversar(&mut self, linha: &str) -> Result<Json> {
        let limpa = linha.replace(['\\n', '\\r'], " ");
        writeln!(self.escrita, "{limpa}")?;
        self.escrita.flush()?;
        let mut resposta = String::new();
        if self.leitor.read_line(&mut resposta)? == 0 {
            return Err(PhxError::Esquema(format!(
                "{} fechou a conexao",
                self.destino
            )));
        }
        Json::analisar(&resposta)
    }
}

pub struct Servidor {''')
open(p,'w').write(s)
print('remoto ok')
