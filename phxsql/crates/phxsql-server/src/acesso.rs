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

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::{PhxError, Result};
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
    /// `None` so depois de um rodizio cujo reabrir falhou -- ver
    /// `registrar`. Fora isso, sempre `Some`: `acessos.log` nao tem o modo
    /// "so memoria" que o Profiler tem.
    arquivo: Option<File>,
    /// Teto de bytes por arquivo. Zero = nao rodizia -- o comportamento de
    /// sempre, e o padrao de quem nao configurou o campo novo do pedido 228
    /// (`acessos.arquivo_mib`). A logica de girar e a MESMA do Profiler --
    /// ver `crate::rodizio`, para onde ela foi extraida.
    teto_do_arquivo: u64,
    /// Quantos arquivos ANTIGOS guardar, alem do corrente.
    manter: usize,
    /// Bytes no arquivo CORRENTE -- decide a hora de girar. Semeado do
    /// TAMANHO REAL do arquivo ao abrir, e nao de zero: religar o servidor
    /// no mesmo `acessos.log` tem de continuar contando de onde o arquivo
    /// estava (mesma razao do `Profiler::ligar`).
    bytes_no_arquivo: u64,
    /// Quantas vezes o arquivo virou desde que abriu.
    rodizios: u64,
    /// Rodizios que nao deram certo -- renomear ou reabrir falhou.
    falhas_de_rodizio: u64,
}

impl LogAcessos {
    /// Abre para acrescentar, criando o arquivo e o diretorio se preciso.
    /// Nasce SEM rodizio (`teto_do_arquivo: 0`) -- quem quiser liga com
    /// [`LogAcessos::definir_rodizio`].
    pub fn abrir(caminho: impl AsRef<Path>) -> Result<LogAcessos> {
        let caminho = caminho.as_ref().to_path_buf();
        if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let arquivo = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&caminho)?;
        let bytes_no_arquivo = arquivo.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(LogAcessos {
            caminho,
            arquivo: Some(arquivo),
            teto_do_arquivo: 0,
            manter: 0,
            bytes_no_arquivo,
            rodizios: 0,
            falhas_de_rodizio: 0,
        })
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Ajusta o rodizio. Vale para o arquivo CORRENTE, como no Profiler
    /// (`Profiler::definir_rodizio`): quem baixou o teto na tela quer o
    /// efeito agora, e nao no proximo arranque do servidor.
    pub fn definir_rodizio(&mut self, teto_do_arquivo: u64, manter: usize) {
        self.teto_do_arquivo = teto_do_arquivo;
        self.manter = manter.min(crate::profiler::MAX_ARQUIVOS_ANTIGOS);
    }

    pub fn teto_do_arquivo(&self) -> u64 {
        self.teto_do_arquivo
    }

    pub fn manter(&self) -> usize {
        self.manter
    }

    pub fn rodizios(&self) -> u64 {
        self.rodizios
    }

    pub fn falhas_de_rodizio(&self) -> u64 {
        self.falhas_de_rodizio
    }

