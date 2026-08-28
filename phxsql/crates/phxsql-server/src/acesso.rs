//! Log de acessos: quem chegou na porta, quando, e o que pediu.
//!
//! Um registro por conexao, em JSON Lines -- uma linha por acesso, legivel a
//! olho nu e facil de filtrar com `grep`:
//!
//! ```text
//! {"quando":"2026-08-27 18:40:12,345","ip":"192.168.50.20","porta":54321,
//!  "op":"ping","autenticado":true,"ok":true,"ms":1}
//! ```
//!
//! **Toda** conexao entra aqui, inclusive as recusadas por IP ou por token
//! errado -- e justamente quem tentou e nao conseguiu que interessa num log de
//! acesso.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::Result;
use phxsql_core::json::Json;

/// Um acesso a porta.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Acesso {
    /// Milissegundos desde a epoca Unix.
    pub quando_ms: i64,
    pub ip: String,
    pub porta_origem: u16,
    pub op: String,
    /// Login de quem fez, quando houve login. Vazio para anonimo.
    pub usuario: String,
    /// O token conferiu?
    pub autenticado: bool,
    /// A operacao terminou bem?
    pub ok: bool,
    pub duracao_ms: u64,
    pub erro: Option<String>,
    /// Sobre QUAL objeto a operacao foi, quando ela nomeia um.
    ///
    /// O log dizia so o que foi feito, e nao em que. "varrer levou 4 s" sem o
    /// nome da tabela e quase inutil para quem opera: nao da para somar por
    /// tabela, nem para achar a que custa caro. Vazio quando a operacao nao
    /// fala de tabela nenhuma -- `ping`, `config`, `usuarios`.
    pub database: String,
    pub tabela: String,
    /// Codigo do erro, para agrupar por causa em vez de por texto.
    pub codigo: u16,
}

impl Acesso {
    pub fn quando(&self) -> String {
        instante_iso(self.quando_ms)
    }

    pub fn para_json(&self) -> Json {
        let mut pares = vec![
            ("quando", Json::texto_de(self.quando())),
            ("quando_ms", Json::Numero(self.quando_ms as f64)),
            ("ip", Json::texto_de(&self.ip)),
            ("porta_origem", Json::de_u64(self.porta_origem as u64)),
            ("op", Json::texto_de(&self.op)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("autenticado", Json::Bool(self.autenticado)),
            ("ok", Json::Bool(self.ok)),
            ("ms", Json::de_u64(self.duracao_ms)),
        ];
        // So entram quando existem: o log e uma linha por acesso, e campo
        // vazio em toda linha e peso morto num arquivo que cresce sozinho.
        if !self.database.is_empty() {
            pares.push(("database", Json::texto_de(&self.database)));
        }
        if !self.tabela.is_empty() {
            pares.push(("tabela", Json::texto_de(&self.tabela)));
        }
        if let Some(e) = &self.erro {
            pares.push(("erro", Json::texto_de(e)));
            pares.push(("codigo", Json::de_u64(self.codigo as u64)));
        }
        Json::objeto(pares)
    }

    fn de_json(j: &Json) -> Option<Acesso> {
        Some(Acesso {
            quando_ms: j.campo("quando_ms")?.numero()? as i64,
            ip: j.texto_ou("ip", "").to_string(),
            porta_origem: j.inteiro_ou("porta_origem", 0) as u16,
            op: j.texto_ou("op", "").to_string(),
            usuario: j.texto_ou("usuario", "").to_string(),
            autenticado: j.booleano_ou("autenticado", false),
            ok: j.booleano_ou("ok", false),
            duracao_ms: j.inteiro_ou("ms", 0).max(0) as u64,
            erro: j.campo("erro").and_then(Json::texto).map(str::to_string),
            // Linha antiga nao tem estes campos, e ler o log antigo tem de
            // continuar funcionando: ausente vira vazio, nao erro.
            database: j.texto_ou("database", "").to_string(),
            tabela: j.texto_ou("tabela", "").to_string(),
            codigo: j.inteiro_ou("codigo", 0).clamp(0, 65_535) as u16,
        })
    }
}

/// Quantas vezes um IP apareceu, e quando foi a ultima.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumoIp {
    pub ip: String,
    pub acessos: u64,
    pub recusados: u64,
    pub primeiro_ms: i64,
    pub ultimo_ms: i64,
}

