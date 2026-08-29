//! O que o SERVIDOR esta fazendo agora: atividades, threads e as series.
//!
//! # O que ele e
//!
//! O equivalente do SQL Check da Idera(R): as faixas de series no topo --
//! esperas, leitura e escrita fisicas, CPU, vazao, cache do `.ndx` -- e,
//! embaixo, as ATIVIDADES vivas, uma bolha cada, do tamanho do peso.
//!
//! Ele nao repete o que ja existe. Quem esta conectado sai de
//! [`crate::ligacoes`]; o que chega pela porta sai do [`crate::profiler`]; a
//! maquina sai de [`crate::sistema`]. O que este modulo acrescenta e o que
//! nenhum dos tres tinha: **o tempo**, guardado numa serie, e **o peso**, que
//! e o que ordena as bolhas.
//!
//! # O portao vem ANTES do trabalho
//!
//! A licao do Profiler esta escrita neste arquivo em cinco lugares: todo
//! ponto de captura comeca por `if !self.ligada() { return; }`, e `ligada` e
//! um `AtomicBool` lido com `Relaxed` -- uma instrucao, sem trava, sem
//! alocacao, sem analisar nada. Desligada, a telemetria custa exatamente
//! isso, e nao mais.
//!
//! Ligada, ela tambem nao pode cobrar caro. Por isso:
//!
//! * as series sao amostradas por UMA thread, de segundo em segundo, e nao
//!   calculadas no caminho de quem pede;
//! * a espera pela trava de dados custa dois `Instant::now()` por OPERACAO --
//!   nao por linha --, num caminho em que a operacao mais barata ja leva
//!   dezenas de microssegundos;
//! * o ponto de cancelamento, que roda por linha, e um `load(Relaxed)` mais
//!   um `fetch_add(Relaxed)` num `Arc` que o laco ja tem na mao;
//! * os acertos do cache do `.ndx` sobem por ARQUIVO FECHADO, e nao por
//!   toque de pagina (ver `phxsql_store::ndx::contadores_de_cache`).
//!
//! # Encerrar uma atividade: cooperativo, e dito na cara
//!
//! Rust nao mata thread no meio, e ainda bem: uma escrita interrompida entre
//! o slot e o indice deixaria a tabela mentindo. Entao encerrar aqui e
//! **marcar**, e a marca so e olhada em PONTO SEGURO -- entre uma linha e a
//! proxima de uma varredura, entre uma linha e a proxima da conversao de uma
//! carga. Quem esta dentro do ponto critico termina o que comecou.
//!
//! Isso quer dizer que ha fase NAO CANCELAVEL, e a resposta diz qual e em vez
//! de prometer um `KILL` instantaneo que nao existe. A lista completa esta em
//! `docs/TELEMETRIA.md`.
//!
//! # A marca morre com a operacao que ela mirou
//!
//! Cada operacao de uma atividade tem um serial, e a marca de encerramento
//! guarda o serial que ela mirou. Sem isso, mandar encerrar uma conexao
//! parada mataria o PROXIMO pedido dela -- um que ninguem pediu para matar,
//! chegando talvez minutos depois. Marca que sobrevive ao alvo e armadilha.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// Quantas amostras ficam em memoria.
///
/// Duzentas a um segundo sao pouco mais de tres minutos de historia, que e a
/// janela que um painel ao vivo mostra. Guardar horas seria guardar o que
/// ninguem olha e comer memoria de um servidor que ja esta apertado -- e a
/// historia longa e trabalho do log de acessos, que vai a disco.
const AMOSTRAS_GUARDADAS: usize = 200;

/// De quanto em quanto tempo o amostrador acorda.
pub const PERIODO_DA_AMOSTRA_MS: u64 = 1_000;

/// A partir de quantos milissegundos a operacao corrente pinta de amarelo.
///
/// O limiar sai do SERVIDOR e vai na resposta, e nao fica escrito na tela:
/// com a regra nos dois lugares, o dia em que um deles mudar a tela pinta uma
/// cor que o servidor nao concorda. E a mesma razao pela qual `sistema` manda
/// o `livre_minimo_percentual` junto com o espaco livre.
///
/// E o valor DE FABRICA: o `config.json` pode troca-lo em
/// `telemetria.alto_uso_ms`, e o que vale e o que esta na [`Painel`] deste
/// registro. Quem nao configurar nada continua com estes 2 s.
pub const ALTO_USO_MS: u64 = 2_000;

/// A partir de quantos milissegundos segurando a trava a atividade e stress.
///
/// Segurar a trava de dados nao e ocupar uma parte do servidor: e ocupar o
/// servidor INTEIRO, porque toda escrita e toda leitura passam por ela. Por
/// isso o limiar aqui e mais curto que o de uso alto.
///
/// Tambem e o valor de fabrica -- ver [`ALTO_USO_MS`].
pub const STRESS_MS: u64 = 5_000;

/// As operacoes que TEM ponto de cancelamento em algum trecho.
///
/// # Por que uma lista, se ja ha a fase
///
/// A fase so abre quando o laco comeca -- e entre o pedido chegar e o laco
/// comecar ha um trecho que pode durar segundos: a espera pela trava de
/// dados. Exercitando a tela apareceu exatamente isso: uma soma de
/// verificacao parada na fila foi marcada, a resposta disse «vai terminar» --
/// e ela ABORTOU, porque a marca estava posta quando o laco comecou.
///
/// A resposta estava pessimista, que e o lado seguro de errar, mas ainda
/// assim errada. Com esta lista dá para separar as duas perguntas:
///
/// * *esta operacao tem onde parar?* -- decide se o botao aparece;
/// * *ela esta nesse ponto AGORA?* -- decide o que a resposta promete.
///
/// A lista anda junto com os `siga()` do `servidor.rs`. Ha teste que a
/// confere contra o codigo.
pub const OPS_CANCELAVEIS: &[&str] = &[
    "checksum",
    "soma_de_verificacao",
    "exportar",
    "export",
    "varrer",
    "inserir_lote",
    "importar",
    "carga",
];

/// Estado de uma atividade, do mais parado ao mais barulhento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Conectada, sem pedido nenhum em curso.
    Ociosa,
    /// Tem pedido, e esta parada na fila da trava de dados.
    Esperando,
    /// Tem pedido, e esta com a trava de dados na mao.
    Executando,
    /// Alguem mandou encerrar e ela ainda nao chegou no ponto seguro.
    Encerrando,
}

impl Estado {
    pub fn nome(self) -> &'static str {
        match self {
            Estado::Ociosa => "ociosa",
            Estado::Esperando => "esperando",
            Estado::Executando => "executando",
            Estado::Encerrando => "encerrando",
        }
    }

    fn de_u8(v: u8) -> Estado {
        match v {
            1 => Estado::Esperando,
            2 => Estado::Executando,
            3 => Estado::Encerrando,
            _ => Estado::Ociosa,
        }
    }

    fn para_u8(self) -> u8 {
        match self {
            Estado::Ociosa => 0,
            Estado::Esperando => 1,
            Estado::Executando => 2,
            Estado::Encerrando => 3,
        }
    }
}

/// O que a atividade esta fazendo agora. Trocado por pedido, e nao por linha.
#[derive(Debug, Clone, Default)]
struct Corrente {
    op: String,
    usuario: String,
    database: String,
    tabela: String,
    inicio_ms: i64,
    /// Em que ponto do trabalho ela esta -- so preenchido em fase cancelavel.
    fase: String,
}

/// Uma atividade viva: uma conexao, um pedido da tela, um laco de fundo.
///
/// # Por que a identidade e do DONO, e nao do pedido
///
/// A bolha precisa de identificador estavel: se ele mudasse a cada pedido, a
/// tela redesenharia bolhas novas duas vezes por segundo e ninguem
/// conseguiria clicar em nenhuma. Entao a chave e do dono -- a conexao, a
/// sessao do navegador, a thread -- e a operacao dentro dela e que troca.
pub struct Atividade {
    /// Identificador estavel: `dados:17`, `web:a1b2c3d4`, `fio:replica-loja`.
    pub chave: String,
    /// De onde ela vem: `dados`, `web` ou `fio`.
    pub origem: &'static str,
    pub ip: String,
    pub desde_ms: i64,
    /// A ligacao da porta de dados, quando ha uma. Zero nos outros casos.
    pub ligacao: u64,
    dentro: Mutex<Corrente>,
    estado: AtomicU64,
    /// Serial da operacao corrente. Sobe a cada pedido.
    serial: AtomicU64,
    /// Serial que alguem mandou encerrar. Zero = ninguem mandou.
    encerrar_serial: AtomicU64,
    /// Quem mandou encerrar, para o log e para a tela.
    quem_encerrou: Mutex<String>,
    /// A fase corrente aceita cancelamento AGORA -- o laco esta rodando.
    cancelavel: AtomicBool,
    /// A operacao corrente tem ponto de cancelamento em algum trecho.
    tem_ponto: AtomicBool,
    /// Esta atividade esta com a TRAVA DE DADOS na mao neste instante.
    ///
    /// Diferente de «executando»: uma operacao que nao toca em dado -- a
    /// propria telemetria, o `ping`, o `catalogo` -- executa sem nunca pedir a
    /// trava, e portanto nunca segura ninguem. Sem esta distincao, a bolha da
    /// tela que esta OLHANDO o painel aparecia em vermelho toda vez que havia
    /// fila, acusada de segurar uma trava que ela nem pediu.
    com_trava: AtomicBool,
    /// Unidades de trabalho ja percorridas na operacao corrente.
    passos: AtomicU64,
    /// Milissegundos de servidor ja gastos pelas operacoes CONCLUIDAS.
    consumido_ms: AtomicU64,
    /// Quantos pedidos ja passaram por aqui.
    pedidos: AtomicU64,
    /// Quando a atividade ENTROU no estado de execucao.
    ///
    /// Nao e o comeco do pedido: e o comeco do trecho em que ela esta de
    /// fato usando o servidor. Quem esta na fila da trava nao gasta nada.
    comecou: Mutex<Option<Instant>>,
    /// Quando a operacao corrente comecou, para o «ha quanto tempo» da tela.
    ///
    /// Este SIM e o relogio de parede do pedido inteiro, espera incluida:
    /// uma consulta parada ha trinta segundos na fila leva trinta segundos
    /// para quem a pediu, e a tela nao pode dizer que leva zero.
    pedido_desde: Mutex<Option<Instant>>,
    /// Quantas vezes uma operacao desta atividade foi encerrada de fato.
    encerradas: AtomicU64,
    /// Quando esta atividade foi vista pela ultima vez.
    ultimo_ms: AtomicI64,
}

