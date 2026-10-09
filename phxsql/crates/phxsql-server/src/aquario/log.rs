//! O `aquario.log` (pedido 707, fatia A6): a linha do tempo das tarefas.
//!
//! Desenho em `docs/propostas/aquario-707.md` §4 e §11.3. Dois arquivos com
//! papeis diferentes, e este e um deles:
//!
//! * **`aquario.log`** (este): a linha do tempo das tarefas e a contagem. E
//!   lido por quem tem `monitorar` e pela TV, e por isso **nunca** leva login
//!   nem IP -- decisao do dono, 09/10. O [`Linha`] nao tem esses campos: a
//!   garantia e do tipo, e nao de um filtro que alguem lembraria de aplicar;
//! * **`ocorrencias.log`** (495): o FATO e a seguranca, para quem administra.
//!
//! # O mesmo escritor do `acessos.log`
//!
//! Grava pelo [`LogAcessos::registrar_json`], numa segunda instancia: abrir
//! 0600, girar por tamanho, contar a falha do rodizio e descarregar na hora
//! sao decisoes que aquele arquivo ja pagou (§4.1, H3).
//!
//! # Grava sem ninguem perguntar
//!
//! A linha `estourou` sai do `anotar` do servidor, o sumidouro de toda
//! resposta, e nao de uma consulta: a TV que abre depois do fato acha o fato
//! no disco. A chamada mora no `anotar` do SERVIDOR, e nao no
//! [`super::Aquario::anotar`], porque a falha de gravacao vai para o
//! `evento_de_disco`, que e do servidor -- o aquario nao o alcanca.
//!
//! # O que grava, e quem
//!
//! `estourou` sai do `anotar` do servidor; `mudou` sai do mesmo `anotar`,
//! quando a base da A4 acha o pedido fora do habitual e o produtor da A3 o
//! marca; `contagem` sai da virada do minuto da A8, no amostrador -- e a
//! mesma A8 a le de volta no arranque, pelo [`LogDoAquario::contagens_desde`].
//! Um escritor so para os tres.
//!
//! # O que AINDA NAO grava
//!
//! `nasceu` pede o id da tarefa no `Acesso` (§4.1) e o amostrador a olhar as
//! vivas; `retrato` e `sedimento` sao da A5. Os nomes estao aqui para que
//! cada fatia grave pela mesma porta.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use super::{Alarme, Classe};
use crate::acesso::{Acesso, LogAcessos};

/// O nome do arquivo, ao lado do `acessos.log`.
pub const NOME_DO_ARQUIVO: &str = "aquario.log";

/// 8 MiB por arquivo (§4.3): disco se mede em bytes, como no Profiler.
pub const TETO_DO_ARQUIVO: u64 = 8 * 1024 * 1024;

/// Sete antigos mais o corrente: 8 × 8 MiB = 64 MiB de teto (§4.3).
pub const ARQUIVOS_ANTIGOS: usize = 7;

/// A tarefa que viveu menos que isto nunca foi bolha, e nao estoura (§4.1).
pub const VIVEU_NO_AQUARIO_MS: u64 = 1_000;

/// Quantas linhas uma consulta devolve sem `max`, e o teto do `max`.
///
/// Com teto: a consulta e de quem tem `monitorar`, e um `max` de um milhao
/// montaria na memoria do servidor o rodizio inteiro, 64 MiB de JSON.
pub const MAX_PADRAO: usize = 500;
pub const MAX_TETO: usize = 5_000;

/// Linha maior que isto no arquivo e lixo (as nossas medem ~220 B): o leitor
/// a pula em vez de acumular um arquivo sem quebra inteiro na memoria.
const TETO_DA_LINHA: usize = 64 * 1024;

/// O pedaco lido de cada vez, de tras para a frente.
const BLOCO: u64 = 64 * 1024;

/// O que aconteceu, na linha do tempo (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// A tarefa passou de [`VIVEU_NO_AQUARIO_MS`] e virou bolha.
    Nasceu,
    /// A tarefa ganhou um alarme.
    Mudou,
    /// A tarefa que foi bolha terminou.
    Estourou,
    /// A tarefa foi encerrada por alguem.
    Morta,
    /// O quadro-chave de 30 s com as vivas.
    Retrato,
    /// Alarme do servidor, que nenhuma tarefa carrega.
    Sedimento,
    /// A contagem do minuto (A8).
    Contagem,
}

