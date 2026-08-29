//! Jobs de execucao: um nome, uma agenda e uma operacao do protocolo.
//!
//! # O desenho ja existia
//!
//! O agendador do backup e o mesmo relogio: acorda de tempos em tempos,
//! pergunta `hora_de_rodar` e, se for, roda e anota. A unica coisa que o job
//! acrescenta e QUE operacao roda -- e como ela ja e um pedido do protocolo,
//! nao ha executor novo. `{"op":"backup", ...}` e `{"op":"reindexar", ...}`
//! entram pela mesma porta que a rede usa.
//!
//! # Um job roda como GENTE, e nao como o servidor
//!
//! Ele carrega o login de um usuario do cadastro, e roda com o poder daquele
//! usuario -- nem mais, nem menos. E deliberado, e e a parte que mais importa:
//! um agendador que roda "como o servidor" e um jeito de dar permissao a quem
//! nao a tem, escrevendo a operacao num arquivo em vez de pedi-la pela rede.
//!
//! Por isso tambem: usuario que sumiu do cadastro ou foi desativado **para o
//! job**, com erro escrito. Ele nao cai para tudo-liberado -- que e o que
//! aconteceria se o codigo apenas deixasse a sessao sem usuario.
//!
//! # O historico e append-only, como todo registro daqui
//!
//! Cada corrida vira uma linha JSON no `.log` ao lado do cadastro. A tela le a
//! cauda do arquivo. Job que falhou calado nao existe: a linha entra igual.

use std::io::Write;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// Quantos bytes do fim do `.log` a tela le. O arquivo cresce uma linha por
/// corrida; ler a cauda evita carregar meses de historico para mostrar vinte
/// linhas.
const CAUDA_DO_LOG: u64 = 64 * 1024;

/// De quanto em quanto tempo se pergunta se chegou a hora.
///
/// Trinta segundos, e nao sessenta como o backup: a menor agenda de um job e
/// de um minuto, e acordar no mesmo periodo do menor intervalo faria a hora
/// marcada escorregar quase um minuto.
pub const PERIODO_DO_RELOGIO_S: u64 = 30;

/// Quando um job roda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Agenda {
    /// Todo dia, no minuto do dia indicado. `hora` vem como "HH:MM".
    Diaria { minuto_do_dia: u64 },
    /// A cada N minutos, contados da ultima corrida.
    Cada { minutos: u64 },
}

impl Agenda {
    /// Le a agenda dos dois campos, do mesmo jeito que o backup: `hora`
    /// preenchida manda, e `cada_minutos` e o resto.
    pub fn de_json(j: &Json) -> Result<Agenda> {
        let hora = j.texto_ou("hora", "").trim().to_string();
        if !hora.is_empty() {
            let minuto_do_dia = minuto_do_dia(&hora).ok_or_else(|| {
                PhxError::Esquema(format!("hora invalida: {hora:?} (use \"HH:MM\", 24 horas)"))
            })?;
            return Ok(Agenda::Diaria { minuto_do_dia });
        }
        let minutos = j.inteiro_ou("cada_minutos", 60).max(1) as u64;
        Ok(Agenda::Cada { minutos })
    }

    pub fn rotulo(&self) -> String {
        match self {
            Agenda::Diaria { minuto_do_dia } => {
                format!(
                    "todo dia as {:02}:{:02}",
                    minuto_do_dia / 60,
                    minuto_do_dia % 60
                )
            }
            Agenda::Cada { minutos } if *minutos % 60 == 0 => {
                format!("a cada {} h", minutos / 60)
            }
            Agenda::Cada { minutos } => format!("a cada {minutos} min"),
        }
    }

    /// Ja passou da hora de rodar de novo?
    ///
    /// `ultimo_ms` zero quer dizer que nunca rodou. A regra e a mesma do
    /// backup, e pelo mesmo motivo: com hora marcada, dispara quando o minuto
    /// do dia chega E ainda nao rodou hoje -- senao dispararia a cada volta do
    /// relogio ate a meia-noite.
    pub fn hora_de_rodar(&self, agora_ms: i64, ultimo_ms: i64) -> bool {
        match self {
            Agenda::Diaria { minuto_do_dia } => {
                let minuto_agora = (agora_ms.rem_euclid(86_400_000) / 60_000) as u64;
                let dia_agora = agora_ms.div_euclid(86_400_000);
                let dia_ultimo = ultimo_ms.div_euclid(86_400_000);
                minuto_agora >= *minuto_do_dia && (ultimo_ms == 0 || dia_agora > dia_ultimo)
            }
            Agenda::Cada { minutos } => {
                let intervalo = *minutos as i64 * 60_000;
                ultimo_ms == 0 || agora_ms - ultimo_ms >= intervalo
            }
        }
    }