impl Atividade {
    fn nova(chave: String, origem: &'static str, ip: String, ligacao: u64, agora_ms: i64) -> Self {
        Atividade {
            chave,
            origem,
            ip,
            desde_ms: agora_ms,
            ligacao,
            dentro: Mutex::new(Corrente::default()),
            estado: AtomicU64::new(Estado::Ociosa.para_u8() as u64),
            serial: AtomicU64::new(0),
            encerrar_serial: AtomicU64::new(0),
            quem_encerrou: Mutex::new(String::new()),
            cancelavel: AtomicBool::new(false),
            tem_ponto: AtomicBool::new(false),
            com_trava: AtomicBool::new(false),
            passos: AtomicU64::new(0),
            consumido_ms: AtomicU64::new(0),
            pedidos: AtomicU64::new(0),
            comecou: Mutex::new(None),
            pedido_desde: Mutex::new(None),
            encerradas: AtomicU64::new(0),
            ultimo_ms: AtomicI64::new(agora_ms),
        }
    }

    pub fn estado(&self) -> Estado {
        Estado::de_u8(self.estado.load(Ordering::Relaxed) as u8)
    }

    /// Troca o estado E fecha a conta do tempo de servidor.
    ///
    /// # Por que a conta mora aqui
    ///
    /// O peso da bolha e tempo de SERVIDOR gasto, e quem esta parado na fila
    /// da trava nao gasta servidor nenhum -- gasta paciencia de quem pediu.
    ///
    /// A primeira versao contava o relogio de parede do pedido, e o painel
    /// exibiu o resultado disso com todas as clareza: com uma soma de
    /// verificacao segurando a trava, as oito conexoes bloqueadas atras dela
    /// apareceram com EXATAMENTE o mesmo peso da soma -- oito bolhas do mesmo
    /// tamanho, e nenhuma pista de qual delas era o problema. O tamanho, que
    /// existe para ordenar, tinha parado de ordenar.
    ///
    /// Contando so o tempo em EXECUCAO, a soma cresce e as bloqueadas nao --
    /// que e a leitura certa: uma delas esta comendo o servidor, sete estao
    /// esperando.
    fn definir_estado(&self, novo: Estado) {
        let antes = self.estado();
        if antes == novo {
            return;
        }
        let executava = matches!(antes, Estado::Executando | Estado::Encerrando);
        let executa = matches!(novo, Estado::Executando | Estado::Encerrando);
        if executava && !executa {
            // Saiu da execucao: fecha o trecho e soma.
            if let Ok(mut i) = self.comecou.lock() {
                if let Some(t) = i.take() {
                    self.consumido_ms
                        .fetch_add(t.elapsed().as_millis() as u64, Ordering::Relaxed);
                }
            }
        } else if !executava && executa {
            if let Ok(mut i) = self.comecou.lock() {
                *i = Some(Instant::now());
            }
        }
        self.estado.store(novo.para_u8() as u64, Ordering::Relaxed);
    }

    /// Comeca um pedido. Devolve o serial dele.
    pub fn comecou_pedido(
        &self,
        op: &str,
        usuario: &str,
        database: &str,
        tabela: &str,
        agora_ms: i64,
    ) -> u64 {
        let serial = self.serial.fetch_add(1, Ordering::SeqCst) + 1;
        self.ultimo_ms.store(agora_ms, Ordering::Relaxed);
        self.passos.store(0, Ordering::Relaxed);
        self.cancelavel.store(false, Ordering::Relaxed);
        self.com_trava.store(false, Ordering::Relaxed);
        self.tem_ponto
            .store(OPS_CANCELAVEIS.contains(&op), Ordering::Relaxed);
        if let Ok(mut c) = self.dentro.lock() {
            c.op = op.to_string();
            if !usuario.is_empty() {
                c.usuario = usuario.to_string();
            }
            c.database = database.to_string();
            c.tabela = tabela.to_string();
            c.inicio_ms = agora_ms;
            c.fase.clear();
        }
        if let Ok(mut i) = self.pedido_desde.lock() {
            *i = Some(Instant::now());
        }
        self.pedidos.fetch_add(1, Ordering::Relaxed);
        // `definir_estado` e que abre o cronometro do tempo de servidor.
        self.definir_estado(Estado::Executando);
        serial
    }

    /// Termina o pedido: a atividade volta a ficar ociosa.
    pub fn terminou_pedido(&self, usuario: &str) {
        // A conta do tempo de servidor fecha na troca para `Ociosa`, la
        // embaixo -- um lugar so, para nao haver duas somas do mesmo trecho.
        if let Ok(mut i) = self.pedido_desde.lock() {
            *i = None;
        }
        self.cancelavel.store(false, Ordering::Relaxed);
        self.tem_ponto.store(false, Ordering::Relaxed);
        self.com_trava.store(false, Ordering::Relaxed);
        if let Ok(mut c) = self.dentro.lock() {
            c.op.clear();
            c.database.clear();
            c.tabela.clear();
            c.fase.clear();
            c.inicio_ms = 0;
            if !usuario.is_empty() {
                c.usuario = usuario.to_string();
            }
        }
        self.definir_estado(Estado::Ociosa);
    }

    /// Marca que esta parada na fila da trava de dados.
    pub fn esperando_trava(&self) {
        if self.estado() == Estado::Executando {
            self.definir_estado(Estado::Esperando);
        }
    }

    /// Marca que conseguiu a trava.
    pub fn com_a_trava(&self) {
        self.com_trava.store(true, Ordering::Relaxed);
        if self.estado() == Estado::Esperando {
            self.definir_estado(Estado::Executando);
        }
    }

    /// Marca que soltou a trava de dados.
    pub fn sem_a_trava(&self) {
        self.com_trava.store(false, Ordering::Relaxed);
    }

