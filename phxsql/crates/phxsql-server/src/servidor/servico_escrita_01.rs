//! Escrita: inserir, atualizar, excluir, lixeira, LGPD, trilha e o
//! bulkinsert.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// Como uma alteracao solta foi gravada -- ver `Servidor::alterar_solto`.
pub(super) struct AlteracaoSolta {
    /// Pela marca: a passada ja levou a janela de durabilidade e a copia
    /// residente de cada tabela, e quem chama nao repete.
    pela_marca: bool,
    /// A cascata quebrou no meio e a recuperacao a completou: o que dizer.
    pub(super) aviso: Option<String>,
}

/// A lista que vai para a marca: a do cliente, com os elos que a
/// pre-conferencia planejou logo depois de quem os puxou, e toda alteracao
/// marcada para aplicar SEM replanejar -- a cascata inteira ja esta nela
/// (achado A1 da revisao do DBA ao 448).
pub(super) fn costurar_os_elos(
    escritas: Vec<crate::transacao::Escrita>,
    elos: Vec<(usize, Vec<crate::transacao::Escrita>)>,
) -> Vec<crate::transacao::Escrita> {
    let extra: usize = elos.iter().map(|(_, v)| v.len()).sum();
    let mut saida = Vec::with_capacity(escritas.len() + extra);
    let mut elos = elos.into_iter().peekable();
    for (i, mut e) in escritas.into_iter().enumerate() {
        if e.acao == crate::transacao::Acao::Atualizar {
            e.cascata_na_lista = true;
        }
        saida.push(e);
        if elos.peek().is_some_and(|(j, _)| *j == i) {
            if let Some((_, grupo)) = elos.next() {
                saida.extend(grupo);
            }
        }
    }
    saida
}