    /// Quando a proxima corrida deveria acontecer.
    ///
    /// E a mesma conta de `hora_de_rodar`, olhada do outro lado: la se
    /// pergunta "ja passou?", aqui "quando chega?". Vivem juntas para a tela
    /// e o relogio nunca discordarem. O instante devolvido pode estar no
    /// passado -- e ai o job esta vencido, e a proxima volta do relogio (se
    /// houver relogio) o dispara.
    pub fn proximo_ms(&self, agora_ms: i64, ultimo_ms: i64) -> i64 {
        match self {
            Agenda::Diaria { minuto_do_dia } => {
                let dia_agora = agora_ms.div_euclid(86_400_000);
                let dia_ultimo = ultimo_ms.div_euclid(86_400_000);
                // Ja rodou hoje: amanha. Senao: hoje, mesmo que o minuto ja
                // tenha passado -- vencido e informacao, nao erro.
                let dia = if ultimo_ms != 0 && dia_ultimo >= dia_agora {
                    dia_agora + 1
                } else {
                    dia_agora
                };
                dia * 86_400_000 + *minuto_do_dia as i64 * 60_000
            }
            Agenda::Cada { minutos } => {
                if ultimo_ms == 0 {
                    // Nunca rodou: a proxima e a primeira volta do relogio.
                    agora_ms
                } else {
                    ultimo_ms + *minutos as i64 * 60_000
                }
            }
        }
    }

    fn campos_para_disco(&self) -> Vec<(String, Json)> {
        match self {
            Agenda::Diaria { minuto_do_dia } => vec![(
                "hora".to_string(),
                Json::texto_de(format!(
                    "{:02}:{:02}",
                    minuto_do_dia / 60,
                    minuto_do_dia % 60
                )),
            )],
            Agenda::Cada { minutos } => {
                vec![("cada_minutos".to_string(), Json::de_u64(*minutos))]
            }
        }
    }
}

/// "HH:MM" em minutos desde a meia-noite.
pub fn minuto_do_dia(hora: &str) -> Option<u64> {
    let (h, m) = hora.split_once(':')?;
    let h: u64 = h.trim().parse().ok()?;
    let m: u64 = m.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// Um job cadastrado.
#[derive(Debug, Clone)]
pub struct Job {
    /// Apelido unico. E por ele que a tela e o `job_rodar` chamam.
    pub nome: String,
    pub descricao: String,
    /// Job nasce DESLIGADO. Um agendamento que comeca a rodar no instante em
    /// que foi salvo nao da a quem escreveu a chance de reler o pedido.
    pub ligado: bool,
    pub agenda: Agenda,
    /// Login do cadastro sob o qual a operacao roda. Vazio so vale em servidor
    /// sem cadastro nenhum, que e o mesmo caso em que a rede tambem entra sem
    /// login.
    pub usuario: String,
    /// O pedido do protocolo, sem `token`: `{"op":"...", ...}`.
    pub pedido: Json,
}

impl Job {
    pub fn de_json(j: &Json) -> Result<Job> {
        let nome = j.texto_ou("nome", "").trim().to_string();
        validar_nome(&nome)?;
        let pedido = j
            .campo("pedido")
            .cloned()
            .ok_or_else(|| PhxError::Esquema(format!("job {nome:?}: falta \"pedido\"")))?;
        let op = pedido.texto_ou("op", "").trim().to_string();
        if op.is_empty() {
            return Err(PhxError::Esquema(format!(
                "job {nome:?}: o \"pedido\" precisa de um \"op\""
            )));
        }
        // Token no pedido seria senha em arquivo por outro nome -- e o job nao
        // precisa dele: ele nao passa pela porta da rede.
        if pedido.campo("token").is_some() {
            return Err(PhxError::Esquema(format!(
                "job {nome:?}: o \"pedido\" nao leva \"token\". O job nao entra pela rede; \
                 quem manda nele e o usuario configurado"
            )));
        }
        Ok(Job {
            nome,
            descricao: j.texto_ou("descricao", "").trim().to_string(),
            ligado: j.booleano_ou("ligado", false),
            agenda: Agenda::de_json(j)?,
            usuario: j.texto_ou("usuario", "").trim().to_string(),
            pedido,
        })
    }

    pub fn op(&self) -> &str {
        self.pedido.texto_ou("op", "")
    }

    fn pares(&self) -> Vec<(String, Json)> {
        let mut p = vec![
            ("nome".to_string(), Json::texto_de(&self.nome)),
            ("descricao".to_string(), Json::texto_de(&self.descricao)),
            ("ligado".to_string(), Json::Bool(self.ligado)),
            ("usuario".to_string(), Json::texto_de(&self.usuario)),
        ];
        p.extend(self.agenda.campos_para_disco());
        p.push(("pedido".to_string(), self.pedido.clone()));
        p
    }

    pub fn para_disco(&self) -> Json {
        Json::Objeto(self.pares())
    }

    /// A ficha que a tela recebe. Acrescenta o que e derivado, para a tela nao
    /// ter de recalcular a agenda a partir de dois campos.
    pub fn ficha(&self) -> Json {
        let mut p = self.pares();
        p.push(("agenda".to_string(), Json::texto_de(self.agenda.rotulo())));
        p.push(("op".to_string(), Json::texto_de(self.op())));
        Json::Objeto(p)
    }
}

/// De quanto em quanto tempo o vigia de jobs parados confere.
///
/// O dobro do relogio: se ha relogio no ar, um job vencido roda em ate 30 s,
/// e o vigia nunca o ve parado. Conferir mais rapido que isso so compraria
/// alarme falso.
pub const PERIODO_DO_VIGIA_S: u64 = 60;

/// O estado que a tela pinta, um por job.
///
/// A ordem e de prioridade e importa: o que acontece AGORA ganha do que ja
/// aconteceu (rodando primeiro), e o desligado ganha do historico -- um job
/// desligado nao esta "ok", esta fora da agenda. `ultima_ok` e o resultado da
/// ultima corrida CONHECIDA (a semeada do log conta), `None` quando nunca
/// rodou.
pub fn estado_do_job(
    ligado: bool,
    rodando: bool,
    ultima_ok: Option<bool>,
    relogio_no_ar: bool,
) -> &'static str {
    if rodando {
        return "rodando";
    }
    if !ligado {
        return "desligado";
    }
    match ultima_ok {
        Some(true) => "ok",
        Some(false) => "falhou",
        // Nunca rodou -- e a diferenca entre os dois e se ALGUEM vai rodar:
        // com relogio, e so esperar; sem, o job esta ligado e abandonado.
        None if relogio_no_ar => "agendado",
        None => "nunca_rodou",
    }
}

