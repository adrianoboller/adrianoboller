//! Replicacao, primeira parte: os lacos da replica, as filas e a aplicacao
//! dos lotes.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// De que origem vem UM nome de database, e quem ja ouviu que nao vem dele.
///
/// O dono e sempre a PRIMEIRA origem a reivindicar o nome -- as declaradas em
/// `replicacao.origens[].databases` reivindicam no arranque, na ordem do
/// arquivo, e as de lista vazia so quando a descoberta diz o que a origem tem.
/// Dai a regra: **lista declarada ganha de lista vazia**, porque quem escreveu
/// o nome disse o que queria e quem deixou vazio disse «o que vier», e entre
/// uma escolha e um curinga ganha a escolha.
///
/// `avisadas` existe para o recado sair UMA vez e nao a cada rodada: a recusa
/// e estavel (uma vez refem de outra origem, sempre), entao repeti-la a cada
/// laco so afogaria o log de quem tem dezenove bases certas.
#[derive(Default)]
pub(super) struct DonoDoDatabase {
    /// O nome da origem que ficou com este database.
    pub(super) origem: String,
    /// As origens que ja receberam a recusa deste database.
    pub(super) avisadas: std::collections::BTreeSet<String>,
}

/// O que a fase 3 de um alcance de tabela devolveu.
pub(super) enum Lote {
    /// Aplicou o lote (a posicao local andou, ou ja estava em outra).
    Aplicado,
    /// O diario do source nao continua o daqui -- o motivo, para o estado.
    Rompido(String),
}

/// Uma tabela no alcance de um database (pedido 676): o que o
/// [`crate::replica::Juntador`] nao sabe -- o nome, a posicao LOCAL e a
/// conferencia de continuidade.
pub(super) struct FilaDaReplica {
    pub(super) no: crate::replica::NoSource,
    pub(super) chave: String,
    /// Quantos eventos o diario local tem -- e de onde o proximo grupo tem
    /// de comecar. Relido com a trava antes de cada grupo.
    pub(super) posicao: u64,
    /// A tabela ja tinha eventos: o primeiro lote dela vem com o evento de
    /// conferencia na frente.
    pub(super) conferir: bool,
    /// O evento `posicao - 1` do source, ainda nao conferido com o daqui.
    pub(super) conferencia: Option<crate::replica::EventoRecebido>,
    pub(super) aplicados: u64,
    /// Em que vez a tabela entra num grupo: a mae antes da filha. Ver
    /// [`ordem_das_maes`].
    pub(super) ordem: usize,
}

