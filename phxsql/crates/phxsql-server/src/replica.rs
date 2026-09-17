//! O lado da REPLICA: puxar os eventos do source e aplicar aqui.
//!
//! # A direcao da conexao
//!
//! Quem procura e a replica; o source nao empurra nada. E o mesmo desenho do
//! MySQL(R), e ele existe por causa do firewall: o source abre UMA porta de
//! entrada para o IP da replica, e nao precisa alcancar a replica de volta.
//!
//! ```text
//!    REPLICA 192.168.50.20  ──── TCP 5000 ────►  SOURCE 10.1.1.102
//!         (quem procura)                            (quem responde)
//! ```
//!
//! # O laco
//!
//! 1. `posicao` no source: quantos eventos cada tabela tem, e o esquema dela;
//! 2. compara com a posicao local -- o evento N e a posicao N, entao a replica
//!    guarda UM numero por tabela e nao ha GTID a inventar;
//! 3. `replicar` a partir dali, em lotes;
//! 4. aplica com `Table::aplicar_evento`, que confere o rowid;
//! 5. dorme e repete.
//!
//! # A senha nao viaja
//!
//! A replica se autentica pelo mesmo desafio-resposta do resto do protocolo:
//! pede um nonce, calcula o HMAC com a chave derivada e manda a PROVA. No
//! `config.json` da replica mora o `senha_hash` -- o mesmo texto que ja mora
//! no cadastro de usuarios --, e dele sai a chave derivada sem nunca haver
//! senha em claro em lugar nenhum.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::schema::Schema;
use phxsql_store::log::Operacao;

use phxsql_core::base64;
use phxsql_core::fio::{Canal, Iniciador, Recebido};

use crate::config::Origem;
use crate::valores::hex_para_bytes;

/// Quantos eventos puxar por vez.
///
/// Nao e so cortesia com a memoria: cada lote e uma resposta JSON inteira, com
/// a imagem de cada linha em hexadecimal. Um lote de dez mil linhas com anexo
/// seria uma resposta de dezenas de megabytes montada de uma vez dos dois
/// lados.
const LOTE: u64 = 500;

// O teto de um registro lido do fio NAO mora mais aqui: ele desceu para o
// `Canal` do `phxsql-core` (`TETO_DO_REGISTRO`), porque a leitura passou a
// ser dele. Um teto nesta camada voltaria a deixar o caminho cifrado sem
// nenhum -- que foi exatamente o risco desta integracao.

/// Uma conexao com o source, falando o JSON por linha da porta de dados.
pub struct Cliente {
    fluxo: TcpStream,
    leitor: BufReader<TcpStream>,
    token: String,
    /// Em claro (como sempre foi) ou dentro do tunel. Ver
    /// `docs/CIFRA-DO-FIO.md`.
    canal: Canal,
}

impl Cliente {
    /// Conecta com o prazo padrao de conexao, [`PRAZO_DE_CONEXAO`].
    ///
    /// Ate 17/09/2026 isto era um `TcpStream::connect` cru, que fica
    /// pendurado ate o sistema desistir do SYN (no Linux de fabrica, seis
    /// retransmissoes, perto de 127 s). O prazo tinha entrado so para o pulso
    /// do cluster, em [`Cliente::conectar_com_prazo`], e os IRMAOS ficaram sem
    /// ele: o laco da replica e a sonda `replicacao_testar` (via `ligar`), o
    /// dblink para outro PhxSql e o console de linha de comando -- todos
    /// chamam esta funcao. Revisao SEC, A5. Ela continua existindo em vez de
    /// ser apagada porque os tres chamadores nao tem prazo proprio a dizer; o
    /// que nao existe mais e um caminho SEM prazo.
    pub fn conectar(host: &str, porta: u16, token: &str, espera: Duration) -> Result<Cliente> {
        Cliente::conectar_com_prazo(host, porta, token, espera, PRAZO_DE_CONEXAO)
    }