    /// Abre uma fase que o encerramento alcanca.
    ///
    /// A guarda devolvida fecha a fase ao sair -- inclusive quando o laco sai
    /// por `?` ou por panico. Sem isso, uma atividade que terminou a
    /// varredura continuaria anunciada como cancelavel enquanto grava.
    pub fn fase_cancelavel(&self, nome: &str) -> FaseAberta<'_> {
        if let Ok(mut c) = self.dentro.lock() {
            c.fase = nome.to_string();
        }
        self.cancelavel.store(true, Ordering::Relaxed);
        FaseAberta { at: self }
    }

    /// **O ponto de cancelamento.** Chame entre duas unidades de trabalho
    /// seguras -- nunca no meio de uma gravacao.
    ///
    /// Custa um `fetch_add` e um `load`, os dois `Relaxed`, num `Arc` que o
    /// laco ja tem. E o unico ponto da telemetria que roda por LINHA, e por
    /// isso e o unico escrito para caber em duas instrucoes.
    pub fn siga(&self, passos: u64) -> Result<()> {
        self.passos.fetch_add(passos, Ordering::Relaxed);
        let mirado = self.encerrar_serial.load(Ordering::Relaxed);
        if mirado != 0 && mirado == self.serial.load(Ordering::Relaxed) {
            self.encerradas.fetch_add(1, Ordering::Relaxed);
            // A marca sai aqui: ela ja cumpriu o que prometia, e deixa-la
            // levantada mataria o proximo pedido desta mesma conexao.
            self.encerrar_serial.store(0, Ordering::Relaxed);
            self.definir_estado(Estado::Executando);
            let quem = self
                .quem_encerrou
                .lock()
                .map(|q| q.clone())
                .unwrap_or_default();
            let onde = self
                .dentro
                .lock()
                .map(|c| {
                    if c.fase.is_empty() {
                        c.op.clone()
                    } else {
                        format!("{} ({})", c.op, c.fase)
                    }
                })
                .unwrap_or_default();
            return Err(PhxError::Cancelado(format!(
                "operacao encerrada por {} apos {} unidade(s) de trabalho em {}; \
                 o que ja estava gravado continua gravado e o arquivo esta integro",
                if quem.is_empty() {
                    "quem administra"
                } else {
                    &quem
                },
                self.passos.load(Ordering::Relaxed),
                if onde.is_empty() { "curso" } else { &onde },
            )));
        }
        Ok(())
    }

    /// Manda encerrar a operacao corrente. Devolve o que da para prometer.
    pub fn encerrar(&self, quem: &str) -> Encerramento {
        let serial = self.serial.load(Ordering::Relaxed);
        let (op, fase) = self
            .dentro
            .lock()
            .map(|c| (c.op.clone(), c.fase.clone()))
            .unwrap_or_default();
        if op.is_empty() {
            return Encerramento::Ociosa;
        }
        if let Ok(mut q) = self.quem_encerrou.lock() {
            *q = quem.to_string();
        }
        self.encerrar_serial.store(serial, Ordering::SeqCst);
        if self.cancelavel.load(Ordering::Relaxed) {
            // Dentro do laco: a proxima unidade de trabalho para. Promessa
            // forte, e a unica que se pode fazer sem ressalva.
            self.definir_estado(Estado::Encerrando);
            Encerramento::Marcada { op, fase }
        } else if self.tem_ponto.load(Ordering::Relaxed) {
            // Tem ponto, mas nao esta nele: ou ainda nao chegou -- o caso da
            // fila da trava, que pode durar segundos --, ou ja passou. A marca
            // fica posta mirando este serial e vale para o primeiro ponto que
            // vier. Prometer menos do que isso seria mentir para o outro lado.
            self.definir_estado(Estado::Encerrando);
            Encerramento::Posta { op }
        } else {
            Encerramento::FaseNaoCancelavel { op }
        }
    }

    /// O peso que da o tamanho da bolha: milissegundos de servidor gastos.
    ///
    /// # Por que tempo, e nao linhas
    ///
    /// Linha lida e linha gravada custam coisas diferentes, e uma tabela
    /// larga custa mais que uma estreita -- somar linhas compararia coisas
    /// que nao se comparam. Tempo de servidor e a moeda unica: e exatamente o
    /// que a atividade tirou de todo mundo, porque a trava de dados e uma so.
    ///
    /// O acumulado entra junto com o corrente de proposito: uma conexao que
    /// fez trezentas consultas de 50 ms pesou 15 segundos do servidor, e uma
    /// tela que so mostrasse a consulta corrente diria que ela nao fez nada.
    pub fn peso_ms(&self) -> u64 {
        let corrente = self
            .comecou
            .lock()
            .ok()
            .and_then(|i| *i)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        self.consumido_ms.load(Ordering::Relaxed) + corrente
    }

    /// Ha quanto tempo a operacao corrente esta em curso -- espera incluida.
    ///
    /// E o relogio de PAREDE do pedido, e nao o tempo de servidor: quem
    /// pediu esta esperando ha tanto tempo, esteja a operacao trabalhando ou
    /// parada na fila. Confundir os dois faria a tela dizer «0 ms» de uma
    /// consulta que ja bloqueou o cliente por meio minuto.
    pub fn ha_ms(&self) -> u64 {
        self.pedido_desde
            .lock()
            .ok()
            .and_then(|i| *i)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// Quanto tempo de SERVIDOR a operacao corrente ja gastou.
    pub fn trabalhando_ha_ms(&self) -> u64 {
        self.comecou
            .lock()
            .ok()
            .and_then(|i| *i)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// A ficha completa da bolha -- o que o descritivo mostra ao clicar.
    ///
    /// `stress` diz se o SERVIDOR esta apertado agora: a cor vermelha nao e
    /// so da atividade, e da situacao. Quem decide isso e quem tem as series
    /// na mao, e nao a atividade sozinha.
    ///
    /// Os limiares chegam por parametro em vez de sairem da constante: quem
    /// os tem na mao e o registro, e o mesmo par que decide o nivel aqui vai
    /// para o campo `limiares` da resposta, que e o que a legenda escreve. Dois
    /// numeros para a mesma regra e como a tela acaba pintando o que o
    /// servidor nao concorda.
    pub fn para_json(
        &self,
        agora_ms: i64,
        stress_no_servidor: bool,
        ha_fila: bool,
        limiares: &crate::config::Painel,
    ) -> Json {
        let c = self
            .dentro
            .lock()
            .map(|c| c.clone())
            .unwrap_or_else(|e| e.into_inner().clone());
        let ha = self.ha_ms();
        let peso = self.peso_ms();
        let estado = self.estado();
        let cancelavel = self.cancelavel.load(Ordering::Relaxed);
        let executando = !c.op.is_empty();
        // A cor sai do SERVIDOR, e nao da tela. A tela pinta o que este campo
        // diz; se a regra morasse la, mudar o limiar exigiria mudar os dois.
        //
        // # O vermelho tem de apontar UMA atividade
        //
        // A primeira versao pintava de vermelho toda atividade em curso
        // enquanto o servidor estivesse em stress. Exercitando, o painel
        // inteiro virou vermelho de uma vez -- e a cor, que existe para
        // separar, deixou de separar qualquer coisa. Quem opera olha o painel
        // justamente para achar QUAL delas e o problema.
        //
        // Agora o vermelho e da atividade que esta segurando todo mundo: ela
        // esta EXECUTANDO (com a trava na mao) enquanto ha gente na fila.
        // Isso, ou ela sozinha ja passou do limiar de stress. O aperto do
        // SERVIDOR continua dito -- em vermelho, com o motivo -- na barra do
        // topo, que e onde ele e do servidor e nao de ninguem em particular.
        // Sem exigir tempo minimo: quem esta com a trava na mao enquanto ha
        // gente na fila ja e o culpado no primeiro milissegundo. Exigir «ha
        // mais de zero ms» so criava um instante em que o painel nao apontava
        // ninguem -- e era justamente o instante em que a fila comecou.
        // «Segurando todo mundo» exige a TRAVA na mao, e nao apenas estar
        // executando: quem nunca pediu a trava nao segura ninguem.
        let com_trava = self.com_trava.load(Ordering::Relaxed);
        let segurando_todo_mundo =
            stress_no_servidor && ha_fila && com_trava && estado == Estado::Executando;
        // O vermelho olha o tempo de TRABALHO, e nao o de parede. Uma conexao
        // parada ha meio minuto na fila tem `ha` enorme e nao esta fazendo
        // nada de errado -- ela e vitima. Pinta-la de vermelho junto com o
        // culpado foi o que deixou o painel inteiro vermelho na primeira
        // rodada, e cor que pinta todo mundo nao separa ninguem.
        let trabalhando = self.trabalhando_ha_ms();
        let nivel = if estado == Estado::Encerrando {
            "encerrando"
        } else if estado == Estado::Executando
            && (trabalhando >= limiares.stress_ms || segurando_todo_mundo)
        {
            "stress"
        } else if executando && (ha >= limiares.alto_uso_ms || estado == Estado::Esperando) {
            "alto"
        } else {
            "normal"
        };
        Json::objeto(vec![
            ("id", Json::texto_de(&self.chave)),
            ("origem", Json::texto_de(self.origem)),
            (
                "ligacao",
                match self.ligacao {
                    0 => Json::Nulo,
                    n => Json::de_u64(n),
                },
            ),
            (
                "usuario",
                match c.usuario.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&c.usuario),
                },
            ),
            ("ip", Json::texto_de(&self.ip)),
            (
                "op",
                match c.op.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&c.op),
                },
            ),
            (
                "alvo",
                match (c.database.is_empty(), c.tabela.is_empty()) {
                    (true, true) => Json::Nulo,
                    (false, true) => Json::texto_de(&c.database),
                    (true, false) => Json::texto_de(&c.tabela),
                    _ => Json::texto_de(format!("{}.{}", c.database, c.tabela)),
                },
            ),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
            ),
            (
                "aberta_s",
                Json::de_u64(((agora_ms - self.desde_ms) / 1_000).max(0) as u64),
            ),
            (
                "op_desde",
                match c.inicio_ms {
                    0 => Json::Nulo,
                    q => Json::texto_de(phxsql_core::datahora::instante_iso(q)),
                },
            ),
            ("ha_ms", Json::de_u64(ha)),
            // Os dois relogios, lado a lado: o de parede e o de servidor. A
            // diferenca entre eles E a espera, e ela e o numero que explica
            // «por que esta lento» sem ninguem ter de deduzir.
            ("trabalhando_ms", Json::de_u64(trabalhando)),
            ("esperou_ms", Json::de_u64(ha.saturating_sub(trabalhando))),
            ("estado", Json::texto_de(estado.nome())),
            ("nivel", Json::texto_de(nivel)),
            ("peso_ms", Json::de_u64(peso)),
            ("passos", Json::de_u64(self.passos.load(Ordering::Relaxed))),
            (
                "pedidos",
                Json::de_u64(self.pedidos.load(Ordering::Relaxed)),
            ),
            (
                "fase",
                match c.fase.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&c.fase),
                },
            ),
            // O que o botao de encerrar pode prometer AGORA. A tela usa isto
            // para nao oferecer um botao que nao cumpre.
            ("com_trava", Json::Bool(com_trava)),
            ("cancelavel", Json::Bool(cancelavel)),
            // «Tem onde parar» e diferente de «esta parando agora»: o botao
            // olha o primeiro, a promessa da resposta olha o segundo.
            (
                "tem_ponto",
                Json::Bool(self.tem_ponto.load(Ordering::Relaxed)),
            ),
            ("encerrando", Json::Bool(estado == Estado::Encerrando)),
            (
                "encerradas",
                Json::de_u64(self.encerradas.load(Ordering::Relaxed)),
            ),
            (
                "esperando_o_que",
                Json::texto_de(match estado {
                    Estado::Esperando => "a trava de dados, que outra atividade esta segurando",
                    Estado::Executando if cancelavel => {
                        "nada: esta trabalhando, em fase cancelavel"
                    }
                    Estado::Executando if !com_trava => {
                        "nada: esta trabalhando e nao precisa da trava de dados"
                    }
                    Estado::Executando => "nada: esta trabalhando, dentro do ponto critico",
                    Estado::Encerrando => "chegar no proximo ponto seguro para abortar",
                    Estado::Ociosa => "o proximo pedido do cliente",
                }),
            ),
        ])
    }
}

/// O que `encerrar` conseguiu prometer.
pub enum Encerramento {
    /// Nao havia operacao em curso.
    Ociosa,
    /// Esta DENTRO do laco: aborta na proxima unidade de trabalho.
    Marcada { op: String, fase: String },
    /// A operacao tem ponto de cancelamento, mas nao esta nele agora --
    /// tipicamente esperando a trava de dados. A marca vale para o primeiro
    /// ponto que vier; pode ser que ela ja tenha passado do ultimo.
    Posta { op: String },
    /// A operacao nao tem ponto de cancelamento nenhum: vai terminar.
    FaseNaoCancelavel { op: String },
}

/// Guarda de fase cancelavel: fecha a fase ao sair, por qualquer caminho.
pub struct FaseAberta<'a> {
    at: &'a Atividade,
}

impl Drop for FaseAberta<'_> {
    fn drop(&mut self) {
        self.at.cancelavel.store(false, Ordering::Relaxed);
        if let Ok(mut c) = self.at.dentro.lock() {
            c.fase.clear();
        }
    }
}

// ------------------------------------------------------------------ threads

/// Uma thread do `phxsqld`, com a finalidade ESCRITA.
///
/// # Por que a finalidade e obrigatoria
///
/// Uma thread sem dono declarado e uma thread que ninguem acompanha: quando
/// ela morre, o servico que ela prestava para de existir e nada avisa. Exigir
/// a frase no momento de subir a thread e o que impede a proxima nascer
/// anonima -- e o tipo de coisa que nao da para acrescentar depois, porque
/// depois ninguem lembra para que ela era.
pub struct Fio {
    pub id: u64,
    pub nome: String,
    pub finalidade: &'static str,
    /// `servico` (vive enquanto o servidor vive), `atendimento` (uma conexao
    /// da porta de dados) ou `web` (um pedido da interface).
    pub familia: &'static str,
    pub desde_ms: i64,
    fazendo: Mutex<String>,
    voltas: AtomicU64,
    viva: AtomicBool,
}

impl Fio {
    /// Anota o que a thread esta fazendo agora, e conta mais uma volta.
    ///
    /// So para laco de fundo, que da uma volta por segundo no maximo. Nao
    /// serve para caminho quente: ele toma uma trava.
    pub fn fazendo(&self, o_que: &str) {
        if let Ok(mut f) = self.fazendo.lock() {
            f.clear();
            f.push_str(o_que);
        }
        self.voltas.fetch_add(1, Ordering::Relaxed);
    }

