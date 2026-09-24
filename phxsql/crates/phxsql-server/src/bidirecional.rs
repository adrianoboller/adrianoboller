//! A parte funda da replicacao bidirecional (multi-master), sem rede.
//!
//! Dois servidores, cada um replica do outro, os dois recebendo escrita. Os
//! dois problemas reais moram aqui, e cada um tem a sua peca:
//!
//! # O laco infinito, e a ORIGEM no evento
//!
//! A alteracao que A aplicou vinda de B nao pode voltar para B. Cada evento do
//! `.log` carrega a origem da escrita ([`hash_id`] do `id_servidor` de onde
//! ela nasceu; zero = local), e o `replicar` com o campo `para` NAO devolve os
//! eventos cuja origem e o proprio destino. A replica ainda descarta por conta
//! propria o que tiver a origem dela -- cinto e suspensorio, porque um source
//! que ignorasse o `para` reabriria o laco.
//!
//! # O conflito, e o carimbo
//!
//! O mesmo registro alterado dos dois lados antes de sincronizar: vence a
//! modificacao MAIS RECENTE, pelo carimbo que o `.log` ja tem por evento.
//! Isso exige uma honestidade dupla:
//!
//! - o evento aplicado guarda o carimbo do NASCIMENTO da escrita, nao o da
//!   chegada (ver `Table::forcar_proximo_evento`) -- senao venceria sempre
//!   quem sincronizou por ultimo;
//! - a regra so e justa com os RELOGIOS SINCRONIZADOS entre os servidores
//!   (NTP). Sem isso, o lado com o relogio adiantado vence sempre. Esta
//!   escrito em `docs/REPLICACAO.md` com todas as letras.
//!
//! Empate de carimbo desempata pela origem numerica MAIOR ([`remoto_vence`]):
//! arbitrario, deterministico, e igual nos dois lados -- que e o que importa
//! para os dois convergirem.
//!
//! # A identidade e a CHAVE, nunca o rowid
//!
//! A ordem de digitacao e sagrada EM CADA SERVIDOR: cada `.reg` mantem a SUA
//! ordem de chegada, e o insert local de A e o de B podem ganhar o mesmo
//! rowid. Entre servidores, a linha se identifica pela chave unica
//! ([`chave_unica`]) -- o mesmo desenho da sincronia do DbLink. Consequencia
//! honesta: **o modo bidirecional exige tabela com chave unica de uma
//! coluna**; sem ela a tabela e recusada com o motivo escrito (o HFSQL(R)
//! tambem impoe identificador adequado para replicar).

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use phxsql_core::error::Result;
use phxsql_core::json::Json;
use phxsql_core::schema::Schema;
use phxsql_store::log::Operacao;

/// A identidade numerica de um servidor, derivada do `id_servidor`.
///
/// # Por que um numero, e por que 16 bits
///
/// O evento do `.log` tem exatamente 2 bytes reservados para a origem, e o
/// texto nao caberia nem deveria: o cabecalho e de largura fixa. O CRC-32 do
/// texto, dobrado ao meio, da um numero estavel em qualquer maquina.
///
/// Zero fica reservado para "escrita local" -- e todo evento gravado antes de
/// a origem existir le zero, que e a leitura certa para eles.
///
/// # A colisao, dita com honestidade
///
/// Dois ids diferentes PODEM cair no mesmo numero (1 chance em 65.535 por
/// par). Colisao aqui suprimiria eventos de um terceiro servidor inocente.
/// Por isso a replica confere ao conectar: se o hash do id do source bater
/// com o do proprio id sendo os textos diferentes, a rodada para com erro --
/// troca-se um id e acabou. Ver `rodada_bidirecional`.
pub fn hash_id(id: &str) -> u16 {
    let c = phxsql_core::crc::crc32(id.trim().as_bytes());
    let dobrado = ((c >> 16) ^ (c & 0xFFFF)) as u16;
    if dobrado == 0 {
        1
    } else {
        dobrado
    }
}

/// O ultimo toque conhecido numa chave: quando, por quem, e se foi exclusao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toque {
    pub carimbo: i64,
    /// [`hash_id`] de quem escreveu. Para escrita local, o hash do PROPRIO
    /// servidor -- e nao zero --, para o empate desempatar igual nos dois
    /// lados.
    pub origem: u16,
    pub excluido: bool,
}

/// O evento remoto vence o ultimo toque local?
///
/// Mais recente vence; empate de carimbo vai para a origem numerica maior.
/// Igual dos dois lados por construcao: os dois comparam os mesmos numeros.
/// Empate total (mesmo carimbo, mesma origem) NAO vence -- e o que faz a
/// reaplicacao de um evento ja visto ser inofensiva.
pub fn remoto_vence(carimbo: i64, origem: u16, local: &Toque) -> bool {
    carimbo > local.carimbo || (carimbo == local.carimbo && origem > local.origem)
}

