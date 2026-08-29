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

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
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
    pub comandos_proibidos: Vec<String>,
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
}

impl Default for Politica {
    fn default() -> Self {
        Politica {
            comandos_proibidos: Vec::new(),
            bases_proibidas: Vec::new(),
            tentativas_ate_bloquear: 5,
            tentativas_para_bloqueio: 1,
            janela_minutos: 10,
            bloqueio_minutos: 60,
            whitelist: Vec::new(),
            firewall: None,
        }
    }
}

impl Politica {
    pub fn de_json(j: &Json) -> Politica {
        let padrao = Politica::default();
        Politica {
            comandos_proibidos: j
                .textos("comandos_proibidos")
                .into_iter()
                .map(|c| c.trim().to_lowercase())
                .filter(|c| !c.is_empty())
                .collect(),
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

    /// A operacao esta proibida por politica?
    pub fn comando_proibido(&self, op: &str) -> bool {
        let alvo = op.trim().to_lowercase();
        self.comandos_proibidos.contains(&alvo)
    }

    /// A base esta proibida por politica?
    pub fn base_proibida(&self, base: &str) -> bool {
        !base.is_empty() && self.bases_proibidas.iter().any(|b| b == base)
    }
}

/// Comando de firewall, como lista de argumentos. `{ip}` vira o endereco.
#[derive(Debug, Clone)]
pub struct Firewall {
    pub ligado: bool,
    pub bloquear: Vec<String>,
    pub desbloquear: Vec<String>,
}

impl Firewall {
    fn de_json(j: &Json) -> Option<Firewall> {
        let f = Firewall {
            ligado: j.booleano_ou("ligado", false),
            bloquear: j.textos("bloquear"),
            desbloquear: j.textos("desbloquear"),
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
        let saida = std::process::Command::new(&trocado[0])
            .args(&trocado[1..])
            .output()?;
        if !saida.status.success() {
            return Err(phxsql_core::error::PhxError::Corrompido(format!(
                "o comando de firewall falhou: {}",
                String::from_utf8_lossy(&saida.stderr).trim()
            )));
        }
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
            std::fs::create_dir_all(dir)?;
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
        std::fs::write(&self.caminho, doc.escrever_identado())?;
        Ok(())
    }

    /// Grava e anota o carimbo, para nao reler a propria escrita.
    fn gravar_e_marcar(&mut self) -> Result<()> {
        self.gravar()?;
        self.lido_em = mtime(&self.caminho);
        Ok(())
    }

    /// Bloqueia um IP e grava. Tenta a regra de firewall, se houver.
    ///
    /// A falha do firewall **nao** cancela o bloqueio: o servidor barra o IP de
    /// qualquer jeito. O erro sai no retorno para virar aviso no log.
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
        let mut no_firewall = false;
        if let Some(fw) = &politica.firewall {
            match fw.bloquear_ip(ip) {
                Ok(aplicou) => no_firewall = aplicou,
                Err(e) => aviso = Some(format!("firewall: {e}")),
            }
        }

        let bloqueio = Bloqueio {
            ip: ip.to_string(),
            desde_ms: agora_ms,
            ate_ms,
            motivo: motivo.to_string(),
            comando: comando.to_string(),
            tentativas,
            firewall: no_firewall,
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

    /// Tira o IP da lista. Devolve `true` se ele estava la.
    pub fn desbloquear(&mut self, ip: &str, politica: &Politica) -> Result<bool> {
        let tinha = self.bloqueios.iter().any(|b| b.ip == ip);
        if !tinha {
            return Ok(false);
        }
        self.bloqueios.retain(|b| b.ip != ip);
        self.tentativas.remove(ip);
        self.tentativas_graves.remove(ip);
        self.gravar_e_marcar()?;
        if let Some(fw) = &politica.firewall {
            fw.desbloquear_ip(ip)?;
        }
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

    /// Tira da lista os bloqueios ja vencidos. Devolve quantos saíram.
    pub fn limpar_vencidos(&mut self, agora_ms: i64, politica: &Politica) -> Result<usize> {
        let antes = self.bloqueios.len();
        let vencidos: Vec<String> = self
            .bloqueios
            .iter()
            .filter(|b| !b.ativo_em(agora_ms))
            .map(|b| b.ip.clone())
            .collect();
        if vencidos.is_empty() {
            return Ok(0);
        }
        self.bloqueios.retain(|b| b.ativo_em(agora_ms));
        if let Some(fw) = &politica.firewall {
            for ip in &vencidos {
                let _ = fw.desbloquear_ip(ip);
            }
        }
        self.gravar_e_marcar()?;
        Ok(antes - self.bloqueios.len())
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

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-bl-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
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
            assert!(cli.desbloquear("10.0.0.7", &p).unwrap());
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
        assert!(bl.desbloquear("10.0.0.9", &p).unwrap());
        assert!(bl.bloqueado("10.0.0.9", T0).is_none());
        assert!(!bl.desbloquear("10.0.0.9", &p).unwrap(), "ja nao estava la");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn limpar_vencidos() {
        let d = dir_temp("vencidos");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.violacao_grave("10.0.0.1", "excluir", "x", &p, T0);
        bl.violacao_grave("10.0.0.2", "excluir", "x", &p, T0);
        assert_eq!(bl.limpar_vencidos(T0 + 1_000, &p).unwrap(), 0);
        assert_eq!(bl.limpar_vencidos(T0 + 61 * 60_000, &p).unwrap(), 2);
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

    #[test]
    fn firewall_desligado_nao_roda_nada() {
        let fw = Firewall {
            ligado: false,
            bloquear: vec!["/bin/false".into(), "{ip}".into()],
            desbloquear: vec![],
        };
        assert!(!fw.bloquear_ip("10.0.0.1").unwrap());
    }

    #[test]
    fn firewall_recusa_o_que_nao_e_endereco() {
        let fw = Firewall {
            ligado: true,
            bloquear: vec!["/bin/true".into(), "{ip}".into()],
            desbloquear: vec![],
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

    #[test]
    fn falha_do_firewall_nao_cancela_o_bloqueio() {
        let d = dir_temp("fw-falha");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = Politica {
            firewall: Some(Firewall {
                ligado: true,
                bloquear: vec!["/comando/que/nao/existe".into(), "{ip}".into()],
                desbloquear: vec![],
            }),
            ..politica()
        };
        let (b, aviso) =
            bloqueou(bl.violacao_grave("10.0.0.3", "excluir", "comando proibido", &p, T0));
        assert!(aviso.is_some(), "a falha do firewall vira aviso");
        assert!(!b.firewall, "a regra nao chegou a ser aplicada");
        assert!(
            bl.bloqueado("10.0.0.3", T0).is_some(),
            "o servidor barra o IP mesmo sem o firewall"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
