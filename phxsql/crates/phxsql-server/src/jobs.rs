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

#[cfg(test)]
use crate::apoio_teste::DirTemp;
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
            // A hora recebida sai pelo `citar` do motor -- pedido 453: a
            // recusa vai ao cliente e ao `acessos.log`, e quem escolhe o
            // tamanho do campo e quem manda o pedido.
            let minuto_do_dia = minuto_do_dia(&hora).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "hora invalida: {} (use \"HH:MM\", 24 horas)",
                    phxsql_core::error::citar(&hora)
                ))
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
    /// O pedido do protocolo, sem credencial nenhuma: `{"op":"...", ...}`.
    ///
    /// Sem `token`, sem `senha`, sem `token_remoto` -- a lista e a de
    /// `crate::segredos`, a mesma pela qual o profiler redige. Este campo vai
    /// para o `jobs.json` e volta na ficha, e senha em arquivo por outro nome
    /// continua senha em arquivo.
    pub pedido: Json,
    /// O segredo que a guarda achou no pedido que VOLTOU do `jobs.json` --
    /// a frase que nomeia o campo, nunca o valor. `None` no job de sempre.
    ///
    /// Existe porque a guarda muda e o arquivo nao: o pedido 497 passou a
    /// perguntar pelas LETRAS `PASSWORD`, e o job legitimo `SELECT login,
    /// password_hash FROM contas`, salvo antes, derrubava o arranque inteiro
    /// (parecer SEC da quarta volta, R1). O job assim fica no cadastro e
    /// RECUSA ao rodar -- ver [`Job::recusa_de_credencial`]. Nao vai para o
    /// disco: sai de novo da guarda a cada leitura.
    pub recusa: Option<String>,
}

impl Job {
    /// O job que CHEGA -- pelo `job_salvar`. Credencial no pedido recusa, e a
    /// recusa nomeia o campo.
    ///
    /// O que volta do disco no arranque passa por [`Job::do_disco`], que nao
    /// recusa: a mesma guarda, com outra consequencia.
    pub fn de_json(j: &Json) -> Result<Job> {
        let job = Job::do_disco(j)?;
        match job.recusa_de_credencial() {
            Some(e) => Err(e),
            None => Ok(job),
        }
    }

    /// O job que VOLTA do `jobs.json`. O arquivo torto continua recusando o
    /// arranque; o pedido que casa a guarda de credencial nao.
    ///
    /// # Por que o job fica RECUSADO AO RODAR, e nao desligado
    ///
    /// «Guarda nova entra pedida, nao imposta»: o arquivo e do dono, e o
    /// servidor nao reescreve a decisao dele. Desligar em memoria vazaria
    /// para o disco no proximo `gravar` de OUTRO job, calado; e nao seguraria
    /// nada -- o `job_ligar` vira a chave sem passar por aqui, e o `job_rodar`
    /// da tela roda job desligado. A porta por onde TODA corrida passa e o
    /// `executar_job`, e e la que a recusa mora: pela agenda e pela tela, com
    /// o motivo no historico, no `acessos.log` e no aviso por e-mail -- o
    /// lugar para onde quem cuida do job ja olha. E o arranque diz o nome dele
    /// ([`Registro::avisos`]).
    pub fn do_disco(j: &Json) -> Result<Job> {
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
        // Credencial no pedido seria senha em arquivo por outro nome: o
        // cadastro vai para o `jobs.json` e volta inteiro na ficha. A guarda
        // recusava UM nome (`token`) e deixava `senha`, `senha_hash`,
        // `token_remoto` e `prova` passarem -- revisao SEC de 17/09/2026,
        // achado A2. Agora e a lista da casa, em qualquer profundidade, e a
        // recusa NOMEIA o campo: quem recebe o erro precisa saber o que tirar.
        //
        // Recusar, e nao redigir: o job EXECUTA o pedido, e um
        // `usuario_alterar` com a senha tapada trocaria a senha por `***`.
        // Aqui so se ANOTA: quem recusa e o `de_json` (ao salvar) e o
        // `executar_job` (ao rodar), pela mesma `recusa_de_credencial`.
        let recusa = crate::segredos::achar_segredo(&pedido);
        Ok(Job {
            nome,
            descricao: j.texto_ou("descricao", "").trim().to_string(),
            ligado: j.booleano_ou("ligado", false),
            agenda: Agenda::de_json(j)?,
            usuario: j.texto_ou("usuario", "").trim().to_string(),
            pedido,
            recusa,
        })
    }