    pub fn viva(&self) -> bool {
        self.viva.load(Ordering::Relaxed)
    }

    pub fn para_json(&self, agora_ms: i64) -> Json {
        Json::objeto(vec![
            ("id", Json::de_u64(self.id)),
            ("nome", Json::texto_de(&self.nome)),
            ("finalidade", Json::texto_de(self.finalidade)),
            ("familia", Json::texto_de(self.familia)),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
            ),
            (
                "viva_s",
                Json::de_u64(((agora_ms - self.desde_ms) / 1_000).max(0) as u64),
            ),
            (
                "fazendo",
                Json::texto_de(
                    self.fazendo
                        .lock()
                        .map(|f| f.clone())
                        .unwrap_or_else(|e| e.into_inner().clone()),
                ),
            ),
            ("voltas", Json::de_u64(self.voltas.load(Ordering::Relaxed))),
            ("viva", Json::Bool(self.viva())),
        ])
    }
}

// ------------------------------------------------------------------ amostra

/// Um instante das series do topo.
#[derive(Debug, Clone, Default)]
pub struct Amostra {
    pub quando_ms: i64,
    /// Quanto tempo passou desde a amostra anterior, de verdade.
    pub intervalo_ms: u64,
    /// Quanto essa distancia passou do periodo pedido -- o atraso do relogio.
    pub atraso_ms: u64,
    pub ociosas: u64,
    pub esperando: u64,
    /// A espera mais longa em curso neste instante, em milissegundos.
    pub espera_maior_ms: u64,
    pub executando: u64,
    pub encerrando: u64,
    pub leituras_s: f64,
    pub escritas_s: f64,
    pub erros_s: f64,
    pub espera_ms_s: f64,
    pub trava_ms_s: f64,
    pub cpu_processo: f64,
    pub cpu_maquina: f64,
    pub ler_bytes_s: f64,
    pub escrever_bytes_s: f64,
    pub cache_acertos_s: f64,
    pub cache_faltas_s: f64,
    pub memoria_kb: u64,
}

impl Amostra {
    fn para_json(&self) -> Json {
        let f2 = |v: f64| Json::texto_de(format!("{v:.2}"));
        Json::objeto(vec![
            ("t", Json::de_i64(self.quando_ms)),
            ("intervalo_ms", Json::de_u64(self.intervalo_ms)),
            ("atraso_ms", Json::de_u64(self.atraso_ms)),
            ("ociosas", Json::de_u64(self.ociosas)),
            ("esperando", Json::de_u64(self.esperando)),
            ("espera_maior_ms", Json::de_u64(self.espera_maior_ms)),
            ("executando", Json::de_u64(self.executando)),
            ("encerrando", Json::de_u64(self.encerrando)),
            ("leituras_s", f2(self.leituras_s)),
            ("escritas_s", f2(self.escritas_s)),
            ("erros_s", f2(self.erros_s)),
            ("espera_ms_s", f2(self.espera_ms_s)),
            ("trava_ms_s", f2(self.trava_ms_s)),
            ("cpu_processo", f2(self.cpu_processo)),
            ("cpu_maquina", f2(self.cpu_maquina)),
            ("ler_bytes_s", f2(self.ler_bytes_s)),
            ("escrever_bytes_s", f2(self.escrever_bytes_s)),
            ("cache_acertos_s", f2(self.cache_acertos_s)),
            ("cache_faltas_s", f2(self.cache_faltas_s)),
            ("memoria_kb", Json::de_u64(self.memoria_kb)),
        ])
    }
}

/// Contadores crus de um instante, para as taxas saírem de duas amostras.
#[derive(Debug, Clone, Default)]
struct Crus {
    quando: Option<Instant>,
    leituras: u64,
    escritas: u64,
    erros: u64,
    espera_us: u64,
    trava_us: u64,
    cache_acertos: u64,
    cache_faltas: u64,
    /// Jiffies do processo (utime + stime).
    cpu_processo: u64,
    /// (ocupado, total) da maquina.
    cpu_maquina: (u64, u64),
    /// Bytes que o processo pediu ao disco DE VERDADE.
    ler_bytes: u64,
    escrever_bytes: u64,
}

// ------------------------------------------------------------------ registro

/// O registro central: atividades, threads, contadores e a serie.
pub struct Telemetria {
    /// **O portao.** Lido com `Relaxed` no comeco de todo ponto de captura.
    ligada: AtomicBool,
    atividades: Mutex<HashMap<String, Arc<Atividade>>>,
    fios: Mutex<Vec<Arc<Fio>>>,
    amostras: Mutex<VecDeque<Amostra>>,
    anterior: Mutex<Crus>,
    proximo_fio: AtomicU64,
    /// O amostrador ja subiu neste processo?
    amostrador: AtomicBool,
    ligada_em_ms: AtomicI64,

    leituras: AtomicU64,
    escritas: AtomicU64,
    erros: AtomicU64,
    /// Microssegundos acumulados na fila da trava de dados.
    espera_us: AtomicU64,
    /// Microssegundos acumulados COM a trava de dados na mao.
    trava_us: AtomicU64,
    /// Quantas atividades foram encerradas desde que o servidor subiu.
    encerramentos: AtomicU64,
    /// Threads de atendimento vivas agora (as efemeras nao entram na lista).
    fios_vivos: AtomicUsize,
    /// As cores e os limiares que o `config.json` escolheu.
    ///
    /// # Por que aqui dentro, e nao num global de processo
    ///
    /// Porque um global seria estado compartilhado entre testes que rodam em
    /// paralelo no mesmo processo: um teste que configura uma cor apagaria o
    /// `sem_cor_configurada_nada_muda` do vizinho de vez em quando, e teste que
    /// falha as vezes e pior que teste que falta. Aqui a pintura pertence ao
    /// registro, como tudo o mais que ele responde.
    ///
    /// E lida uma vez por retrato -- de dois em dois segundos, quando alguem
    /// tem o painel aberto --, e nao no caminho quente de pedido nenhum.
    pintura: Mutex<crate::config::Painel>,
}

impl Default for Telemetria {
    fn default() -> Self {
        Telemetria::nova(true)
    }
}

impl Telemetria {
    /// `ligada` decide se ela nasce coletando.
    pub fn nova(ligada: bool) -> Telemetria {
        Telemetria {
            ligada: AtomicBool::new(ligada),
            atividades: Mutex::new(HashMap::new()),
            fios: Mutex::new(Vec::new()),
            amostras: Mutex::new(VecDeque::with_capacity(AMOSTRAS_GUARDADAS)),
            anterior: Mutex::new(Crus::default()),
            proximo_fio: AtomicU64::new(0),
            amostrador: AtomicBool::new(false),
            ligada_em_ms: AtomicI64::new(0),
            leituras: AtomicU64::new(0),
            escritas: AtomicU64::new(0),
            erros: AtomicU64::new(0),
            espera_us: AtomicU64::new(0),
            trava_us: AtomicU64::new(0),
            encerramentos: AtomicU64::new(0),
            fios_vivos: AtomicUsize::new(0),
            pintura: Mutex::new(crate::config::Painel::default()),
        }
    }

    /// A pintura que o `config.json` pediu.
    ///
    /// **Este e o leitor.** Sem esta chamada -- no arranque do servidor e de
    /// novo a cada gravacao pela tela -- o bloco `telemetria` do arquivo seria
    /// mais um campo que ninguem le, que e pior que campo ausente: o ausente
    /// ninguem ajusta esperando efeito.
    pub fn definir_pintura(&self, p: crate::config::Painel) {
        match self.pintura.lock() {
            Ok(mut v) => *v = p,
            Err(e) => *e.into_inner() = p,
        }
    }