/// Um job PARADO: ligado, com a hora vencida, e sem ninguem para roda-lo --
/// o relogio nao subiu neste arranque (ele so sobe se havia job ligado no
/// arranque) e nao ha corrida em andamento.
///
/// `vencido` sai da MESMA `hora_de_rodar` do relogio, de proposito: se o
/// relogio esta no ar, um vencido roda em ate 30 s e nunca e parado. A tela
/// e o vigia de e-mail usam esta funcao, para os dois nunca discordarem.
pub fn job_parado(ligado: bool, rodando: bool, vencido: bool, relogio_no_ar: bool) -> bool {
    ligado && !rodando && vencido && !relogio_no_ar
}

/// Decide se um aviso repetido sai agora -- e anota que saiu.
///
/// Mesmo desenho do vigia de disco, pelo mesmo motivo: um job quebrado
/// continua quebrado, e avisar a cada corrida vira enxurrada. Quem limpa a
/// chave quando o problema alivia devolve o direito de avisar na hora.
pub fn pode_avisar(
    avisados: &mut std::collections::HashMap<String, i64>,
    chave: &str,
    agora_ms: i64,
    silencio_ms: i64,
) -> bool {
    match avisados.get(chave) {
        Some(quando) if agora_ms - *quando < silencio_ms => false,
        _ => {
            avisados.insert(chave.to_string(), agora_ms);
            true
        }
    }
}

/// Nome de job: letra, digito, `_` e `-`, ate 48. Igual ao do DbLink, e pelo
/// mesmo motivo -- ele aparece em log e em tela, e vira argumento de comando.
pub fn validar_nome(nome: &str) -> Result<()> {
    if nome.is_empty() {
        return Err(PhxError::Esquema("o job precisa de um \"nome\"".into()));
    }
    if nome.len() > 48 {
        return Err(PhxError::Esquema(format!(
            "nome de job longo demais: {nome:?} (maximo 48)"
        )));
    }
    if !nome
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(PhxError::Esquema(format!(
            "nome de job invalido: {nome:?} (use letra, digito, _ ou -)"
        )));
    }
    Ok(())
}

