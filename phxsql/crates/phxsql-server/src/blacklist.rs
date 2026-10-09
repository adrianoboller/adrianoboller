//! Lista de bloqueio: quem tentou o que nao devia, e ate quando fica de fora.
//!
//! O arquivo e o `blacklist.json`, com o IP, a data e a hora, o comando que
//! provocou o bloqueio e ate quando ele vale.
//!
//! # Duas gravidades
//!
//! * **Grave** -- comando proibido ou base proibida. Bloqueia na hora. Nao ha
//!   por que dar cinco chances a quem pediu exatamente o que o
//!   `config.json` diz que ninguem pode pedir.
//! * **Leve** -- token errado, senha errada, IP fora da lista. Conta as
//!   tentativas dentro de uma janela e bloqueia ao passar do limite. Errar a
//!   senha uma vez e humano; errar oito vezes em dois minutos, nao.
//!
//! # Sobre mexer no firewall
//!
//! O bloqueio **sempre** vale dentro do servidor: um IP na lista tem a conexao
//! recusada antes de qualquer outra coisa. Isso nao depende de firewall, nao
//! depende de root e nao pode falhar.
//!
//! A regra de firewall e um EXTRA, desligado por padrao. Quando ligada, o
//! comando vem inteiro do `config.json` como lista de argumentos e e executado
//! **sem shell**, com o IP validado como endereco antes de entrar no lugar do
//! `{ip}`. Um daemon de rede que monta linha de comando com texto vindo de
//! fora e uma porta dos fundos; aqui nao ha interpolacao de shell em lugar
//! nenhum.

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::Result;
use phxsql_core::json::Json;

/// A regra cobre este IP? Aceita endereco exato (`203.0.113.9`) ou faixa
/// CIDR (`203.0.113.0/24`, `2001:db8::/32`).
///
/// IPv4 e IPv6 nao se misturam: `10.0.0.0/8` nunca cobre um endereco v6 --
/// exceto a forma mapeada `::ffff:10.0.0.1`, que E o mesmo endereco chegando
/// por um soquete dual-stack, e por isso e normalizada antes de comparar.
pub fn regra_cobre_ip(regra: &str, ip: &IpAddr) -> bool {
    let ip = normalizar(ip);
    match regra.trim().split_once('/') {
        None => match regra.trim().parse::<IpAddr>() {
            Ok(r) => normalizar(&r) == ip,
            Err(_) => false,
        },
        Some((base, bits)) => {
            let (Ok(base), Ok(bits)) = (base.trim().parse::<IpAddr>(), bits.trim().parse::<u32>())
            else {
                return false;
            };
            let (vb, largura) = como_bits(&normalizar(&base));
            let (vi, li) = como_bits(&ip);
            if largura != li || bits > largura {
                return false;
            }
            if bits == 0 {
                return true;
            }
            let desloc = largura - bits;
            (vb >> desloc) == (vi >> desloc)
        }
    }
}

/// A regra e um IP ou CIDR que o servidor sabe ler? E o que a tela confere
/// ANTES de gravar: regra ilegivel na whitelist e protecao que nao protege.
pub fn regra_valida(regra: &str) -> bool {
    match regra.trim().split_once('/') {
        None => regra.trim().parse::<IpAddr>().is_ok(),
        Some((base, bits)) => match (base.trim().parse::<IpAddr>(), bits.trim().parse::<u32>()) {
            (Ok(b), Ok(n)) => n <= como_bits(&b).1,
            _ => false,
        },
    }
}

/// `::ffff:1.2.3.4` vira `1.2.3.4`: e o mesmo endereco por soquete dual-stack.
fn normalizar(ip: &IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => *ip,
        },
        v4 => *v4,
    }
}

/// O endereco como inteiro, com a largura em bits (32 ou 128).
fn como_bits(ip: &IpAddr) -> (u128, u32) {
    match ip {
        IpAddr::V4(v4) => (u32::from_be_bytes(v4.octets()) as u128, 32),
        IpAddr::V6(v6) => (u128::from_be_bytes(v6.octets()), 128),
    }
}

/// Politica de bloqueio, vinda do `config.json`.
#[derive(Debug, Clone)]
pub struct Politica {
    /// Operacoes que ninguem pode pedir, nem quem tem permissao.
    ///
    /// **Vale para o servidor inteiro**, e continua sendo o que uma string
    /// solta na lista do `config.json` significa.
    pub comandos_proibidos: Vec<String>,
    /// Operacoes proibidas SO NUM BANCO: `(banco, comando)`.
    ///
    /// # Por que a mesma lista, e nao um campo novo
    ///
    /// Porque a pergunta e uma so — «o que ninguem pede aqui?» — e duas listas
    /// para a mesma pergunta e a receita de alguem responder metade. No
    /// `config.json` a entrada por banco e um OBJETO dentro da lista que ja
    /// existe: `{"comando":"reindexar","database":"financeiro"}`.
    ///
    /// # O que ela NAO pode fazer
    ///
    /// **Afrouxar o global.** O global continua valendo para todos os bancos,
    /// e o do banco so ACRESCENTA. Uma politica por banco que pudesse liberar
    /// o que o servidor proibiu seria uma porta dos fundos com nome de
    /// diretiva.
    pub proibidos_por_base: Vec<(String, String)>,
    /// Bases que ninguem pode tocar por esta porta.
    pub bases_proibidas: Vec<String>,
    /// Tentativas leves toleradas dentro da janela.
    pub tentativas_ate_bloquear: u32,
    /// Tentativas GRAVES toleradas antes de bloquear. O padrao e 1 -- comando
    /// proibido bloqueia na hora, como sempre foi. Subir este numero e decisao
    /// de quem administra: recusa continua recusando desde a primeira, so o
    /// bloqueio do IP espera a enesima dentro da janela.
    pub tentativas_para_bloqueio: u32,
    pub janela_minutos: u64,
    /// Duracao do bloqueio. Zero = ate alguem desbloquear.
    pub bloqueio_minutos: u64,
    /// IPs e faixas CIDR que NUNCA sao bloqueados. A recusa da operacao
    /// continua valendo -- whitelist protege o acesso, nao da poder.
    pub whitelist: Vec<String>,
    pub firewall: Option<Firewall>,
    /// Contar a recusa de SQL com COMANDO EMPILHADO como tentativa leve?
    ///
    /// Nasce DESLIGADO, e o padrao e o comportamento de sempre: um erro de
    /// sintaxe nunca bloqueou ninguem aqui, e ligar isso de fabrica faria o
    /// operador que erra a digitacao cinco vezes se trancar para fora --
    /// exatamente o estrago do pedido 203, em que uma replica mal configurada
    /// bloqueou o `127.0.0.1` e derrubou a sessao junto.
    ///
    /// Ligado, a recusa conta pela MESMA politica leve que ja existe
    /// (`tentativas_ate_bloquear` na `janela_minutos`), pelo mesmo caminho do
    /// token invalido e da credencial errada. Nao ha um N proprio, e a
    /// ausencia e deliberada: um segundo contador com um segundo limite seria
    /// a segunda politica que alguem esquece de atualizar.
    pub contar_injecao_sql: bool,
    /// Contar a LINHA ACIMA DO TETO do fio como tentativa leve?
    ///
    /// Nasce DESLIGADO pelo mesmo motivo do de cima: mandar um lote grande
    /// demais e engano de cliente com a mesma cara de ataque, e um cliente que
    /// erra o tamanho do lote cinco vezes trancaria o proprio operador para
    /// fora. Ligado, conta pela politica leve que ja existe.
    pub contar_linha_acima_do_teto: bool,
    /// Contar o `cluster_pulso` com id que NAO e um no deste cluster como
    /// tentativa leve?
    ///
    /// Nasce DESLIGADO, e pela mesma medicao dos dois de cima: o no que
    /// entra a quente no cluster pulsa os antigos ANTES de ser acrescentado
    /// (`bancada/cluster/escalonar.py`, etapa 2: «no4 ja nasce sabendo dos 4»
    /// e e recusado a cada pulso durante a janela inteira). Com isto ligado
    /// de fabrica, cinco pulsos -- dez segundos -- bloqueariam o IP do no novo
    /// por uma hora em cada antigo, e na bancada, onde todos sao 127.0.0.1,
    /// bloqueariam o cluster inteiro. Ligado, a recusa conta pela politica
    /// leve que ja existe, e quem tem a credencial de replicacao deixa de
    /// enumerar os ids do cluster de graca (revisao SEC de 17/09/2026, A11).
    pub contar_pulso_desconhecido: bool,
    /// Observar a FORMA de injecao de SQL nos pedidos (pedido 495, F3)?
    ///
    /// Nasce LIGADO, ao contrario dos tres de cima, e a diferenca e o que ele
    /// faz: os de cima CONTAM para bloquear, e bloquear quem erra tranca o
    /// operador; este so OBSERVA -- a resposta ao cliente sai byte a byte a
    /// mesma, ninguem e recusado nem bloqueado, e o que acusa vira uma
    /// ocorrencia (`InjecaoSuspeita`) para quem administra. Decisao de
    /// 24/09 mantida no desenho (`ia-495-496-desenho.md` §0, item 6).
    ///
    /// E o portao do gancho: desligado, o pedido paga a leitura deste `bool`
    /// e nada mais -- nem a classificacao, nem o estado da thread.
    pub observar_injecao_sql: bool,
}