impl Evento {
    pub fn nome(self) -> &'static str {
        match self {
            Evento::Nasceu => "nasceu",
            Evento::Mudou => "mudou",
            Evento::Estourou => "estourou",
            Evento::Morta => "morta",
            Evento::Retrato => "retrato",
            Evento::Sedimento => "sedimento",
            Evento::Contagem => "contagem",
        }
    }
}

/// Uma linha do `aquario.log`.
///
/// # O que nao tem, de proposito
///
/// Nao tem `usuario` nem `ip`: nao ha como esquecer de tira-los, porque nao
/// ha onde po-los. Do alarme, a linha repete o VALOR -- o nome -- e quem le
/// a gravidade dele decide pelo [`Alarme::de_nome`] (§11.3).
///
/// # A cor vai, e sai da `classificar` (A5)
///
/// A cor de `estourou` e `mudou` e a que a [`super::classificar`] deu NAQUELE
/// instante, a mesma funcao do retrato -- e nao uma regra do log. Ela vai
/// gravada, e nao recalculada na leitura, porque a volta de cinco minutos
/// (A13) tem de mostrar a bolha da cor que a tela MOSTROU; recalcular exigiria
/// gravar todos os fatos da tarefa, e pintaria com a regra de hoje o que
/// ontem tinha outra cor.
#[derive(Debug, Clone, PartialEq)]
pub struct Linha {
    pub evento: Evento,
    /// Quando aconteceu, em ms desde a epoca.
    pub quando_ms: i64,
    /// `"dados:17#42"`, quando a fatia que o carrega existir.
    pub tarefa: String,
    pub op: String,
    pub database: String,
    pub tabela: String,
    /// Quanto a tarefa durou, para `estourou`.
    pub ms: Option<u64>,
    pub ok: Option<bool>,
    pub codigo: Option<u16>,
    pub alarme: Option<Alarme>,
    /// O id da `Ocorrencia` do 495 que este alarme gerou.
    pub ocorrencia: Option<u64>,
    /// Os numeros do evento (o `z` e o `n` da base, a contagem do minuto).
    /// Interno: nenhum produtor poe texto do cliente aqui.
    pub dados: Option<Json>,
    /// A classe que a [`super::classificar`] deu (A5).
    pub classe: Option<Classe>,
    /// Os bits de alarme da tarefa (A3), gravados pelos NOMES.
    pub alarmes: u32,
}

impl Linha {
    pub fn nova(evento: Evento, quando_ms: i64) -> Linha {
        Linha {
            evento,
            quando_ms,
            tarefa: String::new(),
            op: String::new(),
            database: String::new(),
            tabela: String::new(),
            ms: None,
            ok: None,
            codigo: None,
            alarme: None,
            ocorrencia: None,
            dados: None,
            classe: None,
            alarmes: 0,
        }
    }

    /// Pinta a linha com a classe da tarefa e os alarmes dela.
    pub fn com_classe(mut self, classe: Classe, alarmes: u32) -> Linha {
        self.classe = Some(classe);
        self.alarmes = alarmes;
        self
    }

    /// A tarefa terminou. O instante e o FIM (`quando_ms` do `Acesso` e o
    /// comeco), porque e o fim que a linha do tempo desenha.
    ///
    /// Do `Acesso` saem o que a tela pode mostrar a quem so monitora; o
    /// `usuario`, o `ip` e o texto do erro ficam no `acessos.log`.
    pub fn estourou(a: &Acesso) -> Linha {
        let mut l = Linha::nova(
            Evento::Estourou,
            a.quando_ms.saturating_add(a.duracao_ms as i64),
        );
        l.op = a.op.clone();
        l.database = a.database.clone();
        l.tabela = a.tabela.clone();
        l.ms = Some(a.duracao_ms);
        l.ok = Some(a.ok);
        if !a.ok && a.codigo != 0 {
            l.codigo = Some(a.codigo);
        }
        l
    }

    /// A tarefa ganhou um alarme (A3 grava por aqui).
    pub fn mudou(alarme: Alarme, ocorrencia: Option<u64>, quando_ms: i64) -> Linha {
        let mut l = Linha::nova(Evento::Mudou, quando_ms);
        l.alarme = Some(alarme);
        l.ocorrencia = ocorrencia;
        l
    }

    /// A contagem de um minuto fechado (A8). `fechou_ms` e o instante da
    /// virada, e nao o comeco do minuto: o arquivo e lido de tras para a
    /// frente parando no primeiro `quando_ms` anterior ao `desde`, e uma
    /// linha datada no passado no meio das outras cortaria a leitura cedo.
    ///
    /// `minuto` vai inteiro em `dados`, no formato do
    /// [`super::contagem::linha_do_minuto`]: o que a retomada le de volta e
    /// exatamente o que a virada produziu, sem uma segunda traducao.
    pub fn contagem(fechou_ms: i64, minuto: Json) -> Linha {
        let mut l = Linha::nova(Evento::Contagem, fechou_ms);
        l.dados = Some(minuto);
        l
    }

