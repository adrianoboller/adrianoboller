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
    /// Socket por onde o SOURCE ENVIA os eventos para as replicas.
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
    pub retorno: String,
    /// Identidade deste servidor, usada na numeracao global dos eventos.
    pub id_servidor: String,
    /// IPs autorizados a pedir o fluxo de replicacao (so no source).
    pub replicas_autorizadas: Vec<String>,
    /// Origens de onde puxar (so na replica). Varias = multi-source.
    pub origens: Vec<Origem>,
}

impl Replicacao {
    /// Resolve um dos enderecos de replicacao.
    fn resolver(rotulo: &str, texto: &str) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        texto
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("replicacao.{rotulo} invalida {texto:?}: {e}")))?
            .next()
            .ok_or_else(|| {
                PhxError::Esquema(format!("replicacao.{rotulo} sem endereco: {texto:?}"))
            })
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
}

impl Default for Replicacao {
    fn default() -> Self {
        Replicacao {
            papel: Papel::Isolado,
            envio: String::new(),
            retorno: String::new(),
            id_servidor: String::new(),
            replicas_autorizadas: Vec::new(),
            origens: Vec::new(),
        }
    }
}

/// Backup agendado.
///
/// Vem desligado. Backup que roda sozinho num destino que ninguem conferiu e
/// backup que enche o disco e para -- ligar e uma decisao, com um destino
/// escolhido de proposito.
#[derive(Debug, Clone)]
pub struct Backup {
    pub agendado: bool,
    /// Pasta onde os arquivos caem.
    pub destino: PathBuf,
    /// Hora do dia, "HH:MM". Vazia = usa `cada_horas`.
    pub hora: String,
    /// Intervalo em horas, quando nao ha hora marcada.
    pub cada_horas: u64,
    /// Um ZIP unico (padrao) ou a arvore de diretorios.
    pub zip: bool,
    /// Qual database copiar. Vazio = todos.
    pub database: String,
    /// Nome que entra no arquivo, no lugar do usuario.
    pub admin: String,
    /// Quantos arquivos guardar. Zero = nao apaga nada.
    pub manter: usize,
}

impl Default for Backup {
    fn default() -> Self {
        Backup {
            agendado: false,
            destino: PathBuf::from("backups"),
            hora: String::new(),
            cada_horas: 24,
            zip: true,
            database: String::new(),
            admin: "agendado".into(),
            manter: 14,
        }
    }
}

impl Backup {
    fn de_json(j: &Json) -> Result<Backup> {
        let padrao = Backup::default();
        let Some(b) = j.campo("backup") else {
            return Ok(padrao);
        };
        let hora = b.texto_ou("hora", "").trim().to_string();
        if !hora.is_empty() && Backup::minuto_do_dia(&hora).is_none() {
            return Err(PhxError::Esquema(format!(
                "backup.hora invalida: {hora:?} (use \"HH:MM\", 24 horas)"
            )));
        }
        Ok(Backup {
            agendado: b.booleano_ou("agendado", false),
            destino: PathBuf::from(b.texto_ou("destino", "backups")),
            hora,
            cada_horas: b.inteiro_ou("cada_horas", padrao.cada_horas as i64).max(1) as u64,
            zip: b.booleano_ou("zip", true),
            database: b.texto_ou("database", "").trim().to_string(),
            admin: b.texto_ou("admin", "agendado").trim().to_string(),
            manter: b.inteiro_ou("manter", padrao.manter as i64).max(0) as usize,
        })
    }

    /// "HH:MM" em minutos desde a meia-noite. `None` se nao for hora.
    pub fn minuto_do_dia(hora: &str) -> Option<u64> {
        let (h, m) = hora.split_once(':')?;
        let h: u64 = h.trim().parse().ok()?;
        let m: u64 = m.trim().parse().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        Some(h * 60 + m)
    }

