//! Servidor TCP do PhxSql.
//!
//! Protocolo JSON Lines: uma linha JSON por pedido, uma linha JSON por
//! resposta, UTF-8, terminadas em `\n`. A conexao aceita varios pedidos
//! seguidos e cada um vira uma entrada no log de acessos.
//!
//! ```text
//! -> {"token":"...","op":"ping"}
//! <- {"ok":true,"op":"ping","resultado":{"phxsql":"0.1.0"},"ms":0}
//! ```
//!
//! # Concorrencia
//!
//! O motor de armazenamento ainda nao tem travas de arquivo nem de registro,
//! entao TODO acesso a dados passa por um mutex unico: as conexoes sao
//! aceitas em paralelo, mas as operacoes se enfileiram. E lento sob carga e e
//! correto -- o contrario seria rapido e corrompido. Travas finas entram junto
//! com as transacoes.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_store::catalogo::Instancia;
use phxsql_store::log::Operacao;
use phxsql_store::memoria::{Consulta, Filtro, Operador, Ordem, TabelaMemoria};
use phxsql_store::table::{Table, Visao};

use crate::acesso::{Acesso, LogAcessos};
use crate::blacklist::Blacklist;
use crate::config::{Config, Durabilidade};
use crate::dblink::{Definicao, Motor};
use crate::exportar::Formato;
use crate::http;
use crate::juncao::{Lado, Tipo as TipoJuncao, Uniao};
use crate::usuarios::{Atividade, Usuario};
use phxsql_core::schema::Schema;
use phxsql_core::types::DadoPessoal;
use phxsql_core::value::Value;

use crate::pivot::{Agregador, Campo, Granularidade, Juncao};
use crate::valores::{
    bytes_para_hex, hex_para_bytes, json_para_chave, json_para_linha, largura_do_tipo,
    linha_para_json,
};

pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Operacoes que alteram dados. Recusadas quando `somente_leitura` esta ligado.
pub(crate) const OPS_ESCRITA: &[&str] = &[
    "inserir",
    // As duas da sincronia: ligar cria tabela local, sincronizar grava nela.
    "dblink_ligar",
    "dblink_sincronizar",
    "atualizar",
    "excluir",
    "reindexar",
    "criar_database",
    "criar_schema",
    "criar_tabela",
    // Declarar e desdeclarar chave estrangeira regravam o bloco de esquema
    // no `.reg` -- catalogo, mas catalogo gravado em disco.
    "declarar_fk",
    "excluir_fk",
    "excluir_tabela",
    "duplicar_tabela",
    "copiar_tabela",
    "ajustar_sequencia",
    "inserir_lote",
    // Reservar a tabela para carga e declarar intencao de gravar. Num servidor
    // somente-leitura ninguem vai carregar nada, e deixar reservar seria
    // deixar travar a tabela para uma escrita que nunca acontece.
    "bulkinsert",
    // Marcar, desmarcar e esvaziar mexem em dado gravado. Listar a lixeira e
    // os motivos, nao -- essas duas so leem, e continuam valendo no modo
    // somente leitura, que e justamente quando alguem esta investigando.
    "restaurar",
    "esvaziar_lixeira",
    // Gravam o cadastro de ligacoes, que e arquivo deste servidor.
    "dblink_salvar",
    "dblink_excluir",
    // `aplicar` NAO entra aqui, e a ausencia e deliberada. Uma replica roda em
    // `somente_leitura` justamente para a aplicacao nao escrever nela -- e a
    // unica escrita que ela deve aceitar e a que vem do source. Barrar
    // `aplicar` aqui tornaria impossivel replicar para uma replica protegida,
    // que e a unica replica que se sustenta. Quem pode chamar `aplicar` ja
    // passou pelo portao do `administrar`.
    "encerrar_sessao",
    // Gravam o cadastro de jobs, que e arquivo deste servidor. `job_rodar` NAO
    // entra: ele confere o portao com a operacao DE DENTRO do job, entao um
    // job que grava ja e recusado por ela num servidor somente-leitura -- e um
    // job que so le continua rodando, que e o certo.
    "job_salvar",
    "job_excluir",
    "job_ligar",
    // Parar a porta de dados nao grava byte nenhum, mas interrompe o trabalho
    // de todo mundo -- pela mesma razao que `encerrar_sessao` esta aqui. Um
    // servidor declarado somente-leitura nao e um servidor sem dono.
    "servico_parar",
    "servico_subir",
];

/// Estado de uma conexao.
///
/// A senha e conferida com PBKDF2, que custa da ordem de 100 ms de proposito.
/// Fazer isso a cada pedido inviabilizaria o servidor, entao a autenticacao
/// acontece UMA VEZ por conexao e o resultado fica aqui.
#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
    /// A conexao desta sessao, do registro de ligacoes. Zero quando o pedido
    /// veio pela porta web, que nao tem conexao para amarrar nada.
    ///
    /// E a ela que a reserva de carga morre amarrada: sem um id de CONEXAO, a
    /// reserva so poderia ser identificada pelo login -- e aí duas janelas do
    /// mesmo usuario seriam o mesmo dono, o que e exatamente o contrario de
    /// exclusivo.
    ligacao: u64,
    /// Desafio em aberto: (usuario, nonce do servidor, quando expira).
    /// Vale uma vez so -- e consumido no login, dando certo ou errado.
    desafio: Option<(String, String, i64)>,
}

impl Sessao {
    fn login(&self) -> &str {
        self.usuario
            .as_ref()
            .map(|u| u.login.as_str())
            .unwrap_or("")
    }

    /// Id gravado no `.log` da tabela como autor da operacao.
    /// Zero quando a conexao veio pelo token de servico, sem login.
    fn id(&self) -> u32 {
        self.usuario.as_ref().map(|u| u.id).unwrap_or(0)
    }
}

/// Uma conexao viva para outro PhxSql, do lado de ca da interface.
pub struct Remoto {
    pub destino: String,
    leitor: BufReader<TcpStream>,
    escrita: TcpStream,
}

impl Remoto {
    /// Abre a conexao. Nao autentica -- quem autentica e o pedido de login,
    /// que segue por aqui igual a qualquer outro.
    pub fn abrir(destino: &str, timeout_s: u64) -> Result<Remoto> {
        use std::net::ToSocketAddrs;
        let endereco = destino
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("destino {destino:?} nao resolve: {e}")))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("destino {destino:?} sem endereco")))?;
        let fluxo =
            TcpStream::connect_timeout(&endereco, Duration::from_secs(timeout_s.min(10)))
                .map_err(|e| PhxError::Esquema(format!("nao consegui falar com {destino}: {e}")))?;
        fluxo.set_read_timeout(Some(Duration::from_secs(timeout_s)))?;
        let escrita = fluxo.try_clone()?;
        Ok(Remoto {
            destino: destino.to_string(),
            leitor: BufReader::new(fluxo),
            escrita,
        })
    }

    /// Manda uma linha e devolve a resposta, crua.
    ///
    /// Crua de proposito: o que o servidor remoto respondeu e o que o
    /// navegador recebe. Reescrever no meio do caminho seria mentir sobre
    /// quem respondeu o que.
    pub fn conversar(&mut self, linha: &str) -> Result<Json> {
        let limpa = linha.replace(['\n', '\r'], " ");
        writeln!(self.escrita, "{limpa}")?;
        self.escrita.flush()?;
        let mut resposta = String::new();
        if self.leitor.read_line(&mut resposta)? == 0 {
            return Err(PhxError::Esquema(format!(
                "{} fechou a conexao",
                self.destino
            )));
        }
        Json::analisar(&resposta)
    }
}

/// Decide QUANDO uma gravacao vai de fato para o disco.
///
/// O `write` acontece sempre, na hora: os bytes vao para o sistema operacional
/// em toda gravacao, sem buffer nosso, entao outro processo ve o dado
/// imediatamente. O que este contador decide e o `fsync`, que e o que protege
/// de o computador perder energia antes de o sistema descarregar a pagina.
///
/// Medido: sincronizar a cada linha da 1.289 linhas/s; a cada 200, 20.000/s.
/// Eram **95% do tempo da insercao**.
struct Janela {
    modo: Durabilidade,
    a_cada: u64,
    ms: u64,
    /// Gravacoes desde o ultimo `fsync`.
    pendentes: AtomicU64,
    /// Quando a janela corrente abriu.
    desde: Mutex<Instant>,
}

impl Janela {
    fn nova(r: &crate::config::Recursos) -> Janela {
        Janela {
            modo: r.durabilidade,
            a_cada: r.lote_operacoes,
            ms: r.lote_milissegundos,
            pendentes: AtomicU64::new(0),
            desde: Mutex::new(Instant::now()),
        }
    }

    /// Conta mais uma gravacao e diz se e hora de sincronizar.
    ///
    /// Fecha a janela por QUANTIDADE ou por TEMPO, o que vier primeiro. So por
    /// quantidade, um servidor com pouco movimento deixaria a ultima gravacao
    /// pendurada indefinidamente; so por tempo, uma carga em massa encheria a
    /// memoria entre um relogio e outro.
    fn hora_de_gravar(&self) -> bool {
        match self.modo {
            Durabilidade::PorOperacao => true,
            Durabilidade::Sistema => false,
            Durabilidade::PorLote => {
                let n = self.pendentes.fetch_add(1, Ordering::SeqCst) + 1;
                if n >= self.a_cada {
                    self.fechar();
                    return true;
                }
                let mut desde = match self.desde.lock() {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
                if desde.elapsed().as_millis() as u64 >= self.ms {
                    self.pendentes.store(0, Ordering::SeqCst);
                    *desde = Instant::now();
                    return true;
                }
                false
            }
        }
    }

    fn fechar(&self) {
        self.pendentes.store(0, Ordering::SeqCst);
        if let Ok(mut d) = self.desde.lock() {
            *d = Instant::now();
        }
    }

    /// Ha gravacao esperando o `fsync`?
    fn pendente(&self) -> u64 {
        self.pendentes.load(Ordering::SeqCst)
    }
}

/// Quantas dicas de posicao do diario guardar por tabela.
///
/// Uma por replica que puxa dela, mais folga. Oito cobre a topologia que a
/// bancada monta (tres) com sobra, e o custo de cada uma e 20 bytes.
const MARCAS_POR_TABELA: usize = 8;

pub struct Servidor {
    config: Config,
    /// Trava unica de dados. Ver a nota de concorrencia no topo do modulo.
    dados: Mutex<Instancia>,
    /// Quando o gravado vai de fato para o disco.
    janela: Janela,
    /// Tabelas escritas desde o ultimo `fsync`, como "database/tabela".
    ///
    /// Existe porque a tabela e aberta e fechada a cada operacao: quando a
    /// janela fecha, quem esta aberto e so a tabela da operacao corrente, e as
    /// outras tocadas na janela ficariam sem sincronizar. Este conjunto e a
    /// lista do que ainda deve ao disco.
    sujas: Mutex<std::collections::HashSet<String>>,
    log: Mutex<LogAcessos>,
    lista_negra: Mutex<Blacklist>,
    /// Sessoes do navegador. Vazio enquanto a interface web estiver desligada.
    sessoes: Mutex<http::Sessoes>,
    /// Tabelas residentes em RAM, por "database/tabela". Nada entra aqui
    /// sozinho: so o que alguem pediu para carregar.
    residentes: Mutex<HashMap<String, TabelaMemoria>>,
    /// Conexoes abertas para outros PhxSql, uma por sessao do navegador.
    ///
    /// Ficam abertas de proposito: o protocolo da porta 5000 autentica uma vez
    /// por CONEXAO, entao manter o soquete e o que faz o PBKDF2 do servidor
    /// remoto rodar uma vez por login e nao a cada clique.
    remotos: Mutex<HashMap<String, Arc<Mutex<Remoto>>>>,
    /// As conexoes vivas, para o operador ver quem esta falando e poder
    /// derrubar quem travou.
    ligacoes: Mutex<crate::ligacoes::Ligacoes>,
    /// Quando o servidor subiu, para o `ping` poder dizer ha quanto tempo.
    ///
    /// Um servidor que reiniciou sozinho de madrugada parece igual a um que
    /// nunca caiu -- ate alguem olhar o tempo no ar e ver duas horas.
    desde_ms: i64,
    /// Amostra anterior da maquina, para as taxas do painel.
    ///
    /// Guardar aqui, e nao na tela, e o que permite dizer "CPU em 40%": o
    /// `/proc` so traz contadores desde o arranque, e taxa exige duas
    /// amostras. Uma unica trava para todos os navegadores tambem evita cada
    /// aba abrir a propria serie e nenhuma delas fechar conta.
    monitor: Mutex<crate::sistema::Monitor>,
    /// Ultimo aviso mandado por caminho, para nao repetir enquanto o disco
    /// continua cheio.
    avisados: Mutex<HashMap<String, i64>>,
    /// O que esta chegando pela porta, quando alguem liga para olhar.
    /// Tabelas reservadas para carga (`BULKINSERT`).
    cargas: Mutex<crate::carga::Cargas>,
    /// Onde a ultima leitura do diario de cada tabela parou, por
    /// `database/tabela`.
    ///
    /// # Por que aqui, e nao na tabela
    ///
    /// A tabela e aberta e fechada a cada pedido, entao a marca morreria entre
    /// um `replicar` e o seguinte -- que sao exatamente os dois pedidos em que
    /// ela vale. Sem ela, servir «500 eventos a partir de P» caminha pelos P
    /// anteriores lendo o cabecalho de cada um, e alcancar N eventos custa
    /// N^2/2 leituras.
    ///
    /// Medido em `--example custo-do-desde`, num diario de 100.000 eventos:
    /// ler 500 a partir de 0 custa 1,11 us por evento; a partir de 90.000,
    /// 72,65. Alcancar os 100.000 de 500 em 500 gastava 4,07 s so aqui.
    ///
    /// E so uma DICA: perde-la custa uma varredura, e uma errada faz o CRC do
    /// evento recusar. Por isso ela nao vai a disco.
    ///
    /// # Por que uma LISTA, e nao uma marca por tabela
    ///
    /// Um source serve varias replicas, e elas nao estao na mesma posicao --
    /// uma que ficou fora do ar volta atras das outras. Com uma marca so, a
    /// que estivesse mais adiantada a moveria para frente e as outras nunca a
    /// aproveitariam: a marca so serve para uma posicao DEPOIS dela. Guardar
    /// algumas e escolher a maior que ainda cabe atende todas.
    marcas_do_diario: Mutex<HashMap<String, Vec<phxsql_store::log::MarcaDoDiario>>>,
    profiler: Mutex<crate::profiler::Profiler>,
    /// Espelho de `profiler.ligado`, para o caminho quente nao tomar a trava.
    ///
    /// # Por que existe
    ///
    /// Observacao que nao esta ligada nao pode custar nada. Sem este espelho,
    /// TODO pedido pagava, antes de a conferencia acontecer: dois
    /// `Json::analisar` do corpo inteiro -- um para achar database/tabela,
    /// outro para o nome da operacao --, tres `String` alocadas, e um mutex.
    /// Num `inserir_lote` de cinco mil linhas isso e analisar meio megabyte de
    /// JSON duas vezes, para no fim `chegou` olhar `ligado` e devolver `None`.
    /// Medido: 7% da carga pela rede.
    ///
    /// Um `AtomicBool` lido com `Relaxed` custa uma instrucao e nao serializa
    /// ninguem. A trava so e tomada quando ha o que registrar.
    ///
    /// A janela de divergencia e de um pedido: quem liga o profiler pode nao
    /// ver o pedido que ja estava em voo. Ligar a observacao no meio de um
    /// pedido nao promete pegar aquele pedido -- promete pegar os proximos.
    profiler_ligado: AtomicBool,
    /// Ligacoes para bancos de fora.
    dblink: Mutex<crate::dblink::Registro>,
    /// Jobs de execucao: cadastro e a hora da ultima corrida de cada um.
    jobs: Mutex<crate::jobs::Registro>,
    /// O relogio dos jobs subiu neste processo?
    relogio_de_jobs: AtomicBool,
    /// Os jobs em execucao NESTE instante -- para a tela dizer "rodando" e o
    /// vigia nao confundir corrida longa com job parado.
    ///
    /// Trava propria, e nunca tomada com a de `jobs` na mao: quem precisa das
    /// duas tira a foto daqui ANTES de trancar o cadastro.
    jobs_rodando: Mutex<Vec<String>>,
    /// Quando cada aviso de job saiu por e-mail, por chave `falha:nome` /
    /// `parado:nome` -- o silencio entre avisos repetidos, como o do disco.
    avisos_de_jobs: Mutex<HashMap<String, i64>>,
    /// A porta de dados esta aceitando conexao agora?
    ///
    /// Parada, o processo continua vivo e a interface web continua no ar --
    /// e e por ela que a porta volta. Um botao que derrubasse o PROCESSO nao
    /// teria como se desfazer: nao sobraria ninguem para atender o "subir".
    porta_no_ar: AtomicBool,
    /// Sinalizador que o laco de aceitacao le depois de cada `accept`.
    parar_de_aceitar: AtomicBool,
    /// O ouvinte ja preso no endereco novo, esperando o laco soltar o velho.
    ///
    /// A ordem importa e e a garantia contra o tiro no pe: o endereco novo e
    /// PRESO antes de o antigo ser solto. Porta ocupada ou endereco invalido
    /// falham enquanto o servico continua no ar, e nada muda.
    proximo_ouvinte: Mutex<Option<TcpListener>>,
    /// Onde a porta de dados escuta agora, que nem sempre e o `bind`.
    endereco_dos_dados: Mutex<Option<SocketAddr>>,
    conexoes: AtomicUsize,
    /// O estado vivo do cluster -- `None` quando o `config.json` nao traz o
    /// bloco `cluster`, e ai NADA disto existe: nenhuma thread, nenhum portao.
    cluster: Option<Arc<crate::cluster::EstadoCluster>>,
    /// O que o servidor esta fazendo AGORA: atividades, threads e as series.
    ///
    /// `Arc` porque as threads de fundo carregam o registro consigo para
    /// anotar o que estao fazendo, e elas vivem mais que qualquer emprestimo.
    telemetria: Arc<crate::telemetria::Telemetria>,
}

impl Servidor {
    pub fn novo(config: Config) -> Result<Arc<Servidor>> {
        // `recursos.cache_paginas` estava no config.json e na documentacao
        // desde a 0.13.0 -- e nao era lido por ninguem, porque o cache nao
        // existia. Agora existe, e o campo passa a valer. Tem de ser aqui,
        // ANTES de a primeira tabela abrir: o teto vale para o que abrir
        // daqui para a frente.
        phxsql_store::ndx::definir_cache_paginas(config.recursos.cache_paginas);
        let instancia = Instancia::nova(&config.base)?;
        let log = LogAcessos::abrir(&config.log_acessos)?;
        let lista_negra = Blacklist::abrir(&config.blacklist)?;
        let dblink = crate::dblink::Registro::abrir(&config.dblink)?;
        let jobs = crate::jobs::Registro::abrir(&config.jobs)?;
        let cluster = config.cluster.clone().map(|c| {
            Arc::new(crate::cluster::EstadoCluster::novo(
                c,
                &config.base,
                config.replicacao.papel,
            ))
        });
        Ok(Arc::new(Servidor {
            cluster,
            janela: Janela::nova(&config.recursos),
            sujas: Mutex::new(std::collections::HashSet::new()),
            config,
            dados: Mutex::new(instancia),
            log: Mutex::new(log),
            lista_negra: Mutex::new(lista_negra),
            sessoes: Mutex::new(http::Sessoes::default()),
            residentes: Mutex::new(HashMap::new()),
            remotos: Mutex::new(HashMap::new()),
            ligacoes: Mutex::new(crate::ligacoes::Ligacoes::default()),
            desde_ms: crate::agora_ms(),
            monitor: Mutex::new(crate::sistema::Monitor::novo()),
            dblink: Mutex::new(dblink),
            jobs: Mutex::new(jobs),
            relogio_de_jobs: AtomicBool::new(false),
            jobs_rodando: Mutex::new(Vec::new()),
            avisos_de_jobs: Mutex::new(HashMap::new()),
            porta_no_ar: AtomicBool::new(false),
            parar_de_aceitar: AtomicBool::new(false),
            proximo_ouvinte: Mutex::new(None),
            endereco_dos_dados: Mutex::new(None),
            avisados: Mutex::new(HashMap::new()),
            conexoes: AtomicUsize::new(0),
            cargas: Mutex::new(crate::carga::Cargas::default()),
            marcas_do_diario: Mutex::new(HashMap::new()),
            profiler: Mutex::new(crate::profiler::Profiler::default()),
            profiler_ligado: AtomicBool::new(false),
            telemetria: Arc::new(crate::telemetria::Telemetria::default()),
        }))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Toma a trava unica de dados -- e e o UNICO lugar que a toma.
    ///
    /// # Por que passar por aqui
    ///
    /// A trava de dados e o gargalo declarado deste servidor: toda leitura e
    /// toda escrita passam por ela, uma de cada vez. Entao «quanto tempo se
    /// espera por ela» e «quanto tempo alguem a segura» sao os dois numeros
    /// que explicam um servidor lento -- e nenhum dos dois existia.
    ///
    /// Ha 50 tomadas de trava neste arquivo. Medir em cada uma seria copiar a
    /// mesma conta 50 vezes, e a que alguem esquecesse viraria o buraco na
    /// serie -- a mesma razao pela qual o portao de permissao e UM so.
    ///
    /// # O que custa
    ///
    /// Ligada: dois `Instant::now()` por OPERACAO (nao por linha), num
    /// caminho em que a operacao mais barata ja leva dezenas de
    /// microssegundos. Desligada: um `load(Relaxed)`, e nem o relogio e lido.
    fn travar_dados(&self) -> Result<TravaMedida<'_>> {
        let medindo = self.telemetria.ligada();
        let atividade = if medindo {
            crate::telemetria::corrente()
        } else {
            None
        };
        // O estado muda ANTES de a thread parar na fila: e essa marca que faz
        // a bolha aparecer amarela «esperando» enquanto outra atividade
        // segura a trava. Depois seria tarde -- a thread ja estaria bloqueada.
        if let Some(a) = &atividade {
            a.esperando_trava();
        }
        let pedida = medindo.then(Instant::now);
        let guarda = self.dados.lock().map_err(|_| trava_envenenada());
        // UM relogio para as duas contas: o instante em que a trava chegou na
        // mao e o fim da espera e o comeco da posse. Ler o relogio duas vezes
        // aqui pagaria duas chamadas para saber a mesma coisa.
        let obtida = medindo.then(Instant::now);
        if let Some(a) = &atividade {
            a.com_a_trava();
        }
        if let (Some(t), Some(o)) = (pedida, obtida) {
            self.telemetria
                .contar_espera(o.duration_since(t).as_micros() as u64);
        }
        Ok(TravaMedida {
            guarda: guarda?,
            tomada: obtida,
            telemetria: &self.telemetria,
        })
    }

    /// O registro da telemetria, para quem precisa anotar alguma coisa nele.
    pub fn telemetria(&self) -> &Arc<crate::telemetria::Telemetria> {
        &self.telemetria
    }

    /// Sobe o servidor e atende ate o processo ser encerrado.
    pub fn escutar(self: &Arc<Self>) -> Result<()> {
        let endereco = self.config.endereco()?;
        let ouvinte = TcpListener::bind(endereco)
            .map_err(|e| PhxError::Esquema(format!("nao consegui escutar em {endereco}: {e}")))?;
        eprintln!(
            "PhxSql {VERSAO} escutando em {endereco} | base {} | papel {}",
            self.config.base.display(),
            self.config.replicacao.papel.nome()
        );
        eprintln!("log de acessos: {}", self.config.log_acessos.display());
        if self.config.replicacao.papel != crate::config::Papel::Isolado {
            let portas = self.config.replicacao.portas();
            eprintln!(
                "replicacao: papel {} | {}",
                self.config.replicacao.papel.nome(),
                if portas.is_empty() {
                    "envio e retorno pela porta de dados".to_string()
                } else {
                    portas
                        .iter()
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            );
            if self.config.replicacao.papel == crate::config::Papel::Source
                && !self.config.replicacao.imagem_da_linha
            {
                eprintln!(
                    "ATENCAO: source com replicacao.imagem_da_linha DESLIGADA. O \
                     diario grava que a linha mudou, nao grava para que, e as \
                     replicas nao terao o que aplicar."
                );
            }
        }

        self.subir_web();
        self.subir_replicacao();
        self.subir_cluster();
        self.subir_backup_agendado();
        self.subir_jobs();
        self.ligar_relogio_de_gravacao();
        self.ligar_vigia_de_disco();
        self.ligar_vigia_de_jobs();
        self.subir_amostrador();
        // A thread PRINCIPAL tambem entra no registro. Ela nao e criada por
        // ninguem -- e o processo --, e por isso era a unica que faltaria numa
        // lista montada so pelos `spawn`. Um inventario com um buraco e um
        // inventario em que nao se confia.
        let principal = self.telemetria.registrar_fio(
            "aceitador-dados",
            "a thread principal: fica no `accept` da porta de dados e entrega \
             cada conexao nova a uma thread de atendimento",
            "servico",
            crate::agora_ms(),
        );
        principal.fazendo("aceitando conexoes");

        self.anotar_porta_no_ar(&ouvinte);
        let mut atual = Some(ouvinte);
        loop {
            match atual.take() {
                Some(o) => {
                    self.aceitar_ate_mandarem_parar(&o);
                    // A porta so e SOLTA aqui, depois do laco sair -- e o
                    // ouvinte novo, quando ha, ja esta preso desde antes de
                    // qualquer coisa parar. Ver `op_servico_subir`.
                    drop(o);
                    self.porta_no_ar.store(false, Ordering::SeqCst);
                    eprintln!("porta de dados PARADA (a interface web continua no ar)");
                }
                // Parada: a linha de execucao fica aqui, de olho no pedido de
                // subir de novo. Um quarto de segundo de espera so acontece
                // enquanto o servico esta parado, que e o caso raro.
                None => std::thread::sleep(Duration::from_millis(250)),
            }
            if let Ok(mut p) = self.proximo_ouvinte.lock() {
                if let Some(novo) = p.take() {
                    self.anotar_porta_no_ar(&novo);
                    atual = Some(novo);
                }
            }
        }
    }

    /// Aceita conexoes ate alguem pedir para parar.
    ///
    /// # Como o `accept` acorda
    ///
    /// Ele bloqueia, e nao ha como interromper um `accept` bloqueado sem
    /// mexer no laco. As duas saidas eram: pesquisar de tempos em tempos com
    /// o soquete em modo nao bloqueante, ou ACORDAR o laco com uma conexao.
    ///
    /// A pesquisa foi descartada por medicao de custo, e nao por gosto: um
    /// intervalo de 100 ms poe ate 100 ms de espera em TODA conexao nova, o
    /// tempo inteiro, para servir um pedido de parada que acontece uma vez por
    /// mes. O despertador custa zero enquanto ninguem para: quem pede a parada
    /// levanta o sinalizador e conecta no proprio endereco, o `accept`
    /// devolve, e a primeira coisa do laco e olhar o sinalizador.
    ///
    /// A conexao do despertador nao vira sessao: o laco sai antes de atender.
    fn aceitar_ate_mandarem_parar(self: &Arc<Self>, ouvinte: &TcpListener) {
        loop {
            let conexao = ouvinte.accept();
            // ANTES de atender: se o pedido de parada chegou junto com uma
            // conexao de verdade, quem mandou parar ganha -- e a conexao
            // recusada volta a existir quando a porta subir de novo.
            if self.parar_de_aceitar.swap(false, Ordering::SeqCst) {
                return;
            }
            match conexao {
                Ok((fluxo, _)) => {
                    // Sem isto, o Nagle segura a resposta ate 40 ms esperando
                    // mais bytes para encher um pacote -- e nunca vem mais,
                    // porque a resposta acabou. Medido: a pagina de uma tabela
                    // de 20.000 linhas levava 1 ms de servidor e 44 ms de
                    // relogio, e 43 deles eram esta linha faltando.
                    //
                    // O protocolo aqui e pedido-resposta curto, que e o caso
                    // exato em que o Nagle atrapalha em vez de ajudar.
                    let _ = fluxo.set_nodelay(true);
                    let par = fluxo.peer_addr().ok();
                    if self.conexoes.load(Ordering::SeqCst) >= self.config.conexoes_max {
                        // Recusa sem derrubar o servico, e deixa registro.
                        if let Some(p) = par {
                            self.anotar(&Acesso {
                                quando_ms: crate::agora_ms(),
                                ip: p.ip().to_string(),
                                porta_origem: p.port(),
                                op: "conexao".into(),
                                usuario: String::new(),
                                autenticado: false,
                                ok: false,
                                duracao_ms: 0,
                                erro: Some("limite de conexoes atingido".into()),
                                database: String::new(),
                                tabela: String::new(),
                                codigo: 0,
                            });
                        }
                        continue;
                    }
                    let servidor = Arc::clone(self);
                    self.conexoes.fetch_add(1, Ordering::SeqCst);
                    let endereco = par.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
                    self.telemetria.subir(
                        format!("dados-{}", endereco.port()),
                        "atende UMA conexao da porta de dados, do login ate o fim: \
                         le uma linha, despacha o pedido e responde, em laco",
                        "atendimento",
                        crate::agora_ms(),
                        move |fio| {
                            fio.fazendo(&format!("conexao de {endereco}"));
                            servidor.atender(fluxo, endereco);
                            servidor.conexoes.fetch_sub(1, Ordering::SeqCst);
                        },
                    );
                }
                Err(e) => eprintln!("conexao recusada pelo sistema: {e}"),
            }
        }
    }

    /// Guarda o endereco em que a porta de dados esta escutando AGORA.
    ///
    /// Ele pode diferir do `bind` do `config.json` depois de uma troca pela
    /// tela -- e a tela mostra os dois lado a lado justamente por isso.
    /// Configuracao que nao e lida mente; endereco corrente que finge ser o
    /// configurado mente do mesmo jeito.
    fn anotar_porta_no_ar(&self, ouvinte: &TcpListener) {
        if let Ok(e) = ouvinte.local_addr() {
            if let Ok(mut atual) = self.endereco_dos_dados.lock() {
                *atual = Some(e);
            }
            eprintln!("porta de dados escutando em {e}");
        }
        self.porta_no_ar.store(true, Ordering::SeqCst);
    }

    /// Violacao grave: bloqueia na hora e avisa no log.
    fn violacao_grave(&self, ip: &str, comando: &str, motivo: &str) {
        if let Ok(mut lista) = self.lista_negra.lock() {
            let (b, aviso) = lista.violacao_grave(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            );
            eprintln!(
                "BLOQUEADO {ip} ate {} -- {} ({})",
                b.ate(),
                b.motivo,
                b.comando
            );
            if let Some(a) = aviso {
                eprintln!("AVISO: {a}");
            }
        }
    }

    /// Tentativa leve: conta, e bloqueia se passar do limite na janela.
    fn violacao_leve(&self, ip: &str, comando: &str, motivo: &str) {
        if let Ok(mut lista) = self.lista_negra.lock() {
            if let Some((b, aviso)) = lista.tentativa_leve(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            ) {
                eprintln!(
                    "BLOQUEADO {ip} ate {} -- {} apos {} tentativas",
                    b.ate(),
                    b.motivo,
                    b.tentativas
                );
                if let Some(a) = aviso {
                    eprintln!("AVISO: {a}");
                }
            }
        }
    }

    fn anotar(&self, acesso: &Acesso) {
        if let Ok(mut log) = self.log.lock() {
            if let Err(e) = log.registrar(acesso) {
                eprintln!("falha ao gravar o log de acessos: {e}");
            }
        }
    }

    /// Este IP esta barrado agora? Devolve o motivo, ja formatado para o log.
    ///
    /// Reaproveitado pelas duas portas: a de dados e a da interface web. Um IP
    /// bloqueado e bloqueado no servidor inteiro, nao numa porta so.
    fn barrado(&self, ip: &str, agora: i64) -> Option<String> {
        let mut lista = self.lista_negra.lock().ok()?;
        // Outro processo pode ter mexido no arquivo (phxsqld --desbloquear).
        let _ = lista.recarregar_se_mudou();
        let _ = lista.limpar_vencidos(agora, &self.config.politica);
        lista.bloqueado(ip, agora).map(|b| {
            format!(
                "bloqueado desde {} ate {} por {} ({})",
                b.desde(),
                b.ate(),
                b.motivo,
                b.comando
            )
        })
    }

    /// Sobe o relogio do backup agendado, se ligado.
    ///
    /// Confere de minuto em minuto em vez de dormir ate a hora certa: dormir
    /// horas seguidas e frageil -- a maquina suspende, o relogio anda, e o
    /// backup nao acontece sem ninguem notar.
    /// O relogio que fecha a janela de durabilidade quando ninguem grava.
    ///
    /// Sem ele, a gravacao em lote so sincronizaria na PROXIMA gravacao -- e um
    /// servidor que recebe a ultima venda do dia as 18h e fica quieto deixaria
    /// essa venda sem `fsync` a noite inteira. O relogio acorda a cada janela e
    /// descarrega o que ficou.
    ///
    /// Em `por_operacao` e em `sistema` ele nao tem o que fazer: um sincroniza
    /// sempre, o outro nunca.
    fn ligar_relogio_de_gravacao(self: &Arc<Self>) {
        if self.config.recursos.durabilidade != Durabilidade::PorLote {
            return;
        }
        let ms = self.config.recursos.lote_milissegundos.max(20);
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "relogio-gravacao",
            "fecha a janela de durabilidade quando ninguem grava: sem ela, a \
             ultima venda do dia ficaria sem `fsync` a noite inteira",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                std::thread::sleep(Duration::from_millis(ms));
                if servidor.janela.pendente() > 0 {
                    fio.fazendo("descarregando o que ficou pendente");
                    servidor.janela.fechar();
                    servidor.descarregar_sujas();
                } else {
                    fio.fazendo(&format!("nada pendente; acorda a cada {ms} ms"));
                }
            },
        );
    }

    /// Sobe o amostrador das series da telemetria.
    ///
    /// # Por que UMA thread, e nao a conta na hora de perguntar
    ///
    /// Taxa exige dois instantes. Se cada aba aberta calculasse a propria,
    /// cada uma teria a sua base de comparacao e duas telas mostrariam
    /// numeros diferentes do mesmo servidor -- e a primeira pergunta de cada
    /// aba nao teria taxa nenhuma. Uma amostragem so, de segundo em segundo,
    /// da a mesma serie para todo mundo.
    ///
    /// A thread sobe SEMPRE, mesmo com a telemetria desligada, e e barata
    /// justamente por isso: desligada, `amostrar` devolve no portao antes de
    /// ler qualquer `/proc`. Subir a thread junto com o interruptor exigiria
    /// que ligar a telemetria criasse thread -- e ai o custo de ligar
    /// dependeria de quantas vezes alguem ligou e desligou.
    fn subir_amostrador(self: &Arc<Self>) {
        if !self.telemetria.marcar_amostrador() {
            return;
        }
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "amostrador",
            "tira, de segundo em segundo, a amostra das series do painel de \
             telemetria: esperas, vazao, CPU do processo, leitura e escrita \
             fisicas e acertos do cache do .ndx",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                let comeco = Instant::now();
                if servidor.telemetria.ligada() {
                    // A CPU da MAQUINA sai do mesmo monitor que o painel de
                    // sistema ja usa, e nao de uma segunda leitura do
                    // `/proc/stat`: duas leituras com bases diferentes dariam
                    // dois percentuais para o mesmo instante.
                    let maquina = crate::sistema::jiffies_da_maquina();
                    let agora = crate::agora_ms();
                    servidor.telemetria.amostrar(maquina, agora);
                    // A poda anda junto com a amostra porque as duas sao do
                    // mesmo relogio: uma aba fechada para de pedir, e um
                    // minuto depois a bolha dela sai.
                    servidor.telemetria.podar(agora, 60_000);
                    fio.fazendo("amostra tirada");
                } else {
                    fio.fazendo("telemetria desligada: nao amostra");
                }
                // Desconta o que a amostra custou, para o periodo ser o
                // periodo e nao "o periodo mais o trabalho". Sem isso a serie
                // andaria mais devagar do que ela mesma diz que anda.
                let gasto = comeco.elapsed();
                let periodo = Duration::from_millis(crate::telemetria::PERIODO_DA_AMOSTRA_MS);
                std::thread::sleep(periodo.saturating_sub(gasto));
            },
        );
    }

    /// Uma thread por origem, puxando os eventos do source.
    ///
    /// Uma por origem e nao uma so: multi-source e varias conexoes
    /// independentes, e uma origem lenta ou caida nao pode segurar as outras.
    fn subir_replicacao(self: &Arc<Self>) {
        if self.cluster.is_some() {
            // Com cluster, quem puxa e o laco do proprio cluster, do master
            // CORRENTE -- uma lista fixa de origens apontaria para o master
            // de ontem, e dois lacos aplicando na mesma tabela brigariam.
            if !self.config.replicacao.origens.is_empty() {
                eprintln!(
                    "AVISO: replicacao.origens e IGNORADA num servidor com o \
                     bloco cluster -- a origem e o master corrente, descoberto \
                     pelo pulso"
                );
            }
            return;
        }
        if self.config.replicacao.papel != crate::config::Papel::Replica {
            return;
        }
        if self.config.replicacao.origens.is_empty() {
            eprintln!(
                "replicacao: papel replica sem nenhuma origem em \
                 replicacao.origens -- nada a puxar"
            );
            return;
        }
        if !self.config.somente_leitura {
            // Nao e erro, e e uma pedra no caminho conhecida: uma replica
            // escrita pela aplicacao quebra a numeracao dos rowids, e a
            // proxima inclusao vinda do source para a replicacao inteira.
            eprintln!(
                "ATENCAO: replica sem somente_leitura. Se a aplicacao escrever \
                 aqui, os rowids divergem e a replicacao para."
            );
        }
        for origem in self.config.replicacao.origens.clone() {
            if !origem.senha.is_empty() && origem.senha_hash.is_empty() {
                eprintln!(
                    "AVISO: origem {} com a SENHA EM TEXTO PURO no config.json. \
                     Troque por senha_hash: phxsqld --senha",
                    origem.nome
                );
            }
            eprintln!(
                "replicacao: puxando de {} ({}:{}) a cada {}s",
                origem.nome, origem.host, origem.porta, origem.reconectar_em
            );
            let servidor = Arc::clone(self);
            let nome = format!("replica-{}", origem.nome);
            self.telemetria.subir(
                nome,
                "puxa os eventos do diario de UMA origem e os aplica aqui; uma \
                 por origem, para que uma origem caida nao segure as outras",
                "servico",
                crate::agora_ms(),
                move |fio| {
                    fio.fazendo("conectando na origem");
                    servidor.laco_da_replica(origem);
                },
            );
        }
    }

    /// O laco de uma origem: conectar, puxar, aplicar, dormir, repetir.
    ///
    /// Erro nao mata a thread -- ele escreve e espera. Um source que caiu volta
    /// e a replica retoma do numero em que parou; matar a thread exigiria
    /// reiniciar a replica para religar a replicacao.
    /// O laco que puxa do source, para sempre.
    ///
    /// # Rodada produtiva nao dorme
    ///
    /// O `reconectar_em` e o intervalo entre PERGUNTAS EM VAO -- quanto tempo
    /// esperar antes de perguntar de novo a um source que nao tinha nada. Uma
    /// rodada que aplicou eventos nao espera: se o source tinha o que dar,
    /// provavelmente ainda tem, porque ele continuou escrevendo enquanto esta
    /// rodada aplicava.
    ///
    /// Dormir depois de toda rodada era o que fazia a replica parecer lenta.
    /// A bancada media `linhas / tempo_ate_alcancar` e chegava a 4.273
    /// eventos/s -- mas o caminho de CPU inteiro, dos dois lados, custa 23,9 us
    /// por evento (`--example onde-doi-na-replica`), o que da mais de 40.000/s.
    /// O que sobrava era sono, e nao trabalho: o numero media o `reconectar_em`.
    fn laco_da_replica(self: Arc<Self>, origem: crate::config::Origem) {
        let espera = Duration::from_secs(origem.reconectar_em);
        loop {
            match self.rodada_da_replica(&origem) {
                // Nada a fazer: agora sim, espera antes de perguntar de novo.
                Ok(0) => std::thread::sleep(espera),
                Ok(n) => {
                    eprintln!("replicacao [{}]: {n} evento(s) aplicado(s)", origem.nome);
                    // Sem sono: volta ja. `alcancar_tabela` recusa girar em
                    // falso -- ela erra se aplicar e a posicao nao andar --,
                    // entao um `Ok(n)` com n > 0 e progresso de verdade e este
                    // laco nao tem como virar giro em vazio.
                }
                // Erro dorme, e e de proposito: source fora do ar ou conexao
                // caida pedem espera, senao a replica bate na porta fechada
                // num laco fechado.
                Err(e) => {
                    eprintln!("replicacao [{}]: {e}", origem.nome);
                    std::thread::sleep(espera);
                }
            }
        }
    }

    /// Uma passada por todas as tabelas de todos os databases da origem.
    ///
    /// Devolve quantos eventos aplicou.
    fn rodada_da_replica(&self, origem: &crate::config::Origem) -> Result<u64> {
        let mut cliente = crate::replica::ligar(origem)?;
        let databases = if origem.databases.is_empty() {
            cliente.databases()?
        } else {
            origem.databases.clone()
        };

        let mut aplicados = 0u64;
        for database in databases {
            let (com_imagem, tabelas) = crate::replica::posicao(&mut cliente, &database)?;
            if !com_imagem {
                return Err(PhxError::Esquema(format!(
                    "o source de {} esta com replicacao.imagem_da_linha desligada: \
                     o diario dele nao carrega a linha, e nao ha o que aplicar",
                    origem.nome
                )));
            }
            for no in tabelas {
                aplicados += self.alcancar_tabela(&mut cliente, &database, &no)?;
            }
        }
        Ok(aplicados)
    }

    /// Traz UMA tabela ate a posicao do source.
    fn alcancar_tabela(
        &self,
        cliente: &mut crate::replica::Cliente,
        database: &str,
        no: &crate::replica::NoSource,
    ) -> Result<u64> {
        let _trava = self.travar_dados()?;
        let db = _trava.garantir_database(database)?;

        // Tabela que ainda nao existe aqui nasce do MESMO bloco de esquema que
        // o source tem, e nao de uma remontagem a partir de JSON: e assim que
        // o payload da imagem cai byte a byte no lugar certo.
        let mut tabela = match db.abrir_qualificada(&no.nome) {
            Ok(t) => t,
            Err(_) => match &no.esquema {
                Some(e) => {
                    let (schema, nome) = match no.nome.split_once('.') {
                        Some((s, n)) => (Some(s.to_string()), n.to_string()),
                        None => (None, no.nome.clone()),
                    };
                    let _ = nome;
                    eprintln!("replicacao: criando {database}.{} aqui", no.nome);
                    db.criar_tabela(schema.as_deref(), e.clone())?
                }
                None => return Ok(0),
            },
        };
        // O diario DESTA replica tambem carrega a imagem quando configurado.
        // Sem isto, uma replica intermediaria grava eventos sem linha dentro, e
        // a replica que puxa DELA nao tem o que aplicar -- a cascata
        // Master -> Slave01 -> Slave02 morre no segundo salto. Este caminho
        // abre a tabela direto, sem passar pelo `abrir_travada` que liga a
        // imagem para os pedidos que vem pela porta.
        tabela.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);

        let mut posicao = tabela.eventos()?;
        if posicao >= no.eventos {
            return Ok(0);
        }
        let mut aplicados = 0u64;
        while posicao < no.eventos {
            let eventos = crate::replica::puxar(cliente, database, &no.nome, posicao)?;
            if eventos.is_empty() {
                break;
            }
            for e in &eventos {
                tabela.aplicar_evento(e.operacao, e.rowid, &e.imagem)?;
                aplicados += 1;
            }
            // A posicao LOCAL, e nao `posicao + eventos.len()`: aplicar gera
            // eventos no diario daqui, e e por ele que a proxima rodada se
            // orienta. Contar do lado do source deixaria os dois numeros
            // andarem separados no primeiro evento que nao gerasse outro.
            let nova = tabela.eventos()?;
            if nova <= posicao {
                // Aplicou e a posicao nao andou: o proximo pedido traria os
                // mesmos eventos, e o laco giraria em falso para sempre.
                return Err(PhxError::Corrompido(format!(
                    "replicacao de {database}.{}: {} evento(s) aplicado(s) e a \
                     posicao continua em {posicao}",
                    no.nome,
                    eventos.len()
                )));
            }
            posicao = nova;
        }
        tabela.sincronizar()?;
        Ok(aplicados)
    }

    // -------------------------------------------------------------- cluster

    /// Sobe as tres pecas do cluster: o pulso (uma thread por par), o arbitro
    /// e o laco que puxa do master corrente. Sem o bloco `cluster` no
    /// `config.json`, nenhuma delas existe.
    fn subir_cluster(self: &Arc<Self>) {
        let Some(estado) = &self.cluster else { return };
        let c = &estado.config;
        eprintln!(
            "cluster: {} nos | este e {} ({}, epoca {}) | janela {}s | pulso {}s | {}",
            c.nos.len(),
            c.id,
            estado.papel().nome(),
            estado.epoca(),
            c.janela_s,
            c.pulso_s,
            if c.email.ligado {
                format!(
                    "avisa {} a cada {:.1} min enquanto degradado",
                    c.email.para.join(", "),
                    c.avisar_cada_min
                )
            } else {
                "sem e-mail (aviso so no log)".to_string()
            }
        );
        if c.nos.len() == 2 {
            // Nao e recusa: dois nos replicam e redirecionam normalmente. So a
            // PROMOCAO automatica nunca acontece, e melhor dizer no arranque
            // do que deixar descobrir na primeira queda.
            eprintln!(
                "ATENCAO: cluster de DOIS nos nunca se promove sozinho -- com o \
                 master caido o que sobra ve 1 de 2, e metade nao e maioria. \
                 Promocao automatica pede tres ou mais nos."
            );
        }
        for no in c.outros() {
            let servidor = Arc::clone(self);
            let no = no.clone();
            self.telemetria.subir(
                format!("pulso-{}", no.id),
                "manda o pulso para UM no do cluster e escuta o dele: e por \
                 este batimento que a queda do master e descoberta",
                "servico",
                crate::agora_ms(),
                move |fio| {
                    fio.fazendo("pulsando");
                    servidor.laco_do_pulso(no);
                },
            );
        }
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "arbitro-cluster",
            "decide quem e o master: conta os pulsos, apura a maioria e promove \
             quando o master de antes para de responder",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("apurando a maioria");
                servidor.laco_do_arbitro();
            },
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "replica-cluster",
            "puxa do master CORRENTE do cluster, que muda a cada promocao -- e \
             por isso ela nao pode ser uma origem fixa do config.json",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("seguindo o master corrente");
                servidor.laco_da_replica_do_cluster();
            },
        );
    }

    /// Pulsa UM outro no, para sempre: conecta, autentica como a replicacao,
    /// e troca `cluster_pulso` a cada intervalo. Cada lado da troca aprende o
    /// estado do outro -- o pedido leva o meu, a resposta traz o dele.
    ///
    /// Erro aqui e rotina, nao noticia: no caido e exatamente o que o mapa
    /// registra envelhecendo, e quem fala sobre isso e o arbitro, uma vez --
    /// nao esta thread, a cada pulso perdido.
    fn laco_do_pulso(self: Arc<Self>, no: crate::config::NoCluster) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        let intervalo = Duration::from_secs(estado.config.pulso_s);
        loop {
            let _ = self.pulsar(&estado, &no);
            std::thread::sleep(intervalo);
        }
    }

    /// Uma conexao de pulso: dura ate o primeiro erro, e o laco de fora
    /// reconecta. O prazo de conexao e CURTO de proposito -- um no morto nao
    /// pode segurar a conferencia dos vivos alem do proprio pulso.
    fn pulsar(
        &self,
        estado: &crate::cluster::EstadoCluster,
        no: &crate::config::NoCluster,
    ) -> Result<()> {
        let c = &estado.config;
        let espera = Duration::from_secs((c.pulso_s * 2).max(5));
        let prazo = Duration::from_secs(c.pulso_s.clamp(1, 2));
        let mut cliente = crate::replica::Cliente::conectar_com_prazo(
            &no.endereco,
            no.porta,
            &c.token,
            espera,
            prazo,
        )?;
        if !c.usuario.is_empty() {
            cliente.autenticar(&c.usuario, &c.senha_hash, "")?;
        }
        loop {
            let r = cliente.pedir(vec![
                ("op", Json::texto_de("cluster_pulso")),
                ("id", Json::texto_de(&c.id)),
                ("papel", Json::texto_de(estado.papel().nome())),
                ("epoca", Json::de_u64(estado.epoca())),
                ("posicao", Json::de_u64(estado.posicao())),
                ("prioridade", Json::de_i64(c.prioridade)),
            ])?;
            if let Some((id, pulso)) = crate::cluster::PulsoDeNo::de_json(&r) {
                estado.registrar(&id, pulso);
            }
            std::thread::sleep(Duration::from_secs(c.pulso_s));
        }
    }

    /// O arbitro: a cada meio segundo olha o mapa e decide -- rebaixar,
    /// eleger, liberar ou recusar escrita, avisar. As decisoes moram em
    /// `cluster.rs`; aqui e so o relogio e as consequencias.
    fn laco_do_arbitro(self: Arc<Self>) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        let mut ultima_conta = 0i64;
        let mut motivos_anteriores: Vec<String> = Vec::new();
        loop {
            let agora = crate::agora_ms();
            // A posicao local no ritmo do pulso, e nao do tique: ela toma a
            // trava de dados, e o dobro da frequencia nao compraria nada.
            if agora - ultima_conta >= estado.config.pulso_s as i64 * 1_000 {
                let p = self.posicao_do_diario(&estado.config.databases);
                estado.definir_posicao(p);
                ultima_conta = agora;
            }
            let motivos = self.rodada_do_arbitro(&estado, agora);
            // So a MUDANCA vira log: um cluster degradado continua degradado,
            // e repetir a cada tique afogaria a noticia seguinte.
            if motivos != motivos_anteriores {
                for m in &motivos {
                    eprintln!("cluster: {m}");
                }
                if motivos.is_empty() {
                    eprintln!("cluster: normalizado");
                }
                motivos_anteriores = motivos.clone();
            }
            estado.definir_degradacao(motivos);
            self.avisos_do_cluster(&estado, agora);
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Uma rodada de decisao. Devolve os motivos de degradacao ATUAIS.
    fn rodada_do_arbitro(&self, estado: &crate::cluster::EstadoCluster, agora: i64) -> Vec<String> {
        use crate::cluster::{Candidato, PapelVivo};
        let c = &estado.config;
        let mapa = estado.mapa();
        let mut motivos = Vec::new();

        // A graca do arranque: por uma janela, "nunca deu pulso" e "ainda nao
        // deu tempo", nao degradacao -- o primeiro tique roda antes do
        // primeiro pulso, e sem isto todo cluster nasceria doente.
        let em_graca = agora - estado.nascido_ms() <= c.janela_ms();
        let mut vivos_qtd = 1usize; // eu
        for no in c.outros() {
            match mapa.get(&no.id) {
                Some(p) if agora - p.quando_ms <= c.janela_ms() => vivos_qtd += 1,
                Some(p) => motivos.push(format!(
                    "no {} sem pulso ha {}s",
                    no.id,
                    (agora - p.quando_ms) / 1_000
                )),
                None if em_graca => {}
                None => motivos.push(format!("no {} nunca deu pulso", no.id)),
            }
        }

        match estado.papel() {
            PapelVivo::Master => {
                // Epoca maior no ar = houve eleicao sem mim. O destronado se
                // rebaixa SOZINHO -- e o que resolve "dois masters" quando o
                // antigo volta da particao ou do reinicio.
                let maior = estado.maior_epoca_vista();
                if maior > estado.epoca() {
                    eprintln!(
                        "cluster: ha epoca {maior} no ar e a minha e {} -- houve \
                         eleicao enquanto este no esteve fora; REBAIXANDO a replica",
                        estado.epoca()
                    );
                    let _ = estado.rebaixar(maior);
                    motivos.push("este no foi rebaixado: um master de epoca maior assumiu".into());
                    return motivos;
                }
                // Dois masters na MESMA epoca (dois configs com papel source,
                // ou um empate de particao): perde quem a eleicao nao
                // escolheria -- a MESMA conta de `vencedor`, para os dois
                // lados decidirem igual.
                for (id, p) in &mapa {
                    if p.papel == PapelVivo::Master
                        && p.epoca == estado.epoca()
                        && agora - p.quando_ms <= c.janela_ms()
                    {
                        let eu = Candidato {
                            id: c.id.clone(),
                            posicao: estado.posicao(),
                            prioridade: c.prioridade,
                        };
                        let ele = Candidato {
                            id: id.clone(),
                            posicao: p.posicao,
                            prioridade: p.prioridade,
                        };
                        let vence = crate::cluster::vencedor(&[eu, ele], 1).map(|v| v.id.clone());
                        if vence.as_deref() != Some(c.id.as_str()) {
                            eprintln!(
                                "cluster: {id} tambem e master na epoca {} e ganha \
                                 o desempate; REBAIXANDO este no a replica",
                                estado.epoca()
                            );
                            let _ = estado.rebaixar(estado.epoca());
                            motivos.push(format!("este no perdeu o desempate para {id}"));
                            return motivos;
                        }
                    }
                }
                // Master isolado nao escreve: e o que limita o split-brain ao
                // tempo de DETECCAO -- o que entrou antes disso e a cauda que
                // se perde, e docs/CLUSTER.md diz isso sem eufemismo. Na graca
                // do arranque a escrita fica como nasceu (liberada): ainda nao
                // houve tempo de um pulso chegar, e recusar aqui seria recusar
                // todo arranque de master por uma janela.
                let tem_maioria = c.e_maioria(vivos_qtd);
                if tem_maioria || !em_graca {
                    estado.liberar_escrita(tem_maioria);
                }
                if !tem_maioria && !em_graca {
                    motivos.push(format!(
                        "sem maioria visivel ({vivos_qtd} de {}): escrita recusada",
                        c.nos.len()
                    ));
                }
            }
            PapelVivo::Replica => {
                // Diario local A FRENTE do master e a cauda de um antigo
                // master: nao ha como replicar para tras. Aviso, nao conserto
                // -- apagar dado sozinho nunca.
                if let Some((id, _)) = estado.master_atual() {
                    if let Some(p) = mapa.get(&id) {
                        if agora - p.quando_ms <= c.janela_ms() && estado.posicao() > p.posicao {
                            motivos.push(format!(
                                "diario local ({}) a frente do master {id} ({}): \
                                 provavel cauda de escritas perdidas -- ressemeie \
                                 este no a partir do master",
                                estado.posicao(),
                                p.posicao
                            ));
                        }
                    }
                }
                let silencio = agora - estado.master_visto_ms();
                if silencio > c.janela_ms() {
                    let vivos = estado.vivos(agora);
                    match crate::cluster::vencedor(&vivos, c.nos.len()) {
                        // O teste de protecao mais importante da bateria: sem
                        // maioria visivel, ficar degradado E a decisao certa.
                        None => motivos.push(format!(
                            "master calado ha {}s e sem maioria visivel ({} de {}): \
                             NAO promovo",
                            silencio / 1_000,
                            vivos.len(),
                            c.nos.len()
                        )),
                        Some(v) if v.id == c.id => {
                            let motivo = format!(
                                "master calado ha {}s; eleito entre {} vivos de {} \
                                 configurados",
                                silencio / 1_000,
                                vivos.len(),
                                c.nos.len()
                            );
                            if let Err(e) = self.promover_a_master(&motivo) {
                                motivos.push(format!("promocao falhou: {e}"));
                            }
                        }
                        Some(v) => motivos.push(format!(
                            "master calado ha {}s; aguardando {} assumir",
                            silencio / 1_000,
                            v.id
                        )),
                    }
                }
            }
        }
        motivos
    }

    /// PROMOVE este no a master do cluster: epoca nova (a maior vista + 1),
    /// papel persistido, escrita liberada, aviso agendado.
    ///
    /// E o UNICO caminho de promocao. A eleicao automatica chama daqui, e
    /// qualquer promocao MANUAL que venha a existir deve cair aqui tambem --
    /// dois caminhos de promover e a porta dos fundos classica: o que alguem
    /// esquecer de atualizar vira o furo.
    pub fn promover_a_master(&self, motivo: &str) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let epoca = estado.promover(estado.maior_epoca_vista() + 1)?;
        eprintln!("cluster: PROMOVIDO a master na epoca {epoca} -- {motivo}");
        estado.anotar_promocao(format!(
            "O no {} assumiu como master do cluster, na epoca {epoca}.\n\
             Motivo: {motivo}\n\
             As replicas passam a segui-lo sozinhas; clientes que escreverem \
             num outro no recebem REDIRECIONA {}.",
            estado.config.id,
            estado
                .config
                .no(&estado.config.id)
                .map(|n| n.alvo())
                .unwrap_or_default()
        ));
        // O log de acessos guarda tambem o que o servidor decide sozinho --
        // sem isto, a unica prova da promocao seria o comportamento mudar.
        self.anotar(&Acesso {
            quando_ms: crate::agora_ms(),
            ip: "(local)".into(),
            porta_origem: 0,
            op: "cluster_promocao".into(),
            usuario: estado.config.id.clone(),
            autenticado: true,
            ok: true,
            duracao_ms: 0,
            erro: None,
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });
        Ok(Json::objeto(vec![
            ("papel", Json::texto_de("master")),
            ("epoca", Json::de_u64(epoca)),
        ]))
    }

    /// Os e-mails do cluster: a promocao avisa UMA vez; a degradacao repete a
    /// cada `avisar_cada_min` enquanto durar. Sem e-mail configurado, nada.
    fn avisos_do_cluster(&self, estado: &crate::cluster::EstadoCluster, agora: i64) {
        if !estado.config.email.ligado {
            return;
        }
        if let Some(texto) = estado.tomar_aviso_de_promocao() {
            match crate::email::enviar(
                &estado.config.email,
                "PhxSql cluster: promocao de master",
                &texto,
            ) {
                Ok(r) => eprintln!("cluster: e-mail de promocao enviado: {r}"),
                Err(e) => eprintln!("cluster: e-mail de promocao NAO ENVIADO: {e}"),
            }
        }
        let motivos = estado.degradacao();
        if motivos.is_empty() || !estado.hora_de_avisar(agora) {
            return;
        }
        let corpo = format!(
            "O cluster PhxSql esta degradado. O no {} ve:\n\n{}\n\n\
             Este aviso repete a cada {:.1} min enquanto durar.\n\
             Servidor PhxSql {VERSAO}\nQuando: {}\n",
            estado.config.id,
            motivos
                .iter()
                .map(|m| format!("  - {m}"))
                .collect::<Vec<_>>()
                .join("\n"),
            estado.config.avisar_cada_min,
            phxsql_core::datahora::instante_iso(agora)
        );
        let assunto = format!(
            "PhxSql cluster degradado ({} motivo{})",
            motivos.len(),
            if motivos.len() == 1 { "" } else { "s" }
        );
        match crate::email::enviar(&estado.config.email, &assunto, &corpo) {
            Ok(r) => eprintln!("cluster: e-mail de degradacao enviado: {r}"),
            Err(e) => eprintln!("cluster: e-mail de degradacao NAO ENVIADO: {e}"),
        }
    }

    /// A soma dos eventos das tabelas replicadas -- a posicao que o pulso
    /// carrega e que a eleicao compara. Toma a trava de dados; por isso quem
    /// chama e o arbitro, no ritmo do pulso, e o resultado fica em cache.
    fn posicao_do_diario(&self, so_estes: &[String]) -> u64 {
        let Ok(trava) = self.dados.lock() else {
            return 0;
        };
        let bases = if so_estes.is_empty() {
            trava.databases().unwrap_or_default()
        } else {
            so_estes.to_vec()
        };
        let mut total = 0u64;
        for b in bases {
            let Ok(db) = trava.abrir_database(&b) else {
                continue;
            };
            for t in db.todas_as_tabelas().unwrap_or_default() {
                if let Ok(mut tab) = db.abrir_qualificada(&t) {
                    total += tab.eventos().unwrap_or(0);
                }
            }
        }
        total
    }

    /// O laco que puxa do master CORRENTE -- e a unica diferenca para o laco
    /// da replicacao comum: a origem sai do mapa do cluster a cada rodada, em
    /// vez de sair fixa do `config.json`. Promovido, ele para de puxar
    /// sozinho, porque o papel vivo e conferido a cada volta.
    fn laco_da_replica_do_cluster(self: Arc<Self>) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        let espera = Duration::from_secs(estado.config.pulso_s);
        loop {
            let c = &estado.config;
            if estado.papel() == crate::cluster::PapelVivo::Master {
                std::thread::sleep(espera);
                continue;
            }
            let alvo = estado
                .master_atual()
                .filter(|(id, _)| id != &c.id)
                .and_then(|(id, _)| c.no(&id).cloned());
            let Some(no) = alvo else {
                std::thread::sleep(espera);
                continue;
            };
            let origem = crate::config::Origem {
                nome: format!("cluster:{}", no.id),
                host: no.endereco.clone(),
                porta: no.porta,
                token: c.token.clone(),
                databases: c.databases.clone(),
                reconectar_em: c.pulso_s,
                usuario: c.usuario.clone(),
                senha_hash: c.senha_hash.clone(),
                senha: String::new(),
            };
            match self.rodada_da_replica(&origem) {
                // Nada novo: espera o pulso seguinte.
                Ok(0) => std::thread::sleep(espera),
                Ok(n) => eprintln!("cluster: {n} evento(s) aplicado(s) do master {}", no.id),
                Err(e) => {
                    eprintln!("cluster: replicacao do master {}: {e}", no.id);
                    std::thread::sleep(espera);
                }
            }
        }
    }

    fn sem_cluster() -> PhxError {
        PhxError::Esquema(
            "este servidor nao esta em cluster: nao ha o bloco \"cluster\" no \
             config.json"
                .into(),
        )
    }

    /// `cluster_pulso`: registra o pulso de OUTRO no e devolve o proprio --
    /// uma troca, e cada lado sai sabendo do outro.
    fn op_cluster_pulso(&self, p: &Json) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let Some((id, pulso)) = crate::cluster::PulsoDeNo::de_json(p) else {
            return Err(PhxError::Esquema(
                "informe \"id\" e \"papel\" (master ou replica)".into(),
            ));
        };
        // No fora da lista e configuracao torta em algum lugar -- recusar em
        // voz alta e o que faz o erro aparecer no primeiro pulso, e nao numa
        // eleicao com um eleitor fantasma.
        if estado.config.no(&id).is_none() {
            return Err(PhxError::Autorizacao(format!(
                "o no {id:?} nao esta na lista de nos deste cluster"
            )));
        }
        if id == estado.config.id {
            return Err(PhxError::Esquema(format!(
                "o no {id:?} e ESTE servidor -- dois nos com o mesmo id no ar"
            )));
        }
        estado.registrar(&id, pulso);
        Ok(Json::objeto(vec![
            ("id", Json::texto_de(&estado.config.id)),
            ("papel", Json::texto_de(estado.papel().nome())),
            ("epoca", Json::de_u64(estado.epoca())),
            ("posicao", Json::de_u64(estado.posicao())),
            ("prioridade", Json::de_i64(estado.config.prioridade)),
        ]))
    }

    /// `cluster_estado`: quem e o master, a epoca e o mapa dos nos --
    /// respondida igual em QUALQUER no. E o endereco unico do cluster pelo
    /// protocolo: o cliente valida com um endereco qualquer e e apontado ao
    /// certo. VIP de rede e infraestrutura, nao banco.
    fn op_cluster_estado(&self) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let agora = crate::agora_ms();
        let c = &estado.config;
        let mapa = estado.mapa();
        let nos: Vec<Json> = c
            .nos
            .iter()
            .map(|n| {
                let (papel, epoca, posicao, idade_ms) = if n.id == c.id {
                    (
                        Some(estado.papel().nome()),
                        estado.epoca(),
                        estado.posicao(),
                        0i64,
                    )
                } else {
                    match mapa.get(&n.id) {
                        Some(p) => (
                            Some(p.papel.nome()),
                            p.epoca,
                            p.posicao,
                            agora - p.quando_ms,
                        ),
                        None => (None, 0, 0, -1),
                    }
                };
                Json::objeto(vec![
                    ("id", Json::texto_de(&n.id)),
                    ("endereco", Json::texto_de(n.alvo())),
                    ("este", Json::Bool(n.id == c.id)),
                    (
                        "papel",
                        match papel {
                            Some(p) => Json::texto_de(p),
                            None => Json::Nulo,
                        },
                    ),
                    ("epoca", Json::de_u64(epoca)),
                    ("posicao", Json::de_u64(posicao)),
                    // -1 = nunca deu pulso; 0 = este proprio no.
                    ("ultimo_pulso_ms", Json::de_i64(idade_ms)),
                    (
                        "vivo",
                        Json::Bool(n.id == c.id || (0..=c.janela_ms()).contains(&idade_ms)),
                    ),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("id", Json::texto_de(&c.id)),
            ("papel", Json::texto_de(estado.papel().nome())),
            ("epoca", Json::de_u64(estado.epoca())),
            (
                "master",
                match estado.master_atual().and_then(|(id, _)| c.no(&id).cloned()) {
                    Some(n) => Json::objeto(vec![
                        ("id", Json::texto_de(&n.id)),
                        ("endereco", Json::texto_de(n.alvo())),
                    ]),
                    None => Json::Nulo,
                },
            ),
            ("escrita_liberada", Json::Bool(estado.escrita_liberada())),
            (
                "degradado",
                Json::Lista(estado.degradacao().iter().map(Json::texto_de).collect()),
            ),
            ("janela_inatividade_s", Json::de_u64(c.janela_s)),
            ("nos", Json::Lista(nos)),
        ]))
    }

    fn subir_backup_agendado(self: &Arc<Self>) {
        if !self.config.backup.agendado {
            return;
        }
        let b = &self.config.backup;
        eprintln!(
            "backup agendado: {} | destino {} | {} | guarda {}",
            if b.hora.is_empty() {
                format!("a cada {} h", b.cada_horas)
            } else {
                format!("todo dia as {}", b.hora)
            },
            b.destino.display(),
            if b.zip {
                "um zip por vez"
            } else {
                "arvore de diretorios"
            },
            if b.manter == 0 {
                "tudo".to_string()
            } else {
                format!("os {} mais novos", b.manter)
            }
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "backup-agendado",
            "confere de minuto em minuto se chegou a hora do backup e o executa; \
             dormir ate a hora certa seria fragil -- a maquina suspende e o \
             relogio anda",
            "servico",
            crate::agora_ms(),
            move |fio| {
                let mut ultimo = 0i64;
                loop {
                    let agora = crate::agora_ms();
                    if servidor.config.backup.hora_de_rodar(agora, ultimo) {
                        ultimo = agora;
                        fio.fazendo("copiando e conferindo o SHA-256");
                        match servidor.rodar_backup_agendado(agora) {
                            Ok(onde) => eprintln!("backup agendado: {onde}"),
                            Err(e) => eprintln!("backup agendado FALHOU: {e}"),
                        }
                    } else {
                        fio.fazendo("esperando a hora marcada");
                    }
                    std::thread::sleep(Duration::from_secs(60));
                }
            },
        );
    }

    fn rodar_backup_agendado(&self, quando: i64) -> Result<String> {
        let b = &self.config.backup;
        let (onde, r) = {
            let _trava = self.travar_dados()?;
            if b.zip {
                let (caminho, r) = phxsql_store::backup::executar_zip(
                    &self.config.base,
                    &b.destino,
                    &b.database,
                    &b.admin,
                    quando,
                )?;
                (caminho.display().to_string(), r)
            } else {
                let pasta = b.destino.join(
                    phxsql_core::datahora::instante_iso(quando).replace([' ', ':', ','], "-"),
                );
                let r = phxsql_store::backup::executar(
                    &self.config.base,
                    &pasta,
                    &phxsql_core::datahora::instante_iso(quando),
                )?;
                (pasta.display().to_string(), r)
            }
        };

        // O log de acessos guarda tambem o que o servidor faz sozinho: senao,
        // a unica prova de que o backup rodou seria o arquivo existir.
        self.anotar(&Acesso {
            quando_ms: quando,
            ip: "(local)".into(),
            porta_origem: 0,
            op: "backup_agendado".into(),
            usuario: b.admin.clone(),
            autenticado: true,
            ok: true,
            duracao_ms: 0,
            erro: None,
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });

        let apagados = self.limpar_backups_velhos();
        Ok(format!(
            "{onde} ({} arquivos, {} bytes{}{})",
            r.arquivos.len(),
            r.bytes,
            if b.zip {
                format!(", zip de {} bytes", r.comprimido)
            } else {
                String::new()
            },
            if apagados > 0 {
                format!(", {apagados} antigo(s) apagado(s)")
            } else {
                String::new()
            }
        ))
    }

    /// Guarda so os `manter` mais novos. Zero nao apaga nada.
    ///
    /// Olha apenas os `.zip` cujo nome tem a cara dos nossos. Backup nao
    /// apaga arquivo que nao criou -- alguem pode ter guardado outra coisa
    /// nessa pasta.
    fn limpar_backups_velhos(&self) -> usize {
        let b = &self.config.backup;
        if b.manter == 0 || !b.zip {
            return 0;
        }
        let Ok(dir) = std::fs::read_dir(&b.destino) else {
            return 0;
        };
        let nomes: Vec<String> = dir
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();
        let mut apagados = 0;
        for nome in phxsql_store::backup::escolher_para_apagar(&nomes, b.manter) {
            if std::fs::remove_file(b.destino.join(&nome)).is_ok() {
                apagados += 1;
            }
        }
        apagados
    }

    // ----------------------------------------------- a porta de dados, na tela

    /// O que a tela do Serviço precisa saber para nao mentir.
    fn op_servico(&self) -> Result<Json> {
        let corrente = self.endereco_dos_dados.lock().ok().and_then(|e| *e);
        let configurado = self.config.bind.clone();
        Ok(Json::objeto(vec![
            ("no_ar", Json::Bool(self.porta_no_ar.load(Ordering::SeqCst))),
            (
                "endereco",
                match corrente {
                    Some(e) => Json::texto_de(e.to_string()),
                    None => Json::Nulo,
                },
            ),
            // Os dois lado a lado de proposito. Uma troca pela tela vale ate o
            // proximo arranque -- ela NAO reescreve o config.json, que carrega
            // comentario e o resto da configuracao. Sem mostrar os dois, quem
            // reiniciar a maquina meses depois nao entende por que a porta
            // voltou a ser outra.
            ("bind_configurado", Json::texto_de(&configurado)),
            (
                "difere_do_arquivo",
                Json::Bool(match (&corrente, self.config.endereco()) {
                    (Some(c), Ok(cfg)) => *c != cfg,
                    _ => false,
                }),
            ),
            (
                "conexoes",
                Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
            ),
            ("web", Json::texto_de(&self.config.web.bind)),
            ("web_ligada", Json::Bool(self.config.web.ligado)),
        ]))
    }

    /// Para de aceitar conexao nova na porta de dados.
    ///
    /// # Quem fica sem resposta
    ///
    /// Ninguem que ja esta conectado: as conexoes vivas continuam ate elas
    /// mesmas acabarem. O que para e o `accept`. Cliente NOVO recebe recusa de
    /// conexao do sistema operacional -- o mesmo que receberia com o servico
    /// desligado --, e a resposta diz quantas conexoes ficaram abertas para
    /// quem clicou saber o que esta interrompendo.
    ///
    /// # Como se volta
    ///
    /// Pela mesma tela: o processo continua vivo, a interface web continua no
    /// ar na porta dela, e `servico_subir` religa. E por isso que este botao
    /// **nao** derruba o processo: um botao que se desfaz e um botao; um que
    /// nao se desfaz e um alcapao.
    fn op_servico_parar(&self) -> Result<Json> {
        if !self.porta_no_ar.load(Ordering::SeqCst) {
            return Err(PhxError::Esquema("a porta de dados ja esta parada".into()));
        }
        let abertas = self.conexoes.load(Ordering::SeqCst);
        self.parar_de_aceitar.store(true, Ordering::SeqCst);
        self.acordar_o_accept()?;
        Ok(Json::objeto(vec![
            ("parando", Json::Bool(true)),
            ("conexoes_abertas", Json::de_u64(abertas as u64)),
            (
                "aviso",
                Json::texto_de(
                    "as conexoes ja abertas seguem ate acabarem; o que parou foi aceitar \
                     conexao nova. A interface web continua no ar, e e por ela que a porta \
                     volta",
                ),
            ),
        ]))
    }

    /// Sobe a porta de dados, no mesmo endereco ou em outro.
    ///
    /// # A ordem que evita o tiro no pe
    ///
    /// O endereco novo e PRESO primeiro. So depois de o `bind` dar certo e que
    /// o laco antigo e mandado parar e solta o endereco velho. Porta ocupada,
    /// permissao negada (porta abaixo de 1024 sem raiz) ou endereco escrito
    /// errado falham AQUI, com o servico intacto e nada trocado -- em vez de
    /// deixar a maquina sem porta de dados nenhuma e sem jeito de voltar.
    fn op_servico_subir(&self, p: &Json) -> Result<Json> {
        let pedido = p.texto_ou("bind", "").trim().to_string();
        let alvo = if pedido.is_empty() {
            // Sem endereco: volta para onde estava, ou para o do arquivo.
            match self.endereco_dos_dados.lock().ok().and_then(|e| *e) {
                Some(e) => e,
                None => self.config.endereco()?,
            }
        } else {
            crate::config::endereco_de(&pedido)?
        };

        let no_ar = self.porta_no_ar.load(Ordering::SeqCst);
        let atual = self.endereco_dos_dados.lock().ok().and_then(|e| *e);
        if no_ar && atual == Some(alvo) {
            return Err(PhxError::Esquema(format!(
                "a porta de dados ja esta no ar em {alvo}"
            )));
        }

        let novo = TcpListener::bind(alvo).map_err(|e| {
            PhxError::Esquema(format!(
                "nao consegui escutar em {alvo}: {e}. Nada mudou -- o servico continua \
                 como estava"
            ))
        })?;
        {
            let mut prox = self
                .proximo_ouvinte
                .lock()
                .map_err(|_| trava_envenenada())?;
            *prox = Some(novo);
        }
        if no_ar {
            self.parar_de_aceitar.store(true, Ordering::SeqCst);
            self.acordar_o_accept()?;
        }
        Ok(Json::objeto(vec![
            ("subindo_em", Json::texto_de(alvo.to_string())),
            ("trocou_de_porta", Json::Bool(atual != Some(alvo))),
            (
                "aviso",
                Json::texto_de(
                    "vale ate o proximo arranque: o config.json nao foi reescrito. Para \
                     valer sempre, mude o campo bind no arquivo",
                ),
            ),
        ]))
    }

    /// Acorda o `accept` bloqueado conectando no proprio endereco.
    ///
    /// # O endereco para onde conectar nao e sempre o do `bind`
    ///
    /// Um servidor preso em `0.0.0.0:5000` escuta em toda placa, e conectar
    /// literalmente em `0.0.0.0` so funciona por acidente do sistema. Com
    /// endereco nao especificado, o despertador vai pelo `localhost` na mesma
    /// porta, que e o caminho que sempre existe.
    fn acordar_o_accept(&self) -> Result<()> {
        let Some(onde) = self.endereco_dos_dados.lock().ok().and_then(|e| *e) else {
            return Err(PhxError::Esquema(
                "nao sei em que endereco a porta esta escutando".into(),
            ));
        };
        let destino = if onde.ip().is_unspecified() {
            match onde {
                SocketAddr::V4(_) => SocketAddr::from(([127, 0, 0, 1], onde.port())),
                SocketAddr::V6(_) => SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, onde.port())),
            }
        } else {
            onde
        };
        match TcpStream::connect_timeout(&destino, Duration::from_secs(3)) {
            // O soquete morre aqui mesmo: o laco sai antes de atender, e do
            // outro lado isto e so o toque que fez o `accept` devolver.
            Ok(_) => Ok(()),
            Err(e) => {
                // O sinalizador volta: deixa-lo levantado faria a PROXIMA
                // conexao de verdade derrubar a porta, minutos depois, sem
                // ninguem ter pedido.
                self.parar_de_aceitar.store(false, Ordering::SeqCst);
                if let Ok(mut prox) = self.proximo_ouvinte.lock() {
                    *prox = None;
                }
                Err(PhxError::Esquema(format!(
                    "nao consegui acordar o laco de aceitacao em {destino}: {e}. \
                     Nada mudou"
                )))
            }
        }
    }

    // ------------------------------------------------------- jobs de execucao

    /// O cadastro, o estado completo de cada um e o historico das corridas.
    fn op_jobs(&self, p: &Json) -> Result<Json> {
        let quantas = p.inteiro_ou("historico", 50).clamp(0, 500) as usize;
        let relogio = self.relogio_de_jobs_no_ar();
        // A foto dos que rodam agora sai ANTES da trava do cadastro -- a
        // ordem das duas travas e sempre esta, para nunca haver abraco.
        let rodando_agora: Vec<String> = self
            .jobs_rodando
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let r = self.jobs.lock().map_err(|_| trava_envenenada())?;
        let agora = crate::agora_ms();
        let lista: Vec<Json> = r
            .jobs
            .iter()
            .map(|j| {
                let ultimo = r.ultimo_de(&j.nome);
                let rodando = rodando_agora
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(&j.nome));
                let ultima = r.ultima_corrida_de(&j.nome);
                let mut pares = match j.ficha() {
                    Json::Objeto(x) => x,
                    _ => Vec::new(),
                };
                pares.push((
                    "ultimo_ms".to_string(),
                    if ultimo == 0 {
                        Json::Nulo
                    } else {
                        Json::de_i64(ultimo)
                    },
                ));
                // "Venceria agora?" e o que a tela precisa para dizer se um
                // job esta atrasado -- e sai da MESMA funcao que o relogio
                // usa, para os dois nunca discordarem.
                let vencido = j.ligado && j.agenda.hora_de_rodar(agora, ultimo);
                pares.push(("vencido".to_string(), Json::Bool(vencido)));
                pares.push(("rodando".to_string(), Json::Bool(rodando)));
                pares.push((
                    "estado".to_string(),
                    Json::texto_de(crate::jobs::estado_do_job(
                        j.ligado,
                        rodando,
                        ultima.map(|c| c.ok),
                        relogio,
                    )),
                ));
                // Parado e o vencido que ninguem vai rodar -- mesma funcao
                // que o vigia de e-mail usa.
                pares.push((
                    "parado".to_string(),
                    Json::Bool(crate::jobs::job_parado(j.ligado, rodando, vencido, relogio)),
                ));
                // A ultima corrida com resultado -- a semeada do log conta,
                // para "falhou as 03:00" sobreviver a um reinicio.
                pares.push((
                    "ultima".to_string(),
                    match ultima {
                        Some(c) => c.para_json(),
                        None => Json::Nulo,
                    },
                ));
                // A proxima prevista, pela mesma conta do relogio. Pode estar
                // no passado: vencida e informacao, nao erro.
                if j.ligado {
                    let proximo = j.agenda.proximo_ms(agora, ultimo);
                    pares.push(("proximo_ms".to_string(), Json::de_i64(proximo)));
                    pares.push((
                        "proxima".to_string(),
                        Json::texto_de(phxsql_core::datahora::instante_iso(proximo)),
                    ));
                } else {
                    pares.push(("proximo_ms".to_string(), Json::Nulo));
                }
                Json::Objeto(pares)
            })
            .collect();
        let historico: Vec<Json> = r
            .historico(quantas)
            .iter()
            .map(crate::jobs::Corrida::para_json)
            .collect();
        let email = &self.config.alertas.email;
        Ok(Json::objeto(vec![
            ("arquivo", Json::texto_de(r.caminho.display().to_string())),
            (
                "log",
                Json::texto_de(r.caminho_do_log().display().to_string()),
            ),
            ("relogio_no_ar", Json::Bool(relogio)),
            // O estado do aviso por e-mail, para a tela dizer a verdade sobre
            // quem sera avisado -- os enderecos ja aparecem na op `config`,
            // que exige o mesmo `administrar` que esta aqui.
            (
                "aviso_email",
                Json::objeto(vec![
                    // A MESMA funcao que decide se o e-mail sai -- escrita
                    // duas vezes, a copia esquecida viraria uma tela que
                    // mente sobre o aviso.
                    ("ligado", Json::Bool(self.aviso_de_jobs_ligado())),
                    ("email_ligado", Json::Bool(email.ligado)),
                    ("avisar_jobs", Json::Bool(email.avisar_jobs)),
                    (
                        "para",
                        Json::Lista(email.para.iter().map(Json::texto_de).collect()),
                    ),
                    (
                        "repetir_horas",
                        Json::de_u64(self.config.alertas.repetir_horas),
                    ),
                ]),
            ),
            ("jobs", Json::Lista(lista)),
            ("historico", Json::Lista(historico)),
        ]))
    }

    /// Liga ou desliga um job pelo nome, sem tocar no resto da ficha.
    ///
    /// Existe para a tela nao reenviar o job inteiro so para virar uma chave:
    /// reenviar a ficha lida ha minutos gravaria por cima do que outro
    /// administrador mudou nesse meio tempo -- o mesmo estrago da gravacao
    /// sem versao, por outra porta.
    fn op_job_ligar(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        let Some(ligado) = p.campo("ligado").and_then(Json::booleano) else {
            return Err(PhxError::Esquema(
                "informe \"ligado\": true liga, false desliga".into(),
            ));
        };
        let mut r = self.jobs.lock().map_err(|_| trava_envenenada())?;
        let mut job = r.achar(&nome)?.clone();
        let nome = job.nome.clone();
        job.ligado = ligado;
        r.salvar(job)?;
        Ok(Json::objeto(vec![
            ("job", Json::texto_de(nome)),
            ("ligado", Json::Bool(ligado)),
            // Mesmo aviso do salvar: ligar so vale sozinho se ha relogio.
            ("relogio_no_ar", Json::Bool(self.relogio_de_jobs_no_ar())),
        ]))
    }

    fn op_job_salvar(&self, p: &Json) -> Result<Json> {
        let job = crate::jobs::Job::de_json(p.campo("job").unwrap_or(p))?;
        // Conferido na hora de salvar, e nao so na hora de rodar: descobrir
        // que o login nao existe as tres da manha, no historico, e pior do que
        // descobrir agora, com a tela aberta.
        self.sessao_do_job(&job)?;
        let mut r = self.jobs.lock().map_err(|_| trava_envenenada())?;
        let nome = job.nome.clone();
        r.salvar(job)?;
        Ok(Json::objeto(vec![
            ("salvo", Json::texto_de(nome)),
            // O relogio le o cadastro a cada volta, entao ligar um job vale na
            // proxima. Mas se NENHUM estava ligado quando o servidor subiu,
            // nao ha relogio -- e a tela precisa dizer isso, senao o job fica
            // ligado e parado sem ninguem entender por que.
            ("relogio_no_ar", Json::Bool(self.relogio_de_jobs_no_ar())),
        ]))
    }

    fn op_job_excluir(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        let mut r = self.jobs.lock().map_err(|_| trava_envenenada())?;
        r.excluir(&nome)?;
        Ok(Json::objeto(vec![("excluido", Json::texto_de(nome))]))
    }

    /// Roda um job agora, fora da agenda.
    ///
    /// O `rodar_job` faz a conferencia de permissao com o usuario DO JOB, e
    /// nao com quem clicou -- senao rodar agora seria um jeito de emprestar o
    /// proprio poder para o job. Quem clica precisa de `administrar`, que e o
    /// que o portao ja exigiu para chegar ate aqui.
    fn op_job_rodar(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        let inicio = crate::agora_ms();
        let r = self.rodar_job(&nome, "tela");
        if let Ok(mut reg) = self.jobs.lock() {
            reg.anotar_corrida(&nome, inicio);
        }
        Ok(Json::objeto(vec![
            ("job", Json::texto_de(nome)),
            ("ok", Json::Bool(r.is_ok())),
            ("duracao_ms", Json::de_i64(crate::agora_ms() - inicio)),
            (
                "detalhe",
                Json::texto_de(match &r {
                    Ok(j) => resumir_resposta(j),
                    Err(e) => e.to_string(),
                }),
            ),
            ("resposta", r.unwrap_or(Json::Nulo)),
        ]))
    }

    /// Ha relogio de jobs rodando neste processo?
    ///
    /// Ele so sobe se algum job estava ligado no arranque -- entao ligar o
    /// primeiro job pela tela nao acorda ninguem ate o proximo arranque. Dizer
    /// isso e melhor do que subir uma linha de execucao que fica acordando de
    /// trinta em trinta segundos num servidor que nao tem job nenhum.
    fn relogio_de_jobs_no_ar(&self) -> bool {
        self.relogio_de_jobs.load(Ordering::SeqCst)
    }

    /// Sobe o relogio dos jobs, se houver algum ligado.
    ///
    /// Um relogio so para todos, e nao um por job: o trabalho de perguntar
    /// "chegou a hora?" e uma comparacao de inteiros, e uma linha de execucao
    /// por job custaria pilha para ficar dormindo.
    fn subir_jobs(self: &Arc<Self>) {
        let ligados: Vec<String> = match self.jobs.lock() {
            Ok(r) => r
                .jobs
                .iter()
                .filter(|j| j.ligado)
                .map(|j| format!("{} ({}, {})", j.nome, j.op(), j.agenda.rotulo()))
                .collect(),
            Err(_) => return,
        };
        if ligados.is_empty() {
            // Sem job ligado nao ha relogio: instrumentacao desligada custa
            // zero, e o portao que decide isso vem ANTES do trabalho.
            return;
        }
        eprintln!("jobs de execucao: {}", ligados.join(" | "));
        self.relogio_de_jobs.store(true, Ordering::SeqCst);
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "relogio-jobs",
            "acorda de tempos em tempos, ve quais jobs venceram a hora e os \
             executa, um por um, com o poder do usuario de cada um",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                let agora = crate::agora_ms();
                // A trava sai antes de executar: um job de backup segura a
                // trava dos dados por segundos, e prender o cadastro junto
                // travaria a tela de jobs e todos os outros jobs enquanto isso.
                let vencidos = match servidor.jobs.lock() {
                    Ok(mut r) => {
                        let v = r.vencidos(agora);
                        for nome in &v {
                            r.anotar_corrida(nome, agora);
                        }
                        v
                    }
                    Err(_) => Vec::new(),
                };
                if vencidos.is_empty() {
                    fio.fazendo("nenhum job vencido");
                }
                for nome in vencidos {
                    fio.fazendo(&format!("rodando o job {nome}"));
                    let _ = servidor.rodar_job(&nome, "agenda");
                }
                std::thread::sleep(Duration::from_secs(crate::jobs::PERIODO_DO_RELOGIO_S));
            },
        );
    }

    /// Roda um job agora: monta a sessao dele, passa pelos portoes e executa.
    ///
    /// # Por que ele nao roda "como o servidor"
    ///
    /// Porque um agendador com poder proprio e um jeito de contornar a
    /// permissao: bastaria escrever no cadastro de jobs a operacao que a rede
    /// recusaria. O job carrega o login de um usuario do cadastro e roda com o
    /// poder DAQUELE usuario -- e usuario que sumiu ou foi desativado para o
    /// job, com erro escrito, em vez de cair para uma sessao sem dono, que e
    /// o que o `Default` daria.
    ///
    /// # A politica e conferida aqui, e nao no portao comum
    ///
    /// `portoes_do_pedido` deixou de fora o que so faz sentido com um IP do
    /// outro lado. Comando proibido pela politica vale igual para o job -- o
    /// `config.json` diz que ninguem pede aquilo neste servidor --, mas nao ha
    /// IP para bloquear: a recusa vira linha no historico.
    fn rodar_job(&self, nome: &str, disparado_por: &str) -> Result<Json> {
        let inicio = crate::agora_ms();
        // A copia sai de dentro da trava para o job poder rodar por segundos
        // sem prender o cadastro -- e a tela de jobs continua respondendo.
        let job = match self.jobs.lock() {
            Ok(r) => r.achar(nome)?.clone(),
            Err(_) => return Err(trava_envenenada()),
        };
        let op = job.op().to_string();

        // O nome entra na lista dos que rodam agora ANTES de executar e sai
        // logo depois: e o que deixa a tela dizer "rodando" e impede o vigia
        // de tratar um backup de dez minutos como job parado.
        self.marcar_rodando(&job.nome, true);
        let resultado = self.executar_job(&job, &op);
        self.marcar_rodando(&job.nome, false);
        let corrida = crate::jobs::Corrida {
            quando_ms: inicio,
            job: job.nome.clone(),
            op: op.clone(),
            usuario: job.usuario.clone(),
            ok: resultado.is_ok(),
            duracao_ms: crate::agora_ms() - inicio,
            detalhe: match &resultado {
                // A resposta inteira nao entra: uma varredura de vinte mil
                // linhas nao cabe no historico e nao interessa a ele. O que
                // interessa e ter rodado, e o que voltou de resumo.
                Ok(j) => resumir_resposta(j),
                Err(e) => e.to_string(),
            },
        };
        if let Ok(mut r) = self.jobs.lock() {
            r.registrar(&corrida);
        }
        // O aviso por e-mail, se foi pedido -- e a limpeza do silencio quando
        // o job volta a rodar. Depois do registrar: o historico nunca pode
        // depender de o rele estar no ar.
        self.avisar_sobre_a_corrida(&job, &corrida);
        // O job tambem entra no `acessos.log`, como qualquer outra operacao:
        // quem audita o servidor nao deveria precisar saber que existe um
        // segundo arquivo para descobrir que uma tabela foi mexida.
        self.anotar(&Acesso {
            quando_ms: inicio,
            ip: format!("(job:{disparado_por})"),
            porta_origem: 0,
            op: format!("job:{op}"),
            usuario: job.usuario.clone(),
            autenticado: !job.usuario.is_empty(),
            ok: resultado.is_ok(),
            duracao_ms: corrida.duracao_ms.max(0) as u64,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            database: job.pedido.texto_ou("database", "").to_string(),
            tabela: job.pedido.texto_ou("tabela", "").to_string(),
            codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        });
        if let Err(e) = &resultado {
            eprintln!("job {} FALHOU: {e}", job.nome);
        }
        resultado
    }

    /// A politica, que no `despachar` roda antes de tudo -- para um pedido que
    /// NAO veio pela rede.
    ///
    /// O que fica de fora e o que so faz sentido com um IP do outro lado: a
    /// politica de comando proibido bloqueia quem pediu, e bloquear "o
    /// agendador" ou "o tradutor de SQL" nao quer dizer nada. O resto vale
    /// igual, e mora aqui num lugar so pela razao de sempre: a copia que
    /// alguem esquecer de atualizar vira o furo.
    fn politica_do_pedido(&self, op: &str, pedido: &Json) -> Result<()> {
        if self.config.politica.comando_proibido(op) {
            return Err(PhxError::Autorizacao(format!(
                "operacao {op} esta proibida neste servidor pela politica"
            )));
        }
        let base = pedido.texto_ou("database", "");
        if self.config.politica.base_proibida(base) {
            return Err(PhxError::Autorizacao(format!(
                "a base {base} esta proibida neste servidor pela politica"
            )));
        }
        // Mesma sonda de travessia da porta de dados. Um job e escrito por um
        // administrador, mas o arquivo pode ter vindo de outro lugar -- e um
        // `FROM ../../etc` chega pelo tradutor de SQL sem passar pela sonda
        // que o `despachar` fez no pedido de fora.
        for (rotulo, valor) in [
            ("database", base),
            ("tabela", pedido.texto_ou("tabela", "")),
            ("schema", pedido.texto_ou("schema", "")),
        ] {
            if !valor.is_empty() && phxsql_store::catalogo::nome_hostil(valor) {
                return Err(PhxError::Autorizacao(format!(
                    "{rotulo} {valor:?} nao e um nome"
                )));
            }
        }
        Ok(())
    }

    /// Executa um pedido que o SERVIDOR derivou de outro, pelos mesmos portoes.
    ///
    /// # Por que isto existe
    ///
    /// Duas coisas aqui dentro montam um pedido e mandam executar: a op `sql`,
    /// que traduz um `SELECT` para `varrer` ou `buscar`, e -- por outro
    /// caminho -- o agendador de jobs. Nenhuma das duas pode virar a porta dos
    /// fundos: quem nao pode ler a folha de pagamento tambem nao pode le-la
    /// escrevendo `SELECT * FROM folha`, e o portao que confere isso e o
    /// MESMO, lendo o campo `tabela` do pedido TRADUZIDO.
    ///
    /// A licao ja estava escrita no projeto: `juntar` e `unir` foram esse furo
    /// uma vez, porque a tabela delas nao passava pelo campo que o portao olha.
    /// A tradução resolve isso pelo outro lado -- ela PRODUZ o campo que o
    /// portao ja sabe olhar, em vez de pedir um portao novo.
    fn executar_derivado(&self, op: &str, pedido: &Json, sessao: &Sessao) -> Result<Json> {
        self.politica_do_pedido(op, pedido)?;
        self.portoes_do_pedido(op, pedido, sessao)?;
        self.executar(op, pedido, sessao)
    }

    fn executar_job(&self, job: &crate::jobs::Job, op: &str) -> Result<Json> {
        // A politica antes de saber sob qual usuario o job roda: um comando
        // proibido e proibido para todo mundo, e recusar por ele da a mensagem
        // certa a um job cujo dono tambem esta errado.
        self.politica_do_pedido(op, &job.pedido)?;
        let sessao = self.sessao_do_job(job)?;
        self.portoes_do_pedido(op, &job.pedido, &sessao)?;
        self.executar(op, &job.pedido, &sessao)
    }

    /// A sessao sob a qual o job roda.
    ///
    /// Sem cadastro de usuarios, o servidor inteiro entra sem login e o job
    /// acompanha -- e o mesmo comportamento da rede, e nao uma excecao. COM
    /// cadastro, o login e obrigatorio e tem de existir e estar ativo.
    fn sessao_do_job(&self, job: &crate::jobs::Job) -> Result<Sessao> {
        if self.config.cadastro.vazio() {
            return Ok(Sessao::default());
        }
        if job.usuario.is_empty() {
            return Err(PhxError::Autorizacao(format!(
                "job {:?} nao diz sob qual usuario roda, e este servidor tem cadastro. \
                 Um job sem dono rodaria com poder que ninguem concedeu",
                job.nome
            )));
        }
        let u = self
            .config
            .cadastro
            .por_login(&job.usuario)
            .ok_or_else(|| {
                PhxError::Autorizacao(format!(
                    "job {:?}: o usuario {:?} nao esta no cadastro",
                    job.nome, job.usuario
                ))
            })?;
        if !u.ativo {
            return Err(PhxError::Autorizacao(format!(
                "job {:?}: o usuario {:?} esta desativado",
                job.nome, job.usuario
            )));
        }
        Ok(Sessao {
            usuario: Some(u.clone()),
            ..Sessao::default()
        })
    }

    // ------------------------------------------------- aviso de jobs por e-mail

    /// Entra e sai da lista dos jobs em execucao agora.
    fn marcar_rodando(&self, nome: &str, esta: bool) {
        if let Ok(mut g) = self.jobs_rodando.lock() {
            if esta {
                g.push(nome.to_string());
            } else if let Some(i) = g.iter().position(|n| n == nome) {
                g.remove(i);
            }
        }
    }

    /// O aviso esta ligado? E O portao, um so, e vem antes de qualquer
    /// trabalho: desligado, quem chama paga duas leituras de booleano e nada
    /// mais -- nenhuma trava, nenhuma String, nenhum parse.
    ///
    /// Opt-in de proposito: `avisar_jobs` e um campo proprio no bloco de
    /// e-mail. Sem bloco de e-mail nada muda, e quem configurou e-mail so
    /// para o disco tambem continua exatamente como estava.
    fn aviso_de_jobs_ligado(&self) -> bool {
        let email = &self.config.alertas.email;
        email.ligado && email.avisar_jobs
    }

    /// Depois de cada corrida: avisa a falha por e-mail, e limpa o silencio
    /// de quem voltou a rodar.
    ///
    /// A limpeza espelha o vigia de disco: enquanto o job falha, no maximo um
    /// aviso por janela de `repetir_horas`; quando volta a dar certo, a chave
    /// sai do mapa e a PROXIMA falha avisa na hora, porque e noticia nova.
    fn avisar_sobre_a_corrida(&self, job: &crate::jobs::Job, corrida: &crate::jobs::Corrida) {
        if !self.aviso_de_jobs_ligado() {
            return;
        }
        let agora = crate::agora_ms();
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        let chave = format!("falha:{}", job.nome.to_lowercase());
        let mandar = {
            let Ok(mut avisados) = self.avisos_de_jobs.lock() else {
                return;
            };
            // Rodou -- entao parado nao esta.
            avisados.remove(&format!("parado:{}", job.nome.to_lowercase()));
            if corrida.ok {
                avisados.remove(&chave);
                false
            } else {
                crate::jobs::pode_avisar(&mut avisados, &chave, agora, silencio)
            }
        };
        if !mandar {
            return;
        }
        let email = self.config.alertas.email.clone();
        let assunto = format!("PhxSql: job {} falhou", job.nome);
        let corpo = Self::texto_do_aviso_de_falha(job, corrida);
        // Linha de execucao propria: quem dispara pode ser a tela, e ela nao
        // deve esperar o rele -- nem o timeout de um rele fora do ar.
        self.telemetria.subir(
            "aviso-job",
            "entrega UM e-mail de job que falhou e sai; existe em thread \
             propria porque quem dispara pode ser a tela, e ela nao pode \
             esperar o rele -- nem o timeout de um rele fora do ar",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("falando com o rele de e-mail");
                match crate::email::enviar(&email, &assunto, &corpo) {
                    Ok(r) => eprintln!("aviso de job enviado: {r}"),
                    // Falhar em avisar tambem e noticia, como no disco.
                    Err(e) => eprintln!("aviso de job NAO ENVIADO: {e}"),
                }
            },
        );
    }

    /// O corpo do e-mail de falha. Identifica o job, o motivo e a hora -- e
    /// NUNCA carrega credencial: o usuario aparece pelo login, o pedido pela
    /// operacao, e senha nao ha de onde vir.
    fn texto_do_aviso_de_falha(job: &crate::jobs::Job, c: &crate::jobs::Corrida) -> String {
        let mut t = String::new();
        t.push_str(&format!("O job {} falhou.\n\n", job.nome));
        if !job.descricao.is_empty() {
            t.push_str(&format!("  descrição  {}\n", job.descricao));
        }
        t.push_str(&format!(
            "  operação   {}\n  roda como  {}\n  agenda     {}\n  quando     {}\n  duração    {} ms\n\n  erro: {}\n\n",
            c.op,
            if c.usuario.is_empty() { "(sem cadastro)" } else { &c.usuario },
            job.agenda.rotulo(),
            phxsql_core::datahora::instante_iso(c.quando_ms),
            c.duracao_ms,
            c.detalhe
        ));
        t.push_str(
            "Enquanto o job continuar falhando, este aviso se repete no maximo uma vez \
             por janela de silêncio; quando ele voltar a rodar, a próxima falha avisa \
             na hora. O histórico completo está na tela Jobs e no jobs.log.\n\n",
        );
        t.push_str(&format!("Servidor PhxSql {VERSAO}\n"));
        t
    }

    /// Sobe o vigia que avisa por e-mail o job PARADO -- ligado, com a hora
    /// vencida, e sem relogio para roda-lo (ex.: o primeiro job foi ligado
    /// pela tela depois do arranque, e o relogio so sobe no arranque).
    ///
    /// So existe se o aviso foi pedido: desligado nao custa nem a thread.
    /// E dorme ANTES da primeira conferencia, para o arranque terminar de
    /// subir o relogio -- senao todo arranque com job vencido comecaria com
    /// um alarme falso.
    fn ligar_vigia_de_jobs(self: &Arc<Self>) {
        if !self.aviso_de_jobs_ligado() {
            return;
        }
        let email = &self.config.alertas.email;
        eprintln!(
            "aviso de jobs por e-mail: falha e parado | avisa {}",
            email.para.join(", ")
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "vigia-jobs",
            "avisa por e-mail o job PARADO: ligado, com a hora vencida e sem \
             relogio que o rode -- o caso que o proprio relogio nao percebe",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                fio.fazendo("dormindo ate a proxima conferencia");
                std::thread::sleep(Duration::from_secs(crate::jobs::PERIODO_DO_VIGIA_S));
                fio.fazendo("conferindo os jobs parados");
                servidor.conferir_jobs_parados();
            },
        );
    }

    /// Uma rodada do vigia de jobs parados.
    ///
    /// O predicado e o MESMO da tela (`jobs::job_parado`), para os dois nunca
    /// discordarem. O silencio e a limpeza espelham o vigia de disco: quem
    /// deixou de estar parado sai do mapa e volta a ter direito a aviso
    /// imediato.
    fn conferir_jobs_parados(&self) {
        let agora = crate::agora_ms();
        let relogio = self.relogio_de_jobs_no_ar();
        let rodando_agora: Vec<String> = self
            .jobs_rodando
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        // (nome, descricao, agenda, ultima corrida) de cada parado. A copia
        // sai de dentro da trava; o e-mail vai sem ela.
        let parados: Vec<(String, String, String, Option<crate::jobs::Corrida>)> = {
            let Ok(r) = self.jobs.lock() else { return };
            r.jobs
                .iter()
                .filter(|j| {
                    let rodando = rodando_agora
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&j.nome));
                    let vencido = j.agenda.hora_de_rodar(agora, r.ultimo_de(&j.nome));
                    crate::jobs::job_parado(j.ligado, rodando, vencido, relogio)
                })
                .map(|j| {
                    (
                        j.nome.clone(),
                        j.descricao.clone(),
                        j.agenda.rotulo(),
                        r.ultima_corrida_de(&j.nome).cloned(),
                    )
                })
                .collect()
        };
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        let novos: Vec<(String, String, String, Option<crate::jobs::Corrida>)> = {
            let Ok(mut avisados) = self.avisos_de_jobs.lock() else {
                return;
            };
            // Quem deixou de estar parado sai do mapa -- como o disco que
            // aliviou -- para a proxima parada avisar na hora.
            avisados.retain(|chave, _| {
                !chave.starts_with("parado:")
                    || parados
                        .iter()
                        .any(|(n, ..)| chave == &format!("parado:{}", n.to_lowercase()))
            });
            parados
                .into_iter()
                .filter(|(n, ..)| {
                    crate::jobs::pode_avisar(
                        &mut avisados,
                        &format!("parado:{}", n.to_lowercase()),
                        agora,
                        silencio,
                    )
                })
                .collect()
        };
        if novos.is_empty() {
            return;
        }
        for (nome, _, agenda, _) in &novos {
            eprintln!("JOB PARADO: {nome} ({agenda}) -- ligado, vencido e sem relogio");
        }
        let assunto = format!("PhxSql: {} job(s) agendado(s) sem rodar", novos.len());
        let corpo = Self::texto_do_aviso_de_parado(&novos, agora);
        // Este metodo ja roda na thread do vigia: o envio pode ser aqui mesmo.
        match crate::email::enviar(&self.config.alertas.email, &assunto, &corpo) {
            Ok(r) => eprintln!("aviso de job parado enviado: {r}"),
            Err(e) => eprintln!("aviso de job parado NAO ENVIADO: {e}"),
        }
    }

    fn texto_do_aviso_de_parado(
        parados: &[(String, String, String, Option<crate::jobs::Corrida>)],
        agora: i64,
    ) -> String {
        let mut t = String::new();
        t.push_str(
            "Há job agendado que não está rodando: a hora dele venceu e o relógio de \
             jobs não está no ar neste processo.\n\n",
        );
        for (nome, descricao, agenda, ultima) in parados {
            t.push_str(&format!("  {nome}\n"));
            if !descricao.is_empty() {
                t.push_str(&format!("    descrição  {descricao}\n"));
            }
            t.push_str(&format!("    agenda     {agenda}\n"));
            match ultima {
                Some(c) => t.push_str(&format!(
                    "    última     {} -- {}\n",
                    phxsql_core::datahora::instante_iso(c.quando_ms),
                    if c.ok { "ok" } else { "falhou" }
                )),
                None => t.push_str("    última     nunca rodou (que o log saiba)\n"),
            }
            t.push('\n');
        }
        t.push_str(
            "O relógio de jobs só sobe no arranque, e só se já havia job ligado. \
             Reinicie o servidor para a agenda valer -- ou rode o job pela tela \
             Jobs, que funciona sem relógio.\n\n",
        );
        t.push_str(&format!(
            "Servidor PhxSql {VERSAO}\nQuando: {}\n",
            phxsql_core::datahora::instante_iso(agora)
        ));
        t
    }

    // ----------------------------------------------------------- interface web

    /// Sobe a interface web numa linha de execucao propria, se ligada.
    ///
    /// Falhar aqui NAO derruba o servidor: a interface e conforto, os dados
    /// sao o servico. Se a porta da web estiver ocupada, o aviso sai no
    /// terminal e a porta 5000 continua atendendo.
    fn subir_web(self: &Arc<Self>) {
        if !self.config.web.ligado {
            return;
        }
        let endereco = match self.config.web.endereco() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("interface web NAO subiu: {e}");
                return;
            }
        };
        let ouvinte = match TcpListener::bind(endereco) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("interface web NAO subiu em {endereco}: {e}");
                return;
            }
        };
        eprintln!(
            "interface web em http://{endereco} | sessao de {} min",
            self.config.web.sessao_minutos
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "ouvinte-web",
            "aceita as conexoes da interface web e entrega cada pedido a uma \
             thread propria; ela so aceita, nunca atende",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("esperando conexao do navegador");
                for conexao in ouvinte.incoming() {
                    let fluxo = match conexao {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    // Mesma razao da porta de dados: resposta curta, e o Nagle
                    // segurando cada clique da tela por 40 ms.
                    let _ = fluxo.set_nodelay(true);
                    let par = fluxo
                        .peer_addr()
                        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
                    let s = Arc::clone(&servidor);
                    // ACHADO, e ele fica declarado aqui: esta thread nasce SEM
                    // TETO. A porta de dados recusa acima de `conexoes_max`; a
                    // web nao conta nada, entao uma enxurrada de pedidos vira
                    // uma enxurrada de threads. O registro agora ao menos as
                    // MOSTRA -- o teto e decisao de configuracao, e nao deste
                    // agente.
                    s.telemetria.clone().subir(
                        format!("web-{}", par.port()),
                        "atende UM pedido HTTP da interface e sai: o protocolo \
                         aqui e uma resposta por conexao (`Connection: close`)",
                        "web",
                        crate::agora_ms(),
                        move |f| {
                            f.fazendo(&format!("pedido de {par}"));
                            s.atender_http(fluxo, par);
                        },
                    );
                }
            },
        );
    }

    /// Atende um pedido HTTP. Uma resposta por conexao -- `Connection: close`.
    fn atender_http(&self, mut fluxo: TcpStream, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        let agora = crate::agora_ms();
        if let Some(motivo) = self.barrado(&ip, agora) {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip,
                porta_origem: porta,
                op: "web".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(motivo.clone()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });
            let _ = http::erro_json(&mut fluxo, 403, &motivo);
            return;
        }
        if !self.config.ip_permitido(&ip) {
            self.violacao_leve(&ip, "web", "ip fora da lista de permitidos");
            self.anotar(&Acesso {
                quando_ms: agora,
                ip,
                porta_origem: porta,
                op: "web".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });
            let _ = http::erro_json(&mut fluxo, 403, "ip nao autorizado");
            return;
        }

        let pedido = match http::ler_pedido(&fluxo) {
            Some(p) => p,
            None => {
                let _ = http::erro_json(&mut fluxo, 400, "pedido HTTP invalido ou grande demais");
                return;
            }
        };

        match (pedido.metodo.as_str(), pedido.caminho.as_str()) {
            ("GET", "/") | ("GET", "/index.html") => {
                let _ = http::responder(
                    &mut fluxo,
                    200,
                    "text/html; charset=utf-8",
                    &http::montar_pagina(),
                );
            }
            // Sem token de proposito: e so o sinal de vida que a pagina usa
            // para saber se ha servidor desta origem. Nao conta tentativa e
            // nao diz nada sobre os dados.
            ("GET", "/saude") => {
                // Diz o que a pagina precisa para montar o formulario: a porta
                // que este servidor REALMENTE escuta (nao a de fabrica), os
                // servidores que ela pode alcancar e se ha chave a informar.
                // Nada aqui e segredo, e nada aqui depende de token.
                let _ = http::responder_json(
                    &mut fluxo,
                    200,
                    &Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("phxsql", Json::texto_de(VERSAO)),
                        // A porta que ele REALMENTE escuta agora, e nao a do
                        // arquivo: depois de uma troca pela tela, o formulario
                        // de entrada mandaria todo mundo para a porta velha.
                        (
                            "porta_dados",
                            Json::de_u64(
                                self.endereco_dos_dados
                                    .lock()
                                    .ok()
                                    .and_then(|e| *e)
                                    .map(|e| e.port())
                                    .or_else(|| self.config.endereco().ok().map(|e| e.port()))
                                    .unwrap_or(0) as u64,
                            ),
                        ),
                        (
                            "porta_dados_no_ar",
                            Json::Bool(self.porta_no_ar.load(Ordering::SeqCst)),
                        ),
                        (
                            "servidores",
                            Json::Lista(
                                self.config
                                    .web
                                    .servidores
                                    .iter()
                                    .map(Json::texto_de)
                                    .collect(),
                            ),
                        ),
                        (
                            "exige_chave",
                            Json::Bool(self.config.cadastro.alguem_exige_chave()),
                        ),
                    ]),
                );
            }
            ("POST", "/api") => self.api_http(&mut fluxo, &pedido, &ip, porta),
            ("GET", _) | ("HEAD", _) => {
                let _ = http::erro_json(
                    &mut fluxo,
                    404,
                    "esta interface tem tres rotas: /, /saude e /api",
                );
            }
            _ => {
                let _ = http::erro_json(&mut fluxo, 405, "use GET / ou POST /api");
            }
        }
    }

    /// Abre uma conexao para outro PhxSql e manda o login por ela.
    ///
    /// A politica DESTE servidor vale antes de qualquer coisa sair daqui:
    /// comando proibido aqui nao vira pedido la. A interface nao e uma porta
    /// dos fundos para o que a porta da frente recusa.
    #[allow(clippy::type_complexity)]
    fn abrir_remoto(
        &self,
        destino: &str,
        linha: &str,
        ip: &str,
    ) -> std::result::Result<(String, Json, Arc<Mutex<Remoto>>), (String, PhxError)> {
        let op = Json::analisar(linha)
            .map(|j| j.texto_ou("op", "login").to_string())
            .unwrap_or_else(|_| "login".into());

        if !self.config.web.alcanca_outro_servidor() {
            return Err((
                op,
                PhxError::Autorizacao(
                    "esta interface nao fala com outro servidor: preencha web.servidores no config.json".into(),
                ),
            ));
        }
        if !self.config.web.servidor_permitido(destino) {
            // Endereco fora da lista e sondagem de rede, nao engano: alguem
            // esta procurando o que mais existe do outro lado.
            self.violacao_grave(ip, &op, "servidor fora de web.servidores");
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino} nao esta em web.servidores; o IP foi bloqueado"
                )),
            ));
        }
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            let erro = PhxError::Autorizacao(format!("operacao {op} esta proibida neste servidor"));
            return Err((op, erro));
        }

        let mut remoto =
            Remoto::abrir(destino, self.config.timeout_s).map_err(|e| (op.clone(), e))?;
        let resposta = remoto.conversar(linha).map_err(|e| (op.clone(), e))?;
        if !resposta.booleano_ou("ok", false) {
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino}: {}",
                    resposta.texto_ou("erro", "recusou o login")
                )),
            ));
        }
        let valor = resposta.campo("resultado").cloned().unwrap_or(Json::Nulo);
        Ok((op, valor, Arc::new(Mutex::new(remoto))))
    }

    /// Manda o pedido para o servidor remoto desta sessao.
    fn encaminhar(
        &self,
        conexao: &Arc<Mutex<Remoto>>,
        linha: &str,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let op = match Json::analisar(linha) {
            Ok(j) => {
                let o = j.texto_ou("op", "ping").trim().to_string();
                if o.is_empty() {
                    "ping".to_string()
                } else {
                    o
                }
            }
            Err(e) => return ("?".into(), false, Err(e)),
        };
        // A politica local vale para o que passa por aqui, mesmo indo embora.
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            return (
                op.clone(),
                false,
                Err(PhxError::Autorizacao(format!(
                    "operacao {op} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }
        let mut r = match conexao.lock() {
            Ok(r) => r,
            Err(_) => return (op, false, Err(trava_envenenada())),
        };
        match r.conversar(linha) {
            Ok(resposta) => {
                if resposta.booleano_ou("ok", false) {
                    (
                        op,
                        true,
                        Ok(resposta.campo("resultado").cloned().unwrap_or(Json::Nulo)),
                    )
                } else {
                    let erro = resposta
                        .texto_ou("erro", "o servidor remoto recusou")
                        .to_string();
                    (op, true, Err(PhxError::Autorizacao(erro)))
                }
            }
            Err(e) => (op, true, Err(e)),
        }
    }

    /// O `/api`: o mesmo protocolo da porta 5000, um pedido por vez.
    ///
    /// A diferenca esta na identidade. Em TCP a conexao lembra quem entrou; em
    /// HTTP nao ha conexao que dure, entao a memoria e a sessao: o `login`
    /// devolve um identificador, o navegador o repete no cabecalho `X-Sessao`,
    /// e o PBKDF2 de 210.000 iteracoes roda uma vez por login em vez de uma
    /// vez por clique.
    fn api_http(&self, fluxo: &mut TcpStream, pedido: &http::Pedido, ip: &str, porta: u16) {
        let duracao = self.config.web.sessao_ms();
        let agora = crate::agora_ms();
        let id_pedido = pedido
            .cabecalho("x-sessao")
            .unwrap_or("")
            .trim()
            .to_string();

        // Reconstroi, a partir da sessao, o mesmo estado que a conexao TCP
        // teria: quem esta logado e que desafio esta em aberto.
        let mut sessao = Sessao::default();
        let mut id_sessao = String::new();
        if !id_pedido.is_empty() {
            if let Ok(mut vivas) = self.sessoes.lock() {
                if let Some(login) = vivas.usar(&id_pedido, duracao, agora) {
                    id_sessao = id_pedido.clone();
                    sessao.desafio = vivas.tomar_desafio(&id_pedido);
                    if !login.is_empty() {
                        sessao.usuario = self
                            .config
                            .cadastro
                            .por_login(&login)
                            .filter(|u| u.ativo)
                            .cloned();
                    }
                }
            }
        }

        // Abrir conexao para outro PhxSql, se o login pediu um servidor.
        //
        // O campo se chama "servidor" e nao "destino" porque "destino" ja e o
        // diretorio do backup -- e a colisao de nome mandava todo pedido de
        // backup para o relay. Achado ligando a peca, nao lendo o codigo.
        let servidor_remoto = Json::analisar(&pedido.corpo)
            .ok()
            .map(|j| j.texto_ou("servidor", "").trim().to_string())
            .unwrap_or_default();

        let inicio = Instant::now();
        let quando_ms = crate::agora_ms();

        // A atividade da web e da SESSAO, e nao do pedido: HTTP abre uma
        // conexao por clique, e uma bolha por conexao viraria um enxame que
        // nasce e morre a cada volta da tela. Sem sessao -- o login e o
        // `saude` --, a chave e o IP, que e o dono que existe naquele
        // instante.
        let chave_da_atividade = if id_sessao.is_empty() {
            format!("web:{ip}")
        } else {
            format!("web:{id_sessao}")
        };
        let atividade = self
            .telemetria
            .entrar(&chave_da_atividade, "web", ip, 0, quando_ms);
        let _amarrada = crate::telemetria::amarrar(atividade.clone());
        if let Some(a) = &atividade {
            let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
            let nome_op = Json::analisar(&pedido.corpo)
                .ok()
                .map(|j| j.texto_ou("op", "?").to_string())
                .unwrap_or_else(|| "?".into());
            a.comecou_pedido(
                &nome_op,
                sessao.login(),
                &alvo.database,
                &alvo.tabela,
                quando_ms,
            );
        }

        let ja_remota = self
            .remotos
            .lock()
            .ok()
            .and_then(|r| r.get(&id_sessao).cloned());

        let (op, autenticado, resultado) = match (&ja_remota, servidor_remoto.is_empty()) {
            // Sessao ja amarrada a um servidor remoto: tudo vai para la.
            (Some(conexao), _) => self.encaminhar(conexao, &pedido.corpo, ip),
            // Login novo pedindo servidor: abre, encaminha, e guarda se entrou.
            (None, false) => {
                let r = self.abrir_remoto(&servidor_remoto, &pedido.corpo, ip);
                match r {
                    Ok((op, valor, conexao)) => {
                        if id_sessao.is_empty() {
                            if let Ok(mut vivas) = self.sessoes.lock() {
                                id_sessao = vivas.nova("", duracao, agora);
                            }
                        }
                        if let Ok(mut r) = self.remotos.lock() {
                            r.insert(id_sessao.clone(), conexao);
                        }
                        (op, true, Ok(valor))
                    }
                    Err((op, e)) => (op, false, Err(e)),
                }
            }
            (None, true) => {
                // O PROFILER olha aqui tambem. A porta da interface e HTTP e
                // nao JSON por linha, mas o pedido e o mesmo objeto e chega
                // pelo mesmo TCP -- deixar a web de fora faria o profiler
                // mentir por omissao justamente para quem esta olhando por
                // ela.
                let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                    let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
                    let nome_op = Json::analisar(&pedido.corpo)
                        .ok()
                        .map(|j| j.texto_ou("op", "?").to_string())
                        .unwrap_or_else(|| "?".into());
                    self.profiler.lock().ok().and_then(|mut pr| {
                        pr.chegou(
                            &pedido.corpo,
                            &nome_op,
                            sessao.login(),
                            &alvo.database,
                            &alvo.tabela,
                            ip,
                            agora,
                        )
                    })
                } else {
                    None
                };
                let saida = self.despachar(&pedido.corpo, &mut sessao, ip);
                if let Some(serial) = marca {
                    if let Ok(mut pr) = self.profiler.lock() {
                        pr.terminou(
                            serial,
                            inicio.elapsed().as_millis() as u64,
                            saida.2.is_ok(),
                            &saida
                                .2
                                .as_ref()
                                .err()
                                .map(|e| e.to_string())
                                .unwrap_or_default(),
                        );
                    }
                }
                saida
            }
        };
        let remota = ja_remota.is_some() || !servidor_remoto.is_empty();
        let ms = inicio.elapsed().as_millis() as u64;
        if let Some(a) = &atividade {
            a.terminou_pedido(sessao.login());
        }
        self.telemetria
            .contar_pedido(OPS_ESCRITA.contains(&op.as_str()), resultado.is_ok());

        // Um desafio em aberto so e consumido por um login. Qualquer outra
        // operacao no meio do caminho devolve o nonce para a sessao, senao um
        // "ping" entre o desafio e o login derrubaria a prova.
        if op != "login" && op != "desafio" {
            if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone()) {
                vivas.guardar_desafio(&id_sessao, d);
            }
        }

        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() && !remota {
            match op.as_str() {
                "desafio" => {
                    if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone())
                    {
                        // O desafio vem antes da identidade: a sessao nasce
                        // anonima so para carregar o nonce ate o login.
                        if id_sessao.is_empty() {
                            id_sessao = vivas.nova("", duracao, agora);
                        }
                        vivas.guardar_desafio(&id_sessao, d);
                    }
                }
                "login" => {
                    if let Ok(mut vivas) = self.sessoes.lock() {
                        let login = sessao.login().to_string();
                        if id_sessao.is_empty() || !vivas.definir_login(&id_sessao, &login) {
                            id_sessao = vivas.nova(&login, duracao, agora);
                        }
                    }
                }
                "sair" => {
                    if let Ok(mut vivas) = self.sessoes.lock() {
                        vivas.encerrar(&id_sessao);
                    }
                    if let Ok(mut r) = self.remotos.lock() {
                        r.remove(&id_sessao);
                    }
                    id_sessao.clear();
                }
                _ => {}
            }
        }

        let mut campos = match &resultado {
            Ok(valor) => vec![
                ("ok", Json::Bool(true)),
                ("op", Json::texto_de(&op)),
                ("resultado", valor.clone()),
                ("ms", Json::de_u64(ms)),
            ],
            // O codigo vem JUNTO com o texto, e nao no lugar dele: o texto e
            // para quem le, o codigo e para quem programa. Trocar um pelo
            // outro obrigaria alguem a perder.
            Err(e) => vec![
                ("ok", Json::Bool(false)),
                ("op", Json::texto_de(&op)),
                ("erro", Json::texto_de(e.to_string())),
                ("codigo", Json::de_u64(e.codigo() as u64)),
                ("nome", Json::texto_de(e.nome())),
                ("classe", Json::texto_de(e.classe())),
                ("repetir", Json::Bool(e.adianta_repetir())),
                ("ms", Json::de_u64(ms)),
            ],
        };
        if !id_sessao.is_empty() {
            campos.push(("sessao", Json::texto_de(&id_sessao)));
        }

        self.anotar(&Acesso {
            quando_ms,
            ip: ip.to_string(),
            porta_origem: porta,
            op: op.clone(),
            usuario: sessao.login().to_string(),
            autenticado,
            ok: resultado.is_ok(),
            duracao_ms: ms,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            // O objeto sai do proprio pedido: e o unico ponto que ve os dois
            // -- a operacao e sobre o que ela foi.
            ..objeto_do_pedido(&pedido.corpo, &resultado)
        });

        if remota && op == "sair" {
            if let Ok(mut r) = self.remotos.lock() {
                r.remove(&id_sessao);
            }
            if let Ok(mut vivas) = self.sessoes.lock() {
                vivas.encerrar(&id_sessao);
            }
            id_sessao.clear();
        }

        let codigo = match &resultado {
            Ok(_) => 200,
            Err(PhxError::Autorizacao(_)) => 403,
            Err(PhxError::NaoEncontrado(_)) => 404,
            Err(_) => 400,
        };
        let _ = http::responder_json(fluxo, codigo, &Json::objeto(campos));
    }

    fn atender(&self, fluxo: TcpStream, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        // Antes de qualquer coisa: quem esta na lista de bloqueio nao entra.
        let agora = crate::agora_ms();
        if let Some(motivo) = self.barrado(&ip, agora) {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.clone(),
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(motivo.clone()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });
            let escrita = fluxo.try_clone();
            if let Ok(mut saida) = escrita {
                let _ = writeln!(
                    saida,
                    "{}",
                    resposta_erro("conexao", &PhxError::Autorizacao(motivo.clone()), 0).escrever()
                );
            }
            return;
        }

        let permitido = self.config.ip_permitido(&ip);
        let escrita = fluxo.try_clone();
        // O soquete vai para o registro para que `encerrar_sessao` consiga
        // fecha-lo de fora: a thread desta conexao passa a vida parada dentro
        // de um `read_line`, e so um `shutdown` a acorda.
        let para_fechar = fluxo.try_clone().ok().map(Arc::new);
        let mut leitor = BufReader::new(fluxo);
        let mut saida = match escrita {
            Ok(f) => f,
            Err(_) => return,
        };

        if !permitido {
            self.violacao_leve(&ip, "conexao", "ip fora da lista de permitidos");
            self.anotar(&Acesso {
                quando_ms: crate::agora_ms(),
                ip,
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });
            let _ = writeln!(
                saida,
                "{}",
                resposta_erro(
                    "conexao",
                    &PhxError::Autorizacao("ip nao autorizado".into()),
                    0
                )
                .escrever()
            );
            return;
        }

        let (id_ligacao, morrer) = match self.ligacoes.lock() {
            Ok(mut l) => l.entrar(&ip, porta, crate::agora_ms(), para_fechar),
            Err(_) => (0, Arc::new(std::sync::atomic::AtomicBool::new(false))),
        };
        let mut sessao = Sessao {
            ligacao: id_ligacao,
            ..Sessao::default()
        };
        // Sai do registro por qualquer caminho -- inclusive os `return` do
        // meio do laco. Sem isto, uma conexao caida ficaria na lista para
        // sempre, e a lista que existe para dizer a verdade passaria a mentir.
        // A chave da bolha e da CONEXAO, e nao do pedido: uma bolha que
        // trocasse de identificador a cada pedido seria redesenhada duas
        // vezes por segundo e ninguem conseguiria clicar nela.
        let chave_da_atividade = format!("dados:{id_ligacao}");
        let _saida_do_registro = AoSair(|| {
            if let Ok(mut l) = self.ligacoes.lock() {
                l.sair(id_ligacao);
            }
            self.telemetria.sair(&chave_da_atividade);
            // A PRIMEIRA rede de protecao da reserva de carga: a conexao caiu,
            // a tabela solta. Sem isto, um cliente morto no meio de uma carga
            // deixaria a tabela reservada ate o prazo vencer -- e o prazo e
            // medido em dezenas de minutos, de proposito.
            self.soltar_cargas_da_ligacao(id_ligacao);
        });

        let mut linha = String::new();
        loop {
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) => return, // conexao fechada
                Ok(_) => {}
                Err(_) => return,
            }
            // Conferido AQUI, e nao so no `shutdown`: se o pedido chegou junto
            // com o encerramento, quem mandou encerrar ganha.
            if morrer.load(Ordering::SeqCst) {
                return;
            }
            if linha.trim().is_empty() {
                continue;
            }

            let inicio = Instant::now();
            let quando_ms = crate::agora_ms();
            // A atividade e aberta a cada pedido, e nao uma vez por conexao:
            // quem liga a telemetria no meio do expediente precisa ver as
            // conexoes que JA estavam abertas. `entrar` devolve a mesma
            // atividade quando a chave ja existe, entao a bolha nao pisca.
            let atividade =
                self.telemetria
                    .entrar(&chave_da_atividade, "dados", &ip, id_ligacao, quando_ms);
            // Amarra a atividade a ESTA thread: e por ela que os lacos longos
            // acham a marca de encerramento, la no fundo, sem carregar a
            // telemetria por parametro em dezenas de assinaturas.
            let _amarrada = crate::telemetria::amarrar(atividade.clone());
            {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));
                let nome_op = Json::analisar(&linha)
                    .ok()
                    .map(|j| j.texto_ou("op", "?").to_string())
                    .unwrap_or_else(|| "?".into());
                if let Some(a) = &atividade {
                    a.comecou_pedido(
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        quando_ms,
                    );
                }
                if let Ok(mut l) = self.ligacoes.lock() {
                    l.comecou(
                        id_ligacao,
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        quando_ms,
                    );
                }
            }
            // O PROFILER olha AQUI: o pedido chegou pelo soquete e nada foi
            // gravado ainda. Se a operacao travar, ele ja apareceu na tela
            // como «em curso» -- que e justamente o pedido que se quer achar.
            // Desligado, nao custa NADA: nem parse, nem alocacao, nem trava.
            let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));
                let nome_op = Json::analisar(&linha)
                    .ok()
                    .map(|j| j.texto_ou("op", "?").to_string())
                    .unwrap_or_else(|| "?".into());
                self.profiler.lock().ok().and_then(|mut p| {
                    p.chegou(
                        &linha,
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        &ip,
                        quando_ms,
                    )
                })
            } else {
                None
            };

            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;
            if let Some(serial) = marca {
                if let Ok(mut p) = self.profiler.lock() {
                    p.terminou(
                        serial,
                        duracao,
                        resultado.is_ok(),
                        &resultado
                            .as_ref()
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_default(),
                    );
                }
            }
            // O login so se sabe DEPOIS: o pedido que autentica e o proprio
            // `login`, e antes dele a sessao ainda esta anonima.
            if let Ok(mut l) = self.ligacoes.lock() {
                l.terminou(id_ligacao, sessao.login());
            }
            if let Some(a) = &atividade {
                a.terminou_pedido(sessao.login());
            }
            self.telemetria
                .contar_pedido(OPS_ESCRITA.contains(&op.as_str()), resultado.is_ok());

            let resposta = match &resultado {
                Ok(valor) => Json::objeto(vec![
                    ("ok", Json::Bool(true)),
                    ("op", Json::texto_de(&op)),
                    ("resultado", valor.clone()),
                    ("ms", Json::de_u64(duracao)),
                ]),
                Err(e) => resposta_erro(&op, e, duracao),
            };

            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                usuario: sessao.login().to_string(),
                autenticado,
                ok: resultado.is_ok(),
                duracao_ms: duracao,
                erro: resultado.as_ref().err().map(|e| e.to_string()),
                // O objeto do pedido, para o log poder somar por tabela.
                ..objeto_do_pedido(&linha, &resultado)
            });

            if writeln!(saida, "{}", resposta.escrever()).is_err() {
                return;
            }
            let _ = saida.flush();
        }
    }

    /// Le o pedido e o leva pelos portoes, nesta ordem: politica (o que ninguem
    /// pode), token (a rede), login (a identidade) e permissao (o poder).
    fn despachar(
        &self,
        linha: &str,
        sessao: &mut Sessao,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() {
            "ping".to_string()
        } else {
            op
        };
        let base = pedido.texto_ou("database", "").to_string();

        // Portao 0 -- a politica. Vale para todo mundo, root inclusive: e o
        // que o config.json diz que ninguem pede por esta porta. Pedir vira
        // bloqueio na hora, sem contar tentativa.
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            return (
                op.clone(),
                false,
                Err(PhxError::Autorizacao(format!(
                    "operacao {op} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }
        // Nome com ".." ou barra nao e engano de digitacao: e sondagem de
        // travessia de diretorio. O motor ja recusava -- mas recusava calado, e
        // quem sonda podia tentar a noite inteira sem nunca ser barrado. Agora
        // e violacao grave, igual a comando proibido: bloqueia na primeira.
        for (rotulo, valor) in [
            ("database", &base),
            ("tabela", &pedido.texto_ou("tabela", "").to_string()),
            ("schema", &pedido.texto_ou("schema", "").to_string()),
        ] {
            if !valor.is_empty() && phxsql_store::catalogo::nome_hostil(valor) {
                self.violacao_grave(ip, &op, "tentativa de travessia de diretorio");
                return (
                    op,
                    false,
                    Err(PhxError::Autorizacao(format!(
                        "{rotulo} {valor:?} nao e um nome; o IP foi bloqueado"
                    ))),
                );
            }
        }

        if self.config.politica.base_proibida(&base) {
            self.violacao_grave(ip, &op, "base proibida pela politica");
            return (
                op,
                false,
                Err(PhxError::Autorizacao(format!(
                    "a base {base} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            self.violacao_leve(ip, &op, "token invalido");
            return (
                op,
                false,
                Err(PhxError::Autorizacao("token invalido".into())),
            );
        }

        // Portao 2 -- o login.
        if op == "desafio" {
            let r = self.op_desafio(&pedido, sessao);
            return (op, true, r);
        }
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            if r.is_err() {
                self.violacao_leve(ip, "login", "credencial invalida");
            }
            return (op, r.is_ok(), r);
        }
        // Sair nao precisa de poder nenhum: e devolver o que se tem.
        if op == "sair" {
            sessao.usuario = None;
            sessao.desafio = None;
            return (op, true, Ok(Json::objeto(vec![("saiu", Json::Bool(true))])));
        }
        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()
            && Atividade::da_operacao(&op).is_some()
        {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}".into(),
                )),
            );
        }

        // Portoes 2b, 3 e 4 -- ver `portoes_do_pedido`.
        if let Err(e) = self.portoes_do_pedido(&op, &pedido, sessao) {
            return (op, true, Err(e));
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }

    /// Os portoes que valem para QUALQUER origem, e nao so para a rede.
    ///
    /// Estao juntos aqui porque o portao tem de ser UM. O agendador de jobs
    /// nao chega por soquete -- nao passa pelo token nem pela lista negra --,
    /// mas o somente-leitura, o poder do usuario sobre a base E A TABELA e a
    /// reserva de carga valem para ele igual. Escrever essa conferencia num
    /// segundo lugar e exatamente como a porta dos fundos aparece: a copia que
    /// alguem esquecer de atualizar vira o furo, e ninguem acha por leitura.
    ///
    /// O que NAO esta aqui e o que so faz sentido com um IP do outro lado: a
    /// politica de comando proibido bloqueia quem pediu, e bloquear "o
    /// agendador" nao quer dizer nada. Quem chama de outra origem confere a
    /// politica por conta, e o comentario de `rodar_job` diz como.
    fn portoes_do_pedido(&self, op: &str, pedido: &Json, sessao: &Sessao) -> Result<()> {
        // Portao 2b -- a escrita. Com cluster, quem decide e o papel VIVO: a
        // replica redireciona para o master (`REDIRECIONA host:porta`), e um
        // master sem maioria visivel recusa, para conter o split-brain. O
        // `somente_leitura` do config -- que toda replica de cluster liga --
        // deixa de valer no no PROMOVIDO, senao a promocao nao promoveria
        // nada. Sem o bloco `cluster`, a regra e a de sempre.
        if OPS_ESCRITA.contains(&op) {
            if let Some(estado) = &self.cluster {
                if let Some(recusa) = estado.recusa_de_escrita() {
                    return Err(recusa);
                }
            } else if self.config.somente_leitura {
                return Err(PhxError::Autorizacao(
                    "servidor em modo somente leitura".into(),
                ));
            }
        }

        // Portao 3 -- o poder deste usuario sobre a base E A TABELA do pedido.
        //
        // A tabela entra aqui, e nao la dentro de cada operacao, porque o
        // portao tem de ser UM: espalhado por quarenta operacoes, a que
        // alguem esquecer de conferir vira a porta dos fundos, e ninguem
        // descobre por leitura.
        //
        // Pedido sem tabela -- `bancos`, `criar_database`, `sistema` -- cai na
        // regra da base, que e como sempre foi.
        let base = pedido.texto_ou("database", "").to_string();
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(op), sessao.usuario.as_ref())
        {
            let tabela = pedido.texto_ou("tabela", "").trim().to_string();
            if !usuario.pode_em(&base, &tabela, atividade) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de {} em {}",
                    usuario.login,
                    atividade.nome(),
                    match (base.is_empty(), tabela.is_empty()) {
                        (true, _) => "(sem base)".to_string(),
                        (false, true) => base.clone(),
                        (false, false) => format!("{base}.{tabela}"),
                    }
                )));
            }
        }

        // Portao 4 -- a tabela esta reservada para uma carga de outra ligacao?
        //
        // Depois do de permissao, e nao antes: quem nao pode nem ler a tabela
        // nao precisa descobrir que ela esta em carga, e o recado diz QUEM
        // reservou. `bulkinsert` fica de fora para o comando dizer o proprio
        // recado -- e para o administrador conseguir soltar a reserva alheia.
        if op != "bulkinsert" {
            let (db, tab) = (
                pedido.texto_ou("database", ""),
                pedido.texto_ou("tabela", ""),
            );
            if let Some(recado) = self.barrado_por_carga(db, tab, sessao.ligacao) {
                return Err(PhxError::EmCarga(recado));
            }
        }
        Ok(())
    }

    /// Abre um desafio: devolve sal, iteracoes e um nonce de uso unico.
    ///
    /// Usuario que nao existe recebe um desafio de aparencia normal, com sal
    /// derivado do proprio login -- assim quem sonda nao descobre quem existe
    /// pela resposta.
    fn op_desafio(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\"".into()));
        }
        let (sal_hex, iteracoes) = match self.config.cadastro.por_login(&login) {
            Some(u) => {
                let (sal, it) = phxsql_core::senha::sal_e_iteracoes(&u.senha_hash)?;
                (phxsql_core::hash::para_hex(&sal), it)
            }
            None => {
                // Sal falso, estavel por login e indistinguivel de um real.
                let falso =
                    phxsql_core::hash::hmac_sha256(self.config.token.as_bytes(), login.as_bytes());
                (
                    phxsql_core::hash::para_hex(&falso[..16]),
                    phxsql_core::senha::ITERACOES_PADRAO,
                )
            }
        };

        let nonce = phxsql_core::desafio::nonce();
        sessao.desafio = Some((
            login,
            nonce.clone(),
            crate::agora_ms() + phxsql_core::desafio::VALIDADE_MS,
        ));
        Ok(Json::objeto(vec![
            ("sal", Json::texto_de(sal_hex)),
            ("iteracoes", Json::de_u64(iteracoes as u64)),
            ("nonce", Json::texto_de(nonce)),
            (
                "validade_ms",
                Json::de_i64(phxsql_core::desafio::VALIDADE_MS),
            ),
        ]))
    }

    /// Confere a credencial e guarda a identidade na conexao.
    ///
    /// Aceita tres formas, da mais segura para a menos:
    ///
    /// 1. `prova` + `nonce_cliente` -- desafio-resposta. A senha nao sai da
    ///    maquina do cliente.
    /// 2. `senha_b64` -- Base64. Some do grep e do olho, mas quem captura o
    ///    pacote decodifica: NAO e cifra.
    /// 3. `senha` -- texto puro.
    fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = match p.campo("usuario_b64").and_then(Json::texto) {
            Some(b) => phxsql_core::base64::decodificar_texto(b)?,
            None => p.texto_ou("usuario", p.texto_ou("login", "")).to_string(),
        };
        let login = login.trim().to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\" e \"senha\"".into()));
        }

        // Todo caminho de erro devolve a MESMA mensagem, para nao dizer se o
        // que falhou foi o login, a senha ou o desafio.
        let recusa = || PhxError::Autorizacao("usuario ou senha invalidos".into());

        let mut nonces: Option<(String, String)> = None;
        let autenticado = if let Some(prova) = p.campo("prova").and_then(Json::texto) {
            // (1) desafio-resposta
            let (usuario_desafio, nonce, expira) = sessao.desafio.take().ok_or_else(|| {
                PhxError::Autorizacao("peca um desafio antes de mandar a prova".into())
            })?;
            if crate::agora_ms() > expira {
                return Err(PhxError::Autorizacao(
                    "o desafio expirou; peca outro".into(),
                ));
            }
            if usuario_desafio != login {
                return Err(recusa());
            }
            let nonce_cliente = p.texto_ou("nonce_cliente", "");
            nonces = Some((nonce.clone(), nonce_cliente.to_string()));
            match self.config.cadastro.por_login(&login) {
                Some(u) if u.ativo => {
                    let dk = phxsql_core::senha::derivado_do_hash(&u.senha_hash)?;
                    phxsql_core::desafio::conferir_prova(&dk, &nonce, nonce_cliente, &login, prova)
                        .then_some(u)
                }
                _ => None,
            }
        } else {
            // (2) Base64 ou (3) texto puro
            let clara = match p.campo("senha_b64").and_then(Json::texto) {
                Some(b) => phxsql_core::base64::decodificar_texto(b)?,
                None => p.texto_ou("senha", "").to_string(),
            };
            self.config.cadastro.autenticar(&login, &clara)
        };

        // Segundo fator: quem tem chave publica no config.json tambem assina.
        //
        // A mensagem assinada e a MESMA do desafio-resposta -- os dois nonces
        // e o login --, entao a assinatura tambem vale uma vez so. Nao ha
        // atalho: sem desafio aberto nao ha o que assinar.
        if let Some(u) = &autenticado {
            if let Some(publica) = &u.chave_publica {
                let (nonce, nonce_cliente) =
                    match &nonces {
                        Some(par) => par.clone(),
                        None => return Err(PhxError::Autorizacao(
                            "este usuario exige chave: peca um desafio e mande a prova assinada"
                                .into(),
                        )),
                    };
                let hex = p.texto_ou("assinatura", "");
                let assinatura = phxsql_core::ed25519::assinatura_de_hex(hex).ok_or_else(|| {
                    PhxError::Autorizacao(
                        "este usuario exige \"assinatura\" com 128 hexadecimais".into(),
                    )
                })?;
                let mensagem =
                    phxsql_core::desafio::mensagem_assinada(&nonce, &nonce_cliente, &login);
                if !phxsql_core::ed25519::conferir(publica, &mensagem, &assinatura) {
                    return Err(recusa());
                }
            }
        }

        match autenticado {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                Err(recusa())
            }
        }
    }

    fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {
        match op {
            "ping" => Ok(Json::objeto(vec![
                ("phxsql", Json::texto_de(VERSAO)),
                // Num cluster o papel que interessa e o VIVO: um no promovido
                // que respondesse o papel do config.json estaria mentindo.
                (
                    "papel",
                    Json::texto_de(match &self.cluster {
                        Some(e) => e.papel().nome(),
                        None => self.config.replicacao.papel.nome(),
                    }),
                ),
                (
                    "conexoes",
                    Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                ),
                (
                    "no_ar_s",
                    Json::de_u64(((crate::agora_ms() - self.desde_ms) / 1_000).max(0) as u64),
                ),
                (
                    "desde",
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
                ),
            ])),
            "config" => Ok(self.config.para_json()),
            "catalogo" => Ok(self.op_catalogo(p, sessao)),
            "sql" => self.op_sql(p, sessao),
            "quem_sou" => Ok(match &sessao.usuario {
                Some(u) => u.ficha(),
                None => Json::objeto(vec![
                    ("usuario", Json::Nulo),
                    ("via", Json::texto_de("token de servico")),
                ]),
            }),
            "usuarios" => Ok(self.config.cadastro.fichas()),
            "acessos" => self.op_acessos(p),
            "ips" => self.op_ips(),
            "bloqueios" => self.op_bloqueios(),
            "desbloquear" => self.op_desbloquear(p),
            "bancos" => self.op_bancos(),
            "tabelas" => self.op_tabelas(p, sessao),
            "bulkinsert" => self.op_bulkinsert(p, sessao),
            "cargas" => self.op_cargas(),
            "esquema" => self.op_esquema(p, sessao),
            "servico" => self.op_servico(),
            "servico_parar" => self.op_servico_parar(),
            "servico_subir" => self.op_servico_subir(p),
            "jobs" | "job_listar" => self.op_jobs(p),
            "job_salvar" => self.op_job_salvar(p),
            "job_excluir" => self.op_job_excluir(p),
            "job_rodar" => self.op_job_rodar(p),
            "job_ligar" => self.op_job_ligar(p),
            "criar_database" => self.op_criar_database(p),
            "criar_schema" => self.op_criar_schema(p),
            "criar_tabela" => self.op_criar_tabela(p),
            "declarar_fk" => self.op_declarar_fk(p, sessao),
            "excluir_fk" => self.op_excluir_fk(p, sessao),
            "excluir_tabela" => self.op_excluir_tabela(p),
            "duplicar_tabela" => self.op_duplicar_tabela(p),
            "copiar_tabela" => self.op_copiar_tabela(p, sessao),
            "sistabelas" | "systables" => self.op_sistabelas(p, sessao),
            "siscolunas" | "syscolumns" => self.op_siscolunas(p, sessao),
            "dados_pessoais" | "lgpd" => self.op_dados_pessoais(p, sessao),
            "sequencias" | "sequences" => self.op_sequencias(p),
            "ajustar_sequencia" => self.op_ajustar_sequencia(p, sessao),
            "pivotar" | "pivot" => self.op_pivotar(p, sessao),
            "juntar" | "join" => self.op_juntar(p, sessao),
            "unir" | "union" => self.op_unir(p, sessao),
            "ler" => self.op_ler(p, sessao),
            "varrer" => self.op_varrer(p, sessao),
            "buscar" => self.op_buscar(p, sessao),
            "inserir" => self.op_inserir(p, sessao),
            "inserir_lote" | "importar" | "carga" => self.op_inserir_lote(p, sessao),
            "importar_conferir" => self.op_importar_conferir(p, sessao),
            "atualizar" => self.op_atualizar(p, sessao),
            "excluir" => self.op_excluir(p, sessao),
            "restaurar" => self.op_restaurar(p, sessao),
            "lixeira" | "trash" => self.op_lixeira(p, sessao),
            "motivos" | "reasons" => self.op_motivos(p, sessao),
            "esvaziar_lixeira" => self.op_esvaziar_lixeira(p, sessao),
            "diario" => self.op_diario(p, sessao),
            "profiler_ligar" => self.op_profiler_ligar(p),
            "profiler_desligar" => self.op_profiler_desligar(),
            "profiler" => self.op_profiler(p),
            "profiler_limpar" => self.op_profiler_limpar(),
            "posicao" => self.op_posicao(p, sessao),
            "replicar" => self.op_replicar(p, sessao),
            "aplicar" => self.op_aplicar(p, sessao),
            "cluster_pulso" => self.op_cluster_pulso(p),
            "cluster_estado" => self.op_cluster_estado(),
            "memoria_carregar" => self.op_memoria_carregar(p, sessao),
            "memoria_liberar" => self.op_memoria_liberar(p),
            "memoria" => self.op_memoria(),
            "painel" => self.op_painel(sessao),
            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),
            "sessoes" | "processlist" => self.op_sessoes(),
            "telemetria" => self.op_telemetria(p, sessao),
            "telemetria_ligar" => self.op_telemetria_ligar(sessao),
            "telemetria_desligar" => self.op_telemetria_desligar(sessao),
            "telemetria_encerrar" => self.op_telemetria_encerrar(p, sessao),
            "encerrar_sessao" | "kill" => self.op_encerrar_sessao(p),
            "checksum" | "soma_de_verificacao" => self.op_checksum(p, sessao),
            "exportar" | "export" => self.op_exportar(p, sessao),
            "sistema" => Ok(self.op_sistema()),
            "dblink" => self.op_dblink(),
            "dblink_salvar" => self.op_dblink_salvar(p),
            "dblink_excluir" => self.op_dblink_excluir(p),
            "dblink_testar" => self.op_dblink_testar(p),
            "dblink_bancos" => self.op_dblink_bancos(p),
            "dblink_tabelas" => self.op_dblink_tabelas(p),
            "dblink_estrutura" => self.op_dblink_estrutura(p),
            "dblink_ler" => self.op_dblink_ler(p),
            "dblink_consultar" => self.op_dblink_consultar(p),
            "dblink_ligar" => self.op_dblink_ligar(p, sessao),
            "dblink_sincronizar" => self.op_dblink_sincronizar(p, sessao),
            "backup" => self.op_backup(p, sessao),
            "reparar" => self.op_reparar(p, sessao),
            "conferir_backup" => self.op_conferir_backup(p),
            // O nome que o Adriano pediu, e o nome em portugues do projeto.
            // Sao a mesma operacao: a interface usa um, o script usa o outro.
            "SelectMemory" | "selectmemory" | "selecionar_memoria" => {
                self.op_selecionar_memoria(p, sessao)
            }
            "verificar" => self.op_verificar(p, sessao),
            "reindexar" => self.op_reindexar(p, sessao),
            outro => Err(PhxError::NaoEncontrado(format!(
                "operacao desconhecida: {outro}"
            ))),
        }
    }

    // ------------------------------------------------------------ ajudantes

    /// Abre a tabela DENTRO de uma trava que quem chamou ja tomou.
    ///
    /// # Por que a trava vem de fora
    ///
    /// Abrir uma tabela LE o cabecalho, e o cabecalho traz `slot_count` e
    /// `proxima_sequencia` -- os dois contadores que decidem onde a proxima
    /// linha vai. Se a trava for tomada e solta aqui, duas operacoes
    /// simultaneas abrem a tabela, cada uma guarda `slot_count = N`, e as duas
    /// gravam no rowid N+1: **uma sobrescreve a outra, em silencio**.
    ///
    /// Era exatamente o que acontecia. A trava tem de cobrir abrir E gravar,
    /// como um bloco so -- por isso ela entra por parametro, e nao e tomada
    /// aqui dentro.
    fn abrir_travada(&self, _dados: &Instancia, p: &Json, sessao: &Sessao) -> Result<Table> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        let mut t = _dados.abrir_database(database)?.abrir_qualificada(tabela)?;
        // O espelho e decisao do servidor, nao da tabela: ligar no config.json
        // vale para tudo que este servidor abrir daqui para a frente.
        if self.config.espelho && !t.tem_espelho() {
            t.espelhar()?;
        }
        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        // A imagem da linha no diario e decisao do servidor, como o espelho:
        // um source grava, um servidor isolado nao paga por ela.
        t.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);
        Ok(t)
    }

    fn rowid(&self, p: &Json) -> Result<u64> {
        p.campo("rowid")
            .and_then(Json::inteiro)
            .filter(|n| *n > 0)
            .map(|n| n as u64)
            .ok_or_else(|| PhxError::Esquema("informe \"rowid\" maior que zero".into()))
    }

    fn limite(&self, p: &Json) -> u64 {
        let pedido = p.inteiro_ou("max", self.config.max_linhas as i64).max(0) as u64;
        if pedido == 0 {
            self.config.max_linhas
        } else {
            pedido.min(self.config.max_linhas)
        }
    }

    // ------------------------------------------------------------ operacoes

    fn op_acessos(&self, p: &Json) -> Result<Json> {
        let max = self.limite(p) as usize;
        let todos = LogAcessos::ler(&self.config.log_acessos)?;
        let total = todos.len();
        let recentes: Vec<Json> = todos
            .iter()
            .rev()
            .take(max)
            .map(|a| a.para_json())
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("acessos", Json::Lista(recentes)),
        ]))
    }

    fn op_ips(&self) -> Result<Json> {
        let resumo = LogAcessos::resumo_por_ip(&self.config.log_acessos)?;
        Ok(Json::Lista(
            resumo
                .iter()
                .map(|r| {
                    Json::objeto(vec![
                        ("ip", Json::texto_de(&r.ip)),
                        ("acessos", Json::de_u64(r.acessos)),
                        ("recusados", Json::de_u64(r.recusados)),
                        ("primeiro", Json::texto_de(r.primeiro())),
                        ("ultimo", Json::texto_de(r.ultimo())),
                    ])
                })
                .collect(),
        ))
    }

    fn op_bloqueios(&self) -> Result<Json> {
        let lista = self.lista_negra.lock().map_err(|_| trava_envenenada())?;
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            (
                "arquivo",
                Json::texto_de(lista.caminho().display().to_string()),
            ),
            (
                "ativos",
                Json::Lista(
                    lista
                        .ativos(agora)
                        .into_iter()
                        .map(|b| b.para_json())
                        .collect(),
                ),
            ),
        ]))
    }

    fn op_desbloquear(&self, p: &Json) -> Result<Json> {
        let ip = p.texto_ou("ip", "").trim().to_string();
        if ip.is_empty() {
            return Err(PhxError::Esquema("informe \"ip\"".into()));
        }
        let mut lista = self.lista_negra.lock().map_err(|_| trava_envenenada())?;
        let tinha = lista.desbloquear(&ip, &self.config.politica)?;
        Ok(Json::objeto(vec![
            ("ip", Json::texto_de(&ip)),
            ("estava_bloqueado", Json::Bool(tinha)),
        ]))
    }

    fn op_bancos(&self) -> Result<Json> {
        let dados = self.travar_dados()?;
        Ok(Json::Lista(
            dados.databases()?.into_iter().map(Json::texto_de).collect(),
        ))
    }

    /// `tabelas`: as tabelas da base, **so as que quem pediu pode ler**.
    ///
    /// Filtrar aqui nao e enfeite. Sem isto, quem perdeu o direito a `folha`
    /// continuaria vendo o nome dela na arvore e so descobriria a recusa ao
    /// clicar -- e o nome de uma tabela ja conta parte da historia. A arvore
    /// mostra o que da para abrir.
    fn op_tabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(nome)?;
        let todas = db.todas_as_tabelas()?;
        let visiveis: Vec<Json> = todas
            .into_iter()
            .filter(|t| self.pode_ver_tabela(sessao, nome, t))
            .map(Json::texto_de)
            .collect();
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(nome)),
            (
                "schemas",
                Json::Lista(db.schemas()?.into_iter().map(Json::texto_de).collect()),
            ),
            ("tabelas", Json::Lista(visiveis)),
        ]))
    }

    /// Quem esta na sessao pode LER esta tabela desta base?
    ///
    /// Sem sessao -- servidor sem cadastro -- e sim: o portao de usuario nao
    /// existe naquele modo, e inventar um aqui negaria tudo.
    fn pode_ver_tabela(&self, sessao: &Sessao, database: &str, tabela: &str) -> bool {
        match &sessao.usuario {
            None => true,
            Some(u) => u.pode_em(database, tabela, Atividade::Ler),
        }
    }

    fn op_criar_database(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.criar_database(nome)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(db.nome())),
            (
                "caminho",
                Json::texto_de(db.caminho().display().to_string()),
            ),
        ]))
    }

    /// Copia uma tabela para outro database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. Sao
    /// duas operacoes e nao uma porque a permissao e a mesma mas o alcance
    /// nao: colar num database em que o usuario nao pode criar tem de recusar
    /// no database de DESTINO, e nao no de origem.
    fn op_copiar_tabela(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let origem_db = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        let destino_db = match p.texto_ou("destino_database", "").trim() {
            "" => origem_db,
            outro => outro,
        };
        let destino = match p.texto_ou("destino", "").trim() {
            "" => tabela,
            outro => outro,
        };
        // O portao geral confere a permissao contra o database do campo
        // `database`, que aqui e a ORIGEM. O destino precisa da sua propria
        // conferencia: sem esta linha, quem pode ler um database e nao pode
        // criar no outro conseguiria escrever onde nao devia.
        if let Some(u) = &sessao.usuario {
            if !u.pode_em(destino_db, destino, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {destino_db}.{destino}"
                )));
            }
        }

        let dados = self.travar_dados()?;
        let origem = dados.abrir_database(origem_db)?;
        let alvo = dados.abrir_database(destino_db)?;
        let copiados = origem.copiar_tabela_para(tabela, &alvo, destino)?;
        Ok(Json::objeto(vec![
            ("origem_database", Json::texto_de(origem_db)),
            ("origem", Json::texto_de(tabela)),
            ("destino_database", Json::texto_de(destino_db)),
            ("destino", Json::texto_de(destino)),
            ("arquivos", Json::de_u64(copiados as u64)),
        ]))
    }

    /// `SysTables`: o catalogo de tabelas como se fosse uma tabela.
    ///
    /// Uma linha por tabela do database, com o que ela pesa. E o mesmo que a
    /// tela de gestao mostra, mas em forma de dado -- para quem quer consultar
    /// o catalogo em vez de olhar para ele.
    fn op_sistabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            // O catalogo e a mesma lista da arvore por outra porta: se ele nao
            // filtrasse, bastaria pedir `sistabelas` para saber tudo sobre a
            // tabela que a arvore esconde -- nome, colunas, quantas linhas.
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let t = match db.abrir_qualificada(&nome) {
                Ok(t) => t,
                // Uma tabela ilegivel nao pode derrubar o catalogo inteiro: ela
                // vira uma linha que diz que esta ilegivel, que e exatamente a
                // informacao que alguem foi procurar ali.
                Err(e) => {
                    linhas.push(Json::objeto(vec![
                        ("tabela", Json::texto_de(&nome)),
                        ("erro", Json::texto_de(e.to_string())),
                    ]));
                    continue;
                }
            };
            let e = t.esquema().clone();
            let pag = e.paginacao();
            linhas.push(Json::objeto(vec![
                ("tabela", Json::texto_de(&nome)),
                (
                    "schema",
                    match nome.split_once('.') {
                        Some((sc, _)) => Json::texto_de(sc),
                        None => Json::texto_de(""),
                    },
                ),
                ("registros", Json::de_u64(t.registros())),
                ("slots", Json::de_u64(t.slots())),
                ("colunas", Json::de_u64(e.colunas().len() as u64)),
                ("indices", Json::de_u64(e.indices().len() as u64)),
                (
                    "chave_primaria",
                    match e.chave_primaria() {
                        None => Json::Nulo,
                        Some(k) => Json::texto_de(&k.nome),
                    },
                ),
                (
                    "chaves_estrangeiras",
                    Json::de_u64(e.chaves_estrangeiras().len() as u64),
                ),
                ("bytes_por_linha", Json::de_u64(e.payload_len() as u64)),
                ("paginada", Json::Bool(pag.ligada())),
                (
                    "particao",
                    Json::texto_de(match pag.modo.periodo() {
                        None if pag.ligada() => "quantidade".to_string(),
                        None => "".to_string(),
                        Some(p) => p.nome().to_string(),
                    }),
                ),
                ("volumes", Json::de_u64(t.fronteiras().len() as u64)),
            ]));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("tabelas", Json::Lista(linhas)),
        ]))
    }

    /// `SysColumns`: uma linha por coluna de todas as tabelas do database.
    ///
    /// Aceita `tabela` para filtrar. E aqui que os metadados novos aparecem
    /// juntos -- id, caption, descricao, mascara e o papel nas chaves --, que e
    /// o que um dicionario de dados precisa mostrar.
    fn op_siscolunas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };
            let e = t.esquema();
            for (i, c) in e.colunas().iter().enumerate() {
                let papel = e.papel_da_coluna(i);
                linhas.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("posicao", Json::de_u64(i as u64 + 1)),
                    ("id", Json::texto_de(c.id.to_string())),
                    ("nome", Json::texto_de(&c.nome)),
                    ("caption", Json::texto_de(&c.caption)),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("mascara", Json::texto_de(&c.mascara)),
                    ("dado_pessoal", Json::texto_de(c.dado_pessoal.nome())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("obrigatoria", Json::Bool(!c.nullable)),
                    ("primaria", Json::Bool(papel.primaria)),
                    ("estrangeira", Json::Bool(papel.estrangeira)),
                    (
                        "composta",
                        Json::Bool(papel.primaria_composta || papel.estrangeira_composta),
                    ),
                    (
                        "nos_indices",
                        Json::Lista(papel.indices.iter().map(Json::texto_de).collect()),
                    ),
                ]));
            }
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("colunas", Json::Lista(linhas)),
        ]))
    }

    /// Onde estao os dados pessoais desta base (LGPD / GDPR).
    ///
    /// ```json
    /// { "op": "dados_pessoais", "database": "loja" }
    /// ```
    ///
    /// # O portao, que aqui e o assunto e nao o detalhe
    ///
    /// Esta operacao **nao tem campo `tabela`**: ela varre a base inteira. O
    /// portao geral do `despachar` confere o campo `"tabela"` do pedido, e um
    /// pedido sem ele cai na regra da BASE -- que e exatamente o furo ja
    /// documentado no `juntar` e no `unir`.
    ///
    /// Por isso a conferencia por tabela vem aqui dentro, com o mesmo
    /// `pode_ver_tabela` do `tabelas` e do `siscolunas`: quem nao pode ler a
    /// tabela nao descobre por este relatorio que ela existe, nem que ela
    /// guarda CPF. Um relatorio de conformidade que vaza o mapa do dado
    /// sensivel para quem nao pode le-lo e a pior versao possivel desta
    /// funcionalidade.
    ///
    /// # O que ele NAO faz
    ///
    /// Nao adivinha. Coluna chamada `cpf` sem marca nao entra no relatorio, e
    /// isso e de proposito: um mapa de conformidade deduzido por nome de campo
    /// da a quem le a sensacao de estar coberto sem estar. O relatorio conta
    /// quantas colunas ficaram SEM classificacao, que e o numero que diz o
    /// tamanho do trabalho que falta.
    fn op_dados_pessoais(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;

        let mut achados = Vec::new();
        let mut tabelas_com = 0u64;
        let mut colunas_vistas = 0u64;
        let mut pessoais = 0u64;
        let mut sensiveis = 0u64;
        let mut sem_esquema = Vec::new();

        for nome in db.todas_as_tabelas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                // Tabela que nao abre nao vira erro do relatorio inteiro: ela
                // vira uma linha dizendo que nao foi conferida. Um relatorio
                // de conformidade que para no primeiro arquivo quebrado nao
                // audita nada.
                sem_esquema.push(Json::texto_de(&nome));
                continue;
            };
            let e = t.esquema();
            let mut da_tabela = Vec::new();
            for (i, c) in e.colunas().iter().enumerate() {
                if phxsql_core::schema::e_coluna_de_sistema(&c.nome) {
                    continue;
                }
                colunas_vistas += 1;
                if !c.dado_pessoal.e_pessoal() {
                    continue;
                }
                match c.dado_pessoal {
                    DadoPessoal::Sensivel => sensiveis += 1,
                    _ => pessoais += 1,
                }
                da_tabela.push(Json::objeto(vec![
                    ("posicao", Json::de_u64(i as u64 + 1)),
                    ("coluna", Json::texto_de(&c.nome)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("grau", Json::texto_de(c.dado_pessoal.nome())),
                    // Coluna pessoal que tambem e chave aparece em indice, e
                    // indice e o caminho por onde o dado sai sem ninguem ler a
                    // linha. Quem audita precisa ver isso junto.
                    (
                        "nos_indices",
                        Json::Lista(
                            e.papel_da_coluna(i)
                                .indices
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                ]));
            }
            if !da_tabela.is_empty() {
                tabelas_com += 1;
                achados.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("registros", Json::de_u64(t.registros())),
                    ("total", Json::de_u64(da_tabela.len() as u64)),
                    ("colunas", Json::Lista(da_tabela)),
                ]));
            }
        }

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("tabelas_com_dado_pessoal", Json::de_u64(tabelas_com)),
            ("colunas_pessoais", Json::de_u64(pessoais)),
            ("colunas_sensiveis", Json::de_u64(sensiveis)),
            // O numero que conta a HISTORIA: quanto ainda nao foi classificado.
            // Sem ele, uma base sem marca nenhuma parece uma base sem dado
            // pessoal -- e sao coisas muito diferentes.
            (
                "colunas_sem_classificacao",
                Json::de_u64(colunas_vistas.saturating_sub(pessoais + sensiveis)),
            ),
            ("colunas_conferidas", Json::de_u64(colunas_vistas)),
            ("tabelas_que_nao_abriram", Json::Lista(sem_esquema)),
            ("achados", Json::Lista(achados)),
        ]))
    }

    /// Monta a tabulacao cruzada de uma tabela, com junção opcional.
    ///
    /// ```json
    /// { "database": "loja", "tabela": "vendas",
    ///   "juntar": [ {"tabela":"clientes", "coluna":"cliente_id", "prefixo":"cliente"} ],
    ///   "linhas": [ {"campo":"cliente.cidade"} ],
    ///   "colunas": [ {"campo":"emissao", "granularidade":"mes"} ],
    ///   "valor": "total", "agregador": "soma", "max": 200000 }
    /// ```
    fn op_pivotar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // O portao geral ja conferiu `ler` contra este database; a linha
        // abaixo existe para o caso de a tabela de fatos estar num schema que
        // o `abrir` recusaria -- e o mesmo caminho das outras leituras.
        let _ = sessao;
        let agregador = Agregador::de_texto(p.texto_ou("agregador", "soma"))?;
        let max = self.limite_pivot(p);

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(p.texto_ou("database", ""))?;
        let mut t = db.abrir_qualificada(p.texto_ou("tabela", ""))?;
        let esquema = t.esquema().clone();

        // As tabelas de consulta entram inteiras na memoria, uma vez. E o hash
        // join: para a forma de dado de um pivot -- muitos fatos, poucas
        // dimensoes -- ele custa uma varredura em vez de uma descida na arvore
        // por linha de fato.
        let mut juncoes: Vec<Juncao> = Vec::new();
        for (i, j) in p
            .campo("juntar")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let nome = j.texto_ou("tabela", "");
            let local = j.texto_ou("coluna", "");
            let prefixo = match j.texto_ou("prefixo", "").trim() {
                "" => nome.rsplit('.').next().unwrap_or(nome).to_string(),
                outro => outro.to_string(),
            };
            let coluna_local = posicao_da_coluna(&esquema, local).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a junção {i} usa a coluna {local:?}, que nao existe em {}",
                    esquema.nome()
                ))
            })?;
            // A chave da junção e a PRIMEIRA coluna da chave primaria da tabela
            // de consulta, ou a coluna nomeada em "chave". Sem chave primaria
            // nao ha por onde ligar, e dizer isso e melhor do que juntar pela
            // primeira coluna e devolver numero errado.
            let mut alvo = db.abrir_qualificada(nome)?;
            let esq_alvo = alvo.esquema().clone();
            let chave = match j.texto_ou("chave", "").trim() {
                "" => esq_alvo
                    .chave_primaria()
                    .and_then(|k| k.colunas.first())
                    .map(|ic| ic.coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a tabela {nome} nao tem chave primaria; diga por qual \
                             coluna juntar no campo \"chave\""
                        ))
                    })?,
                c => posicao_da_coluna(&esq_alvo, c).ok_or_else(|| {
                    PhxError::Esquema(format!("a coluna {c:?} nao existe em {nome}"))
                })?,
            };

            let mut mapa = HashMap::new();
            let rowids = alvo.varrer()?;
            let lidas = rowids.len();
            for (rowid, _) in rowids.into_iter().take(TETO_JUNCAO) {
                if let Some(linha) = alvo.ler(rowid)? {
                    mapa.insert(crate::pivot::rotulo(&linha[chave], 0), linha);
                }
            }
            if lidas > TETO_JUNCAO {
                return Err(PhxError::LimiteExcedido(format!(
                    "a tabela de consulta {nome} tem {lidas} linhas, acima do teto \
                     de {TETO_JUNCAO} para junção. Ela e lida inteira para a memoria; \
                     junte por uma tabela menor"
                )));
            }
            juncoes.push(Juncao {
                prefixo,
                esquema: esq_alvo,
                coluna_local,
                mapa,
                lidas,
            });
        }

        let campos = |chave: &str| -> Result<Vec<Campo>> {
            let mut out = Vec::new();
            for c in p.campo(chave).and_then(Json::lista).unwrap_or(&[]) {
                // Aceita tanto "cidade" quanto {"campo":"cidade","granularidade":"mes"}.
                let (nome, gran) = match c {
                    Json::Texto(t) => (t.as_str(), "exato"),
                    outro => (
                        outro.texto_ou("campo", ""),
                        outro.texto_ou("granularidade", "exato"),
                    ),
                };
                out.push(resolver_campo(nome, &esquema, &juncoes, gran)?);
            }
            Ok(out)
        };
        let linhas = campos("linhas")?;
        let colunas = campos("colunas")?;
        if linhas.is_empty() {
            return Err(PhxError::Esquema(
                "informe ao menos um campo em \"linhas\"".into(),
            ));
        }

        let nome_valor = p.texto_ou("valor", "").trim().to_string();
        let valor = if nome_valor.is_empty() {
            if agregador.precisa_de_valor() {
                return Err(PhxError::Esquema(format!(
                    "o agregador {} precisa de um campo em \"valor\"",
                    agregador.nome()
                )));
            }
            None
        } else {
            Some(resolver_campo(&nome_valor, &esquema, &juncoes, "exato")?)
        };

        let mut it = LinhasDaTabela {
            rowids: t
                .varrer()?
                .into_iter()
                .map(|(r, _)| r)
                .collect::<Vec<_>>()
                .into_iter(),
            tabela: &mut t,
        };
        let r = crate::pivot::cruzar(
            &mut it,
            &esquema,
            &juncoes,
            &linhas,
            &colunas,
            valor.as_ref(),
            agregador,
            max,
        )?;

        let txt = |o: &Option<String>| match o {
            None => Json::Nulo,
            Some(s) => Json::texto_de(s),
        };
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("agregador", Json::texto_de(agregador.nome())),
            (
                "campos_linha",
                Json::Lista(
                    linhas
                        .iter()
                        .map(|c| Json::texto_de(&c.qualificado))
                        .collect(),
                ),
            ),
            (
                "campos_coluna",
                Json::Lista(
                    colunas
                        .iter()
                        .map(|c| Json::texto_de(&c.qualificado))
                        .collect(),
                ),
            ),
            ("valor", Json::texto_de(&nome_valor)),
            (
                "rotulos_linha",
                Json::Lista(r.rotulos_linha.iter().map(Json::texto_de).collect()),
            ),
            (
                "rotulos_coluna",
                Json::Lista(r.rotulos_coluna.iter().map(Json::texto_de).collect()),
            ),
            (
                "celulas",
                Json::Lista(
                    r.celulas
                        .iter()
                        .map(|l| Json::Lista(l.iter().map(&txt).collect()))
                        .collect(),
                ),
            ),
            (
                "total_linha",
                Json::Lista(r.total_linha.iter().map(&txt).collect()),
            ),
            (
                "total_coluna",
                Json::Lista(r.total_coluna.iter().map(&txt).collect()),
            ),
            ("total", txt(&r.total)),
            ("lidas", Json::de_u64(r.lidas)),
            ("consideradas", Json::de_u64(r.consideradas)),
            (
                "juncoes",
                Json::Lista(
                    juncoes
                        .iter()
                        .map(|j| {
                            Json::objeto(vec![
                                ("prefixo", Json::texto_de(&j.prefixo)),
                                ("tabela", Json::texto_de(j.esquema.nome())),
                                ("linhas", Json::de_u64(j.lidas as u64)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// O teto de linhas que o pivot varre. Separado do `max_linhas` porque o
    /// pivot devolve um RESUMO: ler cem mil linhas para devolver uma grade de
    /// vinte por doze e barato, e o teto da resposta nao se aplica.
    fn limite_pivot(&self, p: &Json) -> u64 {
        let pedido = p.inteiro_ou("max", TETO_PIVOT as i64).max(1) as u64;
        pedido.min(TETO_PIVOT)
    }

    /// Fecha a gravacao no disco, se a janela de durabilidade mandar.
    ///
    /// Chamado depois de toda escrita. Em `por_operacao` sincroniza sempre --
    /// e o que o servidor fazia. Em `por_lote` sincroniza quando a janela
    /// fecha, e o `fsync` de uma vale por todas as da janela. Em `sistema`
    /// nunca sincroniza aqui: o `write` ja aconteceu, e o resto e com o
    /// sistema operacional.
    // --- carga -----------------------------------------------------------
    // `BULKINSERT`: a tabela reservada para quem esta carregando, e so para
    // ele. Ver `crate::carga` para o desenho e para as duas redes de protecao
    // contra reserva orfa.
    /// Solta o que esta ligacao reservou, e sincroniza o que ficou por gravar.
    ///
    /// Roda na saida da conexao, por qualquer caminho. O `sincronizar` vai
    /// junto porque durante a reserva a janela de durabilidade fica aberta de
    /// proposito -- soltar sem fechar deixaria a carga inteira dependendo de o
    /// sistema operacional lembrar dela.
    fn soltar_cargas_da_ligacao(&self, ligacao: u64) {
        let soltas = match self.cargas.lock() {
            Ok(mut c) => c.soltar_da_ligacao(ligacao),
            Err(_) => return,
        };
        if soltas.is_empty() {
            return;
        }
        if let Ok(mut sujas) = self.sujas.lock() {
            for r in &soltas {
                sujas.insert(format!("{}/{}", r.database, r.tabela));
            }
        }
        self.descarregar_sujas();
    }

    /// A tabela deste pedido esta reservada por OUTRA ligacao?
    ///
    /// Uma reserva vencida e limpa aqui, que e onde alguem repara nela: um
    /// relogio de fundo so para isso seria uma linha de execucao acordando
    /// para, quase sempre, nao fazer nada.
    fn barrado_por_carga(&self, database: &str, tabela: &str, ligacao: u64) -> Option<String> {
        if database.is_empty() || tabela.is_empty() {
            return None;
        }
        self.cargas
            .lock()
            .ok()?
            .barra(database, tabela, ligacao, crate::agora_ms())
    }

    /// Esta tabela esta reservada para carga?
    ///
    /// Nao pergunta POR QUEM de proposito: quem chegou ate aqui ja passou pelo
    /// portao, que so deixa o dono escrever numa tabela reservada. Entao
    /// «reservada» e «reservada por mim» sao a mesma coisa neste ponto, e
    /// perguntar de novo pediria a sessao em quarenta lugares.
    ///
    /// Enquanto estiver, a janela de durabilidade fica aberta: a carga inteira
    /// vira um `fsync` so, no fim.
    fn tabela_reservada(&self, p: &Json) -> bool {
        let (db, tab) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
        if db.is_empty() || tab.is_empty() {
            return false;
        }
        let k = crate::carga::chave(db, tab);
        match self.cargas.lock() {
            Ok(c) => c
                .todas()
                .iter()
                .any(|r| crate::carga::chave(&r.database, &r.tabela) == k),
            Err(_) => false,
        }
    }

    /// `bulkinsert`: reserva a tabela para uma carga, ou solta.
    ///
    /// So pela porta de dados. Ver o porque em `crate::carga`.
    fn op_bulkinsert(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").trim().to_string();
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        // Aceita `{"ligado":true}` e tambem `{"bulkinsert":true}`, que e como
        // o comando se le quando a camada SQL existir: BULKINSERT(true).
        let ligar = p
            .campo("ligado")
            .or_else(|| p.campo("bulkinsert"))
            .or_else(|| p.campo("valor"))
            .and_then(Json::booleano)
            .ok_or_else(|| {
                PhxError::Esquema(
                    "informe \"ligado\": true para reservar, false para soltar".into(),
                )
            })?;

        if sessao.ligacao == 0 {
            return Err(PhxError::Esquema(
                "BULKINSERT so vale pela porta de dados: HTTP nao tem conexao \
                 para a reserva morrer amarrada. Pela tela, use \"inserir_lote\", \
                 que ja e uma operacao so"
                    .into(),
            ));
        }

        // A tabela tem de existir -- reservar o que nao existe esconderia um
        // erro de digitacao ate o fim da carga.
        {
            let dados = self.travar_dados()?;
            dados
                .abrir_database(&database)?
                .abrir_qualificada(&tabela)?;
        }

        let agora = crate::agora_ms();
        let mut cargas = self.cargas.lock().map_err(|_| trava_envenenada())?;

        if ligar {
            let prazo = self.config.recursos.carga_prazo_min as i64 * 60_000;
            let r = cargas.reservar(
                &database,
                &tabela,
                sessao.login(),
                sessao.ligacao,
                "",
                agora,
                prazo,
            )?;
            drop(cargas);
            return Ok(Json::objeto(vec![
                ("bulkinsert", Json::Bool(true)),
                ("database", Json::texto_de(&database)),
                ("tabela", Json::texto_de(&tabela)),
                ("reservada", Json::Bool(true)),
                (
                    "expira_em_s",
                    Json::de_u64(((r.expira_ms - agora).max(0) / 1000) as u64),
                ),
                (
                    "prazo_min",
                    Json::de_u64(self.config.recursos.carga_prazo_min),
                ),
            ]));
        }

        // Soltar: o dono solta o seu; o administrador solta o de qualquer um.
        let forcar = sessao.usuario.as_ref().map(|u| u.e_admin()).unwrap_or(true);
        let r = cargas.soltar(&database, &tabela, sessao.ligacao, forcar, agora)?;
        drop(cargas);

        // O fsync que a carga inteira adiou acontece agora.
        {
            let _trava = self.travar_dados()?;
            let mut t = self.abrir_travada(&_trava, p, sessao)?;
            t.sincronizar()?;
        }
        if let Ok(mut sujas) = self.sujas.lock() {
            sujas.remove(&format!("{database}/{tabela}"));
        }

        Ok(Json::objeto(vec![
            ("bulkinsert", Json::Bool(false)),
            ("database", Json::texto_de(&database)),
            ("tabela", Json::texto_de(&tabela)),
            ("liberada", Json::Bool(true)),
            // Em milissegundos, e nao em segundos: uma carga de 300 ms
            // aparecia como "durou 0s", que e um numero que nao ajuda ninguem.
            ("durou_ms", Json::de_u64((agora - r.desde_ms).max(0) as u64)),
            ("sincronizada", Json::Bool(true)),
        ]))
    }

    /// `cargas`: quais tabelas estao reservadas agora, e por quem.
    fn op_cargas(&self) -> Result<Json> {
        let agora = crate::agora_ms();
        let c = self.cargas.lock().map_err(|_| trava_envenenada())?;
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(c.quantas() as u64)),
            (
                "cargas",
                Json::Lista(c.todas().iter().map(|r| r.para_json(agora)).collect()),
            ),
        ]))
    }

    fn gravar_de_verdade(&self, t: &mut Table, p: &Json) -> Result<()> {
        let chave = format!(
            "{}/{}",
            p.texto_ou("database", ""),
            p.texto_ou("tabela", "")
        );
        // Durante uma carga a janela NAO fecha: o `BULKINSERT(false)` e quem
        // sincroniza, uma vez, no fim. E o segundo ganho da reserva -- o
        // primeiro e a exclusividade.
        if self.tabela_reservada(p) || !self.janela.hora_de_gravar() {
            if let Ok(mut s) = self.sujas.lock() {
                s.insert(chave);
            }
            return Ok(());
        }
        // A janela fechou: esta vai agora, e as outras da janela junto.
        t.sincronizar()?;
        if let Ok(mut s) = self.sujas.lock() {
            s.remove(&chave);
        }
        self.descarregar_sujas();
        Ok(())
    }

    /// Sincroniza tudo que foi escrito e ainda nao foi para o disco.
    ///
    /// Reabre cada tabela suja so para sincronizar. Custa um `open` por tabela,
    /// uma vez por janela -- nao por gravacao. Erro aqui nao derruba nada: a
    /// tabela continua na lista e a proxima passada tenta de novo.
    fn descarregar_sujas(&self) {
        let lista: Vec<String> = match self.sujas.lock() {
            Ok(mut s) => s.drain().collect(),
            Err(_) => return,
        };
        if lista.is_empty() {
            return;
        }
        let Ok(dados) = self.dados.lock() else { return };
        let mut faltaram = Vec::new();
        for chave in lista {
            let Some((db, tab)) = chave.split_once('/') else {
                continue;
            };
            let ok = dados
                .abrir_database(db)
                .and_then(|d| d.abrir_qualificada(tab))
                .and_then(|mut t| t.sincronizar())
                .is_ok();
            if !ok {
                faltaram.push(chave);
            }
        }
        if !faltaram.is_empty() {
            if let Ok(mut s) = self.sujas.lock() {
                s.extend(faltaram);
            }
        }
    }

    /// Ha quanto o disco esta devendo, para quem quiser mostrar.
    pub fn pendentes_de_gravacao(&self) -> u64 {
        self.janela.pendente()
    }

    /// `sequences`: o contador de cada tabela do banco, num lugar so.
    ///
    /// # Onde o numero mora de verdade
    ///
    /// Cada tabela guarda o proprio contador no cabecalho do `.reg` dela, e
    /// **continua assim**. Esta operacao junta os contadores para mostrar; nao
    /// e um arquivo `sequences` com uma segunda copia.
    ///
    /// A razao e a mesma que impede gravar "e chave primaria" na coluna: uma
    /// segunda copia e uma segunda verdade, e as duas divergem no primeiro
    /// caminho que esquecer de atualizar uma delas. Alem disso um arquivo
    /// separado custaria uma leitura e uma gravacao a mais por insercao --
    /// justamente na operacao que ja e a mais cara.
    fn op_sequencias(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };
            let e = t.esquema();
            let col = e.coluna_sequencia();
            linhas.push(Json::objeto(vec![
                ("tabela", Json::texto_de(&nome)),
                (
                    "coluna",
                    match col {
                        None => Json::Nulo,
                        Some(i) => Json::texto_de(&e.colunas()[i].nome),
                    },
                ),
                // Zero quer dizer "nunca usada": o primeiro numero sai 1.
                ("proxima", Json::de_u64(t.sequencia_atual())),
                ("registros", Json::de_u64(t.registros())),
                ("tem_sequencia", Json::Bool(col.is_some())),
            ]));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("sequencias", Json::Lista(linhas)),
        ]))
    }

    /// Ajusta o contador de uma tabela -- zerar, ou pular uma faixa.
    ///
    /// Exige `administrar`: baixar o contador abaixo de um numero ja gravado
    /// faz a proxima insercao repetir, e o erro aparece longe de quem causou.
    /// Por isso a resposta diz o que era e o que passou a ser.
    fn op_ajustar_sequencia(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let proxima = p.inteiro_ou("proxima", -1);
        if proxima < 0 {
            return Err(PhxError::Esquema(
                "informe \"proxima\" com o numero que a sequencia deve dar em seguida \
                 (0 = zerar, e o primeiro sai 1)"
                    .into(),
            ));
        }
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        if t.esquema().coluna_sequencia().is_none() {
            return Err(PhxError::Esquema(format!(
                "a tabela {} nao tem coluna Sequence",
                p.texto_ou("tabela", "")
            )));
        }
        let antes = t.sequencia_atual();
        t.ajustar_sequencia(proxima as u64)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("antes", Json::de_u64(antes)),
            ("proxima", Json::de_u64(proxima as u64)),
            (
                "aviso",
                Json::texto_de(if (proxima as u64) < antes {
                    "o contador andou para TRAS: se ja houver numero gravado nessa \
                     faixa, a proxima insercao repete e um indice unico recusa"
                } else {
                    ""
                }),
            ),
        ]))
    }

    /// Cria um schema -- uma pasta dentro do database.
    ///
    /// Estava prometido em dois lugares (a tabela de permissoes e a lista de
    /// operacoes de escrita) e nao existia no despacho: pedir `criar_schema`
    /// pela rede respondia "operacao desconhecida". A biblioteca ja sabia
    /// fazer; faltava a porta.
    fn op_criar_schema(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let schema = p.texto_ou("schema", "").trim();
        if schema.is_empty() {
            return Err(PhxError::Esquema("informe \"schema\"".into()));
        }
        let dados = self.travar_dados()?;
        dados.abrir_database(database)?.criar_schema(schema)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("schema", Json::texto_de(schema)),
        ]))
    }

    /// Cria uma tabela. Fecha o buraco que estava aberto desde a revisao: a
    /// paginacao existia, o `criar_tabela` existia na biblioteca, e nao havia
    /// caminho pela rede -- so escrevendo Rust.
    fn op_criar_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let mut esquema = crate::valores::esquema_de_json(p)?;

        // `filial.clientes` e o schema `filial` mais a tabela `clientes`, e
        // nao uma tabela chamada "filial.clientes".
        //
        // Toda leitura ja separava assim -- `abrir_qualificada` faz isso desde
        // sempre. So a CRIACAO nao fazia, e o resultado eram cinco arquivos
        // chamados `filial.clientes.reg` na raiz do banco, que nenhuma outra
        // operacao conseguia abrir: a tabela nascia inalcancavel, e o servidor
        // respondia "criada".
        let (do_nome, nome) = phxsql_store::catalogo::separar_qualificado(esquema.nome());
        let dito = p.texto_ou("schema", "").trim().to_string();
        let schema = match (do_nome.as_deref(), dito.as_str()) {
            (Some(a), b) if !b.is_empty() && a != b => {
                return Err(PhxError::Esquema(format!(
                    "o nome diz schema {a:?} e o campo \"schema\" diz {b:?}:                      escolha um dos dois"
                )))
            }
            (Some(a), _) => Some(a.to_string()),
            (None, "") => None,
            (None, b) => Some(b.to_string()),
        };
        if do_nome.is_some() {
            esquema.renomear(&nome);
        }

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        if db.existe_tabela(schema.as_deref(), &nome)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {} ja existe em {database}",
                phxsql_store::catalogo::qualificar(schema.as_deref(), &nome)
            )));
        }
        let t = db.criar_tabela(schema.as_deref(), esquema)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            (
                "schema",
                match &schema {
                    Some(s) => Json::texto_de(s),
                    None => Json::Nulo,
                },
            ),
            (
                "tabela",
                Json::texto_de(phxsql_store::catalogo::qualificar(schema.as_deref(), &nome)),
            ),
            ("colunas", Json::de_u64(t.esquema().colunas().len() as u64)),
            ("indices", Json::de_u64(t.esquema().indices().len() as u64)),
            (
                "paginada",
                Json::Bool(t.esquema().paginacao().registros_por_arquivo > 0),
            ),
        ]))
    }

    /// Declara uma chave estrangeira numa tabela QUE JA EXISTE.
    ///
    /// E o que o editor do diagrama ER chama quando alguem puxa uma coluna
    /// ate a coluna de outra tabela. Ate aqui a chave so entrava junto com o
    /// `criar_tabela` -- e o diagrama liga tabelas que ja nasceram.
    ///
    /// Declarar nao e impor: a chave fica no esquema, o `esquema` a devolve e
    /// o diagrama a desenha, mas nenhuma gravacao a confere -- ha teste que
    /// trava esse comportamento. Por isso a operacao pede o poder de CRIAR,
    /// como o `criar_tabela` que sempre pode declara-la, e nao mais.
    fn op_declarar_fk(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // `tabela_ref` e obrigatoria AQUI, embora o leitor da chave aceite
        // `tabela` como apelido dela: neste pedido o campo `tabela` e a
        // tabela que RECEBE a declaracao, e o apelido a transformaria em uma
        // referencia a si mesma sem ninguem ter pedido.
        if p.texto_ou("tabela_ref", "").trim().is_empty() {
            return Err(PhxError::Esquema(
                "informe \"tabela_ref\" (a tabela referenciada); \
                 \"tabela\" e a que recebe a chave"
                    .into(),
            ));
        }
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let nova = crate::valores::chave_estrangeira_de_json(p, 0, t.esquema())?;
        let mut fks = t.esquema().chaves_estrangeiras().to_vec();
        if fks.iter().any(|f| f.nome == nova.nome) {
            return Err(PhxError::Duplicado(format!(
                "a chave {} ja esta declarada em {}; exclua-a antes de redeclarar",
                nova.nome,
                t.nome()
            )));
        }
        let nome = nova.nome.clone();
        fks.push(nova);
        let reescreveu = t.redeclarar_chaves_estrangeiras(fks)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("nome", Json::texto_de(nome)),
            (
                "chaves_estrangeiras",
                Json::de_u64(t.esquema().chaves_estrangeiras().len() as u64),
            ),
            // A chave e DECLARADA. Quem chama precisa poder dizer a verdade
            // na tela sem conhecer o motor de cor.
            ("imposta", Json::Bool(false)),
            ("arquivos_reescritos", Json::Bool(reescreveu)),
        ]))
    }

    /// Desfaz a declaracao de uma chave estrangeira, pelo nome.
    ///
    /// Encolher o bloco de esquema cabe sempre no lugar, entao isto nunca
    /// reescreve arquivo -- e nao toca em dado nenhum, porque a chave nunca
    /// foi imposta.
    fn op_excluir_fk(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        if nome.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"nome\" (o nome da chave declarada)".into(),
            ));
        }
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let mut fks = t.esquema().chaves_estrangeiras().to_vec();
        let antes = fks.len();
        fks.retain(|f| f.nome != nome);
        if fks.len() == antes {
            return Err(PhxError::NaoEncontrado(format!(
                "{} nao tem uma chave chamada {nome:?}; as declaradas sao [{}]",
                t.nome(),
                t.esquema()
                    .chaves_estrangeiras()
                    .iter()
                    .map(|f| f.nome.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        t.redeclarar_chaves_estrangeiras(fks)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("nome", Json::texto_de(nome)),
            (
                "chaves_estrangeiras",
                Json::de_u64(t.esquema().chaves_estrangeiras().len() as u64),
            ),
        ]))
    }

    /// Apaga os cinco arquivos de uma tabela.
    ///
    /// Exige o nome repetido no campo `confirmar`. Nao e burocracia: excluir
    /// uma tabela apaga o `.reg`, o `.ndx`, o `.bin`, o `.memo` e o `.log` de
    /// uma vez, e nao ha desfazer. Um `rowid` errado perde uma linha; um nome
    /// errado aqui perde tudo.
    fn op_excluir_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if p.texto_ou("confirmar", "") != tabela {
            return Err(PhxError::Esquema(format!(
                "para excluir, repita o nome da tabela no campo \"confirmar\": \
                 esperado {tabela:?}"
            )));
        }
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let apagados = db.excluir_tabela(tabela)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("tabela", Json::texto_de(tabela)),
            (
                "arquivos_apagados",
                Json::Lista(apagados.iter().map(Json::texto_de).collect()),
            ),
        ]))
    }

    /// Copia uma tabela inteira para outro nome, no mesmo database.
    ///
    /// Copia byte a byte os cinco arquivos, entao a copia nasce com a MESMA
    /// ordem de digitacao e os MESMOS rowids do original -- que e o que se
    /// espera de uma duplicata, e o que uma reinsercao linha a linha nao
    /// daria.
    fn op_duplicar_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        let destino = p.texto_ou("destino", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let copiados = db.duplicar_tabela(tabela, destino)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("origem", Json::texto_de(tabela)),
            ("destino", Json::texto_de(destino)),
            ("arquivos", Json::de_u64(copiados as u64)),
        ]))
    }

    /// Nome de quem tem este id no cadastro. Vazio quando ninguem tem.
    ///
    /// O `.log`, o `.trash` e o `.reason` guardam o id numerico, e nao o nome:
    /// o id nao muda quando alguem e renomeado, e uma exclusao de 2019 tem de
    /// continuar apontando para a mesma pessoa. Traduzir na hora de MOSTRAR e
    /// o que faz o registro ser legivel sem prender o arquivo ao cadastro.
    fn nome_do_usuario(&self, id: u32) -> String {
        if id == 0 {
            return String::new();
        }
        self.config
            .cadastro
            .root
            .iter()
            .chain(self.config.cadastro.usuarios.iter())
            .find(|u| u.id == id)
            .map(|u| u.nome.clone())
            .unwrap_or_default()
    }

    /// `sql`: um `SELECT` simples traduzido para as operacoes que ja existem.
    ///
    /// # O portao continua sendo UM
    ///
    /// Esta operacao nao le tabela nenhuma por conta propria. Ela faz duas
    /// coisas, e as duas pelo `executar_derivado`, que e o mesmo portao do
    /// pedido que chega pela rede:
    ///
    /// 1. pede o `esquema` da tabela do `FROM` -- que ja exige `ler` naquela
    ///    tabela --, e e de la que saem os indices que o tradutor precisa;
    /// 2. executa o `varrer` ou o `buscar` que a traducao produziu.
    ///
    /// O campo `tabela` do pedido TRADUZIDO e o que o portao confere. Por isso
    /// `SELECT * FROM folha` de quem nao pode ler a folha para no passo 1, com
    /// o mesmo erro de um `{"op":"varrer","tabela":"folha"}` -- e nao ha
    /// conferencia propria aqui que alguem possa esquecer de atualizar.
    ///
    /// # Por que o esquema vem pelo protocolo, e nao de um `abrir_travada`
    ///
    /// Porque abrir a tabela aqui seria o SEGUNDO caminho ate o dado, e o
    /// segundo caminho e sempre o que esquece uma conferencia. Custa um
    /// `esquema` a mais por consulta; a alternativa custa uma porta dos fundos.
    fn op_sql(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let texto = p
            .texto_ou("texto", p.texto_ou("sql", ""))
            .trim()
            .to_string();
        if texto.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"texto\" com o comando SQL".into(),
            ));
        }
        // O erro de sintaxe ja vem com a coluna: «SQL, coluna 14: esperava
        // FROM». Reembalar aqui perderia a posicao, que e a unica parte da
        // mensagem que diz ONDE consertar.
        let selecao = phxsql_sql::analisar(&texto)?;

        let base = match selecao.de.database.trim() {
            "" => p.texto_ou("database", "").trim().to_string(),
            outro => outro.to_string(),
        };
        let tabela = selecao.de.nome_no_protocolo();
        let ped_esquema = Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("tabela", Json::texto_de(&tabela)),
        ]);
        let esquema = self.executar_derivado("esquema", &ped_esquema, sessao)?;

        let plano = phxsql_sql::traduzir(&selecao, &indices_do_esquema(&esquema), &base)?;
        let bruto = self.executar_derivado(&plano.op, &plano.pedido, sessao)?;

        Ok(resposta_do_sql(&texto, &plano, bruto))
    }

    /// O catalogo das operacoes -- o `--help` do protocolo, servido por dados.
    ///
    /// Filtra pelo poder de quem perguntou, e a filtragem NAO e o portao: o
    /// portao continua sendo o do `despachar`, que confere de novo quando a
    /// operacao for chamada. Esconder aqui e cortesia, para nao oferecer
    /// oitenta operacoes a quem so pode chamar tres.
    ///
    /// Com o campo `"operacao"`, detalha uma so -- e e assim que o
    /// `/help <comando>` do `phxsqlcmd` funciona sem carregar o catalogo
    /// inteiro. O campo nao se chama `"op"` porque esse ja e o nome da
    /// operacao chamada: um pedido nao tem a mesma chave duas vezes.
    fn op_catalogo(&self, p: &Json, sessao: &Sessao) -> Json {
        let base = p.texto_ou("database", "");
        let usuario = sessao.usuario.as_ref();
        let visiveis = crate::catalogo::visiveis(usuario, base);

        if let Some(pedida) = Some(p.texto_ou("operacao", "").trim()).filter(|s| !s.is_empty()) {
            // Operacao que existe mas este usuario nao pode chamar responde
            // "nao existe para voce", com a permissao que faltou -- e nao um
            // 404 seco, que mandaria procurar erro de digitacao onde nao ha.
            let achada = visiveis.iter().find(|o| o.nomes().any(|n| n == pedida));
            return match achada {
                Some(o) => Json::objeto(vec![
                    ("pedida", Json::texto_de(o.nome)),
                    ("operacao", o.para_json()),
                ]),
                None => {
                    let existe = crate::catalogo::por_nome(pedida);
                    Json::objeto(vec![
                        ("pedida", Json::texto_de(pedida)),
                        ("operacao", Json::Nulo),
                        (
                            "motivo",
                            Json::texto_de(match existe {
                                Some(o) => format!(
                                    "a operacao {pedida:?} existe, mas exige {}",
                                    o.atividade().map(|a| a.nome()).unwrap_or("login")
                                ),
                                None => format!("a operacao {pedida:?} nao existe"),
                            }),
                        ),
                    ])
                }
            };
        }

        Json::objeto(vec![
            ("total", Json::de_u64(visiveis.len() as u64)),
            // Quantas ficaram de fora por permissao. Sem este numero, quem ve
            // uma lista curta nao sabe se o servidor e pequeno ou se ele e que
            // pode pouco.
            (
                "ocultas",
                Json::de_u64((crate::catalogo::OPERACOES.len() - visiveis.len()) as u64),
            ),
            (
                "operacoes",
                Json::Lista(visiveis.iter().map(|o| o.para_json()).collect()),
            ),
        ])
    }

    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let t = self.abrir_travada(&_trava, p, sessao)?;
        let e = t.esquema();
        let colunas: Vec<Json> = e
            .colunas()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let papel = e.papel_da_coluna(i);
                Json::objeto(vec![
                    ("id", Json::texto_de(c.id.to_string())),
                    ("nome", Json::texto_de(&c.nome)),
                    ("caption", Json::texto_de(&c.caption)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("mascara", Json::texto_de(&c.mascara)),
                    ("dado_pessoal", Json::texto_de(c.dado_pessoal.nome())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    // Coluna do MOTOR: a tela nao a oferece como campo de
                    // formulario. Quem manda nela e o botao de excluir.
                    (
                        "sistema",
                        Json::Bool(phxsql_core::schema::e_coluna_de_sistema(&c.nome)),
                    ),
                    // O papel nas chaves e DERIVADO dos indices e das FKs, e
                    // por isso nao pode discordar delas.
                    ("primaria", Json::Bool(papel.primaria)),
                    ("estrangeira", Json::Bool(papel.estrangeira)),
                    (
                        "composta",
                        Json::Bool(papel.primaria_composta || papel.estrangeira_composta),
                    ),
                    (
                        "nas_chaves_estrangeiras",
                        Json::Lista(
                            papel
                                .chaves_estrangeiras
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                    (
                        "nos_indices",
                        Json::Lista(papel.indices.iter().map(Json::texto_de).collect()),
                    ),
                ])
            })
            .collect();
        let indices: Vec<Json> = e
            .indices()
            .iter()
            .map(|i| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&i.nome)),
                    ("unico", Json::Bool(i.unico)),
                    ("primario", Json::Bool(i.primario)),
                    ("composto", Json::Bool(i.composta())),
                    (
                        "colunas",
                        Json::Lista(
                            i.colunas
                                .iter()
                                .map(|ic| {
                                    Json::objeto(vec![
                                        ("coluna", Json::texto_de(&e.colunas()[ic.coluna].nome)),
                                        ("desc", Json::Bool(ic.desc)),
                                        ("nocase", Json::Bool(ic.nocase)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let fks: Vec<Json> = e
            .chaves_estrangeiras()
            .iter()
            .map(|fk| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&fk.nome)),
                    (
                        "colunas",
                        Json::Lista(
                            fk.colunas
                                .iter()
                                .map(|c| Json::texto_de(&e.colunas()[*c].nome))
                                .collect(),
                        ),
                    ),
                    ("tabela_ref", Json::texto_de(&fk.tabela_ref)),
                    (
                        "colunas_ref",
                        Json::Lista(fk.colunas_ref.iter().map(Json::texto_de).collect()),
                    ),
                    ("ao_excluir", Json::texto_de(format!("{:?}", fk.ao_excluir))),
                    ("ao_alterar", Json::texto_de(format!("{:?}", fk.ao_alterar))),
                ])
            })
            .collect();
        let pag = e.paginacao();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(e.nome())),
            ("registros", Json::de_u64(t.registros())),
            ("slots", Json::de_u64(t.slots())),
            ("colunas", Json::Lista(colunas)),
            ("indices", Json::Lista(indices)),
            ("chaves_estrangeiras", Json::Lista(fks)),
            // Na particao por periodo o volume nao sai de conta: quem sabe
            // onde cada faixa comeca e a tabela de fronteiras, lida dos
            // cabecalhos. Sem isto a tela teria de adivinhar.
            (
                "volumes",
                Json::Lista(
                    t.fronteiras()
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            Json::objeto(vec![
                                ("volume", Json::de_u64(i as u64 + 1)),
                                ("primeiro_rowid", Json::de_u64(f.primeiro_rowid)),
                                (
                                    "periodo",
                                    match pag.modo.periodo() {
                                        None => Json::Nulo,
                                        Some(per) => Json::texto_de(per.rotulo(f.chave_periodo)),
                                    },
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "paginacao",
                if pag.ligada() {
                    Json::objeto(vec![
                        (
                            "registros_por_arquivo",
                            Json::de_u64(pag.registros_por_arquivo),
                        ),
                        ("max_arquivos", Json::de_u64(pag.max_arquivos as u64)),
                        ("capacidade", Json::de_u64(pag.capacidade())),
                        // A largura do sufixo vai junto porque sem ela nao da
                        // para escrever o nome do volume: `_1` e `_001` sao
                        // arquivos diferentes.
                        ("digitos", Json::de_u64(pag.digitos as u64)),
                        (
                            "modo",
                            Json::texto_de(match pag.modo.periodo() {
                                Some(per) => per.nome().to_string(),
                                None => pag.modo.nome().to_string(),
                            }),
                        ),
                        // Os baldes da particao alfanumerica, ja com o nome do
                        // arquivo e quantas linhas tem. So a tabela sabe: a
                        // contagem mora no cabecalho de cada volume, e a tela
                        // nao tem como deduzir dela quantos slots foram usados
                        // no `_S` -- o `slots` daqui e a marca d'agua, e nao
                        // uma contagem.
                        (
                            "baldes",
                            if pag.modo.por_letra() {
                                let baldes = t.baldes();
                                let existentes = t.volumes_por_arquivo().0;
                                Json::Lista(
                                    phxsql_core::paginacao::BALDES
                                        .iter()
                                        .enumerate()
                                        .map(|(i, letra)| {
                                            let n = i as u32 + 1;
                                            Json::objeto(vec![
                                                ("volume", Json::de_u64(n as u64)),
                                                ("letra", Json::texto_de(*letra)),
                                                (
                                                    "arquivo",
                                                    Json::texto_de(format!(
                                                        "{}_{letra}.reg",
                                                        e.nome()
                                                    )),
                                                ),
                                                (
                                                    "registros",
                                                    Json::de_u64(
                                                        baldes.get(i).copied().unwrap_or(0),
                                                    ),
                                                ),
                                                ("existe", Json::Bool(existentes.contains(&n))),
                                                (
                                                    "primeiro_rowid",
                                                    Json::de_u64(
                                                        (n as u64 - 1) * pag.registros_por_arquivo
                                                            + 1,
                                                    ),
                                                ),
                                            ])
                                        })
                                        .collect(),
                                )
                            } else {
                                Json::Nulo
                            },
                        ),
                        (
                            "coluna",
                            match pag.modo.coluna() {
                                None => Json::Nulo,
                                Some(i) => Json::texto_de(&e.colunas()[i].nome),
                            },
                        ),
                        ("bytes_por_arquivo", Json::de_u64(pag.bytes_por_arquivo)),
                    ])
                } else {
                    Json::Nulo
                },
            ),
        ]))
    }

    /// `ler`: uma linha pelo rowid.
    ///
    /// Com `"com_versao": true` a resposta deixa de ser a linha crua e passa
    /// a ser `{linha, rowid, versao}`. A forma muda porque a versao NAO pode
    /// entrar como mais uma chave dentro da linha: ali ela viraria uma coluna
    /// que nao existe no esquema, e todo cliente que percorre as chaves da
    /// resposta comecaria a mandar de volta um campo fantasma. Quem nao pede
    /// continua recebendo exatamente o que sempre recebeu.
    fn op_ler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let com_versao = p.booleano_ou("com_versao", false);
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let linha = match t.ler(rowid)? {
            None => return Ok(Json::Nulo),
            Some(l) => linha_para_json(&l, t.esquema()),
        };
        if !com_versao {
            return Ok(linha);
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("linha", linha),
            ("versao", Json::de_u64(t.versao(rowid)?.unwrap_or(0))),
        ]))
    }

    fn op_varrer(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p);
        let indice = p.texto_ou("indice", "").to_string();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        // `visao` decide o que a varredura enxerga. O padrao e "ativas": a
        // linha marcada como excluida some das listas, senao marcar nao teria
        // efeito nenhum.
        let visao = match p.texto_ou("visao", "ativas").trim() {
            "" | "ativas" | "ativos" => Visao::Ativas,
            "excluidas" | "excluidos" => Visao::Excluidas,
            "todas" | "todos" => Visao::Todas,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "visao {outro:?} nao existe; use ativas, excluidas ou todas"
                )))
            }
        };

        // Quatro modos.
        //
        // `depois` / `antes` sao o CURSOR: a pagina custa o tamanho dela, e
        // nao o tamanho da tabela. `desde_rownum` e o cursor de quem guardou o
        // numero de ordem em vez do rowid. E `pular` e a POSICAO -- o `OFFSET`
        // do SQL, que deixou de andar ate la sempre: quando a posicao e o
        // rownum, ele bisseta. A resposta diz qual dos dois pagou.
        let depois = p.inteiro_ou("depois", -1);
        let antes = p.inteiro_ou("antes", -1);
        let desde_rownum = p.inteiro_ou("desde_rownum", -1);
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let mut salto = None;

        let por_indice = !indice.is_empty();
        let (rowids, modo) = if por_indice {
            // O indice devolve rowid na ordem da CHAVE, e nao na do arquivo:
            // continuar "depois do rowid X" nao quer dizer nada aqui, porque
            // o proximo da chave pode ter rowid menor. Entao por indice vale a
            // posicao, e a resposta diz isso em vez de fingir que paginou.
            let todos = t.varrer_indice(&indice)?;
            let vivos = t.filtrar(&todos, visao)?;
            let corte: Vec<u64> = vivos
                .into_iter()
                .skip(pular as usize)
                .take(max as usize)
                .collect();
            (corte, "posicao")
        } else if antes >= 0 {
            (t.pagina_antes_de(antes as u64, max, visao)?, "cursor")
        } else if depois >= 0 {
            (t.pagina_depois_de(depois as u64, max, visao)?, "cursor")
        } else if desde_rownum >= 0 {
            (
                t.pagina_desde_rownum(desde_rownum as u64, max, visao)?,
                "rownum",
            )
        } else {
            let (rowids, como) = t.pagina_por_posicao(pular, max, visao)?;
            salto = Some(como);
            (rowids, "posicao")
        };

        // PONTO DE CANCELAMENTO. A pagina cabe no teto de linhas, mas o teto
        // e de configuracao e pode ser grande: uma varredura de cem mil linhas
        // segura a trava por segundos como qualquer outra.
        let atividade = crate::telemetria::corrente();
        let _fase = atividade
            .as_ref()
            .map(|a| a.fase_cancelavel("lendo as linhas da pagina"));
        let mut linhas = Vec::with_capacity(rowids.len());
        for &rowid in &rowids {
            if let Some(a) = &atividade {
                a.siga(1)?;
            }
            if let Some(l) = t.ler(rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }

        // O cursor para pedir a proxima pagina e a anterior. Vai pronto na
        // resposta para o cliente nao ter de saber que ele e um rowid -- e
        // para poder deixar de ser um, se um dia a ordem mudar.
        let primeiro = rowids.first().copied().unwrap_or(0);
        let ultimo = rowids.last().copied().unwrap_or(0);
        let rownum_inicio = t.rownum_de(primeiro)?;
        let rownum_fim = t.rownum_de(ultimo)?;
        // "Tem mais" sem contar a tabela: pede UM alem do teto. Uma leitura a
        // mais por pagina, contra uma varredura inteira so para mostrar
        // "pagina 3 de 40" -- que numa tabela grande e o item mais caro da
        // tela e o que ninguem le.
        let ha_mais = ultimo > 0 && !t.pagina_depois_de(ultimo, 1, visao)?.is_empty();
        let ha_antes = primeiro > 1 && !t.pagina_antes_de(primeiro, 1, visao)?.is_empty();

        Ok(Json::objeto(vec![
            // `registros` e o que a tabela tem, e sai do cabecalho: nao custa
            // varredura. `total` era a contagem da varredura inteira, e por
            // isso deixou de existir aqui.
            ("registros", Json::de_u64(t.registros())),
            // Quantas linhas ESTA visao enxerga -- e a conta de «pagina 3 de
            // 40». Sai de dois contadores do cabecalho, sem varrer nada; era
            // por nao existir que a contagem tinha sido tirada da resposta.
            ("visiveis", Json::de_u64(t.contar(visao))),
            ("marcadas", Json::de_u64(t.marcadas())),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            ("modo", Json::texto_de(modo)),
            // Como o inicio da pagina foi achado, quando o modo e por posicao.
            // «bisseccao» sao ~20 leituras; «passo» sao `pular` leituras.
            (
                "salto",
                match salto {
                    Some(s) => Json::texto_de(s.nome()),
                    None => Json::Nulo,
                },
            ),
            ("cursor_inicio", Json::de_u64(primeiro)),
            ("cursor_fim", Json::de_u64(ultimo)),
            // O numero de ordem da primeira e da ultima linha da pagina: e o
            // cursor de quem pagina por `desde_rownum`, e o que a caixa «ir
            // para a linha N» devolve para a tela se localizar.
            ("rownum_inicio", Json::de_u64(rownum_inicio)),
            ("rownum_fim", Json::de_u64(rownum_fim)),
            ("ha_mais", Json::Bool(ha_mais)),
            ("ha_antes", Json::Bool(ha_antes)),
            (
                "ordem",
                Json::texto_de(if por_indice {
                    format!("indice {indice}")
                } else {
                    "digitacao".to_string()
                }),
            ),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    fn op_buscar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let indice = p.texto_ou("indice", "").to_string();
        if indice.is_empty() {
            return Err(PhxError::Esquema("informe \"indice\"".into()));
        }
        let chave_json = p
            .campo("chave")
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"chave\"".into()))?;
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let pos = t
            .esquema()
            .indice_por_nome(&indice)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} nao existe")))?;
        let chave = json_para_chave(&chave_json, t.esquema(), pos)?;
        let rowids = t.buscar(&indice, &chave)?;

        let mut linhas = Vec::new();
        for rowid in rowids.iter().take(self.limite(p) as usize) {
            if let Some(l) = t.ler(*rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(*rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        Ok(Json::objeto(vec![
            ("encontrados", Json::de_u64(rowids.len() as u64)),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    fn op_inserir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let linha = json_para_linha(&valores_json, t.esquema())?;
        let rowid = t.inserir(&linha)?;
        self.gravar_de_verdade(&mut t, p)?;
        // A copia em RAM acompanha DENTRO da mesma trava: nao existe instante
        // em que o disco e a memoria discordem.
        self.residente_mut(p, |m| m.anotar_insercao(rowid, &linha));
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("registros", Json::de_u64(t.registros())),
        ]))
    }

    /// `inserir_lote`: muitas linhas de uma vez, ou uma carga colada.
    ///
    /// # De onde vem o ganho
    ///
    /// Nao e do disco. Cada linha custa o mesmo la dentro -- montar o payload,
    /// conferir a unicidade, gravar o slot, manter cada indice. O ganho e de
    /// tudo que acontecia POR LINHA e passa a acontecer uma vez: abrir a
    /// tabela (sete arquivos), tomar a trava, e o `fsync`.
    ///
    /// Vinte mil insercoes pela rede eram vinte mil aberturas de tabela.
    ///
    /// # Duas formas de mandar
    ///
    /// `"linhas"` com uma lista de objetos, ou `"texto"` com uma carga colada
    /// mais `"formato"` -- json, csv, txt, html ou xml. Sem formato, ele e
    /// adivinhado pelo primeiro caractere.
    fn op_inserir_lote(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let parar = p.booleano_ou("parar_no_erro", true);

        // A carga colada vira lista de objetos ANTES de a trava ser tomada:
        // analisar texto com a trava de dados na mao seguraria todo mundo por
        // causa de um CSV malformado.
        // Duas origens: uma carga COLADA (texto num dos cinco formatos) ou uma
        // lista de objetos JSON ja tipada. A colada e lida antes de a trava
        // ser tomada -- analisar um CSV malformado com a trava de dados na mao
        // seguraria todo mundo.
        let colada = match p.campo("texto").and_then(Json::texto) {
            Some(texto) => {
                let f = match p.texto_ou("formato", "").trim() {
                    "" | "auto" => phxsql_core::carga::adivinhar(texto),
                    outro => phxsql_core::carga::Formato::de_texto(outro)?,
                };
                Some((phxsql_core::carga::ler(texto, f)?, f.nome().to_string()))
            }
            None => None,
        };
        let itens: Vec<Json> = match &colada {
            Some(_) => Vec::new(),
            None => p
                .campo("linhas")
                .or_else(|| p.campo("valores"))
                .and_then(Json::lista)
                .map(|l| l.to_vec())
                .ok_or_else(|| {
                    PhxError::Esquema(
                        "informe \"linhas\" com a lista, ou \"texto\" com a carga colada".into(),
                    )
                })?,
        };
        let formato = match &colada {
            Some((_, f)) => f.clone(),
            None => "lista".to_string(),
        };
        let recebidas = match &colada {
            Some((c, _)) => c.linhas.len(),
            None => itens.len(),
        };
        if recebidas == 0 {
            return Err(PhxError::Esquema("a carga nao tem nenhuma linha".into()));
        }

        let inicio = Instant::now();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        // A conversao acontece aqui, com o esquema na mao. Uma linha que nao
        // converte entra na lista de recusadas em vez de derrubar a carga
        // inteira -- a menos que `parar_no_erro` mande parar.
        let mut linhas: Vec<Vec<phxsql_core::value::Value>> = Vec::with_capacity(recebidas);
        let mut recusadas: Vec<(usize, String)> = Vec::new();
        // PONTO DE CANCELAMENTO -- e ele acaba ANTES da gravacao.
        //
        // A conversao de cinco mil linhas com a trava na mao e a parte longa
        // e a parte segura: nada foi escrito ainda, e abandonar aqui e como
        // se o pedido nunca tivesse chegado. Ja o `inserir_lote` logo abaixo
        // grava slot, indice e diario por linha, e nao aceita marca nenhuma:
        // parar no meio dele deixaria a tabela com metade do lote e o indice
        // com a outra metade. A fase fecha sozinha quando esta chave fecha.
        let atividade = crate::telemetria::corrente();
        let _fase = atividade
            .as_ref()
            .map(|a| a.fase_cancelavel("convertendo as linhas da carga"));
        for i in 0..recebidas {
            if let Some(a) = &atividade {
                a.siga(1)?;
            }
            let convertida = match (&colada, itens.get(i)) {
                (Some((c, _)), _) => phxsql_core::carga::linha_de_texto(c, i, t.esquema()),
                (None, Some(item)) => json_para_linha(item, t.esquema()),
                (None, None) => break,
            };
            match convertida {
                Ok(l) => linhas.push(l),
                Err(e) => {
                    recusadas.push((i, e.to_string()));
                    if parar {
                        return Ok(Self::resposta_do_lote(
                            p,
                            &formato,
                            recebidas,
                            &[],
                            &recusadas,
                            inicio,
                        ));
                    }
                }
            }
        }

        // Daqui para baixo NAO ha ponto de cancelamento: a fase fecha aqui, e
        // a gravacao vai ate o fim.
        drop(_fase);
        let lote = t.inserir_lote(&linhas, parar)?;
        // Uma carga inteira e um `sincronizar`, e nao um por linha.
        t.sincronizar()?;
        for (i, e) in &lote.recusadas {
            recusadas.push((*i, e.clone()));
        }
        // A copia em RAM acompanha dentro da mesma trava.
        for (rowid, linha) in lote.rowids.iter().zip(linhas.iter()) {
            let (r, l) = (*rowid, linha.clone());
            self.residente_mut(p, move |m| m.anotar_insercao(r, &l));
        }
        Ok(Self::resposta_do_lote(
            p,
            &formato,
            recebidas,
            &lote.rowids,
            &recusadas,
            inicio,
        ))
    }

    /// `importar_conferir`: le a carga e devolve o que entendeu, SEM gravar.
    ///
    /// Existe porque uma carga que entra errada e pior que uma que nao entra.
    /// A tela mostra a amostra e as colunas casadas antes de o botao de gravar
    /// ficar disponivel.
    ///
    /// Le pelo MESMO caminho da gravacao. Uma previa escrita no navegador
    /// seria uma segunda implementacao do leitor, e as duas divergiriam no
    /// primeiro caso esquisito -- que e justamente onde a previa serve.
    fn op_importar_conferir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let texto = p
            .campo("texto")
            .and_then(Json::texto)
            .ok_or_else(|| PhxError::Esquema("informe \"texto\" com a carga".into()))?;
        let f = match p.texto_ou("formato", "").trim() {
            "" | "auto" => phxsql_core::carga::adivinhar(texto),
            outro => phxsql_core::carga::Formato::de_texto(outro)?,
        };
        let carga = phxsql_core::carga::ler(texto, f)?;

        let _trava = self.travar_dados()?;
        let t = self.abrir_travada(&_trava, p, sessao)?;
        let e = t.esquema();

        // As duas listas que decidem se a carga serve: o que a tabela nao tem
        // (erro) e o que a carga nao traz (fica nulo).
        let desconhecidas: Vec<Json> = carga
            .colunas
            .iter()
            .filter(|c| e.coluna_por_nome(c).is_none())
            .map(Json::texto_de)
            .collect();
        let faltando: Vec<Json> = e
            .colunas()
            .iter()
            .filter(|c| {
                !phxsql_core::schema::e_coluna_de_sistema(&c.nome)
                    && !carga.colunas.contains(&c.nome)
            })
            .map(|c| Json::texto_de(&c.nome))
            .collect();

        const AMOSTRA: usize = 20;
        let amostra: Vec<Json> = carga
            .linhas
            .iter()
            .take(AMOSTRA)
            .map(|l| Json::Lista(l.iter().map(Json::texto_de).collect()))
            .collect();

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("formato", Json::texto_de(f.nome())),
            ("linhas_lidas", Json::de_u64(carga.linhas.len() as u64)),
            (
                "colunas",
                Json::Lista(carga.colunas.iter().map(Json::texto_de).collect()),
            ),
            ("desconhecidas", Json::Lista(desconhecidas)),
            ("faltando", Json::Lista(faltando)),
            ("amostra", Json::Lista(amostra)),
        ]))
    }

    fn resposta_do_lote(
        p: &Json,
        formato: &str,
        recebidas: usize,
        rowids: &[u64],
        recusadas: &[(usize, String)],
        inicio: Instant,
    ) -> Json {
        let ms = inicio.elapsed().as_millis() as u64;
        Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("formato", Json::texto_de(formato)),
            ("recebidas", Json::de_u64(recebidas as u64)),
            ("gravadas", Json::de_u64(rowids.len() as u64)),
            ("recusadas", Json::de_u64(recusadas.len() as u64)),
            (
                "primeiro_rowid",
                Json::de_u64(rowids.first().copied().unwrap_or(0)),
            ),
            (
                "ultimo_rowid",
                Json::de_u64(rowids.last().copied().unwrap_or(0)),
            ),
            ("ms", Json::de_u64(ms)),
            (
                "por_segundo",
                Json::de_u64(if ms == 0 {
                    0
                } else {
                    (rowids.len() as u64) * 1000 / ms
                }),
            ),
            // A POSICAO na carga, e nao o rowid: a linha recusada nao tem
            // rowid, e quem mandou precisa achar a linha no arquivo dele.
            (
                "erros",
                Json::Lista(
                    recusadas
                        .iter()
                        .take(50)
                        .map(|(i, e)| {
                            Json::objeto(vec![
                                ("linha", Json::de_u64(*i as u64 + 1)),
                                ("erro", Json::texto_de(e)),
                            ])
                        })
                        .collect(),
                ),
            ),
            // Sem transacao, uma carga que para no meio DEIXA gravado o que ja
            // entrou. Dizer isso na resposta e melhor que quem chamou
            // descobrir contando as linhas depois.
            (
                "aviso",
                Json::texto_de(if recusadas.is_empty() {
                    ""
                } else {
                    "nao ha transacao: as linhas gravadas antes do erro ficaram gravadas"
                }),
            ),
        ])
    }

    fn op_atualizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let mut linha = json_para_linha(&valores_json, t.esquema())?;

        // Quem alterou a linha nao mandou a coluna de sistema? Entao ela nao
        // muda. Sem isto, `json_para_linha` preencheria `false` e um
        // `atualizar` de rotina RESSUSCITARIA uma linha excluida -- sem erro
        // nenhum, e sem ninguem perceber ate a linha reaparecer na lista.
        if let Some(i) = t.esquema().coluna_softdeleted() {
            let nome = phxsql_core::schema::COLUNA_SOFTDELETED;
            let veio = matches!(&valores_json, Json::Objeto(_))
                && valores_json.campo(nome).is_some()
                || matches!(&valores_json, Json::Lista(l) if l.len() > i);
            if !veio {
                if let Some(atual) = t.ler(rowid)? {
                    linha[i] = atual[i].clone();
                }
            }
        }

        t.atualizar(rowid, &linha)?;
        self.gravar_de_verdade(&mut t, p)?;
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        // A versao nova volta na resposta: quem grava duas vezes seguidas
        // continua protegido sem precisar reler a linha inteira no meio.
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("versao", Json::de_u64(t.versao(rowid)?.unwrap_or(0))),
        ]))
    }

    /// Exclui. **Suave por padrao**, fisica so quando pedida.
    ///
    /// # Por que o padrao e o suave
    ///
    /// O caminho reversivel e o padrao porque o irreversivel nao pode ser
    /// escolhido por omissao: um cliente antigo que manda `excluir` sem dizer
    /// nada esta pedindo "tira isto da minha lista", e e isso que ele recebe.
    /// Quem quer apagar de vez escreve `"fisico": true` e sabe o que esta
    /// fazendo. Numa tabela sem a coluna de sistema -- as anteriores a v4 do
    /// esquema -- so existe o caminho fisico, e ele e usado sem alarde.
    fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        let fisico = p.booleano_ou("fisico", false);
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let tem_marca = t.esquema().coluna_softdeleted().is_some();

        if fisico || !tem_marca {
            let removeu = t.excluir_de_vez(rowid, &motivo)?;
            self.gravar_de_verdade(&mut t, p)?;
            if removeu {
                self.residente_mut(p, |m| m.anotar_exclusao(rowid));
            }
            return Ok(Json::objeto(vec![
                ("rowid", Json::de_u64(rowid)),
                ("excluido", Json::Bool(removeu)),
                ("modo", Json::texto_de("fisico")),
                ("na_lixeira", Json::Bool(removeu)),
                ("reversivel", Json::Bool(false)),
            ]));
        }

        let marcou = t.excluir_suave(rowid, &motivo)?;
        self.gravar_de_verdade(&mut t, p)?;
        // A copia em RAM tem de esquecer a linha tambem: para quem consulta,
        // marcada e o mesmo que ausente.
        if marcou {
            self.residente_mut(p, |m| m.anotar_exclusao(rowid));
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(marcou)),
            ("modo", Json::texto_de("suave")),
            ("na_lixeira", Json::Bool(false)),
            ("reversivel", Json::Bool(true)),
        ]))
    }

    /// Desfaz uma exclusao suave.
    fn op_restaurar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let voltou = t.restaurar(rowid, &motivo)?;
        self.gravar_de_verdade(&mut t, p)?;
        if voltou {
            // A linha volta a existir para quem consulta em memoria.
            if let Some(linha) = t.ler(rowid)? {
                self.residente_mut(p, |m| m.anotar_insercao(rowid, &linha));
            }
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("restaurado", Json::Bool(voltou)),
        ]))
    }

    /// `lixeira`: as linhas que sairam do `.reg`. **So administrador.**
    ///
    /// Os anexos so vao junto com `"com_anexos": true`: listar mil linhas
    /// carregaria mil fotos para mostrar quem excluiu o que e quando.
    fn op_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 200).max(0) as u64;
        // Um `uuid` pede UMA linha, e ai os anexos vem sempre: quem pediu uma
        // linha especifica quer ela inteira. Sem uuid e listagem, e a listagem
        // nao carrega anexo por padrao -- um memo de megabytes vezes trezentas
        // linhas viraria uma resposta que ninguem consegue usar.
        let so_uma = p.texto_ou("uuid", "").trim().to_string();
        let com_anexos = p.booleano_ou("com_anexos", !so_uma.is_empty());
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let descartadas = if so_uma.is_empty() {
            t.lixeira(pular, limite, com_anexos)?
        } else {
            let alvo = phxsql_core::uuid::Uuid::de_texto(&so_uma)
                .map_err(|e| PhxError::Esquema(format!("uuid da linha descartada: {e}")))?;
            t.lixeira(0, 0, true)?
                .into_iter()
                .filter(|d| d.uuid.bytes() == alvo.bytes())
                .collect()
        };
        let (total, bytes) = t.lixeira_tamanho()?;
        let esquema = t.esquema().clone();

        let mut linhas = Vec::with_capacity(descartadas.len());
        for d in &descartadas {
            // A linha pode nao decodificar: se o esquema mudou depois do
            // descarte, o payload guardado nao bate com ele. Isso nao pode
            // derrubar a listagem inteira -- a entrada aparece com o aviso, e
            // as outras continuam sendo mostradas.
            let (linha, aviso) = match t.linha_da_lixeira(d) {
                Ok(l) => (crate::valores::linha_para_json(&l, &esquema), String::new()),
                Err(e) => (Json::Nulo, e.to_string()),
            };
            linhas.push(Json::objeto(vec![
                ("uuid", Json::texto_de(d.uuid.to_string())),
                ("rowid", Json::de_u64(d.rowid)),
                ("quando", Json::texto_de(d.instante_iso())),
                ("usuario", Json::de_u64(d.usuario as u64)),
                (
                    "usuario_nome",
                    Json::texto_de(self.nome_do_usuario(d.usuario)),
                ),
                ("bytes", Json::de_u64(d.tamanho() as u64)),
                // Do CABECALHO, e nao do vetor: numa listagem leve o vetor
                // esta vazio, e dizer "0 anexos" para uma linha que tem tres
                // faria quem investiga concluir que a foto nunca existiu.
                ("anexos", Json::de_u64(d.n_externos as u64)),
                ("linha", linha),
                ("aviso", Json::texto_de(&aviso)),
            ]));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            ("bytes", Json::de_u64(bytes)),
            // A tela precisa saber se um campo externo vazio quer dizer "nao
            // tinha" ou "nao carreguei". Sao coisas diferentes.
            ("anexos_carregados", Json::Bool(com_anexos)),
            ("colunas", crate::valores::colunas_para_json(&esquema)),
            ("descartadas", Json::Lista(linhas)),
        ]))
    }

    /// `motivos`: por que cada linha foi excluida. **So administrador.**
    fn op_motivos(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 500).max(0) as u64;
        let so_do_rowid = p.campo("rowid").is_some();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let lista = if so_do_rowid {
            t.motivos_de(self.rowid(p)?)?
        } else {
            t.motivos(pular, limite)?
        };
        let total = t.total_de_motivos()?;
        let exige = t.esquema().motivo_obrigatorio();

        let registros = lista
            .iter()
            .map(|m| {
                Json::objeto(vec![
                    ("uuid", Json::texto_de(m.uuid.to_string())),
                    ("rowid", Json::de_u64(m.rowid)),
                    ("quando", Json::texto_de(m.instante_iso())),
                    ("carimbo", Json::de_i64(m.carimbo)),
                    ("tipo", Json::texto_de(m.tipo.nome())),
                    ("motivo", Json::texto_de(&m.motivo)),
                    ("identidade", Json::texto_de(&m.identidade)),
                    ("usuario", Json::de_u64(m.usuario as u64)),
                    (
                        "usuario_nome",
                        Json::texto_de(self.nome_do_usuario(m.usuario)),
                    ),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            ("motivo_obrigatorio", Json::Bool(exige)),
            ("motivos", Json::Lista(registros)),
        ]))
    }

    /// `esvaziar_lixeira`: daqui nao volta. **So administrador.**
    ///
    /// O expurgo e registrado no `.reason` ANTES de a lixeira ser apagada: o
    /// motivo tem de sobreviver ao dado, senao o rastro some junto com ele.
    fn op_esvaziar_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        if motivo.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"motivo\": esvaziar a lixeira nao tem volta, e sem o \
                 registro do por que nao sobra rastro nenhum"
                    .into(),
            ));
        }
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let apagadas = t.esvaziar_lixeira(&motivo)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("apagadas", Json::de_u64(apagadas)),
        ]))
    }

    /// Copia de seguranca, com a trava de dados segurada do inicio ao fim.
    ///
    /// `"zip": true` faz um arquivo unico chamado
    /// `Banco_Admin_Data_HoraMin.zip`, com o manifesto dentro. Sem isso,
    /// copia a arvore de diretorios como antes.
    fn op_backup(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \"destino\"".into()));
        }
        let quando = crate::agora_ms();
        let inicio = Instant::now();
        let em_zip = p.booleano_ou("zip", false);
        let banco = p.texto_ou("database", "").trim().to_string();
        // Quem fez entra no nome do arquivo. Sem login, entrou pelo token de
        // servico -- e o nome diz isso, em vez de fingir um usuario.
        let quem = if sessao.login().is_empty() {
            "servico".to_string()
        } else {
            sessao.login().to_string()
        };

        // A trava fica presa a copia inteira. E o que "consistente" quer dizer
        // sem transacao: nenhuma escrita acontece no meio.
        let (arquivo, r) = {
            let _trava = self.travar_dados()?;
            if em_zip {
                let (caminho, r) = phxsql_store::backup::executar_zip(
                    &self.config.base,
                    std::path::Path::new(&destino),
                    &banco,
                    &quem,
                    quando,
                )?;
                (Some(caminho.display().to_string()), r)
            } else {
                (
                    None,
                    phxsql_store::backup::executar(
                        &self.config.base,
                        std::path::Path::new(&destino),
                        &phxsql_core::datahora::instante_iso(quando),
                    )?,
                )
            }
        };

        let mut campos = vec![
            ("destino", Json::texto_de(&destino)),
            ("arquivos", Json::de_u64(r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
        ];
        if let Some(a) = arquivo {
            campos.push(("arquivo", Json::texto_de(a)));
            campos.push(("comprimido", Json::de_u64(r.comprimido)));
            campos.push((
                "reducao_pct",
                Json::de_u64(if r.bytes > 0 {
                    100 - (r.comprimido * 100 / r.bytes).min(100)
                } else {
                    0
                }),
            ));
        }
        Ok(Json::objeto(campos))
    }

    /// Confere `.reg` contra `.bkp` e conserta o que der.
    fn op_reparar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let (conferidos, reparados, perdidos) = t.reparar()?;
        self.gravar_de_verdade(&mut t, p)?;
        Ok(Json::objeto(vec![
            ("conferidos", Json::de_u64(conferidos)),
            ("reparados", Json::de_u64(reparados)),
            ("perdidos", Json::de_u64(perdidos)),
            ("integro", Json::Bool(perdidos == 0)),
        ]))
    }

    fn op_conferir_backup(&self, p: &Json) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \"destino\"".into()));
        }
        let r = phxsql_store::backup::conferir(std::path::Path::new(&destino))?;
        Ok(Json::objeto(vec![
            ("destino", Json::texto_de(&destino)),
            ("integro", Json::Bool(r.ok())),
            ("arquivos", Json::de_u64(r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            (
                "divergencias",
                Json::Lista(r.divergencias.iter().map(Json::texto_de).collect()),
            ),
        ]))
    }

    /// A impressao digital de uma tabela, para comparar duas copias.
    ///
    /// # Para que serve
    ///
    /// Responder "estas duas tabelas sao a mesma?" sem transportar as duas.
    /// E o que falta para conferir uma replica contra a origem, e para provar
    /// que um backup restaurado ficou igual ao original -- hoje o
    /// `conferir-backup` compara ARQUIVO, e arquivo igual e mais forte do que
    /// preciso: dois `.reg` podem diferir no enchimento e ter o mesmo dado.
    ///
    /// # Como a conta e feita
    ///
    /// CRC-32 de cada linha viva, dobrado num acumulador que **depende da
    /// ordem**. Depender da ordem e de proposito: no PhxSql a ordem de
    /// digitacao E o dado, e duas tabelas com as mesmas linhas em ordem
    /// diferente nao sao a mesma tabela.
    ///
    /// Slot excluido nao entra. Se entrasse, restaurar um backup daria outro
    /// numero so porque os buracos caem em outro lugar.
    fn op_checksum(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let comeco = Instant::now();
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut soma: u64 = 0xcbf2_9ce4_8422_2325; // semente do FNV-1a de 64
        let mut linhas = 0u64;
        // PONTO DE CANCELAMENTO. A soma de verificacao le a tabela inteira
        // segurando a trava de dados: e a operacao que mais para o servidor
        // sem gravar nada. Abandonar entre duas linhas nao deixa rastro
        // nenhum -- nada foi escrito --, e por isso ela e cancelavel do
        // comeco ao fim.
        let atividade = crate::telemetria::corrente();
        let _fase = atividade
            .as_ref()
            .map(|a| a.fase_cancelavel("somando a tabela"));
        for (rowid, _) in t.varrer()? {
            if let Some(a) = &atividade {
                a.siga(1)?;
            }
            let Some(linha) = t.ler(rowid)? else { continue };
            // A linha volta a forma canonica antes de entrar na conta: somar o
            // byte cru do slot faria o enchimento de um `Str` de largura fixa
            // pesar, e duas tabelas iguais com larguras diferentes dariam
            // numeros diferentes.
            let mut texto = String::with_capacity(64);
            for (v, c) in linha.iter().zip(esquema.colunas()) {
                texto.push('\u{1}');
                if v.e_null() {
                    texto.push('\u{0}');
                } else {
                    texto.push_str(&crate::valores::valor_para_json(v, &c.ty).escrever());
                }
            }
            let crc = phxsql_core::crc::crc32(texto.as_bytes()) as u64;
            // Multiplicar antes de somar e o que faz a ordem contar: trocar
            // duas linhas de lugar muda o resultado.
            soma = (soma ^ crc).wrapping_mul(0x1000_0000_01b3);
            linhas += 1;
        }

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("checksum", Json::texto_de(format!("{soma:016x}"))),
            ("linhas", Json::de_u64(linhas)),
            ("slots", Json::de_u64(t.registros())),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// Quem esta falando com o servidor agora.
    ///
    /// E o `SHOW PROCESSLIST`: sem ele, quando uma consulta prende a trava de
    /// dados nao havia como saber QUEM esta segurando -- so que estava lento.
    fn op_sessoes(&self) -> Result<Json> {
        let agora = crate::agora_ms();
        let l = self.ligacoes.lock().map_err(|_| trava_envenenada())?;
        let todas = l.todas();
        // A mais demorada primeiro: quando algo trava, e ela que interessa.
        let mais_longa = todas
            .iter()
            .filter(|x| x.op_desde_ms > 0)
            .map(|x| agora - x.op_desde_ms)
            .max()
            .unwrap_or(0);
        // As sessoes do navegador entram na MESMA lista. Quem pergunta "quem
        // esta conectado?" quer os dois -- e uma lista que so mostra a porta de
        // dados nao mostra quem esta olhando a propria tela.
        let web: Vec<Json> = self
            .sessoes
            .lock()
            .map(|s| {
                s.listar(agora)
                    .into_iter()
                    .map(|(id, login, desde, expira)| {
                        Json::objeto(vec![
                            ("id", Json::texto_de(&id)),
                            ("origem", Json::texto_de("web")),
                            (
                                "usuario",
                                match login.is_empty() {
                                    true => Json::Nulo,
                                    false => Json::texto_de(login),
                                },
                            ),
                            (
                                "desde",
                                Json::texto_de(phxsql_core::datahora::instante_iso(desde)),
                            ),
                            (
                                "aberta_s",
                                Json::de_u64(((agora - desde) / 1_000).max(0) as u64),
                            ),
                            (
                                "expira_em_s",
                                Json::de_u64(((expira - agora) / 1_000).max(0) as u64),
                            ),
                        ])
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Json::objeto(vec![
            ("quantas", Json::de_u64(todas.len() as u64)),
            (
                "executando",
                Json::de_u64(todas.iter().filter(|x| !x.op.is_empty()).count() as u64),
            ),
            ("mais_longa_ms", Json::de_u64(mais_longa.max(0) as u64)),
            (
                "sessoes",
                Json::Lista(
                    todas
                        .iter()
                        .map(|x| {
                            let mut j = x.para_json(agora);
                            if let Json::Objeto(campos) = &mut j {
                                campos.push(("origem".into(), Json::texto_de("dados")));
                            }
                            j
                        })
                        .collect(),
                ),
            ),
            ("web", Json::Lista(web.clone())),
            ("sessoes_web", Json::de_u64(web.len() as u64)),
        ]))
    }

    // ------------------------------------------------------- telemetria

    /// O portao PROPRIO das operacoes de telemetria.
    ///
    /// # Por que ele existe, se ja ha o portao do `despachar`
    ///
    /// E a licao do `juntar`/`unir`, com o sinal trocado. La, o portao geral
    /// olhava o campo `"tabela"` e duas operacoes nao o tinham -- entao elas
    /// escapavam. Aqui a telemetria tambem nao tem `"tabela"` NEM
    /// `"database"`: o portao geral so consegue perguntar «este usuario pode
    /// administrar a base vazia?», e a resposta disso cai na regra `"*"` ou no
    /// nivel. Um usuario com `bases: {"*": {administrar: true}}` e nivel de
    /// leitor passaria.
    ///
    /// E a telemetria mostra, de todo mundo: o login, o IP, a operacao e a
    /// TABELA em que ela mexe. Quem ve isso ve o movimento de bases sobre as
    /// quais nao tem direito nenhum. Entao esta operacao pergunta o que o
    /// portao geral nao consegue perguntar: **e administrador deste servidor?**
    ///
    /// Sem cadastro de usuarios, quem entrou pelo token de servico continua
    /// podendo -- e assim que toda operacao de administracao ja funciona, e
    /// apertar isso aqui tiraria um direito que ninguem pediu para tirar.
    fn portao_da_telemetria(&self, sessao: &Sessao) -> Result<()> {
        match &sessao.usuario {
            None => Ok(()),
            Some(u) if u.e_admin() => Ok(()),
            Some(u) => Err(PhxError::Autorizacao(format!(
                "{} nao e administrador deste servidor; a telemetria mostra o \
                 login, o IP e a tabela de todas as atividades",
                u.login
            ))),
        }
    }

    /// O painel de telemetria: as series do topo e as atividades vivas.
    ///
    /// Uma chamada so, como o `painel` e o `sistema`, e pelo mesmo motivo: a
    /// tela se atualiza sozinha, e tres idas e voltas por volta seriam tres
    /// vezes o custo para desenhar uma coisa so.
    fn op_telemetria(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        let amostras = p.inteiro_ou("amostras", 120).clamp(1, 200) as usize;
        let agora = crate::agora_ms();
        let mut retrato = self.telemetria.para_json(agora, amostras);
        if let Json::Objeto(campos) = &mut retrato {
            let (acertos, faltas, gravacoes) = phxsql_store::ndx::contadores_de_cache();
            campos.push((
                "cache_ndx".into(),
                Json::objeto(vec![
                    (
                        "paginas_teto",
                        Json::de_u64(phxsql_store::ndx::cache_paginas() as u64),
                    ),
                    ("acertos", Json::de_u64(acertos)),
                    ("faltas", Json::de_u64(faltas)),
                    ("gravacoes", Json::de_u64(gravacoes)),
                    (
                        "acerto_percentual",
                        Json::texto_de(format!(
                            "{:.2}",
                            match acertos + faltas {
                                0 => 0.0,
                                t => acertos as f64 / t as f64 * 100.0,
                            }
                        )),
                    ),
                ]),
            ));
            campos.push((
                "servidor".into(),
                Json::objeto(vec![
                    ("phxsql", Json::texto_de(VERSAO)),
                    (
                        "conexoes",
                        Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                    ),
                    (
                        "conexoes_max",
                        Json::de_u64(self.config.conexoes_max as u64),
                    ),
                    (
                        "no_ar_s",
                        Json::de_u64(((agora - self.desde_ms) / 1_000).max(0) as u64),
                    ),
                    ("gravacoes_pendentes", Json::de_u64(self.janela.pendente())),
                ]),
            ));
        }
        Ok(retrato)
    }

    /// Liga a coleta. Desligada ela custa um `load(Relaxed)` por ponto.
    fn op_telemetria_ligar(&self, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        let agora = crate::agora_ms();
        self.telemetria.ligar(agora);
        Ok(Json::objeto(vec![
            ("ligada", Json::Bool(true)),
            (
                "periodo_ms",
                Json::de_u64(crate::telemetria::PERIODO_DA_AMOSTRA_MS),
            ),
            (
                "aviso",
                Json::texto_de(
                    "a serie comeca vazia e a primeira amostra nao tem taxa: taxa \
                     so existe entre dois instantes",
                ),
            ),
        ]))
    }

    /// Desliga a coleta e joga a serie fora.
    fn op_telemetria_desligar(&self, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        self.telemetria.desligar();
        Ok(Json::objeto(vec![
            ("ligada", Json::Bool(false)),
            (
                "aviso",
                Json::texto_de(
                    "a serie foi descartada: um grafico com buraco no meio mente \
                     sobre o que aconteceu ali",
                ),
            ),
        ]))
    }

    /// Encerra a operacao em curso de UMA atividade -- o cancelamento
    /// cooperativo.
    ///
    /// # O que ele promete, e o que ele NAO promete
    ///
    /// Ele marca. A marca so e olhada em ponto seguro, e por isso a resposta
    /// diz em qual dos tres casos o pedido caiu:
    ///
    /// * `encerrando` -- a operacao esta numa fase cancelavel e vai abortar na
    ///   proxima unidade de trabalho;
    /// * `nao_cancelavel` -- ela esta dentro do ponto critico e vai TERMINAR;
    ///   a marca fica posta, mirando esta mesma operacao, e vale se ela ainda
    ///   entrar numa fase cancelavel antes do fim;
    /// * `ociosa` -- nao havia nada em curso.
    ///
    /// Prometer um `KILL` instantaneo seria mentir, e a mentira apareceria no
    /// pior momento: com o operador olhando para uma tela que diz «encerrada»
    /// enquanto a tabela continua sendo escrita.
    fn op_telemetria_encerrar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        let id = p.texto_ou("id", "").trim().to_string();
        if id.is_empty() {
            return Err(PhxError::Esquema(
                "encerrar sem \"id\": ele vem da lista de `telemetria`, na forma \
                 dados:17 ou web:a1b2c3d4"
                    .into(),
            ));
        }
        let quem = match &sessao.usuario {
            Some(u) => u.login.clone(),
            None => "token de servico".to_string(),
        };
        let atividade = self.telemetria.atividade(&id).ok_or_else(|| {
            PhxError::NaoEncontrado(format!(
                "nao ha atividade {id:?}; a lista esta em `telemetria`"
            ))
        })?;
        // Encerrar a propria atividade seria pedir para a tela se matar no
        // meio de perguntar -- e o pedido morreria antes de responder o que
        // aconteceu.
        if let Some(minha) = crate::telemetria::corrente() {
            if minha.chave == id {
                return Err(PhxError::Esquema(
                    "esta e a sua propria atividade: encerra-la mataria o pedido \
                     que esta perguntando"
                        .into(),
                ));
            }
        }
        let agora = crate::agora_ms();
        let desfecho = atividade.encerrar(&quem);
        let (estado, op_alvo, aviso) = match &desfecho {
            crate::telemetria::Encerramento::Ociosa => (
                "ociosa",
                String::new(),
                "nao havia operacao em curso. Para derrubar a CONEXAO inteira, \
                 use `encerrar_sessao` -- ele fecha o soquete."
                    .to_string(),
            ),
            crate::telemetria::Encerramento::Marcada { op, fase } => (
                "encerrando",
                op.clone(),
                format!(
                    "a operacao esta em fase cancelavel ({}) e aborta na proxima \
                     unidade de trabalho. O que ja foi gravado continua gravado e \
                     o arquivo fica integro.",
                    if fase.is_empty() { "sem nome" } else { fase }
                ),
            ),
            crate::telemetria::Encerramento::Posta { op } => (
                "marcada",
                op.clone(),
                "esta operacao TEM ponto de cancelamento, mas nao esta nele \
                 agora -- tipicamente porque esta na fila da trava de dados. A \
                 marca fica posta e vale para o primeiro ponto seguro que \
                 vier; se ela ja tiver passado do ultimo, a operacao termina \
                 normalmente."
                    .to_string(),
            ),
            crate::telemetria::Encerramento::FaseNaoCancelavel { op } => (
                "nao_cancelavel",
                op.clone(),
                "esta operacao esta DENTRO do ponto critico e vai terminar: \
                 abandonar uma gravacao entre o slot e o indice deixaria a \
                 tabela mentindo. A marca fica posta e vale se ela ainda entrar \
                 numa fase cancelavel antes do fim."
                    .to_string(),
            ),
        };
        self.telemetria.contar_encerramento();
        // Vai para o log de acessos, e nao so para a resposta: derrubar o
        // trabalho de outra pessoa e um ato de administracao, e ato de
        // administracao tem de deixar rastro de quem fez o que e quando.
        self.anotar(&Acesso {
            quando_ms: agora,
            ip: String::new(),
            porta_origem: 0,
            op: "telemetria_encerrar".into(),
            usuario: quem.clone(),
            autenticado: true,
            ok: true,
            duracao_ms: 0,
            erro: Some(format!(
                "encerrar {id} ({}) -> {estado}",
                if op_alvo.is_empty() {
                    "sem operacao"
                } else {
                    &op_alvo
                }
            )),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });
        Ok(Json::objeto(vec![
            ("id", Json::texto_de(&id)),
            ("estado", Json::texto_de(estado)),
            (
                "op",
                match op_alvo.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&op_alvo),
                },
            ),
            ("quem", Json::texto_de(&quem)),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
            ("aviso", Json::texto_de(aviso)),
        ]))
    }

    /// Derruba uma conexao pelo numero.
    ///
    /// E o `KILL` -- e o que ele alcanca esta dito na resposta, em vez de
    /// prometer mais do que faz: fecha o soquete, o que e imediato para a
    /// conexao parada esperando pedido. Uma operacao que ja entrou na trava de
    /// dados termina assim mesmo; o que muda e que o resultado nao vai para
    /// lugar nenhum e a conexao nao volta.
    fn op_encerrar_sessao(&self, p: &Json) -> Result<Json> {
        // Sessao do navegador vem por texto ("a1b2c3d4"); conexao da porta de
        // dados, por numero. Aceitar os dois no mesmo campo evita duas
        // operacoes para a mesma pergunta.
        if let Some(texto) = p.campo("id").and_then(Json::texto) {
            if texto.chars().any(|c| !c.is_ascii_digit()) {
                let mut s = self.sessoes.lock().map_err(|_| trava_envenenada())?;
                if !s.encerrar_por_prefixo(texto) {
                    return Err(PhxError::NaoEncontrado(format!(
                        "nao ha sessao web {texto:?}; a lista esta em `sessoes`"
                    )));
                }
                return Ok(Json::objeto(vec![
                    ("encerrada", Json::texto_de(texto)),
                    ("origem", Json::texto_de("web")),
                    ("estava", Json::texto_de("aberta")),
                    (
                        "aviso",
                        Json::texto_de(
                            "a sessao do navegador foi invalidada: o proximo clique cai no login",
                        ),
                    ),
                ]));
            }
        }
        let id = p.inteiro_ou("id", 0);
        if id <= 0 {
            return Err(PhxError::Esquema(
                "encerrar_sessao sem \"id\": o numero vem da operacao `sessoes`".into(),
            ));
        }
        let id = id as u64;
        let agora = crate::agora_ms();
        let mut l = self.ligacoes.lock().map_err(|_| trava_envenenada())?;
        let antes = l.todas().into_iter().find(|x| x.id == id);
        if !l.encerrar(id) {
            return Err(PhxError::NaoEncontrado(format!(
                "nao ha conexao {id}; a lista esta em `sessoes`"
            )));
        }
        let executando = antes.as_ref().map(|x| !x.op.is_empty()).unwrap_or(false);
        Ok(Json::objeto(vec![
            ("encerrada", Json::de_u64(id)),
            (
                "estava",
                Json::texto_de(if executando {
                    "executando"
                } else {
                    "esperando"
                }),
            ),
            (
                "op",
                match antes.as_ref().map(|x| x.op.clone()).unwrap_or_default() {
                    o if o.is_empty() => Json::Nulo,
                    o => Json::texto_de(o),
                },
            ),
            // Dito na resposta, e nao so na documentacao: quem manda encerrar
            // precisa saber se ja acabou ou se ainda vai acabar.
            (
                "aviso",
                Json::texto_de(if executando {
                    "a operacao em curso termina antes de a conexao fechar: nao ha como \
                     abandonar uma varredura no meio sem arriscar deixar a tabela aberta \
                     pela metade. O resultado nao vai para lugar nenhum"
                } else {
                    "a conexao estava esperando pedido e foi fechada na hora"
                }),
            ),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
        ]))
    }

    /// Exporta uma tabela, ou o resultado de uma varredura, em sete formatos.
    ///
    /// Binario (XLSX, DOCX) volta em base64, porque o protocolo e JSON por
    /// linha e byte cru nao atravessa. Texto volta como texto, para caber num
    /// `curl` sem ninguem ter de decodificar nada.
    fn op_exportar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let f = Formato::de_texto(p.texto_ou("formato", "csv"))?;
        let comeco = Instant::now();
        // Teto proprio, e maior que o da varredura: exportar e justamente o
        // caso em que se quer a tabela inteira, e nao a primeira pagina.
        let teto = p.inteiro_ou("max", 100_000).clamp(1, 1_000_000) as usize;

        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut linhas: Vec<Vec<Value>> = Vec::new();
        let mut truncado = false;
        // PONTO DE CANCELAMENTO. So a VARREDURA e cancelavel: dela para
        // frente o formato ja esta sendo montado em memoria, e abandonar no
        // meio da montagem so jogaria fora trabalho ja feito sem soltar a
        // trava mais cedo.
        let atividade = crate::telemetria::corrente();
        {
            let _fase = atividade
                .as_ref()
                .map(|a| a.fase_cancelavel("lendo a tabela para exportar"));
            for (rowid, _) in t.varrer()? {
                if let Some(a) = &atividade {
                    a.siga(1)?;
                }
                if linhas.len() >= teto {
                    truncado = true;
                    break;
                }
                if let Some(l) = t.ler(rowid)? {
                    linhas.push(l);
                }
            }
        }

        let nome = p.texto_ou("tabela", "tabela");
        let base = p.texto_ou("database", "");
        let planilha = crate::exportar::Planilha {
            titulo: nome.to_string(),
            subtitulo: format!(
                "{base} · {} linha(s) · exportado em {}",
                linhas.len(),
                phxsql_core::datahora::instante_iso(crate::agora_ms())
            ),
            colunas: crate::exportar::Planilha::do_esquema(&esquema, nome),
            linhas: &linhas,
        };
        let bytes = planilha.gerar(f)?;

        // O nome do arquivo sai daqui e nao da tela: quem chama por `curl` tem
        // o mesmo nome que quem clica, e o nome carrega a data.
        let arquivo = format!(
            "{}_{}.{}",
            nome.replace('.', "_"),
            phxsql_core::datahora::instante_iso(crate::agora_ms()).replace([' ', ':', ','], "-"),
            f.extensao()
        );

        let mut campos = vec![
            ("formato", Json::texto_de(p.texto_ou("formato", "csv"))),
            ("arquivo", Json::texto_de(&arquivo)),
            ("mime", Json::texto_de(f.mime())),
            ("bytes", Json::de_u64(bytes.len() as u64)),
            ("linhas", Json::de_u64(linhas.len() as u64)),
            ("truncado", Json::Bool(truncado)),
            ("binario", Json::Bool(f.binario())),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ];
        if f.binario() {
            campos.push((
                "base64",
                Json::texto_de(phxsql_core::base64::codificar(&bytes)),
            ));
        } else {
            campos.push((
                "conteudo",
                Json::texto_de(String::from_utf8_lossy(&bytes).to_string()),
            ));
        }
        Ok(Json::objeto(campos))
    }

    // ------------------------------------------------------- estatisticas

    /// O que o log ja sabia e ninguem perguntava.
    ///
    /// # Por que histograma, e nao media
    ///
    /// O painel mostrava "ms medio". Media esconde exatamente o que interessa:
    /// mil respostas de 1 ms e uma de 30 s dao media de 30 ms, e o numero
    /// parece bom enquanto alguem espera meio minuto. O que responde "esta
    /// rapido?" e a cauda -- a mediana, o percentil 95 e o pior caso.
    ///
    /// As faixas dobram (1, 2, 4, 8, 16... ms) porque a diferenca entre 1 ms e
    /// 2 ms importa tanto quanto entre 1 s e 2 s, e faixa de largura fixa
    /// esmagaria a metade rapida num balde so.
    fn op_estatisticas(&self, p: &Json) -> Result<Json> {
        let acessos = LogAcessos::ler(&self.config.log_acessos).unwrap_or_default();
        let desde = match p.inteiro_ou("horas", 0) {
            0 => 0,
            h => crate::agora_ms() - h.max(1) * 3_600_000,
        };
        let considerar: Vec<&Acesso> = acessos.iter().filter(|a| a.quando_ms >= desde).collect();

        // ------------------------------------------------ por operacao
        let mut por_op: HashMap<&str, Contagem> = HashMap::new();
        let mut por_tabela: HashMap<String, Contagem> = HashMap::new();
        let mut por_usuario: HashMap<&str, Contagem> = HashMap::new();
        let mut por_codigo: HashMap<u16, (u64, String)> = HashMap::new();
        let mut geral = Contagem::default();
        let mut duracoes: Vec<u64> = Vec::with_capacity(considerar.len());

        for a in &considerar {
            geral.somar(a);
            por_op.entry(a.op.as_str()).or_default().somar(a);
            if !a.usuario.is_empty() {
                por_usuario.entry(a.usuario.as_str()).or_default().somar(a);
            }
            // A tabela so entra quando o pedido nomeou uma. Contar "sem tabela"
            // como se fosse uma tabela poluiria a lista com o `ping`.
            if !a.tabela.is_empty() {
                let chave = if a.database.is_empty() {
                    a.tabela.clone()
                } else {
                    format!("{}.{}", a.database, a.tabela)
                };
                por_tabela.entry(chave).or_default().somar(a);
            }
            if let (false, Some(e)) = (a.ok, a.erro.as_ref()) {
                let entrada = por_codigo.entry(a.codigo).or_insert((0, e.clone()));
                entrada.0 += 1;
            }
            duracoes.push(a.duracao_ms);
        }
        duracoes.sort_unstable();

        // As mais demoradas, com nome e objeto. E o registro de consulta lenta
        // do MySQL(R), so que sem precisar ligar nada: o log ja tinha o dado, e
        // faltava a pergunta.
        let mut mais_lentas: Vec<&Acesso> = considerar.clone();
        mais_lentas.sort_by(|a, b| b.duracao_ms.cmp(&a.duracao_ms));
        mais_lentas.truncate(15);

        let percentil = |q: f64| -> u64 {
            if duracoes.is_empty() {
                return 0;
            }
            // Percentil pelo metodo do vizinho mais proximo: com poucas
            // amostras, interpolar inventa um valor que ninguem mediu.
            let i = ((duracoes.len() as f64 - 1.0) * q).round() as usize;
            duracoes[i.min(duracoes.len() - 1)]
        };

        // -------------------------------------------------- histograma
        let mut faixas: Vec<(u64, u64, u64)> = Vec::new();
        let mut teto = 1u64;
        while teto <= 65_536 {
            let piso = if teto == 1 { 0 } else { teto / 2 };
            let quantas = duracoes
                .iter()
                .filter(|d| **d >= piso && **d < teto)
                .count() as u64;
            faixas.push((piso, teto, quantas));
            teto *= 2;
        }
        let acima = duracoes.iter().filter(|d| **d >= 65_536).count() as u64;

        let lista = |mut v: Vec<(String, Contagem)>| -> Json {
            v.sort_by(|a, b| b.1.quantas.cmp(&a.1.quantas));
            v.truncate(30);
            Json::Lista(v.iter().map(|(n, c)| c.para_json(n)).collect())
        };

        Ok(Json::objeto(vec![
            (
                "desde",
                match desde {
                    0 => Json::texto_de("sempre"),
                    d => Json::texto_de(phxsql_core::datahora::instante_iso(d)),
                },
            ),
            ("acessos", Json::de_u64(geral.quantas)),
            ("resumo", geral.para_json("tudo")),
            (
                "latencia",
                Json::objeto(vec![
                    ("p50", Json::de_u64(percentil(0.50))),
                    ("p90", Json::de_u64(percentil(0.90))),
                    ("p95", Json::de_u64(percentil(0.95))),
                    ("p99", Json::de_u64(percentil(0.99))),
                    ("pior", Json::de_u64(duracoes.last().copied().unwrap_or(0))),
                ]),
            ),
            (
                "histograma",
                Json::Lista(
                    faixas
                        .iter()
                        .map(|(piso, teto, n)| {
                            Json::objeto(vec![
                                ("de_ms", Json::de_u64(*piso)),
                                ("ate_ms", Json::de_u64(*teto)),
                                ("quantas", Json::de_u64(*n)),
                            ])
                        })
                        .chain(std::iter::once(Json::objeto(vec![
                            ("de_ms", Json::de_u64(65_536)),
                            ("ate_ms", Json::Nulo),
                            ("quantas", Json::de_u64(acima)),
                        ])))
                        .collect(),
                ),
            ),
            (
                "por_operacao",
                lista(
                    por_op
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                ),
            ),
            ("por_tabela", lista(por_tabela.into_iter().collect())),
            (
                "por_usuario",
                lista(
                    por_usuario
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                ),
            ),
            (
                "mais_lentas",
                Json::Lista(
                    mais_lentas
                        .iter()
                        .map(|a| {
                            Json::objeto(vec![
                                ("quando", Json::texto_de(a.quando())),
                                ("op", Json::texto_de(&a.op)),
                                ("ms", Json::de_u64(a.duracao_ms)),
                                ("usuario", Json::texto_de(&a.usuario)),
                                (
                                    "objeto",
                                    match (a.database.is_empty(), a.tabela.is_empty()) {
                                        (true, true) => Json::Nulo,
                                        (false, true) => Json::texto_de(&a.database),
                                        (true, false) => Json::texto_de(&a.tabela),
                                        _ => Json::texto_de(format!("{}.{}", a.database, a.tabela)),
                                    },
                                ),
                                ("ok", Json::Bool(a.ok)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_erro",
                Json::Lista({
                    let mut v: Vec<(u16, (u64, String))> = por_codigo.into_iter().collect();
                    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
                    v.truncate(15);
                    v.iter()
                        .map(|(codigo, (n, exemplo))| {
                            Json::objeto(vec![
                                ("codigo", Json::de_u64(*codigo as u64)),
                                // Zero nao e um erro: e uma linha gravada
                                // antes de o codigo existir. Chama-lo de
                                // "codigo 0" faria parecer um erro novo.
                                (
                                    "nome",
                                    Json::texto_de(match codigo {
                                        0 => "(log anterior ao codigo)",
                                        1001 => "CORROMPIDO",
                                        1002 => "ASSINATURA_INVALIDA",
                                        1003 => "VERSAO_NAO_SUPORTADA",
                                        2001 => "ESQUEMA_INVALIDO",
                                        2002 => "TIPO_INVALIDO",
                                        3001 => "NAO_ENCONTRADO",
                                        3002 => "DUPLICADO",
                                        3003 => "LIMITE_EXCEDIDO",
                                        4001 => "ACESSO_NEGADO",
                                        5001 => "ERRO_DE_ES",
                                        _ => "?",
                                    }),
                                ),
                                ("quantas", Json::de_u64(*n)),
                                ("exemplo", Json::texto_de(exemplo)),
                            ])
                        })
                        .collect()
                }),
            ),
        ]))
    }

    // -------------------------------------------------- junção e união

    /// Resolve as colunas da chave de um lado, aceitando nome ou lista.
    fn chave_do_lado(esquema: &Schema, j: &Json, campo: &str, tabela: &str) -> Result<Vec<usize>> {
        let nomes: Vec<String> = match j.campo(campo) {
            Some(Json::Lista(l)) => l
                .iter()
                .filter_map(|x| x.texto().map(str::to_string))
                .collect(),
            Some(outro) => outro
                .texto()
                .map(|t| vec![t.to_string()])
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if nomes.is_empty() {
            // Sem chave dita, a chave primaria e a escolha obvia -- e a unica
            // que nao e chute. Juntar pela primeira coluna daria numero errado
            // calado, que e pior do que recusar.
            let pk = esquema.chave_primaria().ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{tabela} nao tem chave primaria; diga por qual coluna juntar em {campo:?}"
                ))
            })?;
            return Ok(pk.colunas.iter().map(|ic| ic.coluna).collect());
        }
        nomes
            .iter()
            .map(|n| {
                posicao_da_coluna(esquema, n).ok_or_else(|| {
                    PhxError::Esquema(format!("a coluna {n:?} nao existe em {tabela}"))
                })
            })
            .collect()
    }

    /// Le uma tabela inteira para a memoria, com teto.
    ///
    /// O lado do mapa cabe na memoria ou a junção nao acontece. Recusar com o
    /// numero na mensagem e melhor do que engasgar a maquina.
    fn materializar(t: &mut Table, nome: &str) -> Result<Vec<Vec<Value>>> {
        let rowids = t.varrer()?;
        if rowids.len() > TETO_JUNCAO {
            return Err(PhxError::LimiteExcedido(format!(
                "{nome} tem {} linhas, acima do teto de {TETO_JUNCAO} para o lado que \
                 entra na memoria. Troque a ordem dos lados ou filtre antes",
                rowids.len()
            )));
        }
        let mut v = Vec::with_capacity(rowids.len());
        for (rowid, _) in rowids {
            if let Some(l) = t.ler(rowid)? {
                v.push(l);
            }
        }
        Ok(v)
    }

    /// As sete figuras do diagrama, entre duas tabelas.
    fn op_juntar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let tipo = TipoJuncao::de_texto(p.texto_ou("tipo", "interna"))?;
        let max = self.limite_pivot(p);
        let comeco = Instant::now();

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(p.texto_ou("database", ""))?;

        let (pa, pb) = (
            p.campo("a")
                .ok_or_else(|| PhxError::Esquema("junção sem o lado \"a\"".into()))?,
            p.campo("b")
                .ok_or_else(|| PhxError::Esquema("junção sem o lado \"b\"".into()))?,
        );
        let (na, nb) = (pa.texto_ou("tabela", ""), pb.texto_ou("tabela", ""));
        let mut ta = db.abrir_qualificada(na)?;
        let mut tb = db.abrir_qualificada(nb)?;
        let (ea, eb) = (ta.esquema().clone(), tb.esquema().clone());

        // O portao geral confere o campo `tabela` do pedido -- e uma junção
        // NAO TEM esse campo: as duas tabelas moram em `a.tabela` e
        // `b.tabela`. Sem esta conferencia, juntar seria a porta dos fundos
        // para ler uma tabela negada, bastando pedi-la como o lado B.
        if let Some(u) = &sessao.usuario {
            let base = p.texto_ou("database", "");
            for alvo in [na, nb] {
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }

        let lado = |esquema: Schema, j: &Json, nome: &str| -> Result<Lado> {
            let chave = Self::chave_do_lado(&esquema, j, "chave", nome)?;
            let prefixo = match j.texto_ou("prefixo", "").trim() {
                "" => nome.rsplit('.').next().unwrap_or(nome).to_string(),
                outro => outro.to_string(),
            };
            Ok(Lado {
                prefixo,
                esquema,
                chave,
            })
        };
        let la = lado(ea, pa, na)?;
        let lb = lado(eb, pb, nb)?;
        if la.prefixo == lb.prefixo {
            return Err(PhxError::Esquema(format!(
                "os dois lados usariam o prefixo {:?}; dê um \"prefixo\" a um deles \
                 para as colunas nao se sobreporem na saida",
                la.prefixo
            )));
        }
        crate::juncao::conferir_chaves(&la, &lb)?;

        // Quem entra na memoria e quem NAO precisa sair inteiro. Num RIGHT, o
        // lado que precisa inteiro e B, entao A vira mapa e B streama.
        let r = match tipo.trocando_os_lados() {
            Some(espelho) => {
                let memoria = Self::materializar(&mut ta, na)?;
                let mut fluxo = LinhasDaTabela {
                    rowids: tb
                        .varrer()?
                        .into_iter()
                        .map(|(r, _)| r)
                        .collect::<Vec<_>>()
                        .into_iter(),
                    tabela: &mut tb,
                };
                crate::juncao::juntar(&mut fluxo, &lb, &memoria, &la, espelho, true, max)?
            }
            None => {
                let memoria = Self::materializar(&mut tb, nb)?;
                let mut fluxo = LinhasDaTabela {
                    rowids: ta
                        .varrer()?
                        .into_iter()
                        .map(|(r, _)| r)
                        .collect::<Vec<_>>()
                        .into_iter(),
                    tabela: &mut ta,
                };
                crate::juncao::juntar(&mut fluxo, &la, &memoria, &lb, tipo, false, max)?
            }
        };

        Ok(Json::objeto(vec![
            ("tipo", Json::texto_de(tipo.nome())),
            ("sql", Json::texto_de(tipo.sql())),
            ("a", Json::texto_de(na)),
            ("b", Json::texto_de(nb)),
            ("colunas", colunas_da_juncao(&r.colunas)),
            (
                "linhas",
                Json::Lista(
                    r.linhas
                        .iter()
                        .map(|l| {
                            Json::Lista(
                                l.iter()
                                    .zip(r.colunas.iter())
                                    .map(|(v, c)| crate::valores::valor_para_json(v, &c.ty))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            ("quantas", Json::de_u64(r.linhas.len() as u64)),
            ("lidas_a", Json::de_u64(r.lidas_esquerda)),
            ("lidas_b", Json::de_u64(r.lidas_direita)),
            // Contadas e devolvidas de proposito: um INNER que trouxe menos do
            // que se esperava costuma ter aqui a explicacao, e sem o numero ela
            // vira meia hora de investigacao.
            ("chave_nula_a", Json::de_u64(r.chave_nula_esquerda)),
            ("chave_nula_b", Json::de_u64(r.chave_nula_direita)),
            ("truncado", Json::Bool(r.truncado)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    /// `UNION` e `UNION ALL` entre duas ou mais tabelas.
    fn op_unir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let modo = Uniao::de_texto(p.texto_ou("modo", "distinta"))?;
        let max = self.limite_pivot(p);
        let comeco = Instant::now();
        let base = p.texto_ou("database", "");

        let nomes: Vec<String> = p
            .campo("tabelas")
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .filter_map(|x| {
                        x.texto()
                            .map(str::to_string)
                            .or_else(|| x.campo("tabela").and_then(Json::texto).map(str::to_string))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if nomes.len() < 2 {
            return Err(PhxError::Esquema(
                "a união precisa de ao menos duas tabelas em \"tabelas\"".into(),
            ));
        }

        // A conferencia vem DEPOIS de ler a lista, e nao antes, porque e a
        // lista que diz o que precisa ser conferido: o campo `tabela` que o
        // portao geral olha nao existe numa união. Cada tabela do pedido
        // precisa da sua propria permissao -- senao unir vira a porta dos
        // fundos para ler uma tabela negada.
        if let Some(u) = &sessao.usuario {
            for alvo in &nomes {
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(base)?;

        // As tabelas entram inteiras porque o `UNION` distinto precisa
        // comparar cada linha com todas as anteriores -- e porque abrir varias
        // tabelas e strearmar de todas ao mesmo tempo exigiria manter as
        // referencias vivas juntas, o que o emprestimo nao deixa aqui.
        let mut materias: Vec<(Vec<Vec<Value>>, Schema)> = Vec::with_capacity(nomes.len());
        for n in &nomes {
            let mut t = db.abrir_qualificada(n)?;
            let e = t.esquema().clone();
            materias.push((Self::materializar(&mut t, n)?, e));
        }

        let mut fontes: Vec<LinhasEmMemoria> = materias
            .iter()
            .map(|(l, _)| LinhasEmMemoria(l.clone().into_iter()))
            .collect();
        let esquemas: Vec<&Schema> = materias.iter().map(|(_, e)| e).collect();
        crate::juncao::conferir_uniao(&esquemas)?;

        let mut partes: Vec<(&mut dyn crate::pivot::Iterador, &Schema)> = fontes
            .iter_mut()
            .zip(esquemas.iter())
            .map(|(f, e)| (f as &mut dyn crate::pivot::Iterador, *e))
            .collect();
        let r = crate::juncao::unir(&mut partes, modo, max)?;

        Ok(Json::objeto(vec![
            ("modo", Json::texto_de(modo.nome())),
            ("sql", Json::texto_de(modo.sql())),
            (
                "tabelas",
                Json::Lista(nomes.iter().map(Json::texto_de).collect()),
            ),
            ("colunas", colunas_da_juncao(&r.colunas)),
            (
                "linhas",
                Json::Lista(
                    r.linhas
                        .iter()
                        .map(|l| {
                            Json::Lista(
                                l.iter()
                                    .zip(r.colunas.iter())
                                    .map(|(v, c)| crate::valores::valor_para_json(v, &c.ty))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            ("quantas", Json::de_u64(r.linhas.len() as u64)),
            (
                "por_parte",
                Json::Lista(r.por_parte.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            ("repetidas", Json::de_u64(r.repetidas)),
            ("truncado", Json::Bool(r.truncado)),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    // ------------------------------------------------------------- o DbLink

    /// As ligacoes cadastradas. A senha nunca vem junto.
    fn op_dblink(&self) -> Result<Json> {
        let r = self.dblink.lock().map_err(|_| trava_envenenada())?;
        Ok(Json::objeto(vec![
            ("arquivo", Json::texto_de(r.caminho.display().to_string())),
            (
                "ligacoes",
                Json::Lista(r.ligacoes.iter().map(Definicao::para_json).collect()),
            ),
            (
                "motores",
                Json::Lista(
                    [Motor::MySql, Motor::Postgres]
                        .iter()
                        .map(|m| {
                            Json::objeto(vec![
                                ("nome", Json::texto_de(m.nome())),
                                ("porta", Json::de_u64(m.porta_padrao() as u64)),
                                ("conecta", Json::Bool(m.conecta())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// Cria ou substitui uma ligacao.
    ///
    /// Sem o campo `senha`, a senha que ja estava fica. Isso e o que faz a tela
    /// de edicao funcionar: ela nunca RECEBE a senha, entao nao teria como
    /// devolve-la, e sem esta regra editar a porta apagaria a credencial.
    fn op_dblink_salvar(&self, p: &Json) -> Result<Json> {
        let mut r = self.dblink.lock().map_err(|_| trava_envenenada())?;
        let mut d = Definicao::de_json(p)?;
        if let Ok(antiga) = r.achar(&d.nome) {
            if p.campo("senha").is_none() && d.senha_env.is_empty() {
                d = d.com_a_senha_de(antiga);
            }
            // A tela salva sem mandar as sincronias; um salvar comum nao pode
            // apagar o que o assistente montou.
            if p.campo("sincronias").is_none() {
                d = d.com_as_sincronias_de(antiga);
            }
        }
        let ficha = d.para_json();
        r.salvar(d)?;
        Ok(Json::objeto(vec![
            ("gravado", Json::Bool(true)),
            ("ligacao", ficha),
        ]))
    }

    fn op_dblink_excluir(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").to_string();
        let mut r = self.dblink.lock().map_err(|_| trava_envenenada())?;
        r.excluir(&nome)?;
        Ok(Json::objeto(vec![
            ("excluido", Json::texto_de(nome)),
            ("restam", Json::de_u64(r.ligacoes.len() as u64)),
        ]))
    }

    /// Abre a ligacao pedida e devolve a conexao pronta.
    ///
    /// A definicao e COPIADA antes de conectar, e a trava do cadastro sai da
    /// mao: conectar leva ida e volta de rede, e segurar a trava enquanto isso
    /// travaria a tela de cadastro de todo mundo por causa de um host que nao
    /// responde.
    fn ligar(&self, p: &Json) -> Result<(Definicao, crate::dblink::Conexao)> {
        let d = {
            let r = self.dblink.lock().map_err(|_| trava_envenenada())?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        // `abrir` escolhe a conexao pelo motor da definicao -- e por aqui que
        // o PostgreSQL(R) entra sem que nenhuma operacao precise saber dele.
        let c = d.abrir()?;
        Ok((d, c))
    }

    fn op_dblink_testar(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::testar(&d, c)
    }

    /// As bases do outro servidor.
    fn op_dblink_bancos(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::bancos(&d, c)
    }

    /// As tabelas de uma base do outro servidor, com tamanho e comentario.
    fn op_dblink_tabelas(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::tabelas(&d, c, p)
    }

    /// A estrutura de uma tabela do outro servidor.
    fn op_dblink_estrutura(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::estrutura(&d, c, p)
    }

    /// O conteudo de uma tabela do outro servidor, para a grade.
    fn op_dblink_ler(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::ler(&d, c, p)
    }

    /// Uma instrucao escrita a mao contra o outro servidor.
    ///
    /// Duas travas antes de mandar, e as duas precisam ceder: a ligacao tem de
    /// nao ser somente-leitura E este servidor tambem nao. Um espelho nao vira
    /// caminho de escrita para o banco do outro so porque a ligacao permitia.
    fn op_dblink_consultar(&self, p: &Json) -> Result<Json> {
        let sql = p.texto_ou("sql", "").trim().to_string();
        if sql.is_empty() {
            return Err(PhxError::Esquema("dblink_consultar sem \"sql\"".into()));
        }
        // As duas travas vem ANTES de conectar: recusar depois de abrir a
        // conexao gasta uma ida a rede para dizer nao. A da ligacao precisa da
        // definicao, entao ela e achada primeiro, sem conectar.
        let d = {
            let r = self.dblink.lock().map_err(|_| trava_envenenada())?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        if !crate::dblink::so_consulta(&sql) {
            if d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a instrucao nao e consulta",
                    d.nome
                )));
            }
            if self.config.somente_leitura {
                return Err(PhxError::Autorizacao(
                    "este servidor esta em somente leitura: nao escreve nem pelo dblink".into(),
                ));
            }
        }
        let limite = p
            .inteiro_ou("limite", d.max_linhas as i64)
            .clamp(1, d.max_linhas as i64) as u64;
        let c = d.abrir()?;
        crate::dblink::operacoes::consultar(&d, c, &sql, limite)
    }

    /// `dblink_ligar`: o assistente liga tabelas primas — cria a tabela local
    /// espelhando a remota e registra a sincronia na definicao da ligacao.
    fn op_dblink_ligar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::dblink::sincronia;
        let pedidos = p
            .campo("tabelas")
            .and_then(Json::lista)
            .ok_or_else(|| PhxError::Esquema("informe \"tabelas\" como lista".into()))?;
        if pedidos.is_empty() {
            return Err(PhxError::Esquema("a lista \"tabelas\" veio vazia".into()));
        }

        let (mut d, mut c) = self.ligar(p)?;
        let dados = self.travar_dados()?;
        let mut ligadas = Vec::new();
        for t in pedidos {
            let mut sinc = sincronia::Sincronia::de_json(t)?;
            crate::dblink::nome_seguro(&sinc.remota)?;
            if sinc.local_database.trim().is_empty() {
                return Err(PhxError::Esquema(format!(
                    "sincronia de {:?} sem \"local_database\"",
                    sinc.remota
                )));
            }
            // O portao por tabela, no alvo LOCAL: esta operacao nao tem o
            // campo "tabela" que o portao comum le -- o mesmo furo do juntar.
            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Criar) {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de criar {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
            }
            // So os metadados: LIMIT 0 traz as colunas tipadas sem uma linha.
            let r = c.consultar(
                &format!(
                    "SELECT * FROM {} LIMIT 0",
                    crate::dblink::entre_crases(&sinc.remota)
                ),
                1,
            )?;
            let (esquema, chave) = sincronia::esquema_local_de(&sinc.local_tabela, &r.colunas)?;
            let db = dados.garantir_database(&sinc.local_database)?;
            let criada = match db.abrir_qualificada(&sinc.local_tabela) {
                Ok(existente) => {
                    // Tabela ja existente serve, desde que a chave case; o
                    // resto o mapa por nome confere a cada rodada.
                    drop(existente);
                    false
                }
                Err(_) => {
                    db.criar_tabela(None, esquema)?;
                    true
                }
            };
            sinc.chave = chave;
            d.sincronias.retain(|x| {
                !(x.remota.eq_ignore_ascii_case(&sinc.remota)
                    && x.local_database.eq_ignore_ascii_case(&sinc.local_database)
                    && x.local_tabela.eq_ignore_ascii_case(&sinc.local_tabela))
            });
            ligadas.push(Json::objeto(vec![
                ("remota", Json::texto_de(&sinc.remota)),
                ("local_database", Json::texto_de(&sinc.local_database)),
                ("local_tabela", Json::texto_de(&sinc.local_tabela)),
                ("chave", Json::texto_de(&sinc.chave)),
                ("sentido", Json::texto_de(sinc.sentido.nome())),
                ("dono", Json::texto_de(sinc.dono.nome())),
                ("tabela_criada", Json::Bool(criada)),
            ]));
            d.sincronias.push(sinc);
        }
        c.encerrar();
        drop(dados);
        let mut r = self.dblink.lock().map_err(|_| trava_envenenada())?;
        r.salvar(d)?;
        Ok(Json::objeto(vec![("ligadas", Json::Lista(ligadas))]))
    }

    /// `dblink_sincronizar`: uma rodada de convergencia das tabelas ligadas.
    ///
    /// E a operacao que o job agenda. Exclusao nao viaja, o conflito e por
    /// linha e quem vence e o dono -- o porque de cada limite esta no modulo
    /// `dblink::sincronia`.
    fn op_dblink_sincronizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::dblink::sincronia::{self, Sentido};
        use std::collections::HashMap;

        let so = p.texto_ou("tabela", "").trim().to_string();
        let (d, mut c) = self.ligar(p)?;
        if d.sincronias.is_empty() {
            return Err(PhxError::Esquema(format!(
                "a ligacao {:?} nao tem tabela ligada: rode o assistente do DbLink",
                d.nome
            )));
        }

        let dados = self.travar_dados()?;
        let mut relatorio = Vec::new();
        for sinc in &d.sincronias {
            if !so.is_empty()
                && !so.eq_ignore_ascii_case(&sinc.remota)
                && !so.eq_ignore_ascii_case(&sinc.local_tabela)
            {
                continue;
            }
            // Os portoes locais desta sincronia, conforme o que ela faz.
            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de ler {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
                if sinc.sentido != Sentido::Empurrar
                    && !(u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Inserir)
                        && u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Alterar))
                {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de gravar em {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
            }
            if sinc.sentido != Sentido::Puxar && d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a sincronia de {:?} \
                     empurra: tire o somente_leitura ou mude o sentido para puxar",
                    d.nome, sinc.remota
                )));
            }

            let db = dados.abrir_database(&sinc.local_database)?;
            let mut t = db.abrir_qualificada(&sinc.local_tabela).map_err(|_| {
                PhxError::NaoEncontrado(format!(
                    "a tabela local {}.{} nao existe: rode o assistente do DbLink",
                    sinc.local_database, sinc.local_tabela
                ))
            })?;
            t.definir_usuario(sessao.id());
            t.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);
            let esquema = t.esquema().clone();

            // O lado de la, inteiro -- com uma linha de sobra para saber se o
            // teto cortou. Sincronizar metade e fingir que acabou seria pior
            // que recusar.
            let teto = d.max_linhas;
            let r = c.consultar(
                &format!(
                    "SELECT * FROM {}",
                    crate::dblink::entre_crases(&sinc.remota)
                ),
                teto + 1,
            )?;
            if r.truncado || r.linhas.len() as u64 > teto {
                return Err(PhxError::LimiteExcedido(format!(
                    "a tabela remota {:?} passa das {} linhas da ligacao: suba o \
                     max_linhas ou sincronize por outra estrategia",
                    sinc.remota, teto
                )));
            }

            let negocio = sincronia::posicoes_de_negocio(&esquema);
            let mapa = sincronia::mapa_de_colunas(&esquema, &r.colunas)?;
            let chave_biz = negocio
                .iter()
                .position(|p| esquema.colunas()[*p].nome.eq_ignore_ascii_case(&sinc.chave))
                .ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "a chave {:?} sumiu da tabela local {}.{}",
                        sinc.chave, sinc.local_database, sinc.local_tabela
                    ))
                })?;
            let indice_da_chave = esquema
                .indices()
                .iter()
                .find(|i| {
                    i.unico && i.colunas.len() == 1 && i.colunas[0].coluna == negocio[chave_biz]
                })
                .map(|i| i.nome.clone())
                .ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "{}.{} nao tem indice UNICO na chave {:?} -- e ele que \
                         faz o upsert sem varrer",
                        sinc.local_database, sinc.local_tabela, sinc.chave
                    ))
                })?;

            let mut remotas = HashMap::new();
            for lr in &r.linhas {
                let lv = sincronia::linha_remota_para_negocio(&esquema, &negocio, &mapa, lr)?;
                remotas.insert(sincronia::chave_canonica(&lv[chave_biz]), lv);
            }
            let mut locais = HashMap::new();
            for (_rowid, linha) in t.varrer()? {
                let lv: Vec<_> = negocio.iter().map(|p| linha[*p].clone()).collect();
                locais.insert(sincronia::chave_canonica(&lv[chave_biz]), lv);
            }

            let plano = sincronia::plano(sinc.sentido, sinc.dono, &remotas, &locais);
            let (inseridas, alteradas) =
                sincronia::aplicar_para_ca(&mut t, &indice_da_chave, chave_biz, &plano.para_ca)?;
            t.sincronizar()?;

            let colunas_sql: Vec<(String, phxsql_core::types::ColumnType)> = negocio
                .iter()
                .map(|p| (esquema.colunas()[*p].nome.clone(), esquema.colunas()[*p].ty))
                .collect();
            let mut empurradas = 0u64;
            for sql in sincronia::sql_do_empurrao(&sinc.remota, &colunas_sql, &plano.para_la, 500)?
            {
                let r = c.consultar(&sql, 1)?;
                empurradas += r.afetadas;
            }

            relatorio.push(Json::objeto(vec![
                ("remota", Json::texto_de(&sinc.remota)),
                (
                    "local",
                    Json::texto_de(format!("{}.{}", sinc.local_database, sinc.local_tabela)),
                ),
                ("sentido", Json::texto_de(sinc.sentido.nome())),
                ("puxadas_novas", Json::de_u64(inseridas)),
                ("puxadas_alteradas", Json::de_u64(alteradas)),
                ("empurradas", Json::de_u64(plano.para_la.len() as u64)),
                ("linhas_afetadas_la", Json::de_u64(empurradas)),
                ("iguais", Json::de_u64(plano.iguais)),
                ("conflitos", Json::de_u64(plano.conflitos)),
            ]));
        }
        c.encerrar();
        if relatorio.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhuma sincronia casa com {so:?} na ligacao {:?}",
                d.nome
            )));
        }
        Ok(Json::objeto(vec![(
            "sincronizadas",
            Json::Lista(relatorio),
        )]))
    }

    // ----------------------------------------------------- a maquina embaixo

    /// Os caminhos cujo espaco em disco interessa a este servidor.
    ///
    /// O `base` sempre, porque e onde o dado mora. O destino do backup quando
    /// ha backup agendado -- e o disco que enche calado, porque ninguem olha
    /// para ele ate o dia em que o backup falha. E o que o operador acrescentar
    /// em `alertas.caminhos`.
    ///
    /// Repetido nao entra duas vezes; a mesma particao, sim: `base` e
    /// `backup.destino` podem cair na mesma montagem, e mostrar as duas linhas
    /// e o que responde "o disco do banco esta cheio?" sem obrigar ninguem a
    /// adivinhar qual montagem contem qual pasta.
    pub fn caminhos_vigiados(&self) -> Vec<PathBuf> {
        let mut v = vec![self.config.base.clone()];
        if self.config.backup.agendado {
            v.push(self.config.backup.destino.clone());
        }
        v.extend(self.config.alertas.caminhos.iter().cloned());
        v.dedup_by(|a, b| a == b);
        v
    }

    /// O retrato da maquina: CPU, memoria, discos, placas de rede e IO.
    ///
    /// Uma chamada so, pelo mesmo motivo do painel: cinco pedidos separados
    /// custariam cinco idas e voltas para mostrar um cabecalho.
    fn op_sistema(&self) -> Json {
        let caminhos = self.caminhos_vigiados();
        let refs: Vec<&Path> = caminhos.iter().map(|p| p.as_path()).collect();
        let mut retrato = match self.monitor.lock() {
            Ok(mut m) => m.ler(&refs),
            // Trava envenenada nao pode derrubar o painel: monitor e o que
            // alguem abre JUSTAMENTE quando algo ja deu errado.
            Err(_) => crate::sistema::Monitor::novo().ler(&refs),
        };
        // O limite entra junto com a medida: sem ele a tela teria de conhecer
        // a regra do config para saber que barra pintar de vermelho, e a regra
        // acabaria escrita em dois lugares.
        if let Json::Objeto(campos) = &mut retrato {
            campos.push((
                "alertas".into(),
                Json::objeto(vec![
                    ("ligado", Json::Bool(self.config.alertas.ligado)),
                    (
                        "livre_minimo_percentual",
                        Json::texto_de(format!(
                            "{:.2}",
                            self.config.alertas.livre_minimo_percentual
                        )),
                    ),
                    (
                        "livre_minimo_mb",
                        Json::de_u64(self.config.alertas.livre_minimo_mb),
                    ),
                    ("email", Json::Bool(self.config.alertas.email.ligado)),
                ]),
            ));
            campos.push((
                "apertados".into(),
                Json::Lista(
                    self.discos_apertados()
                        .iter()
                        .map(|e| Json::texto_de(&e.caminho))
                        .collect(),
                ),
            ));
        }
        retrato
    }

    /// Quais dos caminhos vigiados estao abaixo do limite.
    ///
    /// Responde mesmo com os alertas desligados: o limite continua sendo a
    /// regra de "apertado", e o painel pinta a barra de vermelho de qualquer
    /// jeito. Desligado quer dizer "nao manda e-mail", nao "nao olha".
    fn discos_apertados(&self) -> Vec<crate::sistema::EspacoEmDisco> {
        let caminhos = self.caminhos_vigiados();
        let refs: Vec<&Path> = caminhos.iter().map(|p| p.as_path()).collect();
        crate::sistema::espaco(&refs)
            .into_iter()
            .filter(|e| {
                self.config
                    .alertas
                    .apertado(e.livre_percentual(), e.livre_kb)
            })
            .collect()
    }

    /// Confere o espaco de tempos em tempos e avisa quando aperta.
    ///
    /// Thread propria porque a conferencia chama o `df` e, no caso do aviso,
    /// abre uma conexao TCP com o rele de e-mail -- nenhuma das duas coisas
    /// pode acontecer no caminho de uma consulta.
    fn ligar_vigia_de_disco(self: &Arc<Self>) {
        if !self.config.alertas.ligado {
            return;
        }
        let a = &self.config.alertas;
        eprintln!(
            "vigia de disco: a cada {} min | aperta abaixo de {:.0}% livre ou {} MB | {}",
            a.checar_minutos,
            a.livre_minimo_percentual,
            a.livre_minimo_mb,
            if a.email.ligado {
                format!("avisa {}", a.email.para.join(", "))
            } else {
                "so no painel (e-mail desligado)".to_string()
            }
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "vigia-disco",
            "chama o `df` de tempos em tempos e avisa quando o espaco aperta; \
             thread propria porque ela roda um programa do sistema e pode abrir \
             uma conexao com o rele de e-mail -- nenhuma das duas coisas cabe \
             no caminho de uma consulta",
            "servico",
            crate::agora_ms(),
            move |fio| {
                let intervalo = Duration::from_secs(servidor.config.alertas.checar_minutos * 60);
                loop {
                    fio.fazendo("conferindo o espaco em disco");
                    servidor.conferir_disco();
                    fio.fazendo(&format!(
                        "dormindo {} min ate a proxima conferencia",
                        servidor.config.alertas.checar_minutos
                    ));
                    std::thread::sleep(intervalo);
                }
            },
        );
    }

    /// Uma rodada do vigia: olha os discos, avisa o que estiver apertado.
    ///
    /// O silencio entre dois avisos do mesmo caminho e por caminho, e nao
    /// global: dois discos apertando no mesmo dia sao duas noticias, nao uma.
    fn conferir_disco(&self) {
        let apertados = self.discos_apertados();
        if apertados.is_empty() {
            // Aliviou: esquece o que ja foi avisado, para que a proxima vez
            // avise de novo em vez de ficar calado pelas horas do silencio.
            if let Ok(mut a) = self.avisados.lock() {
                a.clear();
            }
            return;
        }
        let agora = crate::agora_ms();
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        let novos: Vec<&crate::sistema::EspacoEmDisco> = {
            let Ok(mut vistos) = self.avisados.lock() else {
                return;
            };
            apertados
                .iter()
                .filter(|e| match vistos.get(&e.caminho) {
                    Some(quando) if agora - *quando < silencio => false,
                    _ => {
                        vistos.insert(e.caminho.clone(), agora);
                        true
                    }
                })
                .collect()
        };
        if novos.is_empty() {
            return;
        }
        for e in &novos {
            eprintln!(
                "DISCO APERTADO: {} ({}) -- {:.1}% livre, {} MB de espaco",
                e.caminho,
                e.montagem,
                e.livre_percentual(),
                e.livre_kb / 1_024
            );
        }
        if !self.config.alertas.email.ligado {
            return;
        }
        let assunto = format!(
            "{} ({} {})",
            self.config.alertas.email.assunto,
            novos.len(),
            if novos.len() == 1 {
                "caminho"
            } else {
                "caminhos"
            }
        );
        let corpo = Self::texto_do_alerta(&novos, agora);
        match crate::email::enviar(&self.config.alertas.email, &assunto, &corpo) {
            Ok(r) => eprintln!("alerta de disco enviado: {r}"),
            // Falhar em avisar tambem e noticia -- e ela nao pode sumir junto
            // com o aviso que nao saiu.
            Err(e) => eprintln!("alerta de disco NAO ENVIADO: {e}"),
        }
    }

    fn texto_do_alerta(discos: &[&crate::sistema::EspacoEmDisco], agora: i64) -> String {
        let mut t = String::new();
        t.push_str("O PhxSql esta com pouco espaco em disco.\n\n");
        for e in discos {
            // O "de" e o ALCANCAVEL, e nao o tamanho do disco: o percentual ao
            // lado ja e sobre ele, e misturar as duas bases daria uma conta que
            // nao fecha para quem le ("45% de 258 GB nao dao 17 GB"). A reserva
            // do sistema de arquivos aparece a parte, quando existe.
            t.push_str(&format!(
                "  {}\n    montagem  {} ({})\n    livre     {} MB de {} MB ({:.1}%)\n",
                e.caminho,
                e.montagem,
                e.dispositivo,
                e.livre_kb / 1_024,
                e.utilizavel_kb() / 1_024,
                e.livre_percentual()
            ));
            if e.reservado_kb() > 0 {
                t.push_str(&format!(
                    "    reserva   {} MB do sistema de arquivos, fora do alcance\n",
                    e.reservado_kb() / 1_024
                ));
            }
            t.push('\n');
        }
        t.push_str(&format!(
            "Servidor PhxSql {VERSAO}\nQuando: {}\n",
            phxsql_core::datahora::instante_iso(agora)
        ));
        t
    }

    // -------------------------------------------------------------- o painel

    /// Tudo que o painel mostra, numa chamada so.
    ///
    /// Poderia ser dez chamadas do navegador, e o painel ficaria dez vezes
    /// mais lento por causa da ida e volta. Agregar aqui tambem deixa a conta
    /// do que o usuario PODE VER acontecer de um lado so: o painel nunca
    /// mostra numero de base que quem esta olhando nao poderia abrir.
    fn op_painel(&self, sessao: &Sessao) -> Result<Json> {
        let agora = crate::agora_ms();

        // ---------------------------------------------------------- bancos
        let (mut bancos, mut tabelas_total, mut registros_total, mut bytes_total) =
            (Vec::new(), 0u64, 0u64, 0u64);
        let mut maiores: Vec<(String, u64, u64)> = Vec::new();
        {
            let dados = self.travar_dados()?;
            for nome in dados.databases()? {
                // O painel so conta o que quem esta olhando poderia abrir.
                if let Some(u) = &sessao.usuario {
                    if !u.pode(&nome, Atividade::Ler) {
                        continue;
                    }
                }
                let db = dados.abrir_database(&nome)?;
                // E so o que quem olha poderia abrir, tabela a tabela: o
                // total do painel nao pode contar linha de tabela negada.
                let lista: Vec<String> = db
                    .todas_as_tabelas()?
                    .into_iter()
                    .filter(|t| self.pode_ver_tabela(sessao, &nome, t))
                    .collect();
                let schemas = db.schemas()?.len() as u64;
                let mut registros_db = 0u64;
                for t in &lista {
                    if let Ok(tab) = db.abrir_qualificada(t) {
                        let regs = tab.registros();
                        registros_db += regs;
                        let bytes: u64 = tab
                            .volumes_por_arquivo()
                            .0
                            .iter()
                            .map(|v| {
                                std::fs::metadata(tab.diretorio().join(format!(
                                    "{}{}.reg",
                                    t.rsplit('.').next().unwrap_or(t),
                                    if *v == 1 {
                                        String::new()
                                    } else {
                                        format!("_{v:03}")
                                    }
                                )))
                                .map(|m| m.len())
                                .unwrap_or(0)
                            })
                            .sum();
                        bytes_total += bytes;
                        maiores.push((format!("{nome}/{t}"), regs, bytes));
                    }
                }
                tabelas_total += lista.len() as u64;
                registros_total += registros_db;
                bancos.push(Json::objeto(vec![
                    ("nome", Json::texto_de(&nome)),
                    ("tabelas", Json::de_u64(lista.len() as u64)),
                    ("schemas", Json::de_u64(schemas)),
                    ("registros", Json::de_u64(registros_db)),
                ]));
            }
        }
        // As dez maiores, por registro. Mais que isso vira lista, nao grafico.
        maiores.sort_by(|a, b| b.1.cmp(&a.1));
        maiores.truncate(10);

        // --------------------------------------------------------- acessos
        //
        // Uma passada so sobre o log, alimentando todas as contagens de uma
        // vez. Ler o arquivo cinco vezes para responder cinco perguntas seria
        // o painel ficando lento com o log crescendo.
        let acessos = LogAcessos::ler(&self.config.log_acessos).unwrap_or_default();
        let dia_ms = 86_400_000i64;
        let desde = agora - dia_ms;
        let mut por_hora = [0u64; 24];
        let mut recusadas_por_hora = [0u64; 24];
        let mut por_op: HashMap<String, (u64, u64)> = HashMap::new();
        let mut por_usuario: HashMap<String, u64> = HashMap::new();
        let (mut ok, mut falhas, mut soma_ms) = (0u64, 0u64, 0u64);
        for a in &acessos {
            if a.ok {
                ok += 1;
            } else {
                falhas += 1;
            }
            soma_ms += a.duracao_ms;
            let e = por_op.entry(a.op.clone()).or_insert((0, 0));
            if a.ok {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
            if !a.usuario.is_empty() {
                *por_usuario.entry(a.usuario.clone()).or_insert(0) += 1;
            }
            if a.quando_ms >= desde {
                // Balde por hora, contando de tras para frente a partir de
                // agora: o balde 23 e a hora corrente.
                let atras = ((agora - a.quando_ms) / 3_600_000) as usize;
                if atras < 24 {
                    let i = 23 - atras;
                    por_hora[i] += 1;
                    if !a.ok {
                        recusadas_por_hora[i] += 1;
                    }
                }
            }
        }
        let mut ops: Vec<(String, u64, u64)> =
            por_op.into_iter().map(|(k, (a, b))| (k, a, b)).collect();
        ops.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
        ops.truncate(12);
        let mut usuarios_ativos: Vec<(String, u64)> = por_usuario.into_iter().collect();
        usuarios_ativos.sort_by(|a, b| b.1.cmp(&a.1));
        usuarios_ativos.truncate(8);

        let ips = LogAcessos::resumo_por_ip(&self.config.log_acessos).unwrap_or_default();
        let mut top_ips: Vec<&crate::acesso::ResumoIp> = ips.iter().collect();
        top_ips.sort_by(|a, b| b.acessos.cmp(&a.acessos));
        top_ips.truncate(8);

        // -------------------------------------------------------- usuarios
        let cadastro = &self.config.cadastro;
        let mut por_nivel: HashMap<&'static str, u64> = HashMap::new();
        for u in cadastro.root.iter().chain(cadastro.usuarios.iter()) {
            *por_nivel.entry(u.nivel.nome()).or_insert(0) += 1;
        }
        let ordem_nivel = ["admin", "dono", "operador", "leitor", "nenhum"];

        // --------------------------------------------------------- estado
        let bloqueios = self
            .lista_negra
            .lock()
            .map(|l| l.ativos(agora).len() as u64)
            .unwrap_or(0);
        let (residentes, bytes_ram) = self
            .residentes
            .lock()
            .map(|r| {
                (
                    r.len() as u64,
                    r.values().map(|m| m.bytes() as u64).sum::<u64>(),
                )
            })
            .unwrap_or((0, 0));
        let sessoes_web = self.sessoes.lock().map(|s| s.quantas() as u64).unwrap_or(0);

        Ok(Json::objeto(vec![
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
            ("versao", Json::texto_de(VERSAO)),
            ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
            (
                "resumo",
                Json::objeto(vec![
                    ("bancos", Json::de_u64(bancos.len() as u64)),
                    ("tabelas", Json::de_u64(tabelas_total)),
                    ("registros", Json::de_u64(registros_total)),
                    ("bytes_reg", Json::de_u64(bytes_total)),
                    (
                        "usuarios",
                        Json::de_u64(
                            (cadastro.usuarios.len() + usize::from(cadastro.root.is_some())) as u64,
                        ),
                    ),
                    (
                        "conexoes",
                        Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                    ),
                    ("sessoes_web", Json::de_u64(sessoes_web)),
                    ("bloqueios", Json::de_u64(bloqueios)),
                    ("tabelas_em_ram", Json::de_u64(residentes)),
                    ("bytes_em_ram", Json::de_u64(bytes_ram)),
                    ("acessos", Json::de_u64(ok + falhas)),
                    ("acessos_ok", Json::de_u64(ok)),
                    ("acessos_recusados", Json::de_u64(falhas)),
                    (
                        "ms_medio",
                        Json::de_u64(if ok + falhas > 0 {
                            soma_ms / (ok + falhas)
                        } else {
                            0
                        }),
                    ),
                    ("espelho", Json::Bool(self.config.espelho)),
                    ("somente_leitura", Json::Bool(self.config.somente_leitura)),
                ]),
            ),
            ("bancos", Json::Lista(bancos)),
            (
                "maiores_tabelas",
                Json::Lista(
                    maiores
                        .iter()
                        .map(|(n, r, b)| {
                            Json::objeto(vec![
                                ("tabela", Json::texto_de(n)),
                                ("registros", Json::de_u64(*r)),
                                ("bytes", Json::de_u64(*b)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_hora",
                Json::Lista(por_hora.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            (
                "recusadas_por_hora",
                Json::Lista(
                    recusadas_por_hora
                        .iter()
                        .map(|n| Json::de_u64(*n))
                        .collect(),
                ),
            ),
            (
                "por_operacao",
                Json::Lista(
                    ops.iter()
                        .map(|(o, a, r)| {
                            Json::objeto(vec![
                                ("op", Json::texto_de(o)),
                                ("ok", Json::de_u64(*a)),
                                ("recusados", Json::de_u64(*r)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_nivel",
                Json::Lista(
                    ordem_nivel
                        .iter()
                        .filter_map(|n| {
                            por_nivel.get(n).map(|q| {
                                Json::objeto(vec![
                                    ("nivel", Json::texto_de(*n)),
                                    ("quantos", Json::de_u64(*q)),
                                ])
                            })
                        })
                        .collect(),
                ),
            ),
            (
                "usuarios_ativos",
                Json::Lista(
                    usuarios_ativos
                        .iter()
                        .map(|(u, q)| {
                            Json::objeto(vec![
                                ("usuario", Json::texto_de(u)),
                                ("acessos", Json::de_u64(*q)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "top_ips",
                Json::Lista(
                    top_ips
                        .iter()
                        .map(|r| {
                            Json::objeto(vec![
                                ("ip", Json::texto_de(&r.ip)),
                                ("acessos", Json::de_u64(r.acessos)),
                                ("recusados", Json::de_u64(r.recusados)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    // ------------------------------------------------------ tabela em memoria

    /// Chave de residencia. Inclui o database porque duas bases podem ter
    /// tabela de mesmo nome -- e teriam, se ninguem cuidasse disso.
    fn chave_residente(p: &Json) -> String {
        format!(
            "{}/{}",
            p.texto_ou("database", ""),
            p.texto_ou("tabela", "")
        )
    }

    /// Mexe na copia residente, se a tabela deste pedido estiver carregada.
    fn residente_mut(&self, p: &Json, f: impl FnOnce(&mut TabelaMemoria)) {
        if let Ok(mut r) = self.residentes.lock() {
            if let Some(m) = r.get_mut(&Self::chave_residente(p)) {
                f(m);
            }
        }
    }

    /// Le a tabela inteira para a RAM.
    fn op_memoria_carregar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let esquema = t.esquema().clone();

        // As colunas com mapa de igualdade. Sem pedido, mapeia as que ja sao
        // primeira coluna de algum indice: quem indexou no disco costuma
        // filtrar pelo mesmo campo na memoria.
        let mapear: Vec<usize> = match p.campo("mapear").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, &esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => {
                let mut v: Vec<usize> = esquema
                    .indices()
                    .iter()
                    .filter_map(|i| i.colunas.first().map(|c| c.coluna))
                    .collect();
                v.sort_unstable();
                v.dedup();
                v
            }
        };

        let inicio = Instant::now();
        let m = {
            let _trava = self.travar_dados()?;
            TabelaMemoria::carregar(&mut t, &mapear, crate::agora_ms())?
        };
        let ficha = ficha_residente(&Self::chave_residente(p), &m);
        let ms = inicio.elapsed().as_millis() as u64;
        self.residentes
            .lock()
            .map_err(|_| trava_envenenada())?
            .insert(Self::chave_residente(p), m);

        let mut campos = ficha;
        campos.push(("carregou_em_ms", Json::de_u64(ms)));
        Ok(Json::objeto(campos))
    }

    fn op_memoria_liberar(&self, p: &Json) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let saiu = self
            .residentes
            .lock()
            .map_err(|_| trava_envenenada())?
            .remove(&chave)
            .is_some();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            ("estava_carregada", Json::Bool(saiu)),
        ]))
    }

    /// O que esta residente agora.
    fn op_memoria(&self) -> Result<Json> {
        let r = self.residentes.lock().map_err(|_| trava_envenenada())?;
        let mut chaves: Vec<&String> = r.keys().collect();
        chaves.sort();
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            ("tabelas", Json::de_u64(r.len() as u64)),
            (
                "bytes",
                Json::de_u64(r.values().map(|m| m.bytes() as u64).sum()),
            ),
            (
                "residentes",
                Json::Lista(
                    chaves
                        .into_iter()
                        .map(|c| {
                            let m = &r[c];
                            let mut f = ficha_residente(c, m);
                            f.push((
                                "carregada_ha_s",
                                Json::de_u64(((agora - m.carregada_ms()) / 1000).max(0) as u64),
                            ));
                            Json::objeto(f)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// `SelectMemory`: a consulta que nao toca em disco.
    ///
    /// Recusa em vez de adivinhar quando a tabela nao esta carregada. Carregar
    /// uma tabela grande sem ninguem ter pedido seria a operacao rapida virando
    /// a operacao lenta, calada, na hora errada.
    fn op_selecionar_memoria(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let r = self.residentes.lock().map_err(|_| trava_envenenada())?;
        let m = r.get(&chave).ok_or_else(|| {
            PhxError::NaoEncontrado(format!(
                "{chave} nao esta em memoria; carregue antes com {{\"op\":\"memoria_carregar\",\"database\":...,\"tabela\":...}}"
            ))
        })?;
        let esquema = m.esquema();

        // O poder vale igual na memoria e no disco. O portao ja passou pelo
        // despachar; isto e o cinto: quem chegar aqui por outro caminho para.
        if let Some(u) = &sessao.usuario {
            let (base, tabela) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
            if !u.pode_em(base, tabela, Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {base}.{tabela}",
                    u.login
                )));
            }
        }

        let mut onde = Vec::new();
        if let Some(l) = p.campo("onde").and_then(Json::lista) {
            for f in l {
                let coluna = coluna_de(
                    f.campo("coluna")
                        .ok_or_else(|| PhxError::Esquema("filtro sem \"coluna\"".into()))?,
                    esquema,
                )?;
                let op = Operador::de_texto(f.texto_ou("op", "="))?;
                let valor = match f.campo("valor") {
                    Some(v) => crate::valores::json_para_valor(v, &esquema.colunas()[coluna].ty)?,
                    None => phxsql_core::value::Value::Null,
                };
                onde.push(Filtro { coluna, op, valor });
            }
        }

        let mut ordenar = Vec::new();
        if let Some(l) = p.campo("ordenar").and_then(Json::lista) {
            for o in l {
                ordenar.push(Ordem {
                    coluna: coluna_de(
                        o.campo("coluna")
                            .ok_or_else(|| PhxError::Esquema("ordem sem \"coluna\"".into()))?,
                        esquema,
                    )?,
                    desc: o.booleano_ou("desc", false),
                });
            }
        }

        let colunas = match p.campo("colunas").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => Vec::new(),
        };

        let consulta = Consulta {
            onde,
            ordenar,
            colunas,
            pular: p.inteiro_ou("pular", 0).max(0) as u64,
            max: self.limite(p),
        };

        let inicio = Instant::now();
        let saida = m.selecionar(&consulta)?;
        let us = inicio.elapsed().as_micros() as u64;

        // A projecao muda as colunas, entao os nomes vem com o resultado --
        // senao quem le nao sabe qual campo e qual.
        let indices: Vec<usize> = if consulta.colunas.is_empty() {
            (0..esquema.colunas().len()).collect()
        } else {
            consulta.colunas.clone()
        };
        let nomes: Vec<String> = indices
            .iter()
            .map(|i| esquema.colunas()[*i].nome.clone())
            .collect();
        let tipos: Vec<phxsql_core::types::ColumnType> =
            indices.iter().map(|i| esquema.colunas()[*i].ty).collect();

        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            (
                "colunas",
                Json::Lista(nomes.iter().map(Json::texto_de).collect()),
            ),
            ("achadas", Json::de_u64(saida.achadas)),
            ("devolvidas", Json::de_u64(saida.linhas.len() as u64)),
            ("examinadas", Json::de_u64(saida.examinadas)),
            (
                "por_mapa",
                match &saida.por_mapa {
                    Some(c) => Json::texto_de(c),
                    None => Json::Nulo,
                },
            ),
            ("us", Json::de_u64(us)),
            (
                "linhas",
                Json::Lista(
                    saida
                        .linhas
                        .iter()
                        .map(|(rowid, l)| {
                            let mut campos = vec![("rowid", Json::de_u64(*rowid))];
                            for ((n, v), ty) in nomes.iter().zip(l.iter()).zip(tipos.iter()) {
                                campos.push((n.as_str(), crate::valores::valor_para_json(v, ty)));
                            }
                            Json::objeto(campos)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    fn op_diario(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p) as usize;
        let rowid = p.campo("rowid").and_then(Json::inteiro).map(|n| n as u64);
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let eventos = match rowid {
            Some(r) => t.historico(r)?,
            None => t.diario(0, 0)?,
        };
        let total = eventos.len();
        let recentes: Vec<Json> = eventos
            .iter()
            .rev()
            .take(max)
            .rev()
            .map(|e| {
                Json::objeto(vec![
                    ("quando", Json::texto_de(e.instante_iso())),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("eventos", Json::Lista(recentes)),
        ]))
    }

    // ------------------------------------------------------------- profiler

    /// `profiler_ligar`: comeca a observar o que chega pela porta.
    ///
    /// **So administrador**, e a razao esta no que ele mostra: o texto dos
    /// pedidos de todo mundo, com os dados que estao sendo gravados dentro.
    /// Quem pode ler uma tabela nao ganha por isto o direito de ver o que os
    /// outros escrevem nela.
    fn op_profiler_ligar(&self, p: &Json) -> Result<Json> {
        let filtro = crate::profiler::Filtro {
            database: p.texto_ou("database", "").trim().to_string(),
            usuario: p.texto_ou("usuario", "").trim().to_string(),
            op: p.texto_ou("operacao", "").trim().to_string(),
            so_escrita: p.booleano_ou("so_escrita", false),
        };
        let arquivo = p.texto_ou("arquivo", "").to_string();
        let teto = p.inteiro_ou("guardar", 500).max(0) as usize;
        let agora = crate::agora_ms();
        let mut prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        prof.ligar(filtro, &arquivo, teto, agora)?;
        // Dentro da trava, e DEPOIS de `ligar` ter dado certo: um espelho que
        // sobe antes faria o caminho quente pagar por um profiler que nao ligou.
        self.profiler_ligado.store(true, Ordering::Relaxed);
        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(true)),
            ("guardar", Json::de_u64(prof.teto() as u64)),
            (
                "arquivo",
                Json::texto_de(prof.caminho().display().to_string()),
            ),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
        ]))
    }

    fn op_profiler_desligar(&self) -> Result<Json> {
        let mut prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        let n = prof.observados();
        prof.desligar(crate::agora_ms());
        self.profiler_ligado.store(false, Ordering::Relaxed);
        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(false)),
            ("observados", Json::de_u64(n)),
        ]))
    }

    fn op_profiler_limpar(&self) -> Result<Json> {
        let mut prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        prof.limpar();
        Ok(Json::objeto(vec![("limpo", Json::Bool(true))]))
    }

    /// `profiler`: o que foi observado, do mais recente para o mais antigo.
    fn op_profiler(&self, p: &Json) -> Result<Json> {
        let max = p.inteiro_ou("max", 200).max(0) as usize;
        // `desde_serial` deixa a tela pedir so o que ainda nao viu, em vez de
        // rebaixar o anel inteiro a cada atualizacao.
        let desde = p.inteiro_ou("desde_serial", 0).max(0) as u64;
        let prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        let f = prof.filtro();

        let eventos: Vec<Json> = prof
            .eventos(max.clamp(1, 5_000))
            .into_iter()
            .filter(|e| e.serial > desde)
            .map(|e| {
                Json::objeto(vec![
                    ("serial", Json::de_u64(e.serial)),
                    (
                        "quando",
                        Json::texto_de(phxsql_core::datahora::instante_iso(e.quando_ms)),
                    ),
                    ("ip", Json::texto_de(e.ip)),
                    ("usuario", Json::texto_de(e.usuario)),
                    ("op", Json::texto_de(e.op)),
                    ("database", Json::texto_de(e.database)),
                    ("tabela", Json::texto_de(e.tabela)),
                    ("bytes", Json::de_u64(e.bytes as u64)),
                    // Ja vem redigido: os campos de senha viraram *** antes de
                    // encostar no anel.
                    ("pedido", Json::texto_de(e.pedido)),
                    (
                        "ms",
                        match e.duracao_ms {
                            Some(ms) => Json::de_u64(ms),
                            None => Json::Nulo,
                        },
                    ),
                    (
                        "ok",
                        match e.ok {
                            Some(v) => Json::Bool(v),
                            None => Json::Nulo,
                        },
                    ),
                    ("erro", Json::texto_de(e.erro)),
                ])
            })
            .collect();

        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(prof.ligado())),
            (
                "arquivo",
                Json::texto_de(prof.caminho().display().to_string()),
            ),
            ("observados", Json::de_u64(prof.observados())),
            ("esquecidos", Json::de_u64(prof.esquecidos())),
            ("guardar", Json::de_u64(prof.teto() as u64)),
            (
                "desde",
                Json::texto_de(if prof.ligado() {
                    phxsql_core::datahora::instante_iso(prof.ligado_em_ms())
                } else {
                    String::new()
                }),
            ),
            (
                "filtro",
                Json::objeto(vec![
                    ("database", Json::texto_de(&f.database)),
                    ("usuario", Json::texto_de(&f.usuario)),
                    ("operacao", Json::texto_de(&f.op)),
                    ("so_escrita", Json::Bool(f.so_escrita)),
                ]),
            ),
            ("eventos", Json::Lista(eventos)),
        ]))
    }

    // ----------------------------------------------------------- replicacao

    /// `posicao`: quantos eventos cada tabela do database ja tem.
    ///
    /// E o equivalente do `SHOW MASTER STATUS`, e o que a replica compara com
    /// a propria posicao para saber o que falta. Sai do cabecalho de cada
    /// volume do `.log`, sem ler evento nenhum.
    ///
    /// Por que POR TABELA e nao por servidor: o PhxSql ainda nao tem transacao
    /// entre tabelas, entao nao existe ordem global a preservar -- e um numero
    /// por tabela deixa as tabelas replicarem em paralelo.
    fn op_posicao(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").to_string();
        if database.is_empty() {
            return Err(PhxError::Esquema("informe \"database\"".into()));
        }
        let _trava = self.travar_dados()?;
        let db = _trava.abrir_database(&database)?;
        let com_esquema = p.booleano_ou("com_esquema", false);
        let mut posicoes = Vec::new();
        for nome in db.todas_as_tabelas()? {
            let mut t = db.abrir_qualificada(&nome)?;
            let mut campos = vec![
                ("eventos".to_string(), Json::de_u64(t.eventos()?)),
                ("registros".to_string(), Json::de_u64(t.registros())),
            ];
            if com_esquema {
                // O bloco de esquema CRU, do jeito que mora no `.reg`. A
                // replica desserializa o mesmo bloco e cria a tabela dela --
                // sem remontar coluna por coluna a partir de JSON, que e onde
                // um tipo ou uma escala se perderiam sem ninguem notar.
                campos.push((
                    "esquema".to_string(),
                    Json::texto_de(bytes_para_hex(&t.esquema().serializar())),
                ));
            }
            posicoes.push((nome, Json::Objeto(campos)));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
            // Sem a imagem ligada o diario existe mas nao replica, e a replica
            // precisa saber disso ANTES de puxar mil eventos inaplicaveis.
            (
                "imagem_da_linha",
                Json::Bool(self.config.replicacao.imagem_da_linha),
            ),
            ("tabelas", Json::Objeto(posicoes)),
            ("usuario", Json::de_u64(sessao.id() as u64)),
        ]))
    }

    /// `replicar`: os eventos a partir da posicao `desde`, com a imagem.
    ///
    /// A imagem vai em hexadecimal porque o transporte e JSON e JSON nao tem
    /// bytes. Dobra o tamanho -- e a alternativa seria acrescentar um formato
    /// binario ao protocolo, que e uma decisao maior do que esta.
    fn op_replicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let desde = p.inteiro_ou("desde", 0).max(0) as u64;
        let max = p.inteiro_ou("max", 500).max(0) as u64;
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let total = t.eventos()?;

        // A dica de onde a leitura anterior desta tabela parou. Sem ela, o
        // `desde` faz o diario ser varrido desde o comeco a cada lote -- ver
        // `marcas_do_diario`.
        let chave = format!(
            "{}/{}",
            p.texto_ou("database", "").to_lowercase(),
            p.texto_ou("tabela", "").to_lowercase()
        );
        if let Ok(m) = self.marcas_do_diario.lock() {
            // A maior que ainda cabe: a marca so serve para uma posicao depois
            // dela.
            t.definir_marca_do_diario(
                m.get(&chave)
                    .and_then(|v| {
                        v.iter()
                            .filter(|k| k.evento <= desde)
                            .max_by_key(|k| k.evento)
                    })
                    .copied(),
            );
        }
        let eventos = t.diario_com_imagem(desde, max)?;
        if let (Ok(mut m), Some(nova)) = (self.marcas_do_diario.lock(), t.marca_do_diario()) {
            let v = m.entry(chave).or_default();
            // A que esta replica acabou de usar sai: ela nao volta atras.
            v.retain(|k| k.evento != desde && k.evento != nova.evento);
            v.push(nova);
            // Teto pequeno: sao dicas, e a mais antiga e a menos util.
            if v.len() > MARCAS_POR_TABELA {
                v.sort_unstable_by_key(|k| k.evento);
                v.remove(0);
            }
        }
        let lidos = eventos.len() as u64;

        let lista: Vec<Json> = eventos
            .into_iter()
            .map(|(e, imagem)| {
                Json::objeto(vec![
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                    ("imagem", Json::texto_de(bytes_para_hex(&imagem))),
                ])
            })
            .collect();

        Ok(Json::objeto(vec![
            ("desde", Json::de_u64(desde)),
            ("ate", Json::de_u64(desde + lidos)),
            ("total", Json::de_u64(total)),
            // `fim` verdadeiro quer dizer "por enquanto acabou": a replica
            // espera e pergunta de novo, em vez de girar em falso.
            ("fim", Json::Bool(desde + lidos >= total)),
            ("eventos", Json::Lista(lista)),
        ]))
    }

    /// `aplicar`: grava na tabela LOCAL os eventos que vieram do source.
    ///
    /// Para no primeiro erro e devolve onde parou. Seguir depois de um erro
    /// espalharia a divergencia -- e o rowid que nao bate ja e o sinal de que
    /// a replica divergiu.
    fn op_aplicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let eventos = p
            .campo("eventos")
            .and_then(Json::lista)
            .ok_or_else(|| PhxError::Esquema("informe \"eventos\" como lista".into()))?
            .to_vec();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let mut aplicados = 0u64;
        let mut erro = None;
        for e in eventos.iter() {
            let operacao = match e.texto_ou("operacao", "") {
                "inclusao" => Operacao::Inclusao,
                "alteracao" => Operacao::Alteracao,
                "exclusao" => Operacao::Exclusao,
                outro => {
                    erro = Some(format!("operacao desconhecida no evento: {outro:?}"));
                    break;
                }
            };
            let rowid = e.inteiro_ou("rowid", 0).max(0) as u64;
            let imagem = match hex_para_bytes(e.texto_ou("imagem", "")) {
                Ok(b) => b,
                Err(x) => {
                    erro = Some(x.to_string());
                    break;
                }
            };
            match t.aplicar_evento(operacao, rowid, &imagem) {
                Ok(_) => aplicados += 1,
                Err(x) => {
                    erro = Some(x.to_string());
                    break;
                }
            }
        }
        self.gravar_de_verdade(&mut t, p)?;

        Ok(Json::objeto(vec![
            ("recebidos", Json::de_u64(eventos.len() as u64)),
            ("aplicados", Json::de_u64(aplicados)),
            ("posicao", Json::de_u64(t.eventos()?)),
            (
                "erro",
                match erro {
                    Some(e) => Json::texto_de(e),
                    None => Json::Nulo,
                },
            ),
        ]))
    }

    fn op_verificar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let r = t.verificar()?;
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&r.tabela)),
            ("registros", Json::de_u64(r.registros)),
            ("slots", Json::de_u64(r.slots)),
            ("eventos", Json::de_u64(r.eventos)),
            (
                "indices",
                Json::Objeto(
                    r.indices
                        .iter()
                        .map(|(n, q)| (n.clone(), Json::de_u64(*q)))
                        .collect(),
                ),
            ),
            (
                "volumes",
                Json::objeto(vec![
                    ("reg", Json::de_u64(r.volumes.0 as u64)),
                    ("bin", Json::de_u64(r.volumes.1 as u64)),
                    ("memo", Json::de_u64(r.volumes.2 as u64)),
                    ("log", Json::de_u64(r.volumes.3 as u64)),
                ]),
            ),
        ]))
    }

    fn op_reindexar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let indices = t.reindexar()?;
        self.gravar_de_verdade(&mut t, p)?;
        Ok(Json::Objeto(
            indices
                .into_iter()
                .map(|(n, q)| (n, Json::de_u64(q)))
                .collect(),
        ))
    }
}

/// A ponte MCP falando com ESTE servidor, no mesmo processo.
///
/// # Por que ela serializa o pedido para depois reanalisá-lo
///
/// Porque `despachar` recebe uma LINHA, e é ali que moram os quatro portões:
/// política, token, login e permissão por base e tabela. Chamar `executar`
/// direto pularia todos eles -- e seria o segundo caminho até o dado, que é
/// sempre o que esquece uma conferência.
///
/// O custo é um `escrever` mais um `analisar` por chamada. Do outro lado desta
/// ponte há um modelo de linguagem fazendo uma pergunta por vez, e não uma
/// carga de cinco mil linhas: o gasto é irrelevante e a garantia não.
///
/// # O login mora aqui
///
/// A sessão é uma só, e viva entre chamadas: o `login` é uma operação como
/// qualquer outra, e o resultado dela é o que decide o que as próximas podem.
/// Sem isto, cada `tools/call` chegaria anônimo.
pub struct ExecutorLocal {
    servidor: Arc<Servidor>,
    sessao: Mutex<Sessao>,
    /// O que aparece no lugar do IP no log de acessos.
    ///
    /// Não é um endereço porque não há um: a ponte fala pelo cano do processo.
    /// Mas o log tem de dizer que veio dali -- uma leitura pelo MCP que não
    /// deixa rastro seria um buraco na auditoria, e é justamente a origem
    /// sobre a qual mais se vai querer perguntar depois.
    origem: String,
}

impl ExecutorLocal {
    pub fn novo(servidor: Arc<Servidor>, origem: &str) -> ExecutorLocal {
        ExecutorLocal {
            servidor,
            sessao: Mutex::new(Sessao::default()),
            origem: origem.to_string(),
        }
    }

    /// Entra com login e senha, pelo mesmo `login` do protocolo.
    ///
    /// A senha em claro nunca é guardada nem ecoada: ela vira um pedido de
    /// `login`, e o que fica na sessão é a ficha do usuário.
    pub fn entrar(&self, usuario: &str, senha: &str) -> Result<Json> {
        use crate::mcp::Executor;
        self.executar(&Json::objeto(vec![
            ("op", Json::texto_de("login")),
            ("usuario", Json::texto_de(usuario)),
            ("senha", Json::texto_de(senha)),
        ]))
    }

    /// O token de serviço deste servidor, para a ponte carimbar nos pedidos.
    pub fn token(&self) -> String {
        self.servidor.config.token.clone()
    }
}

impl crate::mcp::Executor for ExecutorLocal {
    fn executar(&self, pedido: &Json) -> Result<Json> {
        let mut sessao = self.sessao.lock().map_err(|_| trava_envenenada())?;
        let quando_ms = crate::agora_ms();
        let inicio = Instant::now();
        let linha = pedido.escrever();
        let (op, autenticado, resultado) =
            self.servidor.despachar(&linha, &mut sessao, &self.origem);
        let duracao = inicio.elapsed().as_millis() as u64;
        self.servidor.anotar(&Acesso {
            quando_ms,
            ip: self.origem.clone(),
            porta_origem: 0,
            op,
            usuario: sessao.login().to_string(),
            autenticado,
            ok: resultado.is_ok(),
            duracao_ms: duracao,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            ..objeto_do_pedido(&linha, &resultado)
        });
        resultado
    }
}

/// Os indices da tabela, como o tradutor de SQL os espera.
///
/// Saem da resposta do `esquema` -- campo por campo, e nao de uma leitura
/// propria. E a mesma decisao do resto da op `sql`: quem abre tabela e o motor.
fn indices_do_esquema(esquema: &Json) -> Vec<phxsql_sql::IndiceInfo> {
    esquema
        .campo("indices")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|i| phxsql_sql::IndiceInfo {
            nome: i.texto_ou("nome", "").to_string(),
            colunas: i
                .campo("colunas")
                .and_then(Json::lista)
                .unwrap_or(&[])
                .iter()
                .map(|c| phxsql_sql::ColunaDoIndice {
                    nome: c.texto_ou("coluna", "").to_string(),
                    desc: c.booleano_ou("desc", false),
                })
                .collect(),
            unico: i.booleano_ou("unico", false),
            primario: i.booleano_ou("primario", false),
        })
        .collect()
}

/// A resposta do `sql`: o que a operacao devolveu, mais o que a traducao
/// decidiu.
///
/// # Por que as notas viajam
///
/// Porque `ORDER BY nome` pode ter sido atendido pelo `.ndx` e `COUNT(*)` pode
/// ter saido do cabecalho sem varrer nada -- e quem escreveu o comando nao tem
/// como saber qual dos dois aconteceu. Sem as notas, quem pediu uma ordem e
/// recebeu a de digitacao culpa o motor.
fn resposta_do_sql(texto: &str, plano: &phxsql_sql::Plano, bruto: Json) -> Json {
    let mut pares = vec![
        ("sql".to_string(), Json::texto_de(texto)),
        ("op".to_string(), Json::texto_de(&plano.op)),
        (
            "notas".to_string(),
            Json::Lista(plano.notas.iter().map(Json::texto_de).collect()),
        ),
    ];

    match &plano.saida {
        // A contagem sai do cabecalho da tabela (`registros`) ou do total da
        // busca (`encontrados`): nenhuma linha e varrida para contar.
        //
        // E a resposta para por aqui: o `COUNT(*)` traduzido pede `max: 1`
        // para ler o cabecalho, e a linha que vem junto e um efeito colateral
        // do caminho -- nao a resposta. Devolve-la faria um `SELECT COUNT(*)`
        // mostrar UMA linha de dado, que e a pior resposta possivel: quem
        // olha nao sabe se aquela linha quer dizer alguma coisa. Este defeito
        // so apareceu exercitando o console.
        phxsql_sql::Saida::Contagem => {
            let n = match bruto.campo("encontrados") {
                Some(e) => e.inteiro().unwrap_or(0),
                None => bruto.inteiro_ou("registros", 0),
            };
            pares.push(("contagem".to_string(), Json::de_i64(n)));
            if let Some(r) = bruto.campo("registros") {
                pares.push(("registros".to_string(), r.clone()));
            }
            return Json::Objeto(pares);
        }
        phxsql_sql::Saida::LinhaInteira => {}
        // A projecao e do cliente porque o protocolo sempre devolve a linha
        // inteira -- o `.reg` e de slot fixo, e ler meia linha custa a mesma
        // leitura. Aqui o "cliente" e o servidor, porque quem pediu escreveu
        // SQL e espera as colunas que pediu.
        phxsql_sql::Saida::Colunas(cols) => {
            pares.push((
                "colunas".to_string(),
                Json::Lista(
                    cols.iter()
                        .map(|(_, rotulo)| Json::texto_de(rotulo))
                        .collect(),
                ),
            ));
        }
    }

    if let Json::Objeto(campos) = bruto {
        for (k, v) in campos {
            if k == "linhas" {
                if let (phxsql_sql::Saida::Colunas(cols), Json::Lista(linhas)) = (&plano.saida, &v)
                {
                    pares.push((
                        k,
                        Json::Lista(linhas.iter().map(|l| projetar(l, cols)).collect()),
                    ));
                    continue;
                }
            }
            pares.push((k, v));
        }
    }
    Json::Objeto(pares)
}

/// Fica com as colunas pedidas, nesta ordem, com estes rotulos.
///
/// Coluna que o `SELECT` pediu e a linha nao tem vira `null`, e nao some: uma
/// chave ausente faria a resposta ter forma diferente linha a linha, e quem le
/// por posicao quebraria na primeira.
fn projetar(linha: &Json, colunas: &[(String, String)]) -> Json {
    Json::Objeto(
        colunas
            .iter()
            .map(|(nome, rotulo)| {
                (
                    rotulo.clone(),
                    linha.campo(nome).cloned().unwrap_or(Json::Nulo),
                )
            })
            .collect(),
    )
}

fn trava_envenenada() -> PhxError {
    PhxError::Corrompido("uma operacao anterior entrou em panico e deixou a trava suja".into())
}

/// O resumo de uma resposta, para caber numa linha do historico de jobs.
///
/// Ele **analisa e reserializa**, nunca recorta: o corpo de um `varrer` de
/// vinte mil linhas nao entra cortado no meio, porque um pedaco de JSON nao e
/// JSON e a tela nao teria como distinguir "cortado" de "gravado assim". O que
/// nao se resume vira o tamanho em bytes, que e verdade sobre o que voltou.
fn resumir_resposta(j: &Json) -> String {
    let Json::Objeto(pares) = j else {
        return format!("{} bytes de resposta", j.escrever().len());
    };
    let curto: Vec<(String, Json)> = pares
        .iter()
        .filter(|(_, v)| !matches!(v, Json::Lista(_) | Json::Objeto(_)))
        .cloned()
        .collect();
    let grandes: Vec<String> = pares
        .iter()
        .filter_map(|(k, v)| match v {
            Json::Lista(l) => Some(format!("{k}: {} itens", l.len())),
            Json::Objeto(o) => Some(format!("{k}: {} campos", o.len())),
            _ => None,
        })
        .collect();
    let mut texto = Json::Objeto(curto).escrever();
    if !grandes.is_empty() {
        texto.push_str(&format!(" ({})", grandes.join(", ")));
    }
    texto
}

/// A guarda de conflito de escrita, quando o cliente pede.
///
/// # Por que a conferencia e pedida, e nao imposta
///
/// Imposta, todo cliente escrito antes desta versao pararia de gravar de
/// um dia para o outro -- e o que ele estaria recebendo nao e protecao, e um
/// erro que ele nao sabe tratar. Pedida, quem manda a versao ganha a garantia
/// na hora e quem nao manda continua com o comportamento de sempre: a ultima
/// gravacao vence.
///
/// A interface web manda sempre, porque ali existe gente do outro lado e
/// existe a janela de minutos entre abrir a ficha e clicar em salvar. E onde
/// o conflito de fato acontece.
///
/// Zero e ausente sao a mesma coisa: a versao de um registro vivo comeca em
/// 1, entao o zero nao tira nenhum valor legitimo do caminho.
fn conferir_versao_pedida(t: &mut Table, p: &Json, rowid: phxsql_core::RowId) -> Result<()> {
    let esperada = p.inteiro_ou("versao", 0).max(0) as u64;
    if esperada == 0 {
        return Ok(());
    }
    t.conferir_versao(rowid, esperada)
}

/// A resposta de erro do protocolo, com codigo.
///
/// O codigo vem JUNTO com o texto, e nao no lugar dele: o texto e para quem
/// le, o codigo e para quem programa. Sem ele, integrar com o PhxSql obriga a
/// comparar TEXTO -- e melhorar a redacao de uma mensagem quebraria o cliente
/// sem ninguem perceber.
/// Roda a limpeza na saida do escopo, por qualquer caminho.
///
/// Existe por causa dos `return` no meio do laco da conexao: sem ele, cada um
/// deles precisaria lembrar de tirar a conexao do registro, e o dia em que
/// alguem acrescentasse um `return` novo a lista passaria a mostrar conexao
/// que ja morreu -- uma lista que mente e pior do que nenhuma.
/// A trava de dados com o cronometro por dentro.
///
/// Ela se comporta como a `MutexGuard` que embrulha -- `Deref` e `DerefMut`
/// entregam a `Instancia` --, e a unica coisa que acrescenta acontece no
/// `Drop`: o tempo que ela ficou na mao entra na serie. Sem isso, «o servidor
/// esta lento» nunca distinguiria fila de trabalho.
struct TravaMedida<'a> {
    guarda: std::sync::MutexGuard<'a, Instancia>,
    /// Quando a trava foi obtida. `None` com a telemetria desligada -- e ai o
    /// `Drop` nao faz nada.
    tomada: Option<Instant>,
    telemetria: &'a crate::telemetria::Telemetria,
}

impl std::ops::Deref for TravaMedida<'_> {
    type Target = Instancia;
    fn deref(&self) -> &Instancia {
        &self.guarda
    }
}

impl std::ops::DerefMut for TravaMedida<'_> {
    fn deref_mut(&mut self) -> &mut Instancia {
        &mut self.guarda
    }
}

impl Drop for TravaMedida<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.tomada {
            self.telemetria.contar_trava(t.elapsed().as_micros() as u64);
            // A atividade deixa de ser a que segura todo mundo no MESMO
            // instante em que solta a trava -- e nao no fim do pedido. Entre
            // um e outro ela ainda monta a resposta, e acusa-la de segurar a
            // trava nesse trecho seria apontar o culpado errado.
            if let Some(a) = crate::telemetria::corrente() {
                a.sem_a_trava();
            }
        }
    }
}

struct AoSair<F: FnMut()>(F);

impl<F: FnMut()> Drop for AoSair<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Os campos do log que saem do PEDIDO, e nao do resultado.
///
/// Devolve um `Acesso` so para preencher com `..`: os outros campos do
/// registro vem de quem chama, e repetir a leitura do corpo em dois lugares e
/// como os dois caminhos (porta de dados e web) divergiriam com o tempo.
fn objeto_do_pedido(corpo: &str, resultado: &Result<Json>) -> Acesso {
    let j = Json::analisar(corpo).unwrap_or(Json::Nulo);
    Acesso {
        database: j.texto_ou("database", "").to_string(),
        tabela: j.texto_ou("tabela", "").to_string(),
        codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        ..Acesso::default()
    }
}

fn resposta_erro(op: &str, e: &PhxError, ms: u64) -> Json {
    Json::objeto(vec![
        ("ok", Json::Bool(false)),
        ("op", Json::texto_de(op)),
        ("erro", Json::texto_de(e.to_string())),
        ("codigo", Json::de_u64(e.codigo() as u64)),
        ("nome", Json::texto_de(e.nome())),
        ("classe", Json::texto_de(e.classe())),
        ("repetir", Json::Bool(e.adianta_repetir())),
        ("ms", Json::de_u64(ms)),
    ])
}

/// Uma coluna, pelo nome ou pelo numero. Aceitar os dois e o que deixa a
/// consulta legivel a mao e barata pela interface.
fn coluna_de(j: &Json, esquema: &phxsql_core::schema::Schema) -> Result<usize> {
    if let Some(n) = j.inteiro() {
        let i = n as usize;
        if n < 0 || i >= esquema.colunas().len() {
            return Err(PhxError::Esquema(format!("coluna {n} nao existe")));
        }
        return Ok(i);
    }
    let nome = j.texto().unwrap_or("");
    esquema
        .colunas()
        .iter()
        .position(|c| c.nome == nome)
        .ok_or_else(|| PhxError::Esquema(format!("coluna {nome:?} nao existe")))
}

fn ficha_residente(chave: &str, m: &TabelaMemoria) -> Vec<(&'static str, Json)> {
    let nomes: Vec<Json> = m
        .colunas_mapeadas()
        .iter()
        .map(|i| Json::texto_de(&m.esquema().colunas()[*i].nome))
        .collect();
    vec![
        ("tabela", Json::texto_de(chave)),
        ("linhas", Json::de_u64(m.vivos())),
        ("slots", Json::de_u64(m.slots())),
        ("bytes", Json::de_u64(m.bytes() as u64)),
        ("mapas", Json::Lista(nomes)),
    ]
}

/// Quantas linhas o pivot varre, no maximo.
const TETO_PIVOT: u64 = 5_000_000;
/// Quantas linhas uma tabela de consulta pode ter para caber na memoria.
const TETO_JUNCAO: usize = 500_000;

fn posicao_da_coluna(e: &Schema, nome: &str) -> Option<usize> {
    e.colunas().iter().position(|c| c.nome == nome)
}

/// Le `cidade` ou `cliente.cidade` e diz de onde o campo vem.
fn resolver_campo(
    nome: &str,
    esquema: &Schema,
    juncoes: &[Juncao],
    granularidade: &str,
) -> Result<Campo> {
    let g = Granularidade::de_texto(granularidade)?;
    // Prefixo de junção primeiro: uma tabela de consulta chamada `cliente` com
    // uma coluna `cidade` tem de ganhar de uma coluna local chamada
    // `cliente.cidade`, que nao existe -- o ponto so aparece por junção.
    if let Some((pref, campo)) = nome.split_once('.') {
        if let Some(i) = juncoes.iter().position(|j| j.prefixo == pref) {
            let c = posicao_da_coluna(&juncoes[i].esquema, campo).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a coluna {campo:?} nao existe em {}",
                    juncoes[i].esquema.nome()
                ))
            })?;
            return Ok(Campo {
                qualificado: nome.to_string(),
                juncao: Some(i),
                coluna: c,
                granularidade: g,
            });
        }
    }
    let c = posicao_da_coluna(esquema, nome).ok_or_else(|| {
        PhxError::Esquema(format!(
            "a coluna {nome:?} nao existe em {}. Para usar uma coluna de tabela \
             juntada, escreva prefixo.coluna",
            esquema.nome()
        ))
    })?;
    Ok(Campo {
        qualificado: nome.to_string(),
        juncao: None,
        coluna: c,
        granularidade: g,
    })
}

/// Percorre linhas que ja estao na memoria.
struct LinhasEmMemoria(std::vec::IntoIter<Vec<Value>>);

impl crate::pivot::Iterador for LinhasEmMemoria {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
        Ok(self.0.next())
    }
}

/// O cabecalho de uma junção ou união, no formato que a grade da tela espera.
fn colunas_da_juncao(colunas: &[crate::juncao::ColunaSaida]) -> Json {
    Json::Lista(
        colunas
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("lado", Json::texto_de(c.lado)),
                    ("chave", Json::Bool(c.chave)),
                ])
            })
            .collect(),
    )
}

/// O que se conta sobre um grupo de acessos.
///
/// Guarda a soma e o pior caso, e nao a lista: somar por tabela num log de
/// milhoes de linhas nao pode custar uma copia por linha.
#[derive(Debug, Default, Clone)]
struct Contagem {
    quantas: u64,
    recusadas: u64,
    soma_ms: u64,
    pior_ms: u64,
    ultimo_ms: i64,
}

impl Contagem {
    fn somar(&mut self, a: &Acesso) {
        self.quantas += 1;
        if !a.ok {
            self.recusadas += 1;
        }
        self.soma_ms += a.duracao_ms;
        self.pior_ms = self.pior_ms.max(a.duracao_ms);
        self.ultimo_ms = self.ultimo_ms.max(a.quando_ms);
    }

    fn para_json(&self, nome: &str) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(nome)),
            ("quantas", Json::de_u64(self.quantas)),
            ("recusadas", Json::de_u64(self.recusadas)),
            (
                "ms_medio",
                Json::de_u64(if self.quantas > 0 {
                    self.soma_ms / self.quantas
                } else {
                    0
                }),
            ),
            ("ms_pior", Json::de_u64(self.pior_ms)),
            ("ms_total", Json::de_u64(self.soma_ms)),
            (
                "ultimo",
                match self.ultimo_ms {
                    0 => Json::Nulo,
                    q => Json::texto_de(phxsql_core::datahora::instante_iso(q)),
                },
            ),
        ])
    }
}

/// Percorre a tabela de fatos linha a linha, sem materializa-la.
struct LinhasDaTabela<'a> {
    rowids: std::vec::IntoIter<u64>,
    tabela: &'a mut Table,
}

impl crate::pivot::Iterador for LinhasDaTabela<'_> {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
        for rowid in self.rowids.by_ref() {
            if let Some(l) = self.tabela.ler(rowid)? {
                return Ok(Some(l));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod testes_politica {
    use super::*;

    /// Um servidor em `somente_leitura` nao pode criar nem apagar tabela.
    ///
    /// Este teste existe porque o furo ja aconteceu: as tres operacoes de
    /// gestao de tabela entraram no despacho e ficaram DE FORA da lista de
    /// escrita, e um servidor marcado somente-leitura teria aceitado apagar os
    /// cinco arquivos de uma tabela. A lista e escrita a mao, entao quem
    /// acrescentar uma operacao que grava precisa lembrar dela -- e este teste
    /// e o lembrete.
    #[test]
    fn tudo_que_grava_esta_na_lista_de_escrita() {
        for op in [
            "inserir",
            "atualizar",
            "excluir",
            "reindexar",
            "criar_database",
            "criar_tabela",
            "excluir_tabela",
            "duplicar_tabela",
            "copiar_tabela",
            "ajustar_sequencia",
            "dblink_salvar",
            "dblink_excluir",
            // Derrubar conexao alheia nao e leitura: um servidor somente
            // leitura nao deve poder interromper o trabalho de ninguem.
            "encerrar_sessao",
            // Restaurar desmarca a coluna de sistema, e esvaziar apaga a
            // lixeira inteira: os dois gravam.
            "restaurar",
            "esvaziar_lixeira",
            // Carga em lote grava, e grava muito.
            "inserir_lote",
            // Reservar a tabela e declarar que vai gravar.
            "bulkinsert",
        ] {
            assert!(
                OPS_ESCRITA.contains(&op),
                "{op:?} grava e nao esta em OPS_ESCRITA: \
                 um servidor somente-leitura aceitaria"
            );
        }
    }

    #[test]
    fn o_que_so_le_fica_fora_da_lista() {
        for op in [
            "ping",
            "config",
            "bancos",
            "tabelas",
            "esquema",
            "ler",
            "varrer",
            "buscar",
            "diario",
            "verificar",
            "painel",
            "sistema",
            "acessos",
            "usuarios",
            "sistabelas",
            "siscolunas",
            "pivotar",
            "sequencias",
            "juntar",
            "unir",
            "estatisticas",
            "checksum",
            "exportar",
            "sessoes",
            "sistema",
            // Listar a lixeira e os motivos so le -- e e exatamente o que se
            // quer poder fazer num espelho somente-leitura, investigando.
            "lixeira",
            "motivos",
            // Conferir a carga nao grava nada -- e justamente o que se quer
            // poder fazer antes de decidir gravar.
            "importar_conferir",
            "dblink",
            "dblink_testar",
            "dblink_tabelas",
            "dblink_estrutura",
            "dblink_ler",
            // Nao esta na lista de proposito: por ela passa tanto consulta
            // quanto escrita, e a propria operacao confere qual e -- barrando
            // a escrita quando este servidor esta somente-leitura, mas
            // deixando a LEITURA funcionar num espelho.
            "dblink_consultar",
        ] {
            assert!(
                !OPS_ESCRITA.contains(&op),
                "{op:?} so le e esta na lista de escrita: \
                 um servidor somente-leitura recusaria sem motivo"
            );
        }
    }
}

#[cfg(test)]
mod testes_criar_qualificada {
    use super::*;

    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        Servidor::novo(c).unwrap()
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    /// `filial.clientes` e o schema `filial` mais a tabela `clientes`.
    ///
    /// Antes desta correcao a criacao tomava o ponto como parte do NOME e
    /// gravava `filial.clientes.reg` na raiz do banco. O servidor respondia
    /// "criada", e nenhuma outra operacao conseguia abrir a tabela -- toda
    /// leitura separa o ponto, e so a criacao nao separava. Uma tabela que
    /// nasce inalcancavel e pior do que um erro.
    #[test]
    fn criar_com_nome_qualificado_cai_no_schema() {
        let dir = std::env::temp_dir().join(format!("phx-qualif-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = servidor(&dir);
        let sessao = Sessao::default();

        s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
            .unwrap();
        let r = s
            .executar(
                "criar_tabela",
                &pedido(
                    r#"{"database":"loja","tabela":"filial.clientes",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
                ),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.texto_ou("schema", ""), "filial");
        assert_eq!(r.texto_ou("tabela", ""), "filial.clientes");

        // Os arquivos foram para o diretorio do schema, com o nome curto.
        assert!(
            dir.join("loja/filial/clientes.reg").exists(),
            "o .reg nao caiu no schema"
        );
        assert!(
            !dir.join("loja/filial.clientes.reg").exists(),
            "o ponto virou parte do nome do arquivo de novo"
        );

        // E a prova que importa: o que foi criado da para abrir e gravar.
        s.executar(
            "inserir",
            &pedido(r#"{"database":"loja","tabela":"filial.clientes","linha":{"id":1}}"#),
            &sessao,
        )
        .unwrap();
        let v = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"loja","tabela":"filial.clientes"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(v.campo("linhas").and_then(Json::lista).unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O campo `schema` continua valendo, e vale igual.
    #[test]
    fn o_campo_schema_continua_valendo() {
        let dir = std::env::temp_dir().join(format!("phx-qualif2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = servidor(&dir);
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"loja","schema":"matriz","tabela":"estoque",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        assert!(dir.join("loja/matriz/estoque.reg").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Dizer duas coisas diferentes e erro, e nao "uma delas ganha".
    #[test]
    fn nome_e_campo_em_desacordo_param_a_criacao() {
        let dir = std::env::temp_dir().join(format!("phx-qualif3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = servidor(&dir);
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &sessao)
            .unwrap();
        let e = s
            .executar(
                "criar_tabela",
                &pedido(
                    r#"{"database":"loja","schema":"matriz","tabela":"filial.estoque",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
                ),
                &sessao,
            )
            .unwrap_err()
            .to_string();
        assert!(e.contains("escolha um dos dois"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod testes_exclusao {
    use super::*;
    use crate::usuarios::{Cadastro, Nivel, Permissoes, Usuario};

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-excl-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            cadastro,
            ..Config::default()
        };
        Servidor::novo(c).unwrap()
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    /// Um banco com uma tabela de tres linhas.
    fn com_dados(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
        let s = servidor(dir, cadastro);
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        for (id, nome) in [(1, "Adriano"), (2, "Maria"), (3, "Joao")] {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","linha":{{"id":{id},"nome":"{nome}"}}}}"#
                )),
                &sessao,
            )
            .unwrap();
        }
        s
    }

    /// O padrao do protocolo e o caminho REVERSIVEL. Um cliente que manda
    /// `excluir` sem dizer mais nada nao pode perder o dado.
    #[test]
    fn excluir_sem_dizer_nada_e_suave() {
        let dir = dir_temp("padrao");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();

        let r = s
            .executar(
                "excluir",
                &pedido(r#"{"database":"b","tabela":"c","rowid":2,"motivo":"pedido"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.texto_ou("modo", ""), "suave");
        assert!(matches!(r.campo("reversivel"), Some(Json::Bool(true))));

        // Sumiu da varredura...
        let v = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(v.inteiro_ou("devolvidas", -1), 2);

        // ... e a lixeira continua vazia, porque nada foi apagado.
        let lx = s
            .executar(
                "lixeira",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(lx.inteiro_ou("total", -1), 0);

        // E volta.
        let r = s
            .executar(
                "restaurar",
                &pedido(r#"{"database":"b","tabela":"c","rowid":2,"motivo":"engano"}"#),
                &sessao,
            )
            .unwrap();
        assert!(matches!(r.campo("restaurado"), Some(Json::Bool(true))));
        let v = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(v.inteiro_ou("devolvidas", -1), 3);
    }

    #[test]
    fn excluir_fisico_passa_pela_lixeira() {
        let dir = dir_temp("fisico");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();

        let r = s
            .executar(
                "excluir",
                &pedido(
                    r#"{"database":"b","tabela":"c","rowid":2,
                        "fisico":true,"motivo":"duplicidade"}"#,
                ),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.texto_ou("modo", ""), "fisico");
        assert!(matches!(r.campo("na_lixeira"), Some(Json::Bool(true))));

        let lx = s
            .executar(
                "lixeira",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(lx.inteiro_ou("total", -1), 1);
        let itens = lx.campo("descartadas").and_then(Json::lista).unwrap();
        assert_eq!(itens[0].inteiro_ou("rowid", -1), 2);
        // A linha vem decodificada, com o esquema da tabela.
        let linha = itens[0].campo("linha").unwrap();
        assert_eq!(linha.texto_ou("nome", ""), "Maria");
        assert_eq!(itens[0].texto_ou("aviso", "x"), "");

        // E o motivo ficou registrado, com a identidade da linha.
        let m = s
            .executar(
                "motivos",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        let regs = m.campo("motivos").and_then(Json::lista).unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].texto_ou("tipo", ""), "fisica");
        assert_eq!(regs[0].texto_ou("motivo", ""), "duplicidade");
        assert_eq!(regs[0].texto_ou("identidade", ""), "id=2");
    }

    /// O defeito que este teste protege: `atualizar` monta a linha inteira a
    /// partir do JSON, e a coluna de sistema ausente virava `false`. Uma
    /// edicao de rotina RESSUSCITARIA a linha, sem erro e sem aviso.
    #[test]
    fn atualizar_nao_ressuscita_linha_excluida() {
        let dir = dir_temp("ressuscita");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();

        s.executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":2}"#),
            &sessao,
        )
        .unwrap();
        s.executar(
            "atualizar",
            &pedido(r#"{"database":"b","tabela":"c","rowid":2,"linha":{"id":2,"nome":"Outra"}}"#),
            &sessao,
        )
        .unwrap();

        let v = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(
            v.inteiro_ou("devolvidas", -1),
            2,
            "a alteracao ressuscitou a linha excluida"
        );
    }

    #[test]
    fn motivo_obrigatorio_vem_do_esquema() {
        let dir = dir_temp("obrigatorio");
        let s = servidor(&dir, Cadastro::default());
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"c","motivo_obrigatorio":true,
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        s.executar(
            "inserir",
            &pedido(r#"{"database":"b","tabela":"c","linha":{"id":1}}"#),
            &sessao,
        )
        .unwrap();

        let e = s
            .executar(
                "excluir",
                &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
                &sessao,
            )
            .unwrap_err();
        assert!(format!("{e}").contains("motivo"), "{e}");

        // A linha continua viva: a recusa veio antes de qualquer gravacao.
        let v = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(v.inteiro_ou("devolvidas", -1), 1);

        // E a tela sabe que a tabela exige, para pedir antes de mandar.
        let m = s
            .executar(
                "motivos",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert!(matches!(
            m.campo("motivo_obrigatorio"),
            Some(Json::Bool(true))
        ));
    }

    /// Esvaziar apaga sem volta -- e por isso exige a frase escrita, mesmo
    /// numa tabela que nao exige motivo para excluir.
    #[test]
    fn esvaziar_exige_motivo_e_registra_antes_de_apagar() {
        let dir = dir_temp("esvaziar");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();
        s.executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1,"fisico":true}"#),
            &sessao,
        )
        .unwrap();

        let e = s
            .executar(
                "esvaziar_lixeira",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap_err();
        assert!(format!("{e}").contains("motivo"), "{e}");

        let r = s
            .executar(
                "esvaziar_lixeira",
                &pedido(r#"{"database":"b","tabela":"c","motivo":"limpeza anual"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("apagadas", -1), 1);

        let lx = s
            .executar(
                "lixeira",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(lx.inteiro_ou("total", -1), 0);

        // O dado foi; o rastro de que foi, nao.
        let m = s
            .executar(
                "motivos",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        let regs = m.campo("motivos").and_then(Json::lista).unwrap();
        assert!(regs.iter().any(|r| r.texto_ou("tipo", "") == "expurgo"));
    }

    /// O portao de permissao: quem so le e escreve nao ve a lixeira nem os
    /// motivos. E o requisito de "somente o administrador visualiza".
    #[test]
    fn lixeira_e_motivos_exigem_administrar() {
        for op in ["lixeira", "trash", "motivos", "reasons", "esvaziar_lixeira"] {
            assert_eq!(
                Atividade::da_operacao(op),
                Some(Atividade::Administrar),
                "{op:?} nao exige administrar"
            );
        }
        // Excluir e restaurar continuam no poder de excluir, que e o certo:
        // quem pode tirar da lista pode devolver.
        assert_eq!(Atividade::da_operacao("excluir"), Some(Atividade::Excluir));
        assert_eq!(
            Atividade::da_operacao("restaurar"),
            Some(Atividade::Excluir)
        );
    }

    /// A prova da paginacao por cursor: pedir pagina a pagina reconstroi
    /// exatamente a tabela, sem repetir nem pular -- inclusive por cima dos
    /// buracos que a exclusao deixa.
    #[test]
    fn o_cursor_reconstroi_a_tabela_inteira() {
        let dir = dir_temp("cursor");
        let s = servidor(&dir, Cadastro::default());
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        for id in 1..=25 {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","linha":{{"id":{id}}}}}"#
                )),
                &sessao,
            )
            .unwrap();
        }
        // Dois buracos: um marcado, um apagado de vez.
        s.executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":7}"#),
            &sessao,
        )
        .unwrap();
        s.executar(
            "excluir",
            &pedido(r#"{"database":"b","tabela":"c","rowid":13,"fisico":true}"#),
            &sessao,
        )
        .unwrap();

        let mut vistos: Vec<i64> = Vec::new();
        let mut cursor = 0i64;
        let mut paginas = 0;
        loop {
            let r = s
                .executar(
                    "varrer",
                    &pedido(&format!(
                        r#"{{"database":"b","tabela":"c","max":7,"depois":{cursor}}}"#
                    )),
                    &sessao,
                )
                .unwrap();
            assert_eq!(r.texto_ou("modo", ""), "cursor");
            let linhas: Vec<Json> = r.campo("linhas").and_then(Json::lista).unwrap().to_vec();
            if linhas.is_empty() {
                assert!(
                    !matches!(r.campo("ha_mais"), Some(Json::Bool(true))),
                    "disse que ha mais e devolveu vazio"
                );
                break;
            }
            paginas += 1;
            assert!(paginas < 20, "nao terminou -- o cursor nao anda");
            for l in &linhas {
                vistos.push(l.inteiro_ou("id", -1));
            }
            cursor = r.inteiro_ou("cursor_fim", 0);
        }

        let esperado: Vec<i64> = (1..=25).filter(|i| *i != 7 && *i != 13).collect();
        assert_eq!(vistos, esperado, "o cursor pulou ou repetiu linha");
        assert_eq!(paginas, 4, "23 linhas em paginas de 7 dao 4 paginas");
    }

    /// `registros` sai do cabecalho e nao de varredura: e o numero que a tela
    /// mostra sem pagar por ele.
    #[test]
    fn varrer_nao_conta_a_tabela_para_responder() {
        let dir = dir_temp("sem-contar");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();
        let r = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c","max":2}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("devolvidas", -1), 2);
        assert_eq!(r.inteiro_ou("registros", -1), 3);
        assert!(matches!(r.campo("ha_mais"), Some(Json::Bool(true))));
        assert!(matches!(r.campo("ha_antes"), Some(Json::Bool(false))));

        // E a pagina de tras devolve o que veio antes, em ordem crescente.
        let fim = r.inteiro_ou("cursor_fim", 0);
        let atras = s
            .executar(
                "varrer",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","max":5,"antes":{fim}}}"#
                )),
                &sessao,
            )
            .unwrap();
        let ids: Vec<i64> = atras
            .campo("linhas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|l| l.inteiro_ou("id", -1))
            .collect();
        assert_eq!(ids, vec![1]);
    }

    /// O `rownum` chega na resposta e cresce com a ordem de digitacao.
    #[test]
    fn a_resposta_traz_o_numero_de_ordem() {
        let dir = dir_temp("rownum");
        let s = com_dados(&dir, Cadastro::default());
        let sessao = Sessao::default();
        let r = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        let nums: Vec<i64> = r
            .campo("linhas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|l| l.inteiro_ou("rownum", -1))
            .collect();
        assert_eq!(nums, vec![1, 2, 3]);
    }

    /// E o portao de verdade, com um usuario que tem tudo menos administrar.
    #[test]
    fn operador_sem_administrar_e_recusado_na_lixeira() {
        let dir = dir_temp("portao");
        let mut cadastro = Cadastro::default();
        let permissoes = Permissoes {
            ler: true,
            inserir: true,
            alterar: true,
            excluir: true,
            administrar: false,
            ..Permissoes::default()
        };
        cadastro.usuarios.push(Usuario {
            id: 7,
            nome: "Operador".into(),
            login: "op".into(),
            senha_hash: String::new(),
            email: String::new(),
            telefone: String::new(),
            supervisor: false,
            ativo: true,
            nivel: Nivel::Nenhum,
            chave_publica: None,
            bases: vec![("*".into(), permissoes)],
            tabelas: Vec::new(),
        });
        let usuario = cadastro.usuarios[0].clone();
        let s = com_dados(&dir, cadastro);

        let mut sessao = Sessao {
            usuario: Some(usuario),
            ..Sessao::default()
        };
        // Pelo `despachar`, que e por onde o pedido entra de verdade: e ali
        // que mora o portao de permissao, e nao no `executar`.
        let (_, _, r) = s.despachar(
            r#"{"op":"lixeira","token":"t","database":"b","tabela":"c"}"#,
            &mut sessao,
            "1.2.3.4",
        );
        let e = r.unwrap_err();
        assert!(
            format!("{e}").contains("administrar"),
            "o operador entrou na lixeira: {e}"
        );

        let (_, _, r) = s.despachar(
            r#"{"op":"motivos","token":"t","database":"b","tabela":"c"}"#,
            &mut sessao,
            "1.2.3.4",
        );
        assert!(r.is_err(), "o operador leu os motivos");

        // Mas excluir ele pode.
        let (_, _, r) = s.despachar(
            r#"{"op":"excluir","token":"t","database":"b","tabela":"c","rowid":1}"#,
            &mut sessao,
            "1.2.3.4",
        );
        assert!(r.is_ok(), "o operador nao conseguiu excluir: {r:?}");
    }
}

/// A janela de conflito de escrita, pelo protocolo.
///
/// O que estes testes travam nao e so o "recusa quando a versao e velha" --
/// e principalmente o **contrario**: o cliente que nao manda versao nenhuma
/// tem de continuar gravando como sempre gravou. Uma guarda que quebra todo
/// cliente antigo nao e protecao, e um estrago.
#[cfg(test)]
mod testes_conflito {
    use super::*;
    use crate::usuarios::Cadastro;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-conf-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    /// Um banco com uma tabela de uma linha, e a sessao para mexer nela.
    fn com_uma_linha(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            cadastro: Cadastro::default(),
            ..Config::default()
        };
        let s = Servidor::novo(c).unwrap();
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        s.executar(
            "inserir",
            &pedido(r#"{"database":"b","tabela":"c","linha":{"id":1,"nome":"Adriano"}}"#),
            &sessao,
        )
        .unwrap();
        s
    }

    /// Quem nao pede versao recebe a linha crua, como sempre recebeu.
    #[test]
    fn ler_sem_pedir_versao_nao_muda_de_forma() {
        let dir = dir_temp("forma");
        let s = com_uma_linha(&dir);
        let r = s
            .executar(
                "ler",
                &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
                &Sessao::default(),
            )
            .unwrap();
        assert_eq!(r.texto_ou("nome", ""), "Adriano");
        assert!(r.campo("versao").is_none(), "a versao vazou na linha crua");
        assert!(r.campo("linha").is_none(), "a forma da resposta mudou");
    }

    #[test]
    fn ler_com_versao_devolve_a_linha_e_a_versao() {
        let dir = dir_temp("com-versao");
        let s = com_uma_linha(&dir);
        let r = s
            .executar(
                "ler",
                &pedido(r#"{"database":"b","tabela":"c","rowid":1,"com_versao":true}"#),
                &Sessao::default(),
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("versao", -1), 1);
        assert_eq!(r.inteiro_ou("rowid", -1), 1);
        assert_eq!(
            r.campo("linha").unwrap().texto_ou("nome", ""),
            "Adriano",
            "a linha nao veio dentro do envelope"
        );
    }

    /// O cliente antigo -- o que nao sabe o que e versao -- continua gravando.
    #[test]
    fn atualizar_sem_versao_continua_gravando() {
        let dir = dir_temp("antigo");
        let s = com_uma_linha(&dir);
        let sessao = Sessao::default();
        for nome in ["Maria", "Joao"] {
            s.executar(
                "atualizar",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"c","rowid":1,"linha":{{"id":1,"nome":"{nome}"}}}}"#
                )),
                &sessao,
            )
            .unwrap();
        }
        let r = s
            .executar(
                "ler",
                &pedido(r#"{"database":"b","tabela":"c","rowid":1}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.texto_ou("nome", ""), "Joao");
    }

    /// A resposta do `atualizar` traz a versao nova: quem grava duas vezes
    /// seguidas nao precisa reler a linha inteira no meio.
    #[test]
    fn atualizar_devolve_a_versao_nova() {
        let dir = dir_temp("devolve");
        let s = com_uma_linha(&dir);
        let r = s
            .executar(
                "atualizar",
                &pedido(
                    r#"{"database":"b","tabela":"c","rowid":1,
                        "linha":{"id":1,"nome":"Maria"},"versao":1}"#,
                ),
                &Sessao::default(),
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("versao", -1), 2);
    }

    /// Os dois leem a versao 1; o segundo chega depois e e recusado.
    #[test]
    fn atualizar_com_versao_velha_recusa() {
        let dir = dir_temp("velha");
        let s = com_uma_linha(&dir);
        let sessao = Sessao::default();
        s.executar(
            "atualizar",
            &pedido(
                r#"{"database":"b","tabela":"c","rowid":1,
                    "linha":{"id":1,"nome":"Maria"},"versao":1}"#,
            ),
            &sessao,
        )
        .unwrap();

        let e = s
            .executar(
                "atualizar",
                &pedido(
                    r#"{"database":"b","tabela":"c","rowid":1,
                        "linha":{"id":1,"nome":"Joao"},"versao":1}"#,
                ),
                &sessao,
            )
            .unwrap_err();
        assert_eq!(e.codigo(), 3004);
        assert_eq!(e.nome(), "CONFLITO");

        // E nada foi gravado: o trabalho do primeiro esta inteiro.
        let r = s
            .executar(
                "ler",
                &pedido(r#"{"database":"b","tabela":"c","rowid":1,"com_versao":true}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.campo("linha").unwrap().texto_ou("nome", ""), "Maria");
        assert_eq!(r.inteiro_ou("versao", -1), 2);
    }

    /// Excluir uma linha que outra pessoa acabou de alterar e a mesma janela.
    #[test]
    fn excluir_com_versao_velha_recusa() {
        let dir = dir_temp("excluir");
        let s = com_uma_linha(&dir);
        let sessao = Sessao::default();
        s.executar(
            "atualizar",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1,"linha":{"id":1,"nome":"Maria"}}"#),
            &sessao,
        )
        .unwrap();

        let e = s
            .executar(
                "excluir",
                &pedido(r#"{"database":"b","tabela":"c","rowid":1,"versao":1}"#),
                &sessao,
            )
            .unwrap_err();
        assert_eq!(e.nome(), "CONFLITO");

        // A linha continua la, e nao marcada.
        let r = s
            .executar(
                "varrer",
                &pedido(r#"{"database":"b","tabela":"c"}"#),
                &sessao,
            )
            .unwrap();
        assert_eq!(r.inteiro_ou("devolvidas", -1), 1);
    }

    /// Zero e ausente sao a mesma coisa. Sem isto, um cliente que guarda a
    /// versao num campo numerico nao inicializado gravaria sempre -- ou nunca.
    #[test]
    fn versao_zero_e_o_mesmo_que_nao_mandar() {
        let dir = dir_temp("zero");
        let s = com_uma_linha(&dir);
        let sessao = Sessao::default();
        s.executar(
            "atualizar",
            &pedido(r#"{"database":"b","tabela":"c","rowid":1,"linha":{"id":1,"nome":"Maria"}}"#),
            &sessao,
        )
        .unwrap();
        s.executar(
            "atualizar",
            &pedido(
                r#"{"database":"b","tabela":"c","rowid":1,
                    "linha":{"id":1,"nome":"Joao"},"versao":0}"#,
            ),
            &sessao,
        )
        .unwrap();
    }
}

/// Direito no nivel da TABELA.
///
/// Ate a 0.17.0 a permissao parava na base: quem lia a base lia todas as
/// tabelas dela. A folha de pagamento e a tabela de clientes moram no mesmo
/// banco porque o negocio e um so, e o direito de ler as duas nao e o mesmo.
///
/// O que estes testes travam, em ordem de importancia:
///
/// 1. a regra da tabela **tira** de quem le a base inteira;
/// 2. a regra da tabela **da** a quem nao le a base nenhuma;
/// 3. `juntar` e `unir` **nao sao a porta dos fundos** -- as tabelas delas nao
///    passam pelo campo que o portao geral olha;
/// 4. a arvore e o catalogo **escondem** o que nao da para abrir;
/// 5. um `config.json` sem regra de tabela continua se comportando igual.
#[cfg(test)]
mod testes_direito_por_tabela {
    use super::*;
    use crate::usuarios::Cadastro;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-dt-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    /// O cadastro sai do JSON, e nao de uma struct montada a mao: assim o
    /// teste tambem exercita a LEITURA do `config.json`, que e onde o direito
    /// por tabela e escrito de verdade.
    fn cadastro(bases: &str) -> Cadastro {
        Cadastro::de_json(&pedido(&format!(
            r#"{{"usuarios":[{{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
        )))
        .unwrap()
    }

    /// Uma base `b` com duas tabelas: `clientes` e `folha`.
    fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> (Arc<Servidor>, Sessao) {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            cadastro: cadastro.clone(),
            ..Config::default()
        };
        let s = Servidor::novo(c).unwrap();
        let dono = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
            .unwrap();
        for tab in ["clientes", "folha"] {
            s.executar(
                "criar_tabela",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{tab}",
                        "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                                   {{"nome":"nome","tipo":"Str(20)"}}],
                        "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                     "primario":true}}]}}"#
                )),
                &dono,
            )
            .unwrap();
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{tab}","linha":{{"id":1,"nome":"x"}}}}"#
                )),
                &dono,
            )
            .unwrap();
        }
        let sessao = Sessao {
            usuario: cadastro.por_login("ana").cloned(),
            ..Sessao::default()
        };
        (s, sessao)
    }

    /// Pelo `despachar`, que e por onde o pedido entra de verdade: e ali que
    /// mora o portao, e nao no `executar`.
    fn pede(s: &Arc<Servidor>, sessao: &Sessao, corpo: &str) -> Result<Json> {
        let mut ses = Sessao {
            usuario: sessao.usuario.clone(),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        r
    }

    /// O caso do enunciado: le a base inteira, menos a folha.
    #[test]
    fn a_regra_da_tabela_tira_de_quem_le_a_base() {
        let dir = dir_temp("tira");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
        );

        assert!(pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#
        )
        .is_ok());
        let e = pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
        )
        .unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO");
        assert!(
            e.to_string().contains("b.folha"),
            "a recusa tem de dizer QUAL tabela: {e}"
        );
    }

    /// E o contrario, que e o caso que a intersecao nao resolveria: nao le a
    /// base nenhuma, le uma tabela.
    #[test]
    fn a_regra_da_tabela_da_a_quem_nao_le_a_base() {
        let dir = dir_temp("da");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"b":{"tabelas":{"clientes":{"ler":true}}}}"#),
        );

        assert!(pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#
        )
        .is_ok());
        assert!(pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"folha","rowid":1"#
        )
        .is_err());
        // E continua sem poder GRAVAR na que le.
        assert!(pede(
            &s,
            &ses,
            r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":2}"#
        )
        .is_err());
    }

    /// `"*"` de tabela vale para as nao listadas, como o `"*"` de base.
    #[test]
    fn a_estrela_de_tabela_vale_para_as_nao_listadas() {
        let dir = dir_temp("estrela");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"*":{},"clientes":{"ler":true}}}}"#),
        );
        assert!(pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#
        )
        .is_ok());
        assert!(pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"folha","rowid":1"#
        )
        .is_err());
    }

    /// A arvore mostra o que da para abrir, e nao o que existe.
    #[test]
    fn a_arvore_esconde_a_tabela_negada() {
        let dir = dir_temp("arvore");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
        );
        let r = pede(&s, &ses, r#""op":"tabelas","database":"b""#).unwrap();
        let nomes: Vec<String> = r
            .campo("tabelas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .filter_map(|x| x.texto().map(str::to_string))
            .collect();
        assert_eq!(nomes, vec!["clientes".to_string()], "veio {nomes:?}");
    }

    /// **A op `sql` NAO e a porta dos fundos.** Ana le `clientes` e nao le
    /// `folha`; escrever o nome da folha dentro de um SELECT nao muda isso.
    ///
    /// Este e o teste que importa do item inteiro: se ele passar a falhar,
    /// alguem trocou o `executar_derivado` por uma leitura direta da tabela.
    #[test]
    fn o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada() {
        let dir = dir_temp("sql-porta");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
        );

        let ok = pede(
            &s,
            &ses,
            r#""op":"sql","database":"b","texto":"SELECT * FROM clientes""#,
        )
        .expect("a tabela permitida tinha de passar");
        assert_eq!(ok.inteiro_ou("devolvidas", -1), 1);

        let e = pede(
            &s,
            &ses,
            r#""op":"sql","database":"b","texto":"SELECT * FROM folha""#,
        )
        .expect_err("o SELECT leu a tabela negada");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
        assert!(format!("{e}").contains("folha"), "{e}");
    }

    /// O endereco de tres partes -- `banco.schema.tabela` -- tambem nao
    /// contorna nada: a permissao e conferida contra o banco que o SELECT
    /// escolheu, e nao contra o do envelope. Sem isto, o campo `database` do
    /// pedido seria enfeite e o SQL escolheria sozinho onde ler.
    #[test]
    fn o_banco_do_from_e_o_banco_da_permissao() {
        let dir = dir_temp("sql-from-db");
        let (s, ses) = servidor(&dir, cadastro(r#"{"b":{"ler":true}}"#));
        let e = pede(
            &s,
            &ses,
            r#""op":"sql","database":"b","texto":"SELECT * FROM outra.filial.clientes""#,
        )
        .expect_err("leu de um banco que nao esta na regra");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
        assert!(format!("{e}").contains("outra"), "{e}");
    }

    /// A op `catalogo` mostra so o que a sessao consegue chamar.
    ///
    /// Um leitor nao pode ver `excluir_tabela` na lista: oferecer a operacao
    /// que o portao vai negar e mandar o cliente montar um pedido para ouvir
    /// nao. E `ler`, que ele pode, tem de estar la -- esconder demais seria o
    /// mesmo estrago do outro lado.
    #[test]
    fn o_catalogo_lista_so_o_que_a_sessao_pode_chamar() {
        let dir = dir_temp("cat-op");
        let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true}}"#));
        let r = pede(&s, &ses, r#""op":"catalogo","database":"b""#).unwrap();
        let nomes: Vec<String> = r
            .campo("operacoes")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|o| o.texto_ou("nome", "").to_string())
            .collect();
        assert!(nomes.contains(&"ler".to_string()), "{nomes:?}");
        assert!(nomes.contains(&"varrer".to_string()), "{nomes:?}");
        assert!(
            !nomes.contains(&"excluir_tabela".to_string()),
            "o leitor viu uma operacao de administrador: {nomes:?}"
        );
        assert!(
            !nomes.contains(&"inserir".to_string()),
            "o leitor viu uma operacao de escrita: {nomes:?}"
        );
        // E o numero do que ficou de fora, para quem ve a lista curta saber
        // que ela e curta por permissao, e nao por o servidor ser pequeno.
        assert!(r.inteiro_ou("ocultas", 0) > 0);
        assert_eq!(r.inteiro_ou("total", -1), nomes.len() as i64);
    }

    /// `catalogo` com `"op"` detalha uma so -- e e o que o `/help <comando>`
    /// do console usa. Operacao que o usuario nao pode chamar responde POR QUE,
    /// em vez de fingir que nao existe: fingir manda procurar erro de
    /// digitacao onde nao ha.
    #[test]
    fn o_catalogo_detalha_uma_operacao_e_diz_por_que_negou() {
        let dir = dir_temp("cat-uma");
        let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true}}"#));

        let uma = pede(
            &s,
            &ses,
            r#""op":"catalogo","database":"b","operacao":"buscar""#,
        )
        .unwrap()
        .campo("operacao")
        .cloned()
        .unwrap();
        assert_eq!(uma.texto_ou("nome", ""), "buscar");
        assert!(!uma.texto_ou("exemplo", "").is_empty());
        assert!(uma
            .campo("parametros")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .any(|p| p.texto_ou("nome", "") == "indice"));

        let negada = pede(
            &s,
            &ses,
            r#""op":"catalogo","database":"b","operacao":"excluir_tabela""#,
        )
        .unwrap();
        assert!(negada.campo("operacao").unwrap().e_nulo());
        assert!(
            negada.texto_ou("motivo", "").contains("administrar"),
            "{}",
            negada.escrever()
        );
    }

    /// O catalogo e a mesma lista por outra porta.
    #[test]
    fn o_catalogo_esconde_a_tabela_negada() {
        let dir = dir_temp("catalogo");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
        );
        for op in ["sistabelas", "siscolunas"] {
            let r = pede(&s, &ses, &format!(r#""op":"{op}","database":"b""#)).unwrap();
            let texto = r.escrever();
            assert!(
                !texto.contains("folha"),
                "{op} vazou a tabela negada: {texto}"
            );
            assert!(texto.contains("clientes"), "{op} escondeu demais");
        }
    }

    /// Junção nao tem campo `tabela`: as duas moram em `a.tabela` e `b.tabela`.
    /// Sem a conferencia propria, bastaria pedir a folha como lado B.
    #[test]
    fn juntar_nao_e_a_porta_dos_fundos() {
        let dir = dir_temp("juntar");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
        );
        let e = pede(
            &s,
            &ses,
            r#""op":"juntar","database":"b",
               "a":{"tabela":"clientes","chave":"id"},
               "b":{"tabela":"folha","chave":"id"}"#,
        )
        .unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO");
        assert!(e.to_string().contains("b.folha"), "veio {e}");
    }

    /// União tambem nao tem campo `tabela`: tem uma LISTA.
    #[test]
    fn unir_nao_e_a_porta_dos_fundos() {
        let dir = dir_temp("unir");
        let (s, ses) = servidor(
            &dir,
            cadastro(r#"{"*":{"ler":true,"tabelas":{"folha":{}}}}"#),
        );
        let e = pede(
            &s,
            &ses,
            r#""op":"unir","database":"b","tabelas":["clientes","folha"]"#,
        )
        .unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO");
        assert!(e.to_string().contains("b.folha"), "veio {e}");
    }

    /// O `config.json` que ja existia continua se comportando igual. E o teste
    /// que importa: uma regra nova que muda o significado da configuracao
    /// antiga tira o direito de alguem sem ninguem ter pedido.
    #[test]
    fn sem_regra_de_tabela_nada_muda() {
        let dir = dir_temp("igual");
        let (s, ses) = servidor(&dir, cadastro(r#"{"*":{"ler":true}}"#));
        for tab in ["clientes", "folha"] {
            assert!(
                pede(
                    &s,
                    &ses,
                    &format!(r#""op":"ler","database":"b","tabela":"{tab}","rowid":1"#)
                )
                .is_ok(),
                "{tab} deixou de ser legivel"
            );
        }
        let r = pede(&s, &ses, r#""op":"tabelas","database":"b""#).unwrap();
        assert_eq!(r.campo("tabelas").and_then(Json::lista).unwrap().len(), 2);
    }

    /// Supervisor passa por cima de qualquer regra de tabela -- como ja passa
    /// por cima da regra de base.
    #[test]
    fn supervisor_passa_por_cima() {
        let dir = dir_temp("super");
        let c = Cadastro::de_json(&pedido(
            r#"{"usuarios":[{"login":"ana","id":9,"supervisor":true,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"tabelas":{"folha":{}}}}}]}"#,
        ))
        .unwrap();
        let (s, ses) = servidor(&dir, c);
        assert!(pede(
            &s,
            &ses,
            r#""op":"ler","database":"b","tabela":"folha","rowid":1"#
        )
        .is_ok());
    }
}

/// Observacao que nao esta ligada nao pode custar nada.
///
/// Ate a 0.17.0 custava, e escondido: todo pedido pagava dois `Json::analisar`
/// do corpo inteiro, tres `String` e um mutex ANTES de `chegou` olhar `ligado`
/// e devolver `None`. Num `inserir_lote` de cinco mil linhas era analisar meio
/// megabyte de JSON duas vezes, para nada -- medido em 7% da carga pela rede
/// (`bancada/carga/medir.py`).
///
/// O portao barato e um `AtomicBool`, e o que pode dar errado nele e DIVERGIR
/// do estado real: preso em `true` faz o servidor pagar o parse para sempre;
/// preso em `false` faz o profiler nao ver nada, ligado. E isso que estes
/// testes travam.
///
/// A captura em si mora no laco da conexao, e nao no `despachar` -- entao ela
/// nao se exercita daqui. Quem a exercita e a bancada, e o numero dela e o que
/// denuncia se o portao sumir.
#[cfg(test)]
mod testes_profiler_desligado {
    use super::*;
    use crate::usuarios::Cadastro;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-prof-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            cadastro: Cadastro::default(),
            ..Config::default()
        };
        Servidor::novo(c).unwrap()
    }

    /// O espelho e o estado de verdade, lado a lado.
    fn conferir(s: &Arc<Servidor>, esperado: bool) {
        let real = s.profiler.lock().unwrap().ligado();
        let espelho = s.profiler_ligado.load(Ordering::Relaxed);
        assert_eq!(real, esperado, "o profiler de verdade");
        assert_eq!(
            espelho, real,
            "o espelho divergiu: espelho={espelho}, real={real}"
        );
    }

    /// Nasce desligado, senao o caminho quente pagaria desde o arranque por uma
    /// observacao que ninguem pediu.
    #[test]
    fn nasce_desligado() {
        let dir = dir_temp("nasce");
        conferir(&servidor(&dir), false);
    }

    /// Ligar e desligar, varias voltas: o espelho acompanha em todas.
    #[test]
    fn o_espelho_nunca_diverge() {
        let dir = dir_temp("espelho");
        let s = servidor(&dir);
        let sessao = Sessao::default();
        for _ in 0..3 {
            s.executar("profiler_ligar", &pedido("{}"), &sessao)
                .unwrap();
            conferir(&s, true);
            s.executar("profiler_desligar", &pedido("{}"), &sessao)
                .unwrap();
            conferir(&s, false);
        }
    }

    /// Ligar duas vezes seguidas nao pode deixar o espelho para tras -- nem
    /// desligar duas vezes.
    #[test]
    fn ligar_ou_desligar_repetido_nao_confunde_o_espelho() {
        let dir = dir_temp("repetido");
        let s = servidor(&dir);
        let sessao = Sessao::default();

        s.executar("profiler_ligar", &pedido("{}"), &sessao)
            .unwrap();
        s.executar("profiler_ligar", &pedido("{}"), &sessao)
            .unwrap();
        conferir(&s, true);

        s.executar("profiler_desligar", &pedido("{}"), &sessao)
            .unwrap();
        s.executar("profiler_desligar", &pedido("{}"), &sessao)
            .unwrap();
        conferir(&s, false);
    }

    /// Ligar com filtro tambem liga o espelho: o filtro decide o que ENTRA no
    /// anel, e nao se a observacao existe.
    #[test]
    fn ligar_com_filtro_tambem_liga_o_espelho() {
        let dir = dir_temp("filtro");
        let s = servidor(&dir);
        s.executar(
            "profiler_ligar",
            &pedido(r#"{"database":"b","so_escrita":true}"#),
            &Sessao::default(),
        )
        .unwrap();
        conferir(&s, true);
    }

    /// Ligar que FALHA nao pode ligar o espelho -- senao o servidor pagaria o
    /// parse por uma observacao que nunca existiu.
    #[test]
    fn ligar_que_falha_nao_liga_o_espelho() {
        let dir = dir_temp("falha");
        let s = servidor(&dir);
        let r = s.executar(
            "profiler_ligar",
            &pedido(r#"{"arquivo":"/diretorio/que/nao/existe/prof.txt"}"#),
            &Sessao::default(),
        );
        assert!(r.is_err(), "aceitou um caminho que nao existe");
        conferir(&s, false);
    }
}

/// `BULKINSERT`: a tabela reservada para uma carga.
///
/// O que estes testes travam, em ordem de importancia:
///
/// 1. **a reserva de fato barra o outro**, e o recado diz quem reservou;
/// 2. **o dono continua trabalhando** -- reserva que barra o proprio dono seria
///    so uma forma cara de derrubar o servico;
/// 3. **a queda da conexao solta** -- e a primeira rede contra reserva orfa;
/// 4. **o prazo solta** -- e a segunda, para o soquete pendurado vivo;
/// 5. **o erro e repetivel**: `EM_CARGA` diz `repetir: true`, e e o que separa
///    «espere um pouco» de «voce nao pode».
#[cfg(test)]
mod testes_bulkinsert {
    use super::*;
    use crate::usuarios::Cadastro;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-bulk-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    fn com_tabela(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            cadastro: Cadastro::default(),
            ..Config::default()
        };
        let s = Servidor::novo(c).unwrap();
        let sessao = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
            ),
            &sessao,
        )
        .unwrap();
        s
    }

    /// Pelo `despachar`, que e onde mora o portao da carga.
    fn pede(s: &Arc<Servidor>, ligacao: u64, corpo: &str) -> Result<Json> {
        let mut ses = Sessao {
            ligacao,
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        r
    }

    const RESERVA: &str = r#""op":"bulkinsert","database":"b","tabela":"c","ligado":true"#;
    const SOLTA: &str = r#""op":"bulkinsert","database":"b","tabela":"c","ligado":false"#;
    const INSERE: &str = r#""op":"inserir","database":"b","tabela":"c","linha":{"id":1}"#;

    /// O caso do enunciado: reservada, o outro nao entra -- e sabe por quem.
    #[test]
    fn reservada_barra_o_outro_e_diz_quem() {
        let dir = dir_temp("barra");
        let s = com_tabela(&dir);
        pede(&s, 1, RESERVA).unwrap();

        let e = pede(&s, 2, INSERE).unwrap_err();
        assert_eq!(e.nome(), "EM_CARGA");
        assert_eq!(e.codigo(), 4002);
        let texto = e.to_string();
        assert!(
            texto.contains("ligacao 1"),
            "nao disse quem reservou: {texto}"
        );
        assert!(texto.contains("b.c"), "nao disse qual tabela: {texto}");
    }

    /// A leitura tambem para. E de proposito: deixar ler durante a carga e o
    /// que impediria adiar o indice mais tarde.
    #[test]
    fn a_leitura_do_outro_tambem_para() {
        let dir = dir_temp("leitura");
        let s = com_tabela(&dir);
        pede(&s, 1, RESERVA).unwrap();
        let e = pede(&s, 2, r#""op":"varrer","database":"b","tabela":"c""#).unwrap_err();
        assert_eq!(e.nome(), "EM_CARGA");
    }

    /// Uma tabela reservada nao barra a tabela do lado.
    #[test]
    fn a_reserva_e_de_uma_tabela_so() {
        let dir = dir_temp("outra");
        let s = com_tabela(&dir);
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"d",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
        pede(&s, 1, RESERVA).unwrap();
        assert!(pede(&s, 2, r#""op":"varrer","database":"b","tabela":"d""#).is_ok());
    }

    /// Quem reservou continua trabalhando -- senao a reserva seria so uma
    /// forma cara de derrubar o proprio servico.
    #[test]
    fn o_dono_continua_gravando() {
        let dir = dir_temp("dono");
        let s = com_tabela(&dir);
        pede(&s, 1, RESERVA).unwrap();
        for i in 1..=50 {
            pede(
                &s,
                1,
                &format!(r#""op":"inserir","database":"b","tabela":"c","linha":{{"id":{i}}}"#),
            )
            .unwrap();
        }
        let r = pede(&s, 1, SOLTA).unwrap();
        assert!(matches!(r.campo("liberada"), Some(Json::Bool(true))));
        assert!(matches!(r.campo("sincronizada"), Some(Json::Bool(true))));

        // Solta, o outro entra.
        let v = pede(&s, 2, r#""op":"varrer","database":"b","tabela":"c""#).unwrap();
        assert_eq!(v.inteiro_ou("devolvidas", -1), 50);
    }

    /// A PRIMEIRA rede: a conexao caiu, a tabela solta.
    #[test]
    fn a_queda_da_conexao_solta() {
        let dir = dir_temp("queda");
        let s = com_tabela(&dir);
        pede(&s, 1, RESERVA).unwrap();
        assert!(pede(&s, 2, INSERE).is_err());

        // E o que o `AoSair` do laco da conexao chama.
        s.soltar_cargas_da_ligacao(1);
        assert!(
            pede(&s, 2, INSERE).is_ok(),
            "a reserva sobreviveu a queda da conexao"
        );
    }

    /// A SEGUNDA rede: o prazo vence mesmo com o soquete pendurado vivo.
    #[test]
    fn o_prazo_solta() {
        let dir = dir_temp("prazo");
        let s = com_tabela(&dir);
        // Reserva com prazo ja vencido, direto no registro: e o estado em que
        // um cliente morto com o TCP vivo deixaria a tabela.
        let agora = crate::agora_ms();
        s.cargas
            .lock()
            .unwrap()
            .reservar("b", "c", "fulano", 1, "1.2.3.4", agora - 60_000, 1_000)
            .unwrap();
        assert!(
            pede(&s, 2, INSERE).is_ok(),
            "a reserva vencida continuou barrando"
        );
        assert_eq!(
            s.cargas.lock().unwrap().quantas(),
            0,
            "a reserva vencida ficou na lista"
        );
    }

    /// Reservar de novo o que ja e meu renova o prazo, em vez de recusar.
    #[test]
    fn reservar_de_novo_o_meu_renova() {
        let dir = dir_temp("renova");
        let s = com_tabela(&dir);
        pede(&s, 1, RESERVA).unwrap();
        let r = pede(&s, 1, RESERVA).unwrap();
        assert!(matches!(r.campo("reservada"), Some(Json::Bool(true))));
    }

    /// `EM_CARGA` e passageiro, e o protocolo tem de dizer isso: e o que
    /// separa «espere um pouco» de «voce nao pode».
    #[test]
    fn em_carga_pede_nova_tentativa() {
        let dir = dir_temp("repetir");
        let s = com_tabela(&dir);
        pede(&s, 1, RESERVA).unwrap();
        let e = pede(&s, 2, INSERE).unwrap_err();
        assert!(e.adianta_repetir(), "EM_CARGA deveria pedir nova tentativa");
        assert!(
            !PhxError::Autorizacao(String::new()).adianta_repetir(),
            "e ACESSO_NEGADO nao"
        );
    }

    /// Reservar tabela que nao existe recusa na hora, em vez de esconder o
    /// erro de digitacao ate o fim da carga.
    #[test]
    fn reservar_tabela_que_nao_existe_recusa() {
        let dir = dir_temp("inexistente");
        let s = com_tabela(&dir);
        assert!(pede(
            &s,
            1,
            r#""op":"bulkinsert","database":"b","tabela":"nao_existe","ligado":true"#
        )
        .is_err());
    }

    /// Pela web nao vale: HTTP nao tem conexao para a reserva morrer amarrada.
    #[test]
    fn pela_web_recusa_com_o_motivo() {
        let dir = dir_temp("web");
        let s = com_tabela(&dir);
        let e = pede(&s, 0, RESERVA).unwrap_err();
        assert!(
            e.to_string().contains("porta de dados"),
            "o recado nao explica: {e}"
        );
    }
}

/// A op `sql` -- traducao, e nao motor novo.
///
/// O que estes testes travam:
///
/// 1. `SELECT *` vira `varrer` e as linhas chegam;
/// 2. a projecao de colunas acontece, com o rotulo do `AS`;
/// 3. `COUNT(*)` sai do cabecalho, sem varrer;
/// 4. `WHERE col = ?` com indice vira `buscar`;
/// 5. o que NAO tem substrato recusa dizendo o que falta -- e nao devolve a
///    tabela inteira com o filtro esquecido no caminho;
/// 6. erro de sintaxe aponta a COLUNA do texto.
#[cfg(test)]
mod testes_sql {
    use super::*;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-sql-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    /// Uma base `b` com `clientes(id, nome, cidade)` e indice unico por id.
    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        let s = Servidor::novo(c).unwrap();
        let dono = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"cidade","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
            ),
            &dono,
        )
        .unwrap();
        for (id, nome, cidade) in [
            (1, "Adriano", "Blumenau"),
            (2, "Maria", "Joinville"),
            (3, "Joao", "Blumenau"),
        ] {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"clientes",
                         "linha":{{"id":{id},"nome":"{nome}","cidade":"{cidade}"}}}}"#
                )),
                &dono,
            )
            .unwrap();
        }
        s
    }

    /// Pelo `despachar`, que e por onde o pedido entra de verdade.
    fn sql(s: &Arc<Servidor>, texto: &str) -> Result<Json> {
        let mut ses = Sessao::default();
        let corpo = Json::objeto(vec![
            ("token", Json::texto_de("t")),
            ("op", Json::texto_de("sql")),
            ("database", Json::texto_de("b")),
            ("texto", Json::texto_de(texto)),
        ])
        .escrever();
        let (_, _, r) = s.despachar(&corpo, &mut ses, "127.0.0.1");
        r
    }

    fn linhas(j: &Json) -> Vec<Json> {
        j.campo("linhas").and_then(Json::lista).unwrap().to_vec()
    }

    #[test]
    fn select_estrela_vira_varrer_e_traz_as_linhas() {
        let s = servidor(&dir_temp("estrela"));
        let r = sql(&s, "SELECT * FROM clientes").unwrap();
        assert_eq!(r.texto_ou("op", ""), "varrer");
        assert_eq!(linhas(&r).len(), 3);
        assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Adriano");
        // A nota nao e enfeite: sem ela, quem esperava outra ordem culpa o
        // motor em vez de escrever o ORDER BY.
        assert!(r
            .campo("notas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .any(|n| n.texto().unwrap_or("").contains("DIGITACAO")));
    }

    /// A projecao e do servidor porque o protocolo sempre devolve a linha
    /// inteira -- o `.reg` e de slot fixo, e ler meia linha custa a mesma
    /// leitura. Quem escreveu SQL espera as colunas que pediu.
    #[test]
    fn a_projecao_fica_so_com_as_colunas_pedidas_e_usa_o_apelido() {
        let s = servidor(&dir_temp("projecao"));
        let r = sql(&s, "SELECT nome AS quem, cidade FROM clientes").unwrap();
        assert_eq!(
            r.campo("colunas")
                .and_then(Json::lista)
                .unwrap()
                .iter()
                .map(|c| c.texto().unwrap_or("").to_string())
                .collect::<Vec<_>>(),
            vec!["quem", "cidade"]
        );
        let primeira = &linhas(&r)[0];
        assert_eq!(primeira.chaves(), vec!["quem", "cidade"]);
        assert_eq!(primeira.texto_ou("quem", ""), "Adriano");
        // E o que NAO foi pedido nao vem junto -- nem a coluna de sistema.
        assert!(primeira.campo("softdeleted").is_none());
    }

    /// A contagem sai do cabecalho, em O(1). Varrer a tabela para contar e o
    /// erro que a bancada ja cometeu uma vez.
    #[test]
    fn count_estrela_sai_do_cabecalho_sem_varrer() {
        let s = servidor(&dir_temp("count"));
        let r = sql(&s, "SELECT COUNT(*) FROM clientes").unwrap();
        assert_eq!(r.inteiro_ou("contagem", -1), 3);
        assert_eq!(r.inteiro_ou("registros", -1), 3);
    }

    /// **`SELECT COUNT(*)` nao pode devolver uma LINHA de dado.**
    ///
    /// A traducao pede `max: 1` para ler o cabecalho, e a linha que vem junto e
    /// efeito colateral do caminho -- nao a resposta. Devolve-la fazia o
    /// console desenhar uma tabela de uma linha embaixo da contagem, e quem
    /// olha nao tem como saber se aquela linha quer dizer alguma coisa.
    ///
    /// Achado exercitando o console, e nao lendo o codigo: no JSON o campo
    /// extra passa despercebido; na tela ele vira uma tabela inteira.
    #[test]
    fn a_contagem_nao_arrasta_a_linha_que_a_traducao_leu() {
        let s = servidor(&dir_temp("count-limpo"));
        let r = sql(&s, "SELECT COUNT(*) FROM clientes").unwrap();
        assert!(
            r.campo("linhas").is_none(),
            "a contagem veio com linha de dado: {}",
            r.escrever()
        );
        // E nem os campos que descrevem uma pagina que ninguem pediu.
        for campo in ["devolvidas", "cursor_inicio", "ha_mais", "ordem"] {
            assert!(
                r.campo(campo).is_none(),
                "{campo} nao descreve nada numa contagem: {}",
                r.escrever()
            );
        }
        // Ja o COUNT(*) com WHERE conta o que a busca achou.
        let r = sql(&s, "SELECT COUNT(*) FROM clientes WHERE id = 2").unwrap();
        assert_eq!(r.inteiro_ou("contagem", -1), 1);
        assert!(r.campo("linhas").is_none());
    }

    #[test]
    fn where_com_indice_vira_buscar() {
        let s = servidor(&dir_temp("where"));
        let r = sql(&s, "SELECT nome FROM clientes WHERE id = 2").unwrap();
        assert_eq!(r.texto_ou("op", ""), "buscar");
        assert_eq!(linhas(&r).len(), 1);
        assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Maria");
    }

    /// **O que nao tem substrato recusa dizendo o que falta.** `cidade` nao
    /// tem indice, e o `varrer` NAO filtra: aceitar calado devolveria a tabela
    /// inteira com o filtro esquecido no caminho -- ler demais e responder
    /// errado sem avisar.
    #[test]
    fn where_sem_indice_recusa_em_vez_de_trazer_tudo() {
        let s = servidor(&dir_temp("sem-indice"));
        let e = sql(&s, "SELECT * FROM clientes WHERE cidade = 'Blumenau'").unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("cidade"), "{msg}");
        assert!(msg.contains("indice"), "{msg}");
        // E diz qual coluna TEM indice, que e o que permite consertar.
        assert!(msg.contains("id"), "{msg}");
    }

    /// Erro de sintaxe aponta a coluna do texto. Sem a posicao, quem escreveu
    /// um comando de duzentos caracteres procura o erro no lugar errado.
    #[test]
    fn erro_de_sintaxe_diz_a_coluna() {
        let s = servidor(&dir_temp("sintaxe"));
        let msg = sql(&s, "SELECT * FRON clientes").unwrap_err().to_string();
        assert!(msg.contains("coluna"), "{msg}");
        assert!(msg.contains("FROM"), "{msg}");

        let msg = sql(&s, "DELETE FROM clientes").unwrap_err().to_string();
        assert!(msg.contains("SELECT"), "{msg}");
    }

    /// O `LIMIT`/`OFFSET` chega ao `varrer` como `max` e `pular` -- e nao e
    /// aplicado no cliente depois de trazer tudo.
    #[test]
    fn limit_e_offset_viram_max_e_pular() {
        let s = servidor(&dir_temp("limite"));
        let r = sql(&s, "SELECT * FROM clientes LIMIT 1 OFFSET 1").unwrap();
        assert_eq!(linhas(&r).len(), 1);
        assert_eq!(linhas(&r)[0].texto_ou("nome", ""), "Maria");
    }

    /// Pedido sem texto nenhum recusa dizendo o nome do campo. E `sql` e aceito
    /// como sinonimo de `texto`, porque e o nome que um driver escreveria.
    #[test]
    fn sem_texto_recusa_com_o_nome_do_campo() {
        let s = servidor(&dir_temp("vazio"));
        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(
            r#"{"token":"t","op":"sql","database":"b"}"#,
            &mut ses,
            "127.0.0.1",
        );
        assert!(r.unwrap_err().to_string().contains("texto"));

        let (_, _, r) = s.despachar(
            r#"{"token":"t","op":"sql","database":"b","sql":"SELECT COUNT(*) FROM clientes"}"#,
            &mut ses,
            "127.0.0.1",
        );
        assert_eq!(r.unwrap().inteiro_ou("contagem", -1), 3);
    }

    /// A politica vale para a operacao TRADUZIDA, e nao so para a `sql`. Um
    /// servidor que proibe `varrer` nao pode ser varrido escrevendo SELECT.
    #[test]
    fn a_politica_vale_para_a_operacao_traduzida() {
        let dir = dir_temp("politica");
        let mut c = Config {
            base: dir.clone(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        c.politica.comandos_proibidos = vec!["varrer".into()];
        let s = Servidor::novo(c).unwrap();
        let dono = Sessao::default();
        s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
            .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
            ),
            &dono,
        )
        .unwrap();

        let e = sql(&s, "SELECT * FROM clientes").unwrap_err();
        assert!(
            e.to_string().contains("varrer") && e.to_string().contains("proibida"),
            "{e}"
        );
    }
}

/// **Chave estrangeira pelo protocolo** -- o #127 dizia «pronto» e era meia
/// verdade: o formato as suporta e o `esquema` as reporta desde sempre, mas
/// NENHUMA operacao as criava. So dava para declarar uma pela API Rust.
#[cfg(test)]
mod testes_chave_estrangeira {
    use super::*;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-fk-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        let s = Servidor::novo(c).unwrap();
        s.executar(
            "criar_database",
            &pedido(r#"{"database":"b"}"#),
            &Sessao::default(),
        )
        .unwrap();
        s
    }

    /// Pelo `despachar`: e por onde o pedido entra de verdade.
    fn pede(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        );
        r
    }

    /// Cria `pedidos` apontando para `clientes`, e le a chave de volta.
    fn com_fk(s: &Arc<Servidor>, extra: &str) -> Result<Json> {
        pede(
            s,
            &format!(
                r#""op":"criar_tabela","database":"b","tabela":"pedidos",
                   "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                              {{"nome":"cliente_id","tipo":"Int4"}}],
                   "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}}],
                   "chaves_estrangeiras":[{{"nome":"fk_cliente",
                                            "colunas":["cliente_id"],
                                            "tabela_ref":"clientes",
                                            "colunas_ref":["id"]{extra}}}]"#
            ),
        )
    }

    #[test]
    fn criar_tabela_declara_a_chave_e_o_esquema_a_devolve() {
        let s = servidor(&dir_temp("declara"));
        com_fk(&s, r#","ao_excluir":"restringir","ao_alterar":"cascata""#).unwrap();

        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
        let fks = e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap();
        assert_eq!(fks.len(), 1, "a chave nao voltou: {}", e.escrever());
        let fk = &fks[0];
        assert_eq!(fk.texto_ou("nome", ""), "fk_cliente");
        assert_eq!(fk.texto_ou("tabela_ref", ""), "clientes");
        assert_eq!(fk.textos("colunas"), vec!["cliente_id"]);
        assert_eq!(fk.textos("colunas_ref"), vec!["id"]);
        assert_eq!(fk.texto_ou("ao_excluir", ""), "Restringir");
        assert_eq!(fk.texto_ou("ao_alterar", ""), "Cascata");

        // E o papel da coluna, que e DERIVADO das chaves, acompanha: sem isto
        // a tela desenharia a coluna como uma qualquer.
        let coluna = e
            .campo("colunas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .find(|c| c.texto_ou("nome", "") == "cliente_id")
            .cloned()
            .unwrap();
        assert_eq!(coluna.campo("estrangeira").unwrap().booleano(), Some(true));
        assert_eq!(coluna.textos("nas_chaves_estrangeiras"), vec!["fk_cliente"]);
    }

    /// **O teste do comportamento VELHO, e e o que mais importa.** Um pedido
    /// sem o campo tem de criar a tabela exatamente como sempre criou -- todo
    /// cliente escrito antes desta versao manda pedido assim.
    #[test]
    fn sem_o_campo_a_tabela_nasce_igual_ao_que_sempre_foi() {
        let s = servidor(&dir_temp("velho"));
        pede(
            &s,
            r#""op":"criar_tabela","database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
        )
        .unwrap();
        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"clientes""#).unwrap();
        assert!(e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap()
            .is_empty());
        // E a linha entra como sempre entrou.
        pede(
            &s,
            r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":1}"#,
        )
        .unwrap();
    }

    /// Sem `colunas_ref`, referencia colunas de MESMO NOME. Escrever a lista
    /// duas vezes e onde alguem troca a ordem sem perceber.
    #[test]
    fn sem_colunas_ref_vale_o_mesmo_nome() {
        let s = servidor(&dir_temp("mesmo-nome"));
        pede(
            &s,
            r#""op":"criar_tabela","database":"b","tabela":"itens",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true}],
               "chaves_estrangeiras":[{"nome":"fk_id","colunas":["id"],
                                       "tabela_ref":"outra"}]"#,
        )
        .unwrap();
        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"itens""#).unwrap();
        let fk = &e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap()[0];
        assert_eq!(fk.textos("colunas_ref"), vec!["id"]);
        // E o padrao das duas acoes e o unico seguro: quem nao disse o que
        // fazer com a filha nao pediu para apaga-la.
        assert_eq!(fk.texto_ou("ao_excluir", ""), "Restringir");
        assert_eq!(fk.texto_ou("ao_alterar", ""), "Restringir");
    }

    /// A acao aceita o portugues, o SQL e a forma que o `esquema` DEVOLVE --
    /// pela mesma razao do tipo da coluna: o que sai tem de poder voltar.
    #[test]
    fn a_acao_aceita_as_tres_escritas() {
        for (escrito, esperado) in [
            ("cascata", "Cascata"),
            ("CASCADE", "Cascata"),
            ("Cascata", "Cascata"),
            ("set null", "AnularCampos"),
            ("anular", "AnularCampos"),
            ("nada", "NaoFazerNada"),
        ] {
            let s = servidor(&dir_temp(&format!("acao-{}", escrito.replace(' ', "-"))));
            com_fk(&s, &format!(r#","ao_excluir":"{escrito}""#)).unwrap();
            let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
            let fk = &e
                .campo("chaves_estrangeiras")
                .and_then(Json::lista)
                .unwrap()[0];
            assert_eq!(fk.texto_ou("ao_excluir", ""), esperado, "{escrito}");
        }
    }

    /// Chave mal escrita recusa dizendo O QUE esta errado, e a tabela NAO
    /// nasce: meia tabela criada seria pior que nenhuma.
    #[test]
    fn chave_mal_escrita_recusa_e_a_tabela_nao_nasce() {
        let s = servidor(&dir_temp("ruim"));
        // Cada caso com um nome de tabela proprio: com o mesmo nome, o segundo
        // erro seria "ja existe" e o teste passaria pelo motivo errado.
        for (n, chave, esperado) in [
            (
                1,
                r#"{"nome":"fk","colunas":["nao_existe"],"tabela_ref":"c"}"#,
                "nao existe nesta tabela",
            ),
            (
                2,
                r#"{"nome":"fk","colunas":["id"],"tabela_ref":""}"#,
                "tabela_ref",
            ),
            (
                3,
                r#"{"nome":"fk","colunas":["id"],"tabela_ref":"c","ao_excluir":"talvez"}"#,
                "acao de integridade desconhecida",
            ),
            (4, r#"{"colunas":["id"],"tabela_ref":"c"}"#, "nome"),
            (5, r#"{"nome":"fk","tabela_ref":"c"}"#, "colunas"),
        ] {
            let e = pede(
                &s,
                &format!(
                    r#""op":"criar_tabela","database":"b","tabela":"t{n}",
                       "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
                       "chaves_estrangeiras":[{chave}]"#
                ),
            )
            .unwrap_err();
            assert!(
                e.to_string().contains(esperado),
                "caso {n}: {e} (esperava {esperado:?})"
            );

            // E a tabela NAO nasceu: meia tabela criada seria pior que nenhuma,
            // porque o proximo pedido diria "ja existe" e ninguem entenderia.
            let t = pede(&s, r#""op":"tabelas","database":"b""#).unwrap();
            assert!(
                !t.escrever().contains(&format!("t{n}")),
                "caso {n}: a tabela nasceu mesmo com a chave recusada"
            );
        }
    }

    /// **Declarar nao e aplicar, e este teste existe para o documento nao
    /// mentir.**
    ///
    /// A chave fica gravada no esquema, o `esquema` a devolve e o diagrama a
    /// desenha -- mas NENHUMA gravacao a consulta hoje. Uma linha filha
    /// apontando para um pai que nao existe entra sem reclamacao.
    ///
    /// O teste trava o comportamento REAL, e nao o desejado: no dia em que a
    /// imposicao entrar, ele falha e obriga quem a escreveu a atualizar o
    /// MANUAL junto. Sem ele, alguem le "chave estrangeira" no `criar_tabela`
    /// e supoe uma garantia que nao existe -- que e como a lista de pendencias
    /// chegou a dizer "pronto" para isto.
    #[test]
    fn a_chave_e_declarada_mas_ainda_nao_e_imposta_na_gravacao() {
        let s = servidor(&dir_temp("nao-impoe"));
        com_fk(&s, "").unwrap();
        // `clientes` nem existe, e o pai 999 muito menos.
        pede(
            &s,
            r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":999}"#,
        )
        .expect(
            "a insercao passou a ser recusada: a integridade referencial entrou.              Atualize o MANUAL e o docs/PENDENCIAS, que dizem que ela NAO e imposta",
        );
    }

    /// **`duplicar_tabela` preserva a chave.** Ele copia os arquivos byte a
    /// byte, e o esquema mora no `.reg` -- mas isso e uma consequencia de como
    /// ele foi feito, e nao uma promessa escrita. Este teste vira a promessa:
    /// se um dia alguem trocar a copia por uma reinsercao linha a linha, a
    /// chave sumiria em silencio.
    #[test]
    fn duplicar_tabela_preserva_a_chave_estrangeira() {
        let s = servidor(&dir_temp("duplicar"));
        com_fk(&s, r#","ao_alterar":"cascata""#).unwrap();
        pede(
            &s,
            r#""op":"duplicar_tabela","database":"b","tabela":"pedidos",
               "destino":"pedidos_copia""#,
        )
        .unwrap();

        let e = pede(
            &s,
            r#""op":"esquema","database":"b","tabela":"pedidos_copia""#,
        )
        .unwrap();
        let fks = e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap();
        assert_eq!(fks.len(), 1, "a copia perdeu a chave: {}", e.escrever());
        assert_eq!(fks[0].texto_ou("nome", ""), "fk_cliente");
        assert_eq!(fks[0].texto_ou("ao_alterar", ""), "Cascata");
    }

    /// **A chave entra numa tabela que JA existe** -- e o que o editor do
    /// diagrama chama quando alguem liga duas colunas com o mouse. O bloco de
    /// esquema cresce e mora antes do slot 1, entao o nome comprido forca o
    /// caminho caro (reescrever o `.reg`): a linha gravada ANTES tem de
    /// continuar legivel DEPOIS.
    #[test]
    fn declarar_fk_entra_numa_tabela_existente_sem_perder_linha() {
        let s = servidor(&dir_temp("declara-depois"));
        pede(
            &s,
            r#""op":"criar_tabela","database":"b","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"cliente_id","tipo":"Int4"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
        )
        .unwrap();
        pede(
            &s,
            r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":7,"cliente_id":3}"#,
        )
        .unwrap();

        let r = pede(
            &s,
            r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento",
               "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"],
               "ao_excluir":"cascata""#,
        )
        .unwrap();
        // A resposta diz a verdade que a tela precisa repetir.
        assert_eq!(r.campo("imposta").unwrap().booleano(), Some(false));

        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
        let fks = e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap();
        assert_eq!(fks.len(), 1, "a chave nao entrou: {}", e.escrever());
        assert_eq!(fks[0].texto_ou("tabela_ref", ""), "clientes");
        assert_eq!(fks[0].texto_ou("ao_excluir", ""), "Cascata");

        // A linha de antes continua inteira -- declarar e catalogo, nao dado.
        let l = pede(
            &s,
            r#""op":"ler","database":"b","tabela":"pedidos","rowid":1"#,
        )
        .unwrap();
        assert!(
            l.escrever().contains('7'),
            "a linha sumiu: {}",
            l.escrever()
        );

        // Duplicar o nome e recusado -- e a primeira declaracao fica.
        let e2 = pede(
            &s,
            r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento",
               "colunas":["cliente_id"],"tabela_ref":"clientes""#,
        )
        .unwrap_err();
        assert!(e2.to_string().contains("ja esta declarada"), "{e2}");
    }

    /// O leitor da chave aceita `tabela` como apelido de `tabela_ref` -- mas
    /// NESTE pedido `tabela` e a tabela que recebe a declaracao. Sem a recusa,
    /// omitir `tabela_ref` viraria uma chave apontando para si mesma, em
    /// silencio.
    #[test]
    fn declarar_fk_sem_tabela_ref_recusa_em_vez_de_apontar_para_si() {
        let s = servidor(&dir_temp("sem-ref"));
        pede(
            &s,
            r#""op":"criar_tabela","database":"b","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]"#,
        )
        .unwrap();
        let e = pede(
            &s,
            r#""op":"declarar_fk","database":"b","tabela":"pedidos",
               "nome":"fk","colunas":["id"]"#,
        )
        .unwrap_err();
        assert!(e.to_string().contains("tabela_ref"), "{e}");
        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
        assert!(
            e.campo("chaves_estrangeiras")
                .and_then(Json::lista)
                .unwrap()
                .is_empty(),
            "a chave nasceu mesmo recusada"
        );
    }

    /// `excluir_fk` tira a declaracao e NADA mais: a linha fica, e um nome
    /// que nao existe responde com a lista do que existe.
    #[test]
    fn excluir_fk_tira_a_declaracao_e_nada_mais() {
        let s = servidor(&dir_temp("tira"));
        com_fk(&s, "").unwrap();
        pede(
            &s,
            r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":1,"cliente_id":2}"#,
        )
        .unwrap();

        let e = pede(
            &s,
            r#""op":"excluir_fk","database":"b","tabela":"pedidos","nome":"fk_errada""#,
        )
        .unwrap_err();
        assert!(e.to_string().contains("fk_cliente"), "{e}");

        pede(
            &s,
            r#""op":"excluir_fk","database":"b","tabela":"pedidos","nome":"fk_cliente""#,
        )
        .unwrap();
        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();
        assert!(e
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap()
            .is_empty());
        let l = pede(
            &s,
            r#""op":"ler","database":"b","tabela":"pedidos","rowid":1"#,
        )
        .unwrap();
        assert!(l.escrever().contains("cliente_id"), "{}", l.escrever());
    }

    /// O que o `esquema` devolve tem de poder voltar como `criar_tabela`. E o
    /// caminho de recriar uma tabela noutro servidor, e uma chave que so sai e
    /// nao entra quebraria justamente ele.
    #[test]
    fn o_que_o_esquema_devolve_volta_como_criar_tabela() {
        let s = servidor(&dir_temp("ida-e-volta"));
        com_fk(&s, r#","ao_excluir":"cascata""#).unwrap();
        let e = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos""#).unwrap();

        let mut recriar = vec![
            ("op".to_string(), Json::texto_de("criar_tabela")),
            ("database".to_string(), Json::texto_de("b")),
            ("tabela".to_string(), Json::texto_de("pedidos2")),
            ("token".to_string(), Json::texto_de("t")),
        ];
        // As colunas de sistema saem de fora: elas entram sozinhas.
        let colunas: Vec<Json> = e
            .campo("colunas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .filter(|c| c.campo("sistema").and_then(Json::booleano) != Some(true))
            .cloned()
            .collect();
        recriar.push(("colunas".to_string(), Json::Lista(colunas)));
        recriar.push((
            "chaves_estrangeiras".to_string(),
            e.campo("chaves_estrangeiras").cloned().unwrap(),
        ));

        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(&Json::Objeto(recriar).escrever(), &mut ses, "127.0.0.1");
        r.expect("o esquema devolvido nao voltou como pedido");

        let e2 = pede(&s, r#""op":"esquema","database":"b","tabela":"pedidos2""#).unwrap();
        let fk = &e2
            .campo("chaves_estrangeiras")
            .and_then(Json::lista)
            .unwrap()[0];
        assert_eq!(fk.texto_ou("nome", ""), "fk_cliente");
        assert_eq!(fk.texto_ou("ao_excluir", ""), "Cascata");
    }
}