    /// A recusa por credencial no pedido, se houver -- UM texto para as duas
    /// portas que recusam (salvar e rodar), para as duas nunca divergirem.
    pub fn recusa_de_credencial(&self) -> Option<PhxError> {
        let achado = self.recusa.as_deref()?;
        Some(PhxError::Esquema(format!(
            "job {:?}: o \"pedido\" leva {achado}, e credencial nao entra em job -- \
             o cadastro fica em arquivo e volta na ficha. O `token` nao e preciso (o job \
             nao entra pela rede; quem manda nele e o usuario configurado); para uma \
             ligacao, use `senha_env`/`token_remoto_env` com o nome da variavel de ambiente",
            self.nome
        )))
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
    ///
    /// O `pedido` sai pela redacao do Profiler, a MESMA arvore. No job que o
    /// `job_salvar` aceitou ela nao muda nada -- a guarda e a redacao fazem a
    /// mesma pergunta. No que voltou do disco com credencial ela tapa o
    /// valor: aceitar esse arquivo no arranque nao pode virar devolver a
    /// senha dele na resposta da tela. E o recado diz o campo, nao o valor.
    pub fn ficha(&self) -> Json {
        let mut p = self.pares();
        if let Some((_, pedido)) = p.iter_mut().find(|(k, _)| k == "pedido") {
            *pedido = crate::profiler::limpar(pedido);
        }
        p.push(("agenda".to_string(), Json::texto_de(self.agenda.rotulo())));
        p.push(("op".to_string(), Json::texto_de(self.op())));
        if let Some(achado) = &self.recusa {
            p.push(("recusado".to_string(), Json::texto_de(achado)));
        }
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
    /// A linha que ABRE a corrida, antes de executar -- pedido 502. A que a
    /// fecha vem depois, sem esta marca; a abertura sem fechamento e a
    /// corrida que derrubou o processo. Ver [`Registro::fechar_interrompidas`].
    pub em_curso: bool,
}

impl Corrida {
    pub fn para_json(&self) -> Json {
        let mut j = self.para_json_sem_marca();
        // So quando e abertura: a linha que fecha continua byte a byte a de
        // sempre, e o leitor de antes desta marca a le igual.
        if self.em_curso {
            if let Json::Objeto(pares) = &mut j {
                pares.push(("em_curso".to_string(), Json::Bool(true)));
            }
        }
        j
    }

    fn para_json_sem_marca(&self) -> Json {
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
            em_curso: j.booleano_ou("em_curso", false),
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
    /// O arquivo existe e NAO abriu -- o irmao do pedido 466. O texto diz por
    /// que e nomeia o arquivo. Ver [`Registro::abrir_ou_trancar`].
    ilegivel: Option<String>,
}

impl Registro {
    /// O cadastro do ARRANQUE: o que [`Registro::abrir`] le, ou um cadastro
    /// TRANCADO quando o arquivo nao abre.
    ///
    /// E o irmao do DbLink no pedido 466, e pelo mesmo motivo: o `?` do
    /// `Servidor::novo` chamava as duas aberturas uma depois da outra, e um
    /// `jobs.json` torto derrubava o motor inteiro -- por causa de um
    /// agendador. Trancado, o motor sobe sem relogio de jobs (nao ha job
    /// nenhum ligado), e toda operacao de job recusa dizendo o motivo. E nao
    /// grava: o arquivo que nao abriu fica como esta, em vez de ser regravado
    /// so com o job novo.
    pub fn abrir_ou_trancar(caminho: &Path) -> Registro {
        match Registro::abrir(caminho) {
            Ok(r) => r,
            Err(e) => Registro {
                caminho: caminho.to_path_buf(),
                jobs: Vec::new(),
                ultimos: Vec::new(),
                corridas: Vec::new(),
                ilegivel: Some(format!(
                    "os jobs estao TRANCADOS: o cadastro {} nao abriu no arranque ({e}). \
                     Nenhum job roda, e nenhum e gravado, ate o arquivo ser consertado e o \
                     servidor reiniciado -- gravar agora regravaria o arquivo por cima dos \
                     jobs que ele ainda guarda. O resto do servidor esta de pe",
                    caminho.display()
                )),
            },
        }
    }

    /// Recusa, com o motivo e o arquivo, se o cadastro esta trancado.
    pub fn exigir_legivel(&self) -> Result<()> {
        match &self.ilegivel {
            Some(motivo) => Err(PhxError::Corrompido(motivo.clone())),
            None => Ok(()),
        }
    }

    /// O motivo do tranca, para o aviso do arranque.
    pub fn trancado(&self) -> Option<&str> {
        self.ilegivel.as_deref()
    }

    /// Le o arquivo. Arquivo que nao existe e cadastro vazio, e nao erro.
    pub fn abrir(caminho: &Path) -> Result<Registro> {
        let mut r = Registro {
            caminho: caminho.to_path_buf(),
            jobs: Vec::new(),
            ultimos: Vec::new(),
            corridas: Vec::new(),
            ilegivel: None,
        };
        // So o arquivo que NAO EXISTE e cadastro vazio: o que existe e nao se
        // le virava vazio tambem, e o primeiro `job_salvar` o regravava so com
        // o job novo (o irmao do pedido 466).
        let texto = match std::fs::read_to_string(caminho) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                r.semear_corridas();
                return Ok(r);
            }
            Err(e) => {
                return Err(PhxError::Corrompido(format!(
                    "{}: o arquivo existe e nao se le ({e})",
                    caminho.display()
                )))
            }
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
            // `do_disco`, e nao `de_json`: a guarda de credencial nao derruba
            // o arranque -- ver `Job::do_disco`.
            r.jobs.push(Job::do_disco(item)?);
        }
        r.conferir_repetidos()?;
        r.semear_corridas();
        Ok(r)
    }

    /// Os avisos do arranque: um por job que voltou do disco com credencial
    /// no pedido, pela MESMA lista dos outros avisos (a que o `main` imprime)
    /// -- o molde do `dblink::Registro::avisos`, que tranca a ligacao e deixa
    /// o resto subir.
    ///
    /// Nomeia o job e o CAMPO, que e o que diz onde mexer; nunca o pedido.
    pub fn avisos(&self) -> Vec<String> {
        self.jobs
            .iter()
            .filter_map(|j| {
                j.recusa.as_ref().map(|achado| {
                    format!(
                        "job {:?} ({}): o \"pedido\" leva {achado}, e credencial nao entra \
                         em job. O job fica no cadastro e RECUSA ao rodar, pela agenda e \
                         pela tela, ate ser regravado sem ela; o resto do servidor sobe \
                         normalmente.",
                        j.nome,
                        self.caminho.display()
                    )
                })
            })
            .collect()
    }

    /// Recupera da cauda do log a ultima corrida de cada job.
    ///
    /// A cauda basta: quem nao aparece nos ultimos 64 KB nao roda ha muito, e
    /// para ele a tela dizer "nunca rodou (que se saiba)" e mais honesto que
    /// carregar meses de log no arranque.
    fn semear_corridas(&mut self) {
        // O historico vem da mais nova para a mais velha, entao a primeira
        // ocorrencia de cada nome e a que fica. As aberturas (pedido 502) nao
        // sao resultado de corrida nenhuma: quem as le e o
        // `fechar_interrompidas`.
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
        self.exigir_legivel()?;
        self.jobs
            .iter()
            .find(|j| j.nome.eq_ignore_ascii_case(nome))
            .ok_or_else(|| PhxError::NaoEncontrado(format!("job {nome:?} nao existe")))
    }

    /// Grava ou substitui um job pelo nome.
    pub fn salvar(&mut self, j: Job) -> Result<()> {
        // Antes de mexer na lista: o `gravar` recusaria o disco, mas a memoria
        // ja teria o job, e um cadastro trancado passaria a ter um job que o
        // arquivo nao tem.
        self.exigir_legivel()?;
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
        // Antes do «nao existe»: trancado, a lista vazia e mentira.
        self.exigir_legivel()?;
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
        // A porta no UNICO escritor: o cadastro trancado nao regrava o arquivo
        // que nao abriu.
        self.exigir_legivel()?;
        let j = Json::objeto(vec![(
            "jobs",
            Json::Lista(self.jobs.iter().map(Job::para_disco).collect()),
        )]);
        if let Some(pai) = self.caminho.parent() {
            if !pai.as_os_str().is_empty() {
                phxsql_store::permissao::criar_diretorio_do_banco(pai)?;
            }
        }
        // 0600 desde o primeiro byte, e troca atomica: o mesmo molde da chave
        // do fio. O arquivo nao leva credencial (ver `Job::de_json`), mas leva
        // QUAL operacao roda como QUEM -- e nasce fechado como os irmaos.
        crate::config::gravar_privado(&self.caminho, j.escrever_identado().as_bytes())?;
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
    ///
    /// Era `with_extension("log")`, que TROCA a extensao -- e com `"jobs":
    /// "agenda.log"` no config o log virava o proprio cadastro, porque
    /// trocar ".log" por ".log" e' um no-op (pedido 482). Agora reusa o
    /// MESMO motor do temporario de `gravar_privado` (`irmao_do_log`, nome
    /// INTEIRO mais sufixo): o resultado e' sempre mais longo que
    /// `self.caminho`, entao nunca pode ser ele.
    pub fn caminho_do_log(&self) -> PathBuf {
        crate::config::irmao_do_log(&self.caminho)
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
        self.acrescentar_no_log(c);
    }

    /// Uma linha no fim do `.log`. Falhar aqui nao derruba ninguem, pelo
    /// motivo do `registrar`.
    fn acrescentar_no_log(&self, c: &Corrida) {
        let caminho = self.caminho_do_log();
        // Pelo motor da permissao do banco (pedido 542): 0700 e 0600.
        if let Some(pai) = caminho.parent() {
            if !pai.as_os_str().is_empty() {
                let _ = phxsql_store::permissao::criar_diretorio_do_banco(pai);
            }
        }
        if let Ok(mut f) = phxsql_store::permissao::opcoes_do_banco()
            .create(true)
            .append(true)
            .open(&caminho)
        {
            let _ = writeln!(f, "{}", c.para_json().escrever());
        }
    }

    /// As ultimas corridas, da mais nova para a mais velha -- so as que
    /// TERMINARAM: a linha de abertura (pedido 502) nao e resultado, e na tela
    /// ela apareceria como uma falha que nao houve.
    pub fn historico(&self, quantas: usize) -> Vec<Corrida> {
        let mut v: Vec<Corrida> = self
            .cauda_do_log()
            .into_iter()
            .filter(|c| !c.em_curso)
            .collect();
        v.truncate(quantas);
        v
    }

    /// Anota no `.log` que a corrida COMECOU -- pedido 502. E a lapide: se o
    /// processo cair antes da linha que fecha, o arranque seguinte a acha
    /// sozinha e sabe que esta corrida nao terminou.
    ///
    /// # Por que sem `fsync`
    ///
    /// O laco que ela quebra e o do `abort` (a H5 do pedido 451): processo
    /// morto, maquina de pe, e a pagina do arquivo continua no cache do
    /// nucleo -- o arranque seguinte a le. Numa queda de energia a lapide
    /// pode se perder, e ai o job roda de novo no arranque, que e o
    /// comportamento de antes, e nao um laco.
    pub fn registrar_inicio(&mut self, job: &str, op: &str, usuario: &str, quando_ms: i64) {
        self.acrescentar_no_log(&Corrida {
            quando_ms,
            job: job.to_string(),
            op: op.to_string(),
            usuario: usuario.to_string(),
            ok: false,
            duracao_ms: 0,
            detalhe: String::new(),
            em_curso: true,
        });
    }

    /// As corridas que COMECARAM e nunca terminaram -- o processo caiu no
    /// meio delas -- viram corrida que FALHOU, e nao rodam de novo neste
    /// arranque. Devolve as que fechou. Pedido 502.
    ///
    /// # A decisao, e de onde ela vem
    ///
    /// Os tres maduros convergem: o `pg_cron` marca como `failed`, com
    /// `server restarted`, toda corrida que estava `starting` ou `running`
    /// quando o banco reiniciou (`MarkPendingRunsAsFailed`, chamado no
    /// arranque do agendador), e nao a roda de novo; o MySQL e o MariaDB
    /// gravam o `LAST_EXECUTED` do evento ANTES de executa-lo
    /// (`Event_queue::get_top_for_execution_if_time`: `mark_last_executed` e
    /// `update_timing_fields_for_event`), entao o evento que derrubou o
    /// servidor conta como executado e so volta na proxima hora dele. Aqui a
    /// corrida que nunca terminou conta como a ultima, na hora em que
    /// comecou: o job volta na cadencia dele, e nao na do arranque.
    ///
    /// Quem chama e o arranque, e so ele: e escrita no `.log`, e o `abrir`
    /// continua so lendo.
    ///
    /// # A hora da lapide nunca passa de `agora_ms`
    ///
    /// A lapide traz o relogio de quem a escreveu. Se ele voltou depois da
    /// queda (RTC, snapshot de VM, passo do NTP), a hora dela fica no FUTURO, e
    /// contada como a ultima corrida sem teto empurrava o job para depois dela
    /// -- medido pelo DBA com +1 ano: um job «a cada 1 min» so voltava no ano
    /// seguinte, e a tela dizia `parado: false`. Antes da lapide isso nao
    /// acontecia, porque o `ultimos` zerava a cada arranque. O `min` guarda o
    /// que a lapide quer dizer («rodou agora ha pouco») sem herdar o relogio
    /// errado. O detalhe diz a hora escrita, que e o que se investiga.
    pub fn fechar_interrompidas(&mut self, agora_ms: i64) -> Vec<Corrida> {
        // Trancado, nao ha job nenhum para rodar -- e nada se escreve em nome
        // de um cadastro que nao abriu.
        if self.ilegivel.is_some() {
            return Vec::new();
        }
        let mut vistos: Vec<String> = Vec::new();
        let mut abertas: Vec<Corrida> = Vec::new();
        // Da mais nova para a mais velha: a PRIMEIRA linha de cada job e a
        // ultima coisa que aconteceu com ele.
        for c in self.cauda_do_log() {
            if vistos.iter().any(|n| n.eq_ignore_ascii_case(&c.job)) {
                continue;
            }
            vistos.push(c.job.clone());
            if c.em_curso {
                abertas.push(c);
            }
        }
        let mut fechadas = Vec::new();
        for a in abertas {
            let escrita = a.quando_ms;
            let c = Corrida {
                quando_ms: escrita.min(agora_ms),
                ok: false,
                em_curso: false,
                duracao_ms: 0,
                detalhe: format!(
                    "a corrida comecou em {}{} e nunca terminou: o servidor caiu no meio \
                     dela. Ela NAO roda de novo neste arranque; o job volta na proxima \
                     hora da agenda dele, contada desta corrida (pedido 502)",
                    phxsql_core::datahora::instante_iso(escrita),
                    if escrita > agora_ms {
                        " (no FUTURO: o relogio voltou depois da queda, e a conta vale \
                         a partir de agora)"
                    } else {
                        ""
                    }
                ),
                ..a
            };
            self.anotar_corrida(&c.job, c.quando_ms);
            self.registrar(&c);
            fechadas.push(c);
        }
        fechadas
    }

    /// Todas as linhas da cauda do `.log`, da mais nova para a mais velha,
    /// aberturas inclusive.
    fn cauda_do_log(&self) -> Vec<Corrida> {
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
        saida
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tmp(nome: &str) -> (DirTemp, PathBuf) {
        let d = DirTemp::novo(&format!("jobs-{nome}"));
        let arq = d.join("jobs.json");
        (d, arq)
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

    /// **A guarda travava um nome, nao a lei.** `token` era recusado;
    /// `senha`, `senha_hash`, `token_remoto` e `prova` iam para o `jobs.json`
    /// em claro e voltavam na ficha (revisao SEC de 17/09/2026, achado A2).
    /// Agora a regua e a lista da casa (`crate::segredos`), em qualquer
    /// profundidade -- e a recusa nomeia o campo.
    #[test]
    fn credencial_no_pedido_e_recusada_em_qualquer_profundidade() {
        for campo in ["senha", "senha_hash", "token_remoto", "prova", "nova_senha"] {
            let j = Json::analisar(&format!(
                "{{\"nome\":\"x\",\"pedido\":{{\"op\":\"usuario_alterar\",\"login\":\"a\",\"{campo}\":\"MARCA\"}}}}"
            ))
            .unwrap();
            let e = Job::de_json(&j).unwrap_err().to_string();
            assert!(
                e.contains(campo),
                "{campo}: a recusa nao nomeia o campo: {e}"
            );
            assert!(!e.contains("MARCA"), "{campo}: a recusa ecoa o valor: {e}");
        }
        // Dois niveis abaixo: o `lote` de um `usuario_criar` em massa.
        let j = Json::analisar(
            "{\"nome\":\"x\",\"pedido\":{\"op\":\"lote\",\"linhas\":[{\"nome\":\"ok\"},{\"senha\":\"MARCA\"}]}}",
        )
        .unwrap();
        assert!(
            Job::de_json(&j).is_err(),
            "a senha dois niveis abaixo passou"
        );
        // E a senha DENTRO da frase SQL, que nao tem campo chamado `senha`.
        let j = Json::analisar(
            "{\"nome\":\"x\",\"pedido\":{\"op\":\"sql\",\"texto\":\"CREATE USER c PASSWORD 'MARCA'\"}}",
        )
        .unwrap();
        let e = Job::de_json(&j).unwrap_err().to_string();
        assert!(e.contains("SQL"), "{e}");
        assert!(!e.contains("MARCA"), "{e}");
        // O que NAO e credencial continua entrando -- senao a guarda teria
        // fechado o job inteiro: o nome da variavel de ambiente e o caminho
        // certo para uma ligacao agendada.
        let j = Json::analisar(
            "{\"nome\":\"x\",\"pedido\":{\"op\":\"dblink_salvar\",\"nome\":\"erp\",\"motor\":\"phxsql\",\"host\":\"h\",\"token_remoto_env\":\"ERP_TOKEN\"}}",
        )
        .unwrap();
        Job::de_json(&j).unwrap();
        let j = Json::analisar(
            "{\"nome\":\"x\",\"pedido\":{\"op\":\"sql\",\"texto\":\"DROP USER c\"}}",
        )
        .unwrap();
        Job::de_json(&j).unwrap();
    }

    /// **O comportamento VELHO: o `jobs.json` de antes da guarda abre.**
    /// Pedido 497, R1 do parecer SEC da quarta volta: a guarda pelas letras
    /// `PASSWORD` rodava tambem na LEITURA do cadastro, e o job legitimo
    /// `SELECT login, password_hash FROM contas`, salvo antes dela, derrubava
    /// o arranque inteiro (rc=1). Agora ele abre, fica anotado, e o arquivo
    /// volta ao disco como o dono o escreveu -- ligado e com o pedido inteiro.
    /// A guarda continua valendo onde o job CHEGA (`de_json`, o `job_salvar`).
    #[test]
    fn o_jobs_json_de_antes_da_guarda_abre() {
        let (_guarda, caminho) = tmp("antes-da-guarda");
        std::fs::write(
            &caminho,
            "{\"jobs\":[{\"nome\":\"contas_hash\",\"ligado\":true,\"cada_minutos\":60,\
               \"pedido\":{\"op\":\"sql\",\"texto\":\"SELECT login, password_hash FROM contas\"}},\
             {\"nome\":\"troca\",\"pedido\":{\"op\":\"usuario_alterar\",\"login\":\"a\",\
               \"senha\":\"MARCA\"}}]}",
        )
        .unwrap();
        let mut r = match Registro::abrir(&caminho) {
            Ok(r) => r,
            Err(e) => panic!("o jobs.json de antes da guarda nao abriu: {e}"),
        };
        let hash = r.achar("contas_hash").unwrap().clone();
        assert!(
            hash.recusa.is_some(),
            "o job casa a guarda e nao ficou anotado"
        );
        assert!(hash.ligado, "abrir desligou o job do dono");
        // A mesma guarda, onde o job CHEGA: salvar continua recusando.
        let item = Json::analisar(
            "{\"nome\":\"contas_hash\",\"pedido\":{\"op\":\"sql\",\
              \"texto\":\"SELECT login, password_hash FROM contas\"}}",
        )
        .unwrap();
        assert!(
            Job::de_json(&item).is_err(),
            "o job_salvar passou a aceitar"
        );
        // A ficha nao devolve o valor que o arquivo guarda, e diz o campo.
        let ficha = r.achar("troca").unwrap().ficha().escrever();
        assert!(
            !ficha.contains("MARCA"),
            "a ficha devolve a senha do disco: {ficha}"
        );
        assert!(ficha.contains("\"recusado\""), "{ficha}");
        // Gravar OUTRO job reescreve o arquivo inteiro: o do dono volta como
        // estava, e nao desligado nem redigido por um caminho lateral.
        r.salvar(Job::de_json(&job_json("pulso", "")).unwrap())
            .unwrap();
        let texto = std::fs::read_to_string(&caminho).unwrap();
        assert!(texto.contains("password_hash FROM contas"), "{texto}");
        let relido = Registro::abrir(&caminho).unwrap();
        assert!(relido.achar("contas_hash").unwrap().ligado, "{texto}");
    }

    /// O `jobs.json` nasce 0600, e nasce assim desde o primeiro byte -- a
    /// mesma janela que `gravar_chave` fecha para a chave do fio, e que este
    /// arquivo deixava aberta escrevendo com a permissao do `umask` (revisao
    /// SEC de 17/09/2026, achado A4). A resposta e do sistema operacional,
    /// nao de uma flag: e o modo que o `stat` devolve.
    #[cfg(unix)]
    #[test]
    fn o_cadastro_de_jobs_nasce_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let (_guarda, caminho) = tmp("permissao");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(Job::de_json(&job_json("noturno", "")).unwrap())
            .unwrap();
        let modo = std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            modo, 0o600,
            "o jobs.json nasceu legivel por outros: {modo:o}"
        );
        assert!(
            !crate::config::temporario_de(&caminho).exists(),
            "o temporario ficou para tras"
        );
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
        assert!(
            e.contains("\"25:00\""),
            "a hora curta tem de continuar citada: {e}"
        );
    }

    /// **Pedido 453, o irmao da agenda: a hora torta grande nao volta
    /// inteira.** A recusa vai ao cliente e ao `acessos.log`, e quem escolhe o
    /// tamanho do campo e quem manda o pedido.
    #[test]
    fn hora_invalida_grande_nao_volta_inteira() {
        let mut j = Json::objeto(vec![]);
        j.definir("hora", Json::texto_de("9".repeat(1 << 20)));
        let e = Agenda::de_json(&j).unwrap_err().to_string();
        assert!(
            e.len() < 300,
            "o erro tem {} bytes: ecoa a hora recebida",
            e.len()
        );
    }

    #[test]
    fn cadastro_vai_e_volta_do_disco() {
        let (_guarda, caminho) = tmp("ida-e-volta");
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
        let (_guarda, caminho) = tmp("repetido");
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
        let (_guarda, caminho) = tmp("substitui");
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
        let (_guarda, caminho) = tmp("excluir");
        let mut r = Registro::abrir(&caminho).unwrap();
        assert!(r.excluir("fantasma").is_err());
    }

    #[test]
    fn so_o_ligado_e_vencido() {
        let (_guarda, caminho) = tmp("vencidos");
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
        let (_guarda, caminho) = tmp("historico");
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
                em_curso: false,
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
        let (_guarda, caminho) = tmp("semeada");
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
                    em_curso: false,
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
        let (_guarda, caminho) = tmp("excluir-corrida");
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
            em_curso: false,
        });
        r.excluir("x").unwrap();
        assert!(
            r.ultima_corrida_de("x").is_none(),
            "job recriado nao pode nascer vestindo o resultado do antigo"
        );
    }