/// Quanto o carimbo de um evento remoto pode estar A FRENTE do relogio local
/// e ainda contar como «mais recente». Cinco minutos.
///
/// A regra «mais recente vence» confia no PAR, e nao so no NTP dele
/// (`docs/REPLICACAO.md` §12): um par que MENTE o carimbo -- nao um relogio
/// que deriva -- fixaria o toque local num instante que nenhuma escrita
/// daqui alcanca, e passaria a sobrescrever calado toda alteracao deste
/// lado, para sempre (revisao SEC de 17/09/2026, A9). Cinco minutos e a
/// tolerancia de deriva que o Kerberos usa ha decadas: acima de qualquer
/// NTP sadio, e a 2^40 de distancia de `i64::MAX`.
pub const FOLGA_DO_CARIMBO_MS: i64 = 5 * 60 * 1_000;

/// O carimbo esta alem do que o relogio local admite como futuro?
///
/// Quem chama troca o carimbo pelo `agora` local e CONTA a troca
/// (`MapaDeToques::carimbos_do_futuro`) em vez de recusar o evento: recusar
/// pararia o par de servidores por causa de um relogio errado do outro lado
/// (a licao do `Table::inserir_replicado`), e aceitar como veio e o estrago
/// descrito acima. Com o carimbo trocado, a proxima escrita local vence de
/// novo -- o envenenamento nao dura mais que um evento.
pub fn carimbo_alem_da_folga(carimbo: i64, agora: i64) -> bool {
    carimbo > agora.saturating_add(FOLGA_DO_CARIMBO_MS)
}

/// Um evento de INCLUSAO remoto colidindo com uma linha viva de OUTRA origem?
///
/// Esta e a assinatura do defeito (a) do pedido 229: dois masters na MESMA
/// faixa numeraram a mesma chave, e o casamento por chave com "mais recente
/// vence" apaga uma das linhas em silencio -- 4 insercoes viraram 2 linhas
/// (bloco 24 da sonda). O conserto pleno e faixa por no (`inicio`/`passo` no
/// esquema, mudanca de formato, fora desta frente); o minimo seguro e nao
/// deixar o estrago passar CALADO -- este predicado e o olho que o ve.
///
/// So conta como colisao quando as tres coisas valem ao mesmo tempo:
///
/// * a operacao remota e `Inclusao` -- o outro lado diz "criei uma linha NOVA",
///   e nao "alterei a que ja existe";
/// * ja existe um toque LOCAL para a chave, e ele nao e uma lapide (`excluido`)
///   -- ou seja, uma linha VIVA ocupa a chave aqui;
/// * esse toque local e de uma ORIGEM diferente da do evento.
///
/// A terceira condicao e o que separa a colisao real da reaplicacao inofensiva
/// do MESMO evento (posicao recomecada do zero, ou dois caminhos ate a mesma
/// replica): a reaplicacao vem da mesma origem, e nao apaga trabalho de
/// ninguem. Sem ela, todo evento reaplicado seria contado como perda.
pub fn colisao_de_criacao(operacao: Operacao, origem_ev: u16, local: Option<&Toque>) -> bool {
    operacao == Operacao::Inclusao
        && matches!(local, Some(t) if !t.excluido && t.origem != origem_ev)
}

/// A chave unica de UMA coluna que identifica a linha entre servidores.
///
/// Na ordem: a chave primaria, senao o primeiro indice unico de uma coluna.
/// `None` = a tabela nao tem identidade replicavel, e o modo bidirecional a
/// recusa com o motivo escrito. Chave COMPOSTA tambem fica de fora por
/// enquanto -- mesma regra da sincronia do DbLink, ate alguem precisar dela
/// com o pedido na mesa.
pub fn chave_unica(esquema: &Schema) -> Option<(String, usize)> {
    // O `e_coluna_de_sistema`, e nao dois literais crus: um indice unico
    // sobre uma coluna de sistema nova viraria a IDENTIDADE replicavel da
    // tabela, e o casamento entre servidores passaria a ser por um carimbo que
    // e local de cada no. Silencioso, e so aparecendo como linha duplicada.
    let serve = |i: &phxsql_core::schema::IndexDef| {
        i.unico
            && i.colunas.len() == 1
            && !phxsql_core::schema::e_coluna_de_sistema(
                &esquema.colunas()[i.colunas[0].coluna].nome,
            )
    };
    esquema
        .indices()
        .iter()
        .find(|i| i.primario && serve(i))
        .or_else(|| esquema.indices().iter().find(|i| serve(i)))
        .map(|i| (i.nome.clone(), i.colunas[0].coluna))
}