    /// Ja passou da hora de rodar de novo?
    ///
    /// `ultimo_ms` e zero quando nunca rodou. Com hora marcada, dispara quando
    /// o minuto do dia chega e ainda nao rodou hoje -- e nao a cada minuto
    /// depois disso.
    pub fn hora_de_rodar(&self, agora_ms: i64, ultimo_ms: i64) -> bool {
        if !self.agendado {
            return false;
        }
        match Backup::minuto_do_dia(&self.hora) {
            Some(alvo) => {
                let minuto_agora = (agora_ms.rem_euclid(86_400_000) / 60_000) as u64;
                let dia_agora = agora_ms.div_euclid(86_400_000);
                let dia_ultimo = ultimo_ms.div_euclid(86_400_000);
                minuto_agora >= alvo && (ultimo_ms == 0 || dia_agora > dia_ultimo)
            }
            None => {
                let intervalo = self.cada_horas as i64 * 3_600_000;
                ultimo_ms == 0 || agora_ms - ultimo_ms >= intervalo
            }
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
    pub servidores: Vec<String>,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            ligado: false,
            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
            servidores: Vec::new(),
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
                servidores: w.textos("servidores"),
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
    /// Ha algum servidor configurado? Sem isso a interface so fala consigo.
    pub fn alcanca_outro_servidor(&self) -> bool {
        !self.servidores.is_empty()
    }

    pub fn servidor_permitido(&self, alvo: &str) -> bool {
        let d = alvo.trim();
        !d.is_empty() && self.servidores.iter().any(|p| p.trim() == d)
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
    /// Espelha todo `.reg` num `.bkp` irmao -- a segunda chance.
    ///
    /// Custa uma escrita a mais por gravacao e o dobro de espaco do `.reg`.
    /// Protege contra o dado ficar RUIM, nao contra o disco morrer: os dois
    /// arquivos moram no mesmo lugar.
    pub espelho: bool,
    pub replicacao: Replicacao,
    /// Usuarios e o poder de cada um sobre cada base.
    pub cadastro: Cadastro,
    /// Comandos e bases proibidos, e a politica de bloqueio.
    pub politica: Politica,
    /// Arquivo da lista de bloqueio.
    pub blacklist: PathBuf,
    /// Interface web.
    pub web: Web,
    /// Backup agendado.
    pub backup: Backup,
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
            espelho: false,
            replicacao: Replicacao::default(),
            cadastro: Cadastro::default(),
            politica: Politica::default(),
            blacklist: PathBuf::from("blacklist.json"),
            web: Web::default(),
            backup: Backup::default(),
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
            if c.backup.destino.is_relative() {
                c.backup.destino = dir.join(&c.backup.destino);
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
                // "escuta" e o nome antigo de "envio". Continua valendo:
                // config que ja existe nao pode parar de subir por renomeacao.
                envio: r
                    .texto_ou("envio", r.texto_ou("escuta", ""))
                    .trim()
                    .to_string(),
                retorno: r.texto_ou("retorno", "").trim().to_string(),
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
            espelho: j.booleano_ou("espelho", false),
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
            backup: Backup::de_json(j)?,
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
        // Cada porta de replicacao contra a de dados, a da web e a outra.
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
            ("espelho", Json::Bool(self.espelho)),
            ("papel", Json::texto_de(self.replicacao.papel.nome())),
            (
                "replicacao_portas",
                Json::Objeto(
                    self.replicacao
                        .portas()
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), Json::texto_de(v)))
                        .collect(),
                ),
            ),
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
    #[test]
    fn le_a_porta_de_replicacao() {
        // Nome novo: envio e retorno separados.
        let txt = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","envio":"0.0.0.0:5010","retorno":"0.0.0.0:5011"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.endereco_envio().unwrap().port(), 5010);
        assert_eq!(c.replicacao.endereco_retorno().unwrap().port(), 5011);
        assert_eq!(c.replicacao.portas().len(), 2);
        c.validar().unwrap();

        // Nome antigo "escuta" continua valendo como envio: config que ja
        // existe nao pode parar de subir so porque o campo foi renomeado.
        let velho = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","escuta":"0.0.0.0:5010"}}"#;
        let c = Config::de_json(&Json::analisar(velho).unwrap()).unwrap();
        assert_eq!(c.replicacao.envio, "0.0.0.0:5010");
        assert!(
            c.replicacao.retorno.is_empty(),
            "sem retorno = volta pelo envio"
        );
        c.validar().unwrap();
    }

    #[test]
    fn a_replicacao_nao_pode_roubar_a_porta_de_dados_nem_a_da_web() {
        let mesma = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","envio":"127.0.0.1:5000"}}"#;
        assert!(Config::de_json(&Json::analisar(mesma).unwrap())
            .unwrap()
            .validar()
            .is_err());

        let contra_web = r#"{"token":"x","bind":"127.0.0.1:5000",
          "web":{"ligado":true,"bind":"127.0.0.1:5001"},
          "replicacao":{"papel":"source","envio":"127.0.0.1:5001"}}"#;
        assert!(Config::de_json(&Json::analisar(contra_web).unwrap())
            .unwrap()
            .validar()
            .is_err());

        // E o envio contra o proprio retorno.
        let uma_contra_outra = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","envio":"127.0.0.1:5010","retorno":"127.0.0.1:5010"}}"#;
        assert!(Config::de_json(&Json::analisar(uma_contra_outra).unwrap())
            .unwrap()
            .validar()
            .is_err());
    }