    /// Conecta com PRAZO dito por quem chama. O pulso do cluster usa 1-2 s:
    /// um no morto nao pode segurar a conferencia dos vivos alem do proprio
    /// pulso.
    pub fn conectar_com_prazo(
        host: &str,
        porta: u16,
        token: &str,
        espera: Duration,
        prazo_conexao: Duration,
    ) -> Result<Cliente> {
        use std::net::ToSocketAddrs;
        let alvo = format!("{host}:{porta}");
        // O `ErrorKind` do sistema sobrevive ao embrulho: e por ele que a sonda
        // `replicacao_testar` classifica a falha (recusada, prazo, sem rota)
        // sem devolver o texto cru do sistema operacional -- revisao SEC de
        // 17/09/2026, A5. `Error::other` apagava o tipo e obrigava a
        // classificar pela frase, que se compara por chave e nunca por texto.
        let endereco = alvo
            .to_socket_addrs()
            .map_err(|e| PhxError::Io(std::io::Error::new(e.kind(), format!("{alvo}: {e}"))))?
            .next()
            .ok_or_else(|| {
                PhxError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{alvo}: sem endereco"),
                ))
            })?;
        let fluxo = TcpStream::connect_timeout(&endereco, prazo_conexao)
            .map_err(|e| PhxError::Io(std::io::Error::new(e.kind(), format!("{alvo}: {e}"))))?;
        Cliente::montar(fluxo, token, espera)
    }

    fn montar(fluxo: TcpStream, token: &str, espera: Duration) -> Result<Cliente> {
        // Nagle segura a resposta em ate 40 ms, e aqui toda troca e um pedido
        // pequeno esperando resposta -- exatamente o caso em que ele so atrasa.
        let _ = fluxo.set_nodelay(true);
        fluxo.set_read_timeout(Some(espera))?;
        fluxo.set_write_timeout(Some(espera))?;
        let leitor = BufReader::new(fluxo.try_clone()?);
        Ok(Cliente {
            fluxo,
            leitor,
            token: token.to_string(),
            canal: Canal::Claro,
        })
    }

    /// Pede o aperto de mao e passa a falar por dentro do tunel.
    ///
    /// `pino` e a chave publica que se ESPERA do source. Com pino, um source
    /// que apresente outra chave derruba a conexao -- e e assim que a replica
    /// se protege de quem esta no meio. Sem pino, o tunel protege da escuta
    /// PASSIVA e nada mais, porque o atacante apresenta a chave dele e nao ha
    /// com o que comparar.
    ///
    /// Devolve a chave que o source apresentou, para quem quiser anota-la.
    pub fn cifrar(&mut self, pino: Option<[u8; 32]>) -> Result<[u8; 32]> {
        let (iniciador, m1) = Iniciador::comecar(pino);
        let pedido = Json::objeto(vec![
            ("op", Json::texto_de("cifrar")),
            ("e", Json::texto_de(base64::codificar(&m1))),
        ])
        .escrever();
        self.fluxo.write_all(pedido.as_bytes())?;
        self.fluxo.write_all(b"\n")?;
        self.fluxo.flush()?;

        let mut resposta = String::new();
        if self.leitor.read_line(&mut resposta)? == 0 {
            return Err(PhxError::Io(std::io::Error::other(
                "o source fechou a conexao no aperto de mao",
            )));
        }
        let j = Json::analisar(&resposta)?;
        if !j.booleano_ou("ok", false) {
            return Err(PhxError::Autorizacao(format!(
                "o source recusou o aperto de mao: {}",
                j.texto_ou("erro", "sem motivo")
            )));
        }
        let m2 = base64::decodificar(
            j.campo("resultado")
                .map(|r| r.texto_ou("m2", ""))
                .unwrap_or(""),
        )?;
        let (transporte, apresentada) = iniciador.terminar(&m2)?;
        self.canal = Canal::Cifrado(Box::new(transporte));
        Ok(apresentada)
    }

    /// Manda um pedido e devolve o `resultado`, ou o erro que o source disse.
    pub fn pedir(&mut self, mut campos: Vec<(&str, Json)>) -> Result<Json> {
        if !self.token.is_empty() {
            campos.push(("token", Json::texto_de(self.token.clone())));
        }
        let linha = Json::objeto(campos).escrever();
        self.canal.escrever(&mut self.fluxo, &linha)?;

        let resposta = match self.canal.ler(&mut self.leitor)? {
            Recebido::Linha(l) => l,
            Recebido::Fim => {
                return Err(PhxError::Io(std::io::Error::other(
                    "o source fechou a conexao",
                )))
            }
        };
        let j = Json::analisar(&resposta)?;
        if !j.booleano_ou("ok", false) {
            // O erro do outro lado ja vem classificado -- `nome` e `classe`
            // fazem parte da resposta. Reembalar tudo como "acesso negado"
            // fazia o log da replica dizer autorizacao para um database que
            // ainda nao existe, que e o pior tipo de mensagem: a que manda
            // procurar no lugar errado.
            let texto = format!(
                "{}: {}",
                j.texto_ou("op", "?"),
                j.texto_ou("erro", "o source recusou sem dizer o motivo")
            );
            return Err(match j.texto_ou("nome", "") {
                "NAO_ENCONTRADO" => PhxError::NaoEncontrado(texto),
                "ACESSO_NEGADO" => PhxError::Autorizacao(texto),
                "DUPLICADO" => PhxError::Duplicado(texto),
                "CORROMPIDO" => PhxError::Corrompido(texto),
                "TIPO_INVALIDO" => PhxError::Tipo(texto),
                "LIMITE_EXCEDIDO" => PhxError::LimiteExcedido(texto),
                "CONFLITO" => PhxError::Conflito(texto),
                // Os dois nomes caem no MESMO erro: "escrita na replica" e
                // "redireciona" sempre foram o mesmo evento com nomes
                // diferentes, e um servidor de versao anterior ainda manda o
                // nome antigo.
                "REDIRECIONA" | "ESCRITA_NA_REPLICA" => PhxError::Redireciona(texto),
                "SPARE_EM_ESPERA" => PhxError::SpareEmEspera(texto),
                _ => PhxError::Esquema(texto),
            });
        }
        Ok(j.campo("resultado").cloned().unwrap_or(Json::Nulo))
    }

    /// Desafio-resposta. `senha_hash` e o preferido; `senha` e o caminho de
    /// quem ainda nao trocou o `config.json`.
    pub fn autenticar(&mut self, usuario: &str, senha_hash: &str, senha: &str) -> Result<()> {
        let d = self.pedir(vec![
            ("op", Json::texto_de("desafio")),
            ("usuario", Json::texto_de(usuario)),
        ])?;
        let nonce = d.texto_ou("nonce", "").to_string();
        let sal = d.texto_ou("sal", "").to_string();
        let iteracoes = d.inteiro_ou("iteracoes", 0).max(0) as u32;
        let nonce_cliente = phxsql_core::desafio::nonce();

        // Amarracao ao canal: quando a replica fala por dentro do tunel, a
        // prova nasce presa a transcricao do aperto, e o source a confere
        // contra a dele -- e o que derruba um homem-no-meio que tenha
        // terminado o tunel. Sem tunel (`None`), e a prova de sempre. Ver
        // `docs/CIFRA-DO-FIO.md` §10.
        let transcricao = self.canal.transcricao();
        let canal_ref = transcricao.as_ref().map(|t| &t[..]);

        let prova = if !senha_hash.is_empty() {
            // Do hash guardado sai a MESMA chave derivada que o source usa --
            // sem senha em claro em lado nenhum.
            let dk = phxsql_core::senha::derivado_do_hash(senha_hash)?;
            phxsql_core::desafio::calcular_prova(&dk, &nonce, &nonce_cliente, usuario, canal_ref)
        } else {
            phxsql_core::desafio::prova_de_senha(
                senha,
                &sal,
                iteracoes,
                &nonce,
                &nonce_cliente,
                usuario,
                canal_ref,
            )?
        };

        let mut campos = vec![
            ("op", Json::texto_de("login")),
            ("usuario", Json::texto_de(usuario)),
            ("prova", Json::texto_de(prova)),
            ("nonce_cliente", Json::texto_de(nonce_cliente)),
        ];
        if canal_ref.is_some() {
            campos.push(("amarrar_canal", Json::Bool(true)));
        }
        self.pedir(campos)?;
        Ok(())
    }

    /// Os databases do source, para a origem que nao lista nenhum.
    ///
    /// O `bancos` responde uma LISTA direta -- e este leitor procurava um
    /// campo `"bancos"` que nunca existiu. Consequencia: origem com
    /// `databases: []` (= todos) nao replicava NADA, em silencio, e ninguem
    /// viu porque a bancada de replicacao sempre fixou a lista. Foi o laco do
    /// cluster, que depende de descobrir os databases sozinho, que pisou aqui
    /// primeiro. Os dois formatos ficam aceitos, para um source antigo ou
    /// novo responderem igual.
    pub fn databases(&mut self) -> Result<Vec<String>> {
        let r = self.pedir(vec![("op", Json::texto_de("bancos"))])?;
        let lista = r
            .lista()
            .or_else(|| r.campo("bancos").and_then(Json::lista));
        Ok(lista
            .map(|l| {
                l.iter()
                    .map(|b| match b {
                        Json::Texto(t) => t.clone(),
                        outro => outro.texto_ou("nome", "").to_string(),
                    })
                    .filter(|n| !n.is_empty())
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// O que o source diz sobre uma tabela.
pub struct NoSource {
    pub nome: String,
    pub eventos: u64,
    pub esquema: Option<Schema>,
}

/// O que o source diz sobre um database inteiro.
pub struct PosicaoDoSource {
    pub com_imagem: bool,
    /// O `id_servidor` do source -- e com ele que o bidirecional confere a
    /// colisao de hash antes de confiar na supressao de origem.
    pub id_servidor: String,
    pub tabelas: Vec<NoSource>,
}

/// Le a resposta de `posicao`.
pub fn posicao(cliente: &mut Cliente, database: &str) -> Result<PosicaoDoSource> {
    let r = cliente.pedir(vec![
        ("op", Json::texto_de("posicao")),
        ("database", Json::texto_de(database)),
        ("com_esquema", Json::Bool(true)),
    ])?;
    let mut saida = Vec::new();
    if let Some(Json::Objeto(pares)) = r.campo("tabelas") {
        for (nome, v) in pares {
            let hex = v.texto_ou("esquema", "");
            let esquema = if hex.is_empty() {
                None
            } else {
                Some(Schema::desserializar(&hex_para_bytes(hex)?)?)
            };
            saida.push(NoSource {
                nome: nome.clone(),
                eventos: v.inteiro_ou("eventos", 0).max(0) as u64,
                esquema,
            });
        }
    }
    Ok(PosicaoDoSource {
        com_imagem: r.booleano_ou("imagem_da_linha", false),
        id_servidor: r.texto_ou("id_servidor", "").to_string(),
        tabelas: saida,
    })
}

/// Um evento vindo do source, pronto para aplicar.
pub struct EventoRecebido {
    pub operacao: Operacao,
    pub rowid: u64,
    /// Versao do registro depois da operacao -- um dos quatro campos que a
    /// conferencia de continuidade compara com o diario daqui.
    pub versao: u64,
    pub imagem: Vec<u8>,
    /// O instante em que a escrita NASCEU, no relogio de quem a fez.
    pub carimbo_ms: i64,
    /// [`crate::bidirecional::hash_id`] do servidor onde a escrita nasceu.
    pub origem: u16,
    /// A posicao deste evento no diario da ORIGEM -- o nosso equivalente do
    /// LSN que o `ALTER SUBSCRIPTION ... SKIP` do PostgreSQL recebe.
    ///
    /// # Por que ela vem do source, e nao se conta aqui
    ///
    /// Porque `desde + indice_na_lista` esta ERRADO no bidirecional: o source
    /// suprime os eventos cuja origem e quem pediu, e a posicao anda por cima
    /// deles (ver [`LoteRecebido`]). Contar aqui daria uma posicao menor que a
    /// verdadeira, e o `replicacao_pular` andaria para o lugar errado --
    /// pulando um evento inocente e deixando o culpado no caminho.
    ///
    /// `u64::MAX` = o source e velho demais para dizer onde o evento mora. A
    /// operacao de pular RECUSA nesse caso, nomeando: adivinhar a posicao e
    /// pular dado alheio em silencio.
    pub posicao: u64,
}

/// O valor de [`EventoRecebido::posicao`] quando o source nao a informa.
pub const POSICAO_DESCONHECIDA: u64 = u64::MAX;

/// Um lote do `replicar`, com a posicao ATE ONDE o source andou.
///
/// `ate` pode passar da conta dos eventos: no bidirecional o source suprime
/// os eventos cuja origem e quem pediu, e a posicao anda por cima deles.
pub struct LoteRecebido {
    pub eventos: Vec<EventoRecebido>,
    pub ate: u64,
    pub fim: bool,
}

/// Puxa ate `LOTE` eventos a partir de `desde`.
pub fn puxar(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
) -> Result<Vec<EventoRecebido>> {
    Ok(puxar_lote(cliente, database, tabela, desde, None)?.eventos)
}

/// O mesmo que [`puxar`], dizendo QUEM pede -- e devolvendo a posicao.
///
/// `para` e o `id_servidor` de quem puxa: o source nao devolve os eventos que
/// nasceram nele, que e o que mata o laco do bidirecional.
pub fn puxar_lote(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
    para: Option<&str>,
) -> Result<LoteRecebido> {
    puxar_ate(cliente, database, tabela, desde, para, LOTE)
}

/// UM evento do source -- o da posicao `qual` -- ou nenhum, quando o diario
/// de la nao chega ate ele.
///
/// E a conferencia de continuidade no caso em que a replica nao tem nada a
/// aplicar: o evento que ela ja tem, pedido de novo ao source, para saber se
/// o diario de la ainda e o daqui.
pub fn puxar_um(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    qual: u64,
) -> Result<Option<EventoRecebido>> {
    Ok(puxar_ate(cliente, database, tabela, qual, None, 1)?
        .eventos
        .into_iter()
        .next())
}

fn puxar_ate(
    cliente: &mut Cliente,
    database: &str,
    tabela: &str,
    desde: u64,
    para: Option<&str>,
    max: u64,
) -> Result<LoteRecebido> {
    let mut campos = vec![
        ("op", Json::texto_de("replicar")),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("desde", Json::de_u64(desde)),
        ("max", Json::de_u64(max)),
    ];
    if let Some(quem) = para {
        campos.push(("para", Json::texto_de(quem)));
    }
    let r = cliente.pedir(campos)?;
    let mut eventos = Vec::new();
    for e in r.campo("eventos").and_then(Json::lista).unwrap_or(&[]) {
        eventos.push(EventoRecebido {
            operacao: match e.texto_ou("operacao", "") {
                "inclusao" => Operacao::Inclusao,
                "alteracao" => Operacao::Alteracao,
                "exclusao" => Operacao::Exclusao,
                outro => {
                    return Err(PhxError::Corrompido(format!(
                        "o source mandou operacao desconhecida: {outro:?}"
                    )))
                }
            },
            rowid: e.inteiro_ou("rowid", 0).max(0) as u64,
            versao: e.inteiro_ou("versao", 0).max(0) as u64,
            imagem: hex_para_bytes(e.texto_ou("imagem", ""))?,
            carimbo_ms: e.inteiro_ou("carimbo_ms", 0),
            origem: e.inteiro_ou("origem", 0).clamp(0, u16::MAX as i64) as u16,
            posicao: match e.campo("posicao").and_then(Json::inteiro) {
                Some(n) if n >= 0 => n as u64,
                _ => POSICAO_DESCONHECIDA,
            },
        });
    }
    Ok(LoteRecebido {
        eventos,
        ate: r.inteiro_ou("ate", desde as i64).max(0) as u64,
        fim: r.booleano_ou("fim", true),
    })
}

/// Quanto tempo [`ligar`] espera o `connect` antes de desistir.
///
/// Dez segundos: acima de qualquer ida e volta sadia, inclusive fora da rede
/// local, e igual ao `reconectar_em` de fabrica -- esperar mais para conectar
/// do que se espera para tentar de novo nao faz sentido. O pulso do cluster
/// usa 1-2 s porque e rede local e porque um no morto nao pode segurar a
/// conferencia dos vivos; aqui o custo de esperar e so o atraso da replica.
pub const PRAZO_DE_CONEXAO: Duration = Duration::from_secs(10);

/// Abre a conexao e entra autenticado. Usado pelo laco e pelos testes.
pub fn ligar(origem: &Origem) -> Result<Cliente> {
    ligar_com_prazo(origem, PRAZO_DE_CONEXAO)
}

/// [`ligar`] com o prazo de conexao dito por quem chama.
///
/// Existe para a prova contra o sistema operacional: um host que engole o
/// SYN nao pode prender quem liga, e o teste mede isso com um prazo curto em
/// vez de esperar os dez segundos de [`PRAZO_DE_CONEXAO`].
pub fn ligar_com_prazo(origem: &Origem, prazo_conexao: Duration) -> Result<Cliente> {
    let mut c = Cliente::conectar_com_prazo(
        &origem.host,
        origem.porta,
        &origem.token,
        Duration::from_secs(30),
        prazo_conexao,
    )?;
    // O tunel ANTES do login, de proposito: e a prova do desafio-resposta e o
    // token que ele existe para esconder, e depois do login ja seria tarde.
    if origem.cifra {
        c.cifrar(origem.pino_do_fio()?)?;
    }
    if !origem.usuario.is_empty() {
        c.autenticar(&origem.usuario, &origem.senha_hash, &origem.senha)?;
    }
    Ok(c)
}

// ---------------------------------------------------------------------------
// O que o laco faz quando a rodada falha -- e por que sao TRES respostas.
//
// Pedido 203, filmado em 07/09/2026: uma replica com a credencial errada
// tentava entrar a cada `reconectar_em`, o master contava cada recusa como
// tentativa leve e bloqueava o IP em 4 s -- levando junto o operador que
// saia do mesmo 127.0.0.1. Medido pelo soquete
// (`bancada/replicacao/credencial-recusada.py`): 75 tentativas por minuto,
// bloqueio na quinta, por 60 minutos.
//
// O bloqueio do master esta certo e nao mudou. O que estava errado era tratar
// «a origem me recusou» como se fosse «a origem caiu»: uma e deterministica
// -- a mesma prova contra o mesmo hash da a mesma resposta amanha -- e a
// outra e transitoria. Insistir na primeira so gasta a tolerancia do bloqueio
// do outro lado; insistir na segunda e o proprio trabalho da replica.
// ---------------------------------------------------------------------------

/// Por que uma rodada falhou. Decide o que o laco faz em seguida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falha {
    /// A origem RECUSOU o token, a credencial ou a propria conexao (IP
    /// barrado). Nao e transitorio: tentar de novo com a mesma configuracao
    /// da o mesmo resultado, e cada tentativa conta contra o IP no master.
    CredencialRecusada,
    /// Nao deu para chegar na origem, ou a conexao caiu no meio. Passa
    /// sozinho: a origem volta, a rede volta.
    Rede,
    /// Qualquer outra coisa -- esquema, tabela recusada, dado corrompido.
    /// Continua no intervalo fixo de sempre, porque ninguem mediu que o
    /// recuo ajudaria aqui e guarda nova entra pedida.
    Outra,
}

impl Falha {
    /// A classificacao de um erro que veio de [`ligar`] -- a fase em que a
    /// origem diz sim ou nao para QUEM chega.
    ///
    /// So aqui `Autorizacao` vira credencial recusada. Uma `Autorizacao`
    /// depois de entrar (um `replicar` sem direito, por exemplo) e outra
    /// coisa: nao conta como tentativa leve no master, e um administrador de
    /// la pode corrigir sem que ninguem religue aqui.
    pub fn ao_ligar(e: &PhxError) -> Falha {
        match e {
            PhxError::Autorizacao(_) => Falha::CredencialRecusada,
            PhxError::Io(_) => Falha::Rede,
            _ => Falha::Outra,
        }
    }

    /// A classificacao de um erro DEPOIS de entrar: so a rede se distingue.
    pub fn na_rodada(e: &PhxError) -> Falha {
        match e {
            PhxError::Io(_) => Falha::Rede,
            _ => Falha::Outra,
        }
    }
}

/// [`ligar`] com a falha ja classificada, para quem chama decidir sem
/// adivinhar pelo texto do erro -- texto se compara por chave, nunca por
/// frase.
pub fn ligar_classificado(origem: &Origem) -> std::result::Result<Cliente, (Falha, PhxError)> {
    ligar(origem).map_err(|e| (Falha::ao_ligar(&e), e))
}

/// O que o laco faz depois de uma rodada que falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decisao {
    /// Espera tanto e tenta de novo.
    Dormir(Duration),
    /// Para de tentar ate alguem religar (`replicacao_ligar`) ou a
    /// configuracao mudar (reinicio).
    Estacionar,
}

/// O ritmo do laco: intervalo fixo entre rodadas em vao, recuo exponencial
/// para a REDE, estacionamento para a credencial.
///
/// O recuo da rede dobra a partir do `reconectar_em` e para em
/// [`Ritmo::TETO`] -- um minuto. O teto e baixo de proposito: uma conexao por
/// minuto a um host morto nao custa nada, e uma origem que volta depois de
/// uma noite fora e encontrada em ate um minuto, que e o que importa para o
/// atraso da replica. Um `reconectar_em` acima do teto vale como esta: o teto
/// nunca encurta o que o administrador pediu.
#[derive(Debug, Clone)]
pub struct Ritmo {
    base: Duration,
    /// Falhas de rede SEGUIDAS -- o expoente do recuo. Zera no sucesso.
    pub seguidas: u32,
}

impl Ritmo {
    pub const TETO: Duration = Duration::from_secs(60);

    pub fn novo(base: Duration) -> Ritmo {
        Ritmo { base, seguidas: 0 }
    }

    /// O intervalo entre rodadas que nao acharam nada -- o `reconectar_em`.
    pub fn base(&self) -> Duration {
        self.base
    }

    /// A rodada deu certo (achou algo ou nao): o recuo volta ao comeco.
    pub fn sucesso(&mut self) {
        self.seguidas = 0;
    }

    /// A rodada falhou assim: o que fazer.
    pub fn apos(&mut self, falha: Falha) -> Decisao {
        match falha {
            Falha::CredencialRecusada => {
                self.seguidas = 0;
                Decisao::Estacionar
            }
            Falha::Rede => {
                let espera = self.espera_de_rede();
                self.seguidas = self.seguidas.saturating_add(1);
                Decisao::Dormir(espera)
            }
            Falha::Outra => Decisao::Dormir(self.base),
        }
    }

    /// `base * 2^seguidas`, sem passar do teto e sem nunca ficar abaixo da
    /// base. O deslocamento e limitado antes de multiplicar, senao um laco
    /// que passou a noite recuando estouraria o `u32` ao chegar em 2^32.
    fn espera_de_rede(&self) -> Duration {
        let fator = 1u32 << self.seguidas.min(20);
        let crescida = self.base.saturating_mul(fator);
        crescida.min(Self::TETO).max(self.base)
    }
}

#[cfg(test)]
mod testes_do_ritmo {
    use super::*;

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// A tres respostas do `ao_ligar`: e ESTA classificacao que separa
    /// «a origem me recusou» de «a origem caiu».
    #[test]
    fn ao_ligar_separa_credencial_de_rede_e_do_resto() {
        let recusa = PhxError::Autorizacao("login: credencial invalida".into());
        let queda = PhxError::Io(std::io::Error::other("connection refused"));
        let esquema = PhxError::Esquema("imagem_da_linha desligada".into());
        assert_eq!(Falha::ao_ligar(&recusa), Falha::CredencialRecusada);
        assert_eq!(Falha::ao_ligar(&queda), Falha::Rede);
        assert_eq!(Falha::ao_ligar(&esquema), Falha::Outra);
        // Depois de entrar, `Autorizacao` NAO e credencial recusada: um
        // `replicar` sem direito se corrige no master, sem religar aqui.
        assert_eq!(Falha::na_rodada(&recusa), Falha::Outra);
        assert_eq!(Falha::na_rodada(&queda), Falha::Rede);
    }

    /// O defeito do pedido 203 reposto seria este teste caindo: a credencial
    /// recusada tem de ESTACIONAR na primeira, e nao dormir e voltar.
    #[test]
    fn credencial_recusada_estaciona_na_primeira() {
        let mut r = Ritmo::novo(s(1));
        assert_eq!(r.apos(Falha::CredencialRecusada), Decisao::Estacionar);
        // E de novo: religou, recusou de novo, estaciona de novo -- nunca
        // uma segunda tentativa por conta propria.
        assert_eq!(r.apos(Falha::CredencialRecusada), Decisao::Estacionar);
        assert_eq!(r.seguidas, 0);
    }

    /// A rede recua dobrando: 1, 2, 4, 8, 16, 32, e para no teto de 60 s.
    #[test]
    fn rede_recua_dobrando_ate_o_teto() {
        let mut r = Ritmo::novo(s(1));
        let esperas: Vec<Duration> = (0..8)
            .map(|_| match r.apos(Falha::Rede) {
                Decisao::Dormir(d) => d,
                Decisao::Estacionar => panic!("rede nunca estaciona"),
            })
            .collect();
        assert_eq!(
            esperas,
            vec![s(1), s(2), s(4), s(8), s(16), s(32), s(60), s(60)]
        );
        assert_eq!(r.seguidas, 8);
        // O sucesso zera: a proxima queda volta a esperar a base.
        r.sucesso();
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(1)));
    }

    /// Um `reconectar_em` acima do teto vale como esta: o teto nunca encurta
    /// o que o administrador pediu.
    #[test]
    fn o_teto_nunca_encurta_a_base() {
        let mut r = Ritmo::novo(s(300));
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(300)));
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(300)));
    }

    /// Uma noite inteira de recuo nao estoura o expoente.
    #[test]
    fn muitas_falhas_seguidas_nao_estouram() {
        let mut r = Ritmo::novo(s(10));
        for _ in 0..100_000 {
            let _ = r.apos(Falha::Rede);
        }
        assert_eq!(r.apos(Falha::Rede), Decisao::Dormir(s(60)));
    }

    /// As outras falhas ficam no intervalo fixo de sempre -- guarda nova
    /// entra pedida, e ninguem mediu que o recuo ajudaria aqui.
    #[test]
    fn outra_falha_mantem_o_intervalo_fixo() {
        let mut r = Ritmo::novo(s(10));
        assert_eq!(r.apos(Falha::Outra), Decisao::Dormir(s(10)));
        assert_eq!(r.apos(Falha::Outra), Decisao::Dormir(s(10)));
        assert_eq!(r.seguidas, 0);
    }
}