/// O que a replica bidirecional lembra de cada tabela, em memoria.
///
/// Reconstruido do proprio `.log` local: `vistos` diz ate onde a varredura
/// chegou, e o mapa guarda o ultimo toque por chave. Perder isto custa uma
/// varredura, nunca um dado -- mesma filosofia da marca do diario.
#[derive(Debug, Default)]
pub struct MapaDeToques {
    /// Eventos do diario local ja absorvidos no mapa.
    pub vistos: u64,
    /// Chave canonica -> ultimo toque.
    pub toques: HashMap<String, Toque>,
    /// Quantas colisoes de criacao (defeito (a)) esta tabela ja sofreu neste
    /// processo. So sobe; e o numero que `replicacao_estado` publica para o
    /// estrago deixar de ser calado. Ver [`colisao_de_criacao`].
    pub colisoes: u64,
    /// Quantos eventos chegaram com carimbo alem da folga do futuro e
    /// entraram com o relogio local no lugar. So sobe; publicado em
    /// `replicacao_estado` como `carimbos_do_futuro`. Ver
    /// [`carimbo_alem_da_folga`].
    pub carimbos_do_futuro: u64,
    /// Quantos eventos remotos esta tabela recusou por chave duplicada num
    /// indice unico. So sobe; publicado em `replicacao_estado` como
    /// `recusas_por_unicidade`.
    ///
    /// # O contador FICA, e o que mudou foi o que acontece depois dele
    ///
    /// O casamento entre servidores usa UMA chave ([`chave_unica`]), e a
    /// unicidade dos OUTROS indices continua sendo conferida na gravacao --
    /// e esta certo que continue, porque violacao de indice unico nao se cura
    /// quando o proximo lote chega, ao contrario da chave estrangeira. Com
    /// primaria `porId` e um secundario `porEmail`, o evento de A com um
    /// e-mail que ja existe em B e recusado, e a recusa subia pelo `?` do
    /// laco: a posicao consumida nunca andava e o MESMO lote voltava para
    /// sempre, CALADO. Nao e uma linha perdida -- e o par de servidores
    /// parado, sem ninguem saber.
    ///
    /// A parte (2) do pedido 292 contou e SEGUIU; a parte (1), depois de a
    /// regua dos motores maduros derrubar a recusa na declaracao, PARA o par
    /// naquela tabela ([`ParadaDaTabela`]) -- que e o que os tres motores
    /// maduros fazem, e o oposto do laco calado. O numero continua servindo ao
    /// mesmo para que serve o do [`colisao_de_criacao`]: dizer quantas vezes,
    /// e nao so que houve. Ele sobe **uma vez por parada**, porque a rodada
    /// seguinte nem chega ao evento.
    pub recusas_por_unicidade: u64,
}

/// Por que a replicacao de UMA tabela naquele par PAROU, e onde ela parou.
///
/// # Por que parar, se a parte (2) do pedido 292 fazia seguir
///
/// Porque a regua dos motores maduros derrubou a forma antiga e desenhou
/// esta: **nenhum dos tres recusa a tabela** por ter indice unico secundario,
/// e os tres fazem o conflito APARECER em vez de sumir -- o PostgreSQL para a
/// assinatura (`disable_on_error`) e grita com o indice, a chave e as duas
/// linhas; o Galera devolve `ER_LOCK_DEADLOCK` e conta; o Group Replication
/// tira o membro do grupo (`exit_state_action`). Seguir em frente contando e
/// melhor que o laco preso e calado de antes, e continua sendo divergencia
/// permanente que ninguem e obrigado a ver.
///
/// A posicao NAO anda quando o par para, e isso e a mesma decisao escrita no
/// fonte do PostgreSQL (`worker.c`, `replorigin_reset`): nao avancar a origem
/// e o que impede perder o evento. Quem manda o par adiante e um ser humano,
/// pela operacao `replicacao_pular`.
///
/// # Por que isto NAO e gravado em disco
///
/// Porque a parada e DERIVADA do dado: a rodada seguinte reencontra o mesmo
/// conflito e remarca sozinha. Gravar criaria uma segunda verdade, que mente
/// no dia em que o operador conserta a linha pela tela -- a tabela ficaria
/// parada por um conflito que nao existe mais. Mesma filosofia do
/// [`MapaDeToques`]: perder isto custa uma rodada, nunca um dado.
#[derive(Debug, Clone, Default)]
pub struct ParadaDaTabela {
    /// Por que parou, em CHAVE e nao em frase -- quem decide compara por
    /// chave, e frase se reescreve no dia em que alguem melhora a redacao.
    /// Hoje o unico valor e `"conflito_de_unicidade"`.
    pub motivo: String,
    /// A posicao, no diario da ORIGEM, do evento que parou o par. E a que
    /// `replicacao_pular` consome: ele anda para `posicao + 1`.
    pub posicao: u64,
    /// A chave de [`crate::servidor`] em `posicoes_bidi` -- `origem|db/tab`.
    /// Guardada em vez de remontada: remontar exigiria repetir aqui a regra
    /// de como o nome da tabela entra na chave, e duas receitas da mesma
    /// chave divergem no dia em que uma delas aprende um caso a mais.
    pub chave_da_posicao: String,
    /// O grito ja REDIGIDO: indice, valor da chave, a linha daqui e a de la.
    /// E o conteudo que o PostgreSQL carrega no `conflict=insert_exists`.
    pub detalhe: String,
    pub em_ms: i64,
}

impl ParadaDaTabela {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("motivo", Json::texto_de(&self.motivo)),
            ("posicao", Json::de_u64(self.posicao)),
            ("detalhe", Json::texto_de(&self.detalhe)),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.em_ms)),
            ),
        ])
    }
}

