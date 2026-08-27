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

/// Politica de bloqueio, vinda do `config.json`.
#[derive(Debug, Clone)]
pub struct Politica {
    /// Operacoes que ninguem pode pedir, nem quem tem permissao.
    pub comandos_proibidos: Vec<String>,
    /// Bases que ninguem pode tocar por esta porta.
    pub bases_proibidas: Vec<String>,
    /// Tentativas leves toleradas dentro da janela.
    pub tentativas_ate_bloquear: u32,
    pub janela_minutos: u64,
    /// Duracao do bloqueio. Zero = ate alguem desbloquear.
    pub bloqueio_minutos: u64,
    pub firewall: Option<Firewall>,
}

impl Default for Politica {
    fn default() -> Self {
        Politica {
            comandos_proibidos: Vec::new(),
            bases_proibidas: Vec::new(),
            tentativas_ate_bloquear: 5,
            janela_minutos: 10,
            bloqueio_minutos: 60,
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
            janela_minutos: j
                .inteiro_ou("janela_minutos", padrao.janela_minutos as i64)
                .max(1) as u64,
            bloqueio_minutos: j
                .inteiro_ou("bloqueio_minutos", padrao.bloqueio_minutos as i64)
                .max(0) as u64,
            firewall: j.campo("firewall").and_then(Firewall::de_json),
        }
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

/// A lista de bloqueio, com o contador de tentativas recentes.
pub struct Blacklist {
    caminho: PathBuf,
    bloqueios: Vec<Bloqueio>,
    /// Carimbos das tentativas leves recentes, por IP. So em memoria.
    tentativas: HashMap<String, Vec<i64>>,
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
        let bloqueios = match std::fs::read_to_string(&caminho) {
            Err(_) => Vec::new(),
            Ok(texto) => Json::analisar(&texto)
                .ok()
                .and_then(|j| {
                    j.campo("bloqueios")
                        .and_then(Json::lista)
                        .map(|l| l.to_vec())
                })
                .map(|l| l.iter().filter_map(Bloqueio::de_json).collect())
                .unwrap_or_default(),
        };
        let lido_em = mtime(&caminho);
        Ok(Blacklist {
            caminho,
            bloqueios,
            tentativas: HashMap::new(),
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
        self.lido_em = recarregada.lido_em;
        // As tentativas em memoria seguem: elas nao moram no arquivo.
        Ok(true)
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
        self.gravar_e_marcar()?;
        if let Some(fw) = &politica.firewall {
            fw.desbloquear_ip(ip)?;
        }
        Ok(true)
    }

    /// Registra uma violacao GRAVE: bloqueia na hora.
    pub fn violacao_grave(
        &mut self,
        ip: &str,
        comando: &str,
        motivo: &str,
        politica: &Politica,
        agora_ms: i64,
    ) -> (Bloqueio, Option<String>) {
        self.bloquear(ip, motivo, comando, 1, politica, agora_ms)
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
            firewall: None,
        }
    }

    const T0: i64 = 1_800_000_000_000;

    #[test]
    fn comando_proibido_bloqueia_na_hora() {
        let d = dir_temp("grave");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        assert!(bl.bloqueado("203.0.113.9", T0).is_none());

        let (b, aviso) = bl.violacao_grave("203.0.113.9", "excluir", "comando proibido", &p, T0);
        assert!(aviso.is_none());
        assert_eq!(b.comando, "excluir");
        assert_eq!(b.tentativas, 1);
        assert!(bl.bloqueado("203.0.113.9", T0).is_some());
        // O bloqueio vence depois de 60 minutos.
        assert!(bl.bloqueado("203.0.113.9", T0 + 61 * 60_000).is_none());
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
        let (b, aviso) = bl.violacao_grave("10.0.0.3", "excluir", "comando proibido", &p, T0);
        assert!(aviso.is_some(), "a falha do firewall vira aviso");
        assert!(!b.firewall, "a regra nao chegou a ser aplicada");
        assert!(
            bl.bloqueado("10.0.0.3", T0).is_some(),
            "o servidor barra o IP mesmo sem o firewall"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
