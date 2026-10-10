//! O quorum: esperar, materializar e aplicar o que o quorum pede.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// Os lotes de UMA tabela numa resposta do quorum, emendados -- pedido 681.
struct LotesDaTabela {
    no: crate::replica::NoSource,
    /// A posicao que a replica tem de ter: a do PRIMEIRO lote.
    posicao: u64,
    /// Com `posicao > 0`, o primeiro e o de conferencia (o `posicao - 1`).
    eventos: Vec<crate::replica::EventoRecebido>,
    /// Onde o ultimo lote emendado termina.
    fim: u64,
    descontinuo: bool,
}

impl LotesDaTabela {
    fn do_fio(l: &Json) -> Result<(String, LotesDaTabela)> {
        let database = l.texto_ou("database", "").to_string();
        let posicao = l.inteiro_ou("posicao", 0).max(0) as u64;
        let eventos = crate::replica::eventos_do_fio(l.campo("eventos"))?;
        let esquema = match l.texto_ou("esquema", "") {
            "" => None,
            hex => Some(phxsql_core::schema::Schema::desserializar(
                &hex_para_bytes(hex)?,
            )?),
        };
        let no = crate::replica::NoSource {
            nome: l.texto_ou("tabela", "").to_string(),
            eventos: l.inteiro_ou("eventos_do_master", 0).max(0) as u64,
            esquema,
            proxima_sequencia: crate::replica::proxima_sequencia_do_fio(
                l.campo("proxima_sequencia"),
            ),
        };
        // Onde o lote termina: o primeiro evento e o de conferencia quando
        // `posicao > 0`.
        let fim = if posicao == 0 {
            eventos.len() as u64
        } else {
            posicao + (eventos.len() as u64).saturating_sub(1)
        };
        Ok((
            database,
            LotesDaTabela {
                no,
                posicao,
                eventos,
                fim,
                descontinuo: false,
            },
        ))
    }

    /// Emenda o lote seguinte da mesma tabela. O de conferencia dele repete
    /// o ultimo deste e sai. `false` = nao continua daqui.
    fn emendar(&mut self, mut outro: LotesDaTabela) -> bool {
        if outro.posicao != self.fim {
            return false;
        }
        if outro.posicao > 0 && !outro.eventos.is_empty() {
            outro.eventos.remove(0);
        }
        self.eventos.append(&mut outro.eventos);
        self.fim = outro.fim;
        // O lote de depois sabe mais: a contagem e o contador dele valem.
        self.no.eventos = self.no.eventos.max(outro.no.eventos);
        self.no.proxima_sequencia = outro.no.proxima_sequencia;
        true
    }
}

/// O maior lote que um `replicar` serve, pecam o que pedirem.
///
/// Dez vezes o lote da replica: sobra para quem quiser lotes maiores, e nunca
/// o diario inteiro. O teto em BYTES (`TETO_DO_LOTE_SERVIDO`) continua
/// mandando por cima deste: os dois sao tetos de coisas diferentes, e o de
/// eventos existe para um diario de linhas minusculas nao virar cinco milhoes
/// de `Evento` na memoria por causa de um `"max"` absurdo.
pub(super) const TETO_DE_EVENTOS_POR_LOTE: u64 = 5_000;

impl Servidor {
    // ------------------------------------------- o quorum de escrita (207)

    /// As escritas desta tomada esperam quorum? So no MASTER vivo do cluster
    /// e com `quorum_minimo > 0`: duas leituras atomicas, e e o portao que
    /// decide se a anotacao das tocadas liga -- antes de qualquer trabalho.
    pub(super) fn quorum_vale_aqui(&self) -> bool {
        match (&self.quorum, &self.cluster) {
            (Some(q), Some(c)) => q.minimo() > 0 && c.papel() == crate::cluster::PapelVivo::Master,
            _ => false,
        }
    }