/// O conflito de unicidade ja ANALISADO: qual indice, que valor, e as duas
/// linhas redigidas.
///
/// Analisado, e nao recortado da mensagem de erro: `PhxError::Duplicado`
/// carrega o nome do indice dentro de uma frase, e quem recorta frase quebra
/// calado no dia em que alguem melhora a redacao. Aqui as chaves unicas sao
/// percorridas de novo contra o esquema, que e a mesma conta que a gravacao
/// fez -- so que agora para DIZER, e nao para recusar.
#[derive(Debug, Clone, Default)]
pub struct Conflito {
    /// O indice unico que recusou. Vazio quando a analise nao achou nenhum --
    /// e ai o grito diz isso, em vez de inventar um nome.
    pub indice: String,
    /// O valor da chave desse indice, no evento que chegou.
    pub valor: String,
    /// A linha que JA ocupa a chave aqui, redigida.
    pub linha_daqui: String,
    /// A linha que chegou do outro lado, redigida.
    pub linha_de_la: String,
}

/// O que [`crate::servidor`] fez com UM evento do bidirecional.
///
/// Trocou um `bool` porque `false` passou a significar duas coisas muito
/// diferentes: «o toque local venceu», que e o conflito funcionando, e «o
/// indice unico recusou», que PARA o par. Um `bool` obrigaria quem chama a
/// adivinhar pela diferenca, e quem adivinha erra no dia do caso novo.
#[derive(Debug)]
pub enum Aplicacao {
    /// O evento entrou.
    Aplicado,
    /// Nao entrou, e nao ha nada de errado: o toque local venceu, ou a
    /// exclusao nao achou o que excluir.
    Ignorado,
    /// Chave duplicada num indice unico: o par PARA nesta tabela.
    Conflito(Box<Conflito>),
}

impl Aplicacao {
    /// O evento entrou no `.reg` daqui?
    pub fn entrou(&self) -> bool {
        matches!(self, Aplicacao::Aplicado)
    }
}

/// Redige UMA linha para o grito do conflito, coluna a coluna.
///
/// # A petrea, e por que aqui ela e ANALISE e nao recorte
///
/// Dado pessoal e senha nunca vao a log. A linha chega aqui como valores JA
/// TIPADOS contra o esquema, entao nao ha texto cru que alguem precise
/// vasculhar: cada coluna se decide pelo que o esquema diz dela. Coluna
/// marcada como dado pessoal sai como o TAMANHO em bytes, e o mesmo vale para
/// `Bin`/`Memo`, que nao se leem sem carregar o bloco externo, e para o valor
/// que passa do teto -- «o que nao se analisa vira o tamanho em bytes».
///
/// Coluna de sistema fica de fora: ela e local dos dois lados e nunca explica
/// um conflito de chave.
pub fn linha_redigida(esquema: &Schema, valores: &[phxsql_core::value::Value]) -> String {
    /// Quantas colunas cabem antes do resumo. Uma tabela de quarenta colunas
    /// daria uma linha de log ilegivel; as primeiras bastam para reconhecer.
    const TETO_DE_COLUNAS: usize = 12;

    let mut partes: Vec<String> = Vec::new();
    let mut restantes = 0usize;
    for (i, c) in esquema.colunas().iter().enumerate() {
        if phxsql_core::schema::e_coluna_de_sistema(&c.nome) {
            continue;
        }
        let Some(v) = valores.get(i) else { continue };
        if partes.len() >= TETO_DE_COLUNAS {
            restantes += 1;
            continue;
        }
        partes.push(format!("{}={}", c.nome, valor_redigido(c, v)));
    }
    if restantes > 0 {
        partes.push(format!("... (+{restantes} coluna(s))"));
    }
    format!("({})", partes.join(", "))
}

/// UM valor, decidido pelo que o ESQUEMA diz da coluna dele.
///
/// A regra e a petrea: dado pessoal nunca sai, e o que nao sai vira o tamanho
/// em bytes. Vale para a linha inteira e vale para o VALOR DA CHAVE -- e e o
/// ponto em que divergimos do PostgreSQL de proposito: ele imprime a chave
/// (`Key (c)=(1)`) porque nao tem marca de dado pessoal no esquema; nos temos,
/// e uma chave primaria de CPF sairia no log do processo se copiassemos o
/// comportamento dele. Quem opera perde o valor e ganha o tamanho, o nome da
/// coluna e o indice: da para achar a linha sem publicar o dado.
pub fn valor_redigido(
    coluna: &phxsql_core::schema::Column,
    v: &phxsql_core::value::Value,
) -> String {
    /// Acima disto o valor vira o tamanho. Um grito nao e um dump: ele existe
    /// para quem opera reconhecer a linha, e uma coluna de 4 KiB nao ajuda
    /// ninguem a reconhecer nada -- so enche o diario do processo.
    ///
    /// O numero e o do MOTOR (pedido 453): a mensagem de erro de conversao
    /// faz a mesma pergunta -- quanto de um valor ajuda quem le --, e dois 48
    /// escritos em dois lugares divergiriam no dia em que alguem mexesse num.
    const TETO_DO_VALOR: usize = phxsql_core::error::TETO_DA_CITACAO;

    if v.e_null() {
        return "NULO".to_string();
    }
    if coluna.dado_pessoal.e_pessoal() {
        return format!("<dado pessoal, {} bytes>", tamanho_em_bytes(v));
    }
    // `Bin` e `Memo` moram fora do slot: o que a imagem traz e conteudo que
    // so se le carregando o bloco externo. O que nao se analisa vira bytes.
    if coluna.ty.externo() {
        return format!("<{} bytes>", tamanho_em_bytes(v));
    }
    let t = v.para_texto();
    if t.len() > TETO_DO_VALOR {
        format!("<{} bytes>", t.len())
    } else {
        t
    }
}

