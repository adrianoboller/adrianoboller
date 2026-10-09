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
    /// O loopback nunca entra na lista (pedido 766, P9).
    ///
    /// Nasce LIGADO, e o caso que o motivou foi medido em 09/10/2026: cinco
    /// logins com token errado vindos de `127.0.0.1` bloquearam o proprio
    /// loopback por 60 minutos -- a tela, a TV e o `phxsqld --desbloquear`
    /// que o operador rodaria na mesma maquina. E a convergencia de quem ja
    /// apanhou disso: o MySQL nao poe o loopback no *host cache* do
    /// `max_connect_errors`, e o fail2ban nasce com `ignoreself`. A tentativa
    /// continua contando, e a forca bruta continua virando ocorrencia; o que
    /// some e so a porta trancada para quem esta na propria maquina.
    ///
    /// Desligar (`"poupar_loopback": false`) e o comportamento de antes, e e
    /// o que os testes de soquete usam para provar o bloqueio -- todo teste de
    /// soquete chega de `127.0.0.1`.
    pub poupar_loopback: bool,
    /// Escalonar o prazo do bloqueio pela reincidencia (pedido 766, P10):
    /// `bloqueio_minutos x 2^n`, com `n` os bloqueios do IP nos ultimos 30
    /// dias, `n <= 20` como o `bantime.increment` do fail2ban, e teto de 7
    /// dias. Nasce LIGADO pela decisao do dono de 09/10/2026 (765/766: o
    /// padrao de fabrica e proteger); desligado, todo bloqueio dura
    /// `bloqueio_minutos`, como antes.
    pub escalonar: bool,
}

/// O expoente maximo do escalonamento: o `ban.Count < 20` do fail2ban.
pub const TETO_DO_EXPOENTE: u32 = 20;
/// O teto do prazo escalonado: 7 dias. Nunca ENCURTA um `bloqueio_minutos`
/// maior que ele -- o teto e da multiplicacao, nao do que o administrador
/// pediu.
pub const TETO_DO_ESCALONADO_MIN: u64 = 7 * 24 * 60;
/// Quanto tempo um bloqueio conta para o escalonamento: 30 dias.
pub const JANELA_DA_REINCIDENCIA_MS: i64 = 30 * 24 * 3_600_000;
/// Quantos IPs o historico de reincidencia guarda. Quem varia o IP nao faz o
/// `blacklist.json` crescer sem fim: passou disto, sai o IP cujo ultimo
/// bloqueio e o mais velho.
pub const TETO_DO_HISTORICO: usize = 50_000;

/// O IP na forma CANONICA: `::ffff:a.b.c.d` vira `a.b.c.d` (pedido 766,
/// P11). E a forma que vai ao firewall, ao historico e a memoria de IPs: o
/// mesmo endereco chegando por um soquete dual-stack nao pode virar duas
/// chaves, nem chegar ao `iptables` na forma v6. `None` para o que nao e
/// endereco -- e o que nao e endereco nao vai a lugar nenhum.
pub fn ip_canonico(ip: &str) -> Option<IpAddr> {
    ip.parse::<IpAddr>().ok().map(|e| e.to_canonical())
}

/// Por que um IP que passou do limite NAO foi bloqueado (pedido 766, P9).
///
/// As quatro guardas de nao se trancar para fora. Elas so decidem o DESTINO
/// do IP: a operacao continua recusada, a tentativa continua contada e a
/// forca bruta continua virando ocorrencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guarda {
    /// A whitelist fixa do `config.json` ou a editavel da tela.
    Whitelist,
    /// `127.0.0.0/8` ou `::1`, com `poupar_loopback` ligado.
    Loopback,
    /// Um administrador entrou deste IP nas ultimas 24 h: bloquea-lo
    /// trancaria para fora justamente quem solta o bloqueio.
    Administrador,
    /// Dois usuarios distintos entraram deste IP em 30 dias: e um NAT ou um
    /// proxy, e bloquea-lo derrubaria quem nao fez nada.
    Compartilhado,
}

impl Guarda {
    pub fn nome(self) -> &'static str {
        match self {
            Guarda::Whitelist => "whitelist",
            Guarda::Loopback => "loopback",
            Guarda::Administrador => "administrador",
            Guarda::Compartilhado => "compartilhado",
        }
    }
}

/// O que uma tentativa LEVE rendeu.
#[derive(Debug)]
pub enum Leve {
    /// Contou, e ainda nao chegou ao limite.
    Contada(u32),
    /// Chegou ao limite, e uma guarda poupou o IP. A conta recomeca.
    Poupada { guarda: Guarda, tentativas: u32 },
    /// Chegou ao limite e bloqueou.
    Bloqueada(Bloqueio, Option<String>),
}