/// A vez de cada tabela dentro de um grupo do alcance -- pedido 676.
///
/// O grupo aplica tabela por tabela sob uma tomada so: quem le a replica nao
/// ve a ordem, mas a contagem de orfas (pedido 300 §2.7) ve, e a filha
/// aplicada antes da mae que vem no MESMO grupo seria contada como orfa sem
/// nunca ter sido visivel como tal -- alarme falso. Entao a mae entra antes:
/// a vez de uma tabela e um a mais que a maior vez das maes dela que estao
/// no alcance. O ciclo (a tabela que aponta para si, ou duas que se apontam)
/// para no numero de tabelas, e ali a ordem volta a ser a da chegada.
pub(super) fn ordem_das_maes(filas: &[&crate::replica::NoSource]) -> Vec<usize> {
    let nome = |s: &str| phxsql_store::table::nome_simples(s).to_lowercase();
    let nomes: Vec<String> = filas.iter().map(|no| nome(&no.nome)).collect();
    let maes: Vec<Vec<usize>> = filas
        .iter()
        .enumerate()
        .map(|(i, no)| {
            no.esquema
                .as_ref()
                .map(|e| {
                    e.chaves_estrangeiras()
                        .iter()
                        .filter_map(|fk| nomes.iter().position(|n| *n == nome(&fk.tabela_ref)))
                        .filter(|&m| m != i)
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    let mut vez = vec![0usize; filas.len()];
    for _ in 0..filas.len() {
        let mut mudou = false;
        for i in 0..filas.len() {
            let v = maes[i]
                .iter()
                .map(|&m| vez[m] + 1)
                .max()
                .unwrap_or(0)
                .min(filas.len());
            if v != vez[i] {
                vez[i] = v;
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }
    vez
}

/// So em `debug`: o processo se mata por `SIGKILL` de verdade, sem rodar
/// destrutor nenhum -- as provas de queda dos pedidos 682, 698 e 699.
///
/// O sinal vem do proprio processo em vez de um `sleep` esperando o teste:
/// dormir com a trava global na mao subia a catraca `rede-ou-espera` (11 ->
/// 12), e catraca nao sobe. Sem o `kill`, `exit(137)`.
#[cfg(debug_assertions)]
pub(super) fn sigkill_de_teste(aviso: &str) -> ! {
    eprintln!("{aviso}");
    let _ = std::process::Command::new("kill")
        .args(["-KILL", &std::process::id().to_string()])
        .status();
    std::process::exit(137);
}

/// O que um grupo do alcance deu. Ver `Servidor::aplicar_grupo_da_replica`.
pub(super) enum Grupo {
    /// Aplicou `n` eventos; as tabelas em `rompidas` sairam da rodada.
    Aplicado { n: u64, rompidas: Vec<usize> },
    /// A posicao local de alguma tabela andou com a trava solta: nada se
    /// aplicou.
    Andou,
}

/// Quantas dicas de posicao do diario guardar por tabela.
///
/// Uma por replica que puxa dela, mais folga. Oito cobre a topologia que a
/// bancada monta (tres) com sobra, e o custo de cada uma e 20 bytes.
const MARCAS_POR_TABELA: usize = 8;

impl Servidor {
    /// Uma thread por origem, puxando os eventos do source.
    ///
    /// Uma por origem e nao uma so: multi-source e varias conexoes
    /// independentes, e uma origem lenta ou caida nao pode segurar as outras.
    pub(super) fn subir_replicacao(self: &Arc<Self>) {
        self.ligar_guarda_de_databases();
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
        let papel = self.config.replicacao.papel;
        if !papel.puxa_de_origem() {
            return;
        }
        if self.config.replicacao.origens.is_empty() {
            eprintln!(
                "replicacao: papel {} sem nenhuma origem em \
                 replicacao.origens -- nada a puxar",
                papel.nome()
            );
            return;
        }
        if papel != Papel::Multi && !self.config.somente_leitura {
            // Nao e erro, e e uma pedra no caminho conhecida: uma replica
            // escrita pela aplicacao quebra a numeracao dos rowids, e a
            // proxima inclusao vinda do source para a replicacao inteira.
            // O multi fica de fora: ele EXISTE para ser escrito, e casa as
            // linhas pela chave, nao pelo rowid.
            // Pedido 677: o database declarado numa origem ESPELHO ja recusa
            // escrita local (`origem_que_traz`), entao o aviso nomeia so o
            // que continua exposto -- a origem de lista vazia e a que nao
            // pediu `espelho`. Este laco e o irmao da guarda: dizer «so
            // leitura» de uma origem sem o campo mentiria sobre o portao.
            for o in &self.config.replicacao.origens {
                if !o.databases.is_empty() && !o.espelho {
                    eprintln!(
                        "ATENCAO: replica sem somente_leitura e a origem {} sem \
                         \"espelho\": true. Se a aplicacao escrever em {}, que \
                         vem dela, os rowids divergem e a replicacao para; \
                         escreva \"espelho\": true nesta origem para a escrita \
                         local neles ser recusada.",
                        o.nome,
                        o.databases.join(", ")
                    );
                } else if o.databases.is_empty() {
                    eprintln!(
                        "ATENCAO: replica sem somente_leitura e a origem {} sem \
                         databases declarados. Se a aplicacao escrever num \
                         database que vem dela, os rowids divergem e a \
                         replicacao para; declare-os em \
                         replicacao.origens[].databases com \"espelho\": true \
                         para a escrita local neles ser recusada.",
                        o.nome
                    );
                } else {
                    eprintln!(
                        "replicacao [{}]: {} so leitura aqui (vem por \
                         replicacao, um escritor por database)",
                        o.nome,
                        o.databases.join(", ")
                    );
                }
            }
        }
        for origem in self.config.replicacao.origens.clone() {
            if !origem.senha.is_empty() && origem.senha_hash.is_empty() {
                eprintln!(
                    "AVISO: origem {} com a SENHA EM TEXTO PURO no config.json. \
                     Troque por senha_hash: phxsqld --senha",
                    origem.nome
                );
            }
            let modo = if origem.cada_minutos > 0 {
                format!("agendada a cada {} min", origem.cada_minutos)
            } else if !origem.hora.is_empty() {
                format!("diaria as {}", origem.hora)
            } else {
                format!("streaming, laco de {}s", origem.reconectar_em)
            };
            eprintln!(
                "replicacao [{}]: puxando de {}:{} | {modo}",
                origem.nome, origem.host, origem.porta
            );
            self.anotar_estado(&origem.nome, |e| {
                e.modo = if origem.cada_minutos > 0 {
                    format!("cada_{}min", origem.cada_minutos)
                } else if !origem.hora.is_empty() {
                    format!("diaria_{}", origem.hora)
                } else {
                    "streaming".to_string()
                };
            });
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
        if origem.agendada() {
            return self.laco_agendado(origem);
        }
        let mut ritmo = crate::replica::Ritmo::novo(Duration::from_secs(origem.reconectar_em));
        loop {
            // A promocao encerra o laco: um primario nao puxa de ninguem.
            if !self.papel_atual().puxa_de_origem() {
                eprintln!(
                    "replicacao [{}]: laco encerrado, o servidor foi promovido",
                    origem.nome
                );
                return;
            }
            match self.uma_rodada(&origem) {
                // Nada a fazer: agora sim, espera antes de perguntar de novo.
                Ok(0) => {
                    ritmo.sucesso();
                    if !self.dormir_vigiando(&origem.nome, ritmo.base()) {
                        return;
                    }
                }
                Ok(n) => {
                    ritmo.sucesso();
                    eprintln!("replicacao [{}]: {n} evento(s) aplicado(s)", origem.nome);
                    // Sem sono: volta ja. `alcancar_tabela` recusa girar em
                    // falso -- ela erra se aplicar e a posicao nao andar --,
                    // entao um `Ok(n)` com n > 0 e progresso de verdade e este
                    // laco nao tem como virar giro em vazio.
                }
                // A falha decide o passo seguinte, e sao TRES passos -- ver
                // `replica::Ritmo`. Pedido 203: tratar a credencial recusada
                // como se fosse a origem caida, dormindo e voltando, e o que
                // bloqueava o IP no master em 4 s e derrubava o operador junto.
                Err((falha, e)) => {
                    eprintln!("replicacao [{}]: {e}", origem.nome);
                    if !self.apos_a_falha(&origem.nome, &mut ritmo, falha) {
                        return;
                    }
                }
            }
        }
    }

    /// O passo depois de uma rodada que falhou: dorme (vigiando o religar e a
    /// promocao) ou estaciona ate alguem religar. Devolve `false` quando o
    /// laco tem de encerrar, porque o servidor foi promovido no meio.
    ///
    /// Estacionado, o laco so volta por `replicacao_ligar` ou por reinicio --
    /// por tempo, nunca: um laco que voltasse sozinho depois de uma hora seria
    /// o mesmo defeito em camera lenta, gastando a tolerancia do bloqueio do
    /// master uma vez por hora.
    fn apos_a_falha(
        &self,
        origem: &str,
        ritmo: &mut crate::replica::Ritmo,
        falha: crate::replica::Falha,
    ) -> bool {
        self.apos_a_falha_vigiando(origem, ritmo, falha, &|| {
            self.papel_atual().puxa_de_origem()
        })
    }

    /// [`Self::apos_a_falha`] com a pergunta «ainda vale seguir esta origem?»
    /// vinda de fora -- pedido 587. O laco comum pergunta ao papel do
    /// `config.json`; o laco do CLUSTER pergunta se o master corrente ainda e o
    /// mesmo e se este no nao foi promovido. A decisao do recuo (o `Ritmo`) e a
    /// anotacao no `replicacao_estado` sao as MESMAS nos dois: era a decisao
    /// escrita duas vezes, e a copia do cluster tinha ficado sem recuo nenhum.
    /// `false` quando a pergunta disse nao no meio do sono.
    pub(super) fn apos_a_falha_vigiando(
        &self,
        origem: &str,
        ritmo: &mut crate::replica::Ritmo,
        falha: crate::replica::Falha,
        segue: &dyn Fn() -> bool,
    ) -> bool {
        match ritmo.apos(falha) {
            crate::replica::Decisao::Dormir(espera) => {
                let seguidas = ritmo.seguidas;
                self.anotar_estado(origem, |e| {
                    e.falhas_de_rede_seguidas = seguidas;
                    e.proxima_tentativa_ms = crate::agora_ms() + espera.as_millis() as i64;
                });
                let continua = self.dormir_vigiando_com(origem, espera, segue);
                self.anotar_estado(origem, |e| e.proxima_tentativa_ms = 0);
                continua
            }
            crate::replica::Decisao::Estacionar => {
                eprintln!(
                    "replicacao [{origem}]: credencial recusada pela origem; o laco PAROU \
                     de tentar -- corrija a configuracao e religue (replicacao_ligar). \
                     Cada tentativa a mais contaria contra este IP no bloqueio de la"
                );
                self.anotar_estado(origem, |e| {
                    e.parada = "credencial_recusada".to_string();
                    e.falhas_de_rede_seguidas = 0;
                    e.proxima_tentativa_ms = 0;
                });
                let continua = self.esperar_religar_com(origem, segue);
                if continua {
                    self.anotar_estado(origem, |e| {
                        e.parada.clear();
                        e.religadas += 1;
                    });
                    eprintln!("replicacao [{origem}]: religada, tentando de novo");
                }
                continua
            }
        }
    }

    /// Consome o pedido de religar desta origem, se houver.
    pub(super) fn tomar_religar(&self, origem: &str) -> bool {
        match self.estado_replicacao.lock() {
            Ok(mut estados) => match estados.get_mut(origem) {
                Some(e) if e.religar_pedido => {
                    e.religar_pedido = false;
                    true
                }
                _ => false,
            },
            Err(_) => false,
        }
    }

    /// Dorme `quanto` em passos de ate um segundo, acordando antes se alguem
    /// religou a origem -- «tente ja». Devolve `false` se o servidor foi
    /// promovido no meio: o laco tem de encerrar, e nao pode esperar um recuo
    /// inteiro para perceber.
    fn dormir_vigiando(&self, origem: &str, quanto: Duration) -> bool {
        self.dormir_vigiando_com(origem, quanto, &|| self.papel_atual().puxa_de_origem())
    }

    /// [`Self::dormir_vigiando`] com a pergunta de seguir vinda de fora. Os
    /// passos de um segundo sao o que impede o recuo de atrasar a troca de
    /// master no cluster: um sono de 60 s acorda em ate 1 s depois de a
    /// eleicao mudar o master, e nao no fim do recuo.
    fn dormir_vigiando_com(
        &self,
        origem: &str,
        quanto: Duration,
        segue: &dyn Fn() -> bool,
    ) -> bool {
        let fim = Instant::now() + quanto;
        loop {
            if !segue() {
                return false;
            }
            if self.tomar_religar(origem) {
                return true;
            }
            let resta = fim.saturating_duration_since(Instant::now());
            if resta.is_zero() {
                return true;
            }
            std::thread::sleep(resta.min(Duration::from_secs(1)));
        }
    }

    /// Espera, sem prazo, ate alguem religar a origem. `false` se o servidor
    /// foi promovido antes disso.
    fn esperar_religar_com(&self, origem: &str, segue: &dyn Fn() -> bool) -> bool {
        loop {
            if !segue() {
                return false;
            }
            if self.tomar_religar(origem) {
                return true;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// O laco AGENDADO: alcancar tudo, dormir ate a janela, repetir.
    ///
    /// Mora no proprio laco da replica, e nao no subsistema de jobs, de
    /// proposito: a agenda e da ORIGEM (cada uma pode ter a sua), e o job
    /// roda pedidos de protocolo -- a replica nao passa pelo protocolo.
    ///
    /// A primeira rodada acontece no arranque, sem esperar a janela: uma
    /// replica que sobe atrasada nao deve ficar horas fingindo que esta em
    /// dia. Dali em diante, so nas janelas.
    fn laco_agendado(self: Arc<Self>, origem: crate::config::Origem) {
        loop {
            if !self.papel_atual().puxa_de_origem() {
                eprintln!(
                    "replicacao [{}]: laco encerrado, o servidor foi promovido",
                    origem.nome
                );
                return;
            }
            // Alcancar TUDO: repete enquanto houver o que aplicar, porque a
            // proxima chance e so na janela seguinte.
            loop {
                match self.uma_rodada(&origem) {
                    Ok(0) => break,
                    Ok(n) => {
                        eprintln!(
                            "replicacao [{}]: {n} evento(s) aplicado(s) na janela",
                            origem.nome
                        )
                    }
                    // A credencial recusada estaciona aqui tambem: janela a
                    // cada minuto e uma tentativa por minuto, e cinco delas
                    // bloqueiam o IP no master do mesmo jeito (pedido 203).
                    // Religada, tenta de novo agora, sem esperar a janela.
                    Err((crate::replica::Falha::CredencialRecusada, e)) => {
                        eprintln!("replicacao [{}]: {e}", origem.nome);
                        let mut ritmo =
                            crate::replica::Ritmo::novo(Duration::from_secs(origem.reconectar_em));
                        if !self.apos_a_falha(
                            &origem.nome,
                            &mut ritmo,
                            crate::replica::Falha::CredencialRecusada,
                        ) {
                            return;
                        }
                    }
                    // O resto espera a janela seguinte, como sempre.
                    Err((_, e)) => {
                        eprintln!("replicacao [{}]: {e}", origem.nome);
                        break;
                    }
                }
            }
            let ms =
                bidirecional::ms_ate_a_janela(crate::agora_ms(), origem.cada_minutos, &origem.hora)
                    .max(1_000);
            self.anotar_estado(&origem.nome, |e| {
                e.proxima_janela_ms = crate::agora_ms() + ms;
            });
            // Dorme em passos de ate um segundo: a promocao nao pode esperar
            // uma janela diaria inteira para o laco perceber.
            let fim = Instant::now() + Duration::from_millis(ms as u64);
            loop {
                let resta = fim.saturating_duration_since(Instant::now());
                if resta.is_zero() {
                    break;
                }
                if !self.papel_atual().puxa_de_origem() {
                    return;
                }
                std::thread::sleep(resta.min(Duration::from_secs(1)));
            }
        }
    }

    /// Uma rodada, no modo do papel: por rowid (replica fiel) ou por chave
    /// (bidirecional). Anota o estado para `replicacao_estado` nos dois.
    fn uma_rodada(
        &self,
        origem: &crate::config::Origem,
    ) -> std::result::Result<u64, (crate::replica::Falha, PhxError)> {
        let resultado = self.rodada_classificada(origem);
        match &resultado {
            Ok(n) => {
                let n = *n;
                self.anotar_estado(&origem.nome, |e| {
                    e.ultima_rodada_ms = crate::agora_ms();
                    e.aplicados += n;
                    e.ultimo_erro.clear();
                    e.falhas_de_rede_seguidas = 0;
                    e.proxima_tentativa_ms = 0;
                });
            }
            Err((_, erro)) => {
                let texto = erro.to_string();
                self.anotar_estado(&origem.nome, |e| {
                    e.ultima_rodada_ms = crate::agora_ms();
                    e.ultimo_erro = texto;
                });
            }
        }
        resultado
    }

    /// Uma rodada com a falha CLASSIFICADA -- e classificada onde ela e
    /// conhecida: o erro de `ligar` e da fase em que a origem diz sim ou nao
    /// para quem chega (token, credencial, IP barrado); o de depois e da
    /// rodada. Sem esta separacao o laco teria de adivinhar pelo texto do
    /// erro, e texto se compara por chave, nunca por frase.
    pub(super) fn rodada_classificada(
        &self,
        origem: &crate::config::Origem,
    ) -> std::result::Result<u64, (crate::replica::Falha, PhxError)> {
        let cliente = crate::replica::ligar_classificado(origem)?;
        let r = if self.papel_atual() == Papel::Multi {
            self.rodada_bidirecional(cliente, origem)
        } else {
            self.rodada_da_replica(cliente, origem)
        };
        r.map_err(|e| (crate::replica::Falha::na_rodada(&e), e))
    }

    /// Uma passada por todas as tabelas de todos os databases da origem.
    ///
    /// Devolve quantos eventos aplicou.
    fn rodada_da_replica(
        &self,
        mut cliente: crate::replica::Cliente,
        origem: &crate::config::Origem,
    ) -> Result<u64> {
        // Quem puxa se apresenta (pedido 706): a origem que declarou este id
        // em `diario.consumidores` segura o diario ate ele confirmar.
        cliente.dizer_quem_puxa(&self.config.replicacao.id_servidor);
        // O que a origem ANUNCIA -- a lista declarada, ou o que a descoberta
        // trouxe --, menos os nomes que ja sao de outra origem. E aqui que o
        // servidor sabe, pela primeira vez e com certeza, que duas origens vao
        // entregar o mesmo database; ver `so_os_databases_desta_origem`.
        let anunciados = if origem.databases.is_empty() {
            cliente.databases()?
        } else {
            origem.databases.clone()
        };
        let databases = self.so_os_databases_desta_origem(&origem.nome, anunciados);
        // Pedido 300: e AQUI que o cluster aprende o que o master replica --
        // a mesma lista que este laco vai alcancar, e nao uma segunda opiniao
        // sobre ela. A posicao do arbitro passa a somar so isto.
        let do_cluster = self
            .cluster
            .as_deref()
            .filter(|_| origem.nome.starts_with(PREFIXO_DA_ORIGEM_DO_CLUSTER));
        if let Some(e) = do_cluster {
            e.anunciar_databases(&databases);
        }

        let mut aplicados = 0u64;
        for database in databases {
            let mut p = crate::replica::posicao(&mut cliente, &database)?;
            // Pedido 706: a origem ja expurgou o que esta replica precisava.
            // Ela se refaz pelo retrato da origem e segue dali -- a posicao
            // vem dentro do `.log` copiado.
            if self.precisa_de_retrato(&database, &p.bases)? {
                self.refazer_por_retrato(&mut cliente, &database, &origem.nome)?;
                p = crate::replica::posicao(&mut cliente, &database)?;
            }
            if let Some(e) = do_cluster {
                e.anunciar_tabelas(
                    &database,
                    p.tabelas.iter().map(|n| n.nome.clone()).collect(),
                );
            }
            if !p.com_imagem {
                return Err(PhxError::Esquema(format!(
                    "o source de {} esta com replicacao.imagem_da_linha desligada: \
                     o diario dele nao carrega a linha, e nao ha o que aplicar",
                    origem.nome
                )));
            }
            // A origem anterior ao 676 nao manda o id de transacao: tudo
            // chega com zero e vai evento a evento, como sempre foi. Dito UMA
            // vez por origem, porque a promessa da venda inteira nao vale ai.
            let mut avisar = false;
            self.anotar_estado(&origem.nome, |e| {
                avisar = !p.com_tx && !e.origem_sem_id_de_transacao;
                e.origem_sem_id_de_transacao = !p.com_tx;
            });
            if avisar {
                eprintln!(
                    "replicacao [{}]: a origem e anterior ao pedido 676 e nao manda o id \
                     de transacao: os eventos de {database} vao um a um, e uma venda de \
                     varias tabelas pode aparecer aqui pela metade enquanto chega. \
                     Atualize a origem para a mesma versao desta replica",
                    origem.nome
                );
            }
            aplicados +=
                self.alcancar_database(&mut cliente, &database, p.tabelas, &origem.nome)?;
        }
        Ok(aplicados)
    }

    /// Abre (criando se preciso) a tabela local e diz em que posicao ela esta.
    ///
    /// Toma a trava, faz o trabalho de disco e SOLTA -- e a fase 1 das tres em
    /// que [`Self::alcancar_tabela`] esta partida. `None` quer dizer "nao ha
    /// tabela aqui e o source nao mandou o esquema": nada a fazer.
    pub(super) fn abrir_para_replicar(
        &self,
        database: &str,
        no: &crate::replica::NoSource,
    ) -> Result<Option<(u64, Option<String>)>> {
        let trava = self.travar_dados()?;
        let (tabela, pendente) =
            garantir_tabela_da_replica(&trava, database, no, &self.ledger_marcado_recebido)?;
        let Some(mut tabela) = tabela else {
            drop(trava);
            pendente.levar_ao_disco()?;
            return Ok(None);
        };
        // O contador do source ANTES de tudo, e vale tambem para o lote do
        // quorum (que passa por aqui): o numero que ele ja entregou nao pode
        // voltar a sair desta ponta quando ela for promovida. Falhar em
        // adota-lo nao derruba a rodada -- a replica continua aplicando, e o
        // pior caso e o de sempre (o contador do que ela tem).
        if let Err(e) = tabela.adotar_sequencia_do_source(no.proxima_sequencia) {
            eprintln!(
                "replicacao: {database}.{}: o contador da sequencia do source nao \
                 foi adotado: {e}",
                no.nome
            );
        }
        let eventos = tabela.eventos()?;
        let outra_historia = recusa_da_linhagem(tabela.esquema(), database, no);
        // Pedido 589: a tabela fecha sob a trava, como sempre fechou; o
        // `fsync` do que nasceu aqui vai depois de solta-la.
        drop(tabela);
        drop(trava);
        pendente.levar_ao_disco()?;
        Ok(Some((eventos, outra_historia)))
    }

    /// Aplica UM lote ja lido do soquete. Fase 3: so trabalho no dado.
    ///
    /// Com `posicao > 0` o lote tem de comecar em `posicao - 1`: o primeiro
    /// evento e o que esta replica JA TEM, e ele e comparado com o daqui antes
    /// de os seguintes serem aplicados -- a conferencia de continuidade sem
    /// ida e volta a mais (pedido do papel C, 17/09/2026). Com `posicao == 0`
    /// nao ha o que comparar e o lote inteiro se aplica.
    ///
    /// Devolve quantos aplicou e a posicao LOCAL depois disso -- ou o motivo
    /// pelo qual o diario do source nao continua o daqui.
    fn aplicar_lote_da_replica(
        &self,
        database: &str,
        no: &crate::replica::NoSource,
        posicao: u64,
        eventos: &[crate::replica::EventoRecebido],
    ) -> Result<Lote> {
        let trava = self.travar_dados()?;
        let db = trava.abrir_database(database)?;
        // O diario DESTA replica tambem carrega a imagem quando configurado:
        // sem ela, a replica que puxa DESTA nao tem o que aplicar, e a cascata
        // Master -> Slave01 -> Slave02 morre no segundo salto. Quem liga e a
        // politica do diario, herdada do `Database` (pedido 564).
        let mut tabela = db.abrir_qualificada(&no.nome)?;

        // A posicao e RELIDA com a trava na mao. Entre a leitura do soquete e
        // este instante a trava esteve solta, e alguem pode ter escrito aqui;
        // aplicar um lote pedido a partir de outra posicao gravaria o evento
        // errado no rowid errado. Quando ela andou, o lote e descartado e o
        // laco pede de novo a partir de onde a tabela esta agora -- descartar
        // custa uma ida e volta, aplicar torto custaria o dado.
        let agora = tabela.eventos()?;
        if agora != posicao {
            return Ok(Lote::Aplicado);
        }
        let para_aplicar = if posicao == 0 {
            eventos
        } else {
            let Some(primeiro) = eventos.first() else {
                return Ok(Lote::Aplicado);
            };
            let chave = Self::chave_do_diario(database, &no.nome);
            if let Err(motivo) = self.diario_local_continua(&mut tabela, &chave, posicao, primeiro)
            {
                return Ok(Lote::Rompido(motivo));
            }
            &eventos[1..]
        };
        // Pedido 300 §2.7: a replica nao julga a chave estrangeira -- CONTA a
        // filha que entra sem a mae. Liga por lote, no handle deste lote; a
        // tabela sem chave conferida nem liga.
        tabela.contar_orfas();
        let mut aplicados = 0u64;
        let mut falhou = None;
        for e in para_aplicar {
            // O evento daqui nasce com o instante e a origem de LA, como o
            // PITR e o bidirecional ja faziam e a replica fiel nao fazia
            // (papel C, 17/09/2026, §2.3): o diario e trilha de auditoria, e
            // uma replica que jurasse que tudo aconteceu na hora em que ela
            // sincronizou lavaria o carimbo que o proximo salto -- um
            // bidirecional adiante, um backup tirado daqui -- precisa. E e o
            // que faz o diario daqui ser A MESMA HISTORIA, comparavel evento a
            // evento com o de la.
            tabela.forcar_proximo_evento(e.carimbo_ms, e.origem);
            if let Err(erro) = tabela.aplicar_evento(e.operacao, e.rowid, &e.imagem) {
                falhou = Some(erro);
                break;
            }
            aplicados += 1;
        }
        // As que entraram ANTES da falha ficam no disco e nao voltam: contadas
        // aqui, ou nunca.
        self.anotar_orfas(&format!("{database}/{}", no.nome), tabela.orfas_contadas());
        if let Some(erro) = falhou {
            return Err(erro);
        }
        // A posicao LOCAL, e nao `posicao + eventos.len()`: aplicar gera
        // eventos no diario daqui, e e por ele que a proxima rodada se
        // orienta. Contar do lado do source deixaria os dois numeros
        // andarem separados no primeiro evento que nao gerasse outro.
        let nova = tabela.eventos()?;
        if aplicados > 0 && nova <= posicao {
            // Aplicou e a posicao nao andou: o proximo pedido traria os
            // mesmos eventos, e o laco giraria em falso para sempre.
            return Err(PhxError::Corrompido(format!(
                "replicacao de {database}.{}: {aplicados} evento(s) aplicado(s) e a \
                 posicao continua em {posicao}",
                no.nome
            )));
        }
        // SEM `sincronizar` aqui, e isso foi medido. A versao anterior deste
        // conserto sincronizava a cada lote -- 400 `fsync` num alcance de
        // 200.000 eventos em vez de um -- e a bancada mostrou a conta: a vazao
        // caiu para 21.194 eventos/s e o pior `varrer` do cliente subiu para
        // 292 ms, com o `fsync` na mao da trava. A `Table` que sai de escopo
        // aqui leva as paginas sujas ao arquivo pelo `Drop` do `NdxFile`, que
        // e a mesma garantia de sempre contra queda do PROCESSO; a garantia
        // contra queda da MAQUINA vem do `sincronizar` unico no fim do
        // alcance, exatamente onde ela estava antes.
        Ok(Lote::Aplicado)
    }

    /// O diario LOCAL continua o do source?
    ///
    /// Irma da `diario_vivo_continua` do PITR (pedido 232), que compara os
    /// mesmos quatro campos -- carimbo, operacao, rowid e versao -- do evento
    /// de uma posicao nas duas copias. La o conserto entrou e aqui o irmao
    /// ficou: `alcancar_tabela` tinha um `>=` onde devia haver uma pergunta,
    /// e uma tabela apagada e recriada no source deixava a replica com a
    /// tabela velha, calada, para sempre (papel C, 17/09/2026, §2.6.1).
    ///
    /// `dele` e o evento `posicao - 1` do source. O daqui e lido com a marca
    /// do diario, porque o evento nao tem largura fixa e chegar ao N-esimo e
    /// caminhar pelos anteriores: sem a marca cada lote pagaria uma varredura
    /// do volume inteiro com a trava na mao.
    fn diario_local_continua(
        &self,
        tabela: &mut Table,
        chave: &str,
        posicao: u64,
        dele: &crate::replica::EventoRecebido,
    ) -> std::result::Result<(), String> {
        let ultimo = posicao - 1;
        tabela.definir_marca_do_diario(self.marca_do_diario_para(chave, ultimo));
        let meu = tabela.diario(ultimo, 1).map_err(|e| e.to_string())?;
        if let Some(nova) = tabela.marca_do_diario() {
            self.guardar_marca_do_diario(chave.to_string(), ultimo, nova);
        }
        let iso = phxsql_core::datahora::instante_iso;
        match meu.first() {
            Some(m)
                if m.carimbo == dele.carimbo_ms
                    && m.operacao == dele.operacao
                    && m.rowid == dele.rowid
                    && m.versao == dele.versao =>
            {
                Ok(())
            }
            Some(m) => Err(format!(
                "o evento {ultimo} do diario do source nao e o mesmo que esta \
                 replica tem ali (la: {} rowid {} versao {} em {}; aqui: {} rowid \
                 {} versao {} em {}): o diario de la nao continua o daqui -- {}. \
                 Esta tabela ficou como estava; para segui-la de novo, apague-a \
                 nesta replica e ela renasce do esquema do source",
                dele.operacao.nome(),
                dele.rowid,
                dele.versao,
                iso(dele.carimbo_ms),
                m.operacao.nome(),
                m.rowid,
                m.versao,
                iso(m.carimbo),
                self.por_que_nao_continua(chave, ultimo),
            )),
            None => Err(format!(
                "esta replica conta {posicao} evento(s) e o diario dela nao \
                 entrega o evento {ultimo}: nao ha como saber se o diario do \
                 source continua o daqui. Esta tabela ficou como estava"
            )),
        }
    }

    /// A causa que a ruptura de continuidade nomeia -- pedido 300 (4).
    ///
    /// Sao DUAS causas com o mesmo sintoma, e o sintoma nao as separa: a
    /// tabela apagada e recriada no source, e a escrita local nesta replica,
    /// que tomou o lugar do evento do source. Quando esta replica ACEITOU
    /// escrita local na tabela, a causa e essa, e a frase diz quantas e o
    /// conserto. Sem o numero (outro processo aceitou, ou a tabela veio de
    /// SQL sem nome de tabela no pedido) e sem `somente_leitura`, as duas
    /// ficam NOMEADAS -- escolher uma seria mandar procurar no lugar errado,
    /// a licao do pedido 473. Com `somente_leitura` a escrita local nao passa
    /// do portao, e sobra a de sempre.
    pub(super) fn por_que_nao_continua(&self, chave: &str, ultimo: u64) -> String {
        let locais = self
            .escritas_locais_na_replica
            .lock()
            .ok()
            .and_then(|m| m.get(chave).copied())
            .unwrap_or(0);
        if locais > 0 {
            format!(
                "esta replica ACEITOU {locais} pedido(s) de escrita LOCAL nesta \
                 tabela desde o arranque (sem somente_leitura), e a escrita local \
                 tomou o lugar do evento {ultimo} do source no diario daqui. Ligue \
                 somente_leitura nesta replica"
            )
        } else if self.somente_leitura() {
            "a tabela foi apagada e recriada no source".to_string()
        } else {
            "a tabela foi apagada e recriada no source, ou esta replica (sem \
             somente_leitura) foi escrita localmente e a escrita tomou o lugar de \
             um evento do source"
                .to_string()
        }
    }

    /// A chave com que o diario local de uma tabela e lembrado -- pelas
    /// marcas de leitura e pela conferencia de continuidade.
    pub(super) fn chave_do_diario(database: &str, tabela: &str) -> String {
        format!("{}/{}", database.to_lowercase(), tabela.to_lowercase())
    }

    /// A marca do diario de `chave` que ainda serve para ler a partir de
    /// `desde` -- a maior que nao passa dele. Ver `marcas_do_diario`.
    pub(super) fn marca_do_diario_para(
        &self,
        chave: &str,
        desde: u64,
    ) -> Option<phxsql_store::log::MarcaDoDiario> {
        let m = self.marcas_do_diario.lock().ok()?;
        m.get(chave)
            .and_then(|v| {
                v.iter()
                    .filter(|k| k.evento <= desde)
                    .max_by_key(|k| k.evento)
            })
            .copied()
    }

    /// Guarda onde uma leitura a partir de `desde` parou. A que acabou de ser
    /// usada sai: quem le em sequencia nao volta atras. Teto pequeno: sao
    /// dicas, e a mais antiga e a menos util.
    pub(super) fn guardar_marca_do_diario(
        &self,
        chave: String,
        desde: u64,
        nova: phxsql_store::log::MarcaDoDiario,
    ) {
        if let Ok(mut m) = self.marcas_do_diario.lock() {
            let v = m.entry(chave).or_default();
            v.retain(|k| k.evento != desde && k.evento != nova.evento);
            v.push(nova);
            if v.len() > MARCAS_POR_TABELA {
                v.sort_unstable_by_key(|k| k.evento);
                v.remove(0);
            }
        }
    }

    /// Esquece o que se lembrava do diario de uma tabela -- as marcas de
    /// leitura e a conferencia de continuidade. E o que uma tabela apagada
    /// exige: uma homonima que nasca depois tem OUTRO diario, e uma marca da
    /// antiga apontaria para o meio de um evento da nova.
    pub(super) fn esquecer_diario(&self, database: &str, tabela: &str) {
        let chave = Self::chave_do_diario(database, tabela);
        if let Ok(mut m) = self.marcas_do_diario.lock() {
            m.remove(&chave);
        }
        if let Ok(mut c) = self.continuidade_da_replica.lock() {
            c.remove(&chave);
        }
    }

    /// O veredito guardado da conferencia de continuidade de uma tabela, se
    /// ele e desta mesma posicao local.
    fn continuidade_guardada(&self, chave: &str, posicao: u64) -> Option<bool> {
        self.continuidade_da_replica
            .lock()
            .ok()?
            .get(chave)
            .filter(|(p, _)| *p == posicao)
            .map(|(_, ok)| *ok)
    }

    /// O diario do source continua o daqui ate `posicao`: a tabela segue, e
    /// um recado antigo de recusa sai -- recado que sobrevive ao conserto vira
    /// configuracao que mente.
    fn confirmar_continuidade(&self, origem: &str, chave: &str, posicao: u64) {
        if let Ok(mut c) = self.continuidade_da_replica.lock() {
            c.insert(chave.to_string(), (posicao, true));
        }
        self.anotar_estado(origem, |e| {
            e.recusas.remove(chave);
        });
    }

    /// O diario do source NAO continua o daqui: a tabela para de ser seguida,
    /// com o motivo em `replicacao_estado` e no log do processo -- UMA vez,
    /// porque a recusa fica guardada por posicao e a rodada seguinte nao a
    /// repete. Nao e erro da rodada: as outras tabelas continuam.
    fn romper_continuidade(&self, origem: &str, chave: &str, posicao: u64, motivo: String) {
        eprintln!("replicacao [{origem}]: {chave}: {motivo}");
        if let Ok(mut c) = self.continuidade_da_replica.lock() {
            c.insert(chave.to_string(), (posicao, false));
        }
        self.anotar_estado(origem, |e| {
            e.recusas.insert(chave.to_string(), motivo);
        });
    }

    /// Liga (ou nao) a guarda dos nomes de database entre origens.
    ///
    /// **O portao que decide vem antes do trabalho**, e a decisao mora num
    /// lugar so por isso: as tres condicoes sao a MESMA pergunta -- «este
    /// servidor vai ter duas threads puxando em paralelo?» --, e espalha-las
    /// pelo `subir_replicacao` deixaria a condicao que alguem esquecesse
    /// virando guarda ligada onde ela morde o caso certo.
    ///
    /// Desligada, [`Servidor::so_os_databases_desta_origem`] custa uma leitura
    /// atomica e devolve a lista inteira -- nem mapa, nem mutex.
    ///
    /// - **`cluster`**: ali quem puxa e um laco so, do master CORRENTE, e o
    ///   nome da origem muda a cada eleicao (`cluster:<id>`). Um dono guardado
    ///   por nome recusaria ao master novo o database do master velho, que e a
    ///   promocao inteira parando.
    /// - **papel que nao puxa**: nao ha laco nenhum.
    /// - **uma origem so**: e o par 1<->1, e ele nao cruza com ninguem.
    ///
    /// E a pergunta esta escrita UMA vez, em `Config::puxa_de_varias_origens`,
    /// porque o nivel ESTATICO da mesma guarda (o `Config::validar`) tem de
    /// desligar no mesmo caso: duas copias da condicao viraram, no primeiro
    /// corte do 406, um nivel ligado onde o outro estava desligado -- o no de
    /// cluster com origens sobrando deixou de subir.
    pub(super) fn ligar_guarda_de_databases(&self) {
        if !self.config.puxa_de_varias_origens() {
            return;
        }
        self.ha_varias_origens.store(true, Ordering::Relaxed);
        self.reivindicar_databases_declarados();
    }

    /// Reivindica para esta origem os databases que ela declarou no arquivo.
    ///
    /// Roda UMA vez, antes de qualquer thread subir, na ordem em que as
    /// origens estao no `config.json`. E o que torna o vencedor previsivel:
    /// sem isto, entre uma origem que declarou `["vendas"]` e outra de lista
    /// vazia cujo source tambem tem `vendas`, ganharia a thread que chegasse
    /// primeiro -- e a cada arranque poderia ser outra. O database local
    /// receberia linhas de um source num dia e de outro no dia seguinte, que e
    /// exatamente a divergencia de rowid que esta guarda existe para impedir.
    fn reivindicar_databases_declarados(&self) {
        let Ok(mut donos) = self.dono_do_database.lock() else {
            return;
        };
        for origem in &self.config.replicacao.origens {
            for db in &origem.databases {
                donos.entry(db.clone()).or_insert_with(|| DonoDoDatabase {
                    origem: origem.nome.clone(),
                    avisadas: Default::default(),
                });
            }
        }
    }

    /// Dos databases que esta origem anuncia, os que ela pode mesmo replicar.
    ///
    /// # Por que filtrar em vez de recusar a rodada
    ///
    /// Duas origens entregando o mesmo nome de database escrevem no mesmo
    /// `.reg` daqui, e a replica fiel aplica por ROWID: os rowids divergem, a
    /// inclusao fail-stopa e a alteracao sobrescreve CALADA. Mas o remedio nao
    /// pode ser derrubar a rodada -- um nome repetido nao tira do ar as
    /// dezenove bases certas da mesma origem. Entao a recusa e daquele
    /// database, nominal, e o resto anda.
    ///
    /// # Por que ele custa zero em quem nao tem duas origens
    ///
    /// A bandeira vem ANTES do `lock`: num par 1<->1 (e num cluster) isto e
    /// uma leitura atomica e a lista volta inteira, sem mapa e sem mutex.
    ///
    /// # De onde vem a identidade, e quem a garante
    ///
    /// A identidade da origem aqui e o NOME, que e a mesma identidade que
    /// `replicacao_estado`, `replicacao_pular` e `replicacao_ligar` usam. Duas
    /// origens com o mesmo nome se confundiriam nas quatro -- aqui a segunda
    /// se veria como dona (`dono.origem == origem`) e a guarda inteira
    /// passaria batido, sem malicia nenhuma: `"nome"` e opcional e o padrao e
    /// `"origem"`, entao bastava omitir o campo duas vezes. Quem garante a
    /// identidade e o `Config::recusar_nome_de_origem_repetido`, na
    /// DECLARACAO: um remendo aqui nao consertaria as outras tres.
    pub(super) fn so_os_databases_desta_origem(
        &self,
        origem: &str,
        anunciados: Vec<String>,
    ) -> Vec<String> {
        if !self.ha_varias_origens.load(Ordering::Relaxed) {
            return anunciados;
        }
        // O que recusar sai com a trava na mao; o recado sai DEPOIS de solta,
        // para nunca haver dois mutexes de replicacao empilhados.
        let mut recusados: Vec<(String, String)> = Vec::new();
        let meus = match self.dono_do_database.lock() {
            Ok(mut donos) => anunciados
                .into_iter()
                .filter(|db| {
                    let dono = donos.entry(db.clone()).or_insert_with(|| DonoDoDatabase {
                        origem: origem.to_string(),
                        avisadas: Default::default(),
                    });
                    if dono.origem == origem {
                        return true;
                    }
                    if dono.avisadas.insert(origem.to_string()) {
                        recusados.push((db.clone(), dono.origem.clone()));
                    }
                    false
                })
                .collect(),
            // Envenenado so por panico de fora: sob esta trava so ha `clone`
            // e `entry`, que nao entram em panico. O braco existe porque
            // `lock` devolve `Result`, e ele segue SEM filtrar -- a mesma
            // escolha do `esta_parada` logo acima, e pela mesma razao: derrubar
            // a replicacao inteira por um panico alheio tem alcance maior do
            // que o arranjo errado que esta guarda recusa, e esse arranjo o
            // `validar()` ja recusou no arranque quando deu para decidir.
            Err(_) => return anunciados,
        };
        for (db, dono) in recusados {
            let motivo = format!(
                "o database {db:?} ja e replicado da origem {dono:?}. Duas \
                 origens escrevendo no mesmo {db} deste servidor fazem os \
                 rowids divergirem: a inclusao para com fail-stop e a \
                 alteracao sobrescreve calada. As outras bases desta origem \
                 continuam. Use em cada origem um nome de database so dela, \
                 ou escreva replicacao.origens[].databases dizendo qual origem \
                 traz qual -- ver docs/REPLICACAO.md"
            );
            eprintln!("replicacao [{origem}]: {motivo}");
            self.anotar_estado(origem, |e| {
                e.recusas.insert(db, motivo);
            });
        }
        meus
    }

    /// Leva ao disco o que o alcance aplicou. Uma vez por alcance, com a trava.
    pub(super) fn sincronizar_replicada(&self, database: &str, tabela: &str) -> Result<()> {
        self.sincronizar_replicada_contando(database, tabela)
            .map(|_| ())
    }

    /// O mesmo, dizendo quantos arquivos foram ao disco -- o lote do quorum
    /// (pedido 207) conta isso antes de confirmar.
    pub(super) fn sincronizar_replicada_contando(
        &self,
        database: &str,
        tabela: &str,
    ) -> Result<u64> {
        let trava = self.travar_dados()?;
        let db = trava.abrir_database(database)?;
        let mut t = db.abrir_qualificada(tabela)?;
        t.sincronizar()?;
        Ok(t.arquivos_sincronizados())
    }

    /// Prepara UMA tabela para o alcance do database: abre (criando), confere
    /// a continuidade quando nao ha nada a aplicar, e diz de que posicao
    /// local puxar -- `None` quando nao ha o que puxar dela nesta rodada.
    ///
    /// Ate o pedido 676 esta funcao era `alcancar_tabela` e trazia a tabela
    /// INTEIRA, lote a lote, cada lote sob a propria tomada da trava: um
    /// commit de varias tabelas aparecia na replica tabela por tabela, e a
    /// venda ficava sem os itens ate a tabela deles chegar -- ou para sempre,
    /// se o fio caisse no meio. O laco que puxa e aplica mudou para
    /// [`Self::alcancar_database`], que junta as tabelas pelo id de
    /// transacao; o que ficou aqui e o que e de cada tabela.
    ///
    /// # Por que isto esta partido em tres fases
    ///
    /// Ate a 0.18 esta funcao tomava a trava de dados na primeira linha e a
    /// segurava ate o fim -- e no meio dela mora `replica::puxar`, que e uma
    /// IDA E VOLTA DE REDE. Numa rede sa isso e invisivel; num corte
    /// silencioso a leitura fica pendurada ate o prazo de 30 s do cliente e a
    /// trava vai junto. Medido na bancada (`bancada/replicacao/trava.py`): com
    /// o tubo emudecido, `ping` na replica respondia em 4 ms e `varrer` --
    /// que precisa da trava -- em 30.079 ms, e a propria telemetria da replica
    /// contou 35,8 s de trava na mao numa janela de 40 s.
    ///
    /// As tres fases sao: abrir e ler a posicao COM a trava, ler o lote do
    /// soquete SEM ela, aplicar COM ela de novo. A regra que sai daqui e
    /// geral: *nenhuma leitura de rede acontece com a trava de dados na mao*.
    ///
    /// # A continuidade, que o `>=` escondia
    ///
    /// Havia aqui `if posicao >= no.eventos { return Ok(0) }`, e esse silencio
    /// era o achado de maior retorno do papel C em 17/09/2026 (§2.6.1): o
    /// `excluir_tabela` no source leva o `.log` junto, o diario de la volta a
    /// zero, e a replica -- com N eventos de outra vida -- nao fazia nada, para
    /// sempre, sem reclamar. O PITR ja tinha a pergunta certa
    /// (`diario_vivo_continua`) e o irmao ficou. Agora a pergunta e feita em
    /// tres formas, e nenhuma custa uma ida e volta a mais no caminho quente:
    ///
    /// 1. o source tem MENOS eventos que esta replica: rompido, sem rede;
    /// 2. a replica tem algo a aplicar: o lote comeca em `posicao - 1`, e a
    ///    fase 3 compara o primeiro evento com o daqui antes de aplicar o
    ///    resto -- o lote so fica um evento mais longo;
    /// 3. nada a aplicar (`posicao == no.eventos`): um evento pela rede, UMA
    ///    vez por posicao -- o veredito fica guardado e a tabela ociosa nao
    ///    paga isso a cada rodada.
    ///
    /// Rompida, a tabela sai da rodada com o motivo em `replicacao_estado`, e
    /// as OUTRAS continuam: uma tabela recriada no source nao pode parar a
    /// replicacao das que estao sas. Ela volta a ser seguida quando a posicao
    /// local mudar -- o operador a apaga aqui, ela renasce do esquema do
    /// source, e o `posicao == 0` nao tem o que comparar.
    fn preparar_para_alcancar(
        &self,
        cliente: &mut crate::replica::Cliente,
        database: &str,
        no: &crate::replica::NoSource,
        origem: &str,
    ) -> Result<Option<u64>> {
        let Some((posicao, outra_historia)) = self.abrir_para_replicar(database, no)? else {
            return Ok(None);
        };
        let chave = Self::chave_do_diario(database, &no.nome);
        // Pedido 601: de OUTRA historia, nada se aplica -- e a recusa vai pelo
        // MESMO canal da tabela apagada e recriada, que e um dos dois casos
        // que ela pega (o outro e a tabela criada por conta aqui). Antes de
        // qualquer evento, inclusive com a posicao em zero: a insercao e o
        // que a conferencia do carimbo nunca pega.
        if let Some(motivo) = outra_historia {
            self.romper_continuidade(origem, &chave, posicao, motivo);
            return Ok(None);
        }
        if posicao > 0 {
            if self.continuidade_guardada(&chave, posicao) == Some(false) {
                return Ok(None);
            }
            if no.eventos < posicao {
                // A causa sai do MESMO motor da conferencia evento a evento
                // (pedido 626): este ramo fixava «apagada e recriada no
                // source», e a escrita local aceita aqui chega nele sempre que
                // a rodada cai entre ela e a escrita seguinte do source -- a
                // replica culpava o source pelo que foi escrito nela. O evento
                // que a vida daqui tomou e o primeiro que o source ainda nao
                // tem, `no.eventos`.
                self.romper_continuidade(
                    origem,
                    &chave,
                    posicao,
                    format!(
                        "o diario do source tem {} evento(s) e esta replica tem \
                         {posicao}: ele nao continua o daqui -- {}. Esta tabela \
                         ficou como estava; para segui-la de novo, apague-a nesta \
                         replica e ela renasce do esquema do source",
                        no.eventos,
                        self.por_que_nao_continua(&chave, no.eventos),
                    ),
                );
                return Ok(None);
            }
        }
        if posicao >= no.eventos {
            if posicao == 0 || self.continuidade_guardada(&chave, posicao) == Some(true) {
                return Ok(None);
            }
            // FORA da trava, como todo `puxar`: um evento, o que ja esta aqui.
            let Some(dele) = crate::replica::puxar_um(cliente, database, &no.nome, posicao - 1)?
            else {
                // O source contou `posicao` eventos e nao entrega o ultimo:
                // corrida com uma exclusao de tabela la. A proxima rodada ve
                // a contagem nova e decide.
                return Ok(None);
            };
            match self.aplicar_lote_da_replica(
                database,
                no,
                posicao,
                std::slice::from_ref(&dele),
            )? {
                Lote::Rompido(motivo) => self.romper_continuidade(origem, &chave, posicao, motivo),
                Lote::Aplicado => self.confirmar_continuidade(origem, &chave, posicao),
            }
            return Ok(None);
        }
        Ok(Some(posicao))
    }

    /// Traz um DATABASE inteiro ate a fronteira do `posicao`, aplicando cada
    /// transacao da origem inteira ou nada -- pedido 676.
    ///
    /// # O desenho
    ///
    /// 1. cada tabela se prepara como antes ([`Self::preparar_para_alcancar`]):
    ///    abre, cria pelo esquema, confere a continuidade;
    /// 2. o [`crate::replica::Juntador`] diz o que puxar e o que aplicar. Os
    ///    lotes chegam FORA da trava, e uma transacao so vai ao disco quando
    ///    esta inteira na mao, em todas as tabelas que ela tocou;
    /// 3. o grupo vai sob UMA tomada da trava de escrita
    ///    ([`Self::aplicar_grupo_da_replica`]): nenhum leitor da replica ve o
    ///    meio dele. Transacoes inteiras seguidas dividem a tomada, ate o
    ///    tamanho de um lote -- senao o alcance de um diario de autocommits
    ///    pagaria uma tomada por evento.
    ///
    /// # O fio que cai no meio
    ///
    /// O erro do `puxar` sai da rodada com o que esta na mao descartado: nada
    /// daquilo foi gravado, a posicao local de cada tabela continua no comeco
    /// da transacao que estava chegando, e a proxima rodada a pede de novo
    /// desde o comeco. O que ja tinha ido ao disco eram transacoes inteiras,
    /// e por isso o `fsync` do fim acontece tambem na saida por erro.
    fn alcancar_database(
        &self,
        cliente: &mut crate::replica::Cliente,
        database: &str,
        nos: Vec<crate::replica::NoSource>,
        origem: &str,
    ) -> Result<u64> {
        let mut filas: Vec<FilaDaReplica> = Vec::new();
        for no in nos {
            if let Some(posicao) = self.preparar_para_alcancar(cliente, database, &no, origem)? {
                filas.push(FilaDaReplica {
                    chave: Self::chave_do_diario(database, &no.nome),
                    conferir: posicao > 0,
                    conferencia: None,
                    posicao,
                    aplicados: 0,
                    ordem: 0,
                    no,
                });
            }
        }
        if filas.is_empty() {
            return Ok(0);
        }
        let vezes = ordem_das_maes(&filas.iter().map(|f| &f.no).collect::<Vec<_>>());
        for (f, vez) in filas.iter_mut().zip(vezes) {
            f.ordem = vez;
        }
        let alvos: Vec<(u64, u64)> = filas.iter().map(|f| (f.posicao, f.no.eventos)).collect();
        let mut juntador =
            crate::replica::Juntador::novo(&alvos, phxsql_store::log::teto_da_transacao());
        let mut aplicados = 0u64;
        let mut avisadas = 0u64;
        let mut marcas: Vec<PathBuf> = Vec::new();
        let saida = loop {
            match juntador.passo() {
                crate::replica::Passo::Puxar { fila, desde } => {
                    let f = &mut filas[fila];
                    // O primeiro lote de uma tabela que ja tem eventos comeca
                    // UM antes: o evento que esta replica ja tem, conferido
                    // com o daqui antes de qualquer aplicacao (pedido do
                    // papel C, 17/09/2026) -- o mesmo de sempre.
                    let pedir = if f.conferir && f.conferencia.is_none() {
                        desde.saturating_sub(1)
                    } else {
                        desde
                    };
                    // O que ja foi ao disco aqui: a posicao do comeco da
                    // rodada -- o que entrou nela so vai no `fsync` do fim.
                    cliente.dizer_o_duravel(f.posicao.saturating_sub(f.aplicados));
                    let mut eventos =
                        match crate::replica::puxar(cliente, database, &f.no.nome, pedir) {
                            Ok(e) => e,
                            Err(e) => break Err(e),
                        };
                    if pedir < desde && !eventos.is_empty() {
                        f.conferencia = Some(eventos.remove(0));
                        if eventos.is_empty() {
                            // So o de conferencia coube na resposta (o teto
                            // de bytes do source deixa o primeiro passar
                            // sempre): o resto se pede no passo seguinte, ja
                            // sem ele. Entregar a lista vazia ao `Juntador`
                            // tiraria a tabela da rodada como se o source
                            // tivesse encolhido.
                            continue;
                        }
                    }
                    juntador.receber(fila, eventos);
                }
                crate::replica::Passo::Aplicar { grupo, inteiro } => {
                    if !inteiro && juntador.em_pedacos > avisadas {
                        avisadas = juntador.em_pedacos;
                        eprintln!(
                            "replicacao [{origem}]: {database}: uma transacao da origem \
                             passou do teto de {} MiB na memoria desta replica e vai em \
                             PEDACOS -- ate o ultimo chegar, um leitor daqui pode ve-la \
                             pela metade (pedido 676; contada em \
                             replicacao_estado.transacoes_em_pedacos). Desde o pedido \
                             685 a origem recusa o COMMIT acima do teto: o que chega \
                             partido e de uma origem anterior ou de uma escrita fora \
                             de transacao",
                            phxsql_store::log::teto_da_transacao() / (1024 * 1024)
                        );
                    }
                    match self.aplicar_grupo_da_replica(
                        database,
                        &mut filas,
                        grupo,
                        origem,
                        &mut marcas,
                    ) {
                        Ok(Grupo::Aplicado { n, rompidas }) => {
                            aplicados += n;
                            for i in rompidas {
                                juntador.largar(i);
                            }
                        }
                        // A posicao local andou com a trava solta: o que esta
                        // na mao foi pedido de outra posicao. A rodada sai, e
                        // a proxima recomeca de onde a replica esta.
                        Ok(Grupo::Andou) => break Ok(()),
                        Err(e) => break Err(e),
                    }
                }
                crate::replica::Passo::Fim => break Ok(()),
            }
        };
        if juntador.em_pedacos > 0 {
            let partidas = juntador.em_pedacos;
            self.anotar_estado(origem, |e| e.transacoes_em_pedacos += partidas);
        }
        for f in &filas {
            if f.aplicados > 0 {
                self.sincronizar_replicada(database, &f.no.nome)?;
            }
        }
        // Pedido 706: agora o que entrou nesta rodada esta no disco, e a
        // origem que segura o diario por esta replica pode saber. Um evento
        // pedido de onde a tabela parou, com o `duravel` igual -- so quando a
        // rodada aplicou algo, e sem erro de rede derrubando a rodada: a
        // confirmacao que se perde aqui chega na rodada seguinte.
        for f in filas.iter().filter(|_| saida.is_ok()) {
            if f.aplicados > 0 {
                cliente.dizer_o_duravel(f.posicao);
                let _ = crate::replica::puxar_um(cliente, database, &f.no.nome, f.posicao);
            }
        }
        Self::soltar_marcas_da_replica(marcas);
        saida.map(|()| aplicados)
    }

    /// Aplica um grupo do [`crate::replica::Juntador`] -- eventos de uma ou
    /// mais transacoes INTEIRAS, em varias tabelas -- sob UMA tomada da trava
    /// de escrita. Pedido 676.
    ///
    /// Tudo o que pode recusar o grupo se confere ANTES do primeiro evento: a
    /// posicao local de cada tabela (alguem escreveu aqui com a trava solta?)
    /// e a continuidade da que ainda nao foi conferida. A tabela rompida sai
    /// do grupo e da rodada -- ela ja parou de ser seguida, e as outras
    /// continuam, como sempre foi.
    ///
    /// # A queda do PROCESSO no meio -- pedido 682
    ///
    /// A tomada unica da trava protege o LEITOR vivo; o processo que morre no
    /// meio do grupo deixava no disco a venda sem os itens, e o arranque a
    /// servia ate a rodada seguinte completar -- ou para sempre, se a origem
    /// nao voltasse. O grupo agora grava a MESMA marca `.tx` da cascata e do
    /// `COMMIT` antes do primeiro evento (`gravar_marca_da_replica`), e o
    /// arranque a completa pela mesma recuperacao, com a porta fechada. Anda
    /// para a frente, e nunca para tras: desfazer devolveria slot, e o `.reg`
    /// nao reaproveita slot.
    ///
    /// A marca sai so depois do `fsync` das tabelas, no fim da rodada
    /// (`soltar_marcas_da_replica`), pela regra do group commit: apagar
    /// antes abriria a janela em que o dado nao esta no disco e nao ha
    /// bilhete para traze-lo.
    ///
    /// O que NAO e atomico, e e de proposito: o `aplicar_evento` que falha no
    /// meio (rowid que nao bate -- a replica ja divergiu) deixa o que entrou
    /// antes dele e devolve o erro, o fail-stop de sempre, e a marca sai ali
    /// mesmo -- completar no arranque bateria no mesmo erro. Desfazer pediria
    /// a Sombra, que esta parada por decisao do dono.
    ///
    /// # Onde a marca vai parar -- `marcas`
    ///
    /// Entra em `marcas` no instante em que fica EM VOO, e nao so no fim: um
    /// `?` no meio do grupo (a tabela que nao abre, o diario que nao conta)
    /// devolvia o erro com a marca fora da lista, e ela ficava no disco ate o
    /// proximo arranque -- que reaplicaria um grupo cuja rodada ja tinha ido
    /// adiante (pedido 699, a pendencia menor). Na lista, ela sai depois do
    /// `fsync` da rodada, como a de todo grupo.
    pub(super) fn aplicar_grupo_da_replica(
        &self,
        database: &str,
        filas: &mut [FilaDaReplica],
        mut grupo: Vec<(usize, Vec<crate::replica::EventoRecebido>)>,
        origem: &str,
        marcas: &mut Vec<PathBuf>,
    ) -> Result<Grupo> {
        // A mae antes da filha; entre iguais, a ordem de chegada (o sort e
        // estavel). Ver `ordem_das_maes`.
        grupo.sort_by_key(|(i, _)| filas[*i].ordem);
        let mut rompidas: Vec<(usize, String)> = Vec::new();
        let mut aplicadas: Vec<usize> = Vec::new();
        let mut total = 0u64;
        // A conferencia de continuidade so existe no primeiro grupo depois de
        // (re)ligar, e ela tem de vir ANTES da marca: a tabela rompida nao
        // pode entrar no bilhete, senao o arranque que completasse a marca
        // aplicaria eventos de la por cima de um diario que ja divergiu. Por
        // isso uma passada propria com a trava, so quando ha o que conferir;
        // o grupo comum toma a trava uma vez so.
        if grupo.iter().any(|(i, _)| filas[*i].conferencia.is_some()) {
            let trava = self.travar_dados()?;
            let db = trava.abrir_database(database)?;
            for (i, _) in &grupo {
                let f = &filas[*i];
                let mut t = db.abrir_qualificada(&f.no.nome)?;
                if t.eventos()? != f.posicao {
                    return Ok(Grupo::Andou);
                }
                if let Some(c) = &f.conferencia {
                    if let Err(m) = self.diario_local_continua(&mut t, &f.chave, f.posicao, c) {
                        rompidas.push((*i, m));
                    }
                }
            }
        }
        // O bilhete do grupo, gravado e sincronizado ANTES do primeiro evento
        // (pedido 682) -- e ANTES da trava: o `fsync` da marca com a trava
        // global na mao parava todo leitor e todo escritor do servidor pelo
        // tempo de um disco (catraca `alcancam-fsync`). O conteudo ja e
        // conhecido aqui; o que a trava confere depois so pode RECUSAR o
        // grupo, e ai a marca sai. A queda entre a marca e o primeiro evento
        // deixa um bilhete que o arranque completa pela posicao
        // (`aplicar_evento_da_marca`): diario na posicao, aplica; diario que
        // andou por escrita local, o evento nao confere e a marca sai como
        // impossivel, sem gravar nada.
        let marca = self.marcar_o_grupo(database, filas, &grupo, &rompidas)?;
        // So em `debug`, pedido 715: o N-esimo grupo com marca PARA aqui -- a
        // marca no disco e a trava ainda nao tomada --, para a prova pegar uma
        // transacao local que nasce DEPOIS da marca e entra ANTES do grupo. A
        // trava esta solta: dormir aqui nao segura ninguem.
        #[cfg(debug_assertions)]
        if marca.is_some() {
            static GRUPOS: AtomicU64 = AtomicU64::new(0);
            if gancho_de_teste("PHXSQL_TESTE_PARAR_ANTES_DA_TRAVA_DO_GRUPO")
                == Some(GRUPOS.fetch_add(1, Ordering::SeqCst) + 1)
            {
                eprintln!("teste: grupo da replica parado antes da trava");
                loop {
                    std::thread::sleep(Duration::from_secs(3600));
                }
            }
        }
        let soltar = |marca: &Option<PathBuf>| {
            if let Some(m) = marca {
                let _ = std::fs::remove_file(m);
            }
        };
        {
            let mut trava = match self.travar_dados() {
                Ok(t) => t,
                Err(e) => {
                    soltar(&marca);
                    return Err(e);
                }
            };
            // Ninguem pode ter escrito entre a marca e a trava: a posicao
            // local de cada tabela e reconferida, e o grupo que andou sai
            // levando a marca junto -- nenhum evento dele entrou.
            let conferido = (|| -> Result<Option<phxsql_store::catalogo::Database>> {
                let db = trava.abrir_database(database)?;
                if let Some(m) = &marca {
                    if m.parent() != Some(db.caminho()) {
                        return Err(PhxError::Corrompido(format!(
                            "a marca do grupo da replica de {database} foi gravada em {} \
                             e o database esta em {}",
                            m.display(),
                            db.caminho().display()
                        )));
                    }
                }
                for (i, _) in &grupo {
                    let f = &filas[*i];
                    if db.abrir_qualificada(&f.no.nome)?.eventos()? != f.posicao {
                        return Ok(None);
                    }
                }
                Ok(Some(db))
            })();
            let db = match conferido {
                Ok(Some(db)) => db,
                Ok(None) => {
                    soltar(&marca);
                    return Ok(Grupo::Andou);
                }
                Err(e) => {
                    soltar(&marca);
                    return Err(e);
                }
            };
            // EM VOO so agora, com a trava na mao: um panico no meio do grupo
            // e reparado completando ESTA marca (pedido 451, M1).
            if let Some(m) = &marca {
                trava.marca_em_voo = Some(MarcaEmVoo {
                    database: database.to_string(),
                    caminho: m.clone(),
                    gravada: true,
                });
                marcas.push(m.clone());
            }
            #[cfg(debug_assertions)]
            let parar_em = gancho_de_teste("PHXSQL_TESTE_PARAR_NO_GRUPO");
            // So em `debug`, pedido 699: o N-esimo evento do grupo morre DENTRO
            // da inclusao, com o slot no `.reg` e o evento fora do diario.
            #[cfg(debug_assertions)]
            let parar_no_reg = gancho_de_teste("PHXSQL_TESTE_PARAR_NO_REG");
            // So em `debug`, pedido 713: o evento `<tabela>:<rowid>` falha com
            // erro do DADO no lugar de se aplicar -- e falha SEMPRE, como um
            // erro de dado de verdade bate no mesmo evento em toda rodada.
            #[cfg(debug_assertions)]
            let falhar_no_evento = gancho_de_teste_em_texto("PHXSQL_TESTE_FALHAR_NO_EVENTO");
            let mut aplicou = false;
            // Pedido 713: QUALQUER quebra daqui ate o fim do grupo -- o erro
            // do dado no evento, a tabela que nao abre, a posicao que nao
            // anda -- cai no mesmo lugar, `parou`, e e decidida uma vez so
            // logo abaixo. Eram tres saidas: o braco do dado apagava a marca,
            // e os dois `?` a deixavam na lista que a rodada apaga depois do
            // `fsync`. As tres deixavam a venda pela metade para sempre.
            let mut parou: Option<PhxError> = None;
            'grupo: for (i, eventos) in &grupo {
                if rompidas.iter().any(|(j, _)| j == i) {
                    continue;
                }
                let f = &mut filas[*i];
                let mut t = match db.abrir_qualificada(&f.no.nome) {
                    Ok(t) => t,
                    Err(e) => {
                        parou = Some(e);
                        break 'grupo;
                    }
                };
                // Pedido 300 §2.7: a replica nao julga a chave estrangeira --
                // CONTA a filha que entra sem a mae.
                t.contar_orfas();
                let mut n = 0u64;
                let mut falhou = None;
                for e in eventos {
                    // O evento daqui nasce com o instante e a origem de LA
                    // (papel C, 17/09/2026, §2.3) -- ver
                    // `aplicar_lote_da_replica`.
                    t.forcar_proximo_evento(e.carimbo_ms, e.origem);
                    #[cfg(debug_assertions)]
                    if parar_no_reg == Some(total + n + 1) {
                        phxsql_store::ndx::panico_de_teste::armar_gancho(
                            phxsql_store::ndx::panico_de_teste::Ponto::InserirDepoisDoContador,
                            || sigkill_de_teste("teste: parado entre o .reg e o diario"),
                        );
                    }
                    #[cfg(debug_assertions)]
                    if falhar_no_evento.as_deref()
                        == Some(format!("{}:{}", f.no.nome, e.rowid).as_str())
                    {
                        eprintln!(
                            "teste: erro de dado injetado no evento {}:{}",
                            f.no.nome, e.rowid
                        );
                        falhou = Some(PhxError::Duplicado(format!(
                            "teste: erro de dado injetado no evento {}:{}",
                            f.no.nome, e.rowid
                        )));
                        break;
                    }
                    if let Err(erro) = t.aplicar_evento(e.operacao, e.rowid, &e.imagem) {
                        falhou = Some(erro);
                        break;
                    }
                    n += 1;
                    // So em `debug`: a prova do 682 mata o PROCESSO aqui, com
                    // parte do grupo no disco, por SIGKILL de verdade.
                    #[cfg(debug_assertions)]
                    if parar_em == Some(total + n) {
                        sigkill_de_teste("teste: parado no meio do grupo da replica");
                    }
                }
                self.anotar_orfas(&format!("{database}/{}", f.no.nome), t.orfas_contadas());
                f.aplicados += n;
                total += n;
                aplicou |= n > 0;
                if let Some(erro) = falhou {
                    parou = Some(erro);
                    break 'grupo;
                }
                let nova = match t.eventos() {
                    Ok(nova) => nova,
                    Err(e) => {
                        parou = Some(e);
                        break 'grupo;
                    }
                };
                if n > 0 && nova <= f.posicao {
                    parou = Some(PhxError::Corrompido(format!(
                        "replicacao de {database}.{}: {n} evento(s) aplicado(s) e a \
                         posicao continua em {}",
                        f.no.nome, f.posicao
                    )));
                    break 'grupo;
                }
                f.posicao = nova;
                f.conferencia = None;
                f.conferir = false;
                aplicadas.push(*i);
            }
            if let Some(erro) = parou {
                trava.marca_em_voo = None;
                let Some(m) = marca.as_ref().filter(|_| aplicou) else {
                    // Nada do grupo entrou: nao ha meia venda, e a marca sai
                    // com o erro -- o `NadaAplicado` do `COMMIT`.
                    if let Some(m) = &marca {
                        let _ = std::fs::remove_file(m);
                        marcas.retain(|x| x != m);
                    }
                    return Err(erro);
                };
                // Parte entrou. A decisao do dono no 685 e «a venda chega
                // inteira ou nao chega, sem excecao», e desfazer devolveria
                // slot (ordem de digitacao). Entao para a FRENTE, como o
                // `COMMIT` cuja passada quebra depois da marca: completar o
                // resto AGORA, com a mesma trava, pelo motor da recuperacao
                // (`aplicar_evento_da_marca`, pela posicao do diario). O erro
                // que so a rodada ve -- e o injetado pela prova -- nao volta
                // ali; o que volta deixa a marca no disco para o arranque, e
                // ela SAI da lista da rodada: apaga-la depois do `fsync`, como
                // antes, era perder o unico bilhete da metade que falta (I12).
                marcas.retain(|x| x != m);
                let r = crate::transacao::completar_marca_em_voo(&trava, database, m, true);
                if r.houve() {
                    eprintln!("{}", r.texto(&self.config.base));
                }
                if r.completadas == 1 && r.impossiveis.is_empty() && r.paradas.is_empty() {
                    eprintln!(
                        "replicacao [{origem}]: {database}: o grupo parou no meio ({erro}) e \
                         foi completado pela marca, inteiro, com a mesma trava (pedido 713)"
                    );
                    // A posicao de cada tabela mudou por baixo da rodada: ela
                    // sai, e a proxima recomeca de onde a replica esta.
                    return Ok(Grupo::Andou);
                }
                return Err(com_nota(
                    erro,
                    &format!(
                        "o grupo da replica parou no meio e nao se completou agora; a \
                         marca {} fica no disco e o arranque a completa (pedido 713)",
                        m.display()
                    ),
                ));
            }
            // O grupo terminou: a marca deixa de estar EM VOO (o reparo de um
            // panico daqui em diante nao a completa) e espera o `fsync` da
            // rodada. Um grupo em que nada entrou nao tem o que trazer de
            // volta.
            trava.marca_em_voo = None;
            if !aplicou {
                if let Some(m) = &marca {
                    let _ = std::fs::remove_file(m);
                    marcas.retain(|x| x != m);
                }
            }
        }
        // Os recados saem com a trava SOLTA: sao mutex de replicacao, e
        // empilha-los sob a trava de dados nao compra nada.
        for (i, motivo) in &rompidas {
            let f = &filas[*i];
            self.romper_continuidade(origem, &f.chave, f.posicao, motivo.clone());
        }
        for i in aplicadas {
            let f = &filas[i];
            self.confirmar_continuidade(origem, &f.chave, f.posicao);
        }
        Ok(Grupo::Aplicado {
            n: total,
            rompidas: rompidas.into_iter().map(|(i, _)| i).collect(),
        })
    }

    /// Grava e sincroniza a marca do grupo da replica (pedido 682), SEM a
    /// trava de dados: quem chama a toma depois e so entao a poe EM VOO.
    /// `None` quando o grupo nao tem evento a aplicar.
    ///
    /// O diretorio sai da mesma base que a `Raiz` abriu (`config.base`), e o
    /// chamador confere, ja com a trava, que e o do database aberto.
    ///
    /// O id sai do mesmo contador das marcas de `COMMIT`, e por isso nunca
    /// colide com a de uma transacao no mesmo diretorio. Tomar so a trava das
    /// transacoes, sem a de dados, nao inverte a ordem unica (dados antes de
    /// transacoes): a ordem vale para quem segura as duas.
    fn marcar_o_grupo(
        &self,
        database: &str,
        filas: &[FilaDaReplica],
        grupo: &[(usize, Vec<crate::replica::EventoRecebido>)],
        rompidas: &[(usize, String)],
    ) -> Result<Option<PathBuf>> {
        let mut eventos = Vec::new();
        for (i, lista) in grupo {
            if rompidas.iter().any(|(j, _)| j == i) {
                continue;
            }
            let f = &filas[*i];
            for (k, e) in lista.iter().enumerate() {
                eventos.push(crate::transacao::EventoDoGrupo {
                    tabela: &f.no.nome,
                    operacao: e.operacao,
                    rowid: e.rowid,
                    carimbo_ms: e.carimbo_ms,
                    origem: e.origem,
                    posicao: f.posicao + k as u64,
                    imagem: &e.imagem,
                });
            }
        }
        if eventos.is_empty() {
            return Ok(None);
        }
        let dir = self.config.base.join(database);
        let id = self.transacoes.travar().numero_de_marca();
        crate::transacao::gravar_marca_da_replica(&dir, id, crate::agora_ms(), &eventos).map(Some)
    }

    /// Apaga as marcas dos grupos da replica de uma rodada -- pedido 682.
    /// Quem chama ja sincronizou toda tabela que elas nomeiam: e a ordem do
    /// group commit (dado no disco, depois o bilhete sai), e ela nao se
    /// inverte.
    pub(super) fn soltar_marcas_da_replica(marcas: Vec<PathBuf>) {
        for m in marcas {
            let _ = std::fs::remove_file(m);
        }
    }
}

/// Abre a tabela da replica, criando-a -- e o database -- se ainda nao
/// existem aqui. `None` quando ela nao existe e o source nao mandou o esquema
/// -- e o database que nasceu mesmo assim continua no `fsync` devolvido.
///
/// Um so para as duas fases 1 (unidirecional e bidirecional), que repetiam o
/// mesmo `match`: o pedido 589 tinha de entrar nas duas, e a que alguem
/// esquecesse criaria tabela sem `fsync`. O `fsync` volta por fazer, para
/// quem chama leva-lo ao disco depois de soltar a trava.
///
/// Tabela que ainda nao existe aqui nasce do MESMO bloco de esquema que o
/// source tem, e nao de uma remontagem a partir de JSON: e assim que o
/// payload da imagem cai byte a byte no lugar certo.
pub(super) fn garantir_tabela_da_replica(
    dados: &Instancia,
    database: &str,
    no: &crate::replica::NoSource,
    ledger_marcado_recebido: &AtomicU64,
) -> Result<(Option<Table>, PorSincronizar)> {
    let (db, mut pendente) = dados.garantir_database_adiando_o_fsync(database)?;
    let tabela = match db.abrir_qualificada(&no.nome) {
        Ok(t) => t,
        Err(_) => match &no.esquema {
            Some(e) => {
                let schema = no.nome.split_once('.').map(|(s, _)| s.to_string());
                eprintln!("replicacao: criando {database}.{} aqui", no.nome);
                // Pedido 424: o esquema que chega nao passou pela declaracao,
                // e sim por `Schema::desserializar`, que nao julga -- entao a
                // cadeia com coluna marcada nasce aqui igual nasceu la. Nao
                // se recusa (pararia a replicacao de uma cadeia que nasceu
                // legitima, e a petrea ganha da regua 7x2); grita e conta,
                // pelo MESMO motor do censo, para os dois nunca divergirem.
                let (e_ledger, marcadas) = phxsql_store::ledger::recensear(e);
                if e_ledger && !marcadas.is_empty() {
                    ledger_marcado_recebido.fetch_add(1, Ordering::Relaxed);
                    eprintln!(
                        "LEDGER MARCADO RECEBIDO em {database}.{}: a cadeia chega do \
                         source com coluna(s) marcada(s) como dado pessoal ({}) -- o \
                         hash sem sal do conteudo em claro e oraculo de confirmacao. \
                         A replica cria, como o source tem; rode o censo-do-ledger \
                         em todo no (pedido 424)",
                        no.nome,
                        marcadas.join(", ")
                    );
                }
                let (t, criada) = db.criar_tabela_adiando_o_fsync(schema.as_deref(), e.clone())?;
                pendente.juntar(criada);
                t
            }
            None => return Ok((None, pendente)),
        },
    };
    Ok((Some(tabela), pendente))
}

/// O prefixo do nome da origem do cluster (`cluster:<id>`). Um so, porque e
/// por ele que a rodada da replica sabe que o que o master anuncia e o
/// conjunto replicado do cluster (pedido 300) -- escrever o texto em dois
/// lugares deixaria um deles para tras no dia de trocar.
pub(super) const PREFIXO_DA_ORIGEM_DO_CLUSTER: &str = "cluster:";