/// Quantos bytes o valor ocupa -- o que sobra quando o conteudo nao pode sair.
fn tamanho_em_bytes(v: &phxsql_core::value::Value) -> usize {
    use phxsql_core::value::Value;
    match v {
        Value::Bin(b) => b.len(),
        Value::Str(s) | Value::Memo(s) => s.len(),
        outro => outro.para_texto().len(),
    }
}

/// O que o laco de uma origem conta para a operacao `replicacao_estado`.
#[derive(Debug, Default, Clone)]
pub struct EstadoOrigem {
    /// "streaming", "cada_15min" ou "diaria_02:30".
    pub modo: String,
    pub ultima_rodada_ms: i64,
    /// Eventos aplicados desde o arranque, somando todas as tabelas.
    pub aplicados: u64,
    pub ultimo_erro: String,
    /// Tabelas recusadas e o motivo -- sem chave unica no modo multi, ou o
    /// diario do source que deixou de continuar o daqui (a tabela foi
    /// apagada e recriada la; a replica fiel para de segui-la e diz por que).
    pub recusas: BTreeMap<String, String>,
    /// "database/tabela" -> posicao consumida na origem.
    pub posicoes: BTreeMap<String, u64>,
    /// "database/tabela" -> por que a replicacao DAQUELA tabela neste par
    /// esta parada. Vazio no caso comum, e e o que faz o campo servir: mapa
    /// que aparece cheio em toda instalacao sa e mapa que ninguem le quando
    /// enche. Ver [`ParadaDaTabela`] e a operacao `replicacao_pular`.
    pub paradas: BTreeMap<String, ParadaDaTabela>,
    /// Proxima janela do agendamento, ms desde a epoca. Zero = streaming.
    pub proxima_janela_ms: i64,
    /// Por que o laco esta PARADO, quando esta. Vazio enquanto ele roda.
    ///
    /// Hoje o unico valor e `"credencial_recusada"`: a origem recusou o
    /// token, a credencial ou a propria conexao, e o laco nao volta sozinho
    /// -- so por `replicacao_ligar` ou por reinicio. Pedido 203: insistir
    /// numa credencial recusada bloqueava o IP no master em 4 s e derrubava
    /// o operador junto.
    pub parada: String,
    /// Quantas vezes `replicacao_ligar` religou este laco neste processo.
    pub religadas: u64,
    /// Pedido de religar ainda nao atendido. E o LACO quem o consome, e nao
    /// a operacao: um pedido feito com o laco a dormir vale assim que ele
    /// acorda, sem a operacao ter de saber em que passo ele esta.
    pub religar_pedido: bool,
    /// Falhas de REDE seguidas -- o expoente do recuo. Zero quando a ultima
    /// rodada deu certo.
    pub falhas_de_rede_seguidas: u32,
    /// Quando o laco vai tentar de novo depois de uma falha, ms desde a
    /// epoca. Zero quando ele nao esta esperando por falha nenhuma.
    pub proxima_tentativa_ms: i64,
}

impl EstadoOrigem {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("modo", Json::texto_de(&self.modo)),
            (
                "ultima_rodada",
                if self.ultima_rodada_ms == 0 {
                    Json::Nulo
                } else {
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.ultima_rodada_ms))
                },
            ),
            ("aplicados", Json::de_u64(self.aplicados)),
            (
                "ultimo_erro",
                if self.ultimo_erro.is_empty() {
                    Json::Nulo
                } else {
                    Json::texto_de(&self.ultimo_erro)
                },
            ),
            (
                "recusas",
                Json::Objeto(
                    self.recusas
                        .iter()
                        .map(|(k, v)| (k.clone(), Json::texto_de(v)))
                        .collect(),
                ),
            ),
            (
                "posicoes",
                Json::Objeto(
                    self.posicoes
                        .iter()
                        .map(|(k, v)| (k.clone(), Json::de_u64(*v)))
                        .collect(),
                ),
            ),
            (
                "paradas",
                Json::Objeto(
                    self.paradas
                        .iter()
                        .map(|(k, v)| (k.clone(), v.para_json()))
                        .collect(),
                ),
            ),
            (
                "proxima_janela",
                if self.proxima_janela_ms == 0 {
                    Json::Nulo
                } else {
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.proxima_janela_ms))
                },
            ),
            (
                "parada",
                if self.parada.is_empty() {
                    Json::Nulo
                } else {
                    Json::texto_de(&self.parada)
                },
            ),
            ("religadas", Json::de_u64(self.religadas)),
            (
                "falhas_de_rede_seguidas",
                Json::de_u64(self.falhas_de_rede_seguidas as u64),
            ),
            (
                "proxima_tentativa",
                if self.proxima_tentativa_ms == 0 {
                    Json::Nulo
                } else {
                    Json::texto_de(phxsql_core::datahora::instante_iso(
                        self.proxima_tentativa_ms,
                    ))
                },
            ),
        ])
    }
}