    pub fn pintura(&self) -> crate::config::Painel {
        match self.pintura.lock() {
            Ok(v) => v.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    /// **O portao.** Uma instrucao, sem trava e sem alocacao.
    #[inline]
    pub fn ligada(&self) -> bool {
        self.ligada.load(Ordering::Relaxed)
    }

    pub fn ligar(&self, agora_ms: i64) {
        self.ligada.store(true, Ordering::Relaxed);
        self.ligada_em_ms.store(agora_ms, Ordering::Relaxed);
    }

    /// Desliga a coleta, joga fora a serie e esvazia o registro.
    ///
    /// # Por que as tres coisas, e nao so o interruptor
    ///
    /// A serie sai porque ela ficaria com um buraco no meio, e um grafico com
    /// buraco disfarcado de continuidade mente sobre o que aconteceu ali.
    ///
    /// As ATIVIDADES saem pelo mesmo motivo, e foi um teste que mostrou: sem
    /// isso, desligar deixava as bolhas na tela paradas no ultimo estado que
    /// tiveram. Elas continuariam ali, com aparencia de vivas, sem ninguem
    /// atualizando nenhuma -- e o operador acharia que aquela consulta longa
    /// ainda esta rodando quando ela terminou ha meia hora. Painel congelado
    /// mente pior do que painel vazio, porque parece que esta funcionando.
    ///
    /// Ligar de volta nao exige reconectar: a atividade e aberta a cada
    /// pedido, entao a proxima operacao de cada conexao a traz de volta.
    pub fn desligar(&self) {
        self.ligada.store(false, Ordering::Relaxed);
        if let Ok(mut a) = self.amostras.lock() {
            a.clear();
        }
        if let Ok(mut c) = self.anterior.lock() {
            *c = Crus::default();
        }
        if let Ok(mut m) = self.atividades.lock() {
            m.clear();
        }
    }

    pub fn ligada_em_ms(&self) -> i64 {
        self.ligada_em_ms.load(Ordering::Relaxed)
    }

    // ------------------------------------------------------------ atividades

    /// Registra uma atividade, ou devolve a que ja existe com esta chave.
    ///
    /// Reaproveitar a mesma chave e o que da estabilidade a bolha: a tela do
    /// navegador pergunta de dois em dois segundos, e cada pergunta e uma
    /// conexao HTTP nova -- se cada uma virasse uma bolha nova, o painel
    /// piscaria em vez de mostrar alguma coisa.
    pub fn entrar(
        &self,
        chave: &str,
        origem: &'static str,
        ip: &str,
        ligacao: u64,
        agora_ms: i64,
    ) -> Option<Arc<Atividade>> {
        if !self.ligada() {
            return None;
        }
        let mut mapa = self.atividades.lock().ok()?;
        if let Some(a) = mapa.get(chave) {
            return Some(Arc::clone(a));
        }
        let a = Arc::new(Atividade::nova(
            chave.to_string(),
            origem,
            ip.to_string(),
            ligacao,
            agora_ms,
        ));
        mapa.insert(chave.to_string(), Arc::clone(&a));
        Some(a)
    }

    pub fn sair(&self, chave: &str) {
        if let Ok(mut mapa) = self.atividades.lock() {
            mapa.remove(chave);
        }
    }

    pub fn atividade(&self, chave: &str) -> Option<Arc<Atividade>> {
        self.atividades.lock().ok()?.get(chave).cloned()
    }

    /// Tira do registro a atividade da web que sumiu.
    ///
    /// # Por que ela precisa disso e a da porta de dados nao
    ///
    /// A conexao da porta de dados AVISA quando acaba: a thread dela sai do
    /// laco e chama `sair`. Um navegador nao avisa nada -- a aba fecha, a
    /// maquina hiberna, e o servidor so descobre pelo silencio. Sem esta
    /// poda, cada aba que alguem abriu no mes ficaria como uma bolha ociosa
    /// eterna, e o painel viraria um cemiterio.
    ///
    /// A tela pergunta de dois em dois segundos enquanto esta aberta, entao
    /// um minuto de silencio e silencio de verdade.
    pub fn podar(&self, agora_ms: i64, silencio_ms: i64) {
        let Ok(mut mapa) = self.atividades.lock() else {
            return;
        };
        mapa.retain(|_, a| {
            a.origem != "web"
                || a.estado() != Estado::Ociosa
                || agora_ms - a.ultimo_ms.load(Ordering::Relaxed) < silencio_ms
        });
    }

    /// Todas as atividades vivas, da mais pesada para a mais leve.
    ///
    /// Ja ordenado aqui, e nao na tela: a ordem por tamanho e o que o painel
    /// de bolhas promete, e uma tela que reordenasse por conta poderia
    /// discordar do que o servidor mediu.
    pub fn atividades(&self) -> Vec<Arc<Atividade>> {
        let mut v: Vec<Arc<Atividade>> = match self.atividades.lock() {
            Ok(m) => m.values().cloned().collect(),
            Err(e) => e.into_inner().values().cloned().collect(),
        };
        v.sort_by(|a, b| {
            b.peso_ms()
                .cmp(&a.peso_ms())
                .then_with(|| a.chave.cmp(&b.chave))
        });
        v
    }

    // ------------------------------------------------------------ os contadores

    /// Conta um pedido concluido. Um `fetch_add` atras do portao.
    pub fn contar_pedido(&self, escrita: bool, ok: bool) {
        if !self.ligada() {
            return;
        }
        if escrita {
            self.escritas.fetch_add(1, Ordering::Relaxed);
        } else {
            self.leituras.fetch_add(1, Ordering::Relaxed);
        }
        if !ok {
            self.erros.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn contar_espera(&self, micros: u64) {
        if !self.ligada() {
            return;
        }
        self.espera_us.fetch_add(micros, Ordering::Relaxed);
    }

    pub fn contar_trava(&self, micros: u64) {
        if !self.ligada() {
            return;
        }
        self.trava_us.fetch_add(micros, Ordering::Relaxed);
    }

    pub fn contar_encerramento(&self) {
        self.encerramentos.fetch_add(1, Ordering::Relaxed);
    }

    // ------------------------------------------------------------ threads

    /// Registra uma thread e devolve a ficha dela.
    pub fn registrar_fio(
        &self,
        nome: impl Into<String>,
        finalidade: &'static str,
        familia: &'static str,
        agora_ms: i64,
    ) -> Arc<Fio> {
        let f = Arc::new(Fio {
            id: self.proximo_fio.fetch_add(1, Ordering::SeqCst) + 1,
            nome: nome.into(),
            finalidade,
            familia,
            desde_ms: agora_ms,
            fazendo: Mutex::new("subindo".into()),
            voltas: AtomicU64::new(0),
            viva: AtomicBool::new(true),
        });
        self.fios_vivos.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut v) = self.fios.lock() {
            v.push(Arc::clone(&f));
            // As efemeras (uma por conexao, uma por pedido web) sao muitas ao
            // longo do dia, e a lista nao pode crescer para sempre: o que ja
            // morreu sai quando ha gente demais. As de servico nunca saem --
            // elas sao a lista que interessa.
            if v.len() > 512 {
                v.retain(|x| x.viva() || x.familia == "servico");
            }
        }
        f
    }

    pub fn fio_morreu(&self, f: &Arc<Fio>) {
        f.viva.store(false, Ordering::Relaxed);
        if let Ok(mut fazendo) = f.fazendo.lock() {
            *fazendo = "encerrada".into();
        }
        self.fios_vivos.fetch_sub(1, Ordering::Relaxed);
    }

    /// Sobe uma thread ja registrada, com nome e finalidade.
    ///
    /// Devolve nada de proposito: quem sobe um laco de fundo nao espera por
    /// ele. O `JoinHandle` seria descartado do mesmo jeito, e guarda-lo daria
    /// a impressao de que alguem faz `join`.
    pub fn subir<F>(
        self: &Arc<Self>,
        nome: impl Into<String>,
        finalidade: &'static str,
        familia: &'static str,
        agora_ms: i64,
        corpo: F,
    ) where
        F: FnOnce(Arc<Fio>) + Send + 'static,
    {
        let ficha = self.registrar_fio(nome, finalidade, familia, agora_ms);
        let eu = Arc::clone(self);
        let para_thread = Arc::clone(&ficha);
        // O nome tambem vai para o sistema operacional: e o que aparece no
        // `top -H` e no `gdb`, e sem ele toda thread do processo se chama
        // `phxsqld` e o `top` nao ajuda ninguem.
        let nome_do_so: String = ficha.nome.chars().take(15).collect();
        let subiu = std::thread::Builder::new().name(nome_do_so).spawn(move || {
            corpo(Arc::clone(&para_thread));
            eu.fio_morreu(&para_thread);
        });
        if subiu.is_err() {
            // Falhar em CRIAR a thread e noticia: o servico que ela prestava
            // nao vai existir, e uma ficha «viva» de uma thread que nunca
            // nasceu seria a pior mentira que este registro poderia contar.
            self.fio_morreu(&ficha);
            eprintln!(
                "AVISO: nao consegui subir a thread {} ({})",
                ficha.nome, ficha.finalidade
            );
        }
    }

    /// As threads, as de servico primeiro.
    pub fn fios(&self) -> Vec<Arc<Fio>> {
        let mut v: Vec<Arc<Fio>> = match self.fios.lock() {
            Ok(f) => f.clone(),
            Err(e) => e.into_inner().clone(),
        };
        v.sort_by_key(|f| (f.familia != "servico", f.id));
        v
    }

    // ------------------------------------------------------------ amostragem

    /// Tira uma amostra das series. Chamado pelo amostrador, uma vez por
    /// periodo -- nunca pelo caminho de um pedido.
    pub fn amostrar(&self, cpu_maquina: (u64, u64), agora_ms: i64) {
        if !self.ligada() {
            return;
        }
        let (acertos, faltas, _) = phxsql_store::ndx::contadores_de_cache();
        let (cpu_proc, memoria_kb) = cpu_e_memoria_do_processo();
        let (lidos, escritos) = bytes_do_processo();
        let agora = Crus {
            quando: Some(Instant::now()),
            leituras: self.leituras.load(Ordering::Relaxed),
            escritas: self.escritas.load(Ordering::Relaxed),
            erros: self.erros.load(Ordering::Relaxed),
            espera_us: self.espera_us.load(Ordering::Relaxed),
            trava_us: self.trava_us.load(Ordering::Relaxed),
            cache_acertos: acertos,
            cache_faltas: faltas,
            cpu_processo: cpu_proc,
            cpu_maquina,
            ler_bytes: lidos,
            escrever_bytes: escritos,
        };

        let mut anterior = match self.anterior.lock() {
            Ok(a) => a,
            Err(e) => e.into_inner(),
        };
        let segundos = match (anterior.quando, agora.quando) {
            (Some(a), Some(b)) => b.duration_since(a).as_secs_f64(),
            // A primeira amostra nao tem taxa, pelo mesmo motivo do monitor da
            // maquina: taxa so existe entre dois instantes. Ela entra na serie
            // com zeros, e nao com o acumulado desde o arranque disfarcado de
            // "agora".
            _ => 0.0,
        };
        let taxa = |novo: u64, velho: u64| {
            if segundos > 0.0 && novo >= velho {
                (novo - velho) as f64 / segundos
            } else {
                0.0
            }
        };

        let (mut ociosas, mut esperando, mut executando, mut encerrando) = (0, 0, 0, 0);
        // A espera ACUMULADA so entra na conta quando a espera termina -- ela
        // e somada no instante em que a trava chega na mao. Entao, no meio de
        // uma fila longa, o «ms/s na fila» ainda diz zero, e a tela pareceria
        // calma justamente no pior momento. A maior espera EM CURSO conserta
        // isso: ela existe enquanto a fila existe.
        let mut espera_maior_ms = 0;
        for a in self.atividades() {
            match a.estado() {
                Estado::Ociosa => ociosas += 1,
                Estado::Esperando => {
                    esperando += 1;
                    espera_maior_ms = espera_maior_ms.max(a.ha_ms());
                }
                Estado::Executando => executando += 1,
                Estado::Encerrando => encerrando += 1,
            }
        }

        // Jiffy do Linux e 1/100 s. Multiplicar por 100 e dividir pelo tempo
        // decorrido da o percentual de UM nucleo -- que passa de 100 num
        // processo com varias threads ocupadas, e e isso mesmo.
        let cpu_processo = if segundos > 0.0 {
            (agora.cpu_processo.saturating_sub(anterior.cpu_processo) as f64) / segundos
        } else {
            0.0
        };
        let (ocup, tot) = (
            agora.cpu_maquina.0.saturating_sub(anterior.cpu_maquina.0),
            agora.cpu_maquina.1.saturating_sub(anterior.cpu_maquina.1),
        );
        let cpu_maquina = if tot > 0 && anterior.cpu_maquina.1 > 0 {
            ocup as f64 / tot as f64 * 100.0
        } else {
            0.0
        };

        let intervalo_ms = (segundos * 1000.0) as u64;
        let amostra = Amostra {
            quando_ms: agora_ms,
            intervalo_ms,
            atraso_ms: intervalo_ms.saturating_sub(PERIODO_DA_AMOSTRA_MS),
            ociosas,
            esperando,
            espera_maior_ms,
            executando,
            encerrando,
            leituras_s: taxa(agora.leituras, anterior.leituras),
            escritas_s: taxa(agora.escritas, anterior.escritas),
            erros_s: taxa(agora.erros, anterior.erros),
            // Microssegundos acumulados viram milissegundos POR SEGUNDO: e o
            // numero que se le como "quantos milissegundos de cada segundo o
            // servidor passou esperando".
            espera_ms_s: taxa(agora.espera_us, anterior.espera_us) / 1_000.0,
            trava_ms_s: taxa(agora.trava_us, anterior.trava_us) / 1_000.0,
            cpu_processo,
            cpu_maquina,
            ler_bytes_s: taxa(agora.ler_bytes, anterior.ler_bytes),
            escrever_bytes_s: taxa(agora.escrever_bytes, anterior.escrever_bytes),
            cache_acertos_s: taxa(agora.cache_acertos, anterior.cache_acertos),
            cache_faltas_s: taxa(agora.cache_faltas, anterior.cache_faltas),
            memoria_kb,
        };
        *anterior = agora;
        drop(anterior);

        if let Ok(mut fila) = self.amostras.lock() {
            if fila.len() >= AMOSTRAS_GUARDADAS {
                fila.pop_front();
            }
            fila.push_back(amostra);
        }
    }

    /// A serie, da mais antiga para a mais nova.
    pub fn amostras(&self, max: usize) -> Vec<Amostra> {
        let fila = match self.amostras.lock() {
            Ok(f) => f,
            Err(e) => e.into_inner(),
        };
        let sobra = fila.len().saturating_sub(max);
        fila.iter().skip(sobra).cloned().collect()
    }

    pub fn amostrador_no_ar(&self) -> bool {
        self.amostrador.load(Ordering::Relaxed)
    }

    pub fn marcar_amostrador(&self) -> bool {
        !self.amostrador.swap(true, Ordering::SeqCst)
    }

    /// O servidor esta em stress agora? Devolve o flag e o MOTIVO.
    ///
    /// # Por que o motivo anda junto
    ///
    /// "Servidor em stress" e um adjetivo, e adjetivo nao se conserta. O que
    /// se conserta e o motivo -- a CPU no teto, meio segundo de cada segundo
    /// gasto na fila da trava, uma atividade parada ha mais de cinco segundos
    /// esperando. Uma bolha vermelha sem motivo escrito manda quem opera
    /// adivinhar.
    ///
    /// # Por que o disco NAO entra aqui
    ///
    /// Saber se o disco esta apertado custa um `df`, que e um processo do
    /// sistema. A tela pergunta de dois em dois segundos: seria um processo
    /// novo a cada duas segundos so para pintar uma bolha. O aperto de disco
    /// ja tem dono -- o vigia de disco e o painel de sistema -- e ele avisa
    /// por e-mail, que e mais util do que uma cor.
    pub fn stress(&self) -> (bool, String) {
        let ultima = {
            let fila = match self.amostras.lock() {
                Ok(f) => f,
                Err(e) => e.into_inner(),
            };
            fila.back().cloned()
        };
        let mut motivos = Vec::new();
        if let Some(a) = &ultima {
            if a.cpu_maquina >= 90.0 {
                motivos.push(format!("CPU da maquina em {:.0}%", a.cpu_maquina));
            }
            // Meio segundo de espera em cada segundo quer dizer que, na media,
            // ha sempre alguem parado na fila da trava de dados.
            if a.espera_ms_s >= 500.0 {
                motivos.push(format!(
                    "{:.0} ms de cada segundo na fila da trava de dados",
                    a.espera_ms_s
                ));
            }
        }
        let stress_ms = self.pintura().stress_ms;
        for at in self.atividades() {
            if at.estado() == Estado::Esperando && at.ha_ms() >= stress_ms {
                motivos.push(format!(
                    "{} esperando a trava ha {} s",
                    at.chave,
                    at.ha_ms() / 1_000
                ));
                break;
            }
        }
        (!motivos.is_empty(), motivos.join("; "))
    }

    /// O retrato inteiro, pronto para a tela.
    pub fn para_json(&self, agora_ms: i64, max_amostras: usize) -> Json {
        let (stress, motivo_do_stress) = self.stress();
        let amostras = self.amostras(max_amostras);
        let ultima = amostras.last().map(|a| a.quando_ms).unwrap_or(0);
        let atividades = self.atividades();
        // Ha alguem na fila da trava? E o que distingue «uma consulta longa
        // rodando sozinha», que nao atrapalha ninguem, de «uma consulta longa
        // segurando o servidor inteiro».
        let ha_fila = atividades.iter().any(|a| a.estado() == Estado::Esperando);
        // Lida UMA vez por retrato, e usada nos tres lugares que precisam
        // concordar: o nivel de cada atividade, os limiares que a legenda
        // escreve e as cores que a tela pinta.
        let pintura = self.pintura();
        let mut retrato = Json::objeto(vec![
            ("ligada", Json::Bool(self.ligada())),
            (
                "ligada_em",
                match self.ligada_em_ms() {
                    0 => Json::Nulo,
                    q => Json::texto_de(phxsql_core::datahora::instante_iso(q)),
                },
            ),
            ("amostrador", Json::Bool(self.amostrador_no_ar())),
            ("periodo_ms", Json::de_u64(PERIODO_DA_AMOSTRA_MS)),
            (
                "agora",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora_ms)),
            ),
            // O instante da ULTIMA amostra e a distancia dele para agora: e o
            // que responde "ha atraso?" sem obrigar quem olha a comparar dois
            // relogios. Sem isto, uma serie congelada parece uma serie calma.
            (
                "ultima_amostra",
                match ultima {
                    0 => Json::Nulo,
                    q => Json::texto_de(phxsql_core::datahora::instante_iso(q)),
                },
            ),
            (
                "atraso_ms",
                Json::de_u64(match ultima {
                    0 => 0,
                    q => (agora_ms - q).max(0) as u64,
                }),
            ),
            // Qual das bolhas e a de QUEM ESTA PERGUNTANDO. A tela usa isto
            // para nao oferecer a alguem o botao de encerrar a si mesmo -- o
            // pedido morreria antes de responder o que aconteceu --, e o
            // teste usa para achar a propria atividade sem adivinhar o
            // numero da conexao.
            (
                "voce",
                match corrente() {
                    Some(a) => Json::texto_de(&a.chave),
                    None => Json::Nulo,
                },
            ),
            ("stress", Json::Bool(stress)),
            (
                "stress_por_que",
                match motivo_do_stress.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&motivo_do_stress),
                },
            ),
            (
                "limiares",
                Json::objeto(vec![
                    ("alto_uso_ms", Json::de_u64(pintura.alto_uso_ms)),
                    ("stress_ms", Json::de_u64(pintura.stress_ms)),
                ]),
            ),
            (
                "totais",
                Json::objeto(vec![
                    (
                        "leituras",
                        Json::de_u64(self.leituras.load(Ordering::Relaxed)),
                    ),
                    (
                        "escritas",
                        Json::de_u64(self.escritas.load(Ordering::Relaxed)),
                    ),
                    ("erros", Json::de_u64(self.erros.load(Ordering::Relaxed))),
                    (
                        "espera_ms",
                        Json::de_u64(self.espera_us.load(Ordering::Relaxed) / 1_000),
                    ),
                    (
                        "trava_ms",
                        Json::de_u64(self.trava_us.load(Ordering::Relaxed) / 1_000),
                    ),
                    (
                        "encerramentos",
                        Json::de_u64(self.encerramentos.load(Ordering::Relaxed)),
                    ),
                    (
                        "threads_vivas",
                        Json::de_u64(self.fios_vivos.load(Ordering::Relaxed) as u64),
                    ),
                ]),
            ),
            (
                "series",
                Json::Lista(amostras.iter().map(Amostra::para_json).collect()),
            ),
            (
                "atividades",
                Json::Lista(
                    atividades
                        .iter()
                        .map(|a| a.para_json(agora_ms, stress, ha_fila, &pintura))
                        .collect(),
                ),
            ),
            (
                "threads",
                Json::Lista(self.fios().iter().map(|f| f.para_json(agora_ms)).collect()),
            ),
        ]);
        // O campo so NASCE quando alguem escolheu cor. Sem escolha nenhuma a
        // resposta e a de sempre, e a tela pinta com as variaveis do tema --
        // que e o que faz as quatro escurecerem sozinhas no tema claro.
        if let Some(cores) = pintura.cores_json() {
            retrato.definir("cores", cores);
        }
        retrato
    }
}