    /// Campo vazio nao entra (a regra do `acessos.log`): o arquivo cresce
    /// sozinho, e campo vazio em toda linha e peso morto.
    ///
    /// Os campos que vem do pedido passam pelo `de_uma_linha` do Profiler, e
    /// nao por um segundo redutor: o `Json::escrever` ja impede a quebra
    /// forjada de virar linha, mas um `op` de dez mil bytes viraria uma linha
    /// de dez mil bytes, e o `\n` cru dentro do campo ainda quebraria a TELA
    /// de quem mostra o valor.
    pub fn para_json(&self) -> Json {
        let livre = |s: &str| {
            Json::texto_de(crate::profiler::de_uma_linha(
                s,
                crate::profiler::TETO_DO_CAMPO,
            ))
        };
        let mut pares = vec![
            ("quando", Json::texto_de(instante_iso(self.quando_ms))),
            ("quando_ms", Json::de_i64(self.quando_ms)),
            ("evento", Json::texto_de(self.evento.nome())),
        ];
        if !self.tarefa.is_empty() {
            pares.push(("tarefa", livre(&self.tarefa)));
        }
        if !self.op.is_empty() {
            pares.push(("op", livre(&self.op)));
        }
        if !self.database.is_empty() {
            pares.push(("database", livre(&self.database)));
        }
        if !self.tabela.is_empty() {
            pares.push(("tabela", livre(&self.tabela)));
        }
        if let Some(ms) = self.ms {
            pares.push(("ms", Json::de_u64(ms)));
        }
        if let Some(ok) = self.ok {
            pares.push(("ok", Json::Bool(ok)));
        }
        if let Some(c) = self.codigo {
            pares.push(("codigo", Json::de_u64(c as u64)));
        }
        if let Some(a) = self.alarme {
            pares.push(("alarme", Json::texto_de(a.nome())));
        }
        if let Some(id) = self.ocorrencia {
            pares.push(("ocorrencia", Json::de_u64(id)));
        }
        if let Some(k) = &self.classe {
            pares.extend(k.campos());
        }
        if self.alarmes != 0 {
            pares.push(("alarmes", super::classe::nomes_dos_alarmes(self.alarmes)));
        }
        if let Some(d) = &self.dados {
            pares.push(("dados", d.clone()));
        }
        Json::objeto(pares)
    }
}

/// O escritor do `aquario.log` e o que ele sabe da propria saude.
///
/// Nasce FECHADO (`Default`): o `Aquario` nasce dentro do `Telemetria`, antes
/// de haver config, e o servidor o abre no arranque. Fechado, gravar nao faz
/// nada e consultar diz que nao ha arquivo -- e nao «nada aconteceu».
#[derive(Default)]
pub struct LogDoAquario {
    escritor: Mutex<Option<LogAcessos>>,
    /// Linhas que chegaram ao arquivo desde o arranque.
    gravadas: AtomicU64,
    /// Linhas que NAO chegaram. E o «avisa» do disco cheio visto pelo proprio
    /// aquario: a consulta o devolve, e quem olha a tela sabe que a linha do
    /// tempo tem buraco.
    falhas: AtomicU64,
    /// O texto da ultima falha (de gravar ou de abrir), para a consulta.
    ultima_falha: Mutex<Option<String>>,
}

impl fmt::Debug for LogDoAquario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogDoAquario")
            .field("caminho", &self.caminho())
            .field("gravadas", &self.gravadas())
            .field("falhas", &self.falhas())
            .finish()
    }
}