/// O que ficou registrado de uma corrida.
#[derive(Debug, Clone)]
pub struct Corrida {
    pub quando_ms: i64,
    pub job: String,
    pub op: String,
    pub usuario: String,
    pub ok: bool,
    pub duracao_ms: i64,
    /// Resumo da resposta, ou o texto do erro. Nunca o corpo inteiro: uma
    /// varredura de vinte mil linhas nao cabe -- e nao interessa -- no log.
    pub detalhe: String,
}

impl Corrida {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("quando_ms", Json::de_i64(self.quando_ms)),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.quando_ms)),
            ),
            ("job", Json::texto_de(&self.job)),
            ("op", Json::texto_de(&self.op)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("ok", Json::Bool(self.ok)),
            ("duracao_ms", Json::de_i64(self.duracao_ms)),
            ("detalhe", Json::texto_de(&self.detalhe)),
        ])
    }

    fn de_json(j: &Json) -> Option<Corrida> {
        Some(Corrida {
            quando_ms: j.campo("quando_ms")?.inteiro()?,
            job: j.texto_ou("job", "").to_string(),
            op: j.texto_ou("op", "").to_string(),
            usuario: j.texto_ou("usuario", "").to_string(),
            ok: j.booleano_ou("ok", false),
            duracao_ms: j.inteiro_ou("duracao_ms", 0),
            detalhe: j.texto_ou("detalhe", "").to_string(),
        })
    }
}

/// O cadastro dos jobs, em arquivo proprio.
///
/// Separado do `config.json` pelo mesmo motivo do DbLink: o cadastro muda pela
/// tela, e reescrever o `config.json` inteiro a cada job novo arriscaria os
/// comentarios e o resto da configuracao.
#[derive(Debug)]
pub struct Registro {
    pub caminho: PathBuf,
    pub jobs: Vec<Job>,
    /// Quando cada job rodou pela ultima vez, em memoria.
    ///
    /// Vive so enquanto o processo vive, e e de proposito: depois de um
    /// reinicio, um job "a cada 6 h" roda uma vez logo -- que e o
    /// comportamento do backup agendado e o que se quer de um relogio que
    /// perdeu a hora. Quem precisa de "no maximo uma vez por dia" usa `hora`.
    ultimos: Vec<(String, i64)>,
    /// A ultima corrida CONHECIDA de cada job, para a tela e o estado.
    ///
    /// Semeada da cauda do log no `abrir`, para "falhou as 03:00" sobreviver
    /// a um reinicio. So informa -- NAO alimenta o agendamento: `ultimos`
    /// continua zerando a cada arranque, pelo motivo escrito nele.
    corridas: Vec<(String, Corrida)>,
}