    #[test]
    fn linha_quebrada_no_recorte_e_descartada() {
        let (_guarda, caminho) = tmp("quebrada");
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

    /// Pedido 482: `caminho_do_log` usava `with_extension("log")`, que TROCA
    /// a extensao. Com `"jobs": "agenda.log"` no config -- um nome que JA
    /// termina em `.log` --, trocar ".log" por ".log" e um no-op: o log
    /// virava o PROPRIO cadastro, e `registrar` escrevia a corrida em cima
    /// dos jobs (append numa "escrita" que na verdade era o cadastro
    /// inteiro). Prova real: cadastro num arquivo que TERMINA em `.log`,
    /// registra uma corrida, e confere que o cadastro continua intacto
    /// (bytes iguais, e ainda um cadastro que se rele) e que a corrida foi
    /// para o irmao.
    #[test]
    fn log_dos_jobs_nao_colide_com_cadastro_que_termina_em_log() {
        let dir = DirTemp::novo("colisao-log");
        // O nome que fundou o pedido: um "jobs" do config que ja termina em
        // ".log", como "agenda.log".
        let caminho = dir.join("agenda.log");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(Job::de_json(&job_json("noturno", "")).unwrap())
            .unwrap();
        let cadastro_antes = std::fs::read(&caminho).unwrap();
        assert!(
            !cadastro_antes.is_empty(),
            "o cadastro tinha de ter sido gravado antes de registrar a corrida"
        );

        r.registrar(&Corrida {
            quando_ms: 1,
            job: "noturno".into(),
            op: "ping".into(),
            usuario: "adm".into(),
            ok: true,
            duracao_ms: 1,
            detalhe: "ok".into(),
            em_curso: false,
        });

        let log = r.caminho_do_log();
        assert_ne!(
            log, caminho,
            "o log nao pode ser o proprio cadastro -- e exatamente o defeito do 482"
        );
        let cadastro_depois = std::fs::read(&caminho).unwrap();
        assert_eq!(
            cadastro_antes, cadastro_depois,
            "registrar a corrida nao pode mexer um byte do cadastro"
        );
        // E o cadastro continua um cadastro VALIDO -- reabre e acha o job,
        // em vez de ter virado uma linha de corrida por cima do JSON.
        let r2 = Registro::abrir(&caminho).unwrap();
        assert_eq!(r2.jobs.len(), 1, "o cadastro nao pode ter virado log");
        assert_eq!(r2.jobs[0].nome, "noturno");
        // E a corrida realmente foi para o irmao.
        let texto_do_log = std::fs::read_to_string(&log).unwrap();
        assert!(
            texto_do_log.contains("\"noturno\""),
            "a corrida nao apareceu no log: {texto_do_log}"
        );
    }

    /// **Pedido 502, no registro:** a abertura sem fechamento e a corrida que
    /// derrubou o processo. Ela vira FALHOU, conta como a ultima (o job nao
    /// vence na partida) e some da tela como abertura; o job que fechou a dele
    /// segue a regra de sempre, e roda na partida. E o segundo arranque nao
    /// fecha de novo o que ja fechou.
    #[test]
    fn corrida_aberta_sem_fecho_vira_falhou_e_nao_vence_na_partida() {
        let (_guarda, caminho) = tmp("interrompida");
        let mut r = Registro::abrir(&caminho).unwrap();
        for nome in ["noturno", "limpo"] {
            r.salvar(
                Job::de_json(&job_json(nome, ",\"ligado\":true,\"cada_minutos\":60")).unwrap(),
            )
            .unwrap();
        }
        r.registrar_inicio("noturno", "backup", "adm", 5_000);
        r.registrar_inicio("limpo", "backup", "adm", 6_000);
        r.registrar(&Corrida {
            quando_ms: 6_000,
            job: "limpo".into(),
            op: "backup".into(),
            usuario: "adm".into(),
            ok: true,
            duracao_ms: 3,
            detalhe: "ok".into(),
            em_curso: false,
        });
        drop(r);

        let mut r = Registro::abrir(&caminho).unwrap();
        assert!(
            r.historico(10).iter().all(|c| !c.em_curso),
            "a abertura apareceu no historico da tela como uma corrida"
        );
        let fechadas = r.fechar_interrompidas(5_000 + 1_000);
        assert_eq!(fechadas.len(), 1, "{fechadas:?}");
        assert_eq!(fechadas[0].job, "noturno");
        assert_eq!(r.ultimo_de("noturno"), 5_000);
        let agora = 5_000 + 59_000;
        let v = r.vencidos(agora);
        assert!(
            !v.contains(&"noturno".to_string()),
            "a corrida que derrubou o processo venceria de novo na partida: {v:?}"
        );
        assert!(
            v.contains(&"limpo".to_string()),
            "o job que terminou perdeu a regra de sempre (roda na partida): {v:?}"
        );
        let u = r.ultima_corrida_de("noturno").unwrap();
        assert!(!u.ok && u.detalhe.contains("nunca terminou"), "{u:?}");
        drop(r);

        let mut r = Registro::abrir(&caminho).unwrap();
        assert!(
            r.fechar_interrompidas(5_000 + 2_000).is_empty(),
            "o segundo arranque fechou de novo a mesma corrida"
        );
    }

    /// **C1 do parecer do DBA (lote 502):** a lapide com hora no FUTURO -- o
    /// relogio voltou depois da queda -- nao empurra o job para depois dela.
    ///
    /// Sem o `min`: com +1 ano, o job «a cada 1 min» nao vence nem um minuto
    /// depois do arranque, e a ultima corrida fica no ano seguinte.
    #[test]
    fn lapide_do_futuro_nao_empurra_o_job_para_depois_dela() {
        let (_guarda, caminho) = tmp("lapide-futuro");
        let agora: i64 = 1_790_000_000_000;
        let um_ano: i64 = 365 * 86_400_000;
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(Job::de_json(&job_json("eco", ",\"ligado\":true,\"cada_minutos\":1")).unwrap())
            .unwrap();
        r.registrar_inicio("eco", "ping", "adm", agora + um_ano);
        drop(r);

        let mut r = Registro::abrir(&caminho).unwrap();
        let fechadas = r.fechar_interrompidas(agora);
        assert_eq!(fechadas.len(), 1, "{fechadas:?}");
        assert!(
            r.ultimo_de("eco") <= agora,
            "a lapide do futuro virou a ultima corrida: {} contra agora {agora}",
            r.ultimo_de("eco")
        );
        let v = r.vencidos(agora + 60_000);
        assert!(
            v.contains(&"eco".to_string()),
            "o job de 1 min nao vence 1 min depois do arranque -- a lapide do futuro o \
             empurrou: {v:?}"
        );
        assert!(
            fechadas[0].detalhe.contains("FUTURO"),
            "o detalhe nao diz que a hora escrita estava no futuro: {}",
            fechadas[0].detalhe
        );
    }

    /// **O irmao do pedido 466, no registro:** o arquivo que existe e nao se
    /// le NAO vira cadastro vazio -- o `salvar` seguinte o regravaria so com o
    /// job novo. Trancado, recusa ler e gravar, e diz o arquivo.
    #[test]
    fn cadastro_de_jobs_que_nao_se_le_tranca_em_vez_de_abrir_vazio() {
        let (guarda, caminho) = tmp("trancado");
        std::fs::create_dir_all(&caminho).unwrap();
        assert!(
            Registro::abrir(&caminho).is_err(),
            "o diretorio abriu como cadastro"
        );
        let mut r = Registro::abrir_ou_trancar(&caminho);
        let e = r
            .salvar(Job::de_json(&job_json("novo", "")).unwrap())
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("TRANCADOS") && e.contains(&caminho.display().to_string()),
            "{e}"
        );
        assert!(
            r.jobs.is_empty(),
            "a memoria ganhou um job que o disco nao tem"
        );
        assert!(caminho.is_dir(), "o cadastro trancado foi regravado");
        drop(guarda);
    }
}