/// O mutex envenenado nao derruba o log: o que ele guarda e um descritor e um
/// texto, e nenhum dos dois fica pela metade num panico.
fn travar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl LogDoAquario {
    /// Abre (ou reabre) o arquivo, com o rodizio de 8 MiB × 8.
    ///
    /// A falha fica anotada para a consulta, e volta para quem chamou decidir:
    /// o arranque AVISA e sobe -- o aquario e acessorio, e um servidor que nao
    /// sobe porque a linha do tempo nao abriu trocaria o menor pelo maior.
    pub fn abrir(&self, caminho: impl AsRef<Path>) -> Result<()> {
        match LogAcessos::abrir(caminho.as_ref()) {
            Ok(mut log) => {
                log.definir_rodizio(TETO_DO_ARQUIVO, ARQUIVOS_ANTIGOS);
                *travar(&self.escritor) = Some(log);
                Ok(())
            }
            Err(e) => {
                *travar(&self.ultima_falha) =
                    Some(format!("nao abriu {}: {e}", caminho.as_ref().display()));
                Err(e)
            }
        }
    }

    pub fn caminho(&self) -> Option<PathBuf> {
        travar(&self.escritor)
            .as_ref()
            .map(|l| l.caminho().to_path_buf())
    }

    pub fn gravadas(&self) -> u64 {
        self.gravadas.load(Ordering::Relaxed)
    }

    pub fn falhas(&self) -> u64 {
        self.falhas.load(Ordering::Relaxed)
    }

    /// Grava uma linha. Fechado, nao faz nada.
    ///
    /// A falha e CONTADA aqui e devolvida: quem chama a leva ao
    /// `evento_de_disco`. Nada de panico nem de `?` que suba ate a conexao: o
    /// cliente cujo pedido terminou nao tem culpa do disco cheio, e nao pode
    /// receber erro por uma linha que ele nem pediu.
    pub fn gravar(&self, linha: &Linha) -> Result<()> {
        let mut escritor = travar(&self.escritor);
        let Some(log) = escritor.as_mut() else {
            return Ok(());
        };
        match log.registrar_json(&linha.para_json()) {
            Ok(()) => {
                self.gravadas.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                drop(escritor);
                self.falhas.fetch_add(1, Ordering::Relaxed);
                *travar(&self.ultima_falha) = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// O fim de um pedido, visto pelo `anotar` do servidor: so a tarefa que
    /// viveu no aquario estoura.
    ///
    /// O corte por duracao vem PRIMEIRO, antes de qualquer alocacao e antes da
    /// trava: e o caminho de quase todo pedido, e ele nao paga nada.
    ///
    /// As `OPS_DE_REPLICACAO` ficam fora pelo mesmo motivo de ficarem fora da
    /// base (§11.1): o `replicar_aguardar` espera por desenho, e estouraria
    /// uma linha por rodada de cada replica, enterrando as tarefas de gente.
    ///
    /// A cor chega por `classe`, que so roda DEPOIS do corte: a
    /// classificacao le a pintura do servidor, e o pedido de 3 ms nao paga
    /// isso. E quem a calcula e o servidor, pela [`super::classificar`] -- o
    /// log nao tem regra de cor propria (A5).
    pub fn tarefa_terminou(
        &self,
        a: &Acesso,
        classe: impl FnOnce() -> (Classe, u32),
    ) -> Result<()> {
        if a.duracao_ms < VIVEU_NO_AQUARIO_MS
            || crate::servidor::OPS_DE_REPLICACAO.contains(&a.op.as_str())
        {
            return Ok(());
        }
        let (k, alarmes) = classe();
        self.gravar(&Linha::estourou(a).com_classe(k, alarmes))
    }

    /// As contagens por minuto gravadas a partir de `desde_ms`, em ordem
    /// cronologica -- o `dados` de cada linha `contagem`, que e o que o
    /// [`super::contagem::Contagem::retomar`] le. Fechado, nenhuma.
    ///
    /// Sem o teto `max` da consulta: num servidor ocupado, as linhas
    /// `estourou` de duas horas passariam do teto e cortariam justamente os
    /// minutos mais velhos da hora que o arranque precisa refazer.
    pub fn contagens_desde(&self, desde_ms: i64) -> Result<Vec<Json>> {
        let Some(caminho) = self.caminho() else {
            return Ok(Vec::new());
        };
        let mut minutos = Vec::new();
        percorrer(&caminho, |j, q| {
            if q < desde_ms {
                return false;
            }
            if j.texto_ou("evento", "") == Evento::Contagem.nome() {
                if let Some(d) = j.campo("dados") {
                    minutos.push(d.clone());
                }
            }
            true
        })?;
        minutos.reverse();
        Ok(minutos)
    }

    /// `aquario_log`: as linhas entre `desde` e `ate` (ms desde a epoca), as
    /// `max` mais recentes, em ordem cronologica.
    ///
    /// # Nenhum filtro chamado `tabela`
    ///
    /// O portao geral de permissao le o campo `"tabela"` do pedido. Um filtro
    /// com esse nome faria o portao conferir o direito NAQUELA tabela, e o
    /// aquario e do servidor inteiro (`portao_do_aquario`).
    ///
    /// # De tras para a frente
    ///
    /// Le do fim do arquivo corrente para o comeco, e depois `.1`, `.2`...,
    /// parando na primeira linha anterior a `desde` ou ao juntar `max`. O
    /// `op_acessos` le o arquivo inteiro para devolver os N ultimos; com 8 MiB
    /// por arquivo isso seria o parse de dezenas de milhares de linhas por
    /// clique (§4.3).
    pub fn consultar(&self, pedido: &Json) -> Result<Json> {
        let desde = inteiro_opcional(pedido, "desde")?;
        let ate = inteiro_opcional(pedido, "ate")?;
        if let (Some(d), Some(a)) = (desde, ate) {
            if d > a {
                return Err(PhxError::Tipo(format!(
                    "aquario_log: \"desde\" ({d}) depois de \"ate\" ({a})"
                )));
            }
        }
        let max = match inteiro_opcional(pedido, "max")? {
            None => MAX_PADRAO,
            Some(m) if m < 1 => {
                return Err(PhxError::Tipo(format!(
                    "aquario_log: \"max\" tem de ser ao menos 1, veio {m}"
                )))
            }
            Some(m) => (m as u64).min(MAX_TETO as u64) as usize,
        };
        let Some(caminho) = self.caminho() else {
            let porque = travar(&self.ultima_falha)
                .clone()
                .map(|f| format!(" ({f})"))
                .unwrap_or_default();
            return Err(PhxError::NaoEncontrado(format!(
                "aquario_log: o aquario.log ainda nao esta aberto neste servidor{porque}"
            )));
        };

        let mut linhas: Vec<Json> = Vec::new();
        let mut truncado = false;
        let arquivos_lidos = percorrer(&caminho, |j, q| {
            if desde.is_some_and(|d| q < d) {
                return false;
            }
            if ate.is_some_and(|a| q > a) {
                return true;
            }
            if linhas.len() == max {
                truncado = true;
                return false;
            }
            linhas.push(j);
            true
        })?;
        linhas.reverse();

        let mut pares = vec![
            ("linhas", Json::Lista(linhas)),
            ("truncado", Json::Bool(truncado)),
            ("arquivos_lidos", Json::de_u64(arquivos_lidos)),
            ("gravadas", Json::de_u64(self.gravadas())),
            ("falhas_de_escrita", Json::de_u64(self.falhas())),
        ];
        if let Some(f) = travar(&self.ultima_falha).clone() {
            pares.push(("ultima_falha", Json::texto_de(f)));
        }
        Ok(Json::objeto(pares))
    }
}

/// Percorre o `aquario.log` e o rodizio dele da linha MAIS NOVA para a mais
/// velha, entregando cada linha legivel com o `quando_ms` dela, ate `cada`
/// devolver `false`. Devolve quantos arquivos abriu.
///
/// Um percurso so para a consulta e para a retomada da contagem: os dois
/// leem o mesmo arquivo, e dois lacos sobre o rodizio divergiriam no dia em
/// que um aprendesse a pular um tipo de lixo que o outro nao pula.
fn percorrer(caminho: &Path, mut cada: impl FnMut(Json, i64) -> bool) -> Result<u64> {
    let mut arquivos_lidos = 0u64;
    for n in 0..=ARQUIVOS_ANTIGOS {
        let alvo = if n == 0 {
            caminho.to_path_buf()
        } else {
            crate::rodizio::com_sufixo(caminho, n)
        };
        let Ok(arquivo) = File::open(&alvo) else {
            // O rodizio ainda nao chegou a este numero: os seguintes
            // tambem nao existem.
            break;
        };
        arquivos_lidos += 1;
        let mut parar = false;
        de_tras_para_frente(arquivo, |texto| {
            let Ok(j) = Json::analisar(texto) else {
                // Linha cortada pelo disco cheio ou pela queda: pula.
                return true;
            };
            let Some(q) = j.campo("quando_ms").and_then(Json::inteiro) else {
                return true;
            };
            parar = !cada(j, q);
            !parar
        })?;
        if parar {
            break;
        }
    }
    Ok(arquivos_lidos)
}

/// Um inteiro opcional do pedido. Presente com outro tipo e ERRO, e nao
/// ausente: `"desde":"ontem"` tratado como sem filtro devolveria o arquivo
/// inteiro a quem pediu so um pedaco.
fn inteiro_opcional(p: &Json, campo: &str) -> Result<Option<i64>> {
    match p.campo(campo) {
        None | Some(Json::Nulo) => Ok(None),
        Some(v) => v.inteiro().map(Some).ok_or_else(|| {
            PhxError::Tipo(format!(
                "aquario_log: \"{campo}\" e um inteiro (ms desde a epoca, ou a contagem)"
            ))
        }),
    }
}

/// Entrega as linhas do arquivo da ULTIMA para a primeira, ate `cada`
/// devolver `false`. Linha vazia, nao UTF-8 ou maior que [`TETO_DA_LINHA`]
/// e pulada.
fn de_tras_para_frente(mut arquivo: File, mut cada: impl FnMut(&str) -> bool) -> Result<()> {
    let mut pos = arquivo.seek(SeekFrom::End(0))?;
    // O comeco de linha que o bloco de tras deixou sem a quebra da frente.
    let mut resto: Vec<u8> = Vec::new();
    // O resto passou do teto: descarta-se ate a proxima quebra.
    let mut descartando = false;
    let mut bloco = vec![0u8; BLOCO as usize];
    let mut entregar = |bytes: &[u8], descartar: bool| -> bool {
        if descartar || bytes.is_empty() {
            return true;
        }
        match std::str::from_utf8(bytes) {
            Ok(t) if !t.trim().is_empty() => cada(t),
            _ => true,
        }
    };
    while pos > 0 {
        let n = BLOCO.min(pos);
        pos -= n;
        arquivo.seek(SeekFrom::Start(pos))?;
        let pedaco = &mut bloco[..n as usize];
        arquivo.read_exact(pedaco)?;
        // `pedaco` + `resto`: o fim do pedaco continua a linha do resto.
        let mut fim = pedaco.len();
        let mut primeira = true;
        while let Some(i) = pedaco[..fim].iter().rposition(|&b| b == b'\n') {
            let continuar = if primeira {
                // A linha que atravessa o limite do bloco.
                let mut linha = pedaco[i + 1..fim].to_vec();
                linha.extend_from_slice(&resto);
                let d = descartando || linha.len() > TETO_DA_LINHA;
                resto.clear();
                descartando = false;
                entregar(&linha, d)
            } else {
                entregar(&pedaco[i + 1..fim], fim - (i + 1) > TETO_DA_LINHA)
            };
            primeira = false;
            if !continuar {
                return Ok(());
            }
            fim = i;
        }
        // O que sobrou antes da primeira quebra do bloco e o fim de uma linha
        // que comeca num bloco anterior.
        if primeira {
            let mut novo = pedaco[..fim].to_vec();
            novo.extend_from_slice(&resto);
            resto = novo;
        } else {
            resto = pedaco[..fim].to_vec();
        }
        if resto.len() > TETO_DA_LINHA {
            resto.clear();
            descartando = true;
        }
    }
    entregar(&resto, descartando);
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    /// A classe que o servidor daria a uma tarefa sem nada de especial --
    /// estes testes sao do escritor, e a cor e da `classificar`.
    fn verde() -> (Classe, u32) {
        let f = super::super::Fatos::do_fim(0, 1_500, 1_500, super::super::base::Habitual::SemBase);
        (
            super::super::classificar(&f, &crate::config::Painel::default()),
            0,
        )
    }

    fn lento(op: &str, quando_ms: i64, ms: u64) -> Acesso {
        Acesso {
            quando_ms,
            ip: "203.0.113.77".into(),
            porta_origem: 5555,
            op: op.into(),
            usuario: "login-secreto".into(),
            autenticado: true,
            ok: true,
            duracao_ms: ms,
            database: "loja".into(),
            tabela: "vendas".into(),
            ..Acesso::default()
        }
    }

    fn aberto(rotulo: &str) -> (DirTemp, LogDoAquario) {
        let d = DirTemp::novo(&format!("aqlog-{rotulo}"));
        let log = LogDoAquario::default();
        log.abrir(d.join(NOME_DO_ARQUIVO)).unwrap();
        (d, log)
    }

    fn linhas_de(r: &Json) -> Vec<Json> {
        r.campo("linhas").and_then(Json::lista).unwrap().to_vec()
    }

    /// RED do pedido: `\n` forjado no nome da op nao vira duas linhas, nem o
    /// evento inventado que vinha atras dele. Trocar o `de_uma_linha` por
    /// `texto_de` cru deixa o `\n` no valor e cai a segunda conferencia; e um
    /// escritor que nao escapasse cairia a primeira.
    #[test]
    fn quebra_forjada_no_op_nao_quebra_a_linha() {
        let (d, log) = aberto("forjada");
        let op = "varrer\n{\"quando_ms\":1,\"evento\":\"morta\",\"op\":\"x\"}";
        log.tarefa_terminou(&lento(op, 1_000_000, 2_000), verde)
            .unwrap();
        let texto = std::fs::read_to_string(d.join(NOME_DO_ARQUIVO)).unwrap();
        assert_eq!(texto.lines().count(), 1, "a quebra virou linha: {texto}");
        let l = linhas_de(&log.consultar(&Json::Nulo).unwrap());
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].texto_ou("evento", ""), "estourou");
        let gravado = l[0].texto_ou("op", "");
        assert!(
            !gravado.contains('\n'),
            "o \\n cru ficou no valor: {gravado:?}"
        );
        assert!(gravado.starts_with("varrer\\n"), "{gravado:?}");
    }

    /// Decisao do dono: login e IP nunca vao ao aquario. O `Acesso` traz os
    /// dois; a linha nao traz nem o valor nem a chave.
    #[test]
    fn a_linha_nunca_leva_login_nem_ip() {
        let (d, log) = aberto("sem-ip");
        log.tarefa_terminou(&lento("varrer", 5_000, 1_500), verde)
            .unwrap();
        let texto = std::fs::read_to_string(d.join(NOME_DO_ARQUIVO)).unwrap();
        for proibido in ["203.0.113.77", "login-secreto", "\"ip\"", "\"usuario\""] {
            assert!(!texto.contains(proibido), "{proibido} vazou: {texto}");
        }
        let l = Json::analisar(texto.trim()).unwrap();
        assert_eq!(l.inteiro_ou("quando_ms", 0), 6_500, "o instante e o FIM");
        assert_eq!(l.inteiro_ou("ms", 0), 1_500);
        assert_eq!(l.texto_ou("tabela", ""), "vendas");
    }

    /// A linha `mudou` leva o alarme pelo nome e o id da ocorrencia -- o
    /// valor, nunca a decisao (cor, grupo).
    #[test]
    fn mudou_leva_o_alarme_e_a_ocorrencia() {
        let (_d, log) = aberto("mudou");
        log.gravar(&Linha::mudou(Alarme::ErroDeDisco, Some(42), 7_000))
            .unwrap();
        let l = linhas_de(&log.consultar(&Json::Nulo).unwrap());
        assert_eq!(l[0].texto_ou("evento", ""), "mudou");
        assert_eq!(
            Alarme::de_nome(l[0].texto_ou("alarme", "")),
            Some(Alarme::ErroDeDisco)
        );
        assert_eq!(l[0].inteiro_ou("ocorrencia", 0), 42);
        assert!(l[0].campo("grupo").is_none() && l[0].campo("cor").is_none());
    }

    /// Pedido curto nunca foi bolha, e a replicacao espera por desenho: os
    /// dois nao estouram. O de 1 s estoura.
    #[test]
    fn so_estoura_quem_viveu_no_aquario() {
        let (_d, log) = aberto("viveu");
        log.tarefa_terminou(&lento("inserir", 1, VIVEU_NO_AQUARIO_MS - 1), verde)
            .unwrap();
        log.tarefa_terminou(&lento("replicar_aguardar", 2, 30_000), verde)
            .unwrap();
        log.tarefa_terminou(&lento("inserir", 3, VIVEU_NO_AQUARIO_MS), verde)
            .unwrap();
        assert_eq!(log.gravadas(), 1);
    }

    #[test]
    fn filtros_desde_ate_e_max() {
        let (_d, log) = aberto("filtros");
        for i in 0..10i64 {
            let mut l = Linha::nova(Evento::Estourou, 1_000 + i * 100);
            l.op = format!("op{i}");
            log.gravar(&l).unwrap();
        }
        let ops = |r: Json| -> Vec<String> {
            linhas_de(&r)
                .iter()
                .map(|l| l.texto_ou("op", "").to_string())
                .collect()
        };
        let p = |s: &str| Json::analisar(s).unwrap();
        assert_eq!(
            ops(log.consultar(&p(r#"{"desde":1500,"ate":1700}"#)).unwrap()),
            ["op5", "op6", "op7"]
        );
        let r = log.consultar(&p(r#"{"max":2}"#)).unwrap();
        assert!(r.booleano_ou("truncado", false));
        assert_eq!(ops(r), ["op8", "op9"], "os mais RECENTES, em ordem");
        let r = log.consultar(&p(r#"{"desde":1800}"#)).unwrap();
        assert!(!r.booleano_ou("truncado", true));
        assert_eq!(ops(r), ["op8", "op9"]);
        for ruim in [
            r#"{"desde":"ontem"}"#,
            r#"{"max":0}"#,
            r#"{"desde":5,"ate":4}"#,
        ] {
            assert_eq!(
                log.consultar(&p(ruim)).unwrap_err().nome(),
                "TIPO_INVALIDO",
                "{ruim}"
            );
        }
    }

    /// O rodizio: 8 arquivos no maximo, e a consulta atravessa do corrente
    /// para os antigos sem perder a ordem.
    #[test]
    fn a_consulta_atravessa_o_rodizio() {
        let (d, log) = aberto("rodizio");
        {
            let mut e = travar(&log.escritor);
            e.as_mut().unwrap().definir_rodizio(400, ARQUIVOS_ANTIGOS);
        }
        for i in 0..60i64 {
            let mut l = Linha::nova(Evento::Estourou, 10_000 + i);
            l.op = format!("op{i:02}");
            log.gravar(&l).unwrap();
        }
        let caminho = d.join(NOME_DO_ARQUIVO);
        assert!(crate::rodizio::com_sufixo(&caminho, ARQUIVOS_ANTIGOS).exists());
        assert!(!crate::rodizio::com_sufixo(&caminho, ARQUIVOS_ANTIGOS + 1).exists());
        let r = log.consultar(&Json::Nulo).unwrap();
        let q: Vec<i64> = linhas_de(&r)
            .iter()
            .map(|l| l.inteiro_ou("quando_ms", 0))
            .collect();
        assert!(q.len() > 10 && q.len() < 60, "{}", q.len());
        assert!(q.windows(2).all(|w| w[0] + 1 == w[1]), "{q:?}");
        assert_eq!(*q.last().unwrap(), 10_059);
        assert_eq!(r.inteiro_ou("arquivos_lidos", 0), 8);
    }

    /// O leitor de tras para a frente, com linhas atravessando o limite do
    /// bloco, linha cortada no fim e lixo sem quebra maior que o teto.
    #[test]
    fn leitor_de_tras_atravessa_blocos_e_pula_lixo() {
        let d = DirTemp::novo("aqlog-leitor");
        let alvo = d.join("x.log");
        let mut texto = String::new();
        let mut esperadas = Vec::new();
        for i in 0..5_000 {
            let l = format!("{{\"i\":{i},\"p\":\"{}\"}}", "y".repeat(i % 97));
            texto.push_str(&l);
            texto.push('\n');
            esperadas.push(l);
            if i == 2_000 {
                texto.push_str(&"z".repeat(TETO_DA_LINHA * 3));
                texto.push('\n');
            }
        }
        texto.push_str("{\"cortada");
        std::fs::write(&alvo, &texto).unwrap();
        let mut lidas = Vec::new();
        de_tras_para_frente(File::open(&alvo).unwrap(), |t| {
            lidas.push(t.to_string());
            true
        })
        .unwrap();
        assert_eq!(lidas.remove(0), "{\"cortada");
        lidas.reverse();
        assert_eq!(lidas, esperadas);
    }

    #[test]
    fn fechado_diz_que_nao_ha_arquivo_e_nao_grava() {
        let log = LogDoAquario::default();
        log.tarefa_terminou(&lento("varrer", 1, 5_000), verde)
            .unwrap();
        assert_eq!(log.gravadas(), 0);
        let e = log.consultar(&Json::Nulo).unwrap_err();
        assert_eq!(e.nome(), "NAO_ENCONTRADO");
    }

    /// Disco cheio pelo kernel: `/dev/full` aceita abrir e recusa toda
    /// escrita com ENOSPC. A falha volta (para o servidor levar ao
    /// `evento_de_disco`), e fica contada para a consulta. A prova com tmpfs
    /// de verdade, pelo servidor, esta em `testes_do_aquario.rs`.
    #[cfg(target_os = "linux")]
    #[test]
    fn disco_cheio_conta_e_devolve_a_falha() {
        if !Path::new("/dev/full").exists() {
            eprintln!("PULADO: sem /dev/full");
            return;
        }
        let log = LogDoAquario::default();
        log.abrir("/dev/full").unwrap();
        let e = log
            .tarefa_terminou(&lento("varrer", 1, 5_000), verde)
            .expect_err("/dev/full aceitou a escrita");
        assert!(matches!(e, PhxError::Io(_)), "{e}");
        assert_eq!((log.gravadas(), log.falhas()), (0, 1));
        assert!(travar(&log.ultima_falha).is_some());
    }
}