    /// A espera do commit, chamada do `Drop` da `TravaMedida` com a trava de
    /// escrita AINDA na mao -- a linha so fica visivel depois do ok ou do
    /// prazo (o `xact.c:1541` do PostgreSQL, «continue to hold locks»).
    ///
    /// Ordem, e cada passo e uma decisao do contrato (§207.5):
    ///
    /// 1. as tabelas que nenhuma replica recebe saem da conta -- fora de
    ///    `cluster.databases`, ou que o usuario do cluster nao pode
    ///    `replicar`. Esperar por elas seria degradar por desenho;
    /// 2. degradado: anota a posicao e responde na hora, DIZENDO;
    /// 3. `sincronizar` local (D-local): o master que cai sem `fsync` e volta
    ///    como master ficaria ATRAS das replicas que confirmaram;
    /// 4. os eventos saem pelo MESMO motor do `replicar`
    ///    ([`Self::eventos_para_o_fio`]) e vao ao cubo, que espera.
    pub(super) fn esperar_o_quorum(
        &self,
        dados: &Instancia,
        tocadas: Vec<phxsql_store::log::Tocada>,
    ) {
        let (Some(cubo), Some(estado)) = (&self.quorum, &self.cluster) else {
            return;
        };
        let comeco = Instant::now();
        let ficha = cubo.ficha();
        let replicados = &estado.config.databases;
        let mut alvos: Vec<(crate::quorum::Exigencia, u64)> = Vec::new();
        let mut incerta = false;
        for t in tocadas {
            let Some((database, tabela)) = nome_da_tocada(dados.base(), &t) else {
                continue;
            };
            if !replicados.is_empty() && !replicados.iter().any(|d| d == &database) {
                continue;
            }
            if !replica_alcanca(ficha.as_ref(), &database, &tabela) {
                continue;
            }
            if t.depois <= t.antes {
                // O diario cresceu e o total nao se leu: nao ha posicao para
                // exigir. A gravacao fica e a resposta diz que nao se sabe.
                incerta = true;
                continue;
            }
            alvos.push((
                crate::quorum::Exigencia {
                    database,
                    tabela,
                    posicao: t.depois,
                },
                t.antes,
            ));
        }
        if alvos.is_empty() && !incerta {
            return;
        }
        let pedido = cubo.minimo();
        let exigencias: Vec<crate::quorum::Exigencia> =
            alvos.iter().map(|(x, _)| x.clone()).collect();
        if cubo.modo() == crate::quorum::Modo::Degradado {
            cubo.anotar_mestre(&exigencias);
            guardar_quorum_do_pedido(crate::quorum::Resultado {
                pedido,
                confirmado: 0,
                alcancado: false,
                degradado: true,
                ms: comeco.elapsed().as_secs_f64() * 1000.0,
            });
            return;
        }
        let mut lotes = Vec::new();
        let mut esperar_por = Vec::new();
        for (x, antes) in &alvos {
            match self.materializar_para_o_quorum(dados, cubo, x, *antes) {
                Ok(mut l) => {
                    lotes.append(&mut l);
                    esperar_por.push(x.clone());
                }
                Err(e) => {
                    eprintln!(
                        "quorum: {}/{}: os eventos do commit nao se materializaram ({e}); \
                         a gravacao fica, e a resposta diz alcancado:false",
                        x.database, x.tabela
                    );
                    incerta = true;
                }
            }
        }
        cubo.anotar_mestre(&exigencias);
        let mut r = if esperar_por.is_empty() {
            crate::quorum::Resultado {
                pedido,
                confirmado: 0,
                alcancado: false,
                degradado: false,
                ms: 0.0,
            }
        } else {
            cubo.esperar(lotes, &esperar_por)
        };
        if incerta {
            r.alcancado = false;
        }
        r.ms = comeco.elapsed().as_secs_f64() * 1000.0;
        guardar_quorum_do_pedido(r);
    }

