# Add destinos allow-list to web config
# 27/08 20:33

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
velho='''pub struct Web {
    pub ligado: bool,
    /// Endereco de escuta da interface. Padrao: so o proprio computador.
    pub bind: String,
    /// Minutos que uma sessao do navegador vale sem uso. Cada clique renova.
    pub sessao_minutos: u64,
}'''
novo='''pub struct Web {
    pub ligado: bool,
    /// Endereco de escuta da interface. Padrao: so o proprio computador.
    pub bind: String,
    /// Minutos que uma sessao do navegador vale sem uso. Cada clique renova.
    pub sessao_minutos: u64,
    /// Servidores PhxSql que esta interface pode alcancar, como "host:porta".
    ///
    /// VAZIO = so este servidor. E o padrao, e e o padrao certo: uma interface
    /// que fala com qualquer endereco e um proxy aberto de saida, e quem
    /// invadir a porta da web ganha a rede inteira junto.
    pub destinos: Vec<String>,
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)
s=s.replace('''            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
        }''','''            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
            destinos: Vec::new(),
        }''')
s=s.replace('''                sessao_minutos: w
                    .inteiro_ou("sessao_minutos", padrao.sessao_minutos as i64)
                    .max(1) as u64,
            },''','''                sessao_minutos: w
                    .inteiro_ou("sessao_minutos", padrao.sessao_minutos as i64)
                    .max(1) as u64,
                destinos: w.textos("destinos"),
            },''')
s=s.replace('''    /// Prazo da sessao em milissegundos.
    pub fn sessao_ms(&self) -> i64 {
        self.sessao_minutos as i64 * 60_000
    }''','''    /// Prazo da sessao em milissegundos.
    pub fn sessao_ms(&self) -> i64 {
        self.sessao_minutos as i64 * 60_000
    }

    /// A interface pode abrir conexao para este endereco?
    ///
    /// Compara o texto exato do `config.json`. Nada de resolver nome e
    /// comparar IP: quem controla o DNS decidiria o que a lista permite.
    pub fn destino_permitido(&self, destino: &str) -> bool {
        let d = destino.trim();
        !d.is_empty() && self.destinos.iter().any(|p| p.trim() == d)
    }''')
open(p,'w').write(s)