/// Quantos milissegundos dormir ate a proxima janela do agendamento.
///
/// `cada_minutos` e um intervalo simples a partir de agora; `hora` e a
/// proxima ocorrencia daquele minuto do dia, em UTC -- a MESMA convencao do
/// backup agendado, para as duas agendas do servidor nao discordarem de fuso.
/// Com os dois vazios devolve zero, que e o streaming.
pub fn ms_ate_a_janela(agora_ms: i64, cada_minutos: u64, hora: &str) -> i64 {
    if cada_minutos > 0 {
        return cada_minutos as i64 * 60_000;
    }
    let Some(alvo_min) = crate::config::Backup::minuto_do_dia(hora) else {
        return 0;
    };
    let no_dia = agora_ms.rem_euclid(86_400_000);
    let alvo = alvo_min as i64 * 60_000;
    if alvo > no_dia {
        alvo - no_dia
    } else {
        alvo - no_dia + 86_400_000
    }
}

/// As posicoes consumidas por origem, num arquivo ao lado dos dados.
///
/// # Por que um arquivo, e por que perder ele nao e grave
///
/// No modo A a posicao e o proprio diario da replica: cada evento aplicado
/// gera exatamente um evento local. No bidirecional isso quebra -- o diario
/// local mistura escrita local com aplicada, e eventos suprimidos pelo `para`
/// avancam a posicao sem gerar nada aqui. Entao a posicao consumida vira
/// estado proprio, gravado aqui.
///
/// Perder o arquivo recomeca do zero, e recomecar e INOFENSIVO: a aplicacao e
/// por chave com "mais recente vence", e reaplicar um evento ja visto perde
/// para o toque igual que ja esta no mapa. Custa releitura, nunca dado.
pub fn ler_posicoes(caminho: &Path) -> HashMap<String, u64> {
    let Ok(texto) = std::fs::read_to_string(caminho) else {
        return HashMap::new();
    };
    let Ok(j) = Json::analisar(&texto) else {
        return HashMap::new();
    };
    match j {
        Json::Objeto(pares) => pares
            .into_iter()
            .filter_map(|(k, v)| v.numero().map(|n| (k, n.max(0.0) as u64)))
            .collect(),
        _ => HashMap::new(),
    }
}