    /// Sincroniza a tabela e materializa os eventos `[antes, depois)` em
    /// lotes que cabem no fio -- com o evento `antes - 1` na frente de cada
    /// um, a conferencia de continuidade que o pull ja faz.
    fn materializar_para_o_quorum(
        &self,
        dados: &Instancia,
        cubo: &crate::quorum::Cubo,
        x: &crate::quorum::Exigencia,
        antes: u64,
    ) -> Result<Vec<crate::quorum::LoteDoQuorum>> {
        let db = dados.abrir_database(&x.database)?;
        let mut t = db.abrir_qualificada(&x.tabela)?;
        // D-local (L1): PG 4 + MySQL 2 = 6 contra MariaDB 3.
        let ja = t.arquivos_sincronizados();
        t.sincronizar()?;
        cubo.contar_fsync_local(t.arquivos_sincronizados().saturating_sub(ja));
        let chave = Self::chave_do_diario(&x.database, &x.tabela);
        let dado_pessoal = t.tem_dado_pessoal();
        let linhagem = match t.esquema().linhagem() {
            Some(l) => Json::texto_de(l.to_string()),
            None => Json::Nulo,
        };
        let esquema = bytes_para_hex(&t.esquema().serializar());
        let mut lotes = Vec::new();
        let mut ini = antes;
        while ini < x.posicao {
            let desde = ini.saturating_sub(1);
            let max = (x.posicao - desde).min(TETO_DE_EVENTOS_POR_LOTE);
            let (eventos, lidos) = self.eventos_para_o_fio(&mut t, &chave, desde, max, None)?;
            let fim = desde + lidos;
            if fim <= ini {
                return Err(PhxError::LimiteExcedido(format!(
                    "o evento {ini} do diario de {}/{} nao coube no lote do quorum",
                    x.database, x.tabela
                )));
            }
            // A trilha de dado pessoal, como no `replicar` (A8): a imagem
            // viaja com a coluna marcada dentro.
            Self::trilhar_acesso(&mut t, 0, eventos.len() as u64, || {
                format!("replicar_aguardar desde={desde} ate={fim}")
            })?;
            let bytes = eventos
                .iter()
                .map(|e| e.texto_ou("imagem", "").len() + 200)
                .sum::<usize>()
                + esquema.len();
            lotes.push(crate::quorum::LoteDoQuorum {
                database: x.database.clone(),
                tabela: x.tabela.clone(),
                dado_pessoal,
                eventos_do_master: x.posicao,
                proxima_sequencia: if t.esquema().coluna_sequencia().is_some() {
                    t.sequencia_atual()
                } else {
                    0
                },
                posicao: ini,
                eventos,
                linhagem: linhagem.clone(),
                esquema: esquema.clone(),
                bytes,
                // O cubo numera ao entregar: um commit, um numero.
                commit: 0,
            });
            ini = fim;
        }
        Ok(lotes)
    }