impl Leve {
    /// Chegou ao limite -- bloqueando ou nao. E a «tentativa seguida» que a
    /// forca bruta conta.
    pub fn chegou_ao_limite(&self) -> bool {
        !matches!(self, Leve::Contada(_))
    }
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
            poupar_loopback: true,
            escalonar: true,
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
            poupar_loopback: j.booleano_ou("poupar_loopback", padrao.poupar_loopback),
            escalonar: j.booleano_ou("escalonar", padrao.escalonar),
        }
    }

    /// Quantos minutos dura o bloqueio de um IP com `anteriores` bloqueios
    /// nos ultimos 30 dias. Zero continua sendo «permanente».
    pub fn minutos_do_bloqueio(&self, anteriores: u32) -> u64 {
        let base = self.bloqueio_minutos;
        if base == 0 || !self.escalonar {
            return base;
        }
        let fator = 1u64 << anteriores.min(TETO_DO_EXPOENTE);
        base.saturating_mul(fator)
            .min(TETO_DO_ESCALONADO_MIN.max(base))
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

/// Comando de firewall, como lista de argumentos.
///
/// # Os marcadores (pedido 766, P11)
///
/// * `{ip}` -- o endereco REESCRITO a partir do `IpAddr` analisado, na forma
///   canonica: nunca o texto que chegou. E esta a barreira contra injecao, e
///   nao a falta de shell: o `nft` junta o argv numa linha e a analisa de
///   novo, entao `127.0.0.3 }; delete table inet x` passaria inteiro por um
///   argv sem shell nenhum (medido, M4 do desenho).
/// * `{familia}` -- `4` ou `6`, para o conjunto certo (`negros{familia}`).
/// * `{segundos}` -- o prazo que falta do bloqueio, inteiro; `0` no
///   permanente e no desbloquear. E o `timeout` do conjunto com prazo no
///   kernel: a regra se desfaz sozinha mesmo com o PhxSql morto.
#[derive(Debug, Clone)]
pub struct Firewall {
    pub ligado: bool,
    pub bloquear: Vec<String>,
    pub desbloquear: Vec<String>,
    /// O comando que LISTA o que esta no firewall, para a reconciliacao do
    /// arranque. A saida e analisada token a token como `IpAddr`; o que nao
    /// e endereco e ignorado. Vazio: sem reconciliacao.
    pub listar: Vec<String>,
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
            listar: j.textos("listar"),
            timeout_s: (j.inteiro_ou("timeout_s", PRAZO_DO_FIREWALL_S as i64).max(1) as u64)
                .min(TETO_DO_PRAZO_DO_FIREWALL_S),
        };
        if f.bloquear.is_empty() {
            None
        } else {
            Some(f)
        }
    }

    /// O firewall ligado aponta para programas que existem, por caminho
    /// absoluto, e que so root ou o usuario do servidor podem trocar? E a
    /// MESMA conferencia do gancho (`gancho::conferir_programa_de`), no
    /// arranque: o `PATH` do servidor nao escolhe qual `nft` roda como quem
    /// tem o direito de mexer no firewall.
    pub fn validar(&self) -> std::result::Result<(), String> {
        if !self.ligado {
            return Ok(());
        }
        for (campo, argv) in [
            ("seguranca.firewall.bloquear", &self.bloquear),
            ("seguranca.firewall.desbloquear", &self.desbloquear),
            ("seguranca.firewall.listar", &self.listar),
        ] {
            let Some(programa) = argv.first() else {
                continue;
            };
            if argv.len() > 32 || argv.iter().any(|a| a.len() > 1024 || a.contains('\0')) {
                return Err(format!(
                    "{campo}: no maximo 32 itens de 1024 bytes, sem byte nulo"
                ));
            }
            crate::gancho::conferir_programa_de(&format!("{campo}[0]"), programa)?;
        }
        Ok(())
    }

    /// Roda o comando, trocando os marcadores (ver [`Firewall`]).
    ///
    /// Devolve `Ok(false)` quando o firewall esta desligado. Sem shell: cada
    /// argumento vai inteiro, e o IP so entra REESCRITO do endereco
    /// analisado.
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
    pub fn aplicar(&self, argumentos: &[String], ip: &str, segundos: u64) -> Result<bool> {
        if !self.ligado || argumentos.is_empty() {
            return Ok(false);
        }
        let trocado = trocar_marcadores(argumentos, ip, segundos)?;
        crate::gancho::rodar(&self.execucao(&trocado, 0)).map_err(|e| {
            phxsql_core::error::PhxError::Corrompido(format!("o comando de firewall falhou: {e}"))
        })?;
        Ok(true)
    }

    fn execucao<'a>(
        &self,
        argv: &'a [String],
        teto_da_saida: usize,
    ) -> crate::gancho::Execucao<'a> {
        crate::gancho::Execucao {
            rotulo: "comando de firewall",
            argv,
            path: PATH_DO_FIREWALL,
            ambiente: &[],
            entrada: None,
            prazo_s: self.timeout_s,
            teto_da_saida,
        }
    }

    pub fn bloquear_ip(&self, ip: &str, segundos: u64) -> Result<bool> {
        self.aplicar(&self.bloquear, ip, segundos)
    }

    pub fn desbloquear_ip(&self, ip: &str) -> Result<bool> {
        self.aplicar(&self.desbloquear, ip, 0)
    }

    /// O que o firewall do SO diz que esta bloqueado, pelo comando `listar`.
    /// `None` quando nao ha o que listar (desligado ou sem comando).
    ///
    /// A saida e ANALISADA, nunca usada como texto: separada nos espacos e
    /// na pontuacao do `nft`/`ipset`/`netsh`, e so o pedaco que e um
    /// `IpAddr` inteiro conta -- `timeout`, `2m`, `10.0.0.0/8` e nome de
    /// conjunto caem fora. Sem duplicata, na forma canonica.
    pub fn listar_ips(&self) -> Result<Option<Vec<IpAddr>>> {
        if !self.ligado || self.listar.is_empty() {
            return Ok(None);
        }
        let saida = crate::gancho::rodar_e_ler(
            &self.execucao(&self.listar, crate::gancho::TETO_DA_SAIDA_LIDA),
        )
        .map_err(|e| {
            phxsql_core::error::PhxError::Corrompido(format!(
                "o comando de firewall (listar) falhou: {e}"
            ))
        })?;
        Ok(Some(ips_da_listagem(&saida)))
    }
}

/// Os marcadores trocados. O IP entra so depois de analisado, e REESCRITO da
/// forma canonica -- o texto que chegou nunca vai ao argv.
fn trocar_marcadores(argumentos: &[String], ip: &str, segundos: u64) -> Result<Vec<String>> {
    let Some(endereco) = ip_canonico(ip) else {
        return Err(phxsql_core::error::PhxError::Tipo(format!(
            "{ip:?} nao e um endereco IP; nao vai para o firewall"
        )));
    };
    let texto = endereco.to_string();
    let familia = if endereco.is_ipv6() { "6" } else { "4" };
    let segundos = segundos.to_string();
    Ok(argumentos
        .iter()
        .map(|a| {
            a.replace("{ip}", &texto)
                .replace("{familia}", familia)
                .replace("{segundos}", &segundos)
        })
        .collect())
}