impl Default for Politica {
    fn default() -> Self {
        Politica {
            comandos_proibidos: Vec::new(),
            proibidos_por_base: Vec::new(),
            bases_proibidas: Vec::new(),
            tentativas_ate_bloquear: 5,
            tentativas_para_bloqueio: 1,
            janela_minutos: 10,
            bloqueio_minutos: 60,
            whitelist: Vec::new(),
            firewall: None,
            contar_injecao_sql: false,
            contar_linha_acima_do_teto: false,
            contar_pulso_desconhecido: false,
            observar_injecao_sql: true,
        }
    }
}

impl Politica {
    pub fn de_json(j: &Json) -> Politica {
        let padrao = Politica::default();
        Politica {
            comandos_proibidos: proibidos_globais(j),
            proibidos_por_base: proibidos_por_base(j),
            bases_proibidas: j
                .textos("bases_proibidas")
                .into_iter()
                .map(|b| b.trim().to_string())
                .filter(|b| !b.is_empty())
                .collect(),
            tentativas_ate_bloquear: j
                .inteiro_ou(
                    "tentativas_ate_bloquear",
                    padrao.tentativas_ate_bloquear as i64,
                )
                .max(1) as u32,
            tentativas_para_bloqueio: j
                .inteiro_ou(
                    "tentativas_para_bloqueio",
                    padrao.tentativas_para_bloqueio as i64,
                )
                .max(1) as u32,
            janela_minutos: j
                .inteiro_ou("janela_minutos", padrao.janela_minutos as i64)
                .max(1) as u64,
            bloqueio_minutos: j
                .inteiro_ou("bloqueio_minutos", padrao.bloqueio_minutos as i64)
                .max(0) as u64,
            whitelist: j
                .textos("whitelist")
                .into_iter()
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty())
                .collect(),
            firewall: j.campo("firewall").and_then(Firewall::de_json),
            contar_injecao_sql: j.booleano_ou("contar_injecao_sql", padrao.contar_injecao_sql),
            contar_linha_acima_do_teto: j.booleano_ou(
                "contar_linha_acima_do_teto",
                padrao.contar_linha_acima_do_teto,
            ),
            contar_pulso_desconhecido: j.booleano_ou(
                "contar_pulso_desconhecido",
                padrao.contar_pulso_desconhecido,
            ),
            observar_injecao_sql: j
                .booleano_ou("observar_injecao_sql", padrao.observar_injecao_sql),
        }
    }

    /// O IP esta na whitelist FIXA do `config.json`?
    ///
    /// So a fixa: a editavel pela tela mora no arquivo da blacklist, e quem
    /// responde pelas duas juntas e [`Blacklist::protegido`].
    pub fn na_whitelist(&self, ip: &str) -> bool {
        let Ok(endereco) = ip.trim().parse::<IpAddr>() else {
            return false;
        };
        self.whitelist.iter().any(|r| regra_cobre_ip(r, &endereco))
    }

    /// A operacao esta proibida por politica, no servidor inteiro?
    pub fn comando_proibido(&self, op: &str) -> bool {
        let alvo = op.trim().to_lowercase();
        self.comandos_proibidos.contains(&alvo)
    }

    /// A operacao esta proibida NESTE banco?
    ///
    /// So a lista por banco: quem responde pelo global e
    /// [`Politica::comando_proibido`], e o portao chama os dois — o global
    /// primeiro, porque ele nao depende do campo `database` e vale para o
    /// pedido que nem nomeia banco.
    pub fn comando_proibido_na_base(&self, op: &str, base: &str) -> bool {
        if base.is_empty() {
            return false;
        }
        let alvo = op.trim().to_lowercase();
        self.proibidos_por_base
            .iter()
            .any(|(b, c)| b == base && *c == alvo)
    }

    /// A base esta proibida por politica?
    pub fn base_proibida(&self, base: &str) -> bool {
        !base.is_empty() && self.bases_proibidas.iter().any(|b| b == base)
    }
}