impl Servidor {
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
    pub(super) fn soltar_cargas_da_ligacao(&self, ligacao: u64) {
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
    ///
    /// Confere TODAS as tabelas que o pedido alcanca, e nao so o campo
    /// `"tabela"` (pedido 322). O portao que decide se ha trabalho vem antes do
    /// trabalho: sem reserva nenhuma -- o servidor de quase sempre -- nao se
    /// monta a lista de tabelas, que e uma alocacao por pedido no laco quente
    /// do `inserir`.
    pub(super) fn barrado_por_carga(
        &self,
        op: &str,
        pedido: &Json,
        ligacao: u64,
    ) -> Option<String> {
        let database = pedido.texto_ou("database", "");
        if database.is_empty() {
            return None;
        }
        let mut cargas = self.cargas.lock().ok()?;
        if cargas.quantas() == 0 {
            return None;
        }
        let agora = crate::agora_ms();
        crate::direito_coluna::tabelas_do_pedido(op, pedido)
            .iter()
            .find_map(|t| cargas.barra(database, t, ligacao, agora))
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
    pub(super) fn tabela_reservada(&self, p: &Json) -> bool {
        let (db, tab) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
        if db.is_empty() || tab.is_empty() {
            return false;
        }
        self.reservada(db, tab)
    }

    /// A tabela `db/tab` tem reserva de carga, de quem quer que seja.
    pub(super) fn reservada(&self, db: &str, tab: &str) -> bool {
        self.cargas
            .lock()
            .map(|c| c.reservada(db, tab))
            .unwrap_or(false)
    }

    /// `bulkinsert`: reserva a tabela para uma carga, ou solta.
    ///
    /// So pela porta de dados. Ver o porque em `crate::carga`.
    pub(super) fn op_bulkinsert(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
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

        // `"adiar_indice": true` so vale no `ligado: true`, e so pedido
        // (pedido 324). Soltar nao pergunta: o que decide reconstruir e a
        // marca no `.ndx`, e nao a lembranca de quem pediu -- a reserva pode
        // ter vencido, ou o administrador pode estar soltando a carga alheia.
        let adiar = p.campo("adiar_indice").and_then(Json::booleano) == Some(true);
        if adiar && !ligar {
            return Err(PhxError::Esquema(
                "\"adiar_indice\" vale so para reservar (\"ligado\": true); ao \
                 soltar, o indice adiado se reconstroi sozinho"
                    .into(),
            ));
        }

        // UMA secao critica para os dois sentidos, e a ordem das travas e
        // sempre dados -> cargas (ninguem toma a de dados segurando a de
        // cargas). A tabela tem de existir -- reservar o que nao existe
        // esconderia um erro de digitacao ate o fim da carga --, e o indice
        // adiado precisa dela aberta nos dois pontos: suspender antes da
        // primeira linha, e reconstruir antes de soltar. Duas secoes seriam
        // duas tomadas da trava para a mesma carga, e a do meio deixaria
        // outro pedido entrar entre reservar e suspender.
        let trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&trava, p, sessao)?;
        let agora = crate::agora_ms();
        let mut cargas = self.cargas.tomar("cargas")?;

        if ligar {
            let prazo = self.config.recursos.carga_prazo_min as i64 * 60_000;
            let ja_era_minha = cargas.todas().iter().any(|r| {
                r.ligacao == sessao.ligacao && r.database == database && r.tabela == tabela
            });
            let r = cargas.reservar(
                &database,
                &tabela,
                sessao.login(),
                sessao.ligacao,
                "",
                agora,
                prazo,
            )?;
            // Reservar e suspender sao um passo so para quem ve de fora: a
            // suspensao que recusa desfaz a reserva que ACABOU de nascer (a
            // renovada fica, ela era do cliente antes deste pedido). As
            // recusas da declaracao sao todas do motor -- `adiar_indice`.
            if adiar {
                if let Err(e) = t.adiar_indice() {
                    if !ja_era_minha {
                        let _ = cargas.soltar(&database, &tabela, sessao.ligacao, true, agora);
                    }
                    return Err(e);
                }
                // Na lista das sujas desde ja: se a reserva vencer ou a
                // conexao cair sem nenhuma linha gravada, e o fecho da janela
                // que reconstroi o indice -- e ele so olha as sujas.
                if let Ok(mut sujas) = self.sujas.lock() {
                    sujas.insert(format!("{database}/{tabela}"));
                }
            }
            drop(cargas);
            return Ok(Json::objeto(vec![
                ("bulkinsert", Json::Bool(true)),
                ("database", Json::texto_de(&database)),
                ("tabela", Json::texto_de(&tabela)),
                ("reservada", Json::Bool(true)),
                ("indice_adiado", Json::Bool(t.indice_suspenso())),
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
        cargas.conferir_soltura(&database, &tabela, sessao.ligacao, forcar, agora)?;

        // O indice adiado se reconstroi ANTES de a reserva sair (pedido 324):
        // do `remove` em diante qualquer ligacao pode perguntar ao indice, e
        // ele tem de estar inteiro. O `reindexar` monta em lote a partir do
        // `.reg` e deixa o byte 52 em 1 com o 53 em 0 -- quem baixa o 52 e o
        // `sincronizar` logo abaixo, depois dos dois `fsync`. Se falhar, a
        // reserva FICA e o erro sobe: soltar com a arvore suspensa entregaria
        // a tabela aos outros recusando toda busca.
        let reconstruido = if t.indice_suspenso() {
            let inicio = Instant::now();
            t.reindexar()?;
            Some(inicio.elapsed().as_millis() as u64)
        } else {
            None
        };
        let r = cargas.soltar(&database, &tabela, sessao.ligacao, forcar, agora)?;
        drop(cargas);

        // O fsync que a carga inteira adiou acontece agora -- e e o MESMO
        // fecho da janela, na mesma ordem: sincronizar, tirar das sujas,
        // drenar as marcas. Um COMMIT feito dentro da reserva deixou a marca
        // `.tx` pendurada esperando exatamente este `fsync` (a janela nao
        // fecha em tabela reservada); sincronizar sem drenar era o pedido
        // 254: a marca de um commit ja duravel sobrevivia ao «ok», e toda
        // tomada chutada depois dele fazia o arranque reportar «achadas 1 /
        // ja aplicadas N» para um commit que ja tinha acabado.
        //
        // O `sincronizar` proprio fica, em vez de deixar a tabela na lista
        // para o fecho: o fecho engole o erro de E/S (a chave volta para as
        // sujas e a marca fica, que e o lado seguro), e o cliente receberia
        // `"sincronizada": true` sobre um `fsync` que falhou. Aqui o erro
        // sobe para quem pediu.
        //
        // E tudo sob a trava de dados, inclusive o `remove`: a reserva ja
        // foi solta, e entre o `fsync` e o `remove` outro escritor podia
        // entrar, sujar a tabela de novo e ve-la sair das sujas sem `fsync`.
        t.sincronizar()?;
        if let Ok(mut sujas) = self.sujas.lock() {
            sujas.remove(&format!("{database}/{tabela}"));
        }
        self.descarregar_sujas_com(&trava);
        drop(t);
        drop(trava);

        let mut resposta = vec![
            ("bulkinsert", Json::Bool(false)),
            ("database", Json::texto_de(&database)),
            ("tabela", Json::texto_de(&tabela)),
            ("liberada", Json::Bool(true)),
            // Em milissegundos, e nao em segundos: uma carga de 300 ms
            // aparecia como "durou 0s", que e um numero que nao ajuda ninguem.
            ("durou_ms", Json::de_u64((agora - r.desde_ms).max(0) as u64)),
            ("sincronizada", Json::Bool(true)),
        ];
        if let Some(ms) = reconstruido {
            resposta.push(("indice_reconstruido", Json::Bool(true)));
            resposta.push(("reconstruir_ms", Json::de_u64(ms)));
        }
        Ok(Json::objeto(resposta))
    }

    /// `cargas`: quais tabelas estao reservadas agora, e por quem.
    pub(super) fn op_cargas(&self) -> Result<Json> {
        let agora = crate::agora_ms();
        let c = self.cargas.tomar("cargas")?;
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(c.quantas() as u64)),
            (
                "cargas",
                Json::Lista(c.todas().iter().map(|r| r.para_json(agora)).collect()),
            ),
        ]))
    }

    pub(super) fn op_inserir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        // Lido ANTES da trava: `se_existir` invalido e erro do pedido, e
        // recusar com a trava global na mao seria segurar todo mundo por causa
        // de uma palavra digitada errada.
        let se_existir = crate::upsert::SeExistir::de_texto(p.texto_ou("se_existir", ""))?;
        // O `atualizar` (o SET do ON CONFLICT) e conferido aqui pelo mesmo
        // motivo: `atualizar` sem `se_existir: "atualizar"` e erro do pedido.
        let atualizar = crate::upsert::atualizar_do_pedido(p, se_existir)?;
        // O portao dos gatilhos, ANTES de qualquer trabalho: sem gatilho no
        // servidor inteiro e um load atomico, e nada mais.
        let (antes, depois) = self.gatilhos_para(p, phxsql_sql::rotina::Evento::Inserir)?;
        Self::conferir_gatilhos_compilam(&antes, &depois)?;
        // Os gatilhos de UPDATE, so quando o upsert PODE virar um. O ramo que
        // ele vira decide qual BEFORE e qual AFTER rodam -- PostgreSQL,
        // MariaDB e MySQL, os tres --, e o gatilho quebrado barra a escrita
        // antes de ela comecar, inclusive o desse ramo. Lidos aqui, antes da
        // trava, como os de cima.
        let (antes_upd, depois_upd) = match se_existir {
            Some(crate::upsert::SeExistir::Atualizar) => {
                self.gatilhos_para(p, phxsql_sql::rotina::Evento::Atualizar)?
            }
            _ => (Vec::new(), Vec::new()),
        };
        Self::conferir_gatilhos_compilam(&antes_upd, &depois_upd)?;
        let mut _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let mut linha = json_para_linha(&valores_json, t.esquema())?;
        // O upsert que virou alteracao com cascata foi pela marca (pedido
        // 540)? Ver `alterar_solto`.
        let mut pela_marca = false;
        let mut aviso_da_cascata: Option<String> = None;
        // BEFORE INSERT ve a linha ja tipada — e o SIGNAL daqui cancela a
        // escrita antes de qualquer byte ir para o disco. Ele roda tambem
        // quando o upsert vai virar atualizacao: e a linha PROPOSTA que ele
        // julga, e nos tres motores ele dispara antes de a chave repetida ser
        // descoberta.
        if !antes.is_empty() {
            self.rodar_gatilhos_antes(&antes, Some(&mut linha), None, t.esquema())?;
        }
        // O UPSERT, e ele e opt-in: sem `se_existir` o `inserir` e o de sempre
        // e RECUSA a chave repetida. Guarda nova entra pedida -- um cliente
        // escrito antes disto nao pode passar a sobrescrever linha nenhuma
        // por conta de um campo que ele nao mandou.
        let feito = match se_existir {
            None => crate::upsert::Feito::inserida(t.inserir(&linha)?),
            Some(modo) => {
                let indice =
                    crate::upsert::indice_do_upsert(t.esquema(), p.texto_ou("indice", ""))?;
                // O BEFORE UPDATE do ramo que o upsert virou roda pelo gancho,
                // sobre a linha MESCLADA e com a trava na mao -- a mesma
                // ordem do `op_atualizar`. Sem o gancho ele nao rodava: o
                // gatilho decidia sobre a linha do VALUES, que com SET nunca
                // vai existir (o gap da G4-MOTOR no pedido 245). O gancho
                // existe quando ha gatilho de UPDATE nesta tabela, BEFORE ou
                // AFTER: e ele que traz o OLD que o AFTER precisa.
                let mut gancho = |nova: &mut Vec<Value>, velha: &[Value], esquema: &Schema| {
                    if antes_upd.is_empty() {
                        return Ok(());
                    }
                    self.rodar_gatilhos_antes(&antes_upd, Some(nova), Some(velha), esquema)
                };
                let gancho: Option<crate::upsert::AntesDeAtualizar<'_>> =
                    if antes_upd.is_empty() && depois_upd.is_empty() {
                        None
                    } else {
                        Some(&mut gancho)
                    };
                let herda_marca = crate::valores::herda_a_marca(&valores_json, t.esquema());
                // O ramo que atualiza grava pela porta da alteracao solta
                // (pedido 540): com filha na chave que muda, a marca vai antes.
                let mut alterar = |t: &mut Table, rowid: u64, nova: &[Value]| {
                    let feita = self.alterar_solto(&mut _trava, t, p, sessao, rowid, nova)?;
                    pela_marca = feita.pela_marca;
                    aviso_da_cascata = feita.aviso;
                    Ok(())
                };
                crate::upsert::aplicar(
                    &mut t,
                    &indice,
                    &linha,
                    modo,
                    atualizar,
                    gancho,
                    herda_marca,
                    &mut alterar,
                )?
            }
        };
        let rowid = feito.rowid;
        if !pela_marca {
            self.gravar_de_verdade(&_trava, &mut t, p)?;
        }
        // A copia em RAM acompanha DENTRO da mesma trava: nao existe instante
        // em que o disco e a memoria discordem. O upsert que ATUALIZOU anota
        // alteracao, e nao insercao -- anotar insercao criaria uma segunda
        // linha em memoria para um rowid que ja estava la. E anota a linha
        // como FICOU: com `atualizar`, e a lida mesclada, nao a do pedido.
        // Pela marca, a passada ja anotou (pedido 540).
        if feito.ignorada || pela_marca {
            // Nada mudou no disco, entao nada muda na memoria.
        } else if feito.atualizada {
            let ficou: &Vec<Value> = feito.gravada.as_ref().unwrap_or(&linha);
            self.residente_mut(p, |m| m.anotar_alteracao(rowid, ficou));
        } else {
            self.residente_mut(p, |m| m.anotar_insercao(rowid, &linha));
        }
        let registros = t.registros();
        // O AFTER e o do ramo que o upsert VIROU: AFTER UPDATE com o OLD
        // quando atualizou, AFTER INSERT quando inseriu, e NENHUM quando
        // ignorou -- nada foi gravado, e um "entrou" na auditoria por uma
        // linha que ja estava la e mentira sobre o dado. E o consenso dos
        // tres motores; antes disto o AFTER INSERT rodava nos tres casos.
        // A cascata que a recuperacao completou nao roda AFTER: a regra do
        // COMMIT, e o aviso diz isso (pedido 540).
        let (depois, velha_json): (&[Arc<crate::rotinas::Gatilho>], Option<Json>) =
            if feito.ignorada || aviso_da_cascata.is_some() {
                (&[], None)
            } else if feito.atualizada {
                (
                    &depois_upd,
                    feito
                        .velha
                        .as_deref()
                        .map(|l| linha_para_json(l, t.esquema())),
                )
            } else {
                (&depois, None)
            };
        // O NEW do AFTER e a linha como FICOU gravada — sequencia preenchida,
        // rownum de verdade — lida de volta ainda dentro da trava.
        let gravada = if depois.is_empty() {
            None
        } else {
            t.ler(rowid)?.map(|l| linha_para_json(&l, t.esquema()))
        };
        drop(t);
        drop(_trava);
        let avisos = self.rodar_gatilhos_depois(depois, gravada, velha_json, p, sessao);
        let mut resposta = vec![
            ("rowid", Json::de_u64(rowid)),
            ("registros", Json::de_u64(registros)),
        ];
        // Os dois campos so aparecem quando ha o que dizer: uma resposta que
        // trouxesse `"ignorada": false` em todo `inserir` mudaria a forma da
        // resposta de todo cliente que existe hoje.
        if feito.ignorada {
            resposta.push(("ignorada", Json::Bool(true)));
        }
        if feito.atualizada {
            resposta.push(("atualizada", Json::Bool(true)));
        }
        if let Some(texto) = aviso_da_cascata {
            resposta.push(("aviso", Json::texto_de(texto)));
        }
        if !avisos.is_empty() {
            resposta.push(("gatilhos_avisos", Json::Lista(avisos)));
        }
        Ok(Json::objeto(resposta))
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
    pub(super) fn op_inserir_lote(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
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

        // O portao dos gatilhos, antes da trava: sem gatilho, um load atomico.
        let (antes, depois) = self.gatilhos_para(p, phxsql_sql::rotina::Evento::Inserir)?;
        Self::conferir_gatilhos_compilam(&antes, &depois)?;

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
            // O BEFORE roda por linha, sobre o valor ja tipado — o MESMO
            // caminho da insercao de uma. Um SIGNAL recusa A LINHA, como um
            // erro de conversao: a posicao entra em `erros` e a carga segue
            // (ou para, se `parar_no_erro` mandou).
            let convertida = convertida.and_then(|mut l| {
                if !antes.is_empty() {
                    self.rodar_gatilhos_antes(&antes, Some(&mut l), None, t.esquema())?;
                }
                Ok(l)
            });
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
        // PEDIDO 686, a decisao do dono do 685 («inteira ou nao chega») na
        // carga: o lote inteiro vai numa tomada so, e a tomada e UMA transacao
        // para a replica. Acima do teto ela chegaria em pedacos, entao e
        // recusada aqui, antes de gravar -- pela mesma conta do COMMIT
        // (`custo_na_transacao`, via `Table::custo_previsto_da_carga`).
        // Partir a carga em transacoes menores seria decidir por quem mandou
        // onde ela pode ficar pela metade; quem manda divide, sabendo.
        let custo = t.custo_previsto_da_carga(&linhas);
        let teto = phxsql_store::log::teto_da_transacao();
        if teto < custo {
            // O IRMAO do COMMIT (pedido 769): a mesma conta, a mesma recusa,
            // o mesmo alarme -- para a replica a carga e UMA transacao.
            crate::telemetria::sinal(
                crate::aquario::Alarme::TransacaoAcimaDoTeto,
                &format!("carga de {custo} bytes, teto {teto}"),
            );
            return Err(PhxError::LimiteExcedido(self.msg(
                "erro.carga_acima_do_teto",
                &[
                    ("linhas", &linhas.len().to_string()),
                    ("bytes", &custo.to_string()),
                    ("teto", &teto.to_string()),
                ],
            )));
        }
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
        // O NEW de cada AFTER e a linha como ficou gravada, lida na trava.
        let mut gravadas = Vec::new();
        if !depois.is_empty() {
            for rowid in &lote.rowids {
                if let Some(l) = t.ler(*rowid)? {
                    gravadas.push(linha_para_json(&l, t.esquema()));
                }
            }
        }
        drop(t);
        drop(_trava);
        let mut avisos = Vec::new();
        for nova in gravadas {
            avisos.extend(self.rodar_gatilhos_depois(&depois, Some(nova), None, p, sessao));
        }
        // O teto e o mesmo dos erros do lote — e corta a LISTA, nunca a
        // execucao: todo AFTER rodou; o que se limita e a resposta.
        avisos.truncate(50);
        let resposta =
            Self::resposta_do_lote(p, &formato, recebidas, &lote.rowids, &recusadas, inicio);
        if avisos.is_empty() {
            return Ok(resposta);
        }
        let Json::Objeto(mut pares) = resposta else {
            return Ok(resposta);
        };
        pares.push(("gatilhos_avisos".to_string(), Json::Lista(avisos)));
        Ok(Json::Objeto(pares))
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
    pub(super) fn op_importar_conferir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
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
                // O texto NAO muda, e a decisao e deliberada. Ele e visivel
                // pelo protocolo, e todo cliente escrito antes desta rodada o
                // le -- e ele continua VERDADE para esta operacao: o
                // `inserir_lote` nao entra em transacao (ver `OPS_EMPILHAVEIS`
                // e a §3.4 do `docs/TRANSACOES.md`), entao quando ele para no
                // meio nao ha transacao nenhuma cobrindo o que ja gravou.
                //
                // Trocar a frase por «esta operacao nao entra em transacao»
                // seria mais preciso e quebraria quem casa o texto -- e a
                // precisao que falta esta no documento, que e onde ela cabe.
                Json::texto_de(if recusadas.is_empty() {
                    ""
                } else {
                    "nao ha transacao: as linhas gravadas antes do erro ficaram gravadas"
                }),
            ),
        ])
    }

    pub(super) fn op_atualizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let (antes, depois) = self.gatilhos_para(p, phxsql_sql::rotina::Evento::Atualizar)?;
        Self::conferir_gatilhos_compilam(&antes, &depois)?;
        let mut _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        // O OLD dos gatilhos: a linha como ela E, lida na mesma trava que vai
        // gravar a nova — nao ha janela entre o que o gatilho ve e o que sai.
        let velha = if antes.is_empty() && depois.is_empty() {
            None
        } else {
            t.ler(rowid)?
        };
        let mut linha = json_para_linha(&valores_json, t.esquema())?;

        // Quem alterou a linha nao mandou a coluna de sistema? Entao ela nao
        // muda. Sem isto, `json_para_linha` preencheria `false` e um
        // `atualizar` de rotina RESSUSCITARIA uma linha excluida -- sem erro
        // nenhum, e sem ninguem perceber ate a linha reaparecer na lista.
        // Sem as externas (pedido 381): a marca mora no `.reg`, e um `.memo`
        // estragado nao pode recusar o `atualizar` que vem substitui-lo.
        if crate::valores::herda_a_marca(&valores_json, t.esquema()) {
            if let Some(atual) = t.ler_sem_externos_na_visao(rowid)? {
                crate::valores::herdar_a_marca(&mut linha, &atual, t.esquema());
            }
        }

        if !antes.is_empty() {
            self.rodar_gatilhos_antes(&antes, Some(&mut linha), velha.as_deref(), t.esquema())?;
        }

        // PEDIDO 540: a alteracao que cascateia grava a MARCA antes -- ver
        // `alterar_solto`. Pela marca, a passada ja levou a janela e a copia
        // residente de cada tabela; sem filha, e o caminho de sempre.
        let feita = self.alterar_solto(&mut _trava, &mut t, p, sessao, rowid, &linha)?;
        if !feita.pela_marca {
            self.gravar_de_verdade(&_trava, &mut t, p)?;
            self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        }
        let aviso = feita.aviso;
        // A versao nova volta na resposta: quem grava duas vezes seguidas
        // continua protegido sem precisar reler a linha inteira no meio.
        let versao = t.versao(rowid)?.unwrap_or(0);
        // A cascata que quebrou no meio e a recuperacao completou nao roda o
        // AFTER -- a mesma regra do COMMIT, e o aviso diz isso.
        let depois = if aviso.is_some() { Vec::new() } else { depois };
        let (gravada, velha_json) = if depois.is_empty() {
            (None, None)
        } else {
            (
                t.ler(rowid)?.map(|l| linha_para_json(&l, t.esquema())),
                velha.as_deref().map(|l| linha_para_json(l, t.esquema())),
            )
        };
        drop(t);
        drop(_trava);
        let avisos = self.rodar_gatilhos_depois(&depois, gravada, velha_json, p, sessao);
        let mut resposta = vec![
            ("rowid", Json::de_u64(rowid)),
            ("versao", Json::de_u64(versao)),
        ];
        // So quando houve: a resposta de sempre nao ganha campo vazio.
        if let Some(texto) = aviso {
            resposta.push(("aviso", Json::texto_de(texto)));
        }
        if !avisos.is_empty() {
            resposta.push(("gatilhos_avisos", Json::Lista(avisos)));
        }
        Ok(Json::objeto(resposta))
    }

    /// **Pedido 540: a alteracao SOLTA de uma linha**, e a porta unica dela
    /// -- o `op_atualizar`, o upsert do `op_inserir` e a sincronia do DbLink
    /// passam por aqui, e nao cada um pelo seu `t.atualizar`.
    ///
    /// Sem filha para levar, grava por `t`, como sempre, e quem chama fecha a
    /// janela. Com filha, e uma transacao de uma instrucao
    /// (`atualizar_com_a_marca`): a passada grava pelos punhos DELA, e `t` e
    /// trocado por um punho novo, que ve o que ela gravou.
    ///
    /// # O `t` de quem chama pode ter escrito (C1 do papel C)
    ///
    /// Pela `op_atualizar` e pelo upsert ele chega limpo; pela sincronia do
    /// DbLink, nao -- ela insere pelo mesmo punho as linhas novas da rodada, e
    /// so depois altera a mae. Por isso, antes da marca, `t` desce ao nucleo
    /// o que so ele tem em RAM ([`Table::descer_ao_nucleo`]): o punho da
    /// passada abre a mae enxergando o que `t` escreveu, e o `Drop` do `t`
    /// velho, na troca, nao tem mais o que gravar por cima da passada.
    pub(super) fn alterar_solto(
        &self,
        trava: &mut TravaMedida<'_>,
        t: &mut Table,
        p: &Json,
        sessao: &Sessao,
        rowid: u64,
        linha: &[Value],
    ) -> Result<AlteracaoSolta> {
        let database = p.texto_ou("database", "").trim();
        // A linha que esta alteracao grava, pelo portao que ela NAO passou: o
        // upsert chega aqui pelo `inserir`, e o portao dele so pergunta pelo
        // fim da tabela (pedido 561, d).
        self.linhas_barradas_para_o_solto(
            sessao,
            database,
            std::iter::once((p.texto_ou("tabela", "").trim(), rowid)),
        )?;
        let (atual, plano) = Self::plano_da_cascata_solta(t, rowid, linha)?;
        // Pedido 496, F8: o plano inteiro, antes da marca e de toda escrita.
        // O irmao e o `planejar_cascata_empilhada`, a mesma pergunta dentro
        // da transacao.
        crate::plano_largo::observar_a_cascata(database, &plano);
        // E as filhas que a cascata grava sem que o pedido as nomeie (561,
        // a-c): antes da marca, com nada gravado.
        self.linhas_barradas_para_o_solto(
            sessao,
            database,
            plano.iter().map(|e| (e.tabela.as_str(), e.rowid)),
        )?;
        if plano.is_empty() {
            // O mesmo plano que o `atualizar` refaria por dentro, e ele saiu
            // vazio: refaze-lo repetiria a varredura das irmas.
            t.atualizar_sem_cascata(rowid, linha)?;
            return Ok(AlteracaoSolta {
                pela_marca: false,
                aviso: None,
            });
        }
        // Antes da marca, e nao depois da passada: com a pagina suja ainda em
        // `t`, o punho da passada acharia o byte 52 em 1 sem atestado e
        // recusaria, a recuperacao reconstruiria a mae pelo `.reg`, e o `Drop`
        // deste `t`, na troca la embaixo, gravaria a arvore VELHA por cima --
        // `buscar` pela chave nova dando 0, medido 5 de 5 pela sincronia.
        // Passar o `t` para a passada nao bastaria: o `completar_marca_em_voo` do
        // braco que quebra no meio abre o punho DELE na mesma mae. Sem
        // `fsync`, e o `t` limpo da `op_atualizar` nao grava nada aqui.
        t.descer_ao_nucleo()?;
        let aviso = self.atualizar_com_a_marca(
            trava,
            sessao,
            database,
            p.texto_ou("tabela", "").trim(),
            rowid,
            linha,
            atual,
            plano,
        )?;
        *t = self.abrir_travada(trava, p, sessao)?;
        Ok(AlteracaoSolta {
            pela_marca: true,
            aviso,
        })
    }

    /// **Pedido 561:** alguma das linhas que a escrita SOLTA vai gravar esta
    /// travada por uma transacao? O `barrado_por_travas` so ve a linha que o
    /// pedido nomeia; a filha da cascata, a linha do upsert e as da sincronia
    /// do DbLink nao estao no pedido, e passavam por cima do X de T1 -- o
    /// COMMIT dele recusava depois, ou apagava a escrita (update perdido).
    ///
    /// Mesma regra do portao: a escrita solta nao espera, recusa com
    /// `EM_TRANSACAO` e `repetir`. Aqui dentro a trava de dados esta na mao, e
    /// toda transacao toma a trava da linha ANTES dela (`empilhar`), entao
    /// nenhuma trava nova nasce entre esta pergunta e a escrita.
    fn linhas_barradas_para_o_solto<'a>(
        &self,
        sessao: &Sessao,
        database: &str,
        linhas: impl Iterator<Item = (&'a str, u64)>,
    ) -> Result<()> {
        #[cfg(test)]
        if self.solto_sem_trava_de_linha.load(Ordering::SeqCst) {
            return Ok(());
        }
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }
        let meu = match self.transacoes.travar().de(sessao.ligacao) {
            Some(tx) => tx.id,
            None => 0,
        };
        let barrada = {
            let travas = self.travas.travar();
            linhas.into_iter().find_map(|(tabela, rowid)| {
                let chave = crate::carga::chave(database, tabela);
                travas.conflito_de_linha(&chave, meu, rowid)
            })
        };
        match barrada {
            Some(b) => {
                let recado = self
                    .transacoes
                    .travar()
                    .recado_da_barrada(&b, crate::agora_ms());
                Err(PhxError::EmTransacao(recado))
            }
            None => Ok(()),
        }
    }

    /// Pedido 540: o plano da cascata de uma alteracao SOLTA, e a linha de
    /// antes dele -- o que decide se a alteracao grava marca.
    ///
    /// E o MESMO plano que o `atualizar` faria por dentro (a mesma
    /// `planejar_cascata_com`, a mesma arvore conferida), feito um passo antes
    /// para caber uma marca entre ele e a primeira escrita. O portao dele e o
    /// do `atualizar`: sem coluna indexada mudando, sai na primeira linha sem
    /// abrir irma nenhuma. Linha que nao existe devolve plano vazio, e o
    /// `atualizar` diz o erro dela.
    fn plano_da_cascata_solta(
        t: &mut Table,
        rowid: u64,
        linha: &[Value],
    ) -> Result<(Vec<Value>, Vec<phxsql_store::table::EscritaDaCascata>)> {
        // Sem carregar as externas: e a leitura do proprio `atualizar`, e a
        // linha com o `.memo` ilegivel continua gravavel como sempre foi.
        let Some(antes) = t.ler_sem_externos(rowid)? else {
            return Ok((Vec::new(), Vec::new()));
        };
        let plano = t.planejar_cascata_da_alteracao(&antes, linha, None)?;
        Ok((antes, plano))
    }

    /// **Pedido 540: a alteracao SOLTA que cascateia e uma transacao de uma
    /// instrucao** -- a marca `.tx` antes, a passada do COMMIT, e a
    /// recuperacao que completa.
    ///
    /// Antes, a cascata solta rodava por dentro do `Table::atualizar`, sem
    /// marca nenhuma: uma queda ou um panico entre a mae e a ultima filha
    /// deixava filha na chave velha, e o `reindexar` -- e, com o 522, o proprio
    /// arranque -- reconstruia o indice dela em silencio, com a orfa dentro.
    /// O 490 so conseguia fazer a tabela RECUSAR ate alguem reindexar.
    ///
    /// Agora a lista -- a mae e cada elo do plano, achatados -- vai para a
    /// marca, sincronizada, ANTES de qualquer escrita, e e aplicada pela MESMA
    /// passada do COMMIT (`passada_sob_a_marca`): o panico e completado pelo
    /// reparo da trava (a marca fica EM VOO, pedido 451), o `SIGKILL` pelo
    /// arranque, e a E/S que quebra no meio pela recuperacao da hora. As
    /// filhas passam pelos punhos da passada, entao entram na janela de
    /// durabilidade como a mae -- e a marca so sai depois do `fsync` delas.
    ///
    /// O preco e um `fsync` de marca por troca de chave COM filha, que e
    /// operacao rara; a alteracao sem filha nao paga nada disto. Os gatilhos
    /// AFTER das filhas nao rodam, e quem decide e a passada, pelo
    /// `elo_da_cascata` (pedido 562) -- o mesmo lugar que decide no COMMIT.
    /// A lista que ela devolve aqui so traz o AFTER da mae, e ele e
    /// descartado porque o `op_atualizar` o roda.
    #[allow(clippy::too_many_arguments)]
    fn atualizar_com_a_marca(
        &self,
        trava: &mut TravaMedida<'_>,
        sessao: &Sessao,
        database: &str,
        tabela: &str,
        rowid: u64,
        linha: &[Value],
        antes: Vec<Value>,
        plano: Vec<phxsql_store::table::EscritaDaCascata>,
    ) -> Result<Option<String>> {
        use crate::transacao::{Acao, Escrita};
        let mut escritas = Vec::with_capacity(1 + plano.len());
        escritas.push(Escrita {
            database: database.to_string(),
            tabela: tabela.to_string(),
            acao: Acao::Atualizar,
            rowid,
            linha: linha.to_vec(),
            linha_antiga: antes,
            motivo: String::new(),
            cascata_na_lista: true,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        });
        for elo in plano {
            escritas.push(Escrita {
                database: database.to_string(),
                tabela: elo.tabela,
                acao: Acao::Atualizar,
                rowid: elo.rowid,
                linha: elo.linha,
                linha_antiga: elo.linha_antiga,
                motivo: String::new(),
                cascata_na_lista: true,
                elo_do_empilhar: false,
                elo_da_cascata: true,
            });
        }
        // A MESMA pre-conferencia do COMMIT, antes da marca (pedido 567): a
        // arvore do plano so conferia restringir, CHECK e profundidade, e a FK
        // da filha para OUTRA mae -- duas chaves na mesma coluna -- so
        // aparecia na passada, depois da mae gravada: `ParouNoMeio` com as
        // filhas orfas na chave velha. Aqui a recusa sai com nada gravado. O
        // plano ja esta achatado na lista, entao elo novo nao nasce; se
        // nascesse, nao ha transacao para travar a linha dele, e ele recusa
        // -- recusa segura, nunca meia cascata.
        let conferida =
            self.pre_conferir_a_lista(trava, database, &mut escritas, sessao, |i, elos| {
                let elo = &elos[0];
                Err(RecusaDaLista {
                    posicao: i,
                    elo: Some((elo.tabela.clone(), elo.rowid)),
                    erro: PhxError::Integridade(format!(
                        "a cascata desta alteracao acha filhas que o plano dela nao \
                         levou ({} rowid {}) -- faca a alteracao numa transacao",
                        elo.tabela, elo.rowid
                    )),
                })
            });
        let elos = conferida.map_err(|recusa| {
            let na_cascata = match &recusa.elo {
                Some((t, r)) => format!(", no elo da cascata que ela leva a {t} rowid {r}"),
                None => String::new(),
            };
            // A lista inteira acima do teto (pedido 685) nao tem escrita
            // culpada: nomeia a mae, que e quem o cliente pediu.
            let w = escritas.get(recusa.posicao).unwrap_or(&escritas[0]);
            com_nota(
                recusa.erro,
                &format!(
                    "a alteracao com cascata foi recusada ANTES da marca ({} rowid \
                         {}{na_cascata}): nada foi gravado, nem a mae nem as filhas",
                    w.tabela, w.rowid
                ),
            )
        })?;
        let escritas = if elos.is_empty() {
            escritas
        } else {
            costurar_os_elos(escritas, elos)
        };
        let dir = trava.abrir_database(database)?.caminho().to_path_buf();
        // Dados antes de transacoes: a ordem unica das travas.
        let id = self.transacoes.travar().numero_de_marca();
        // EM VOO desde ANTES de ir ao disco, como a do COMMIT (451, M1).
        trava.marca_em_voo = Some(MarcaEmVoo {
            database: database.to_string(),
            caminho: crate::transacao::caminho_da_marca(&dir, id),
            gravada: false,
        });
        let marca = match self.gravar_a_marca_da_lista(
            trava,
            database,
            sessao,
            &dir,
            id,
            crate::agora_ms(),
            &escritas,
        ) {
            Ok(c) => c,
            Err(e) => {
                trava.marca_em_voo = None;
                return Err(e);
            }
        };
        if let Some(em_voo) = trava.marca_em_voo.as_mut() {
            em_voo.gravada = true;
        }
        match self.passada_sob_a_marca(
            trava,
            sessao,
            database,
            &escritas,
            &marca,
            NomeDaPassada::DA_CASCATA_SOLTA,
        ) {
            FimDaPassada::Aplicada(_) => Ok(None),
            FimDaPassada::Completada((texto, _)) => Ok(Some(texto)),
            FimDaPassada::NadaAplicado { erro, .. } | FimDaPassada::ParouNoMeio(erro) => Err(erro),
        }
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
    pub(super) fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        let fisico = p.booleano_ou("fisico", false);
        let (antes, depois) = self.gatilhos_para(p, phxsql_sql::rotina::Evento::Excluir)?;
        Self::conferir_gatilhos_compilam(&antes, &depois)?;
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let tem_marca = t.esquema().coluna_softdeleted().is_some();
        // O OLD dos gatilhos. O DELETE dispara nos dois modos — suave e
        // fisico — porque nos dois a linha some da lista de quem consulta.
        let velha = if antes.is_empty() && depois.is_empty() {
            None
        } else {
            t.ler(rowid)?
        };
        if let Some(l) = velha.as_deref().filter(|_| !antes.is_empty()) {
            // Um SIGNAL aqui protege a linha: nada foi tirado ainda. Linha
            // que ja nao existe nao passa por gatilho — nao ha o que proteger.
            self.rodar_gatilhos_antes(&antes, None, Some(l), t.esquema())?;
        }

        let (saiu, modo, na_lixeira, reversivel) = if fisico || !tem_marca {
            let removeu = t.excluir_de_vez(rowid, &motivo)?;
            self.gravar_de_verdade(&_trava, &mut t, p)?;
            if removeu {
                self.residente_mut(p, |m| m.anotar_exclusao(rowid));
            }
            (removeu, "fisico", removeu, false)
        } else {
            let marcou = t.excluir_suave(rowid, &motivo)?;
            self.gravar_de_verdade(&_trava, &mut t, p)?;
            // A copia em RAM tem de esquecer a linha tambem: para quem
            // consulta, marcada e o mesmo que ausente.
            if marcou {
                self.residente_mut(p, |m| m.anotar_exclusao(rowid));
            }
            (marcou, "suave", false, true)
        };
        let velha_json = if saiu && !depois.is_empty() {
            velha.as_deref().map(|l| linha_para_json(l, t.esquema()))
        } else {
            None
        };
        drop(t);
        drop(_trava);
        let avisos = if saiu {
            self.rodar_gatilhos_depois(&depois, None, velha_json, p, sessao)
        } else {
            Vec::new()
        };
        let mut resposta = vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(saiu)),
            ("modo", Json::texto_de(modo)),
            ("na_lixeira", Json::Bool(na_lixeira)),
            ("reversivel", Json::Bool(reversivel)),
        ];
        if !avisos.is_empty() {
            resposta.push(("gatilhos_avisos", Json::Lista(avisos)));
        }
        Ok(Json::objeto(resposta))
    }

    /// Desfaz uma exclusao suave.
    pub(super) fn op_restaurar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        conferir_versao_pedida(&mut t, p, rowid)?;
        let voltou = t.restaurar(rowid, &motivo)?;
        self.gravar_de_verdade(&_trava, &mut t, p)?;
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
    pub(super) fn op_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
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
    pub(super) fn op_motivos(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
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
                    // Pela CHAVE, e nao pelo texto da identidade: o tipo
                    // `expurgo` tem dois donos, e quem os separa e o bit do
                    // registro (pedido 368, condicao C1).
                    ("expurgo_da_trilha", Json::Bool(m.expurgo_da_trilha())),
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

    /// `marcar_lgpd`: classifica colunas como dado pessoal. **Administrador.**
    ///
    /// ```json
    /// {"op":"marcar_lgpd","database":"loja","tabela":"clientes",
    ///  "colunas":{"nome":"pessoal","cpf":"pessoal","laudo":"sensivel",
    ///             "limite_credito":"nao"}}
    /// ```
    ///
    /// # Por que a caixa da tela manda `"pessoal"`, e nao `true`
    ///
    /// A tela mostra uma caixa de marcar, que e o gesto certo para quem esta
    /// cadastrando o campo. Mas o que vai gravado continua sendo o GRAU, e a
    /// caixa e so o atalho: marcar sem dizer mais nada da `pessoal`, e quem
    /// precisa de `sensivel` escolhe ao lado.
    ///
    /// Se a caixa mandasse um booleano, marcar uma coluna ja gravada como
    /// `sensivel` a REBAIXARIA para `pessoal` sem ninguem pedir -- e o
    /// rebaixamento e justamente o que muda o regime legal do campo. Uma
    /// interface nova nao pode apagar em silencio a classificacao que alguem
    /// ja fez.
    pub(super) fn op_marcar_lgpd(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let Some(Json::Objeto(pares)) = p.campo("colunas") else {
            return Err(PhxError::Esquema(
                "informe \"colunas\" como objeto: \
                 {\"colunas\":{\"cpf\":\"pessoal\"}}"
                    .into(),
            ));
        };
        let mut marcas = Vec::with_capacity(pares.len());
        for (nome, valor) in pares {
            // `de_texto` aceita "sim"/"true"/"1" como `pessoal` e "nao" como
            // `nao`, entao a caixa de marcar da tela chega aqui sem tradutor.
            let grau = match valor {
                Json::Bool(b) => {
                    if *b {
                        DadoPessoal::Pessoal
                    } else {
                        DadoPessoal::Nao
                    }
                }
                outro => DadoPessoal::de_texto(outro.texto().unwrap_or(""))?,
            };
            marcas.push((nome.clone(), grau));
        }
        // Pedido 422: a mesma forma do `declarar_fk` -- congela, solta, FASE A
        // fora da trava (a reescrita da tabela gravada antes da v6, quando o
        // bloco nao cabe), retoma, grava. Transacao viva na vizinhanca: quem
        // cede e a declaracao (D5 do 426).
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        if let Some(recado) = self.transacao_na_vizinhanca(
            &dados,
            p.texto_ou("database", ""),
            p.texto_ou("tabela", ""),
            &t,
        )? {
            return Err(PhxError::EmTransacao(recado));
        }
        let posse =
            phxsql_store::congelamento::congelar(t.diretorio(), t.nome(), "marcando dado pessoal")?;
        drop(dados);
        let troca = t.preparar_marcas(&marcas)?;
        // Em bloco, e nao solto: o mapa da trava le `#[cfg(test)]` ate a
        // chave seguinte, e solto ele engolia a secao da trava retomada.
        #[cfg(test)]
        {
            self.rodar_gancho_da_janela();
        }
        let dados = self.travar_dados()?;
        let reescreveu = t.marcar_dado_pessoal_com(&marcas, troca)?;
        t.sincronizar()?;
        drop(posse);
        drop(dados);
        let marcadas: Vec<Json> = t
            .colunas_marcadas()
            .iter()
            .map(|n| Json::texto_de(*n))
            .collect();
        // Pedido 339, item 3b: marcar nao refaz a arvore -- e a decisao do
        // DBA, guarda nova entra pedida, e refazer aqui cobraria uma
        // varredura da tabela inteira sob a trava a quem so declarou. Diz-se
        // o que ficou em claro e o comando que sela.
        let avisos: Vec<Json> = if t.ndx_em_claro_sobre_coluna_marcada() {
            vec![Json::texto_de(
                self.msg("erro.marcar_ndx_em_claro", &[("tabela", t.nome())]),
            )]
        } else {
            Vec::new()
        };
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("alteradas", Json::de_u64(marcas.len() as u64)),
            ("colunas_marcadas", Json::Lista(marcadas)),
            ("avisos", Json::Lista(avisos)),
            // O caminho caro reescreve os volumes; a tela avisa quando foi ele,
            // porque numa tabela grande isso demora e quem clicou merece saber
            // por que. Numa tabela ja v6 e sempre `false`.
            ("arquivos_reescritos", Json::Bool(reescreveu)),
        ]))
    }

    /// `criptografar` / `descriptografar` (pedido 268): leva ao disco, PEDIDO,
    /// o que `cifra.tabelas` so declara. **Administrador.**
    ///
    /// ```json
    /// {"op":"criptografar","database":"loja","tabela":"clientes"}
    /// ```
    ///
    /// # O roteiro e o do `marcar_lgpd`, e e proposital
    ///
    /// Uma funcao para os dois sentidos, e o mesmo motor de troca do 632 por
    /// baixo: a recusa de escopo ANTES de congelar e de gravar byte, a
    /// transacao viva na vizinhanca (`EM_TRANSACAO`: quem cede e a migracao),
    /// o congelamento, a trava global SOLTA durante a FASE A -- a parte cara,
    /// O(linhas) -- e retomada so para a FASE B, que e `rename`. Por isso a
    /// operacao nao entra na conta das secoes que alcancam `fsync` com a
    /// trava na mao: o `sincronizar` que o `marcar_lgpd` faz sob a trava aqui
    /// nao existe, porque a FASE A ja sincronizou cada `*.novo` e a FASE B
    /// troca por `trocar_duravel`.
    ///
    /// # O que a resposta diz em voz alta
    ///
    /// `.log`, `.trash` e `.reason` ficam em claro, e a migracao e LOCAL: nao
    /// replica. O `.ndx` com indice sobre coluna marcada sai SELADO: a FASE A
    /// o monta ao lado, fora da trava, e a FASE B o troca por `rename`
    /// (pedido 339). Fica no campo `avisos`, na lingua do servidor.
    pub(super) fn op_migrar_cifra(&self, p: &Json, sessao: &Sessao, cifrar: bool) -> Result<Json> {
        let inicio = std::time::Instant::now();
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        // O motor decide; daqui sai so a FRASE, pela fabrica.
        if let Err(r) = t.conferir_migracao_da_cifra(cifrar) {
            return Err(PhxError::Esquema(self.recusa_da_migracao(&r, t.nome())));
        }
        if let Some(recado) = self.transacao_na_vizinhanca(
            &dados,
            p.texto_ou("database", ""),
            p.texto_ou("tabela", ""),
            &t,
        )? {
            return Err(PhxError::EmTransacao(recado));
        }
        let posse = phxsql_store::congelamento::congelar(
            t.diretorio(),
            t.nome(),
            if cifrar {
                "criptografando"
            } else {
                "descriptografando"
            },
        )?;
        drop(dados);
        let troca = t.preparar_migracao_da_cifra(cifrar)?;
        // Em bloco, e nao solto: o mapa da trava le `#[cfg(test)]` ate a
        // chave seguinte, e solto ele engolia a secao da trava retomada.
        #[cfg(test)]
        {
            self.rodar_gancho_da_janela();
        }
        let dados = self.travar_dados()?;
        let slots = t.aplicar_migracao_da_cifra(troca)?;
        let registros = t.registros();
        drop(posse);
        drop(dados);
        // Pedido 647: o inode velho morre AQUI, fora da trava global.
        t.soltar_volumes_velhos();
        let ms = inicio.elapsed().as_secs_f64() * 1e3;
        let aviso = self.msg(
            if cifrar {
                "erro.migracao_aviso_criptografou"
            } else {
                "erro.migracao_aviso_descriptografou"
            },
            &[],
        );
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("cifrada", Json::Bool(cifrar)),
            ("versao", Json::de_u64(if cifrar { 5 } else { 4 })),
            ("slots_reescritos", Json::de_u64(slots)),
            ("registros", Json::de_u64(registros)),
            ("ms", Json::Numero(ms)),
            ("avisos", Json::Lista(vec![Json::texto_de(aviso)])),
        ]))
    }

    /// A frase de uma recusa da migracao da cifra, pela fabrica de idiomas.
    /// Por CHAVE, nunca pela comparacao do texto: o motor devolve a razao
    /// como dado justamente para a redacao poder mudar sem quebrar a
    /// traducao.
    fn recusa_da_migracao(&self, r: &phxsql_store::RecusaDaMigracao, tabela: &str) -> String {
        use phxsql_store::RecusaDaMigracao as R;
        match r {
            R::JaCifrada => self.msg("erro.migracao_ja_cifrada", &[("tabela", tabela)]),
            R::JaEmClaro => self.msg("erro.migracao_ja_em_claro", &[("tabela", tabela)]),
            R::ColunaExterna(c) => self.msg("erro.migracao_coluna_externa", &[("coluna", c)]),
            R::IndiceDeTexto(i, c) => self.msg(
                "erro.migracao_indice_de_texto",
                &[("indice", i), ("coluna", c)],
            ),
            R::NadaACifrar => self.msg("erro.migracao_nada_a_cifrar", &[("tabela", tabela)]),
            R::CofreDesligado => self.msg("erro.migracao_cofre_desligado", &[("tabela", tabela)]),
        }
    }

    /// `trilha`: a trilha de LGPD da tabela. **So administrador.**
    ///
    /// # Ela NAO se registra a si mesma
    ///
    /// Ler a trilha e, sem duvida, acessar dado pessoal -- e a pergunta
    /// "quem leu a trilha?" tem de ter resposta. Ela tem, e nao e aqui.
    ///
    /// Registrar a leitura da trilha DENTRO da trilha teria dois defeitos. O
    /// primeiro e a recursao pratica: cada abertura da tela acrescentaria um
    /// registro, que apareceria na proxima abertura, que acrescentaria outro
    /// -- e em pouco tempo a trilha de uma tabela seria majoritariamente a
    /// historia de quem a auditou, com os fatos sobre o DADO afogados no meio.
    /// Uma auditoria que atrapalha a propria leitura nao e auditoria.
    ///
    /// O segundo e que seria o lugar errado. Esta operacao exige
    /// `Administrar`, e **toda** operacao que passa por esta porta ja e
    /// gravada no registro de acessos do servidor, com data, hora, IP, login,
    /// operacao, base, tabela e se deu certo -- e por isso que `op_acessos`
    /// existe. "Quem leu a trilha da tabela X, quando e de onde" se responde
    /// la, que e o arquivo de quem-chamou-o-que. A trilha responde outra
    /// pergunta: o que aconteceu com o DADO. Misturar as duas faria cada uma
    /// responder pior.
    pub(super) fn op_trilha(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 500).max(0) as u64;
        let so_do_rowid = p.campo("rowid").is_some();
        // Filtro por tipo, para a tela poder separar "quem mexeu" de "quem
        // viu" sem trazer as duas listas e jogar metade fora.
        let so_tipo = p.texto_ou("tipo", "").trim().to_lowercase();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let marcadas: Vec<Json> = t
            .colunas_marcadas()
            .iter()
            .map(|n| Json::texto_de(*n))
            .collect();
        let tem_trilha = t.tem_trilha();
        let total = t.total_da_trilha()?;
        // `depois_de` e o cursor estavel da exportacao (pedido 487): o `pular`
        // conta posicao, e um expurgo entre duas paginas faz a contagem
        // deslizar por cima de registros que nunca saem. Os dois juntos nao
        // tem leitura unica -- pular a partir do cursor? -- e a ambiguidade
        // vira recusa em vez de palpite.
        let cursor = match p.campo("depois_de").filter(|v| !matches!(v, Json::Nulo)) {
            None => None,
            Some(v) => {
                if p.campo("pular").is_some() {
                    return Err(PhxError::Esquema(
                        "mande \"depois_de\" ou \"pular\", e nao os dois: o cursor ja diz \
                         onde a pagina comeca"
                            .into(),
                    ));
                }
                let texto = v.texto().unwrap_or("");
                Some(
                    phxsql_core::uuid::Uuid::de_texto(texto.trim()).map_err(|_| {
                        PhxError::Esquema(
                            "\"depois_de\" e o \"uuid\" do ultimo registro da pagina anterior \
                         (o \"proximo\" da resposta)"
                                .into(),
                        )
                    })?,
                )
            }
        };
        let mut cursor_achado = None;
        let lista = if so_do_rowid {
            t.trilha_de(self.rowid(p)?)?
        } else if let Some(c) = &cursor {
            let (l, achou) = t.trilha_depois_de(c, limite)?;
            cursor_achado = Some(achou);
            l
        } else {
            t.trilha(pular, limite)?
        };
        // O cursor da proxima pagina e o ULTIMO LIDO, antes do filtro por
        // tipo: o filtro roda depois do limite, e o ultimo ENTREGUE deixaria
        // a proxima pagina reler o que o filtro jogou fora. Pagina que nao
        // encheu e a ultima, e diz isso com `null`.
        let proximo = match lista.last() {
            Some(e) if !so_do_rowid && limite > 0 && lista.len() as u64 >= limite => {
                Json::texto_de(e.uuid.to_string())
            }
            _ => Json::Nulo,
        };

        let registros: Vec<Json> = lista
            .iter()
            .filter(|e| so_tipo.is_empty() || e.tipo.nome() == so_tipo)
            .map(|e| {
                Json::objeto(vec![
                    ("uuid", Json::texto_de(e.uuid.to_string())),
                    ("quando", Json::texto_de(e.instante_iso())),
                    ("carimbo", Json::de_i64(e.carimbo)),
                    ("tipo", Json::texto_de(e.tipo.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("identidade", Json::texto_de(&e.identidade)),
                    ("coluna", Json::texto_de(&e.coluna)),
                    ("antes", Json::texto_de(&e.antes)),
                    ("depois", Json::texto_de(&e.depois)),
                    // A tela precisa saber que aquele texto e uma MARCA de
                    // redacao, e nao o valor -- senao mostraria "(redigido: 60
                    // bytes)" como se fosse o conteudo do campo.
                    ("antes_redigido", Json::Bool(e.antes_redigido())),
                    ("depois_redigido", Json::Bool(e.depois_redigido())),
                    // E pelo mesmo motivo dos dois de cima que este bit sai
                    // aqui: quem consome o JSON precisa distinguir "o valor
                    // velho nao pode ser lido" de "o valor velho era este
                    // texto". Sem ele, a unica saida seria comparar o texto
                    // com a FRASE de `trilha::INDISPONIVEL` -- e comparacao
                    // por frase quebra calada no dia em que alguem melhorar a
                    // redacao. A tela ja lia o bit; o JSON e que nao o dava.
                    ("antes_indisponivel", Json::Bool(e.antes_indisponivel())),
                    ("linhas", Json::de_u64(e.linhas as u64)),
                    ("ip", Json::texto_de(&e.ip)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                    (
                        "usuario_nome",
                        Json::texto_de(self.nome_do_usuario(e.usuario)),
                    ),
                ])
            })
            .collect();

        let resposta = Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            // A diferenca que a tela precisa para nao mentir: "nao ha arquivo"
            // e "o arquivo esta vazio" sao a mesma tela, e nenhum dos dois e
            // "esta tabela nao tem dado pessoal". Quem responde isso e
            // `colunas_marcadas`.
            ("tem_arquivo", Json::Bool(tem_trilha)),
            ("colunas_marcadas", Json::Lista(marcadas)),
            (
                "alteracoes_ligadas",
                Json::Bool(phxsql_store::trilha::alteracoes_ligadas()),
            ),
            (
                "acessos_ligados",
                Json::Bool(phxsql_store::trilha::acessos_ligados()),
            ),
            ("registros", Json::Lista(registros)),
        ]);
        let Json::Objeto(mut pares) = resposta else {
            return Ok(resposta);
        };
        pares.push(("proximo".to_string(), proximo));
        // So aparece para quem mandou cursor: `false` diz que o registro do
        // cursor foi expurgado e a pagina saiu pela comparacao dos UUIDs --
        // o auditor precisa SABER disso, e nao deduzir.
        if let Some(achou) = cursor_achado {
            pares.push(("cursor_achado".to_string(), Json::Bool(achou)));
        }
        Ok(Json::Objeto(pares))
    }

    /// `esvaziar_lixeira`: daqui nao volta. **So administrador.**
    ///
    /// O expurgo e registrado no `.reason` ANTES de a lixeira ser apagada: o
    /// motivo tem de sobreviver ao dado, senao o rastro some junto com ele.
    pub(super) fn op_esvaziar_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        if motivo.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"motivo\": esvaziar a lixeira nao tem volta, e sem o \
                 registro do por que nao sobra rastro nenhum"
                    .into(),
            ));
        }
        let trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&trava, p, sessao)?;
        let (apagadas, pendente) = t.esvaziar_lixeira_adiando_o_fsync(&motivo);
        let apagadas = apagadas.and_then(|a| t.sincronizar().map(|()| a));
        // Pedido 591: o `fsync` da pasta do `.trash` fora da trava e antes da
        // resposta -- sem ele o dado apagado de vez volta numa queda. E nos
        // DOIS casos (pedido 598): o erro no meio do esvaziar deixa volumes
        // que ja sairam, e eles devem o mesmo `fsync`. A recusa do disco, se
        // vier, fala mais alto que o erro de antes.
        drop(t);
        drop(trava);
        pendente.levar_ao_disco()?;
        let apagadas = apagadas?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("apagadas", Json::de_u64(apagadas)),
        ]))
    }

    /// `expurgar_trilha`: derruba os volumes do `.lgpd` cujo registro MAIS
    /// NOVO e anterior a `ate`. **So administrador.** Pedido 368.
    ///
    /// Decisao do dono, 24/09/2026: «Sim, apos 5 anos pode limpar ou pelo
    /// admin». Esta e a metade «pelo admin»; a outra e o relogio da retencao
    /// ([`Servidor::expurgar_trilhas_vencidas`]). As duas chamam o MESMO motor
    /// ([`Servidor::expurgar_trilha_da_tabela`]) -- dois caminhos para a mesma
    /// decisao seriam o comeco de duas regras.
    ///
    /// # `ate` e obrigatorio, e nao aceita o futuro
    ///
    /// Sem `ate` nao ha limite, e um expurgo sem limite apagaria todo volume
    /// fechado -- ausencia de campo nao pode ser a ordem mais destrutiva que a
    /// operacao sabe dar. E o futuro e recusado, alem da folga de deriva de
    /// relogio que a casa ja admite (`FOLGA_DO_CARIMBO_MS`): nenhum registro
    /// tem carimbo do futuro, entao um `ate` de 2062 compraria o mesmo que o de
    /// agora -- e quem digita 2062 queria 2026. Para derrubar todo volume
    /// fechado, manda-se o instante de agora, escrito.
    pub(super) fn op_expurgar_trilha(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        if motivo.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"motivo\": expurgar a trilha nao tem volta, e sem o \
                 registro do por que nao sobra rastro nenhum"
                    .into(),
            ));
        }
        let Some(limite) = Self::instante_pedido(p, "ate", "ate_ms")? else {
            return Err(PhxError::Esquema(
                "informe \"ate\": sai o volume cujo registro mais novo e anterior \
                 a este instante (2021-09-24, ou 2021-09-24T15:00:00Z; tudo em \
                 UTC). Sem ele nao ha limite -- e sem limite nada se apaga"
                    .into(),
            ));
        };
        let agora = crate::agora_ms();
        if limite > agora + crate::bidirecional::FOLGA_DO_CARIMBO_MS {
            return Err(PhxError::Esquema(format!(
                "\"ate\" esta no futuro ({}): nenhum registro da trilha e de depois \
                 de agora, entao o expurgo so derruba o que ja passou. Para \
                 derrubar todo volume fechado, mande o instante de agora",
                phxsql_core::datahora::instante_iso(limite)
            )));
        }
        let ExpurgoDaTabela {
            expurgo: e,
            restam,
            fechou,
        } = self.expurgar_trilha_da_tabela(
            p,
            sessao,
            limite,
            &motivo,
            agora,
            p.booleano_ou("fechar_ativo", false),
        )?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            (
                "limite",
                Json::texto_de(phxsql_core::datahora::instante_iso(e.limite)),
            ),
            ("limite_ms", Json::de_i64(e.limite)),
            (
                "volumes",
                Json::Lista(
                    e.volumes
                        .iter()
                        .map(|v| {
                            Json::objeto(vec![
                                ("volume", Json::de_u64(v.volume as u64)),
                                ("registros", Json::de_u64(v.registros)),
                                (
                                    "mais_novo",
                                    v.mais_novo
                                        .map(|c| {
                                            Json::texto_de(phxsql_core::datahora::instante_iso(c))
                                        })
                                        .unwrap_or(Json::Nulo),
                                ),
                                ("bytes", Json::de_u64(v.bytes)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("registros", Json::de_u64(e.registros())),
            // Por que parou, pela CHAVE: quem le decide por ela, nunca pela
            // frase. `volume_ativo` ou `fronteira` com `retido_vencido_desde`
            // preenchido quer dizer que ha dado vencido que ESTE expurgo nao
            // alcanca -- ele sai quando o volume fechar -- e isso tem de
            // aparecer, e nao sumir da resposta.
            ("parada", Json::texto_de(e.parada.nome())),
            ("parou_no_volume", Json::de_u64(e.parou_no_volume as u64)),
            (
                "retido_vencido_desde",
                e.retido_vencido_desde
                    .map(|c| Json::texto_de(phxsql_core::datahora::instante_iso(c)))
                    .unwrap_or(Json::Nulo),
            ),
            ("restam", Json::de_u64(restam)),
            // O volume que fechou ANTES do plano -- por idade, ou pelo
            // `fechar_ativo` do pedido. Nulo quando nenhum fechou.
            (
                "fechou",
                fechou.map(|v| Json::de_u64(v as u64)).unwrap_or(Json::Nulo),
            ),
        ]))
    }

    /// O motor UNICO do expurgo da trilha: a op do administrador e o relogio
    /// da retencao chamam isto, e nada mais. Devolve o que saiu e quantos
    /// registros a trilha tem depois.
    ///
    /// # Tres fases, e a do meio sem a trava global
    ///
    /// A ordem e a do `esvaziar_lixeira`: o rastro vai ao `.reason` e ao disco
    /// ANTES do primeiro volume sair -- o motivo tem de sobreviver ao dado.
    /// Aqui o `fsync` do rastro acontece FORA da trava global: ele nao precisa
    /// dela (e um arquivo que so esta funcao ainda vai tocar, levado ao disco
    /// por um descritor proprio, sem mexer no registro de escritas de
    /// ninguem), e segura-la durante um `fsync` e o que a catraca
    /// `alcancam-fsync-2` do mapa da trava existe para impedir.
    ///
    /// O que isso custa: entre as fases o servidor atende outros pedidos. A
    /// fase 3 confere de novo que nenhum volume planejado virou o ativo e que
    /// cada um e o mesmo que foi planejado (`TrilhaFile::apagar_expurgados`),
    /// e um expurgo por vez no processo (`expurgo_da_trilha`) impede o relogio
    /// e o administrador de planejarem o mesmo volume duas vezes.
    #[allow(clippy::too_many_arguments)]
    fn expurgar_trilha_da_tabela(
        &self,
        p: &Json,
        sessao: &Sessao,
        limite: i64,
        motivo: &str,
        agora: i64,
        fechar_ativo: bool,
    ) -> Result<ExpurgoDaTabela> {
        let _um_por_vez = self.expurgo_da_trilha.tomar("expurgo_da_trilha")?;
        // Fase 1, COM a trava: fecha o ativo que passou da idade (ou que o
        // administrador mandou fechar) -- o expurgo nunca derruba o ativo, e
        // e fechando-o que a tabela de pouco movimento chega a ter o que
        // expurgar --, decide o que sai e grava o rastro.
        let (fechou, preparado) = {
            let dados = self.travar_dados()?;
            let mut t = self.abrir_travada(&dados, p, sessao)?;
            let fechou = t.fechar_volume_da_trilha(fechar_ativo, agora)?;
            let e = t.preparar_expurgo_da_trilha(limite, motivo)?;
            if e.volumes.is_empty() {
                let restam = t.total_da_trilha()?;
                return Ok(ExpurgoDaTabela {
                    expurgo: e,
                    restam,
                    fechou,
                });
            }
            (fechou, e)
        };
        // Fase 2, SEM a trava: o rastro vai ao disco.
        let selado = preparado.selar()?;
        // Fase 3, com a trava de novo: os volumes saem.
        let (restam, pendente) = {
            let dados = self.travar_dados()?;
            let mut t = self.abrir_travada(&dados, p, sessao)?;
            let (saiu, pendente) = t.concluir_expurgo_da_trilha_adiando_o_fsync(&selado);
            (saiu.and_then(|_| t.total_da_trilha()), pendente)
        };
        // Fase 4, SEM a trava (pedido 591): o `fsync` da pasta, onde moravam
        // os volumes que sairam. Sem ele o volume vencido volta numa queda,
        // com o rastro selado dizendo que saiu. Tambem no erro da fase 3
        // (pedido 598): os volumes apagados antes dele devem o mesmo `fsync`.
        pendente.levar_ao_disco()?;
        let restam = restam?;
        Ok(ExpurgoDaTabela {
            expurgo: selado.em_expurgo(),
            restam,
            fechou,
        })
    }

    /// O relogio da retencao, uma passada: expurga, tabela por tabela, os
    /// volumes da trilha que o prazo de `lgpd.retencao_anos` venceu. `agora`
    /// vem de fora para o teste poder viver seis anos num segundo.
    ///
    /// # Uma tomada da trava por tabela
    ///
    /// A lista de tabelas sai numa tomada so, e cada tabela e o motor de
    /// sempre ([`Servidor::expurgar_trilha_da_tabela`]), com as tomadas dele.
    /// Segurar a trava pela passada inteira pararia o servidor pelo tempo de
    /// varrer todas as trilhas de todos os bancos.
    ///
    /// # O relatorio diz o que NAO fez
    ///
    /// Tabela com registro vencido que ficou -- porque mora no volume ativo,
    /// ou num de fronteira -- entra pelo nome em `retidas`. Um relogio que so
    /// contasse o que apagou esconderia exatamente o dado que ainda nao pode
    /// sair.
    ///
    /// # O ativo velho fecha ANTES do plano
    ///
    /// O motor fecha, em cada tabela, o ativo cujo primeiro registro passou de
    /// `lgpd.volume_dias` (formato B). «O ativo nunca sai» continua literal: o
    /// que sai e o volume que ACABOU de fechar, na mesma passada, se o registro
    /// mais novo dele ja passou do prazo.
    pub(super) fn expurgar_trilhas_vencidas(&self, agora: i64) -> Result<RetencaoDaTrilha> {
        let anos = self.config.lgpd.retencao_anos;
        let mut r = RetencaoDaTrilha::default();
        if anos == 0 {
            return Ok(r);
        }
        let limite = phxsql_core::datahora::recuar_anos(agora, anos);
        r.limite = limite;
        let motivo = format!("expurgo automatico: retencao de {anos} ano(s) (lgpd.retencao_anos)");
        let tabelas: Vec<(String, String)> = {
            let dados = self.travar_dados()?;
            let mut v = Vec::new();
            for db in dados.databases()? {
                match dados.abrir_database(&db).and_then(|b| b.todas_as_tabelas()) {
                    Ok(lista) => v.extend(lista.into_iter().map(|t| (db.clone(), t))),
                    Err(e) => r.falhas.push(format!("{db}: {e}")),
                }
            }
            v
        };
        // Sem login e sem IP, e e a verdade: quem pede e o proprio servidor.
        // O `.reason` grava usuario 0, que e o que ja quer dizer «servico».
        let sessao = Sessao::default();
        for (db, tab) in tabelas {
            let p = Json::objeto(vec![
                ("database", Json::texto_de(&db)),
                ("tabela", Json::texto_de(&tab)),
            ]);
            match self.expurgar_trilha_da_tabela(&p, &sessao, limite, &motivo, agora, false) {
                Ok(ExpurgoDaTabela {
                    expurgo: e, fechou, ..
                }) => {
                    r.fechados += u64::from(fechou.is_some());
                    if e.parada != phxsql_store::trilha::Parada::SemTrilha {
                        r.com_trilha += 1;
                    }
                    r.volumes += e.volumes.len() as u64;
                    r.registros += e.registros();
                    if e.retido_vencido_desde.is_some() {
                        r.retidas.push(format!("{db}.{tab} ({})", e.parada.nome()));
                    }
                }
                Err(e) => r.falhas.push(format!("{db}.{tab}: {e}")),
            }
        }
        Ok(r)
    }

    /// Sobe o relogio da retencao da trilha, se `lgpd.retencao_anos` pedir.
    ///
    /// O portao vem ANTES do trabalho: com o prazo em zero nao ha thread, nem
    /// uma que acorde para descobrir que nao tem o que fazer.
    ///
    /// Uma passada por dia, e a primeira um minuto depois do arranque -- nao
    /// no arranque, que e quando a recuperacao das transacoes e a primeira
    /// leva de clientes disputam a trava. Acorda de minuto em minuto e
    /// compara o relogio, pelo mesmo motivo do backup agendado: dormir um dia
    /// inteiro seria fragil, porque a maquina suspende e o relogio anda.
    pub(super) fn subir_retencao_da_trilha(self: &Arc<Self>) {
        let anos = self.config.lgpd.retencao_anos;
        if anos == 0 {
            return;
        }
        eprintln!(
            "retencao da trilha LGPD: uma vez por dia saem os volumes do .lgpd \
             cujo registro mais novo passou de {anos} ano(s); o volume ativo nunca"
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "retencao-trilha",
            "uma vez por dia, fecha o ativo do .lgpd que passou de \
             lgpd.volume_dias e derruba os volumes fechados cujo registro mais \
             novo passou do prazo de lgpd.retencao_anos -- volume inteiro, nunca \
             o ativo, com o rastro no .reason antes",
            "servico",
            crate::agora_ms(),
            move |fio| {
                // O INTERVALO sai do relogio monotonico, e so o LIMITE do
                // prazo sai do relogio de parede (achado A6 do parecer do
                // DBA). Um ajuste de hora para tras de um dia nao pode calar
                // a passada, nem um para a frente fazer duas no mesmo dia.
                let mut ultima: Option<Instant> = None;
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                    if ultima.is_some_and(|u| u.elapsed() < UM_DIA) {
                        fio.fazendo("esperando a passada do dia");
                        continue;
                    }
                    ultima = Some(Instant::now());
                    fio.fazendo("varrendo as trilhas vencidas");
                    match servidor.expurgar_trilhas_vencidas(crate::agora_ms()) {
                        Ok(r) => eprintln!("{}", r.resumo()),
                        Err(e) => eprintln!("retencao da trilha FALHOU: {e}"),
                    }
                }
            },
        );
    }
}

/// De quanto em quanto o relogio da retencao da trilha faz uma passada.
///
/// Um dia, e nao menos: o prazo e contado em ANOS, e o volume que venceu
/// hoje as 10h e o mesmo que vence amanha as 10h a menos de um dia de
/// diferenca num prazo de cinco anos. Mais passadas so pagariam mais
/// varreduras do volume de fronteira pelo mesmo resultado.
const UM_DIA: Duration = Duration::from_secs(24 * 60 * 60);

/// O que o motor do expurgo da trilha fez numa tabela.
struct ExpurgoDaTabela {
    expurgo: phxsql_store::trilha::Expurgo,
    /// Registros que a trilha tem depois.
    restam: u64,
    /// O volume ativo que fechou antes do plano, se algum.
    fechou: Option<u32>,
}

/// O relatorio de uma passada do relogio da retencao.
#[derive(Debug, Default)]
pub(super) struct RetencaoDaTrilha {
    /// O limite desta passada: saiu o volume cujo registro mais novo e
    /// anterior a ele.
    limite: i64,
    /// Tabelas que tem `.lgpd`.
    pub(super) com_trilha: u64,
    /// Volumes ativos que fecharam por idade nesta passada.
    pub(super) fechados: u64,
    pub(super) volumes: u64,
    pub(super) registros: u64,
    /// Tabelas com registro VENCIDO que ficou, com a causa pela chave.
    pub(super) retidas: Vec<String>,
    pub(super) falhas: Vec<String>,
}

impl RetencaoDaTrilha {
    /// Uma linha para o log do servidor -- e ela diz o que NAO fez tambem.
    pub(super) fn resumo(&self) -> String {
        let mut s = format!(
            "retencao da trilha: {} volume(s) e {} registro(s) expurgados em {} \
             tabela(s) com trilha, {} ativo(s) fechado(s) por idade (limite {})",
            self.volumes,
            self.registros,
            self.com_trilha,
            self.fechados,
            phxsql_core::datahora::instante_iso(self.limite)
        );
        if !self.retidas.is_empty() {
            s.push_str(&format!(
                "; {} tabela(s) guardam registro VENCIDO que nao sai sem cortar volume ao \
                 meio: {}",
                self.retidas.len(),
                self.retidas.join(", ")
            ));
        }
        if !self.falhas.is_empty() {
            s.push_str(&format!(
                "; {} falha(s): {}",
                self.falhas.len(),
                self.falhas.join(" | ")
            ));
        }
        s
    }
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
pub(super) fn conferir_versao_pedida(
    t: &mut Table,
    p: &Json,
    rowid: phxsql_core::RowId,
) -> Result<()> {
    let esperada = p.inteiro_ou("versao", 0).max(0) as u64;
    if esperada == 0 {
        return Ok(());
    }
    t.conferir_versao(rowid, esperada)
}