pub fn gravar_posicoes(caminho: &Path, posicoes: &HashMap<String, u64>) -> Result<()> {
    let mut pares: Vec<(String, Json)> = posicoes
        .iter()
        .map(|(k, v)| (k.clone(), Json::de_u64(*v)))
        .collect();
    pares.sort_by(|a, b| a.0.cmp(&b.0));
    std::fs::write(caminho, Json::Objeto(pares).escrever())?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    // ------------------------------------------------------------- o hash

    #[test]
    fn hash_e_estavel_nunca_zero_e_distingue_ids_normais() {
        assert_eq!(hash_id("curitiba-01"), hash_id("curitiba-01"));
        assert_eq!(hash_id(" curitiba-01 "), hash_id("curitiba-01"));
        assert_ne!(hash_id("curitiba-01"), hash_id("belgica-01"));
        assert_ne!(hash_id("a"), 0);
        assert_ne!(hash_id(""), 0, "zero e reservado para escrita local");
    }

    // --------------------------------------------------------- o conflito

    #[test]
    fn mais_recente_vence_nos_dois_sentidos() {
        let local = Toque {
            carimbo: 1_000,
            origem: 10,
            excluido: false,
        };
        assert!(remoto_vence(1_001, 5, &local), "remoto mais novo vence");
        assert!(!remoto_vence(999, 5, &local), "remoto mais velho perde");
    }

    /// O empate desempata pela origem MAIOR -- e o desenho garante que os dois
    /// servidores fazem a MESMA conta, entao exatamente um lado aplica e os
    /// dois convergem.
    #[test]
    fn empate_de_carimbo_desempata_pela_origem_e_e_simetrico() {
        let (a, b) = (hash_id("servidor-a"), hash_id("servidor-b"));
        assert_ne!(a, b, "o teste precisa de hashes distintos");

        // No servidor A: toque local com origem a; chega o evento de B.
        let em_a = Toque {
            carimbo: 500,
            origem: a,
            excluido: false,
        };
        // No servidor B: toque local com origem b; chega o evento de A.
        let em_b = Toque {
            carimbo: 500,
            origem: b,
            excluido: false,
        };
        let b_vence_em_a = remoto_vence(500, b, &em_a);
        let a_vence_em_b = remoto_vence(500, a, &em_b);
        assert_ne!(
            b_vence_em_a, a_vence_em_b,
            "exatamente UM lado aplica; os dois aplicando desfariam um ao outro"
        );
    }

    /// **Prova real do A9 (revisao SEC de 17/09/2026, pedido 286).** Um par
    /// que mente o carimbo (`i64::MAX`) fixava o toque local num instante que
    /// nenhuma escrita daqui alcanca: a partir dali TODA alteracao local
    /// perdia, calada, para sempre. Com o carimbo trocado pelo `agora` local
    /// quando passa da folga, a escrita local seguinte volta a vencer.
    ///
    /// Repondo o defeito (`carimbo_alem_da_folga` devolvendo sempre `false`),
    /// a ultima asercao cai: `i64::MAX` continua maior que `agora + 1`.
    #[test]
    fn carimbo_no_futuro_nao_ganha_para_sempre() {
        let agora = 1_800_000_000_000i64;
        let mentiroso = i64::MAX;
        assert!(carimbo_alem_da_folga(mentiroso, agora));
        // Dentro da folga, o carimbo vale como veio: e a deriva de relogio
        // que a regra sempre tolerou.
        assert!(!carimbo_alem_da_folga(agora + FOLGA_DO_CARIMBO_MS, agora));
        assert!(!carimbo_alem_da_folga(agora - 1, agora));
        assert!(carimbo_alem_da_folga(
            agora + FOLGA_DO_CARIMBO_MS + 1,
            agora
        ));

        // O que quem aplica guarda no toque e o carimbo SANEADO...
        let guardado = if carimbo_alem_da_folga(mentiroso, agora) {
            agora
        } else {
            mentiroso
        };
        let toque = Toque {
            carimbo: guardado,
            origem: 9,
            excluido: false,
        };
        // ...e por isso a escrita local de um milissegundo depois vence de
        // novo: o envenenamento nao dura mais que um evento.
        assert!(
            remoto_vence(agora + 1, 3, &toque),
            "a escrita local seguinte perdeu para um carimbo do futuro"
        );
    }

    /// Reaplicar o proprio evento (posicao recomecada do zero) nao vence: o
    /// toque igual ja esta no mapa, e igual nao e maior.
    #[test]
    fn reaplicacao_do_mesmo_evento_e_inofensiva() {
        let toque = Toque {
            carimbo: 500,
            origem: 7,
            excluido: false,
        };
        assert!(!remoto_vence(500, 7, &toque));
    }

    // -------------------------------------------------- a colisao, defeito (a)

    /// Defeito (a): duas insercoes de origens diferentes na MESMA chave sao uma
    /// colisao -- o casamento por chave apagaria uma das linhas em silencio.
    ///
    /// Reponha o defeito trocando `colisao_de_criacao` por `|_,_,_| false` (o
    /// olho fechado de antes): a primeira asserção deste teste cai, e o
    /// `replicacao_estado` volta a publicar zero colisoes enquanto perde linha.
    #[test]
    fn inclusao_de_outra_origem_sobre_chave_viva_e_colisao() {
        let (alfa, beta) = (hash_id("alfa"), hash_id("beta"));
        let local_alfa = Toque {
            carimbo: 500,
            origem: alfa,
            excluido: false,
        };
        // Chega a INCLUSAO de beta numa chave que alfa ja ocupa: colisao.
        assert!(colisao_de_criacao(
            Operacao::Inclusao,
            beta,
            Some(&local_alfa)
        ));
    }

    #[test]
    fn nao_ha_colisao_sem_esses_tres_sinais() {
        let (alfa, beta) = (hash_id("alfa"), hash_id("beta"));
        let vivo_alfa = Toque {
            carimbo: 500,
            origem: alfa,
            excluido: false,
        };
        // 1. Reaplicar a criacao da MESMA origem nao apaga nada de ninguem.
        assert!(!colisao_de_criacao(
            Operacao::Inclusao,
            alfa,
            Some(&vivo_alfa)
        ));
        // 2. Alteracao de outra origem e o conflito NORMAL (a mesma linha
        //    logica editada dos dois lados), nao uma perda de identidade.
        assert!(!colisao_de_criacao(
            Operacao::Alteracao,
            beta,
            Some(&vivo_alfa)
        ));
        // 3. Primeira vez que a chave aparece aqui: nao ha linha local a
        //    perder.
        assert!(!colisao_de_criacao(Operacao::Inclusao, beta, None));
        // 4. Toque local que ja e lapide (excluido): a chave esta livre, a
        //    criacao remota pode ocupa-la sem apagar linha viva.
        let lapide_alfa = Toque {
            carimbo: 500,
            origem: alfa,
            excluido: true,
        };
        assert!(!colisao_de_criacao(
            Operacao::Inclusao,
            beta,
            Some(&lapide_alfa)
        ));
    }

    // ------------------------------------------------------------ a chave

    fn esquema(indices: Vec<IndexDef>) -> Schema {
        Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cpf", ColumnType::Str(11)),
                Column::new("nome", ColumnType::Str(40)),
            ],
            indices,
        )
        .unwrap()
    }

    #[test]
    fn chave_unica_prefere_a_primaria() {
        let e = esquema(vec![
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico(),
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
        ]);
        let (indice, pos) = chave_unica(&e).unwrap();
        assert_eq!(indice, "porId");
        assert_eq!(pos, 0);
    }

    #[test]
    fn sem_primaria_serve_o_primeiro_unico_de_uma_coluna() {
        let e = esquema(vec![
            IndexDef::new("porNome", vec![IndexColumn::asc(2)]),
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico(),
        ]);
        let (indice, pos) = chave_unica(&e).unwrap();
        assert_eq!(indice, "porCpf");
        assert_eq!(pos, 1);
    }

    /// Sem chave unica nao ha identidade entre servidores: a tabela e recusada
    /// no modo bidirecional, com o motivo escrito -- e composta idem.
    #[test]
    fn sem_chave_unica_ou_com_composta_nao_ha_identidade() {
        let sem = esquema(vec![IndexDef::new("porNome", vec![IndexColumn::asc(2)])]);
        assert!(chave_unica(&sem).is_none());

        let composta = esquema(vec![IndexDef::new(
            "porIdCpf",
            vec![IndexColumn::asc(0), IndexColumn::asc(1)],
        )
        .unico()]);
        assert!(chave_unica(&composta).is_none());
    }

    // -------------------------------------------------------- o agendador

    #[test]
    fn cada_minutos_e_um_intervalo_simples() {
        assert_eq!(ms_ate_a_janela(123, 15, ""), 15 * 60_000);
        assert_eq!(ms_ate_a_janela(123, 1, "ignorada"), 60_000);
    }

    #[test]
    fn hora_marcada_e_a_proxima_ocorrencia_no_dia() {
        let meia_noite = 20_000i64 * 86_400_000;
        // 01:00 pedindo 02:30 -> 1h30 de espera.
        assert_eq!(
            ms_ate_a_janela(meia_noite + 3_600_000, 0, "02:30"),
            5_400_000
        );
        // 03:00 pedindo 02:30 -> amanha.
        assert_eq!(
            ms_ate_a_janela(meia_noite + 3 * 3_600_000, 0, "02:30"),
            86_400_000 - 1_800_000
        );
        // Sem agenda nenhuma: zero, que e streaming.
        assert_eq!(ms_ate_a_janela(meia_noite, 0, ""), 0);
    }

    // -------------------------------------------------------- as posicoes

    #[test]
    fn posicoes_atravessam_o_arquivo_e_arquivo_sumido_recomeca_do_zero() {
        let dir = DirTemp::novo("posicoes");
        let caminho = dir.join("replicacao-posicoes.json");

        assert!(ler_posicoes(&caminho).is_empty(), "sem arquivo, do zero");

        let mut p = HashMap::new();
        p.insert("b|loja/clientes".to_string(), 1234u64);
        p.insert("b|loja/pedidos".to_string(), 7u64);
        gravar_posicoes(&caminho, &p).unwrap();
        assert_eq!(ler_posicoes(&caminho), p);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------- a redacao do grito (292/1)

    fn esquema_do_grito() -> Schema {
        use phxsql_core::schema::Column;
        use phxsql_core::types::{ColumnType, DadoPessoal};
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8),
                Column::new("cpf", ColumnType::Str(14)).com_dado_pessoal(DadoPessoal::Pessoal),
                Column::new("obs", ColumnType::Memo),
                Column::new("cidade", ColumnType::Str(60)),
            ],
            vec![],
        )
        .unwrap()
    }

    /// **Prova real da petrea.** O grito do conflito carrega a linha, e a
    /// coluna marcada como dado pessoal sai como TAMANHO -- nunca o valor.
    ///
    /// **Defeito reposto**: tirar o ramo do `dado_pessoal` de
    /// `valor_redigido`; o CPF aparece inteiro e a primeira asercao cai.
    #[test]
    fn a_coluna_marcada_sai_como_tamanho_e_nunca_como_valor() {
        use phxsql_core::value::Value;
        let e = esquema_do_grito();
        let linha = vec![
            Value::Int(7),
            Value::Str("111.444.777-35".into()),
            // CURTO de proposito: um `Memo` grande cairia no teto do valor e
            // o teste passaria por engano, sem provar o ramo do externo.
            Value::Memo("segredo".into()),
            Value::Str("Blumenau".into()),
        ];
        let saida = linha_redigida(&e, &linha);
        assert!(
            !saida.contains("111.444.777-35"),
            "o dado pessoal vazou no grito: {saida}"
        );
        assert!(
            saida.contains("cpf=<dado pessoal, 14 bytes>"),
            "o tamanho nao saiu no lugar do valor: {saida}"
        );
        // O que nao se analisa vira o tamanho em bytes -- e `Memo` mora fora
        // do slot, entao ele nunca sai por extenso nem quando e curto.
        assert!(
            saida.contains("obs=<7 bytes>"),
            "o externo nao virou tamanho: {saida}"
        );
        // E o que NAO e nem marcado nem externo continua legivel: o grito
        // existe para quem opera reconhecer a linha.
        assert!(
            saida.contains("id=7") && saida.contains("cidade=Blumenau"),
            "{saida}"
        );
    }

    /// Valor grande demais numa coluna comum tambem vira tamanho: um grito
    /// nao e um dump do registro.
    #[test]
    fn valor_alem_do_teto_vira_tamanho_e_o_nulo_se_diz() {
        use phxsql_core::value::Value;
        let e = esquema_do_grito();
        let saida = linha_redigida(
            &e,
            &[
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Str("a".repeat(200)),
            ],
        );
        assert!(saida.contains("cidade=<200 bytes>"), "{saida}");
        assert!(saida.contains("id=NULO"), "{saida}");
        // Coluna marcada e NULA nao inventa tamanho de nada.
        assert!(saida.contains("cpf=NULO"), "{saida}");
    }
}