/// As entradas em forma de STRING: proibidas no servidor inteiro.
///
/// E o comportamento velho, byte a byte — um `config.json` escrito antes da
/// forma por banco continua significando exatamente o que significava.
pub fn proibidos_globais(j: &Json) -> Vec<String> {
    j.campo("comandos_proibidos")
        .and_then(Json::lista)
        .map(|l| {
            l.iter()
                .filter_map(Json::texto)
                .map(|c| c.trim().to_lowercase())
                .filter(|c| !c.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// As entradas em forma de OBJETO: `(banco, comando)`.
///
/// Entrada sem `comando` ou sem `database` e ignorada, e a escolha e
/// deliberada: uma entrada pela metade que virasse «proibido em todo lugar»
/// derrubaria o servidor de quem digitou errado, e uma que virasse «proibido
/// em lugar nenhum» seria uma guarda que nao guarda. Ignorar deixa o campo
/// `estranhas` do arranque reclamar, que e onde reclamacao de configuracao ja
/// mora.
pub fn proibidos_por_base(j: &Json) -> Vec<(String, String)> {
    j.campo("comandos_proibidos")
        .and_then(Json::lista)
        .map(|l| {
            l.iter()
                .filter(|x| matches!(x, Json::Objeto(_)))
                .filter_map(|x| {
                    let comando = x.texto_ou("comando", "").trim().to_lowercase();
                    let base = x
                        .texto_ou("database", x.texto_ou("banco", ""))
                        .trim()
                        .to_string();
                    if comando.is_empty() || base.is_empty() {
                        return None;
                    }
                    Some((base, comando))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Comando de firewall, como lista de argumentos. `{ip}` vira o endereco.
#[derive(Debug, Clone)]
pub struct Firewall {
    pub ligado: bool,
    pub bloquear: Vec<String>,
    pub desbloquear: Vec<String>,
    /// Prazo duro de CADA execucao, em segundos. Passou, o filho leva `kill`.
    pub timeout_s: u64,
}

/// O prazo de fabrica do comando de firewall, e o maior que a configuracao
/// aceita. `iptables` sem `-w` sob a trava do xtables ou um `nft` travado
/// penduram sem limite; o prazo e o que impede o comando de segurar a
/// conexao de quem o provocou para sempre.
pub const PRAZO_DO_FIREWALL_S: u64 = 10;
pub const TETO_DO_PRAZO_DO_FIREWALL_S: u64 = 120;

/// O `PATH` do comando de firewall: o do gancho nao basta, porque `iptables`
/// e `nft` moram em `sbin`.
const PATH_DO_FIREWALL: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

impl Firewall {
    fn de_json(j: &Json) -> Option<Firewall> {
        let f = Firewall {
            ligado: j.booleano_ou("ligado", false),
            bloquear: j.textos("bloquear"),
            desbloquear: j.textos("desbloquear"),
            timeout_s: (j.inteiro_ou("timeout_s", PRAZO_DO_FIREWALL_S as i64).max(1) as u64)
                .min(TETO_DO_PRAZO_DO_FIREWALL_S),
        };
        if f.bloquear.is_empty() {
            None
        } else {
            Some(f)
        }
    }

    /// Roda o comando, trocando `{ip}` pelo endereco.
    ///
    /// Devolve `Ok(false)` quando o firewall esta desligado. Sem shell: cada
    /// argumento vai inteiro, e o IP so entra depois de ser validado como
    /// endereco de verdade.
    ///
    /// # Quem chama NAO pode estar segurando a `lista_negra`
    ///
    /// Pode demorar ate `timeout_s`. Foi rodando sob o mutex da lista que o
    /// comando pendurado parava o servidor inteiro (pedido 638): `barrado()`
    /// pega esse mutex em TODA conexao. Por isso a `Blacklist` nao executa
    /// nada -- so devolve o que fazer -- e as funcoes `*_no_firewall` abaixo
    /// pedem o `&Mutex`, e nao um guarda.
    ///
    /// A execucao e a do gancho do operador (`gancho::rodar`): prazo com
    /// `kill`+`wait`, ambiente limpo, saida descartada. O erro nunca leva o
    /// que o programa imprimiu.
    pub fn aplicar(&self, argumentos: &[String], ip: &str) -> Result<bool> {
        if !self.ligado || argumentos.is_empty() {
            return Ok(false);
        }
        if ip.parse::<IpAddr>().is_err() {
            return Err(phxsql_core::error::PhxError::Tipo(format!(
                "{ip:?} nao e um endereco IP; nao vai para o firewall"
            )));
        }
        let trocado: Vec<String> = argumentos.iter().map(|a| a.replace("{ip}", ip)).collect();
        crate::gancho::rodar(&crate::gancho::Execucao {
            rotulo: "comando de firewall",
            argv: &trocado,
            path: PATH_DO_FIREWALL,
            ambiente: &[],
            entrada: None,
            prazo_s: self.timeout_s,
        })
        .map_err(|e| {
            phxsql_core::error::PhxError::Corrompido(format!("o comando de firewall falhou: {e}"))
        })?;
        Ok(true)
    }

    pub fn bloquear_ip(&self, ip: &str) -> Result<bool> {
        self.aplicar(&self.bloquear, ip)
    }

    pub fn desbloquear_ip(&self, ip: &str) -> Result<bool> {
        self.aplicar(&self.desbloquear, ip)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloqueio {
    pub ip: String,
    pub desde_ms: i64,
    /// Zero = permanente, ate alguem desbloquear.
    pub ate_ms: i64,
    pub motivo: String,
    /// O comando que provocou o bloqueio.
    pub comando: String,
    pub tentativas: u32,
    /// A regra de firewall chegou a ser aplicada?
    pub firewall: bool,
}

impl Bloqueio {
    pub fn ativo_em(&self, agora_ms: i64) -> bool {
        self.ate_ms == 0 || agora_ms < self.ate_ms
    }

    pub fn desde(&self) -> String {
        instante_iso(self.desde_ms)
    }

    pub fn ate(&self) -> String {
        if self.ate_ms == 0 {
            "permanente".to_string()
        } else {
            instante_iso(self.ate_ms)
        }
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ip", Json::texto_de(&self.ip)),
            ("desde", Json::texto_de(self.desde())),
            ("desde_ms", Json::Numero(self.desde_ms as f64)),
            ("ate", Json::texto_de(self.ate())),
            ("ate_ms", Json::Numero(self.ate_ms as f64)),
            ("motivo", Json::texto_de(&self.motivo)),
            ("comando", Json::texto_de(&self.comando)),
            ("tentativas", Json::de_u64(self.tentativas as u64)),
            ("firewall", Json::Bool(self.firewall)),
        ])
    }

    fn de_json(j: &Json) -> Option<Bloqueio> {
        Some(Bloqueio {
            ip: j.campo("ip")?.texto()?.to_string(),
            desde_ms: j.campo("desde_ms")?.numero()? as i64,
            ate_ms: j.campo("ate_ms").and_then(Json::numero).unwrap_or(0.0) as i64,
            motivo: j.texto_ou("motivo", "").to_string(),
            comando: j.texto_ou("comando", "").to_string(),
            tentativas: j.inteiro_ou("tentativas", 1).max(0) as u32,
            firewall: j.booleano_ou("firewall", false),
        })
    }
}

/// O que uma violacao grave rendeu. Quem chamou redige o erro conforme o
/// caso -- e o caso importa: dizer "o IP foi bloqueado" quando nao foi seria
/// mentira na resposta do protocolo.
#[derive(Debug)]
pub enum Grave {
    /// O IP esta na whitelist: a operacao foi recusada, mas nada conta e
    /// nada bloqueia.
    Protegido,
    /// Contou dentro da janela e ainda nao chegou ao limite.
    Contada { tentativas: u32, limite: u32 },
    /// Bloqueou. O aviso e a falha nao-fatal do firewall, quando houver.
    Bloqueado(Bloqueio, Option<String>),
}

/// Aplica no firewall a regra de um bloqueio que ACABOU de ser gravado, e
/// anota na lista que ela valeu. Pede o `&Mutex` e nao um guarda de
/// proposito: o comando pode demorar ate `timeout_s`, e quem o chamasse com a
/// lista na mao parava `barrado()` -- ou seja, toda conexao -- ate ele voltar
/// (pedido 638). O servidor ja barra o IP desde o `bloquear`; a falha do
/// firewall **nao** cancela nada, so vira aviso.
pub fn aplicar_no_firewall(
    lista: &Mutex<Blacklist>,
    politica: &Politica,
    b: &mut Bloqueio,
    aviso: &mut Option<String>,
) {
    let Some(fw) = &politica.firewall else {
        return;
    };
    match fw.bloquear_ip(&b.ip) {
        Ok(true) => {
            b.firewall = true;
            let marcado = match lista.lock() {
                Ok(mut l) => l.marcar_firewall(&b.ip, b.desde_ms),
                Err(_) => Ok(()),
            };
            if let Err(e) = marcado {
                junta_aviso(aviso, format!("nao consegui gravar a blacklist: {e}"));
            }
        }
        Ok(false) => {}
        Err(e) => junta_aviso(aviso, format!("firewall: {e}")),
    }
}

fn junta_aviso(aviso: &mut Option<String>, novo: String) {
    *aviso = Some(match aviso.take() {
        Some(antes) => format!("{antes}; {novo}"),
        None => novo,
    });
}

/// Solta no firewall a regra de cada IP. Melhor esforco, como sempre foi: o
/// IP ja saiu da lista e o servidor ja o aceita. Fora do mutex, pelo mesmo
/// motivo de `aplicar_no_firewall`. Devolve o primeiro erro.
pub fn soltar_no_firewall(politica: &Politica, ips: &[String]) -> Result<()> {
    let Some(fw) = &politica.firewall else {
        return Ok(());
    };
    let mut primeiro = None;
    for ip in ips {
        if let Err(e) = fw.desbloquear_ip(ip) {
            primeiro.get_or_insert(e);
        }
    }
    primeiro.map_or(Ok(()), Err)
}

/// A lista de bloqueio, com o contador de tentativas recentes.
pub struct Blacklist {
    caminho: PathBuf,
    bloqueios: Vec<Bloqueio>,
    /// IPs e faixas CIDR que nunca bloqueiam, editaveis pela tela.
    ///
    /// Moram AQUI, e nao no `config.json`, pelo mesmo motivo do dblink e dos
    /// jobs: o que muda pela tela vive em arquivo proprio, senao cada clique
    /// reescreveria o config inteiro e arriscaria os comentarios. A whitelist
    /// do config continua valendo -- a efetiva e a uniao das duas.
    whitelist: Vec<String>,
    /// Carimbos das tentativas leves recentes, por IP. So em memoria.
    tentativas: HashMap<String, Vec<i64>>,
    /// Carimbos das tentativas GRAVES recentes, por IP. Separado das leves de
    /// proposito: misturar faria um token errado adiantar o bloqueio de um
    /// comando proibido, e os dois limites sao configuraveis em separado.
    tentativas_graves: HashMap<String, Vec<i64>>,
    /// Quando o arquivo foi gravado da ultima vez que o lemos.
    ///
    /// O `phxsqld --desbloquear` roda em OUTRO processo e mexe no mesmo
    /// arquivo. Sem isto, o servidor continuaria barrando um IP que ja saiu da
    /// lista -- foi exatamente o que aconteceu no primeiro teste ao vivo.
    lido_em: Option<SystemTime>,
}

impl Blacklist {
    /// Abre a lista, criando o arquivo quando ainda nao existe.
    pub fn abrir(caminho: impl AsRef<Path>) -> Result<Blacklist> {
        let caminho = caminho.as_ref().to_path_buf();
        if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            phxsql_store::permissao::criar_diretorio_do_banco(dir)?;
        }
        let (bloqueios, whitelist) = match std::fs::read_to_string(&caminho) {
            Err(_) => (Vec::new(), Vec::new()),
            Ok(texto) => match Json::analisar(&texto) {
                Err(_) => (Vec::new(), Vec::new()),
                Ok(j) => (
                    j.campo("bloqueios")
                        .and_then(Json::lista)
                        .map(|l| l.iter().filter_map(Bloqueio::de_json).collect())
                        .unwrap_or_default(),
                    j.textos("whitelist")
                        .into_iter()
                        .map(|w| w.trim().to_string())
                        .filter(|w| !w.is_empty())
                        .collect(),
                ),
            },
        };
        let lido_em = mtime(&caminho);
        Ok(Blacklist {
            caminho,
            bloqueios,
            whitelist,
            tentativas: HashMap::new(),
            tentativas_graves: HashMap::new(),
            lido_em,
        })
    }

    /// Rele o arquivo se alguem de fora mexeu nele. Devolve `true` se releu.
    ///
    /// Custa um `stat` por chamada, e e o que faz o `--desbloquear` de outro
    /// processo valer sem reiniciar o servidor.
    pub fn recarregar_se_mudou(&mut self) -> Result<bool> {
        let agora = mtime(&self.caminho);
        if agora == self.lido_em {
            return Ok(false);
        }
        let recarregada = Blacklist::abrir(&self.caminho)?;
        self.bloqueios = recarregada.bloqueios;
        self.whitelist = recarregada.whitelist;
        self.lido_em = recarregada.lido_em;
        // As tentativas em memoria seguem: elas nao moram no arquivo.
        Ok(true)
    }

    /// A whitelist editavel pela tela, como esta no arquivo.
    pub fn whitelist(&self) -> &[String] {
        &self.whitelist
    }

    /// Substitui a whitelist editavel e grava. Recusa regra ilegivel INTEIRA,
    /// sem gravar nada: meia whitelist gravada e meia protecao, e quem clicou
    /// em salvar nao tem como saber qual metade valeu.
    pub fn definir_whitelist(&mut self, lista: Vec<String>) -> Result<()> {
        let limpa: Vec<String> = lista
            .into_iter()
            .map(|w| w.trim().to_string())
            .filter(|w| !w.is_empty())
            .collect();
        if let Some(ruim) = limpa.iter().find(|w| !regra_valida(w)) {
            return Err(phxsql_core::error::PhxError::Tipo(format!(
                "{ruim:?} nao e um IP nem uma faixa CIDR"
            )));
        }
        self.whitelist = limpa;
        self.gravar_e_marcar()
    }

    /// O IP esta protegido contra bloqueio? Uniao das duas whitelists: a fixa
    /// do `config.json` e a editavel deste arquivo. Whitelist vence SEMPRE --
    /// inclusive sobre um bloqueio ja gravado antes de a regra entrar.
    pub fn protegido(&self, politica: &Politica, ip: &str) -> bool {
        if politica.na_whitelist(ip) {
            return true;
        }
        let Ok(endereco) = ip.trim().parse::<IpAddr>() else {
            return false;
        };
        self.whitelist.iter().any(|r| regra_cobre_ip(r, &endereco))
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    pub fn lista(&self) -> &[Bloqueio] {
        &self.bloqueios
    }

    /// Bloqueios ainda em vigor, do mais recente para o mais antigo.
    pub fn ativos(&self, agora_ms: i64) -> Vec<&Bloqueio> {
        let mut v: Vec<&Bloqueio> = self
            .bloqueios
            .iter()
            .filter(|b| b.ativo_em(agora_ms))
            .collect();
        v.sort_by(|a, b| b.desde_ms.cmp(&a.desde_ms));
        v
    }

    /// O IP esta bloqueado agora?
    pub fn bloqueado(&self, ip: &str, agora_ms: i64) -> Option<&Bloqueio> {
        self.bloqueios
            .iter()
            .find(|b| b.ip == ip && b.ativo_em(agora_ms))
    }

    fn gravar(&self) -> Result<()> {
        let doc = Json::objeto(vec![
            ("_comentario", Json::texto_de(
                "Lista de bloqueio do PhxSql. Gerado pelo servidor; pode ser editado com o servidor parado.",
            )),
            ("atualizado_em", Json::texto_de(instante_iso(crate::agora_ms()))),
            (
                "whitelist",
                Json::Lista(self.whitelist.iter().map(Json::texto_de).collect()),
            ),
            (
                "bloqueios",
                Json::Lista(self.bloqueios.iter().map(Bloqueio::para_json).collect()),
            ),
        ]);
        // Pedido 598: pela troca duravel (temporario 0600 pelo motor da
        // permissao, `fsync`, `rename`, `fsync` da pasta), e nao NO LUGAR.
        // Regravado no lugar, uma queda entre o truncar e o escrever deixava
        // o arquivo vazio ou pela metade, e o `abrir` do arranque recusa JSON
        // torto -- o servidor nao subia por causa da lista de bloqueio. E a
        // queda logo depois da resposta podia devolver o IP ja desbloqueado
        // pelo administrador. Nenhum chamador segura a trava global de dados:
        // os portoes da porta tomam so a `lista_negra`.
        phxsql_store::sincronia::gravar_duravel(&self.caminho, doc.escrever_identado().as_bytes())
    }

    /// Grava e anota o carimbo, para nao reler a propria escrita.
    fn gravar_e_marcar(&mut self) -> Result<()> {
        self.gravar()?;
        self.lido_em = mtime(&self.caminho);
        Ok(())
    }

    /// Bloqueia um IP e grava. NAO toca no firewall: quem chama aplica a regra
    /// DEPOIS de soltar o mutex (`aplicar_no_firewall`), porque o comando pode
    /// demorar ate o prazo e `barrado()` pega este mutex em toda conexao
    /// (pedido 638). O bloqueio ja vale no servidor desde aqui.
    ///
    /// O aviso devolvido e so o da gravacao da lista.
    pub fn bloquear(
        &mut self,
        ip: &str,
        motivo: &str,
        comando: &str,
        tentativas: u32,
        politica: &Politica,
        agora_ms: i64,
    ) -> (Bloqueio, Option<String>) {
        let ate_ms = if politica.bloqueio_minutos == 0 {
            0
        } else {
            agora_ms + (politica.bloqueio_minutos as i64) * 60_000
        };

        let mut aviso = None;
        let bloqueio = Bloqueio {
            ip: ip.to_string(),
            desde_ms: agora_ms,
            ate_ms,
            motivo: motivo.to_string(),
            comando: comando.to_string(),
            tentativas,
            firewall: false,
        };

        self.bloqueios.retain(|b| b.ip != ip);
        self.bloqueios.push(bloqueio.clone());
        self.tentativas.remove(ip);
        self.tentativas_graves.remove(ip);
        if let Err(e) = self.gravar_e_marcar() {
            aviso = Some(format!("nao consegui gravar a blacklist: {e}"));
        }
        (bloqueio, aviso)
    }

    /// Anota que a regra de firewall do bloqueio `(ip, desde_ms)` foi aplicada.
    /// O `desde_ms` e a identidade: se o IP foi solto e bloqueado de novo no
    /// meio do comando, o bloqueio novo e outro e nao ganha a marca.
    pub fn marcar_firewall(&mut self, ip: &str, desde_ms: i64) -> Result<()> {
        let Some(b) = self
            .bloqueios
            .iter_mut()
            .find(|b| b.ip == ip && b.desde_ms == desde_ms)
        else {
            return Ok(());
        };
        b.firewall = true;
        self.gravar_e_marcar()
    }

    /// Tira o IP da lista. Devolve `true` se ele estava la. Como o `bloquear`,
    /// NAO toca no firewall: a regra sai por `soltar_no_firewall`, fora do
    /// mutex.
    pub fn desbloquear(&mut self, ip: &str) -> Result<bool> {
        let tinha = self.bloqueios.iter().any(|b| b.ip == ip);
        if !tinha {
            return Ok(false);
        }
        self.bloqueios.retain(|b| b.ip != ip);
        self.tentativas.remove(ip);
        self.tentativas_graves.remove(ip);
        self.gravar_e_marcar()?;
        Ok(true)
    }

    /// Registra uma violacao GRAVE.
    ///
    /// Com `tentativas_para_bloqueio: 1` -- o padrao, e o comportamento de
    /// sempre -- bloqueia na primeira. Acima de 1, conta dentro da janela e so
    /// bloqueia na enesima: a recusa da operacao acontece SEMPRE, quem muda e
    /// so o destino do IP. Whitelist vence tudo: recusa sem contar nada.
    pub fn violacao_grave(
        &mut self,
        ip: &str,
        comando: &str,
        motivo: &str,
        politica: &Politica,
        agora_ms: i64,
    ) -> Grave {
        if self.protegido(politica, ip) {
            return Grave::Protegido;
        }
        let limite = politica.tentativas_para_bloqueio.max(1);
        if limite <= 1 {
            let (b, aviso) = self.bloquear(ip, motivo, comando, 1, politica, agora_ms);
            return Grave::Bloqueado(b, aviso);
        }
        let janela = (politica.janela_minutos as i64) * 60_000;
        let recentes = self.tentativas_graves.entry(ip.to_string()).or_default();
        recentes.retain(|t| agora_ms - *t < janela);
        recentes.push(agora_ms);
        let quantas = recentes.len() as u32;
        if quantas >= limite {
            self.tentativas_graves.remove(ip);
            let (b, aviso) = self.bloquear(ip, motivo, comando, quantas, politica, agora_ms);
            Grave::Bloqueado(b, aviso)
        } else {
            Grave::Contada {
                tentativas: quantas,
                limite,
            }
        }
    }

    /// Registra uma tentativa LEVE. Bloqueia quando passar do limite dentro
    /// da janela. Devolve o bloqueio quando ele acontece.
    pub fn tentativa_leve(
        &mut self,
        ip: &str,
        comando: &str,
        motivo: &str,
        politica: &Politica,
        agora_ms: i64,
    ) -> Option<(Bloqueio, Option<String>)> {
        if self.protegido(politica, ip) {
            return None;
        }
        let janela = (politica.janela_minutos as i64) * 60_000;
        let recentes = self.tentativas.entry(ip.to_string()).or_default();
        recentes.retain(|t| agora_ms - *t < janela);
        recentes.push(agora_ms);
        let quantas = recentes.len() as u32;

        if quantas >= politica.tentativas_ate_bloquear {
            Some(self.bloquear(ip, motivo, comando, quantas, politica, agora_ms))
        } else {
            None
        }
    }

    /// Quantas tentativas leves recentes este IP tem.
    pub fn tentativas_de(&self, ip: &str) -> usize {
        self.tentativas.get(ip).map(Vec::len).unwrap_or(0)
    }

    /// Tira da lista os bloqueios ja vencidos. Devolve os IPs que saíram, para
    /// o chamador soltar a regra no firewall FORA do mutex (pedido 638: este
    /// metodo roda em `barrado()`, em toda conexao).
    pub fn limpar_vencidos(&mut self, agora_ms: i64) -> Result<Vec<String>> {
        let vencidos: Vec<String> = self
            .bloqueios
            .iter()
            .filter(|b| !b.ativo_em(agora_ms))
            .map(|b| b.ip.clone())
            .collect();
        if vencidos.is_empty() {
            return Ok(vencidos);
        }
        self.bloqueios.retain(|b| b.ativo_em(agora_ms));
        self.gravar_e_marcar()?;
        Ok(vencidos)
    }

    /// A blacklist num formato que um firewall de verdade consome.
    ///
    /// O servidor nao mexe em `iptables` sozinho -- isso exigiria root, e um
    /// banco de dados com root e mais superficie do que protecao. O que ele
    /// entrega e o texto pronto, uma linha por IP, para quem TEM o privilegio
    /// aplicar (`docs/SEGURANCA.md` mostra o comando de cada formato).
    ///
    /// So os bloqueios ATIVOS saem: exportar um vencido recriaria no firewall
    /// um bloqueio que o servidor ja soltou.
    pub fn exportar(&self, formato: &str, agora_ms: i64) -> Result<String> {
        let ativos = self.ativos(agora_ms);
        // So o que e endereco de verdade vira linha: o mesmo cuidado do
        // comando de firewall, agora no texto que alguem vai aplicar.
        let enderecos: Vec<(String, bool)> = ativos
            .iter()
            .filter_map(|b| {
                b.ip.parse::<IpAddr>()
                    .ok()
                    .map(|e| (b.ip.clone(), e.is_ipv6()))
            })
            .collect();
        let mut linhas = Vec::with_capacity(enderecos.len());
        match formato.trim().to_lowercase().as_str() {
            "texto" | "" => {
                for (ip, _) in &enderecos {
                    linhas.push(ip.clone());
                }
            }
            "iptables" => {
                for (ip, v6) in &enderecos {
                    let comando = if *v6 { "ip6tables" } else { "iptables" };
                    linhas.push(format!("{comando} -I INPUT -s {ip} -j DROP"));
                }
            }
            "nftables" => {
                for (ip, v6) in &enderecos {
                    let conjunto = if *v6 {
                        "phxsql_bloqueados6"
                    } else {
                        "phxsql_bloqueados"
                    };
                    linhas.push(format!("add element inet filter {conjunto} {{ {ip} }}"));
                }
            }
            "fail2ban" => {
                for (ip, _) in &enderecos {
                    linhas.push(format!("fail2ban-client set phxsql banip {ip}"));
                }
            }
            outro => {
                return Err(phxsql_core::error::PhxError::Esquema(format!(
                    "formato {outro:?} nao existe; use texto, iptables, nftables ou fail2ban"
                )))
            }
        }
        let mut texto = linhas.join("\n");
        if !texto.is_empty() {
            texto.push('\n');
        }
        Ok(texto)
    }
}

/// Carimbo de alteracao do arquivo, ou `None` se ele nao existe.
fn mtime(caminho: &Path) -> Option<SystemTime> {
    std::fs::metadata(caminho).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_temp(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("bl-{rotulo}"))
    }

    /* -------------------------------------------- proibido SO num banco

    O pedido 220: «`comandos_proibidos` e global, nao por banco — e o pedido
    era para um banco x». A forma nova entra na MESMA lista, como objeto, e o
    teste que mais importa e o do comportamento VELHO. */

    /// **O comportamento velho.** String solta continua proibindo em TODO
    /// banco — quem escreveu o `config.json` antes desta rodada nao muda de
    /// significado por causa dela.
    #[test]
    fn a_string_solta_continua_valendo_para_todos_os_bancos() {
        let j = Json::analisar(r#"{"comandos_proibidos":["reindexar","EXCLUIR"]}"#).unwrap();
        let p = Politica::de_json(&j);
        assert_eq!(p.comandos_proibidos, vec!["reindexar", "excluir"]);
        assert!(
            p.proibidos_por_base.is_empty(),
            "{:?}",
            p.proibidos_por_base
        );
        assert!(p.comando_proibido("reindexar"));
        assert!(p.comando_proibido("Reindexar"), "a caixa nao decide nada");
        // E, sem entrada por banco, a conferencia por banco nao proibe nada.
        assert!(!p.comando_proibido_na_base("reindexar", "financeiro"));
    }

    /// A forma nova: objeto na mesma lista, e ele so vale NAQUELE banco.
    #[test]
    fn o_objeto_proibe_so_no_banco_que_ele_nomeia() {
        let j = Json::analisar(
            r#"{"comandos_proibidos":[
                 "excluir_tabela",
                 {"comando":"Reindexar","database":"financeiro"}]}"#,
        )
        .unwrap();
        let p = Politica::de_json(&j);
        // O global continua sendo so a string.
        assert_eq!(p.comandos_proibidos, vec!["excluir_tabela"]);
        assert_eq!(
            p.proibidos_por_base,
            vec![("financeiro".to_string(), "reindexar".to_string())]
        );
        assert!(p.comando_proibido_na_base("reindexar", "financeiro"));
        // E o mesmo comando em OUTRO banco passa: se proibisse aqui tambem, a
        // forma por banco seria um global com nome enfeitado.
        assert!(!p.comando_proibido_na_base("reindexar", "loja"));
        assert!(!p.comando_proibido("reindexar"), "nao virou global");
        // Pedido sem banco nenhum nao casa com regra de banco.
        assert!(!p.comando_proibido_na_base("reindexar", ""));
    }

    /// Entrada pela metade e IGNORADA — nem vira global, nem vira do banco.
    #[test]
    fn entrada_pela_metade_nao_vira_proibicao_nenhuma() {
        let j = Json::analisar(
            r#"{"comandos_proibidos":[
                 {"comando":"reindexar"},
                 {"database":"financeiro"},
                 {"comando":"","database":"loja"}]}"#,
        )
        .unwrap();
        let p = Politica::de_json(&j);
        assert!(p.comandos_proibidos.is_empty());
        assert!(
            p.proibidos_por_base.is_empty(),
            "{:?}",
            p.proibidos_por_base
        );
    }

    fn politica() -> Politica {
        Politica {
            comandos_proibidos: vec!["excluir".into(), "reindexar".into()],
            bases_proibidas: vec!["financeiro".into()],
            tentativas_ate_bloquear: 3,
            janela_minutos: 10,
            bloqueio_minutos: 60,
            ..Politica::default()
        }
    }

    /// Desembrulha o caso que o teste espera: bloqueou.
    fn bloqueou(g: Grave) -> (Bloqueio, Option<String>) {
        match g {
            Grave::Bloqueado(b, aviso) => (b, aviso),
            outro => panic!("esperava bloqueio, veio {outro:?}"),
        }
    }

    const T0: i64 = 1_800_000_000_000;

    /// O filho que o teste de baixo roda debaixo do `strace`: um bloqueio e um
    /// desbloqueio, as duas gravacoes que a porta faz.
    #[cfg(unix)]
    #[test]
    #[ignore = "roda so dentro de a_lista_vai_ao_disco_pela_troca_duravel"]
    fn filho_da_lista_duravel() {
        let d = PathBuf::from(std::env::var("PHX_598_DIR").unwrap());
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        let (_, aviso) =
            bloqueou(bl.violacao_grave("203.0.113.9", "excluir", "comando proibido", &p, T0));
        assert!(aviso.is_none(), "{aviso:?}");
        assert!(bl.desbloquear("203.0.113.9").unwrap());
    }

    /// **Pedido 598, contra o sistema operacional:** cada gravacao do
    /// `blacklist.json` e temporario com `fsync` no descritor que o escreveu,
    /// `rename` para o nome final e `fsync` da pasta, NESTA ordem (`strace
    /// -y`). Era `escrever_do_banco` NO LUGAR, sem `fsync`: a queda no meio
    /// deixava o arquivo pela metade, e o arranque recusa JSON torto.
    ///
    /// **Defeito reposto** (o `escrever_do_banco` de antes): nao ha `rename`
    /// nenhum, e a contagem das trocas cai.
    ///
    /// **Nao medido:** a queda em si; o que se prova e a ordem e o descritor.
    #[cfg(unix)]
    #[test]
    fn a_lista_vai_ao_disco_pela_troca_duravel() {
        use std::process::Command;
        if Command::new("strace").arg("-V").output().is_err() {
            eprintln!("sem strace nesta maquina: a prova do 598 NAO MEDIDA");
            return;
        }
        let t = dir_temp("598-strace");
        let d = std::fs::canonicalize(&t).unwrap();
        let traco = d.join("traco.txt");
        let saida = Command::new("strace")
            .args([
                "-f",
                "-qq",
                "-y",
                "-e",
                "trace=openat,write,fsync,close,rename,renameat,renameat2",
                "-o",
            ])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "blacklist::tests::filho_da_lista_duravel",
                "--test-threads=1",
            ])
            .env("PHX_598_DIR", &d)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto
            .lines()
            .filter(|l| l.contains(" = ") && !l.contains("= -1"))
            .collect();
        let final_ = d.join("blacklist.json").display().to_string();
        let tmp = format!("{final_}.novo");
        let pasta = d.display().to_string();
        let trocas: Vec<usize> = linhas
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.contains("rename")
                    && l.contains(&format!("\"{tmp}\", "))
                    && l.contains(&format!("\"{final_}\""))
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            trocas.len(),
            2,
            "o bloqueio e o desbloqueio nao trocaram o arquivo por rename:\n{texto}"
        );
        let mut desde = 0;
        for i in trocas {
            let antes = &linhas[desde..i];
            let fd = antes
                .iter()
                .rev()
                .find(|l| l.contains("openat(") && l.contains(&format!("\"{tmp}\"")))
                .and_then(|l| l.rsplit("= ").next())
                // O `-y` imprime o retorno como `3</caminho>`: so o numero.
                .map(|s| s.split('<').next().unwrap_or("").trim().to_string())
                .unwrap_or_else(|| panic!("o temporario nao nasceu por openat:\n{texto}"));
            let escreveu = antes
                .iter()
                .position(|l| l.contains(&format!("write({fd}<{tmp}>")))
                .unwrap_or_else(|| panic!("nada escrito no temporario:\n{texto}"));
            assert!(
                antes[escreveu..]
                    .iter()
                    .any(|l| l.contains(&format!("fsync({fd}<{tmp}>)"))),
                "rename sem fsync do temporario no descritor que escreveu:\n{texto}"
            );
            let depois = &linhas[i + 1..];
            let fim = depois
                .iter()
                .position(|l| l.contains("rename"))
                .unwrap_or(depois.len());
            assert!(
                depois[..fim]
                    .iter()
                    .any(|l| l.contains("fsync(") && l.contains(&format!("<{pasta}>)"))),
                "rename sem fsync da pasta depois:\n{texto}"
            );
            desde = i + 1;
        }
    }

    #[test]
    fn comando_proibido_bloqueia_na_hora() {
        let d = dir_temp("grave");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        assert!(bl.bloqueado("203.0.113.9", T0).is_none());

        let (b, aviso) =
            bloqueou(bl.violacao_grave("203.0.113.9", "excluir", "comando proibido", &p, T0));
        assert!(aviso.is_none());
        assert_eq!(b.comando, "excluir");
        assert_eq!(b.tentativas, 1);
        assert!(bl.bloqueado("203.0.113.9", T0).is_some());
        // O bloqueio vence depois de 60 minutos.
        assert!(bl.bloqueado("203.0.113.9", T0 + 61 * 60_000).is_none());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A guarda pedida, e nao imposta: com `tentativas_para_bloqueio` acima
    /// de 1, a recusa continua na primeira, mas o bloqueio espera a enesima.
    #[test]
    fn tentativas_para_bloqueio_conta_antes_de_bloquear() {
        let d = dir_temp("grave-contada");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = Politica {
            tentativas_para_bloqueio: 3,
            ..politica()
        };
        match bl.violacao_grave("203.0.113.7", "excluir", "comando proibido", &p, T0) {
            Grave::Contada {
                tentativas: 1,
                limite: 3,
            } => {}
            outro => panic!("primeira deveria so contar, veio {outro:?}"),
        }
        assert!(bl.bloqueado("203.0.113.7", T0).is_none());
        match bl.violacao_grave("203.0.113.7", "excluir", "comando proibido", &p, T0 + 1_000) {
            Grave::Contada { tentativas: 2, .. } => {}
            outro => panic!("segunda deveria so contar, veio {outro:?}"),
        }
        let (b, _) = bloqueou(bl.violacao_grave(
            "203.0.113.7",
            "excluir",
            "comando proibido",
            &p,
            T0 + 2_000,
        ));
        assert_eq!(b.tentativas, 3);
        assert!(bl.bloqueado("203.0.113.7", T0 + 2_000).is_some());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Graves fora da janela nao contam -- a mesma regra das leves.
    #[test]
    fn tentativas_graves_fora_da_janela_nao_contam() {
        let d = dir_temp("grave-janela");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = Politica {
            tentativas_para_bloqueio: 2,
            ..politica()
        };
        bl.violacao_grave("203.0.113.8", "excluir", "x", &p, T0);
        // Onze minutos depois, a primeira saiu da janela de dez.
        match bl.violacao_grave("203.0.113.8", "excluir", "x", &p, T0 + 11 * 60_000) {
            Grave::Contada { tentativas: 1, .. } => {}
            outro => panic!("deveria recomecar a contagem, veio {outro:?}"),
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// **Whitelist nunca bloqueia** -- nem grave, nem leve. E o teste da prova
    /// real desta guarda: com a conferencia de `protegido` removida do
    /// `violacao_grave`, ele falha.
    #[test]
    fn whitelist_nunca_bloqueia() {
        let d = dir_temp("whitelist");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = Politica {
            whitelist: vec!["10.0.0.99".into()],
            ..politica()
        };
        match bl.violacao_grave("10.0.0.99", "excluir", "comando proibido", &p, T0) {
            Grave::Protegido => {}
            outro => panic!("whitelist deveria proteger, veio {outro:?}"),
        }
        assert!(bl.bloqueado("10.0.0.99", T0).is_none());
        // Nem cem tentativas leves bloqueiam quem esta na whitelist.
        for i in 0..100 {
            assert!(bl
                .tentativa_leve("10.0.0.99", "login", "senha errada", &p, T0 + i)
                .is_none());
        }
        assert!(bl.bloqueado("10.0.0.99", T0 + 200).is_none());
        // Quem NAO esta na whitelist continua bloqueando normalmente.
        bloqueou(bl.violacao_grave("10.0.0.98", "excluir", "x", &p, T0));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Whitelist vence ate um bloqueio ja gravado ANTES de a regra entrar:
    /// `protegido` e o que o servidor confere primeiro na conexao.
    #[test]
    fn whitelist_por_cidr_e_a_dinamica_do_arquivo() {
        let d = dir_temp("whitelist-cidr");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = Politica {
            whitelist: vec!["192.168.50.0/24".into(), "2001:db8::/32".into()],
            ..politica()
        };
        assert!(bl.protegido(&p, "192.168.50.20"));
        assert!(bl.protegido(&p, "2001:db8::7"));
        assert!(!bl.protegido(&p, "192.168.51.20"));
        assert!(!bl.protegido(&p, "2001:db9::7"));

        // A dinamica, gravada pelo arquivo, soma com a do config.
        bl.definir_whitelist(vec!["203.0.113.9".into()]).unwrap();
        assert!(bl.protegido(&p, "203.0.113.9"));
        // E persiste: outra abertura do arquivo ve a mesma lista.
        let bl2 = Blacklist::abrir(bl.caminho()).unwrap();
        assert_eq!(bl2.whitelist(), &["203.0.113.9".to_string()]);

        // Regra ilegivel nao grava NADA.
        let erro = bl.definir_whitelist(vec!["10.0.0.1".into(), "nao-e-ip".into()]);
        assert!(erro.is_err());
        assert_eq!(bl.whitelist(), &["203.0.113.9".to_string()]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn regra_cobre_ip_sabe_cidr_e_mapeado() {
        let v4 = "10.1.2.3".parse().unwrap();
        assert!(regra_cobre_ip("10.1.2.3", &v4));
        assert!(regra_cobre_ip("10.0.0.0/8", &v4));
        assert!(regra_cobre_ip("10.1.2.0/24", &v4));
        assert!(!regra_cobre_ip("10.1.3.0/24", &v4));
        assert!(regra_cobre_ip("0.0.0.0/0", &v4));
        assert!(!regra_cobre_ip("2001:db8::/32", &v4));
        // O endereco v4 mapeado em v6 e o MESMO endereco.
        let mapeado = "::ffff:10.1.2.3".parse().unwrap();
        assert!(regra_cobre_ip("10.0.0.0/8", &mapeado));
        let v6 = "2001:db8:1::9".parse().unwrap();
        assert!(regra_cobre_ip("2001:db8::/32", &v6));
        assert!(!regra_cobre_ip("10.0.0.0/8", &v6));
        // Lixo nao cobre nada.
        assert!(!regra_cobre_ip("", &v4));
        assert!(!regra_cobre_ip("10.0.0.0/33", &v4));
        assert!(!regra_cobre_ip("banana/8", &v4));

        assert!(regra_valida("127.0.0.1"));
        assert!(regra_valida("10.0.0.0/8"));
        assert!(regra_valida("::1"));
        assert!(regra_valida("2001:db8::/32"));
        assert!(!regra_valida("10.0.0.0/33"));
        assert!(!regra_valida("localhost"));
        assert!(!regra_valida(""));
    }

    /// A exportacao entrega o texto que um firewall DE VERDADE consome, uma
    /// linha por IP ativo -- vencido nao sai, e IPv6 vai para o comando v6.
    #[test]
    fn exportar_uma_linha_por_ip_ativo() {
        let d = dir_temp("exporta");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.violacao_grave("203.0.113.9", "excluir", "x", &p, T0);
        bl.violacao_grave("2001:db8::7", "excluir", "x", &p, T0);
        // Este vence antes do instante da exportacao: nao pode sair.
        bl.violacao_grave("198.51.100.1", "excluir", "x", &p, T0 - 61 * 60_000);

        let texto = bl.exportar("texto", T0).unwrap();
        assert!(texto.contains("203.0.113.9\n"));
        assert!(texto.contains("2001:db8::7\n"));
        assert!(!texto.contains("198.51.100.1"));

        let ipt = bl.exportar("iptables", T0).unwrap();
        assert!(ipt.contains("iptables -I INPUT -s 203.0.113.9 -j DROP\n"));
        assert!(ipt.contains("ip6tables -I INPUT -s 2001:db8::7 -j DROP\n"));

        let nft = bl.exportar("nftables", T0).unwrap();
        assert!(nft.contains("add element inet filter phxsql_bloqueados { 203.0.113.9 }\n"));
        assert!(nft.contains("add element inet filter phxsql_bloqueados6 { 2001:db8::7 }\n"));

        let f2b = bl.exportar("fail2ban", T0).unwrap();
        assert!(f2b.contains("fail2ban-client set phxsql banip 203.0.113.9\n"));

        assert!(bl.exportar("xml", T0).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn tentativa_leve_so_bloqueia_no_limite() {
        let d = dir_temp("leve");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        assert!(bl
            .tentativa_leve("10.0.0.5", "login", "senha errada", &p, T0)
            .is_none());
        assert!(bl
            .tentativa_leve("10.0.0.5", "login", "senha errada", &p, T0 + 1_000)
            .is_none());
        let bloqueou = bl.tentativa_leve("10.0.0.5", "login", "senha errada", &p, T0 + 2_000);
        let (b, _) = bloqueou.expect("a terceira tentativa deveria bloquear");
        assert_eq!(b.tentativas, 3);
        assert!(bl.bloqueado("10.0.0.5", T0 + 2_000).is_some());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn tentativas_fora_da_janela_nao_contam() {
        let d = dir_temp("janela");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.tentativa_leve("10.0.0.6", "login", "x", &p, T0);
        bl.tentativa_leve("10.0.0.6", "login", "x", &p, T0 + 1_000);
        // Onze minutos depois, as duas primeiras sairam da janela de dez.
        assert!(bl
            .tentativa_leve("10.0.0.6", "login", "x", &p, T0 + 11 * 60_000)
            .is_none());
        assert_eq!(bl.tentativas_de("10.0.0.6"), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_lista_sobrevive_ao_reinicio() {
        let d = dir_temp("persiste");
        let caminho = d.join("blacklist.json");
        let p = politica();
        {
            let mut bl = Blacklist::abrir(&caminho).unwrap();
            bl.violacao_grave("198.51.100.7", "reindexar", "comando proibido", &p, T0);
        }
        let bl = Blacklist::abrir(&caminho).unwrap();
        let b = bl
            .bloqueado("198.51.100.7", T0)
            .expect("deveria continuar bloqueado");
        assert_eq!(b.comando, "reindexar");
        assert_eq!(b.motivo, "comando proibido");
        // O arquivo e JSON legivel, com data e hora por extenso.
        let texto = std::fs::read_to_string(&caminho).unwrap();
        assert!(texto.contains("\"ip\": \"198.51.100.7\""));
        assert!(texto.contains("\"desde\":"));
        assert!(texto.contains("\"comando\": \"reindexar\""));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rele_o_arquivo_quando_outro_processo_mexe() {
        let d = dir_temp("recarrega");
        let caminho = d.join("blacklist.json");
        let p = politica();

        let mut servidor = Blacklist::abrir(&caminho).unwrap();
        servidor.violacao_grave("10.0.0.7", "excluir", "comando proibido", &p, T0);
        assert!(servidor.bloqueado("10.0.0.7", T0).is_some());

        // Outro processo -- o `phxsqld --desbloquear` -- tira o IP da lista.
        {
            let mut cli = Blacklist::abrir(&caminho).unwrap();
            assert!(cli.desbloquear("10.0.0.7").unwrap());
        }
        // Sem reler, o servidor continuaria barrando.
        assert!(
            servidor.recarregar_se_mudou().unwrap(),
            "deveria ter relido"
        );
        assert!(
            servidor.bloqueado("10.0.0.7", T0).is_none(),
            "o servidor tem de enxergar o desbloqueio feito de fora"
        );
        // Sem mudanca no arquivo, nao rele a toa.
        assert!(!servidor.recarregar_se_mudou().unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn nao_rele_a_propria_escrita() {
        let d = dir_temp("propria");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.violacao_grave("10.0.0.8", "excluir", "x", &p, T0);
        assert!(!bl.recarregar_se_mudou().unwrap(), "gravou ele mesmo");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn desbloquear_tira_da_lista() {
        let d = dir_temp("desbloqueia");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.violacao_grave("10.0.0.9", "excluir", "comando proibido", &p, T0);
        assert!(bl.desbloquear("10.0.0.9").unwrap());
        assert!(bl.bloqueado("10.0.0.9", T0).is_none());
        assert!(!bl.desbloquear("10.0.0.9").unwrap(), "ja nao estava la");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn limpar_vencidos() {
        let d = dir_temp("vencidos");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.violacao_grave("10.0.0.1", "excluir", "x", &p, T0);
        bl.violacao_grave("10.0.0.2", "excluir", "x", &p, T0);
        assert!(bl.limpar_vencidos(T0 + 1_000).unwrap().is_empty());
        assert_eq!(bl.limpar_vencidos(T0 + 61 * 60_000).unwrap().len(), 2);
        assert!(bl.lista().is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn politica_reconhece_proibidos_sem_ligar_para_caixa() {
        let p = politica();
        assert!(p.comando_proibido("excluir"));
        assert!(p.comando_proibido("EXCLUIR"));
        assert!(p.comando_proibido("  Excluir "));
        assert!(!p.comando_proibido("ler"));
        assert!(p.base_proibida("financeiro"));
        assert!(!p.base_proibida("Z"));
        assert!(!p.base_proibida(""));
    }

    /// Config que nao e lida mente: o `timeout_s` do firewall tem leitor,
    /// padrao e teto.
    #[test]
    fn o_prazo_do_firewall_vem_do_config_com_padrao_e_teto() {
        let f = |t: &str| {
            Firewall::de_json(&Json::analisar(&format!(r#"{{"bloquear":["x"]{t}}}"#)).unwrap())
                .unwrap()
                .timeout_s
        };
        assert_eq!(f(""), PRAZO_DO_FIREWALL_S);
        assert_eq!(f(r#","timeout_s":3"#), 3);
        assert_eq!(f(r#","timeout_s":99999"#), TETO_DO_PRAZO_DO_FIREWALL_S);
        assert_eq!(f(r#","timeout_s":0"#), 1);
    }

    #[test]
    fn firewall_desligado_nao_roda_nada() {
        let fw = Firewall {
            ligado: false,
            bloquear: vec!["/bin/false".into(), "{ip}".into()],
            desbloquear: vec![],
            timeout_s: 5,
        };
        assert!(!fw.bloquear_ip("10.0.0.1").unwrap());
    }

    #[test]
    fn firewall_recusa_o_que_nao_e_endereco() {
        let fw = Firewall {
            ligado: true,
            bloquear: vec!["/bin/true".into(), "{ip}".into()],
            desbloquear: vec![],
            timeout_s: 5,
        };
        // Endereco de verdade passa.
        assert!(fw.bloquear_ip("192.0.2.1").unwrap());
        assert!(fw.bloquear_ip("2001:db8::1").unwrap());
        // Qualquer outra coisa e recusada ANTES de virar argumento.
        for ruim in [
            "; rm -rf /",
            "10.0.0.1 && reboot",
            "$(whoami)",
            "",
            "localhost",
        ] {
            assert!(fw.bloquear_ip(ruim).is_err(), "deveria recusar {ruim:?}");
        }
    }

    fn politica_com_firewall(bloquear: Vec<String>, desbloquear: Vec<String>) -> Politica {
        Politica {
            firewall: Some(Firewall {
                ligado: true,
                bloquear,
                desbloquear,
                timeout_s: 1,
            }),
            ..politica()
        }
    }

    #[test]
    fn falha_do_firewall_nao_cancela_o_bloqueio() {
        let d = dir_temp("fw-falha");
        let bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica_com_firewall(
            vec!["/comando/que/nao/existe".into(), "{ip}".into()],
            vec![],
        );
        let lista = Mutex::new(bl);
        let (mut b, mut aviso) = bloqueou(lista.lock().unwrap().violacao_grave(
            "10.0.0.3",
            "excluir",
            "comando proibido",
            &p,
            T0,
        ));
        // A `Blacklist` nao executa nada: o bloqueio vale antes do firewall.
        assert!(aviso.is_none());
        assert!(lista.lock().unwrap().bloqueado("10.0.0.3", T0).is_some());
        aplicar_no_firewall(&lista, &p, &mut b, &mut aviso);
        assert!(aviso.is_some(), "a falha do firewall vira aviso");
        assert!(!b.firewall, "a regra nao chegou a ser aplicada");
        assert!(
            lista.lock().unwrap().bloqueado("10.0.0.3", T0).is_some(),
            "o servidor barra o IP mesmo sem o firewall"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Caminho feliz: a regra vale e a marca `firewall` vai ao ARQUIVO, que e
    /// o que o painel le.
    #[cfg(unix)]
    #[test]
    fn firewall_que_funciona_marca_o_bloqueio_no_arquivo() {
        let d = dir_temp("fw-ok");
        let caminho = d.join("blacklist.json");
        let p = politica_com_firewall(vec!["/bin/true".into(), "{ip}".into()], vec![]);
        let lista = Mutex::new(Blacklist::abrir(&caminho).unwrap());
        let (mut b, mut aviso) = bloqueou(lista.lock().unwrap().violacao_grave(
            "10.0.0.4",
            "excluir",
            "comando proibido",
            &p,
            T0,
        ));
        assert!(!b.firewall, "a Blacklist sozinha nunca diz que aplicou");
        aplicar_no_firewall(&lista, &p, &mut b, &mut aviso);
        assert!(b.firewall && aviso.is_none(), "{aviso:?}");
        let relida = Blacklist::abrir(&caminho).unwrap();
        assert!(relida.lista()[0].firewall, "a marca nao foi ao arquivo");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// **Pedido 638.** O texto que o programa imprime no `stderr` e no
    /// `stdout` nao entra no erro (que vai a log e a tela), o prazo mata o que
    /// pendura e o ambiente do servidor nao atravessa.
    ///
    /// Defeito reposto (`Command::output()` e `stderr` dentro do erro): a
    /// sentinela aparece no aviso, o `sleep` segura o teste por 30 s e o
    /// `CARGO_*` chega ao filho.
    #[cfg(unix)]
    #[test]
    fn o_firewall_roda_pelo_motor_do_gancho() {
        use crate::gancho::apoio_de_teste::{dir, script};
        let apoio = dir("fw");
        let ip = "192.0.2.7";

        // 1. Saida do filho nao volta no erro.
        let ruim = script(
            &apoio,
            "ruim.sh",
            "echo SEGREDO-NO-STDOUT; echo SEGREDO-NO-STDERR >&2; exit 3",
        );
        let fw = Firewall {
            ligado: true,
            bloquear: vec![ruim, "{ip}".into()],
            desbloquear: vec![],
            timeout_s: 5,
        };
        let e = fw.bloquear_ip(ip).unwrap_err().to_string();
        assert!(e.contains("codigo 3"), "{e}");
        assert!(!e.contains("SEGREDO"), "a saida do filho vazou: {e}");

        // 2. Ambiente limpo e o IP chegando LITERAL como argumento.
        assert!(std::env::vars().any(|(k, _)| k.starts_with("CARGO")));
        let saida = apoio.join("saida.txt");
        let bom = script(
            &apoio,
            "bom.sh",
            &format!("{{ echo \"$1\"; env | sort; }} > {}", saida.display()),
        );
        let fw = Firewall {
            ligado: true,
            bloquear: vec![bom, "{ip}".into()],
            desbloquear: vec![],
            timeout_s: 5,
        };
        assert!(fw.bloquear_ip(ip).unwrap());
        let t = std::fs::read_to_string(&saida).unwrap();
        assert!(t.starts_with("192.0.2.7\n"), "{t}");
        assert!(!t.contains("CARGO"), "o ambiente do servidor vazou: {t}");
        assert!(t.contains("PATH=/usr/local/sbin"), "{t}");

        // 3. O que pendura leva kill no prazo (e e colhido).
        let pidfile = apoio.join("pid");
        let pendura = script(
            &apoio,
            "pendura.sh",
            &format!("echo $$ > {}; exec sleep 30", pidfile.display()),
        );
        let fw = Firewall {
            ligado: true,
            bloquear: vec![pendura],
            desbloquear: vec![],
            timeout_s: 1,
        };
        let t0 = std::time::Instant::now();
        let e = fw.bloquear_ip(ip).unwrap_err().to_string();
        assert!(
            t0.elapsed() < std::time::Duration::from_secs(5),
            "{:?}",
            t0.elapsed()
        );
        assert!(e.contains("foi morto"), "{e}");
        let pid = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .to_string();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "o filho {pid} continua na tabela de processos"
        );
    }

    /// O mutex nao fica preso durante o comando: com um firewall que dorme,
    /// outro fio toma a lista em milissegundos enquanto `aplicar_no_firewall`
    /// ainda roda. Defeito reposto (exec dentro do `bloquear`): o `lock` do
    /// observador espera o prazo inteiro.
    #[cfg(unix)]
    #[test]
    fn a_lista_fica_livre_enquanto_o_firewall_roda() {
        let d = dir_temp("fw-livre");
        let p = politica_com_firewall(vec!["/bin/sleep".into(), "30".into()], vec![]);
        let lista = std::sync::Arc::new(Mutex::new(
            Blacklist::abrir(d.join("blacklist.json")).unwrap(),
        ));
        let (mut b, mut aviso) = bloqueou(lista.lock().unwrap().violacao_grave(
            "10.0.0.5",
            "excluir",
            "comando proibido",
            &p,
            T0,
        ));
        let l2 = std::sync::Arc::clone(&lista);
        let p2 = p.clone();
        let fio = std::thread::spawn(move || aplicar_no_firewall(&l2, &p2, &mut b, &mut aviso));
        std::thread::sleep(std::time::Duration::from_millis(200));
        let t0 = std::time::Instant::now();
        assert!(lista.lock().unwrap().bloqueado("10.0.0.5", T0).is_some());
        assert!(
            t0.elapsed() < std::time::Duration::from_millis(500),
            "a lista ficou presa pelo firewall: {:?}",
            t0.elapsed()
        );
        fio.join().unwrap();
        std::fs::remove_dir_all(&d).unwrap();
    }
}