    #[test]
    fn sem_escuta_a_replicacao_usa_a_porta_de_dados() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(c.replicacao.envio.is_empty());
        assert!(c.replicacao.retorno.is_empty());
        assert!(c.replicacao.portas().is_empty());
        c.validar().unwrap();
    }

    #[test]
    fn a_lista_de_servidores_da_web_e_exata() {
        let txt = r#"{"token":"x","web":{"servidores":["10.1.1.5:5000","curitiba:5000"]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.alcanca_outro_servidor());
        assert!(c.web.servidor_permitido("10.1.1.5:5000"));
        assert!(c.web.servidor_permitido(" curitiba:5000 "));
        // Sem porta, com outra porta, ou vazio: nao entra.
        assert!(!c.web.servidor_permitido("10.1.1.5"));
        assert!(!c.web.servidor_permitido("10.1.1.5:5001"));
        assert!(!c.web.servidor_permitido(""));

        let fechado = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!fechado.web.alcanca_outro_servidor());
        assert!(!fechado.web.servidor_permitido("qualquer:5000"));
    }
    #[test]
    fn o_backup_vem_desligado() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.backup.agendado);
        assert!(c.backup.zip, "zip e o padrao quando ligarem");
        assert_eq!(c.backup.manter, 14);
        assert!(
            !c.backup.hora_de_rodar(1_000_000, 0),
            "desligado nunca roda"
        );
    }

    #[test]
    fn hora_marcada_dispara_uma_vez_por_dia() {
        let txt = r#"{"token":"x","backup":{"agendado":true,"hora":"03:00"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let dia = 20_000i64 * 86_400_000;

        // 02:59 ainda nao.
        assert!(!c.backup.hora_de_rodar(dia + 2 * 3_600_000 + 59 * 60_000, 0));
        // 03:00 sim, porque nunca rodou.
        let as_tres = dia + 3 * 3_600_000;
        assert!(c.backup.hora_de_rodar(as_tres, 0));
        // 03:01, ja tendo rodado as 03:00: NAO de novo.
        assert!(!c.backup.hora_de_rodar(as_tres + 60_000, as_tres));
        // 23:59 do mesmo dia: ainda nao.
        assert!(!c.backup.hora_de_rodar(dia + 86_340_000, as_tres));
        // 03:00 do dia seguinte: sim.
        assert!(c.backup.hora_de_rodar(as_tres + 86_400_000, as_tres));
    }

    #[test]
    fn sem_hora_marcada_vale_o_intervalo() {
        let txt = r#"{"token":"x","backup":{"agendado":true,"cada_horas":6}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(
            c.backup.hora_de_rodar(1_000_000_000, 0),
            "nunca rodou, roda"
        );
        let t = 1_000_000_000i64;
        assert!(!c.backup.hora_de_rodar(t + 5 * 3_600_000, t));
        assert!(c.backup.hora_de_rodar(t + 6 * 3_600_000, t));
    }

    #[test]
    fn hora_invalida_nao_sobe() {
        for h in ["25:00", "12:60", "meia-noite", "3", "03;00"] {
            let txt = format!(r#"{{"token":"x","backup":{{"agendado":true,"hora":"{h}"}}}}"#);
            assert!(
                Config::de_json(&Json::analisar(&txt).unwrap()).is_err(),
                "{h:?} passou"
            );
        }
        assert_eq!(Backup::minuto_do_dia("03:00"), Some(180));
        assert_eq!(Backup::minuto_do_dia("23:59"), Some(1439));
        assert_eq!(Backup::minuto_do_dia("00:00"), Some(0));
    }
}
