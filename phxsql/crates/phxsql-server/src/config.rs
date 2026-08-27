//! Configuracao do servidor, lida do `config.json`.
//!
//! O arquivo e JSON puro, lido pelo leitor do proprio projeto -- nenhuma
//! dependencia externa entra so por causa da configuracao.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::blacklist::Politica;
use crate::usuarios::Cadastro;

/// Porta padrao do PhxSql.
pub const PORTA_PADRAO: u16 = 5000;

/// Porta padrao da interface web. Outra porta de proposito: quem fala HTTP
/// nao e quem fala JSON Lines, e separar deixa o firewall escolher.
pub const PORTA_WEB_PADRAO: u16 = 5001;

/// Papel do servidor na replicacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    /// Servidor sozinho, sem replicacao.
    Isolado,
    /// Origem: mantem o diario e atende as replicas que se conectam.
    Source,
    /// Replica: conecta no source, le os eventos e aplica localmente.
    Replica,
}

impl Papel {
    fn de_texto(s: &str) -> Result<Papel> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "isolado" | "standalone" => Papel::Isolado,
            "source" | "master" | "origem" => Papel::Source,
            "replica" | "slave" => Papel::Replica,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "papel de replicacao desconhecido: {outro}"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Papel::Isolado => "isolado",
            Papel::Source => "source",
            Papel::Replica => "replica",
        }
    }
}

/// De onde a replica puxa os eventos.
#[derive(Debug, Clone)]
pub struct Origem {
    pub nome: String,
    pub host: String,
    pub porta: u16,
    pub token: String,
    /// Databases a replicar. Vazio = todos.
    pub databases: Vec<String>,
    /// Segundos entre tentativas quando a conexao cai.
    pub reconectar_em: u64,
}

#[derive(Debug, Clone)]
pub struct Replicacao {
    pub papel: Papel,
    /// Identidade deste servidor, usada na numeracao global dos eventos.
    pub id_servidor: String,
    /// IPs autorizados a pedir o fluxo de replicacao (so no source).
    pub replicas_autorizadas: Vec<String>,
    /// Origens de onde puxar (so na replica). Varias = multi-source.
    pub origens: Vec<Origem>,
}

impl Default for Replicacao {
    fn default() -> Self {
        Replicacao {
            papel: Papel::Isolado,
            id_servidor: String::new(),
            replicas_autorizadas: Vec::new(),
            origens: Vec::new(),
        }
    }
}

/// Interface web: um servidor HTTP separado, que serve a pagina do Centro de
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
    /// Servidores PhxSql que esta interface pode alcancar, como "host:porta".
    ///
    /// VAZIO = so este servidor. E o padrao, e e o padrao certo: uma interface
    /// que fala com qualquer endereco e um proxy aberto de saida, e quem
    /// invadir a porta da web ganha a rede inteira junto.
    pub destinos: Vec<String>,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            ligado: false,
            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
            destinos: Vec::new(),
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
                destinos: w.textos("destinos"),
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

    /// A interface pode abrir conexao para este endereco?
    ///
    /// Compara o texto exato do `config.json`. Nada de resolver nome e
    /// comparar IP: quem controla o DNS decidiria o que a lista permite.
    /// Ha algum destino configurado? Sem isso a interface so fala consigo.
    pub fn destinos_permitidos_algum(&self) -> bool {
        !self.destinos.is_empty()
    }

    pub fn destino_permitido(&self, destino: &str) -> bool {
        let d = destino.trim();
        !d.is_empty() && self.destinos.iter().any(|p| p.trim() == d)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Endereco e porta de escuta.
    pub bind: String,
    /// Raiz onde ficam os databases.
    pub base: PathBuf,
    /// Segredo exigido em todo pedido.
    pub token: String,
    /// Teto de linhas devolvidas numa varredura.
    pub max_linhas: u64,
    /// Arquivo do log de acessos.
    pub log_acessos: PathBuf,
    /// IPs autorizados. Vazio = qualquer origem (so use atras de firewall).
    pub ips_permitidos: Vec<String>,
    /// Conexoes simultaneas aceitas.
    pub conexoes_max: usize,
    /// Segundos de espera por um pedido antes de encerrar a conexao.
    pub timeout_s: u64,
    /// Recusa qualquer operacao de escrita.
    pub somente_leitura: bool,
    pub replicacao: Replicacao,
    /// Usuarios e o poder de cada um sobre cada base.
    pub cadastro: Cadastro,
    /// Comandos e bases proibidos, e a politica de bloqueio.
    pub politica: Politica,
    /// Arquivo da lista de bloqueio.
    pub blacklist: PathBuf,
    /// Interface web.
    pub web: Web,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: format!("0.0.0.0:{PORTA_PADRAO}"),
            base: PathBuf::from("dados"),
            token: String::new(),
            max_linhas: 1_000,
            log_acessos: PathBuf::from("acessos.log"),
            ips_permitidos: Vec::new(),
            conexoes_max: 64,
            timeout_s: 30,
            somente_leitura: false,
            replicacao: Replicacao::default(),
            cadastro: Cadastro::default(),
            politica: Politica::default(),
            blacklist: PathBuf::from("blacklist.json"),
            web: Web::default(),
        }
    }
}