/// O prazo de conexao de [`ligar`], provado contra o sistema operacional.
///
/// Revisao SEC de 17/09/2026, A5: `ligar` caia num `connect` sem prazo, e o
/// irmao com prazo (`conectar_com_prazo`) so servia o pulso do cluster.
#[cfg(test)]
mod testes_do_prazo_de_conexao {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Instant;

    /// Um buraco negro LOCAL: um ouvinte que nunca aceita, com a fila de
    /// `accept` cheia.
    ///
    /// Com a fila cheia o nucleo DESCARTA o SYN em vez de recusar, e quem
    /// chega fica pendurado retransmitindo -- e o mesmo que um host que
    /// engole pacotes, sem depender de rota nenhuma. Medido antes de escrever
    /// (17/09/2026): neste ambiente `192.0.2.1` responde «Connection refused»
    /// na hora, entao NAO serve de buraco negro; a fila cheia no loopback
    /// pendurou um `connect` de 1 s ate o prazo. As conexoes que encheram a
    /// fila voltam junto, porque fechar qualquer uma abriria uma vaga.
    ///
    /// `None` quando o sistema nao encheu a fila em 4.096 conexoes: ai nao ha
    /// como provar, e o teste diz isso em vez de passar por engano.
    fn buraco_negro() -> Option<(TcpListener, Vec<TcpStream>, u16)> {
        let ouvinte = TcpListener::bind("127.0.0.1:0").ok()?;
        let endereco = ouvinte.local_addr().ok()?;
        let porta = endereco.port();
        let mut presos = Vec::new();
        for _ in 0..4_096 {
            match TcpStream::connect_timeout(&endereco, Duration::from_millis(200)) {
                Ok(c) => presos.push(c),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    return Some((ouvinte, presos, porta));
                }
                Err(_) => return None,
            }
        }
        None
    }

    fn origem_para(porta: u16) -> Origem {
        Origem {
            nome: "buraco-negro".into(),
            host: "127.0.0.1".into(),
            porta,
            token: String::new(),
            databases: Vec::new(),
            reconectar_em: 1,
            usuario: String::new(),
            senha_hash: String::new(),
            senha: String::new(),
            cada_minutos: 0,
            hora: String::new(),
            cifra: false,
            chave_do_fio: String::new(),
        }
    }

    /// **Prova real do A5.** Com o `connect` sem prazo reposto em `ligar`,
    /// este teste nao falha: PENDURA -- e por isso ele mede numa thread e
    /// reprova pelo `recv_timeout`, em vez de esperar o SO desistir do SYN.
    ///
    /// E a falha tem de sair classificada como REDE: e assim que o laco
    /// recua dobrando em vez de estacionar.
    #[test]
    fn ligar_nao_fica_pendurado_num_host_que_engole_o_syn() {
        let Some((_ouvinte, presos, porta)) = buraco_negro() else {
            eprintln!(
                "este sistema nao encheu a fila de accept em 4.096 conexoes: \
                 sem buraco negro nao ha como provar o prazo aqui"
            );
            return;
        };
        let prazo = Duration::from_millis(500);
        let (envio, volta) = mpsc::channel();
        std::thread::spawn(move || {
            let inicio = Instant::now();
            let r = ligar_com_prazo(&origem_para(porta), prazo);
            let _ = envio.send((
                r.map(|_| ()).map_err(|e| Falha::ao_ligar(&e)),
                inicio.elapsed(),
            ));
        });
        match volta.recv_timeout(Duration::from_secs(5)) {
            Ok((resultado, levou)) => {
                assert_eq!(
                    resultado,
                    Err(Falha::Rede),
                    "um host que engole o SYN e falha de REDE, e nada mais"
                );
                assert!(
                    levou < Duration::from_secs(3),
                    "`ligar` levou {levou:?} para desistir com prazo de {prazo:?}"
                );
            }
            Err(_) => panic!(
                "`ligar` ficou pendurado alem de 5 s num host que engole o SYN \
                 (prazo pedido: {prazo:?}): o prazo de conexao nao esta valendo"
            ),
        }
        // As conexoes presas so sao soltas AQUI, depois da medida.
        assert!(
            !presos.is_empty(),
            "a fila de accept encheu sem nenhuma conexao?"
        );
    }

    /// O padrao que o laco usa tem de ser o de dez segundos -- e a origem que
    /// existe (um ouvinte que aceita) continua entrando pelo mesmo caminho.
    #[test]
    fn o_prazo_padrao_e_de_dez_segundos_e_a_origem_viva_conecta() {
        assert_eq!(PRAZO_DE_CONEXAO, Duration::from_secs(10));
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        // Sem token e sem usuario, `ligar` so conecta -- e o que se quer medir.
        let c = ligar(&origem_para(porta));
        assert!(c.is_ok(), "{:?}", c.err());
        let (aceita, _) = ouvinte.accept().unwrap();
        drop(aceita);
    }
}