// ------------------------------------------------------- a atividade da vez

thread_local! {
    /// A atividade que ESTA thread esta servindo agora.
    ///
    /// # Por que thread local, e nao um parametro
    ///
    /// O ponto de cancelamento precisa ser alcancavel de dentro de qualquer
    /// laco longo -- e eles estao espalhados por operacoes que recebem
    /// `(&self, pedido, sessao)` e mais nada. Passar a atividade por
    /// parametro obrigaria a mudar a assinatura de dezenas de funcoes que nao
    /// tem nada a ver com telemetria, so para carrega-la ate la.
    ///
    /// Thread local casa com o modelo do servidor: uma thread por conexao,
    /// uma conexao por atividade. Quem serve o pedido e quem le a marca.
    static CORRENTE: std::cell::RefCell<Option<Arc<Atividade>>> =
        const { std::cell::RefCell::new(None) };
}

/// Amarra a atividade a esta thread ate a guarda sair de escopo.
pub struct Amarrada;

impl Drop for Amarrada {
    fn drop(&mut self) {
        CORRENTE.with(|c| *c.borrow_mut() = None);
    }
}

/// Diz que esta thread esta servindo esta atividade.
pub fn amarrar(a: Option<Arc<Atividade>>) -> Amarrada {
    CORRENTE.with(|c| *c.borrow_mut() = a);
    Amarrada
}