    /// `replicar_aguardar`: o canal aberto do quorum, do lado do master.
    ///
    /// A replica abre a conexao e FICA (rota (b), decisao do dono de
    /// 07/09/2026: o firewall do master continua com uma porta de entrada so).
    /// Cada pedido confirma o que ela aplicou e gravou, e espera ate
    /// `esperar_ms` por lotes novos. **Nunca chama `travar_dados()`** -- o
    /// commit que espera esta confirmacao segura a trava, e e isso que a
    /// guarda `quorum-ack-pede-a-trava` repoe.
    pub(super) fn op_replicar_aguardar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let Some(cubo) = &self.quorum else {
            return Err(Self::sem_cluster());
        };
        let epoca = estado.epoca();
        if cubo.minimo() == 0 || estado.papel() != crate::cluster::PapelVivo::Master {
            return Ok(Json::objeto(vec![
                ("epoca", Json::de_u64(epoca)),
                ("desligado", Json::Bool(true)),
                ("papel", Json::texto_de(estado.papel().nome())),
            ]));
        }
        let id = p.texto_ou("id", "").trim().to_string();
        if id == estado.config.id || estado.no(&id).is_none() {
            return Err(PhxError::Autorizacao(
                self.msg("erro.pulso_de_no_desconhecido", &[("id", &id)]),
            ));
        }
        // Pedido 649: o `id` e declarado pelo cliente e nao amarra a sessao,
        // entao a confirmacao so vale para a tabela que ESTA sessao passa por
        // `replica_alcanca` -- o mesmo portao que filtra os lotes entregues.
        // Sem isto, um `Replicar` so da tabela A fechava o quorum de B sem
        // nenhuma replica ter gravado B (a durabilidade anunciada, mentira).
        let confirmados: Vec<crate::quorum::Exigencia> =
            crate::quorum::Exigencia::da_lista(p.campo("confirmado"))
                .into_iter()
                .filter(|c| replica_alcanca(sessao.usuario.as_ref(), &c.database, &c.tabela))
                .collect();
        // A ficha do cubo (que tabela as replicas alcancam) so e gravada por
        // quem entra com a credencial do CLUSTER: o ultimo-a-chegar-vence
        // deixava um `Replicar` fraco estreitar o alcance e degradar o
        // servidor. Cluster sem `usuario` declarado = comportamento velho.
        let ficha = if sessao_e_do_cluster(&estado.config, sessao) {
            sessao.usuario.as_ref()
        } else {
            None
        };
        // A espera cabe no pulso: a replica tem de voltar a conversar antes
        // de o silencio dela parecer queda.
        let teto = estado.config.pulso_s.saturating_mul(1_000).max(100);
        let esperar =
            Duration::from_millis(p.inteiro_ou("esperar_ms", 0).clamp(0, teto as i64) as u64);
        let entrega = cubo.aguardar(&id, ficha, &confirmados, esperar);
        let cifrado = self.fio_cifrado(sessao);
        let mut lotes = Vec::with_capacity(entrega.lotes.len());
        for l in &entrega.lotes {
            // Os portoes que dependem de QUEM leva, conferidos na entrega:
            // o alcance do usuario e a cifra do 342. Lote barrado nao vai, a
            // replica nao confirma, e o commit degrada DIZENDO.
            if !replica_alcanca(sessao.usuario.as_ref(), &l.database, &l.tabela) {
                continue;
            }
            if l.dado_pessoal && !cifrado {
                continue;
            }
            lotes.push(l.para_json());
        }
        Ok(Json::objeto(vec![
            ("epoca", Json::de_u64(epoca)),
            ("modo", Json::texto_de(entrega.modo.nome())),
            ("lotes", Json::Lista(lotes)),
            (
                "alcance",
                Json::Lista(entrega.alcance.iter().map(|x| x.para_json()).collect()),
            ),
        ]))
    }

    /// O canal aberto do quorum, do lado da REPLICA -- pedido 207.
    ///
    /// Fica numa conversa so com o master corrente enquanto ele estiver
    /// sincrono: leva o lote, aplica, grava em disco, e so entao confirma, no
    /// pedido seguinte. **Nao puxa pelo `replicar` enquanto o master esta
    /// sincrono**: o `replicar` pede a trava de dados do master, e um commit
    /// esperando esta replica a segura -- o pull ficaria parado ate o prazo.
    /// Volta ao pull quando ficou para tras: um lote que nao se aplicou, ou o
    /// degradado anunciando um alcance que ela nao tem.
    pub(super) fn esperar_pelo_master(
        &self,
        estado: &crate::cluster::EstadoCluster,
        origem: &crate::config::Origem,
        master: &str,
        espera: Duration,
        canal: &mut Option<(String, crate::replica::Cliente)>,
        confirmar: &mut Vec<crate::quorum::Exigencia>,
    ) {
        loop {
            if estado.papel() == crate::cluster::PapelVivo::Master
                || estado.master_atual().map(|(id, _)| id).as_deref() != Some(master)
            {
                return;
            }
            if canal.as_ref().map(|(m, _)| m.as_str()) != Some(master) {
                *canal = None;
                match crate::replica::ligar_classificado(origem) {
                    Ok(c) => *canal = Some((master.to_string(), c)),
                    Err((_, e)) => {
                        eprintln!("cluster: o canal do quorum com {master} nao abriu: {e}");
                        std::thread::sleep(espera);
                        return;
                    }
                }
            }
            let Some((_, cliente)) = canal.as_mut() else {
                return;
            };
            let pedido = vec![
                ("op", Json::texto_de("replicar_aguardar")),
                ("id", Json::texto_de(&estado.config.id)),
                ("epoca", Json::de_u64(estado.epoca())),
                (
                    "confirmado",
                    Json::Lista(confirmar.iter().map(|x| x.para_json()).collect()),
                ),
                ("esperar_ms", Json::de_u64(espera.as_millis() as u64)),
            ];
            let r = match cliente.pedir(pedido) {
                Ok(r) => r,
                Err(e) => {
                    // Master de versao anterior (op desconhecida) ou conexao
                    // caida: o canal fecha e o pull de sempre segue.
                    eprintln!("cluster: canal do quorum com {master}: {e}");
                    *canal = None;
                    std::thread::sleep(espera);
                    return;
                }
            };
            // Mandadas: a confirmacao chegou ao master.
            confirmar.clear();
            if r.booleano_ou("desligado", false) {
                std::thread::sleep(espera);
                return;
            }
            let dele = r.inteiro_ou("epoca", 0).max(0) as u64;
            if !crate::quorum::aceita_a_epoca(estado.epoca(), dele) {
                eprintln!(
                    "cluster: {master} mandou lote da epoca {dele}, e esta replica ja \
                     conhece a {}: nada se aplica nem se confirma (master rebaixado)",
                    estado.epoca()
                );
                *canal = None;
                std::thread::sleep(espera);
                return;
            }
            let lotes = r.campo("lotes").and_then(Json::lista).unwrap_or(&[]);
            let (feitos, atras) = self.aplicar_lotes_do_quorum(&origem.nome, lotes, master);
            for x in feitos {
                confirmar.retain(|c| c.chave() != x.chave());
                confirmar.push(x);
            }
            if atras {
                return;
            }
            if lotes.is_empty() {
                // Degradado: o master diz o que conhece, e esta replica diz
                // se esta em dia. Em dia, confirma -- e e assim que o master
                // volta ao sincrono depois do recuo. Atras, puxa: o commit
                // degradado nao segura a trava.
                for x in crate::quorum::Exigencia::da_lista(r.campo("alcance")) {
                    match self.posicao_local(&x.database, &x.tabela) {
                        Ok(p) if p >= x.posicao => {
                            confirmar.push(crate::quorum::Exigencia { posicao: p, ..x })
                        }
                        _ => return,
                    }
                }
            }
        }
    }

    /// Aplica os lotes de UMA resposta do quorum por transacao -- pedido 681.
    ///
    /// # Por que nao lote a lote
    ///
    /// Era assim ate o 681: cada lote (uma tabela de um commit) ia ao disco
    /// sob a propria tomada da trava, e entre o lote dos itens e o da venda um
    /// leitor desta replica via a venda pela metade -- o defeito que o 676
    /// fechou no pull e que ficou aqui, no irmao. Agora o caminho e o MESMO do
    /// pull: os lotes de cada database viram as filas de um
    /// [`crate::replica::Juntador`], e cada grupo de transacoes inteiras vai
    /// ao disco por [`Self::aplicar_grupo_da_replica`], numa tomada so.
    ///
    /// # O que garante que a transacao esta inteira na resposta
    ///
    /// O cubo so corta a entrega entre commits (`quorum::Cubo::aguardar`),
    /// entao a fronteira de cada tabela e o fim do ultimo lote dela, e o
    /// `Juntador` nunca precisa puxar. Se pedir, o database fica para o pull.
    ///
    /// # O que volta
    ///
    /// As confirmacoes das tabelas que chegaram ao fim do lote E foram ao
    /// disco (o «ok» e aplicou e gravou, dono 17/09/2026), e `true` quando
    /// alguma ficou atras -- o chamador entao nao confirma o degradado e o
    /// pull alcanca.
    fn aplicar_lotes_do_quorum(
        &self,
        origem: &str,
        lotes: &[Json],
        master: &str,
    ) -> (Vec<crate::quorum::Exigencia>, bool) {
        // Os lotes por database e, dentro dele, por tabela, na ordem do fio.
        let mut bases: Vec<(String, Vec<LotesDaTabela>)> = Vec::new();
        let mut atras = false;
        for l in lotes {
            let (database, novo) = match LotesDaTabela::do_fio(l) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("cluster: lote do quorum de {master}: {e}");
                    atras = true;
                    continue;
                }
            };
            let posicao = match bases.iter().position(|(d, _)| *d == database) {
                Some(i) => i,
                None => {
                    bases.push((database, Vec::new()));
                    bases.len() - 1
                }
            };
            let tabelas = &mut bases[posicao].1;
            match tabelas.iter_mut().find(|t| t.no.nome == novo.no.nome) {
                // Dois lotes da mesma tabela que nao se continuam: o database
                // inteiro fica para o pull, e nada dele se aplica pela metade.
                Some(t) => {
                    if !t.emendar(novo) {
                        t.descontinuo = true;
                    }
                }
                None => tabelas.push(novo),
            }
        }
        let mut feitos = Vec::new();
        for (database, tabelas) in bases {
            match self.aplicar_database_do_quorum(origem, &database, tabelas) {
                Ok((mut x, a)) => {
                    feitos.append(&mut x);
                    atras |= a;
                }
                Err(e) => {
                    eprintln!("cluster: lote do quorum de {master} ({database}): {e}");
                    atras = true;
                }
            }
        }
        (feitos, atras)
    }

    /// Um database de uma resposta do quorum, pelo motor do pull (681).
    fn aplicar_database_do_quorum(
        &self,
        origem: &str,
        database: &str,
        tabelas: Vec<LotesDaTabela>,
    ) -> Result<(Vec<crate::quorum::Exigencia>, bool)> {
        if tabelas.iter().any(|t| t.descontinuo) {
            return Ok((Vec::new(), true));
        }
        // Fase 1, como no pull: abrir (criando pelo esquema) e conferir que a
        // replica esta onde cada lote comeca. Uma tabela fora do lugar deixa
        // o database inteiro para o pull -- aplicar as outras partiria o
        // commit que a inclui.
        let mut filas: Vec<FilaDaReplica> = Vec::new();
        let mut eventos: Vec<Vec<crate::replica::EventoRecebido>> = Vec::new();
        let mut fins: Vec<u64> = Vec::new();
        for t in tabelas {
            let Some((local, outra_historia)) = self.abrir_para_replicar(database, &t.no)? else {
                return Ok((Vec::new(), true));
            };
            if let Some(motivo) = outra_historia {
                eprintln!("cluster: {database}/{}: {motivo}", t.no.nome);
                return Ok((Vec::new(), true));
            }
            if local != t.posicao {
                return Ok((Vec::new(), true));
            }
            let mut lista = t.eventos;
            let conferencia = if t.posicao > 0 && !lista.is_empty() {
                Some(lista.remove(0))
            } else {
                None
            };
            fins.push(t.posicao + lista.len() as u64);
            eventos.push(lista);
            filas.push(FilaDaReplica {
                chave: Self::chave_do_diario(database, &t.no.nome),
                conferir: conferencia.is_some(),
                conferencia,
                posicao: t.posicao,
                aplicados: 0,
                ordem: 0,
                no: t.no,
            });
        }
        let vezes = ordem_das_maes(&filas.iter().map(|f| &f.no).collect::<Vec<_>>());
        for (f, vez) in filas.iter_mut().zip(vezes) {
            f.ordem = vez;
        }
        let alvos: Vec<(u64, u64)> = filas
            .iter()
            .zip(&fins)
            .map(|(f, &fim)| (f.posicao, fim))
            .collect();
        let mut juntador =
            crate::replica::Juntador::novo(&alvos, phxsql_store::log::teto_da_transacao());
        for (i, lista) in eventos.into_iter().enumerate() {
            if !lista.is_empty() {
                juntador.receber(i, lista);
            }
        }
        let mut atras = false;
        let mut marcas: Vec<PathBuf> = Vec::new();
        let saida = loop {
            match juntador.passo() {
                // Tudo ja esta na mao; pedir mais e sinal de resposta partida.
                crate::replica::Passo::Puxar { .. } => {
                    atras = true;
                    break Ok(());
                }
                crate::replica::Passo::Aplicar { grupo, .. } => {
                    match self.aplicar_grupo_da_replica(
                        database,
                        &mut filas,
                        grupo,
                        origem,
                        &mut marcas,
                    ) {
                        Ok(Grupo::Aplicado { .. }) => {}
                        // Pedido 299, F2, o irmao do pull: a continuidade que
                        // rompe recusa o grupo inteiro, e o database fica
                        // para o pull -- que diz a ruptura e monta a barreira
                        // por transacao. Largar so a tabela aqui deixaria as
                        // irmas da mesma venda entrarem pelo quorum.
                        Ok(Grupo::Rompidas(_)) | Ok(Grupo::Andou) => {
                            atras = true;
                            break Ok(());
                        }
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
        // O «ok» e APLICOU E GRAVOU EM DISCO (dono, 17/09/2026): o `fsync`
        // vem antes de a confirmacao existir. O que entrou e nao chegou ao
        // fim tambem vai ao disco -- sem confirmar.
        let mut feitos = Vec::new();
        for (f, &fim) in filas.iter().zip(&fins) {
            let arquivos = if f.aplicados > 0 {
                self.sincronizar_replicada_contando(database, &f.no.nome)?
            } else {
                0
            };
            if f.posicao < fim {
                atras = true;
                continue;
            }
            if let Some(cubo) = &self.quorum {
                cubo.contar_ack(arquivos);
            }
            feitos.push(crate::quorum::Exigencia {
                database: database.to_string(),
                tabela: f.no.nome.clone(),
                posicao: f.posicao,
            });
        }
        Self::soltar_marcas_da_replica(marcas);
        saida.map(|()| (feitos, atras))
    }
}

/// Guarda o resultado de uma espera do quorum para a resposta deste pedido.
fn guardar_quorum_do_pedido(r: crate::quorum::Resultado) {
    QUORUM_DO_PEDIDO.with(|q| {
        let mut q = q.borrow_mut();
        *q = Some(match q.take() {
            Some(antes) => antes.juntar(r),
            None => r,
        });
    });
}

/// A replica que entra com esta ficha alcanca esta tabela?
///
/// UM motor para as duas perguntas que tem a mesma resposta: o `posicao` (que
/// tabelas o source mostra a replica) e a posicao do master no cluster (o que
/// ele serve, pedido 300). Duas contas divergiriam no dia em que o portao
/// aprendesse uma regra nova -- e a divergencia apareceria como eleicao
/// perdida, nao como erro. Sem usuario (sessao interna, ou o token) alcanca
/// tudo, que e o comportamento de sempre.
pub(super) fn replica_alcanca(usuario: Option<&Usuario>, database: &str, tabela: &str) -> bool {
    usuario.is_none_or(|u| u.pode_em(database, tabela, Atividade::Replicar))
}

/// `(database, tabela qualificada)` de uma tabela tocada, pelo caminho do
/// diario dela: `base/<database>/<tabela>` ou `base/<database>/<schema>/<tabela>`
/// -- o mesmo arranjo do `Database::diretorio`. Fora da base (um palco de
/// restauracao, por exemplo) nao e tabela do cluster: `None`.
fn nome_da_tocada(base: &Path, t: &phxsql_store::log::Tocada) -> Option<(String, String)> {
    let rel = t.diretorio.strip_prefix(base).ok()?;
    let partes: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    match partes.as_slice() {
        [db] => Some((db.clone(), t.nome.clone())),
        [db, schema] => Some((db.clone(), format!("{schema}.{}", t.nome))),
        _ => None,
    }
}