impl Config {
    /// Le o `config.json` do caminho informado.
    pub fn ler(caminho: impl AsRef<Path>) -> Result<Config> {
        let caminho = caminho.as_ref();
        let texto = std::fs::read_to_string(caminho).map_err(|e| {
            PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", caminho.display()))
        })?;
        let json = Json::analisar(&texto)?;
        let mut c = Config::de_json(&json)?;
        // Caminhos relativos valem a partir do diretorio do config.json.
        if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            if c.base.is_relative() {
                c.base = dir.join(&c.base);
            }
            if c.log_acessos.is_relative() {
                c.log_acessos = dir.join(&c.log_acessos);
            }
            if c.blacklist.is_relative() {
                c.blacklist = dir.join(&c.blacklist);
            }
        }
        c.validar()?;
        Ok(c)
    }

    pub fn de_json(j: &Json) -> Result<Config> {
        let padrao = Config::default();
        let rep = match j.campo("replicacao") {
            None => Replicacao::default(),
            Some(r) => Replicacao {
                papel: Papel::de_texto(r.texto_ou("papel", "isolado"))?,
                id_servidor: r.texto_ou("id_servidor", "").to_string(),
                replicas_autorizadas: r.textos("replicas_autorizadas"),
                origens: r
                    .campo("origens")
                    .and_then(Json::lista)
                    .map(|l| {
                        l.iter()
                            .map(|o| Origem {
                                nome: o.texto_ou("nome", "origem").to_string(),
                                host: o.texto_ou("host", "127.0.0.1").to_string(),
                                porta: o.inteiro_ou("porta", PORTA_PADRAO as i64) as u16,
                                token: o.texto_ou("token", "").to_string(),
                                databases: o.textos("databases"),
                                reconectar_em: o.inteiro_ou("reconectar_em", 10).max(1) as u64,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            },
        };

        Ok(Config {
            bind: j.texto_ou("bind", &padrao.bind).to_string(),
            base: PathBuf::from(j.texto_ou("base", "dados")),
            token: j.texto_ou("token", "").to_string(),
            max_linhas: j.inteiro_ou("max_linhas", padrao.max_linhas as i64).max(1) as u64,
            log_acessos: PathBuf::from(j.texto_ou("log_acessos", "acessos.log")),
            ips_permitidos: j.textos("ips_permitidos"),
            conexoes_max: j
                .inteiro_ou("conexoes_max", padrao.conexoes_max as i64)
                .max(1) as usize,
            timeout_s: j.inteiro_ou("timeout_s", padrao.timeout_s as i64).max(1) as u64,
            somente_leitura: j.booleano_ou("somente_leitura", false),
            replicacao: rep,
            cadastro: Cadastro::de_json(j)?,
            politica: match j.campo("seguranca") {
                Some(seg) => Politica::de_json(seg),
                None => Politica::default(),
            },
            blacklist: PathBuf::from(
                j.campo("seguranca")
                    .map(|seg| seg.texto_ou("blacklist", "blacklist.json"))
                    .unwrap_or("blacklist.json"),
            ),
            web: Web::de_json(j),
        })
    }

    fn validar(&self) -> Result<()> {
        if self.token.trim().is_empty() {
            return Err(PhxError::Esquema(
                "config.json sem token: preencha o campo \"token\" antes de subir o servidor"
                    .into(),
            ));
        }
        self.endereco()?;
        if self.web.ligado {
            let web = self.web.endereco()?;
            if web == self.endereco()? {
                return Err(PhxError::Esquema(format!(
                    "web.bind e bind apontam para o mesmo endereco ({web}): a interface precisa de uma porta so dela"
                )));
            }
        }
        if self.replicacao.papel == Papel::Replica && self.replicacao.origens.is_empty() {
            return Err(PhxError::Esquema(
                "papel replica exige ao menos uma origem em replicacao.origens".into(),
            ));
        }
        Ok(())
    }

    pub fn endereco(&self) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        self.bind
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("bind invalido {:?}: {e}", self.bind)))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("bind sem endereco: {:?}", self.bind)))
    }

    /// O IP tem permissao de conectar? Lista vazia libera todos.
    pub fn ip_permitido(&self, ip: &str) -> bool {
        self.ips_permitidos.is_empty() || self.ips_permitidos.iter().any(|p| p == ip)
    }

    /// Comparacao de token em tempo constante, para nao vazar o segredo pelo
    /// tempo de resposta.
    pub fn token_confere(&self, oferecido: &str) -> bool {
        let a = self.token.as_bytes();
        let b = oferecido.as_bytes();
        if a.len() != b.len() {
            return false;
        }
        let mut diferenca = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diferenca |= x ^ y;
        }
        diferenca == 0
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("bind", Json::texto_de(&self.bind)),
            ("base", Json::texto_de(self.base.display().to_string())),
            ("token", Json::texto_de("(oculto)")),
            ("max_linhas", Json::de_u64(self.max_linhas)),
            (
                "log_acessos",
                Json::texto_de(self.log_acessos.display().to_string()),
            ),
            (
                "ips_permitidos",
                Json::Lista(self.ips_permitidos.iter().map(Json::texto_de).collect()),
            ),
            ("conexoes_max", Json::de_u64(self.conexoes_max as u64)),
            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("papel", Json::texto_de(self.replicacao.papel.nome())),
            (
                "comandos_proibidos",
                Json::Lista(
                    self.politica
                        .comandos_proibidos
                        .iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ),
            (
                "firewall",
                Json::Bool(
                    self.politica
                        .firewall
                        .as_ref()
                        .map(|f| f.ligado)
                        .unwrap_or(false),
                ),
            ),
            (
                "web",
                Json::texto_de(if self.web.ligado {
                    self.web.bind.clone()
                } else {
                    "desligada".to_string()
                }),
            ),
            (
                "usuarios",
                Json::de_u64(
                    (self.cadastro.usuarios.len() + usize::from(self.cadastro.root.is_some()))
                        as u64,
                ),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padroes_quando_o_json_e_minimo() {
        let j = Json::analisar(r#"{"token":"segredo"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert_eq!(c.bind, "0.0.0.0:5000");
        assert_eq!(c.max_linhas, 1_000);
        assert_eq!(c.replicacao.papel, Papel::Isolado);
        assert!(c.ips_permitidos.is_empty());
        c.validar().unwrap();
    }

    #[test]
    fn sem_token_nao_sobe() {
        let j = Json::analisar("{}").unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.validar().is_err());
    }

    #[test]
    fn replica_sem_origem_nao_sobe() {
        let j = Json::analisar(r#"{"token":"x","replicacao":{"papel":"replica"}}"#).unwrap();
        assert!(Config::de_json(&j).unwrap().validar().is_err());
    }

    #[test]
    fn le_origens_de_replicacao() {
        let txt = r#"{
          "token":"x",
          "replicacao":{
            "papel":"replica",
            "id_servidor":"belgica-01",
            "origens":[
              {"nome":"curitiba","host":"10.1.1.102","porta":5000,"token":"t1","databases":["Z"]},
              {"nome":"saopaulo","host":"10.2.1.10","porta":5000,"token":"t2"}
            ]
          }
        }"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.papel, Papel::Replica);
        assert_eq!(c.replicacao.origens.len(), 2);
        assert_eq!(c.replicacao.origens[0].host, "10.1.1.102");
        assert_eq!(c.replicacao.origens[0].databases, vec!["Z"]);
        assert_eq!(c.replicacao.origens[1].porta, 5000);
        assert_eq!(c.replicacao.origens[1].reconectar_em, 10);
        c.validar().unwrap();
    }

    #[test]
    fn le_a_secao_de_seguranca() {
        let txt = r#"{
          "token":"x",
          "seguranca":{
            "comandos_proibidos":["excluir","reindexar"],
            "bases_proibidas":["financeiro"],
            "tentativas_ate_bloquear":3,
            "janela_minutos":5,
            "bloqueio_minutos":120,
            "blacklist":"bl.json",
            "firewall":{"ligado":true,"bloquear":["/sbin/iptables","-s","{ip}"]}
          }
        }"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.politica.comando_proibido("excluir"));
        assert!(c.politica.comando_proibido("REINDEXAR"));
        assert!(!c.politica.comando_proibido("ler"));
        assert!(c.politica.base_proibida("financeiro"));
        assert_eq!(c.politica.tentativas_ate_bloquear, 3);
        assert_eq!(c.politica.bloqueio_minutos, 120);
        assert!(c.politica.firewall.as_ref().unwrap().ligado);
        assert_eq!(c.blacklist, PathBuf::from("bl.json"));
    }

    #[test]
    fn sem_secao_de_seguranca_nada_e_proibido() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.politica.comando_proibido("excluir"));
        assert!(c.politica.firewall.is_none());
        assert_eq!(c.politica.tentativas_ate_bloquear, 5);
    }

    #[test]
    fn lista_de_ips_filtra() {
        let j = Json::analisar(r#"{"token":"x","ips_permitidos":["192.168.50.20"]}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.ip_permitido("192.168.50.20"));
        assert!(!c.ip_permitido("10.0.0.1"));

        let livre = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(livre.ip_permitido("qualquer"));
    }

    #[test]
    fn token_em_tempo_constante() {
        let j = Json::analisar(r#"{"token":"abc123"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.token_confere("abc123"));
        assert!(!c.token_confere("abc124"));
        assert!(!c.token_confere("abc"));
        assert!(!c.token_confere(""));
    }

    #[test]
    fn papel_aceita_nomenclatura_antiga_e_nova() {
        assert_eq!(Papel::de_texto("master").unwrap(), Papel::Source);
        assert_eq!(Papel::de_texto("source").unwrap(), Papel::Source);
        assert_eq!(Papel::de_texto("slave").unwrap(), Papel::Replica);
        assert_eq!(Papel::de_texto("replica").unwrap(), Papel::Replica);
        assert!(Papel::de_texto("banana").is_err());
    }
    #[test]
    fn a_interface_web_vem_desligada_e_presa_ao_proprio_computador() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.web.ligado);
        assert_eq!(c.web.bind, "127.0.0.1:5001");
        assert_eq!(c.web.sessao_minutos, 60);
        assert_eq!(c.web.sessao_ms(), 3_600_000);
    }

    #[test]
    fn le_a_secao_web() {
        let txt =
            r#"{"token":"x","web":{"ligado":true,"bind":"0.0.0.0:8080","sessao_minutos":15}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.ligado);
        assert_eq!(c.web.bind, "0.0.0.0:8080");
        assert_eq!(c.web.sessao_ms(), 900_000);
        c.validar().unwrap();
    }

    #[test]
    fn a_web_nao_pode_roubar_a_porta_de_dados() {
        let txt = r#"{"token":"x","bind":"127.0.0.1:5000","web":{"ligado":true,"bind":"127.0.0.1:5000"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.validar().is_err());
    }

    #[test]
    fn web_desligada_nao_valida_o_endereco() {
        // Um bind ruim numa interface desligada nao impede o servidor de subir.
        let txt = r#"{"token":"x","web":{"bind":"isso nao e endereco"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        c.validar().unwrap();
    }
}