/// A atividade que esta thread serve agora, se houver.
pub fn corrente() -> Option<Arc<Atividade>> {
    CORRENTE.with(|c| c.borrow().clone())
}

// ------------------------------------------------------------- /proc, na mao

/// (jiffies de CPU do processo, memoria residente em KB).
///
/// Sai de `/proc/self/stat`, que e o unico lugar que sabe quanto ESTE
/// processo gastou -- `/proc/stat` mede a maquina, e num servidor
/// compartilhado os dois numeros contam historias diferentes.
fn cpu_e_memoria_do_processo() -> (u64, u64) {
    let Ok(t) = std::fs::read_to_string("/proc/self/stat") else {
        return (0, 0);
    };
    // O campo 2 e o nome do executavel entre parenteses, e ele pode conter
    // espaco -- por isso a divisao comeca DEPOIS do ultimo `)`, e nao no
    // primeiro espaco. Sem isso, um binario chamado `phx sqld` desalinharia
    // todos os campos seguintes e a CPU viraria numero aleatorio.
    let Some(resto) = t.rsplit_once(')') else {
        return (0, 0);
    };
    let campos: Vec<&str> = resto.1.split_whitespace().collect();
    // Depois do `)` o primeiro campo e o `state`, que e o de indice 3 na
    // numeracao do proc(5). Entao utime (14) esta em 14-3-1 = 10.
    let n = |i: usize| {
        campos
            .get(i)
            .and_then(|x| x.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let jiffies = n(10) + n(11);
    // rss (24) vem em PAGINAS de 4 KiB.
    let rss_kb = n(20) * 4;
    (jiffies, rss_kb)
}

/// (bytes lidos, bytes escritos) que o processo mandou ao disco DE VERDADE.
///
/// `read_bytes` e `write_bytes` do `/proc/self/io` contam o que foi ao
/// dispositivo -- e nao o que passou pelo `read()`, que o cache do sistema
/// pode ter servido de graca. E a diferenca entre "leitura fisica" e
/// "leitura", e num painel de banco de dados so a primeira e noticia.
fn bytes_do_processo() -> (u64, u64) {
    let Ok(t) = std::fs::read_to_string("/proc/self/io") else {
        return (0, 0);
    };
    let campo = |nome: &str| -> u64 {
        t.lines()
            .find_map(|l| l.strip_prefix(nome))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    (campo("read_bytes:"), campo("write_bytes:"))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::Painel;

    /// Os limiares de fabrica, para os testes que nao sao sobre eles.
    fn de_fabrica() -> Painel {
        Painel::default()
    }

    fn nova_atividade() -> Arc<Atividade> {
        Arc::new(Atividade::nova(
            "dados:1".into(),
            "dados",
            "10.0.0.1".into(),
            1,
            1_000,
        ))
    }

    /// **O teste que faz o cancelamento valer alguma coisa.** Reponha o
    /// defeito -- um laco que nao chama `siga` -- e ele fica rodando para
    /// sempre depois de marcado; com a chamada, ele para.
    #[test]
    fn o_laco_que_confere_a_marca_para_e_o_que_nao_confere_nao() {
        let a = nova_atividade();
        a.comecou_pedido("checksum", "adm", "loja", "clientes", 2_000);
        let _fase = a.fase_cancelavel("varrendo");
        assert!(matches!(a.encerrar("root"), Encerramento::Marcada { .. }));

        // O laco CERTO: consulta a marca em ponto seguro e para.
        let mut linhas_certas = 0u64;
        let mut parou = false;
        for _ in 0..1_000 {
            if a.siga(1).is_err() {
                parou = true;
                break;
            }
            linhas_certas += 1;
        }
        assert!(parou, "o laco nao parou apesar da marca");
        assert_eq!(linhas_certas, 0, "parou tarde demais");

        // O laco ERRADO -- o defeito reposto -- nao consulta nada e roda
        // inteiro. E o que este teste existe para impedir de voltar.
        let mut linhas_erradas = 0u64;
        for _ in 0..1_000 {
            linhas_erradas += 1;
        }
        assert_eq!(linhas_erradas, 1_000);
    }

    /// A marca mira UMA operacao. Sem isso, encerrar uma conexao parada
    /// mataria o proximo pedido dela -- um que ninguem mandou matar.
    #[test]
    fn a_marca_nao_atravessa_para_o_pedido_seguinte() {
        let a = nova_atividade();
        a.comecou_pedido("varrer", "adm", "loja", "clientes", 2_000);
        let fase = a.fase_cancelavel("varrendo");
        a.encerrar("root");
        assert!(a.siga(1).is_err(), "a marca nao pegou o pedido mirado");
        drop(fase);
        a.terminou_pedido("adm");

        a.comecou_pedido("varrer", "adm", "loja", "clientes", 3_000);
        let _fase = a.fase_cancelavel("varrendo");
        assert!(
            a.siga(1).is_ok(),
            "a marca do pedido anterior matou o pedido seguinte"
        );
    }

    /// Encerrar uma atividade parada nao promete nada -- e diz isso.
    #[test]
    fn atividade_ociosa_nao_tem_o_que_encerrar() {
        let a = nova_atividade();
        assert!(matches!(a.encerrar("root"), Encerramento::Ociosa));
    }

    /// Fase nao cancelavel nao pode devolver «encerrando»: a tela mostraria
    /// «encerrando…» para sempre numa gravacao que vai terminar.
    #[test]
    fn fase_nao_cancelavel_se_declara() {
        let a = nova_atividade();
        a.comecou_pedido("inserir", "adm", "loja", "clientes", 2_000);
        match a.encerrar("root") {
            Encerramento::FaseNaoCancelavel { op } => assert_eq!(op, "inserir"),
            _ => panic!("prometeu encerrar uma gravacao no meio"),
        }
        assert_eq!(a.estado(), Estado::Executando, "mentiu «encerrando»");
    }

    /// A fase fecha sozinha ao sair do escopo: uma varredura que acabou nao
    /// pode continuar anunciada como cancelavel enquanto grava.
    #[test]
    fn a_fase_fecha_ao_sair_do_escopo() {
        let a = nova_atividade();
        a.comecou_pedido("exportar", "adm", "loja", "clientes", 2_000);
        {
            let _f = a.fase_cancelavel("varrendo");
            assert!(a.cancelavel.load(Ordering::Relaxed));
        }
        assert!(!a.cancelavel.load(Ordering::Relaxed));
    }

    /// **O portao vem antes do trabalho.** Desligada, nada entra em lugar
    /// nenhum -- nem atividade, nem contador, nem amostra.
    #[test]
    fn desligada_nao_registra_nada() {
        let t = Telemetria::nova(false);
        assert!(t.entrar("dados:1", "dados", "ip", 1, 0).is_none());
        t.contar_pedido(true, true);
        t.contar_espera(1_000);
        t.contar_trava(1_000);
        t.amostrar((1, 2), 0);
        assert!(t.atividades().is_empty());
        assert!(t.amostras(10).is_empty());
        let j = t.para_json(0, 10).escrever();
        assert!(j.contains("\"ligada\":false"), "{j}");
    }

    #[test]
    fn ligada_registra_e_ordena_pela_bolha_maior() {
        let t = Telemetria::nova(true);
        let a = t.entrar("dados:1", "dados", "ip", 1, 0).unwrap();
        let b = t.entrar("dados:2", "dados", "ip", 2, 0).unwrap();
        // A mesma chave devolve a MESMA atividade -- e o que da estabilidade
        // a bolha entre duas perguntas da tela.
        assert!(Arc::ptr_eq(
            &a,
            &t.entrar("dados:1", "dados", "ip", 1, 0).unwrap()
        ));
        a.consumido_ms.store(500, Ordering::Relaxed);
        b.consumido_ms.store(9_000, Ordering::Relaxed);
        let ordem: Vec<String> = t.atividades().iter().map(|x| x.chave.clone()).collect();
        assert_eq!(ordem, vec!["dados:2", "dados:1"], "nao ordenou por peso");
        t.sair("dados:1");
        assert_eq!(t.atividades().len(), 1);
    }

    /// A primeira amostra nao inventa taxa: sem instante anterior nao ha
    /// divisao possivel, e o acumulado desde o arranque nao e "agora".
    #[test]
    fn a_primeira_amostra_nao_tem_taxa() {
        let t = Telemetria::nova(true);
        t.contar_pedido(false, true);
        t.contar_pedido(false, true);
        t.amostrar((0, 0), 1_000);
        let a = t.amostras(10);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].leituras_s, 0.0);
        std::thread::sleep(std::time::Duration::from_millis(30));
        t.contar_pedido(false, true);
        t.amostrar((0, 0), 1_030);
        let a = t.amostras(10);
        assert_eq!(a.len(), 2);
        assert!(a[1].leituras_s > 0.0, "a segunda amostra devia ter taxa");
    }

    /// A serie nao cresce para sempre: um painel esquecido aberto num
    /// servidor movimentado nao pode comer a memoria da maquina.
    #[test]
    fn a_serie_esquece_a_mais_antiga() {
        let t = Telemetria::nova(true);
        for i in 0..AMOSTRAS_GUARDADAS + 50 {
            t.amostrar((0, 0), i as i64);
        }
        assert_eq!(t.amostras(10_000).len(), AMOSTRAS_GUARDADAS);
    }

    /// Toda thread registrada tem finalidade escrita -- e o registro guarda
    /// as de servico mesmo depois de morrerem.
    #[test]
    fn o_registro_de_threads_guarda_nome_e_finalidade() {
        let t = Arc::new(Telemetria::nova(true));
        let f = t.registrar_fio("vigia-de-disco", "confere o espaco livre", "servico", 0);
        f.fazendo("dormindo 15 min");
        let j = f.para_json(1_000).escrever();
        assert!(j.contains("vigia-de-disco"), "{j}");
        assert!(j.contains("confere o espaco livre"), "{j}");
        assert!(j.contains("dormindo 15 min"), "{j}");
        assert_eq!(t.fios().len(), 1);
    }

    /// `subir` roda o corpo e marca a ficha como morta no fim.
    #[test]
    fn a_thread_que_termina_deixa_de_ser_viva() {
        let t = Arc::new(Telemetria::nova(true));
        let (envia, recebe) = std::sync::mpsc::channel();
        t.subir("teste", "roda uma vez e sai", "servico", 0, move |f| {
            f.fazendo("trabalhando");
            let _ = envia.send(());
        });
        recebe.recv().unwrap();
        // A thread ainda pode nao ter chegado ao fim: espera pela ficha.
        for _ in 0..200 {
            if t.fios().iter().all(|f| !f.viva()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            t.fios().iter().all(|f| !f.viva()),
            "ficou viva depois de sair"
        );
    }

    /// **A lista de operacoes cancelaveis sai do CODIGO, e nao da memoria de
    /// quem a escreveu.**
    ///
    /// O teste le o fonte do servidor, acha toda funcao `op_*` que abre uma
    /// fase cancelavel, e exige que o nome dela esteja na lista. Sem isto, a
    /// proxima operacao que ganhar um ponto de cancelamento nasceria com o
    /// botao desabilitado na tela -- e ninguem descobriria por leitura, porque
    /// nada quebra: o botao simplesmente nao aparece.
    ///
    /// A direcao conferida e a que causa MENTIRA: op com ponto fora da lista.
    /// Apelido a mais na lista e inofensivo -- ele so faz o botao aparecer
    /// para o mesmo trabalho por outro nome.
    #[test]
    fn toda_operacao_com_ponto_de_cancelamento_esta_na_lista() {
        const FONTE: &str = include_str!("servidor.rs");
        let mut achadas = Vec::new();
        for pedaco in FONTE.split("    fn op_").skip(1) {
            let Some((nome, corpo)) = pedaco.split_once('(') else {
                continue;
            };
            // O corpo da funcao vai ate a proxima -- basta olhar o trecho.
            if corpo.contains("fase_cancelavel(") {
                achadas.push(nome.to_string());
            }
        }
        assert!(
            achadas.len() >= 4,
            "o leitor do fonte achou so {} operacoes com fase cancelavel: ele \
             quebrou, e um teste que nao ve nada passa por engano -- {achadas:?}",
            achadas.len()
        );
        for nome in &achadas {
            assert!(
                OPS_CANCELAVEIS.contains(&nome.as_str()),
                "op_{nome} abre uma fase cancelavel e NAO esta em \
                 OPS_CANCELAVEIS: a tela vai mostrar o botao desabilitado para \
                 uma operacao que da para encerrar"
            );
        }
        let mut vistos: Vec<&str> = Vec::new();
        for o in OPS_CANCELAVEIS {
            assert!(!vistos.contains(o), "{o:?} aparece duas vezes na lista");
            vistos.push(o);
        }
    }

    /// Marcar uma operacao que TEM ponto mas ainda nao chegou nele -- o caso
    /// da fila da trava de dados -- nao pode responder «vai terminar».
    ///
    /// Foi o que apareceu exercitando: uma soma de verificacao parada na fila
    /// respondeu «nao cancelavel» e, um instante depois, abortou. A resposta
    /// estava pessimista, que e o lado seguro de errar, mas errada.
    #[test]
    fn operacao_com_ponto_fora_da_fase_responde_marcada() {
        let a = nova_atividade();
        a.comecou_pedido("checksum", "adm", "loja", "clientes", 2_000);
        // Ainda nao entrou no laco: e o trecho em que ela espera a trava.
        match a.encerrar("root") {
            Encerramento::Posta { op } => assert_eq!(op, "checksum"),
            Encerramento::Marcada { .. } => panic!("prometeu demais: ela nao esta no laco"),
            _ => panic!("prometeu de menos: o checksum TEM ponto de cancelamento"),
        }
        // E a marca vale mesmo: quando o laco comeca, ela para na primeira.
        let _fase = a.fase_cancelavel("somando a tabela");
        assert!(a.siga(1).is_err(), "a marca posta antes do laco se perdeu");
    }

    /// Operacao sem ponto nenhum continua dizendo que vai terminar.
    #[test]
    fn operacao_sem_ponto_continua_nao_cancelavel() {
        let a = nova_atividade();
        a.comecou_pedido("inserir", "adm", "loja", "clientes", 2_000);
        assert!(matches!(
            a.encerrar("root"),
            Encerramento::FaseNaoCancelavel { .. }
        ));
    }

    /// **O vermelho tem de apontar UMA atividade.**
    ///
    /// Com o servidor apertado, a versao anterior pintava TODA atividade em
    /// curso de vermelho -- e o painel inteiro virava vermelho de uma vez, o
    /// que e o mesmo que nao ter cor. Quem esta na fila e vitima, nao culpado.
    #[test]
    fn sob_stress_so_quem_segura_a_trava_fica_vermelho() {
        let culpado = nova_atividade();
        culpado.comecou_pedido("checksum", "adm", "loja", "clientes", 0);
        culpado.com_a_trava();

        let vitima = Arc::new(Atividade::nova(
            "dados:2".into(),
            "dados",
            "10.0.0.2".into(),
            2,
            0,
        ));
        vitima.comecou_pedido("varrer", "adm", "loja", "clientes", 0);
        vitima.esperando_trava();

        let j = culpado.para_json(0, true, true, &de_fabrica()).escrever();
        assert!(
            j.contains("\"nivel\":\"stress\""),
            "o culpado nao ficou vermelho: {j}"
        );
        let j = vitima.para_json(0, true, true, &de_fabrica()).escrever();
        assert!(
            j.contains("\"nivel\":\"alto\""),
            "quem espera na fila e vitima, e nao culpado: {j}"
        );
        // E sem fila, uma consulta longa sozinha nao incomoda ninguem.
        let j = culpado.para_json(0, true, false, &de_fabrica()).escrever();
        assert!(
            !j.contains("\"nivel\":\"stress\""),
            "sem ninguem na fila, ela nao esta segurando nada: {j}"
        );
    }

    /// **O numero que decide a cor e o numero que a legenda escreve.**
    ///
    /// A mesma atividade, o mesmo instante: com o limiar de fabrica ela e
    /// `normal`, e com o limiar do `config.json` ela e `alto` -- e a resposta
    /// leva junto o limiar que decidiu, que e o que a legenda da tela escreve.
    /// Dois numeros para a mesma regra e como a tela acaba pintando o que o
    /// servidor nao concorda.
    #[test]
    fn o_limiar_configurado_decide_o_nivel_e_vai_na_legenda() {
        let t = Telemetria::nova(true);
        let a = t.entrar("dados:1", "dados", "10.0.0.1", 1, 0).unwrap();
        a.comecou_pedido("varrer", "adm", "loja", "clientes", 0);
        // O relogio da atividade e o de verdade (`Instant::elapsed`), entao a
        // unica forma honesta de passar de um limiar e esperar. 20 ms cobrem
        // com folga o limiar de 5 ms e ficam longe dos 2 s de fabrica.
        std::thread::sleep(std::time::Duration::from_millis(20));

        let j = a.para_json(0, false, false, &de_fabrica()).escrever();
        assert!(j.contains("\"nivel\":\"normal\""), "de fabrica: {j}");

        let apertado = Painel {
            alto_uso_ms: 5,
            ..Painel::default()
        };
        let j = a.para_json(0, false, false, &apertado).escrever();
        assert!(
            j.contains("\"nivel\":\"alto\""),
            "com o limiar do config: {j}"
        );

        t.definir_pintura(apertado);
        let r = t.para_json(0, 1).escrever();
        assert!(
            r.contains("\"limiares\":{\"alto_uso_ms\":5,\"stress_ms\":5000}"),
            "a legenda tem de escrever o limiar que decidiu: {r}"
        );
    }

    /// **O comportamento velho.** Sem cor configurada o retrato nao muda.
    #[test]
    fn sem_cor_configurada_nada_muda() {
        let t = Telemetria::nova(true);
        let r = t.para_json(0, 1).escrever();
        assert!(!r.contains("\"cores\""), "{r}");
        assert!(
            r.contains("\"limiares\":{\"alto_uso_ms\":2000,\"stress_ms\":5000}"),
            "{r}"
        );

        // E a cor escolhida entra sem levar junto a que ninguem escolheu.
        t.definir_pintura(Painel {
            cor_encerrando: "#7b2ff7".into(),
            ..Painel::default()
        });
        let r = t.para_json(0, 1).escrever();
        assert!(r.contains("\"cores\":{\"encerrando\":\"#7b2ff7\"}"), "{r}");
    }

    /// **Quem espera na fila nao engorda.**
    ///
    /// O defeito reposto e trocar `trabalhando_ha_ms` por `ha_ms` no peso: aí
    /// a atividade bloqueada cresce junto com a que a bloqueia, as duas
    /// bolhas ficam do mesmo tamanho, e o painel deixa de dizer qual das duas
    /// e o problema. Foi exatamente o que apareceu no navegador: oito bolhas
    /// identicas atras de uma soma de verificacao.
    #[test]
    fn quem_espera_na_fila_nao_ganha_peso() {
        let a = nova_atividade();
        a.comecou_pedido("varrer", "adm", "loja", "clientes", 0);
        a.esperando_trava();
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert_eq!(
            a.peso_ms(),
            0,
            "esperar a trava nao gasta servidor: o peso tem de ficar em zero"
        );
        // Mas o relogio de PAREDE anda: quem pediu esta esperando de verdade.
        assert!(
            a.ha_ms() >= 55,
            "o «ha quanto tempo» tem de contar a espera: {} ms",
            a.ha_ms()
        );

        // Com a trava na mao, o peso passa a andar.
        a.com_a_trava();
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(
            a.peso_ms() >= 55,
            "trabalhando, o peso tem de andar: {} ms",
            a.peso_ms()
        );
        assert!(a.trabalhando_ha_ms() >= 55);

        // E o acumulado sobrevive ao fim do pedido.
        a.terminou_pedido("adm");
        let guardado = a.peso_ms();
        assert!(guardado >= 55, "o peso nao ficou guardado: {guardado}");
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert_eq!(
            a.peso_ms(),
            guardado,
            "ociosa nao pode continuar engordando"
        );
    }

    /// A leitura do `/proc/self/stat` tem de pular o nome entre parenteses.
    #[cfg(target_os = "linux")]
    #[test]
    fn le_a_cpu_do_proprio_processo() {
        let (jiffies, kb) = cpu_e_memoria_do_processo();
        // Jiffies podem ser zero num processo recem-nascido; a memoria nao.
        let _ = jiffies;
        assert!(kb > 0, "memoria residente zero: o campo saiu do lugar");
    }
}