impl Registro {
    /// Le o arquivo. Arquivo que nao existe e cadastro vazio, e nao erro.
    pub fn abrir(caminho: &Path) -> Result<Registro> {
        let mut r = Registro {
            caminho: caminho.to_path_buf(),
            jobs: Vec::new(),
            ultimos: Vec::new(),
            corridas: Vec::new(),
        };
        let Ok(texto) = std::fs::read_to_string(caminho) else {
            r.semear_corridas();
            return Ok(r);
        };
        if texto.trim().is_empty() {
            r.semear_corridas();
            return Ok(r);
        }
        let j = Json::analisar(&texto)?;
        let lista = j
            .campo("jobs")
            .and_then(Json::lista)
            .or_else(|| j.lista())
            .ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{}: esperava uma lista de jobs, ou um objeto com \"jobs\"",
                    caminho.display()
                ))
            })?;
        for item in lista {
            r.jobs.push(Job::de_json(item)?);
        }
        r.conferir_repetidos()?;
        r.semear_corridas();
        Ok(r)
    }

    /// Recupera da cauda do log a ultima corrida de cada job.
    ///
    /// A cauda basta: quem nao aparece nos ultimos 64 KB nao roda ha muito, e
    /// para ele a tela dizer "nunca rodou (que se saiba)" e mais honesto que
    /// carregar meses de log no arranque.
    fn semear_corridas(&mut self) {
        // O historico vem da mais nova para a mais velha, entao a primeira
        // ocorrencia de cada nome e a que fica.
        for c in self.historico(usize::MAX) {
            if !self
                .corridas
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case(&c.job))
            {
                self.corridas.push((c.job.clone(), c));
            }
        }
    }

    fn conferir_repetidos(&self) -> Result<()> {
        let mut vistos = std::collections::HashSet::new();
        for j in &self.jobs {
            if !vistos.insert(j.nome.to_lowercase()) {
                return Err(PhxError::Esquema(format!(
                    "dois jobs com o nome {:?}: o apelido tem de ser unico",
                    j.nome
                )));
            }
        }
        Ok(())
    }

    pub fn achar(&self, nome: &str) -> Result<&Job> {
        self.jobs
            .iter()
            .find(|j| j.nome.eq_ignore_ascii_case(nome))
            .ok_or_else(|| PhxError::NaoEncontrado(format!("job {nome:?} nao existe")))
    }

    /// Grava ou substitui um job pelo nome.
    pub fn salvar(&mut self, j: Job) -> Result<()> {
        match self
            .jobs
            .iter()
            .position(|x| x.nome.eq_ignore_ascii_case(&j.nome))
        {
            Some(i) => self.jobs[i] = j,
            None => self.jobs.push(j),
        }
        self.gravar()
    }

    pub fn excluir(&mut self, nome: &str) -> Result<()> {
        let antes = self.jobs.len();
        self.jobs.retain(|j| !j.nome.eq_ignore_ascii_case(nome));
        if self.jobs.len() == antes {
            return Err(PhxError::NaoEncontrado(format!("job {nome:?} nao existe")));
        }
        self.ultimos.retain(|(n, _)| !n.eq_ignore_ascii_case(nome));
        // A ultima corrida sai junto: um job recriado com o mesmo nome e um
        // job novo, e nao pode nascer vestindo o resultado do antigo.
        self.corridas.retain(|(n, _)| !n.eq_ignore_ascii_case(nome));
        self.gravar()
    }

    fn gravar(&self) -> Result<()> {
        let j = Json::objeto(vec![(
            "jobs",
            Json::Lista(self.jobs.iter().map(Job::para_disco).collect()),
        )]);
        if let Some(pai) = self.caminho.parent() {
            if !pai.as_os_str().is_empty() {
                std::fs::create_dir_all(pai)?;
            }
        }
        std::fs::write(&self.caminho, j.escrever_identado())?;
        Ok(())
    }

    pub fn ultimo_de(&self, nome: &str) -> i64 {
        self.ultimos
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, t)| *t)
            .unwrap_or(0)
    }

    pub fn anotar_corrida(&mut self, nome: &str, quando_ms: i64) {
        match self
            .ultimos
            .iter_mut()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
        {
            Some(p) => p.1 = quando_ms,
            None => self.ultimos.push((nome.to_string(), quando_ms)),
        }
    }

    /// Quais jobs devem rodar agora. Devolve os nomes, e nao os jobs, porque
    /// quem chama vai soltar a trava antes de executar.
    pub fn vencidos(&self, agora_ms: i64) -> Vec<String> {
        self.jobs
            .iter()
            .filter(|j| j.ligado && j.agenda.hora_de_rodar(agora_ms, self.ultimo_de(&j.nome)))
            .map(|j| j.nome.clone())
            .collect()
    }

    /// O `.log` das corridas, ao lado do cadastro.
    pub fn caminho_do_log(&self) -> PathBuf {
        self.caminho.with_extension("log")
    }

    /// A ultima corrida conhecida deste job, se houver.
    pub fn ultima_corrida_de(&self, nome: &str) -> Option<&Corrida> {
        self.corridas
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, c)| c)
    }

    /// Anota a corrida no fim do arquivo. Falhar aqui nao pode derrubar o job:
    /// perder a linha do historico e ruim; nao rodar o job por causa dela e
    /// pior.
    pub fn registrar(&mut self, c: &Corrida) {
        match self
            .corridas
            .iter_mut()
            .find(|(n, _)| n.eq_ignore_ascii_case(&c.job))
        {
            Some(p) => p.1 = c.clone(),
            None => self.corridas.push((c.job.clone(), c.clone())),
        }
        let caminho = self.caminho_do_log();
        if let Some(pai) = caminho.parent() {
            if !pai.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(pai);
            }
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&caminho)
        {
            let _ = writeln!(f, "{}", c.para_json().escrever());
        }
    }

    /// As ultimas corridas, da mais nova para a mais velha.
    pub fn historico(&self, quantas: usize) -> Vec<Corrida> {
        let caminho = self.caminho_do_log();
        let Ok(mut f) = std::fs::File::open(&caminho) else {
            return Vec::new();
        };
        use std::io::{Read, Seek, SeekFrom};
        let tamanho = f.metadata().map(|m| m.len()).unwrap_or(0);
        // Le so a cauda. Se ela cair no meio de uma linha, a primeira linha do
        // pedaco sai quebrada -- e por isso ela e descartada quando nao
        // analisa. Perder a linha mais velha do recorte nao muda nada.
        let de = tamanho.saturating_sub(CAUDA_DO_LOG);
        if f.seek(SeekFrom::Start(de)).is_err() {
            return Vec::new();
        }
        let mut texto = String::new();
        if f.read_to_string(&mut texto).is_err() {
            return Vec::new();
        }
        let mut saida: Vec<Corrida> = texto
            .lines()
            .filter_map(|l| Json::analisar(l).ok())
            .filter_map(|j| Corrida::de_json(&j))
            .collect();
        saida.reverse();
        saida.truncate(quantas);
        saida
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phxsql-jobs-{}-{}", std::process::id(), nome));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("jobs.json")
    }

    fn job_json(nome: &str, extra: &str) -> Json {
        Json::analisar(&format!(
            "{{\"nome\":\"{nome}\",\"usuario\":\"adm\",\
              \"pedido\":{{\"op\":\"backup\",\"database\":\"Comercial\"}}{extra}}}"
        ))
        .unwrap()
    }

    #[test]
    fn job_nasce_desligado() {
        let j = Job::de_json(&job_json("noturno", "")).unwrap();
        assert!(
            !j.ligado,
            "um job que comeca a rodar no instante em que foi salvo nao da chance de reler"
        );
    }

    #[test]
    fn pedido_precisa_de_op() {
        let j = Json::analisar("{\"nome\":\"x\",\"pedido\":{}}").unwrap();
        let e = Job::de_json(&j).unwrap_err().to_string();
        assert!(e.contains("op"), "{e}");
    }

    #[test]
    fn pedido_com_token_e_recusado() {
        let j =
            Json::analisar("{\"nome\":\"x\",\"pedido\":{\"op\":\"ping\",\"token\":\"segredo\"}}")
                .unwrap();
        let e = Job::de_json(&j).unwrap_err().to_string();
        assert!(e.contains("token"), "{e}");
    }

    #[test]
    fn nome_hostil_nao_passa() {
        for n in ["", "com espaco", "../fora", "a".repeat(49).as_str()] {
            assert!(validar_nome(n).is_err(), "{n:?} devia ser recusado");
        }
        assert!(validar_nome("limpeza-noturna_2").is_ok());
    }

    #[test]
    fn agenda_diaria_dispara_uma_vez_por_dia() {
        let a = Agenda::de_json(&Json::analisar("{\"hora\":\"03:00\"}").unwrap()).unwrap();
        assert_eq!(a, Agenda::Diaria { minuto_do_dia: 180 });
        let dia = 20_000i64 * 86_400_000;
        let as_tres = dia + 3 * 3_600_000;
        assert!(!a.hora_de_rodar(dia + 2 * 3_600_000, 0));
        assert!(a.hora_de_rodar(as_tres, 0));
        // Rodou: nao roda de novo hoje, mas roda amanha.
        assert!(!a.hora_de_rodar(as_tres + 60_000, as_tres));
        assert!(a.hora_de_rodar(as_tres + 86_400_000, as_tres));
    }

    #[test]
    fn agenda_por_intervalo() {
        let a = Agenda::de_json(&Json::analisar("{\"cada_minutos\":15}").unwrap()).unwrap();
        assert_eq!(a.rotulo(), "a cada 15 min");
        assert!(a.hora_de_rodar(1_000_000, 0), "nunca rodou, roda ja");
        assert!(!a.hora_de_rodar(1_000_000 + 14 * 60_000, 1_000_000));
        assert!(a.hora_de_rodar(1_000_000 + 15 * 60_000, 1_000_000));
    }

    #[test]
    fn hora_invalida_e_erro_escrito() {
        let e = Agenda::de_json(&Json::analisar("{\"hora\":\"25:00\"}").unwrap())
            .unwrap_err()
            .to_string();
        assert!(e.contains("HH:MM"), "{e}");
    }

    #[test]
    fn cadastro_vai_e_volta_do_disco() {
        let caminho = tmp("ida-e-volta");
        let mut r = Registro::abrir(&caminho).unwrap();
        assert!(r.jobs.is_empty(), "arquivo que nao existe e cadastro vazio");
        let mut j = Job::de_json(&job_json("noturno", ",\"hora\":\"03:00\"")).unwrap();
        j.ligado = true;
        r.salvar(j).unwrap();

        let r2 = Registro::abrir(&caminho).unwrap();
        assert_eq!(r2.jobs.len(), 1);
        assert_eq!(r2.jobs[0].nome, "noturno");
        assert!(r2.jobs[0].ligado);
        assert_eq!(r2.jobs[0].agenda, Agenda::Diaria { minuto_do_dia: 180 });
        assert_eq!(r2.jobs[0].op(), "backup");
    }

    #[test]
    fn nome_repetido_no_arquivo_e_erro() {
        let caminho = tmp("repetido");
        std::fs::write(
            &caminho,
            "{\"jobs\":[{\"nome\":\"a\",\"pedido\":{\"op\":\"ping\"}},\
                       {\"nome\":\"A\",\"pedido\":{\"op\":\"ping\"}}]}",
        )
        .unwrap();
        let e = Registro::abrir(&caminho).unwrap_err().to_string();
        assert!(e.contains("unico"), "{e}");
    }

    #[test]
    fn salvar_pelo_nome_substitui() {
        let caminho = tmp("substitui");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(Job::de_json(&job_json("x", "")).unwrap()).unwrap();
        let mut segundo = Job::de_json(&job_json("x", "")).unwrap();
        segundo.descricao = "segunda versao".into();
        r.salvar(segundo).unwrap();
        assert_eq!(r.jobs.len(), 1);
        assert_eq!(r.jobs[0].descricao, "segunda versao");
    }

    #[test]
    fn excluir_o_que_nao_existe_avisa() {
        let caminho = tmp("excluir");
        let mut r = Registro::abrir(&caminho).unwrap();
        assert!(r.excluir("fantasma").is_err());
    }

    #[test]
    fn so_o_ligado_e_vencido() {
        let caminho = tmp("vencidos");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(Job::de_json(&job_json("desligado", ",\"cada_minutos\":1")).unwrap())
            .unwrap();
        let mut lig = Job::de_json(&job_json("ligado", ",\"cada_minutos\":1")).unwrap();
        lig.ligado = true;
        r.salvar(lig).unwrap();
        assert_eq!(r.vencidos(1_000_000), vec!["ligado".to_string()]);
        // Depois de anotado, so vence de novo passado o intervalo.
        r.anotar_corrida("ligado", 1_000_000);
        assert!(r.vencidos(1_000_000 + 30_000).is_empty());
        assert_eq!(r.vencidos(1_000_000 + 60_000), vec!["ligado".to_string()]);
    }

    #[test]
    fn historico_le_a_cauda_do_log() {
        let caminho = tmp("historico");
        let mut r = Registro::abrir(&caminho).unwrap();
        assert!(r.historico(10).is_empty(), "sem log ainda");
        for i in 0..5 {
            r.registrar(&Corrida {
                quando_ms: 1_000 + i,
                job: format!("j{i}"),
                op: "ping".into(),
                usuario: "adm".into(),
                ok: i % 2 == 0,
                duracao_ms: i,
                detalhe: "ok".into(),
            });
        }
        let h = r.historico(3);
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].job, "j4", "a mais nova vem primeiro");
        assert!(h[0].ok, "a j4 foi gravada com ok");
        assert!(!h[1].ok, "e a j3 sem ok -- as duas voltam como entraram");
    }

    #[test]
    fn a_proxima_prevista_e_o_outro_lado_da_hora_de_rodar() {
        // A cada 15 min: a proxima e a ultima mais o intervalo -- e e exatamente
        // o instante em que `hora_de_rodar` comeca a dizer sim.
        let a = Agenda::Cada { minutos: 15 };
        let proxima = a.proximo_ms(2_000_000, 1_000_000);
        assert_eq!(proxima, 1_000_000 + 15 * 60_000);
        assert!(!a.hora_de_rodar(proxima - 1, 1_000_000));
        assert!(a.hora_de_rodar(proxima, 1_000_000));
        // Nunca rodou: a proxima e a primeira volta do relogio, ou seja, ja.
        assert_eq!(a.proximo_ms(2_000_000, 0), 2_000_000);

        // Diaria as 03:00. Rodou hoje: amanha. Nao rodou: hoje -- mesmo que o
        // minuto ja tenha passado, porque vencido e informacao, nao erro.
        let d = Agenda::Diaria { minuto_do_dia: 180 };
        let dia = 20_000i64 * 86_400_000;
        let as_tres = dia + 3 * 3_600_000;
        assert_eq!(d.proximo_ms(dia + 3_600_000, 0), as_tres, "ainda vem hoje");
        assert_eq!(
            d.proximo_ms(as_tres + 3_600_000, as_tres),
            as_tres + 86_400_000,
            "rodou hoje, a proxima e amanha"
        );
        let vencida = d.proximo_ms(dia + 5 * 3_600_000, 0);
        assert_eq!(vencida, as_tres, "nunca rodou e o minuto passou: vencida");
        assert!(d.hora_de_rodar(dia + 5 * 3_600_000, 0));
    }

    #[test]
    fn o_estado_segue_a_prioridade() {
        // O agora ganha do historico; o desligado ganha do resultado.
        assert_eq!(estado_do_job(true, true, Some(false), true), "rodando");
        assert_eq!(estado_do_job(false, false, Some(true), true), "desligado");
        assert_eq!(estado_do_job(true, false, Some(true), true), "ok");
        assert_eq!(estado_do_job(true, false, Some(false), true), "falhou");
        assert_eq!(estado_do_job(true, false, None, true), "agendado");
        assert_eq!(estado_do_job(true, false, None, false), "nunca_rodou");
    }

    #[test]
    fn parado_e_vencido_sem_relogio() {
        // Com relogio no ar um vencido roda em ate 30 s: nao esta parado.
        assert!(!job_parado(true, false, true, true));
        assert!(job_parado(true, false, true, false));
        // Desligado, rodando ou em dia nao sao parado.
        assert!(!job_parado(false, false, true, false));
        assert!(!job_parado(true, true, true, false));
        assert!(!job_parado(true, false, false, false));
    }

    #[test]
    fn o_silencio_segura_o_aviso_repetido() {
        let mut avisados = std::collections::HashMap::new();
        assert!(pode_avisar(&mut avisados, "falha:x", 1_000, 3_600_000));
        assert!(
            !pode_avisar(&mut avisados, "falha:x", 2_000, 3_600_000),
            "o mesmo problema nao vira enxurrada"
        );
        assert!(
            pode_avisar(&mut avisados, "parado:x", 2_000, 3_600_000),
            "outra chave e outra noticia"
        );
        assert!(
            pode_avisar(&mut avisados, "falha:x", 1_000 + 3_600_000, 3_600_000),
            "passado o silencio, avisa de novo"
        );
        // E quem limpa a chave devolve o direito de avisar na hora -- e o que
        // o servidor faz quando o job volta a rodar bem.
        avisados.remove("falha:x");
        assert!(pode_avisar(
            &mut avisados,
            "falha:x",
            1_000 + 3_600_000,
            3_600_000
        ));
    }

    #[test]
    fn a_ultima_corrida_sobrevive_ao_reinicio_sem_mexer_no_relogio() {
        let caminho = tmp("semeada");
        {
            let mut r = Registro::abrir(&caminho).unwrap();
            r.salvar(Job::de_json(&job_json("noturno", ",\"cada_minutos\":60")).unwrap())
                .unwrap();
            for (quando, ok) in [(1_000, true), (2_000, false)] {
                r.registrar(&Corrida {
                    quando_ms: quando,
                    job: "noturno".into(),
                    op: "backup".into(),
                    usuario: "adm".into(),
                    ok,
                    duracao_ms: 7,
                    detalhe: if ok {
                        "ok".into()
                    } else {
                        "disco cheio".into()
                    },
                });
            }
            let u = r.ultima_corrida_de("noturno").unwrap();
            assert!(!u.ok, "a ultima e a de 2000, que falhou");
        }
        // Reabriu: a ultima corrida volta da cauda do log...
        let r2 = Registro::abrir(&caminho).unwrap();
        let u = r2.ultima_corrida_de("noturno").unwrap();
        assert_eq!(u.quando_ms, 2_000);
        assert_eq!(u.detalhe, "disco cheio");
        // ...e o agendamento NAO: `ultimos` zera de proposito, para um
        // "a cada 6 h" rodar logo depois do arranque.
        assert_eq!(r2.ultimo_de("noturno"), 0);
    }

    #[test]
    fn excluir_apaga_a_ultima_corrida_junto() {
        let caminho = tmp("excluir-corrida");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(Job::de_json(&job_json("x", "")).unwrap()).unwrap();
        r.registrar(&Corrida {
            quando_ms: 1,
            job: "x".into(),
            op: "ping".into(),
            usuario: String::new(),
            ok: true,
            duracao_ms: 1,
            detalhe: "ok".into(),
        });
        r.excluir("x").unwrap();
        assert!(
            r.ultima_corrida_de("x").is_none(),
            "job recriado nao pode nascer vestindo o resultado do antigo"
        );
    }

    #[test]
    fn linha_quebrada_no_recorte_e_descartada() {
        let caminho = tmp("quebrada");
        let r = Registro::abrir(&caminho).unwrap();
        std::fs::write(
            r.caminho_do_log(),
            "ndo\":1}\n{\"quando_ms\":2,\"job\":\"bom\",\"ok\":true}\n",
        )
        .unwrap();
        let h = r.historico(10);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].job, "bom");
    }
}