/// Os enderecos que aparecem numa listagem de firewall, analisados token a
/// token. Ver [`Firewall::listar_ips`].
pub fn ips_da_listagem(saida: &str) -> Vec<IpAddr> {
    let mut vistos: Vec<IpAddr> = Vec::new();
    let separa = |c: char| c.is_whitespace() || matches!(c, ',' | '{' | '}' | ';' | '"' | '=');
    for pedaco in saida.split(separa) {
        if let Ok(ip) = pedaco.parse::<IpAddr>() {
            let ip = ip.to_canonical();
            if !vistos.contains(&ip) {
                vistos.push(ip);
            }
        }
    }
    vistos
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

    /// O prazo que falta, em segundos inteiros arredondados para CIMA -- o
    /// `{segundos}` do firewall. Zero no permanente: o conjunto sem prazo e
    /// decisao de quem escreveu o comando, nao um numero inventado aqui.
    pub fn segundos_restantes(&self, agora_ms: i64) -> u64 {
        if self.ate_ms == 0 {
            return 0;
        }
        let ms = (self.ate_ms - agora_ms).max(0) as u64;
        ms.div_ceil(1_000).max(1)
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
    // O alarme do bloqueio (pedido 769, P0 do 765). AQUI, e nao no
    // `Blacklist::bloquear`: este e o ponto por onde os dois irmaos -- a
    // violacao grave e a leve -- passam com o bloqueio recem-gravado e a lista
    // ja SOLTA, e o produtor nao entra no mutex que o `barrado()` de toda
    // conexao pega. E ANTES do `firewall` opcional: o bloqueio que vale e o do
    // servidor, com ou sem regra no sistema operacional.
    crate::telemetria::sinal(crate::aquario::Alarme::FirewallBloqueou, &b.motivo);
    let Some(fw) = &politica.firewall else {
        return;
    };
    match fw.bloquear_ip(&b.ip, b.segundos_restantes(crate::agora_ms())) {
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

/// O que a reconciliacao do arranque fez (pedido 766, P11).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reconciliacao {
    /// Ativos na lista e ausentes no SO (o reboot esvaziou o conjunto, ou o
    /// processo morreu antes do `add`): voltaram, com o prazo que falta.
    pub acrescentados: Vec<String>,
    /// No SO e sem bloqueio ativo na lista (o PhxSql morreu antes do `del`,
    /// ou o bloqueio venceu com ele parado): sairam.
    pub removidos: Vec<String>,
    /// As falhas de comando, uma frase por IP; a reconciliacao nao para nelas.
    pub falhas: Vec<String>,
}

/// Reconcilia o firewall do SO com a lista, no arranque (pedido 766, P11).
///
/// `Ok(None)` quando nao ha o que reconciliar: firewall desligado ou sem o
/// comando `listar`. O SO e a fonte do que ESTA aplicado; a lista e a fonte
/// do que DEVERIA estar. A comparacao e pelo IP canonico, e o que o SO lista
/// so conta se for um `IpAddr` inteiro ([`ips_da_listagem`]).
///
/// Pelo `&Mutex`, e a lista so na mao para tirar o retrato dos ativos e para
/// marcar o que voltou: os comandos rodam com ela SOLTA, pelo motivo do
/// pedido 638.
///
/// A whitelist e o loopback poupado nunca voltam ao firewall, mesmo com um
/// bloqueio antigo na lista: o `barrado()` ja os deixa entrar, e regra no SO
/// trancaria por fora o que o servidor solta por dentro.
pub fn reconciliar_firewall(
    lista: &Mutex<Blacklist>,
    politica: &Politica,
    agora_ms: i64,
) -> Result<Option<Reconciliacao>> {
    let Some(fw) = &politica.firewall else {
        return Ok(None);
    };
    let Some(no_so) = fw.listar_ips()? else {
        return Ok(None);
    };
    let ativos: Vec<(IpAddr, Bloqueio)> = {
        let Ok(l) = lista.lock() else {
            return Ok(None);
        };
        l.ativos(agora_ms)
            .into_iter()
            .filter(|b| l.guarda(politica, &b.ip, || None).is_none())
            .filter_map(|b| ip_canonico(&b.ip).map(|ip| (ip, b.clone())))
            .collect()
    };
    let mut r = Reconciliacao::default();
    for ip in &no_so {
        if ativos.iter().any(|(a, _)| a == ip) {
            continue;
        }
        let texto = ip.to_string();
        match fw.desbloquear_ip(&texto) {
            Ok(_) => r.removidos.push(texto),
            Err(e) => r.falhas.push(format!("{texto}: {e}")),
        }
    }
    for (ip, b) in &ativos {
        if no_so.contains(ip) {
            continue;
        }
        match fw.bloquear_ip(&b.ip, b.segundos_restantes(agora_ms)) {
            Ok(true) => {
                if let Ok(mut l) = lista.lock() {
                    if let Err(e) = l.marcar_firewall(&b.ip, b.desde_ms) {
                        r.falhas.push(format!("{}: {e}", b.ip));
                    }
                }
                r.acrescentados.push(ip.to_string());
            }
            Ok(false) => {}
            Err(e) => r.falhas.push(format!("{ip}: {e}")),
        }
    }
    Ok(Some(r))
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
    /// Os bloqueios de cada IP nos ultimos 30 dias, pelo instante (pedido
    /// 766, P10): o `n` do escalonamento. Mora no ARQUIVO, e nao em memoria
    /// como as tentativas: reiniciar o servidor nao pode devolver a quem ja
    /// foi bloqueado dez vezes o prazo de quem foi bloqueado uma. Sobrevive
    /// ao `desbloquear` pelo mesmo motivo -- soltar e perdoar ESTE bloqueio,
    /// nao a reincidencia.
    historico: HashMap<String, Vec<i64>>,
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
        let (bloqueios, whitelist, historico) = match std::fs::read_to_string(&caminho) {
            Err(_) => (Vec::new(), Vec::new(), HashMap::new()),
            Ok(texto) => match Json::analisar(&texto) {
                Err(_) => (Vec::new(), Vec::new(), HashMap::new()),
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
                    historico_de_json(j.campo("historico")),
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
            historico,
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
        self.historico = recarregada.historico;
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
    ///
    /// Desde o pedido 766 (P9) o loopback poupado entra aqui tambem: um
    /// `127.0.0.1` bloqueado ANTES da guarda existir volta a entrar na
    /// proxima conexao, sem esperar o bloqueio vencer.
    pub fn protegido(&self, politica: &Politica, ip: &str) -> bool {
        self.guarda(politica, ip, || None).is_some()
    }

    /// A guarda que poupa este IP, se alguma -- o ponto UNICO da decisao «nao
    /// se trancar para fora» (pedido 766, P9).
    ///
    /// As duas guardas fixas moram aqui (whitelist e loopback); as duas que
    /// dependem de quem ENTROU deste IP (administrador, compartilhado) vem
    /// de fora em `externa`, porque a memoria de IPs vistos e do servidor --
    /// e entram depois das fixas, que sao as que o operador escreveu.
    pub fn guarda(
        &self,
        politica: &Politica,
        ip: &str,
        externa: impl FnOnce() -> Option<Guarda>,
    ) -> Option<Guarda> {
        if politica.na_whitelist(ip) {
            return Some(Guarda::Whitelist);
        }
        if let Ok(endereco) = ip.trim().parse::<IpAddr>() {
            if self.whitelist.iter().any(|r| regra_cobre_ip(r, &endereco)) {
                return Some(Guarda::Whitelist);
            }
            if politica.poupar_loopback && endereco.to_canonical().is_loopback() {
                return Some(Guarda::Loopback);
            }
        }
        externa()
    }

    /// Quantos bloqueios este IP teve nos ultimos 30 dias -- o `n` do
    /// escalonamento.
    pub fn bloqueios_anteriores(&self, ip: &str, agora_ms: i64) -> u32 {
        let chave = chave_do_historico(ip);
        self.historico.get(&chave).map_or(0, |v| {
            v.iter()
                .filter(|t| agora_ms - **t < JANELA_DA_REINCIDENCIA_MS)
                .count() as u32
        })
    }

    /// Anota um bloqueio no historico, com as podas: so os 30 dias, no
    /// maximo `TETO_DO_EXPOENTE + 1` instantes por IP (mais que isso nao
    /// muda o prazo), e no maximo `TETO_DO_HISTORICO` IPs.
    fn anotar_no_historico(&mut self, ip: &str, agora_ms: i64) {
        let chave = chave_do_historico(ip);
        let v = self.historico.entry(chave.clone()).or_default();
        v.retain(|t| agora_ms - *t < JANELA_DA_REINCIDENCIA_MS);
        v.push(agora_ms);
        let sobra = v.len().saturating_sub(TETO_DO_EXPOENTE as usize + 1);
        v.drain(..sobra);
        if self.historico.len() > TETO_DO_HISTORICO {
            self.historico.retain(|_, v| {
                v.retain(|t| agora_ms - *t < JANELA_DA_REINCIDENCIA_MS);
                !v.is_empty()
            });
        }
        while self.historico.len() > TETO_DO_HISTORICO {
            let Some(velho) = self
                .historico
                .iter()
                .filter(|(k, _)| **k != chave)
                .min_by_key(|(_, v)| v.last().copied().unwrap_or(0))
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.historico.remove(&velho);
        }
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
            ("historico", historico_para_json(&self.historico)),
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
        // O escalonamento (pedido 766, P10): o prazo sai da reincidencia
        // ANTES de anotar este bloqueio, entao o primeiro dura a base.
        let minutos = politica.minutos_do_bloqueio(self.bloqueios_anteriores(ip, agora_ms));
        let ate_ms = if minutos == 0 {
            0
        } else {
            agora_ms.saturating_add((minutos as i64).saturating_mul(60_000))
        };
        self.anotar_no_historico(ip, agora_ms);

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
        self.violacao_grave_guardada(ip, comando, motivo, politica, agora_ms, || None)
    }

    /// A [`Blacklist::violacao_grave`] com as guardas que dependem de quem
    /// entrou deste IP (pedido 766, P9). E esta que o servidor chama.
    pub fn violacao_grave_guardada(
        &mut self,
        ip: &str,
        comando: &str,
        motivo: &str,
        politica: &Politica,
        agora_ms: i64,
        externa: impl FnOnce() -> Option<Guarda>,
    ) -> Grave {
        if self.guarda(politica, ip, externa).is_some() {
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
        match self.tentativa_leve_guardada(ip, comando, motivo, politica, agora_ms, || None) {
            Leve::Bloqueada(b, aviso) => Some((b, aviso)),
            _ => None,
        }
    }

    /// A [`Blacklist::tentativa_leve`] com as guardas (pedido 766, P9). E
    /// esta que o servidor chama.
    ///
    /// # O IP poupado CONTA
    ///
    /// Antes desta guarda, o protegido saia sem contar, e a forca bruta que
    /// vinha da whitelist era invisivel. Agora conta como todo mundo, e ao
    /// chegar ao limite devolve [`Leve::Poupada`] em vez de bloquear: quem
    /// chama transforma isso em ocorrencia, e a conta recomeca.
    pub fn tentativa_leve_guardada(
        &mut self,
        ip: &str,
        comando: &str,
        motivo: &str,
        politica: &Politica,
        agora_ms: i64,
        externa: impl FnOnce() -> Option<Guarda>,
    ) -> Leve {
        let janela = (politica.janela_minutos as i64) * 60_000;
        let recentes = self.tentativas.entry(ip.to_string()).or_default();
        recentes.retain(|t| agora_ms - *t < janela);
        recentes.push(agora_ms);
        let quantas = recentes.len() as u32;
        if quantas < politica.tentativas_ate_bloquear {
            return Leve::Contada(quantas);
        }
        if let Some(guarda) = self.guarda(politica, ip, externa) {
            self.tentativas.remove(ip);
            return Leve::Poupada {
                guarda,
                tentativas: quantas,
            };
        }
        let (b, aviso) = self.bloquear(ip, motivo, comando, quantas, politica, agora_ms);
        Leve::Bloqueada(b, aviso)
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

/// A chave do historico: o IP canonico, para o `::ffff:` nao ser outro IP;
/// o texto como veio quando nao e endereco (nao deveria chegar aqui).
fn chave_do_historico(ip: &str) -> String {
    ip_canonico(ip).map_or_else(|| ip.to_string(), |e| e.to_string())
}

/// `{"203.0.113.9":[ms, ms], ...}` -- o historico como vai ao arquivo.
fn historico_para_json(h: &HashMap<String, Vec<i64>>) -> Json {
    let mut chaves: Vec<&String> = h.keys().collect();
    chaves.sort();
    Json::Objeto(
        chaves
            .into_iter()
            .map(|k| {
                (
                    k.clone(),
                    Json::Lista(h[k].iter().map(|t| Json::Numero(*t as f64)).collect()),
                )
            })
            .collect(),
    )
}

/// O historico lido de volta. Campo ausente (arquivo de antes do 766) e
/// historico vazio: todo IP comeca do zero, que e o comportamento de antes.
fn historico_de_json(j: Option<&Json>) -> HashMap<String, Vec<i64>> {
    let Some(Json::Objeto(pares)) = j else {
        return HashMap::new();
    };
    pares
        .iter()
        .filter_map(|(k, v)| {
            let instantes: Vec<i64> = v
                .lista()?
                .iter()
                .filter_map(Json::numero)
                .map(|n| n as i64)
                .collect();
            (!instantes.is_empty()).then(|| (k.clone(), instantes))
        })
        .collect()
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
            listar: vec![],
            timeout_s: 5,
        };
        assert!(!fw.bloquear_ip("10.0.0.1", 60).unwrap());
    }

    #[test]
    fn firewall_recusa_o_que_nao_e_endereco() {
        let fw = Firewall {
            ligado: true,
            bloquear: vec!["/bin/true".into(), "{ip}".into()],
            desbloquear: vec![],
            listar: vec![],
            timeout_s: 5,
        };
        // Endereco de verdade passa.
        assert!(fw.bloquear_ip("192.0.2.1", 60).unwrap());
        assert!(fw.bloquear_ip("2001:db8::1", 60).unwrap());
        // Qualquer outra coisa e recusada ANTES de virar argumento.
        for ruim in [
            "; rm -rf /",
            "10.0.0.1 && reboot",
            "$(whoami)",
            "",
            "localhost",
        ] {
            assert!(
                fw.bloquear_ip(ruim, 60).is_err(),
                "deveria recusar {ruim:?}"
            );
        }
    }

    fn politica_com_firewall(bloquear: Vec<String>, desbloquear: Vec<String>) -> Politica {
        Politica {
            firewall: Some(Firewall {
                ligado: true,
                bloquear,
                desbloquear,
                listar: vec![],
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
            listar: vec![],
            timeout_s: 5,
        };
        let e = fw.bloquear_ip(ip, 60).unwrap_err().to_string();
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
            listar: vec![],
            timeout_s: 5,
        };
        assert!(fw.bloquear_ip(ip, 60).unwrap());
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
            listar: vec![],
            timeout_s: 1,
        };
        let t0 = std::time::Instant::now();
        let e = fw.bloquear_ip(ip, 60).unwrap_err().to_string();
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

    /* ------------------------------------------------ pedido 766, P9-P11 */

    /// Configuracao que nao e lida mente: os dois interruptores novos e o
    /// `listar` tem leitor, e o padrao de cada um e o escrito.
    #[test]
    fn os_interruptores_novos_vem_do_config() {
        let p = Politica::de_json(&Json::analisar("{}").unwrap());
        assert!(p.poupar_loopback && p.escalonar);
        let p = Politica::de_json(
            &Json::analisar(r#"{"poupar_loopback":false,"escalonar":false}"#).unwrap(),
        );
        assert!(!p.poupar_loopback && !p.escalonar);
        let fw = Firewall::de_json(
            &Json::analisar(
                r#"{"ligado":true,"bloquear":["/bin/true"],"listar":["/bin/true","-a"]}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(fw.listar, ["/bin/true", "-a"]);
    }

    /// O firewall ligado so aceita programa por caminho absoluto, pela
    /// conferencia do gancho; desligado, nada se confere (o comportamento de
    /// antes para quem nunca o ligou).
    #[cfg(unix)]
    #[test]
    fn o_firewall_ligado_exige_programa_por_caminho_absoluto() {
        let fw = |ligado: bool, programa: &str| Firewall {
            ligado,
            bloquear: vec![programa.into(), "{ip}".into()],
            desbloquear: vec![],
            listar: vec![],
            timeout_s: 5,
        };
        let e = fw(true, "nft").validar().unwrap_err();
        assert!(
            e.contains("seguranca.firewall.bloquear[0]") && e.contains("absoluto"),
            "{e}"
        );
        assert!(fw(true, "/bin/true").validar().is_ok());
        assert!(fw(false, "nft").validar().is_ok());
        let mut listar_ruim = fw(true, "/bin/true");
        listar_ruim.listar = vec!["/nao/existe/nft".into()];
        assert!(listar_ruim.validar().unwrap_err().contains("listar"));
        // E o arranque chama a conferencia: o `config.json` com `nft` pelo
        // PATH nao sobe.
        let ler = |programa: &str| {
            crate::config::Config::de_json(
                &Json::analisar(&format!(
                    r#"{{"token":"x","seguranca":{{"firewall":{{"ligado":true,"bloquear":["{programa}","{{ip}}"]}}}}}}"#
                ))
                .unwrap(),
            )
        };
        let e = ler("nft").unwrap_err().to_string();
        assert!(e.contains("o firewall ligado nao sobe assim"), "{e}");
        assert!(e.contains("seguranca.firewall.bloquear[0]"), "{e}");
        assert!(ler("/bin/true").is_ok());
    }

    /// **P10, a regra:** `bloqueio_minutos x 2^n`, n <= 20, teto de 7 dias
    /// que nunca encurta o que o administrador pediu; zero continua
    /// permanente; `escalonar: false` e o comportamento de antes.
    #[test]
    fn o_prazo_escalona_por_dois_ate_sete_dias() {
        let p = Politica::default();
        assert_eq!(p.minutos_do_bloqueio(0), 60);
        assert_eq!(p.minutos_do_bloqueio(1), 120);
        assert_eq!(p.minutos_do_bloqueio(2), 240);
        assert_eq!(p.minutos_do_bloqueio(7), 7_680);
        assert_eq!(p.minutos_do_bloqueio(8), TETO_DO_ESCALONADO_MIN);
        assert_eq!(p.minutos_do_bloqueio(24), TETO_DO_ESCALONADO_MIN);
        assert_eq!(p.minutos_do_bloqueio(u32::MAX), TETO_DO_ESCALONADO_MIN);
        let permanente = Politica {
            bloqueio_minutos: 0,
            ..Politica::default()
        };
        assert_eq!(permanente.minutos_do_bloqueio(5), 0);
        let velho = Politica {
            escalonar: false,
            ..Politica::default()
        };
        assert_eq!(velho.minutos_do_bloqueio(5), 60);
        let longo = Politica {
            bloqueio_minutos: 20_000,
            ..Politica::default()
        };
        assert_eq!(
            longo.minutos_do_bloqueio(0),
            20_000,
            "o teto encurtou o pedido"
        );
        assert_eq!(longo.minutos_do_bloqueio(3), 20_000);
    }

    /// **O aceite da P10:** 3o bloqueio = 240 min; o 25o = 7 dias, sem
    /// estouro; o reinicio (reabrir o arquivo) preserva o `n`; 30 dias
    /// depois, volta a base. RED: o `historico` fora do `gravar` -> depois
    /// de reabrir, o 4o volta a 60.
    #[test]
    fn o_terceiro_bloqueio_dura_240_e_o_vigesimo_quinto_sete_dias() {
        let d = dir_temp("escalona");
        let caminho = d.join("blacklist.json");
        let p = politica();
        let ip = "203.0.113.90";
        let minutos = |b: &Bloqueio| (b.ate_ms - b.desde_ms) / 60_000;
        let mut bl = Blacklist::abrir(&caminho).unwrap();
        let mut t = T0;
        let mut prazos = Vec::new();
        for _ in 0..3 {
            let (b, _) = bloqueou(bl.violacao_grave(ip, "excluir", "comando proibido", &p, t));
            prazos.push(minutos(&b));
            bl.desbloquear(ip).unwrap();
            t += 60_000;
        }
        assert_eq!(prazos, vec![60, 120, 240]);
        // O reinicio: o historico mora no arquivo.
        let mut bl = Blacklist::abrir(&caminho).unwrap();
        let (b, _) = bloqueou(bl.violacao_grave(ip, "excluir", "comando proibido", &p, t));
        assert_eq!(minutos(&b), 480, "o reinicio zerou a reincidencia");
        for _ in 4..25 {
            bl.desbloquear(ip).unwrap();
            t += 60_000;
            let _ = bl.violacao_grave(ip, "excluir", "comando proibido", &p, t);
        }
        let b = bl.bloqueado(ip, t).unwrap().clone();
        assert_eq!(minutos(&b), TETO_DO_ESCALONADO_MIN as i64);
        assert_eq!(bl.bloqueios_anteriores(ip, t), TETO_DO_EXPOENTE + 1);
        // O historico nao cresce alem do que muda o prazo.
        assert_eq!(
            bl.historico[ip].len() as u32,
            bl.bloqueios_anteriores(ip, t),
            "o historico guardou instante que nao conta"
        );
        // 30 dias depois do ultimo, a reincidencia acabou.
        bl.desbloquear(ip).unwrap();
        let depois = t + JANELA_DA_REINCIDENCIA_MS;
        let (b, _) = bloqueou(bl.violacao_grave(ip, "excluir", "comando proibido", &p, depois));
        assert_eq!(minutos(&b), 60);
        // E a forma mapeada e o MESMO IP para a reincidencia.
        assert_eq!(bl.bloqueios_anteriores("::ffff:203.0.113.90", depois), 1);
    }

    /// O historico tem teto de IPs: quem varia o IP nao faz o arquivo
    /// crescer sem fim. RED: tirar o `while ... > TETO_DO_HISTORICO`.
    #[test]
    fn o_historico_de_reincidencia_tem_teto() {
        let d = dir_temp("historico-teto");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        for i in 0..=TETO_DO_HISTORICO as u32 {
            let ip = std::net::Ipv4Addr::from(0x0A00_0000 + i).to_string();
            bl.anotar_no_historico(&ip, T0 + i as i64);
        }
        assert_eq!(bl.historico.len(), TETO_DO_HISTORICO);
        assert!(
            !bl.historico.contains_key("10.0.0.0"),
            "o mais velho tinha de sair"
        );
    }

    /// **P9, a guarda unica:** whitelist e loopback sao fixas; a externa so
    /// e perguntada quando nenhuma fixa vale (e a closure nem roda).
    #[test]
    fn a_guarda_fixa_vem_antes_da_externa_e_a_externa_e_preguicosa() {
        let d = dir_temp("guarda");
        let bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = Politica {
            whitelist: vec!["198.51.100.0/24".into()],
            ..Politica::default()
        };
        let nunca = || -> Option<Guarda> { panic!("a guarda externa rodou sem precisar") };
        assert_eq!(
            bl.guarda(&p, "198.51.100.7", nunca),
            Some(Guarda::Whitelist)
        );
        assert_eq!(bl.guarda(&p, "127.0.0.1", nunca), Some(Guarda::Loopback));
        assert_eq!(bl.guarda(&p, "127.8.9.10", nunca), Some(Guarda::Loopback));
        assert_eq!(bl.guarda(&p, "::1", nunca), Some(Guarda::Loopback));
        assert_eq!(
            bl.guarda(&p, "::ffff:127.0.0.1", nunca),
            Some(Guarda::Loopback)
        );
        assert_eq!(
            bl.guarda(&p, "203.0.113.1", || Some(Guarda::Compartilhado)),
            Some(Guarda::Compartilhado)
        );
        assert_eq!(bl.guarda(&p, "203.0.113.1", || None), None);
        let sem = Politica {
            poupar_loopback: false,
            ..Politica::default()
        };
        assert_eq!(bl.guarda(&sem, "127.0.0.1", || None), None);
    }

    /// O poupado CONTA e, no limite, volta `Poupada` -- e a conta recomeca.
    #[test]
    fn a_tentativa_leve_do_poupado_conta_e_recomeca() {
        let d = dir_temp("poupada");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        let ip = "127.0.0.1";
        for n in 1..3 {
            match bl.tentativa_leve_guardada(ip, "ping", "token invalido", &p, T0, || None) {
                Leve::Contada(c) => assert_eq!(c, n),
                outro => panic!("{outro:?}"),
            }
        }
        match bl.tentativa_leve_guardada(ip, "ping", "token invalido", &p, T0, || None) {
            Leve::Poupada {
                guarda: Guarda::Loopback,
                tentativas: 3,
            } => {}
            outro => panic!("{outro:?}"),
        }
        assert!(bl.bloqueado(ip, T0).is_none());
        assert_eq!(bl.tentativas_de(ip), 0, "a conta tinha de recomecar");
    }

    /// **P11, os marcadores:** o `{ip}` e o endereco REESCRITO, na forma
    /// canonica; `{familia}` e `{segundos}` saem de tipos, e nao de texto.
    #[test]
    fn os_marcadores_saem_do_endereco_analisado() {
        let argv: Vec<String> = [
            "nft",
            "negros{familia}",
            "{",
            "{ip}",
            "timeout",
            "{segundos}s",
            "}",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            trocar_marcadores(&argv, "::ffff:203.0.113.9", 3_600).unwrap(),
            [
                "nft",
                "negros4",
                "{",
                "203.0.113.9",
                "timeout",
                "3600s",
                "}"
            ]
        );
        assert_eq!(
            trocar_marcadores(&argv, "2001:db8:0:0::1", 5).unwrap()[3],
            "2001:db8::1"
        );
        assert_eq!(
            trocar_marcadores(&argv, "2001:db8::1", 5).unwrap()[1],
            "negros6"
        );
        for ruim in [
            "127.0.0.3 }; delete table inet vitima; add element inet phxsql negros4 { 127.0.0.4",
            "203.0.113.9 ",
            "",
            "localhost",
        ] {
            assert!(trocar_marcadores(&argv, ruim, 5).is_err(), "{ruim:?}");
        }
        let b = Bloqueio {
            ip: "x".into(),
            desde_ms: T0,
            ate_ms: T0 + 1_500,
            motivo: String::new(),
            comando: String::new(),
            tentativas: 1,
            firewall: false,
        };
        assert_eq!(b.segundos_restantes(T0), 2, "arredonda para cima");
        assert_eq!(
            b.segundos_restantes(T0 + 10_000),
            1,
            "nunca zero no temporario"
        );
        assert_eq!(Bloqueio { ate_ms: 0, ..b }.segundos_restantes(T0), 0);
    }

    /// **P11, o `listar` analisado:** so o pedaco que e um `IpAddr` inteiro
    /// conta -- a saida do `nft`, do `ipset` e do `netsh` passam pela mesma
    /// faca.
    #[test]
    fn a_listagem_e_analisada_token_a_token() {
        let nft = "table inet phxsql {\n\tset negros4 {\n\t\ttype ipv4_addr\n\t\tflags timeout\n\
                   \t\telements = { 203.0.113.9 timeout 2m expires 1m58s, 10.0.0.0/8,\n\
                   \t\t\t     ::ffff:198.51.100.2, 203.0.113.9 }\n\t}\n}\n";
        let ips: Vec<String> = ips_da_listagem(nft).iter().map(|i| i.to_string()).collect();
        assert_eq!(ips, ["203.0.113.9", "198.51.100.2"]);
        let netsh = "Rule Name: phxsql-2001:db8::5\nRemoteIP: 2001:db8::5/128\n";
        let ips: Vec<String> = ips_da_listagem(netsh)
            .iter()
            .map(|i| i.to_string())
            .collect();
        assert_eq!(
            ips,
            Vec::<String>::new(),
            "o nome e a faixa nao sao IP inteiro"
        );
        assert!(ips_da_listagem("Ok.\n").is_empty());
    }

    /// **P11, a reconciliacao:** o ativo ausente no SO volta com o prazo que
    /// falta; o que esta no SO sem bloqueio ativo sai; o loopback poupado e
    /// a whitelist nunca vao ao firewall. Com scripts no lugar do `nft`.
    #[cfg(unix)]
    #[test]
    fn a_reconciliacao_poe_o_que_falta_e_tira_o_que_sobra() {
        use crate::gancho::apoio_de_teste::{dir, script};
        let apoio = dir("fw-reconcilia");
        let registro = apoio.join("feito.txt");
        let listar = script(
            &apoio,
            "listar.sh",
            "echo 'elements = { 203.0.113.50 timeout 1h, 203.0.113.99 timeout 5m }'",
        );
        let add = script(
            &apoio,
            "add.sh",
            &format!("echo \"add $1 $2 $3\" >> {}", registro.display()),
        );
        let del = script(
            &apoio,
            "del.sh",
            &format!("echo \"del $1\" >> {}", registro.display()),
        );
        let p = Politica {
            whitelist: vec!["198.51.100.1".into()],
            firewall: Some(Firewall {
                ligado: true,
                bloquear: vec![add, "{ip}".into(), "{familia}".into(), "{segundos}".into()],
                desbloquear: vec![del, "{ip}".into()],
                listar: vec![listar],
                timeout_s: 5,
            }),
            ..politica()
        };
        let d = dir_temp("reconcilia");
        let caminho = d.join("blacklist.json");
        let agora = crate::agora_ms();
        {
            let mut bl = Blacklist::abrir(&caminho).unwrap();
            let sem_guarda = Politica {
                poupar_loopback: false,
                whitelist: Vec::new(),
                ..p.clone()
            };
            for ip in ["203.0.113.50", "203.0.113.51", "127.0.0.1", "198.51.100.1"] {
                bl.bloquear(ip, "teste", "ping", 1, &sem_guarda, agora);
            }
        }
        let lista = Mutex::new(Blacklist::abrir(&caminho).unwrap());
        let r = reconciliar_firewall(&lista, &p, agora + 1_000)
            .unwrap()
            .unwrap();
        assert_eq!(r.acrescentados, ["203.0.113.51"]);
        assert_eq!(r.removidos, ["203.0.113.99"]);
        assert!(r.falhas.is_empty(), "{:?}", r.falhas);
        let feito = std::fs::read_to_string(&registro).unwrap();
        assert!(feito.contains("del 203.0.113.99"), "{feito}");
        assert!(feito.contains("add 203.0.113.51 4 3599"), "{feito}");
        assert!(
            !feito.contains("127.0.0.1") && !feito.contains("198.51.100.1"),
            "{feito}"
        );
        let relida = Blacklist::abrir(&caminho).unwrap();
        assert!(relida.bloqueado("203.0.113.51", agora).unwrap().firewall);
        // Sem o `listar`, nao ha reconciliacao -- e nada roda.
        let mut sem_listar = p.clone();
        sem_listar.firewall.as_mut().unwrap().listar.clear();
        assert!(reconciliar_firewall(&lista, &sem_listar, agora)
            .unwrap()
            .is_none());
    }

    /// O filho que roda DENTRO de `unshare --net`: um namespace de rede so
    /// dele, com o `nft` de verdade. Ver o teste de baixo.
    #[cfg(unix)]
    #[test]
    #[ignore = "roda so dentro de o_nft_de_verdade_recusa_a_injecao_e_reconcilia"]
    fn filho_do_nft_em_namespace_proprio() {
        use std::process::Command;
        let nft = std::env::var("PHX_766_NFT").unwrap();
        let nft_ok = |args: &[&str]| Command::new(&nft).args(args).status().unwrap().success();
        let lista_do_conjunto = || {
            String::from_utf8(
                Command::new(&nft)
                    .args(["list", "set", "inet", "phxsql", "negros4"])
                    .output()
                    .unwrap()
                    .stdout,
            )
            .unwrap()
        };
        assert!(nft_ok(&["add", "table", "inet", "vitima"]));
        assert!(nft_ok(&["add", "table", "inet", "phxsql"]));
        assert!(nft_ok(&[
            "add",
            "set",
            "inet",
            "phxsql",
            "negros4",
            "{ type ipv4_addr; flags timeout; }"
        ]));
        let por_argv = |modelo: &[&str]| -> Vec<String> {
            std::iter::once(nft.clone())
                .chain(modelo.iter().map(|s| s.to_string()))
                .collect()
        };
        let fw = Firewall {
            ligado: true,
            bloquear: por_argv(&[
                "add",
                "element",
                "inet",
                "phxsql",
                "negros{familia}",
                "{",
                "{ip}",
                "timeout",
                "{segundos}s",
                "}",
            ]),
            desbloquear: por_argv(&[
                "delete",
                "element",
                "inet",
                "phxsql",
                "negros{familia}",
                "{",
                "{ip}",
                "}",
            ]),
            listar: por_argv(&["list", "set", "inet", "phxsql", "negros4"]),
            timeout_s: 5,
        };
        let injecao =
            "127.0.0.3 }; delete table inet vitima; add element inet phxsql negros4 { 127.0.0.4";

        // O CONTROLE POSITIVO -- o defeito reposto, aqui dentro: o mesmo argv
        // com o texto CRU no lugar do `{ip}`, sem analisar, pelo mesmo motor.
        // O `nft` junta o argv e a injecao apaga a tabela. E isto que prova
        // que a afirmacao de baixo mede alguma coisa.
        assert!(nft_ok(&["add", "table", "inet", "controle"]));
        let cru: Vec<String> = fw
            .bloquear
            .iter()
            .map(|a| {
                a.replace("{ip}", &injecao.replace("vitima", "controle"))
                    .replace("{familia}", "4")
                    .replace("{segundos}", "60")
            })
            .collect();
        let _ = crate::gancho::rodar(&fw.execucao(&cru, 0));
        assert!(
            !nft_ok(&["list", "table", "inet", "controle"]),
            "o controle falhou: a injecao crua nao apagou a tabela, e o teste nao mede nada"
        );

        // O caminho de verdade recusa ANTES do `nft`, e a vitima continua la.
        assert!(fw.bloquear_ip(injecao, 60).is_err());
        assert!(
            nft_ok(&["list", "table", "inet", "vitima"]),
            "a injecao passou"
        );

        // O caminho feliz: o elemento entra com prazo, e o `listar` o ve.
        assert!(fw.bloquear_ip("::ffff:203.0.113.9", 120).unwrap());
        assert!(
            lista_do_conjunto().contains("203.0.113.9"),
            "{}",
            lista_do_conjunto()
        );
        let vistos = fw.listar_ips().unwrap().unwrap();
        assert!(
            vistos.contains(&"203.0.113.9".parse().unwrap()),
            "{vistos:?}"
        );
        // O escalonamento chega ao kernel pelo MESMO `bloquear`: o `add` do
        // elemento que ja existe troca o prazo dele (P10; a hipotese de que o
        // prazo antigo ficava morreu medida -- cognicao de 09/10/2026 19:40).
        assert!(fw.bloquear_ip("203.0.113.9", 600).unwrap());
        assert!(
            lista_do_conjunto().contains("timeout 10m"),
            "{}",
            lista_do_conjunto()
        );

        // A reconciliacao contra o SO de verdade: o «reboot» (`flush set`)
        // tirou tudo; a lista tem 203.0.113.10 ativo; o SO tem 203.0.113.11
        // que nao esta na lista.
        assert!(nft_ok(&["flush", "set", "inet", "phxsql", "negros4"]));
        assert!(nft_ok(&[
            "add",
            "element",
            "inet",
            "phxsql",
            "negros4",
            "{ 203.0.113.11 timeout 600s }"
        ]));
        let d = dir_temp("nft-reconcilia");
        let p = Politica {
            firewall: Some(fw.clone()),
            ..Politica::default()
        };
        let agora = crate::agora_ms();
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        bl.bloquear("203.0.113.10", "teste", "ping", 1, &p, agora);
        let lista = Mutex::new(bl);
        let r = reconciliar_firewall(&lista, &p, agora).unwrap().unwrap();
        assert!(r.falhas.is_empty(), "{:?}", r.falhas);
        let depois = lista_do_conjunto();
        assert!(
            depois.contains("203.0.113.10"),
            "o ativo nao voltou: {depois}"
        );
        assert!(
            !depois.contains("203.0.113.11"),
            "o orfao nao saiu: {depois}"
        );
        assert!(depois.contains("expires"), "voltou sem prazo: {depois}");
    }

    /// **P11 contra o sistema operacional:** o `nft` de verdade, num
    /// namespace de rede proprio (`unshare --net`, nada do host e tocado).
    /// (1) a injecao pelo IP e recusada e a tabela `vitima` EXISTE -- com o
    /// controle positivo dentro do filho: o mesmo argv com o texto cru apaga
    /// a tabela `controle`, que e o RED (M4 do desenho) reposto a cada
    /// corrida; (2) o elemento entra com prazo e o `listar` o ve; (3) a
    /// reconciliacao, depois de um `flush set` (o reboot), devolve o ativo
    /// e tira o orfao.
    ///
    /// **Nao medido aqui:** o SYN do IP bloqueado que nao chega (M5 do
    /// desenho mediu); o `netsh` do Windows.
    #[cfg(unix)]
    #[test]
    fn o_nft_de_verdade_recusa_a_injecao_e_reconcilia() {
        use std::process::Command;
        let Some(nft) = ["/usr/sbin/nft", "/sbin/nft"]
            .into_iter()
            .find(|p| Path::new(p).exists())
        else {
            eprintln!("sem nft nesta maquina: a prova da P11 contra o SO NAO MEDIDA");
            return;
        };
        let pode = Command::new("unshare")
            .args(["--net", "true"])
            .status()
            .is_ok_and(|s| s.success());
        if !pode {
            eprintln!("sem unshare --net (precisa de root): a prova da P11 NAO MEDIDA");
            return;
        }
        let saida = Command::new("unshare")
            .arg("--net")
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "blacklist::tests::filho_do_nft_em_namespace_proprio",
                "--test-threads=1",
                "--nocapture",
            ])
            .env("PHX_766_NFT", nft)
            .output()
            .unwrap();
        let texto = String::from_utf8_lossy(&saida.stdout).into_owned()
            + &String::from_utf8_lossy(&saida.stderr);
        assert!(saida.status.success(), "{texto}");
        assert!(texto.contains("1 passed"), "o filho nao rodou: {texto}");
    }
}