    /// Grava e descarrega na hora: um log de acesso que se perde no buffer
    /// quando o processo cai nao serve para nada.
    ///
    /// Confere ANTES de escrever se a linha estoura o teto -- mesma ordem do
    /// `profiler::girar_se_encheu`, pelo mesmo motivo: conferir depois
    /// deixaria a ultima linha de cada arquivo passar do teto.
    pub fn registrar(&mut self, a: &Acesso) -> Result<()> {
        let linha = a.para_json().escrever();
        let cabem = linha.len() as u64 + 1;
        if crate::rodizio::deve_girar(self.bytes_no_arquivo, cabem, self.teto_do_arquivo) {
            // Solta o descritor ANTES de girar -- ver a nota em
            // `rodizio::girar` sobre o Windows recusar renomear um arquivo
            // aberto.
            self.arquivo = None;
            let (novo, deu_errado) = crate::rodizio::girar(&self.caminho, self.manter);
            self.arquivo = novo;
            self.bytes_no_arquivo = 0;
            self.rodizios += 1;
            if deu_errado {
                self.falhas_de_rodizio += 1;
            }
        }
        let arquivo = self.arquivo.as_mut().ok_or_else(|| {
            PhxError::Io(std::io::Error::other(format!(
                "{} sem descritor -- o rodizio falhou ao reabrir",
                self.caminho.display()
            )))
        })?;
        writeln!(arquivo, "{linha}")?;
        arquivo.flush()?;
        self.bytes_no_arquivo += cabem;
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

    fn dir_temp(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("acesso-{rotulo}"))
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

    /// Pedido 228, sentido 1: **sem configurar nada, o comportamento e o de
    /// sempre** -- o arquivo so cresce, e nao ha `.1`. E o teste que a regra
    /// petrea "guarda nova entra pedida" exige: uma protecao nova que muda
    /// quem nao pediu nada e estrago, nao protecao.
    #[test]
    fn sem_rodizio_configurado_o_arquivo_so_cresce() {
        let d = dir_temp("sem-rodizio");
        let caminho = d.join("acessos.log");
        let mut l = LogAcessos::abrir(&caminho).unwrap();
        assert_eq!(l.teto_do_arquivo(), 0, "nasce sem rodizio");
        for i in 0..50 {
            l.registrar(&acesso("10.0.0.1", 1_000 * i, true)).unwrap();
        }
        assert_eq!(l.rodizios(), 0, "teto 0 nunca gira");
        assert!(!crate::rodizio::com_sufixo(&caminho, 1).exists());
        assert_eq!(LogAcessos::ler(&caminho).unwrap().len(), 50);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Pedido 228, sentido 2: **passar do teto configurado gira o arquivo**.
    /// Prova real do defeito reposto: com `crate::rodizio::deve_girar`
    /// forcado a sempre devolver `false` (o defeito antigo -- crescer para
    /// sempre), este teste tem de FALHAR na asserção de `rodizios() > 0` e
    /// na existencia do `.1`; com o rodizio ligado, os dois passam.
    #[test]
    fn estourar_o_teto_gira_o_arquivo() {
        let d = dir_temp("com-rodizio");
        let caminho = d.join("acessos.log");
        let mut l = LogAcessos::abrir(&caminho).unwrap();
        // Teto minusculo (a primeira linha ja passa dele) e `manter` folgado
        // o bastante para as ~10 gravacoes nao evictarem nada -- senao o
        // proprio rodizio, funcionando certo, apagaria o mais velho de
        // proposito (mesma regra do Profiler), e o total abaixo cairia por
        // um motivo que NAO e o defeito que este teste prova.
        l.definir_rodizio(80, 20);
        for i in 0..10 {
            l.registrar(&acesso("10.0.0.1", 1_000 * i, true)).unwrap();
        }
        assert!(l.rodizios() > 0, "tinha de ter girado pelo menos uma vez");
        assert_eq!(l.falhas_de_rodizio(), 0, "nenhum passo do rodizio falhou");
        let sufixo_1 = crate::rodizio::com_sufixo(&caminho, 1);
        assert!(
            sufixo_1.exists(),
            "o rodizio tinha de deixar um .1 para tras"
        );
        // Nada se perde: somando o corrente com TODOS os antigos que o
        // rodizio produziu, as 10 linhas continuam legiveis -- girar nao e
        // sinonimo de perder log quando `manter` cobre o que foi gerado.
        let mut total = LogAcessos::ler(&caminho).unwrap().len();
        for n in 1..=l.manter() {
            let sufixo = crate::rodizio::com_sufixo(&caminho, n);
            if !sufixo.exists() {
                break;
            }
            total += LogAcessos::ler(&sufixo).unwrap().len();
        }
        assert_eq!(total, 10, "girar nao pode perder nem duplicar linha");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
