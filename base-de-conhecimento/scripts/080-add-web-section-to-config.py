# Add web section to config
# 27/08 19:43

import re
p='crates/phxsql-server/src/config.rs'
s=open(p).read()

# 1. struct Web antes de "#[derive(Debug, Clone)]\npub struct Config {"
web = '''/// Interface web: um servidor HTTP separado, que serve a pagina do Centro de
/// Controle e traduz o clique do navegador no mesmo protocolo da porta 5000.
///
/// Vem DESLIGADA e presa ao proprio computador. Ligar abre uma porta a mais, e
/// isso e uma decisao de quem administra -- nao um padrao herdado.
#[derive(Debug, Clone)]
pub struct Web {
    pub ligado: bool,
    /// Endereco de escuta da interface. Padrao: so o proprio computador.
    pub bind: String,
    /// Minutos que uma sessao do navegador vale sem uso. Cada clique renova.
    pub sessao_minutos: u64,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            ligado: false,
            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
        }
    }
}

impl Web {
    fn de_json(j: &Json) -> Web {
        let padrao = Web::default();
        match j.campo("web") {
            None => padrao,
            Some(w) => Web {
                ligado: w.booleano_ou("ligado", false),
                bind: w.texto_ou("bind", &padrao.bind).to_string(),
                sessao_minutos: w
                    .inteiro_ou("sessao_minutos", padrao.sessao_minutos as i64)
                    .max(1) as u64,
            },
        }
    }

    pub fn endereco(&self) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        self.bind
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("web.bind invalido {:?}: {e}", self.bind)))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("web.bind sem endereco: {:?}", self.bind)))
    }

    /// Prazo da sessao em milissegundos.
    pub fn sessao_ms(&self) -> i64 {
        self.sessao_minutos as i64 * 60_000
    }
}

'''
alvo = '#[derive(Debug, Clone)]\npub struct Config {'
assert s.count(alvo)==1
s = s.replace(alvo, web+alvo)

# porta padrao web
s = s.replace('pub const PORTA_PADRAO: u16 = 5000;',
              'pub const PORTA_PADRAO: u16 = 5000;\n\n/// Porta padrao da interface web. Outra porta de proposito: quem fala HTTP\n/// nao e quem fala JSON Lines, e separar deixa o firewall escolher.\npub const PORTA_WEB_PADRAO: u16 = 5001;')

# campo no struct
s = s.replace('''    /// Arquivo da lista de bloqueio.
    pub blacklist: PathBuf,
}''','''    /// Arquivo da lista de bloqueio.
    pub blacklist: PathBuf,
    /// Interface web.
    pub web: Web,
}''')

# default
s = s.replace('''            blacklist: PathBuf::from("blacklist.json"),
        }''','''            blacklist: PathBuf::from("blacklist.json"),
            web: Web::default(),
        }''')

# de_json
s = s.replace('''                    .unwrap_or("blacklist.json"),
            ),
        })''','''                    .unwrap_or("blacklist.json"),
            ),
            web: Web::de_json(j),
        })''')

# validar
s = s.replace('''        self.endereco()?;
        if self.replicacao.papel''','''        self.endereco()?;
        if self.web.ligado {
            let web = self.web.endereco()?;
            if web == self.endereco()? {
                return Err(PhxError::Esquema(format!(
                    "web.bind e bind apontam para o mesmo endereco ({web}): a interface precisa de uma porta so dela"
                )));
            }
        }
        if self.replicacao.papel''')

# para_json
s = s.replace('''            (
                "usuarios",''','''            (
                "web",
                Json::texto_de(if self.web.ligado {
                    self.web.bind.clone()
                } else {
                    "desligada".to_string()
                }),
            ),
            (
                "usuarios",''')
open(p,'w').write(s)
print("config.rs ok")