impl ResumoIp {
    pub fn primeiro(&self) -> String {
        instante_iso(self.primeiro_ms)
    }
    pub fn ultimo(&self) -> String {
        instante_iso(self.ultimo_ms)
    }
}

pub struct LogAcessos {
    caminho: PathBuf,
    arquivo: File,
}

impl LogAcessos {
    /// Abre para acrescentar, criando o arquivo e o diretorio se preciso.
    pub fn abrir(caminho: impl AsRef<Path>) -> Result<LogAcessos> {
        let caminho = caminho.as_ref().to_path_buf();
        if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let arquivo = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&caminho)?;
        Ok(LogAcessos { caminho, arquivo })
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Grava e descarrega na hora: um log de acesso que se perde no buffer
    /// quando o processo cai nao serve para nada.
    pub fn registrar(&mut self, a: &Acesso) -> Result<()> {
        writeln!(self.arquivo, "{}", a.para_json().escrever())?;
        self.arquivo.flush()?;
        Ok(())
    }

    /// Le os acessos do arquivo, do mais antigo para o mais recente.
    /// Linhas ilegiveis sao puladas -- um log truncado ainda deve ser lido.
    pub fn ler(caminho: impl AsRef<Path>) -> Result<Vec<Acesso>> {
        let caminho = caminho.as_ref();
        if !caminho.exists() {
            return Ok(Vec::new());
        }
        let arquivo = File::open(caminho)?;
        Ok(BufReader::new(arquivo)
            .lines()
            .map_while(std::result::Result::ok)
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| Json::analisar(&l).ok())
            .filter_map(|j| Acesso::de_json(&j))
            .collect())
    }

    /// Um resumo por IP, do mais recente para o mais antigo.
    pub fn resumo_por_ip(caminho: impl AsRef<Path>) -> Result<Vec<ResumoIp>> {
        let acessos = Self::ler(caminho)?;
        let mut resumos: Vec<ResumoIp> = Vec::new();
        for a in acessos {
            match resumos.iter_mut().find(|r| r.ip == a.ip) {
                Some(r) => {
                    r.acessos += 1;
                    if !a.ok {
                        r.recusados += 1;
                    }
                    r.primeiro_ms = r.primeiro_ms.min(a.quando_ms);
                    r.ultimo_ms = r.ultimo_ms.max(a.quando_ms);
                }
                None => resumos.push(ResumoIp {
                    ip: a.ip.clone(),
                    acessos: 1,
                    recusados: u64::from(!a.ok),
                    primeiro_ms: a.quando_ms,
                    ultimo_ms: a.quando_ms,
                }),
            }
        }
        resumos.sort_by(|a, b| b.ultimo_ms.cmp(&a.ultimo_ms));
        Ok(resumos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-acesso-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn acesso(ip: &str, ms: i64, ok: bool) -> Acesso {
        Acesso {
            quando_ms: ms,
            ip: ip.to_string(),
            porta_origem: 54_321,
            op: "ping".into(),
            usuario: "adriano".into(),
            autenticado: ok,
            ok,
            duracao_ms: 3,
            erro: if ok {
                None
            } else {
                Some("token invalido".into())
            },
            codigo: if ok { 0 } else { 4001 },
            ..Acesso::default()
        }
    }

    #[test]
    fn grava_e_le_com_ip_data_e_hora() {
        let d = dir_temp("basico");
        let caminho = d.join("acessos.log");
        {
            let mut l = LogAcessos::abrir(&caminho).unwrap();
            l.registrar(&acesso("192.168.50.20", 1_700_000_000_000, true))
                .unwrap();
            l.registrar(&acesso("10.1.1.102", 1_700_000_060_000, false))
                .unwrap();
        }
        let lidos = LogAcessos::ler(&caminho).unwrap();
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[0].ip, "192.168.50.20");
        assert_eq!(lidos[0].porta_origem, 54_321);
        assert_eq!(lidos[0].usuario, "adriano", "o log diz quem fez");
        assert!(lidos[0].ok);
        assert_eq!(lidos[1].erro.as_deref(), Some("token invalido"));
        // A data e hora saem legiveis.
        assert_eq!(lidos[0].quando(), "2023-11-14 22:13:20,000");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn acesso_recusado_tambem_e_registrado() {
        let d = dir_temp("recusado");
        let caminho = d.join("acessos.log");
        {
            let mut l = LogAcessos::abrir(&caminho).unwrap();
            l.registrar(&acesso("203.0.113.9", 1_700_000_000_000, false))
                .unwrap();
        }
        let lidos = LogAcessos::ler(&caminho).unwrap();
        assert_eq!(lidos.len(), 1);
        assert!(
            !lidos[0].autenticado,
            "quem tentou e falhou tem de aparecer"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn resumo_agrupa_por_ip() {
        let d = dir_temp("resumo");
        let caminho = d.join("acessos.log");
        {
            let mut l = LogAcessos::abrir(&caminho).unwrap();
            l.registrar(&acesso("10.0.0.1", 1_000, true)).unwrap();
            l.registrar(&acesso("10.0.0.2", 2_000, false)).unwrap();
            l.registrar(&acesso("10.0.0.1", 3_000, true)).unwrap();
            l.registrar(&acesso("10.0.0.1", 4_000, false)).unwrap();
        }
        let r = LogAcessos::resumo_por_ip(&caminho).unwrap();
        assert_eq!(r.len(), 2);
        // Ordenado pelo acesso mais recente.
        assert_eq!(r[0].ip, "10.0.0.1");
        assert_eq!(r[0].acessos, 3);
        assert_eq!(r[0].recusados, 1);
        assert_eq!(r[0].primeiro_ms, 1_000);
        assert_eq!(r[0].ultimo_ms, 4_000);
        assert_eq!(r[1].ip, "10.0.0.2");
        assert_eq!(r[1].recusados, 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn continua_acrescentando_depois_de_reabrir() {
        let d = dir_temp("append");
        let caminho = d.join("acessos.log");
        for i in 0..3 {
            let mut l = LogAcessos::abrir(&caminho).unwrap();
            l.registrar(&acesso("10.0.0.9", 1_000 * (i + 1), true))
                .unwrap();
        }
        assert_eq!(LogAcessos::ler(&caminho).unwrap().len(), 3);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn linha_corrompida_nao_derruba_a_leitura() {
        let d = dir_temp("corrompido");
        let caminho = d.join("acessos.log");
        {
            let mut l = LogAcessos::abrir(&caminho).unwrap();
            l.registrar(&acesso("10.0.0.1", 1_000, true)).unwrap();
        }
        {
            let mut f = OpenOptions::new().append(true).open(&caminho).unwrap();
            writeln!(f, "{{isso nao e json").unwrap();
            writeln!(f).unwrap();
        }
        {
            let mut l = LogAcessos::abrir(&caminho).unwrap();
            l.registrar(&acesso("10.0.0.2", 2_000, true)).unwrap();
        }
        let lidos = LogAcessos::ler(&caminho).unwrap();
        assert_eq!(lidos.len(), 2, "a linha ruim e pulada, o resto vem");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn arquivo_inexistente_devolve_vazio() {
        assert!(LogAcessos::ler("/nao/existe/acessos.log")
            .unwrap()
            .is_empty());
    }
}
