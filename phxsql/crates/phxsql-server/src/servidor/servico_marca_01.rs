//! A marca: aplicar o conjunto, as sujas, os sinais de parar e o reparo da
//! trava.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// O nome da tabela sem o esquema qualificador: `vendas.clientes` vira
/// `clientes`. A chave estrangeira carrega o nome qualificado; o handle no mapa
/// da passada de commit e o da recuperacao mora pela chave que o pedido usou.
pub(crate) fn nome_simples_da_tabela(qualificado: &str) -> &str {
    qualificado.rsplit_once('.').map_or(qualificado, |(_, t)| t)
}

/// A recusa da pre-conferencia do COMMIT: a posicao da escrita do cliente
/// (a partir de zero), o elo da cascata dela quando foi ele que recusou, e o
/// erro.
pub(super) struct RecusaDaLista {
    pub(super) posicao: usize,
    pub(super) elo: Option<(String, u64)>,
    pub(super) erro: PhxError,
}

/// O que os gatilhos AFTER de cada escrita da passada precisam saber: o
/// pedido da tabela, o evento, a linha nova e a velha.
type DepoisDeRodar = Vec<(Json, phxsql_sql::rotina::Evento, Option<Json>, Option<Json>)>;

/// Como termina a passada de uma lista com a marca no disco -- ver
/// [`Servidor::passada_sob_a_marca`]. Cada braco e uma linha da tabela de
/// desfechos do [`Servidor::depois_da_marca`], e o que cada chamador faz
/// com ele (a lista da transacao devolvida, a resposta) fica com ele.
pub(super) enum FimDaPassada {
    /// A lista inteira foi aplicada; a marca saiu, ou ficou pendente da
    /// janela de durabilidade.
    Aplicada(DepoisDeRodar),
    /// Quebrou no meio e a recuperacao a completou, ou a deixou pendente:
    /// `(aviso, pendente)`.
    Completada((String, bool)),
    /// Nada chegou ao disco, e a marca saiu. `de_acesso`: a quebra foi de
    /// acesso (4xxx), e repetir adianta.
    NadaAplicado { erro: PhxError, de_acesso: bool },
    /// Erro do dado com parte ja gravada: a marca saiu, e o erro diz quantas
    /// ficaram -- e defeito do motor.
    ParouNoMeio(PhxError),
}

/// O nome da passada nas frases da resposta: o COMMIT de uma transacao, ou a
/// alteracao solta que cascateia (pedido 540). A passada e a mesma, e as
/// frases sao duas porque «NAO repita a transacao» para quem nao abriu
/// transacao nenhuma mandaria procurar o que nao existe.
#[derive(Clone, Copy)]
pub(super) struct NomeDaPassada {
    /// «o COMMIT parou na escrita...»
    quem: &'static str,
    /// «a passada de COMMIT quebrou...»
    passada: &'static str,
    /// «a transacao ESTA ...»
    unidade: &'static str,
    /// «... ESTA confirmada»
    feita: &'static str,
    /// O que foi conferido antes da marca, na frase do `ParouNoMeio`. O
    /// COMMIT tem a pre-conferencia da lista inteira; a cascata solta, so a
    /// arvore do plano -- e dizer «a conferencia tinha aprovado esta lista»
    /// para ela seria afirmar uma conferencia que nao aconteceu (C2 do papel
    /// C, pedido 540).
    conferida: &'static str,
}

impl NomeDaPassada {
    const DO_COMMIT: NomeDaPassada = NomeDaPassada {
        quem: "o COMMIT",
        passada: "a passada de COMMIT",
        unidade: "transacao",
        feita: "confirmada",
        conferida: "A conferencia de antes da marca tinha aprovado esta lista: isto e \
                    DEFEITO DO MOTOR, e vale reportar",
    };
    pub(super) const DA_CASCATA_SOLTA: NomeDaPassada = NomeDaPassada {
        quem: "a alteracao com cascata",
        passada: "a cascata da alteracao",
        unidade: "alteracao",
        feita: "gravada",
        // O remedio e o medido pelo papel C na re-checagem do C1 (P5,
        // `parou2fk`): voltar a mae a chave velha leva junto as filhas que ja
        // tinham ido, e sobra `clientes [5]`, `pedidos [5, 5]` -- ninguem
        // orfao. A frase de antes do 540 dizia so «conserte-a antes de
        // seguir», sem dizer como.
        // Desde o 567 a cascata solta passa pela MESMA pre-conferencia do
        // COMMIT (FK, arvore e unicidade), entao chegar aqui e defeito do motor.
        conferida: "A conferencia de antes da marca tinha aprovado esta lista (FK, \
                    arvore da cascata e unicidade): isto e DEFEITO DO MOTOR. As filhas que ficaram na chave velha estao ORFAS \
                    quando essa chave era so desta mae: volte a mae a chave velha, que \
                    as reencontra e leva de volta as que ja tinham ido, ou aponte as \
                    que faltam para a chave nova. Isto vale reportar",
    };
}

/// So em `debug`: `PHXSQL_TESTE_PARAR_NO_COMMIT=N`, lido UMA vez -- a prova
/// do pedido 702 (`tests/commit-inteiro-na-queda.rs`).
#[cfg(debug_assertions)]
fn parar_no_commit_de_teste() -> Option<u64> {
    static N: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("PHXSQL_TESTE_PARAR_NO_COMMIT")
            .ok()
            .and_then(|v| v.parse().ok())
    })
}

impl Servidor {
    /// O que o `COMMIT` confere com a trava na mao e ANTES da marca: o
    /// diretorio dela, e se alguma tabela que a passada vai abrir esta
    /// congelada (a rede do pedido 426 -- ver `congelada_no_alcance`).
    pub(super) fn preparar_a_marca(
        &self,
        trava: &Instancia,
        database: &str,
        escritas: &[crate::transacao::Escrita],
    ) -> Result<PathBuf> {
        let dir = trava.abrir_database(database)?.caminho().to_path_buf();
        if phxsql_store::congelamento::quantas() == 0 {
            return Ok(dir);
        }
        // Por schema, porque a chave estrangeira nao atravessa diretorio: o
        // componente de uma tabela mora no diretorio dela.
        let mut por_schema: std::collections::BTreeMap<Option<String>, Vec<String>> =
            std::collections::BTreeMap::new();
        for e in escritas {
            let (schema, nome) = phxsql_store::catalogo::separar_qualificado(&e.tabela);
            let nomes = por_schema.entry(schema).or_default();
            if !nomes.contains(&nome) {
                nomes.push(nome);
            }
        }
        let recusa = |recado: String| {
            PhxError::EmMigracao(format!(
                "{recado} -- e o COMMIT abriria essa tabela. NADA foi gravado e a \
                 transacao continua ativa: mande COMMIT de novo quando a reescrita \
                 terminar, ou ROLLBACK"
            ))
        };
        let db = trava.abrir_database(database)?;
        for (schema, nomes) in por_schema {
            // O diretorio RESOLVIDO sai de uma tabela aberta -- a primeira da
            // lista, que a passada abre de qualquer jeito. Congelada, ela
            // recusa ja aqui, e com a MESMA frase das vizinhas.
            let qualificada = phxsql_store::catalogo::qualificar(schema.as_deref(), &nomes[0]);
            let t = match db.abrir_qualificada(&qualificada) {
                Ok(t) => t,
                Err(PhxError::EmMigracao(m)) => return Err(recusa(m)),
                Err(e) => return Err(e),
            };
            if let Some(recado) = self.congelada_no_alcance(
                trava,
                database,
                schema.as_deref(),
                t.diretorio(),
                &nomes,
            )? {
                return Err(recusa(recado));
            }
        }
        Ok(dir)
    }

    /// **A pre-conferencia do COMMIT (pedido 448):** a lista inteira, na
    /// ordem, ANTES da marca -- a chave estrangeira nos dois sentidos, a arvore
    /// do `ao_alterar` e a unicidade. Devolve a POSICAO (a partir de zero) da
    /// primeira escrita recusada, junto do erro.
    ///
    /// # Por que ela existe, se a passada ja confere
    ///
    /// Porque a passada confere DEPOIS da marca, e ali recusa nao desfaz: a
    /// ordem de digitacao proibe devolver slot, entao `[mae, filha-orfa,
    /// outra]` saia com a mae gravada e o resto nao -- medido antes do
    /// conserto, com a resposta dizendo «as 1 anteriores JA ESTAO gravadas».
    /// Fere o D2 do parecer do 426: a transacao confirmada e inteira ou nao e.
    /// A passada continua conferindo, como cinto.
    ///
    /// # A visibilidade e de PREFIXO, e e ela que segura a petrea
    ///
    /// A escrita i ve o disco mais as escritas 0..i-1, e nenhuma das que vem
    /// depois: cada uma so entra na sobreposicao do handle dela DEPOIS de
    /// conferida. E isso que mantem «filho antes do pai» recusado sem
    /// `DEFERRABLE` -- a filha da posicao 1 nao ve a mae da posicao 2 --, e e
    /// isso que deixa a exclusao da mae na posicao 2 ver a filha nascida na 1.
    ///
    /// # Uma guarda, e nao duas
    ///
    /// Cada escrita passa pela `Table::pre_conferir`, que chama as MESMAS
    /// guardas que a passada chama, e pela `conferir_unicidade`, a mesma do
    /// `empilhar`. O que so existe aqui e o que so a LISTA sabe: a cascata que
    /// ja esta nela tem de cobrir o plano refeito, e a que a passada fara por
    /// conta propria nao pode cair numa tabela que a propria lista escreveu.
    ///
    /// # Quem trava o elo novo
    ///
    /// `travar` recebe cada grupo de elos que a lista ganhou, antes de ele
    /// se conferir. O COMMIT passa [`Self::travar_os_elos`]; a cascata solta
    /// (pedido 567), que nao tem transacao, passa uma recusa -- o plano dela
    /// ja esta achatado na lista, e elo novo ali e plano que o motor nao
    /// encadeou. Ficar FORA daqui tambem tira a espera de trava do caminho da
    /// alteracao solta, que a catraca `rede-ou-espera-2` do mapa da trava ve.
    pub(super) fn pre_conferir_a_lista(
        &self,
        trava: &Instancia,
        database: &str,
        escritas: &mut [crate::transacao::Escrita],
        sessao: &Sessao,
        mut travar: impl FnMut(
            usize,
            &[crate::transacao::Escrita],
        ) -> std::result::Result<(), RecusaDaLista>,
    ) -> std::result::Result<Vec<(usize, Vec<crate::transacao::Escrita>)>, RecusaDaLista> {
        use crate::transacao::Acao;
        let recusa = |posicao: usize, elo: Option<(String, u64)>, erro: PhxError| RecusaDaLista {
            posicao,
            elo,
            erro,
        };
        // Onde cada linha e reescrita pela lista, pela ULTIMA vez. A posicao e
        // `(i, k)`: `k` zero e a escrita i do cliente, e `k` > 0 o k-esimo elo
        // que a pre-conferencia pos depois dela. «Alguma escrita DEPOIS desta
        // reescreve a filha» e uma consulta aqui, e nao uma volta na lista.
        let mut reescrita_em: HashMap<(String, u64), (usize, usize)> = HashMap::new();
        for (j, e) in escritas.iter().enumerate() {
            if matches!(e.acao, Acao::Atualizar | Acao::ExcluirDeVez) {
                let nome = nome_simples_da_tabela(&e.tabela).to_ascii_lowercase();
                reescrita_em.insert((nome, e.rowid), (j, 0));
            }
        }
        let mut abertas: HashMap<String, Table> = HashMap::new();
        let mut elos_da_lista = Vec::new();
        for (i, e) in escritas.iter_mut().enumerate() {
            // O elo do `empilhar` se refaz sobre a linha ATUAL -- o disco com o
            // prefixo que a lista ja escreveu -- ANTES de se conferir e de ir
            // para a marca (pedido 537). Aqui, e nao na passada, porque e o
            // unico ponto em que «atual» e estavel (a trava de dados esta na
            // mao) e que a marca ainda vai herdar: a passada e a recuperacao
            // aplicam a linha refeita, e a marca nao muda de formato.
            if e.elo_do_empilhar {
                self.refazer_o_elo(trava, database, sessao, &mut abertas, e)
                    .map_err(|erro| recusa(i, None, erro))?;
            }
            let e = &*e;
            let plano = self
                .pre_conferir_uma(
                    trava,
                    database,
                    sessao,
                    &mut abertas,
                    e,
                    (i, 0),
                    &reescrita_em,
                )
                .map_err(|erro| recusa(i, None, erro))?;
            if plano.is_empty() {
                continue;
            }
            // Os elos que faltam entram logo depois da escrita que os puxou,
            // na ordem do plano -- pai antes de filha --, e cada um passa pela
            // MESMA conferencia, com o prefixo que ja o inclui.
            let elos: Vec<crate::transacao::Escrita> = plano
                .into_iter()
                .map(|elo| crate::transacao::Escrita {
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
                })
                .collect();
            // O elo trava a linha dele ANTES de se conferir (pedido 516) -- e
            // quem trava e quem chama, pelo `travar`: so o COMMIT tem
            // transacao para travar em nome dela.
            travar(i, &elos)?;
            for (k, elo) in elos.iter().enumerate() {
                let chave = (elo.tabela.to_ascii_lowercase(), elo.rowid);
                let aqui = (i, k + 1);
                let vale = reescrita_em.get(&chave).map_or(aqui, |&j| j.max(aqui));
                reescrita_em.insert(chave, vale);
            }
            for (k, elo) in elos.iter().enumerate() {
                let mais = self
                    .pre_conferir_uma(
                        trava,
                        database,
                        sessao,
                        &mut abertas,
                        elo,
                        (i, k + 1),
                        &reescrita_em,
                    )
                    .map_err(|erro| recusa(i, Some((elo.tabela.clone(), elo.rowid)), erro))?;
                // O plano que puxou este elo ja desceu a arvore inteira com o
                // prefixo; um elo que ainda ache filha por levar e um plano
                // que o motor nao soube encadear -- e isso se diz, em vez de
                // gravar meia cascata.
                if !mais.is_empty() {
                    return Err(recusa(
                        i,
                        Some((elo.tabela.clone(), elo.rowid)),
                        PhxError::Integridade(format!(
                            "a cascata desta alteracao desce por {} rowid {} e acha \
                             filhas que o plano de cima nao levou ({} rowid {}) -- o \
                             motor nao encadeia esse caminho; faca as alteracoes em \
                             transacoes separadas, a da mae primeiro",
                            elo.tabela, elo.rowid, mais[0].tabela, mais[0].rowid
                        )),
                    ));
                }
            }
            elos_da_lista.push((i, elos));
        }
        // PEDIDO 685, decisao do dono: a transacao que a replica nao
        // aplicaria inteira e recusada AQUI, antes da marca, com nada
        // gravado. A conta e a soma do que cada escrita -- elos da cascata
        // inclusive, que passaram pela mesma `pre_conferir` -- custara no
        // diario, com o MESMO teto e o mesmo custo por evento que o
        // `replica::Juntador` usa (`phxsql_store::log`). Uma conta so,
        // nenhuma releitura: cada tabela somou a dela ao conferir.
        let custo: usize = abertas
            .values()
            .map(Table::custo_previsto_no_diario)
            .fold(0usize, usize::saturating_add);
        let teto = phxsql_store::log::teto_da_transacao();
        if custo > teto {
            return Err(recusa(
                escritas.len(),
                None,
                PhxError::LimiteExcedido(self.msg(
                    "erro.transacao_acima_do_teto",
                    &[("bytes", &custo.to_string()), ("teto", &teto.to_string())],
                )),
            ));
        }
        Ok(elos_da_lista)
    }

    /// O elo que o COMMIT acrescenta escreve numa linha que a transacao pode
    /// nunca ter travado -- a filha que nasceu na chave velha depois do
    /// `empilhar` --, e passava por cima da leitura repetivel de outra
    /// transacao (pedido 516): T3 lia 5 e relia 6. Ele trava pelo mesmo
    /// caminho de toda escrita da transacao, sem esperar, porque a trava de
    /// dados esta na mao; quem ja tinha a trava (a linha que a lista escreveu)
    /// passa pela idempotencia do gestor.
    pub(super) fn travar_os_elos(
        &self,
        sessao: &Sessao,
        database: &str,
        i: usize,
        elos: &[crate::transacao::Escrita],
    ) -> std::result::Result<(), RecusaDaLista> {
        for elo in elos {
            let recusa = |erro| RecusaDaLista {
                posicao: i,
                elo: Some((elo.tabela.clone(), elo.rowid)),
                erro,
            };
            let chave = crate::carga::chave(database, &elo.tabela);
            let barrada = self
                .travar_para_escrever(sessao, &chave, elo.rowid, Espera::Nenhuma)
                .map_err(recusa)?;
            if let Some(b) = barrada {
                return Err(recusa(self.elo_barrado(sessao, elo, &b)));
            }
        }
        Ok(())
    }

    /// O erro do COMMIT cujo elo esbarrou na trava de `b` -- e o desempate
    /// quando os dois COMMITs se barram (condicao C1 do papel C ao 516).
    ///
    /// Sem ciclo, e a espera comum: `EM_TRANSACAO`, `repetir: true`, nada
    /// gravado e a transacao ativa. Com ciclo, a MAIS NOVA cede --
    /// `TRANSACAO_ABORTADA`, `repetir: false` -- e a mais velha continua
    /// mandada repetir: na proxima vez ela passa, porque o `op_commit` solta
    /// as travas da que cedeu antes de responder. Medido pelo papel C sem o
    /// desempate: 1.870 rodadas em 11 s com os dois mandados repetir, e
    /// ninguem saia.
    fn elo_barrado(
        &self,
        sessao: &Sessao,
        elo: &crate::transacao::Escrita,
        b: &crate::travas::Barrada,
    ) -> PhxError {
        let mut reg = self.transacoes.travar();
        let recado = reg.recado_da_barrada(b, crate::agora_ms());
        let onde = format!(
            "a cascata desta transacao leva o elo a {} rowid {}, que ela nao tinha \
             travado -- {recado}",
            elo.tabela, elo.rowid
        );
        match reg.commit_barrado(sessao.ligacao, b.transacao) {
            Some(c) if c.ceder => PhxError::TransacaoAbortada(format!(
                "{onde}; e o COMMIT dela tambem esta barrado por esta: ciclo com a \
                 transacao {}. Esta e a mais nova e cede -- NADA foi gravado, as \
                 travas dela ja sairam. Mande ROLLBACK",
                c.outra
            )),
            Some(c) => PhxError::EmTransacao(format!(
                "{onde}; ciclo com a transacao {}, que e a mais nova e cede no \
                 COMMIT dela. NADA foi gravado e esta transacao continua ativa: \
                 mande COMMIT de novo",
                c.outra
            )),
            None => PhxError::EmTransacao(format!(
                "{onde}. O COMMIT nao espera trava com a trava de dados na mao: NADA \
                 foi gravado e a transacao continua ativa -- mande COMMIT de novo \
                 quando a outra terminar, ou ROLLBACK"
            )),
        }
    }

    /// A tabela `tabela` aberta no mapa da pre-conferencia -- uma vez por
    /// COMMIT, e achada pelo nome SIMPLES, que e como o elo a nomeia. Devolve
    /// a chave dela no mapa. Uma porta para quem confere e para quem refaz o
    /// elo: duas buscas no mapa seriam dois jeitos de achar -- ou de abrir
    /// de novo, por um segundo descritor -- a mesma tabela.
    fn aberta_na_pre_conferencia(
        &self,
        trava: &Instancia,
        database: &str,
        sessao: &Sessao,
        abertas: &mut HashMap<String, Table>,
        tabela: &str,
    ) -> Result<String> {
        let chave = abertas
            .keys()
            .find(|k| {
                nome_simples_da_tabela(k).eq_ignore_ascii_case(nome_simples_da_tabela(tabela))
            })
            .cloned();
        Ok(match chave {
            Some(k) => k,
            None => {
                let ped = pedido_da_tabela(database, tabela);
                let t = self.abrir_travada_sem_sobrepor(trava, &ped, sessao)?;
                abertas.insert(tabela.to_string(), t);
                tabela.to_string()
            }
        })
    }

    /// Pedido 537: o elo planejado no `empilhar` refeito sobre a linha ATUAL.
    ///
    /// O elo traz a filha como ela estava no `empilhar`, com a chave nova. Se
    /// outra conexao mudou uma coluna dela depois -- solta, em transacao, ou
    /// pela cascata SOLTA de outra mae dela, que ate o 561 nao perguntava por
    /// trava de linha e hoje e o cinto disto --, regravar a linha inteira apagava a mudanca: `x=1` voltava a
    /// `x=0`, o update perdido medido pelo papel C. Aqui so a CHAVE viaja:
    /// cada coluna que o elo muda (a diferenca entre `linha` e
    /// `linha_antiga`) entra sobre a linha atual, e o resto fica como esta.
    ///
    /// E so entra onde a filha ainda tem o valor de antes: a coluna que outro
    /// ja tirou da chave velha nao e mais desta cascata, e o elo nao a arrasta
    /// de volta -- a mesma regra do `ON UPDATE CASCADE`, que leva as linhas
    /// que apontam para a chave velha, e nao as que apontavam.
    ///
    /// A filha que ja nao existe fica como veio: a conferencia logo adiante
    /// recusa com o nome dela, em vez de este passo inventar um desfecho.
    fn refazer_o_elo(
        &self,
        trava: &Instancia,
        database: &str,
        sessao: &Sessao,
        abertas: &mut HashMap<String, Table>,
        e: &mut crate::transacao::Escrita,
    ) -> Result<()> {
        let chave = self.aberta_na_pre_conferencia(trava, database, sessao, abertas, &e.tabela)?;
        let t = abertas
            .get_mut(&chave)
            .expect("a tabela acabou de ser achada ou inserida no mapa");
        let Some(atual) = t.ler(e.rowid)? else {
            return Ok(());
        };
        let mut refeita = atual.clone();
        for (c, novo) in e.linha.iter().enumerate() {
            let (Some(antigo), Some(agora)) = (e.linha_antiga.get(c), atual.get(c)) else {
                continue;
            };
            if novo != antigo && agora == antigo {
                refeita[c] = novo.clone();
            }
        }
        e.linha = refeita;
        e.linha_antiga = atual;
        Ok(())
    }

    /// Uma escrita da pre-conferencia: abre a tabela dela (uma vez por
    /// COMMIT), confere pela `Table::pre_conferir` -- que a poe no prefixo --
    /// e devolve os elos da cascata que a lista ainda nao leva.
    ///
    /// # O que decide se um elo falta, ou se a escrita recusa
    ///
    /// O plano sai contra o disco E o prefixo. Para a alteracao empilhada
    /// SEM cascata (`cascata_na_lista` falso), o plano inteiro falta: a
    /// passada nao planeja mais nada, e o que a pre-conferencia nao puser na
    /// lista nao acontece. Para a que JA leva a cascata, cada filha do plano
    /// ou e reescrita adiante pela propria lista, ou foi escrita pela lista e
    /// o `empilhar` nao a viu (o plano dele e so do disco) -- e ai o elo dela
    /// falta --, ou mudou por OUTRA sessao depois do `empilhar`, e ai a
    /// escrita recusa: a filha ficaria orfa, e a transacao que a trouxe nao a
    /// conhece.
    #[allow(clippy::too_many_arguments)]
    fn pre_conferir_uma(
        &self,
        trava: &Instancia,
        database: &str,
        sessao: &Sessao,
        abertas: &mut HashMap<String, Table>,
        e: &crate::transacao::Escrita,
        posicao: (usize, usize),
        reescrita_em: &HashMap<(String, u64), (usize, usize)>,
    ) -> Result<Vec<phxsql_store::table::EscritaDaCascata>> {
        let chave = self.aberta_na_pre_conferencia(trava, database, sessao, abertas, &e.tabela)?;
        let mut t = abertas
            .remove(&chave)
            .expect("a tabela acabou de ser achada ou inserida no mapa");
        let plano = {
            let mut maes = MaesAbertas { abertas };
            t.pre_conferir(e.rowid, pendente_de(e), &e.motivo, &mut maes)
        };
        let tocou: Vec<bool> = match &plano {
            Ok(plano) if e.cascata_na_lista => plano
                .iter()
                .map(|elo| {
                    abertas
                        .iter()
                        .find(|(k, _)| nome_simples_da_tabela(k).eq_ignore_ascii_case(&elo.tabela))
                        .and_then(|(_, f)| f.sobreposicao().map(|s| s.tocou(elo.rowid)))
                        .unwrap_or(false)
                })
                .collect(),
            _ => Vec::new(),
        };
        abertas.insert(chave, t);
        let plano = plano?;
        if !e.cascata_na_lista || plano.is_empty() {
            return Ok(plano);
        }
        let mut faltam = Vec::new();
        let mut pular_ate: Option<usize> = None;
        for (n, elo) in plano.into_iter().enumerate() {
            // A subarvore de uma filha que a lista ja reescreve adiante e dela:
            // e la, na posicao dela, que o plano dela se refaz.
            if let Some(nivel) = pular_ate {
                if elo.nivel > nivel {
                    continue;
                }
                pular_ate = None;
            }
            let adiante = reescrita_em
                .get(&(elo.tabela.to_ascii_lowercase(), elo.rowid))
                .is_some_and(|&j| j > posicao);
            if adiante {
                pular_ate = Some(elo.nivel);
                continue;
            }
            if elo.nivel == 1 && !tocou[n] {
                return Err(PhxError::Integridade(format!(
                    "{}: a linha {} de {} passou a apontar para a chave que esta \
                     alteracao muda DEPOIS de a alteracao ser empilhada, por outra \
                     sessao -- a cascata que a transacao leva foi planejada antes e \
                     nao a inclui, e ela ficaria orfa. Numa transacao nova o plano ja \
                     a encontra",
                    e.tabela, elo.rowid, elo.tabela
                )));
            }
            faltam.push(elo);
        }
        Ok(faltam)
    }

    /// Devolve a lista a transacao e a tira de `COMMITTING`: o `COMMIT` foi
    /// recusado ANTES da marca, e nada aconteceu.
    ///
    /// So a transacao que CONTINUA em `COMMITTING` volta a `ACTIVE` (pedido
    /// 559). A que o gestor encerrou no meio -- `ABORT_ONLY`, travas ja soltas
    /// -- fica encerrada e sem a lista: devolver punha `ACTIVE` sem condicao, e
    /// o COMMIT seguinte gravava uma lista que nao segurava trava nenhuma.
    pub(super) fn devolver_a_lista(&self, ligacao: u64, escritas: Vec<crate::transacao::Escrita>) {
        if let Some(tx) = self.transacoes.travar().de_mut(ligacao) {
            if tx.estado == crate::transacao::Estado::Confirmando {
                tx.escritas = escritas;
                tx.estado = crate::transacao::Estado::Ativa;
            }
        }
    }

    /// Tudo o que vem DEPOIS da marca.
    ///
    /// # Os tres desfechos de uma passada que quebra, e o `repetir` de cada um
    ///
    /// A marca e o ponto de compromisso -- mas so para a lista que PODE ser
    /// aplicada. A pergunta que decide e a CLASSE do erro
    /// ([`crate::transacao::ClasseDoErro`], a mesma regra que a instrucao ja
    /// usa) e quantas escritas terminaram antes dele:
    ///
    /// | quebra | aplicadas | desfecho |
    /// |---|---|---|
    /// | de ACESSO (4xxx: congelada, reservada) | zero | a marca SAI; a transacao volta a `ACTIVE` com a lista, e o erro vai com o `repetir` dele -- nada foi aplicado, repetir o COMMIT e verdade |
    /// | do DADO (2xxx/3xxx: chave, tipo) | zero | a marca SAI; a transacao sai como sempre saiu, com o erro -- a lista nao se aplica como foi escrita |
    /// | do DADO | alguma | a marca SAI e a resposta DIZ quantas ficaram: a ordem de digitacao proibe desfazer, e completar o resto aplicaria uma transacao invalida. `repetir` e falso |
    /// | qualquer outra | qualquer | `COMMITTED`: a transacao aconteceu, e o que falta se completa para a FRENTE, na hora e com a mesma trava |
    ///
    /// Nenhuma linha manda repetir com escrita aplicada -- a assertiva do §5
    /// item 9 do parecer do 426. A terceira linha era a lacuna que sobrava --
    /// a chave estrangeira so se conferia aqui, depois da marca -- e deixou de
    /// ser caminho de dado valido no pedido 448: a `pre_conferir_a_lista` faz
    /// as mesmas perguntas ANTES da marca e recusa com zero aplicado. Ela
    /// continua aqui como CINTO, e quando dispara e defeito do motor (a
    /// pre-conferencia aprovou o que a passada recusou), e a resposta diz isso.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn depois_da_marca(
        &self,
        mut trava: TravaMedida<'_>,
        sessao: &Sessao,
        id: u64,
        database: &str,
        escritas: Vec<crate::transacao::Escrita>,
        marca: &Path,
        inicio: Instant,
    ) -> Result<Json> {
        let quantas = escritas.len();
        let mut avisos = Vec::new();
        let mut aviso_da_passada: Option<(String, bool)> = None;
        let fim = self.passada_sob_a_marca(
            &mut trava,
            sessao,
            database,
            &escritas,
            marca,
            NomeDaPassada::DO_COMMIT,
        );
        drop(trava);
        match fim {
            FimDaPassada::Aplicada(depois_de_rodar) => {
                // Os AFTER rodam depois da trava, como em toda escrita deste
                // servidor. Um gatilho que nem se deixa LER vira aviso como o
                // que falha rodando: o `?` que havia aqui devolvia erro de um
                // COMMIT ja gravado, e ainda deixava a transacao no registro,
                // presa em `COMMITTING` com as travas na mao.
                for (ped, evento, nova, velha) in depois_de_rodar {
                    match self.gatilhos_para(&ped, evento) {
                        Ok((_, depois)) => avisos
                            .extend(self.rodar_gatilhos_depois(&depois, nova, velha, &ped, sessao)),
                        Err(e) => avisos.push(Json::texto_de(format!(
                            "os gatilhos AFTER de {} nao rodaram: {e}",
                            ped.texto_ou("tabela", "")
                        ))),
                    }
                }
            }
            FimDaPassada::Completada(aviso) => aviso_da_passada = Some(aviso),
            // As duas primeiras linhas da tabela: nada chegou ao disco. A de
            // ACESSO devolve a lista -- repetir o COMMIT e verdade --, e a do
            // dado termina a transacao, como sempre terminou.
            FimDaPassada::NadaAplicado { erro, de_acesso } => {
                if de_acesso {
                    self.devolver_a_lista(sessao.ligacao, escritas);
                } else {
                    self.descartar_transacao(sessao.ligacao);
                }
                return Err(erro);
            }
            FimDaPassada::ParouNoMeio(erro) => {
                self.descartar_transacao(sessao.ligacao);
                return Err(erro);
            }
        }
        self.descartar_transacao(sessao.ligacao);
        let mut resposta = vec![
            ("transaction_id", Json::de_u64(id)),
            (
                "transaction_state",
                Json::texto_de(crate::transacao::Estado::Confirmada.nome()),
            ),
            ("gravadas", Json::de_u64(quantas as u64)),
            ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
        ];
        // So quando houve: a resposta de sempre nao ganha campo vazio.
        if let Some((texto, pendente)) = aviso_da_passada {
            resposta.push(("aviso", Json::texto_de(texto)));
            if pendente {
                resposta.push(("completando", Json::Bool(true)));
            }
        }
        avisos.truncate(50);
        if !avisos.is_empty() {
            resposta.push(("gatilhos_avisos", Json::Lista(avisos)));
        }
        Ok(Json::objeto(resposta))
    }

    /// A passada de uma lista cuja marca JA esta no disco, e o destino da
    /// marca -- o miolo do [`Servidor::depois_da_marca`], sem o que e so de
    /// transacao (a lista devolvida ou descartada, a resposta do COMMIT).
    ///
    /// Saiu dele quando a alteracao SOLTA que cascateia passou a gravar marca
    /// (pedido 540): as duas sao a mesma passada, e uma segunda tabela de
    /// desfechos -- a de «quebrou no meio» -- seria a copia que diverge. A
    /// trava fica na mao de quem chama, e e ele quem a solta.
    pub(super) fn passada_sob_a_marca(
        &self,
        trava: &mut TravaMedida<'_>,
        sessao: &Sessao,
        database: &str,
        escritas: &[crate::transacao::Escrita],
        marca: &Path,
        nome: NomeDaPassada,
    ) -> FimDaPassada {
        let quantas = escritas.len();
        let mut aplicadas = 0usize;
        match self.aplicar_conjunto(trava, database, escritas, sessao, &mut aplicadas) {
            Ok(depois_de_rodar) => {
                // A passada terminou: a marca deixa de estar EM VOO antes de
                // ir para as pendentes (pedido 451, M1). Completa-la num
                // panico daqui em diante acharia tudo aplicado e APAGARIA o
                // bilhete antes do `fsync` da janela.
                trava.marca_em_voo = None;
                // A marca so sai depois de a tabela estar sincronizada. A
                // janela fechou na passada? Entao sai agora. Ainda aberta? A
                // marca fica pendurada, e quem a apaga e o `descarregar_sujas`
                // -- **depois** de o `fsync` acontecer, nunca antes.
                if self.tabelas_ainda_sujas(database, escritas) {
                    if let Ok(mut m) = self.marcas_pendentes.lock() {
                        m.push(marca.to_path_buf());
                    }
                } else {
                    let _ = std::fs::remove_file(marca);
                }
                FimDaPassada::Aplicada(depois_de_rodar)
            }
            Err(e) => {
                let do_pedido = crate::transacao::ClasseDoErro::do_erro(&e)
                    == crate::transacao::ClasseDoErro::Instrucao;
                let de_acesso = e.codigo() / 1000 == 4;
                // As duas primeiras linhas da tabela: nada da lista chegou ao
                // disco, e o erro e do pedido ou do acesso -- entao a marca
                // pode sair, e a lista deixa de ter acontecido. Se a marca
                // NAO sai (o `unlink` falhou), ela vai ser completada no
                // arranque, e dizer «nada aconteceu» seria mentir: cai no
                // caminho da frente, la embaixo.
                if do_pedido && aplicadas == 0 && std::fs::remove_file(marca).is_ok() {
                    return FimDaPassada::NadaAplicado { erro: e, de_acesso };
                }
                // A terceira linha: erro do DADO com parte ja gravada.
                if do_pedido && !de_acesso {
                    let _ = std::fs::remove_file(marca);
                    let feitas: Vec<String> = escritas[..aplicadas]
                        .iter()
                        .map(|w| format!("{} {} rowid {}", w.acao.nome(), w.tabela, w.rowid))
                        .collect();
                    return FimDaPassada::ParouNoMeio(com_nota(
                        e,
                        &format!(
                            "{} parou na escrita {} de {quantas}, e as {aplicadas} \
                             anteriores JA ESTAO gravadas e nao se desfazem -- a ordem \
                             de digitacao proibe desfazer ({}). Nada mais desta \
                             {} sera aplicado; NAO a repita inteira. {}",
                            nome.quem,
                            aplicadas + 1,
                            feitas.join(", "),
                            nome.unidade,
                            nome.conferida
                        ),
                    ));
                }
                // A passada quebrou no meio, e a marca ESTA no disco -- entao
                // a lista ja foi confirmada, e o unico caminho e completa-la,
                // andando para a FRENTE (desfazer devolveria slot, e o `.reg`
                // nunca reaproveita slot).
                //
                // Com a MESMA trava, e e isso o conserto da camada (c) do
                // pedido 426: este braco pedia `travar_dados()` de novo com a
                // do topo ainda viva, a trava nao e reentrante, a recusa caia
                // num `if let Ok` que a engolia, e a recuperacao NUNCA rodava
                // -- enquanto este comentario dizia que rodava. Soltar e tomar
                // de novo tambem nao serve: abre a fresta em que outra escrita
                // entra no meio da lista confirmada.
                //
                // O codigo e o da recuperacao do arranque, para uma marca so;
                // a diferenca de politica (a marca da operacao impossivel
                // FICA) esta em `transacao::completar_marca_em_voo`. E a marca
                // e a que ESTE processo gravou e sincronizou: a que nao se rele
                // fica no disco (pedido 503, 1b) -- antes ela saia, e a resposta
                // dizia «pendente, se completa na proxima recuperacao», que era
                // falso.
                let r = crate::transacao::completar_marca_em_voo(trava, database, marca, true);
                if r.houve() {
                    eprintln!("{}", r.texto(&self.config.base));
                }
                // A copia residente nao sabe o que a recuperacao gravou por
                // baixo dela: sai da memoria, e a leitura volta ao disco --
                // mostrar a copia velha seria mentir sobre o dado.
                let soltas = self.soltar_residentes_de(database, escritas);
                // «Completou» so com a marca LIDA e completada sem nenhuma
                // operacao impossivel. Qualquer outra coisa -- inclusive a
                // marca que nem se leu -- e dita como pendente: a resposta nao
                // afirma o que nao conferiu.
                let pendente =
                    !(r.completadas == 1 && r.impossiveis.is_empty() && r.paradas.is_empty());
                let mut texto = if pendente {
                    format!(
                        "{} quebrou depois da marca ({e}), e a {} ESTA {}: a \
                         aplicacao ficou pendente ({}) e se completa na proxima \
                         recuperacao do servidor. NAO repita a {} -- repetir \
                         duplicaria o que ja foi gravado",
                        nome.passada,
                        nome.unidade,
                        nome.feita,
                        r.impossiveis
                            .iter()
                            .chain(r.paradas.iter())
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("; "),
                        nome.unidade
                    )
                } else {
                    format!(
                        "{} quebrou depois da marca ({e}), e a recuperacao a \
                         completou na hora: a {} inteira esta gravada, e nao ha o \
                         que repetir. Os gatilhos AFTER dela nao rodaram",
                        nome.passada, nome.unidade
                    )
                };
                if !soltas.is_empty() {
                    texto.push_str(&format!(
                        ". A copia residente de {} saiu da memoria",
                        soltas.join(", ")
                    ));
                }
                FimDaPassada::Completada((texto, pendente))
            }
        }
    }

    /// Tira da memoria a copia residente das tabelas destas escritas. Devolve
    /// quais sairam.
    fn soltar_residentes_de(
        &self,
        database: &str,
        escritas: &[crate::transacao::Escrita],
    ) -> Vec<String> {
        let mut soltas = Vec::new();
        if let Ok(mut r) = self.residentes.lock() {
            for e in escritas {
                let chave = Self::chave_residente(&pedido_da_tabela(database, &e.tabela));
                if r.remove(&chave).is_some() {
                    soltas.push(e.tabela.clone());
                }
            }
        }
        soltas
    }

    /// A passada: aplica o conjunto de escrita, na ordem, com a trava na mao.
    ///
    /// Devolve o que os AFTER precisam saber, para eles rodarem DEPOIS de a
    /// trava sair -- que e onde eles ja rodam em toda escrita deste servidor.
    ///
    /// `aplicadas` conta as escritas que TERMINARAM -- e o que o braco de
    /// erro do `COMMIT` precisa para saber se a quebra veio antes de qualquer
    /// byte da lista (pedido 426).
    #[allow(clippy::type_complexity)]
    fn aplicar_conjunto(
        &self,
        trava: &Instancia,
        database: &str,
        escritas: &[crate::transacao::Escrita],
        sessao: &Sessao,
        aplicadas: &mut usize,
    ) -> Result<Vec<(Json, phxsql_sql::rotina::Evento, Option<Json>, Option<Json>)>> {
        use crate::transacao::Acao;
        let mut abertas: HashMap<String, Table> = HashMap::new();
        let mut depois_de_rodar = Vec::new();
        let ha_gatilhos = self.ha_gatilhos.load(Ordering::Relaxed);
        for e in escritas {
            // O elo da cascata nao dispara o AFTER da filha (pedido 562,
            // `INTEGRIDADE.md` §7.3). Os motores empatam no voto -- PG 4 +
            // SQLite 1 = 5 disparam, MariaDB 3 + MySQL 2 = 5 nao --, entao vale
            // a decisao escrita. E ela mora AQUI, e nao em quem chama: a
            // alteracao solta e o COMMIT passam por esta mesma passada, e
            // antes so a solta descartava, dando auditoria diferente para a
            // mesma cascata dentro e fora da transacao.
            let dispara = ha_gatilhos && !e.elo_da_cascata;
            let ped = pedido_da_tabela(database, &e.tabela);
            #[cfg(test)]
            if self.passada_quebra_na_escrita.load(Ordering::SeqCst) == *aplicadas + 1 {
                if self.passada_quebra_congela.load(Ordering::SeqCst) {
                    let t = self.abrir_travada(trava, &ped, sessao)?;
                    let c = phxsql_store::congelamento::congelar(
                        t.diretorio(),
                        t.nome(),
                        "quebra de teste da passada",
                    )?;
                    if let Ok(mut g) = self.passada_quebra_congelando.lock() {
                        *g = Some(c);
                    }
                }
                return Err(PhxError::EmMigracao(format!(
                    "quebra de teste antes da escrita {} ({})",
                    *aplicadas + 1,
                    e.tabela
                )));
            }
            // Abre a tabela UMA vez e a guarda no mapa. A seguir ela e RETIRADA
            // do mapa enquanto grava, para que a conferencia de FK possa
            // emprestar as MAES (as outras entradas) -- o conserto do P0. Ela
            // volta ao mapa no fim do laco; so o caminho de erro a solta, e ali
            // a passada inteira aborta.
            if !abertas.contains_key(&e.tabela) {
                let aberta = self.abrir_travada(trava, &ped, sessao)?;
                abertas.insert(e.tabela.clone(), aberta);
            }
            let mut t = abertas
                .remove(&e.tabela)
                .expect("a tabela acabou de ser inserida no mapa");
            let velha = if dispara && e.acao != Acao::Inserir {
                t.ler(e.rowid)?.map(|l| linha_para_json(&l, t.esquema()))
            } else {
                None
            };
            let evento = match e.acao {
                Acao::Inserir => phxsql_sql::rotina::Evento::Inserir,
                Acao::Atualizar => phxsql_sql::rotina::Evento::Atualizar,
                _ => phxsql_sql::rotina::Evento::Excluir,
            };
            match e.acao {
                Acao::Inserir => {
                    let saiu = {
                        let mut maes = MaesAbertas {
                            abertas: &mut abertas,
                        };
                        t.inserir_com_maes(&e.linha, &mut maes)?
                    };
                    // A garantia que a marca prometeu. Divergir aqui quer
                    // dizer que alguem escreveu nesta tabela apesar da
                    // reserva -- e o commit para em vez de gravar no lugar
                    // errado.
                    if saiu != e.rowid {
                        return Err(PhxError::Corrompido(format!(
                            "a transacao reservou o rowid {} em {} e a insercao \
                             saiu {saiu}",
                            e.rowid, e.tabela
                        )));
                    }
                    self.residente_mut(&ped, |m| m.anotar_insercao(saiu, &e.linha));
                }
                Acao::Atualizar => {
                    {
                        let mut maes = MaesAbertas {
                            abertas: &mut abertas,
                        };
                        // ACID-C: quando a cascata ja e a lista, a mae e cada
                        // elo aplicam SEM cascatear -- senao a filha, que ja e
                        // uma escrita da lista, seria gravada duas vezes.
                        if e.cascata_na_lista {
                            t.atualizar_sem_cascata_com_maes(e.rowid, &e.linha, &mut maes)?;
                        } else {
                            t.atualizar_com_maes(e.rowid, &e.linha, &mut maes)?;
                        }
                    }
                    self.residente_mut(&ped, |m| m.anotar_alteracao(e.rowid, &e.linha));
                }
                // As exclusoes emprestam as FILHAS que a passada ja abriu, e a
                // restauracao as MAES -- o irmao do conserto do P0 (pedido
                // 448). Um segundo descritor sobre a filha que a passada ja
                // escreveu batia na guarda do `.ndx`, e `[excluir a filha de
                // vez, excluir a mae]`, transacao valida, saia daqui mandando
                // reparar um indice sao.
                Acao::ExcluirSuave => {
                    let apagou = {
                        let mut maes = MaesAbertas {
                            abertas: &mut abertas,
                        };
                        t.excluir_suave_com_maes(e.rowid, &e.motivo, &mut maes)?
                    };
                    if apagou {
                        self.residente_mut(&ped, |m| m.anotar_exclusao(e.rowid));
                    }
                }
                Acao::ExcluirDeVez => {
                    let apagou = {
                        let mut maes = MaesAbertas {
                            abertas: &mut abertas,
                        };
                        t.excluir_de_vez_com_maes(e.rowid, &e.motivo, &mut maes)?
                    };
                    if apagou {
                        self.residente_mut(&ped, |m| m.anotar_exclusao(e.rowid));
                    }
                }
                Acao::Restaurar => {
                    let mut maes = MaesAbertas {
                        abertas: &mut abertas,
                    };
                    t.restaurar_com_maes(e.rowid, &e.motivo, &mut maes)?;
                }
            }
            if dispara {
                let nova = match e.acao {
                    Acao::ExcluirSuave | Acao::ExcluirDeVez => None,
                    _ => t.ler(e.rowid)?.map(|l| linha_para_json(&l, t.esquema())),
                };
                depois_de_rodar.push((ped, evento, nova, velha));
            }
            // A filha volta ao mapa: o group commit no fim do laco sincroniza
            // todas as tabelas abertas, e ela precisa estar la.
            abertas.insert(e.tabela.clone(), t);
            *aplicadas += 1;
            // So em `debug`, pedido 702: o processo morre depois da N-esima
            // escrita da passada, com a marca no disco e parte do commit no
            // diario -- a queda que o arranque completa.
            #[cfg(debug_assertions)]
            if parar_no_commit_de_teste() == Some(*aplicadas as u64) {
                sigkill_de_teste("teste: parado no meio da passada do COMMIT");
            }
        }
        // **O GROUP COMMIT, e ele e a janela de durabilidade que ja existia.**
        //
        // A primeira versao chamava `sincronizar()` aqui, um `fsync` por
        // tabela por commit. Medido: um commit de UMA linha custava 1,542 ms,
        // dos quais 0,342 ms sao a marca e 0,063 ms o trabalho de verdade --
        // sobravam **1,14 ms, 74% do commit, so no `fsync` da tabela**. Uma
        // insercao SOLTA, sem transacao, custa 0,066 ms justamente porque ela
        // passa por esta janela.
        //
        // Adiar e seguro porque a marca ja esta no disco: uma queda antes de a
        // janela fechar deixa o `.tx` la, e a recuperacao completa o commit.
        // O que NAO se adia e a marca -- ela e o ponto de compromisso.
        for (nome, mut t) in abertas {
            let ped = pedido_da_tabela(database, &nome);
            self.gravar_de_verdade(trava, &mut t, &ped)?;
        }
        Ok(depois_de_rodar)
    }

    /// Fecha (ou adia) a janela de durabilidade desta gravacao.
    ///
    /// # Por que a instancia entra por parametro
    ///
    /// Porque quem chama JA ESTA com a trava de dados na mao -- as oito
    /// chamadas desta funcao vem logo depois de um `travar_dados`. Pedir a
    /// trava de novo aqui dentro e o abraco mortal com a propria trava:
    /// `std::sync::Mutex` nao e reentrante, e a thread para para sempre
    /// segurando o servidor inteiro. Era exatamente isso que acontecia quando
    /// a janela fechava com OUTRA tabela suja no conjunto -- duas tabelas
    /// gravadas alternadamente bastavam, sem gatilho nenhum no meio.
    pub(super) fn gravar_de_verdade(
        &self,
        dados: &Instancia,
        t: &mut Table,
        p: &Json,
    ) -> Result<()> {
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
        self.descarregar_sujas_com(dados);
        Ok(())
    }

    /// Sincroniza tudo que foi escrito e ainda nao foi para o disco.
    ///
    /// Para quem NAO tem a trava de dados: o relogio de gravacao e a saida de
    /// uma conexao com carga reservada. Quem ja a tem chama a `_com`, senao
    /// trava o servidor.
    pub(super) fn descarregar_sujas(&self) {
        // Sem nada sujo nem se pede a trava: e o caminho comum do relogio de
        // fundo, que acorda muito mais vezes do que acha trabalho.
        if self.sujas.lock().map(|s| s.is_empty()).unwrap_or(true) {
            return;
        }
        let Ok(dados) = self.travar_dados() else {
            return;
        };
        self.descarregar_sujas_com(&dados);
    }

    /// A parada pedida (pedido 687): leva ao disco o que a janela de
    /// durabilidade segurava e FECHA a porta da escrita para sempre.
    ///
    /// # Por que e o fecho da janela, e nao um caminho novo
    ///
    /// O `.ndx` so baixa a marca de «ficou para tras numa queda» no
    /// `sincronizar` -- o `fechar` nunca baixa (pedido 522). Toda tabela
    /// escrita desde o ultimo fecho esta nas `sujas`, e e o
    /// `descarregar_sujas` que as sincroniza e apaga as marcas pendentes na
    /// ordem que a §12.6 do `docs/CONCORRENCIA.md` exige. A parada e esse
    /// mesmo fecho chamado uma ultima vez: um segundo caminho de «fechar
    /// tudo» seria a mesma decisao escrita duas vezes, e a que alguem
    /// esquecesse de atualizar deixaria a parada mais fraca que o relogio.
    ///
    /// # Por que a trava fica PRESA no fim
    ///
    /// Entre o fecho soltar a trava e o processo sair, um escritor que
    /// estava na fila entraria e sujaria uma tabela de novo, com a marca no
    /// disco em 1 e ninguem mais para baixa-la. Entao a trava se toma de
    /// volta e se confere a lista: vazia, a guarda e esquecida de proposito
    /// e a escrita nao volta a entrar; cheia, o fecho roda outra vez. Por
    /// isso esta funcao e TERMINAL -- quem a chama sai do processo depois.
    ///
    /// A conferencia sob a trava so le a lista: o `fsync` fica no fecho de
    /// sempre, fora de secao critica nova (a catraca `alcancam-fsync`).
    ///
    /// Devolve as chaves que NAO sincronizaram (disco recusando, trava
    /// envenenada). Vazio e a parada limpa; cheio, a marca continua em 1 e o
    /// arranque seguinte reconstroi -- que e o lado seguro, e e dito.
    pub fn parar_em_ordem(&self) -> Vec<String> {
        // A janela de `por_lote` passa a contar do zero: o fecho abaixo e o
        // dela, e o relogio nao precisa acordar para mais nada.
        self.janela.fechar();
        let mut ficaram = Vec::new();
        for _ in 0..5 {
            self.descarregar_sujas();
            let trava = self.travar_dados();
            // Lista envenenada: le-se o que ela tem, e nao se afirma que a
            // janela fechou.
            ficaram = self.sujas.lock().map_or_else(
                |e| e.into_inner().iter().cloned().collect(),
                |lista| lista.iter().cloned().collect(),
            );
            if ficaram.is_empty() {
                if let Ok(t) = trava {
                    // A porta da escrita fecha aqui e nao reabre: ninguem
                    // mais grava depois do ultimo `fsync`.
                    std::mem::forget(t);
                }
                break;
            }
            drop(trava);
        }
        ficaram.sort();
        ficaram
    }

    /// Liga a parada pelo `SIGTERM`/`SIGINT` (pedido 687): uma thread olha o
    /// pedido que o tratador grava (`crate::sinais`) e, quando ele chega,
    /// chama [`Servidor::parar_em_ordem`] e sai do processo.
    ///
    /// Um olhar a cada 100 ms, e nao um despertador: o tratador de sinal nao
    /// pode tomar mutex nem escrever em soquete, e o custo do olhar e um
    /// `load` atomico -- o da parada e no maximo um decimo de segundo a mais,
    /// uma vez por vida do processo.
    ///
    /// Chamada pelo `phxsqld` (servidor e ponte MCP), nunca pelo `escutar`: o
    /// servidor embutido num teste ou num app nao e dono do processo, e
    /// instalar tratador de sinal no processo alheio seria tomar uma decisao
    /// que nao e dele.
    pub fn parar_ao_sinal(self: &Arc<Self>) -> bool {
        if !crate::sinais::instalar() {
            return false;
        }
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "vigia-de-sinais",
            "espera o SIGTERM/SIGINT da parada pedida para levar a janela de \
             durabilidade ao disco antes de sair, em vez de morrer pelo padrao \
             do nucleo com o .ndx marcado",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("esperando SIGTERM ou SIGINT");
                let sinal = loop {
                    if let Some(s) = crate::sinais::pedido() {
                        break s;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                };
                crate::sinais::devolver_ao_padrao();
                fio.fazendo("parada pedida: levando a janela ao disco");
                eprintln!(
                    "{} recebido: parada em ordem (um segundo sinal derruba sem esperar)",
                    crate::sinais::nome(sinal)
                );
                let ficaram = servidor.parar_em_ordem();
                if ficaram.is_empty() {
                    eprintln!("parada em ordem concluida: o que estava escrito foi ao disco");
                    std::process::exit(0);
                }
                eprintln!(
                    "ATENCAO: {} tabela(s) nao sincronizaram na parada ({}): o \
                     indice delas continua marcado, e o proximo arranque o \
                     reconstroi a partir do .reg",
                    ficaram.len(),
                    ficaram.join(", ")
                );
                std::process::exit(1);
            },
        );
        true
    }

    /// O mesmo, com a trava de dados JA na mao.
    ///
    /// Reabre cada tabela suja so para sincronizar. Custa um `open` por tabela,
    /// uma vez por janela -- nao por gravacao. Erro de ANTES do disco aqui nao
    /// derruba nada -- a tabela que nao abriu, o `.pag` que nao se grava --: a
    /// tabela continua na lista e a proxima passada tenta de novo. O `fsync`
    /// RECUSADO nao volta para ca: ele derruba o processo no instante da
    /// recusa (pedido 509), porque tentar de novo e justamente o que responde
    /// Ok sem o dado. Ver [`fsync_recusado_derruba_o_processo`].
    ///
    /// # O `fsync` das K tabelas acontece JUNTO, e a ordem continua inteira
    ///
    /// O comboio desta funcao esta medido na secao 12 do `docs/CONCORRENCIA.md`:
    /// com os escritores fixos em 4, o p99 de um leitor que le uma tabela que
    /// NINGUEM escreve sobe 2,01x de K=1 para K=4, so porque outras tabelas
    /// ficaram sujas. E o `o-comboio-por-dentro` dividiu o custo: `abrir` 5-7%,
    /// `fsync` 93-96%.
    ///
    /// A saida obvia -- soltar a trava entre uma tabela e a seguinte -- esta
    /// **RECUSADA com a matriz de queda** (`docs/CONCORRENCIA.md` §12.6): esta
    /// funcao apaga as marcas pendentes depois de sincronizar, e o que faz isso
    /// ser seguro nao e a ordem sozinha, e o ENCONTRO SER ATOMICO. Soltar a
    /// trava no meio deixa outro escritor entrar, sujar uma tabela que ja
    /// passou e pendurar a marca dele -- e o fim deste laco apagaria a marca de
    /// um commit cujo dado nunca foi sincronizado.
    ///
    /// O que se pode fazer sem encostar nisso e sincronizar as K ao mesmo
    /// tempo: **nao ha ordem ENTRE tabelas para preservar**, porque o encontro
    /// so termina quando todas terminam. A ordem DENTRO de cada tabela
    /// (`.trash` antes do `.reg`) continua inteira, e o numero de `fsync` e o
    /// mesmo -- medido em 32 para K=4 nos dois arranjos, 8 por tabela, que e a
    /// `TETO_FSYNC_POR_FECHO_V2`. Medido: 1,62x em K=4 e 2,52x em K=16
    /// (`--example o-comboio-em-paralelo`).
    ///
    /// O `abrir` fica em serie de proposito: sao os 5-7%, e o catalogo tem
    /// estado compartilhado que nao se ganha nada em disputar.
    /// O `fsync` do fecho recusado: o IRMAO do gancho do `anotar`.
    ///
    /// O fecho da janela nao responde a cliente nenhum, entao um `EIO` aqui
    /// nunca passa por `anotar` -- a chave voltava para as sujas e ninguem
    /// ficava sabendo. E o caso mais grave da saude do disco, porque e o
    /// disco recusando justamente o passo que torna o commit duravel.
    ///
    /// Desde o pedido 509 o que chega aqui e o erro de ANTES do `fsync` -- a
    /// pagina do `.ndx` que o disco cheio recusou, o volume que nao abriu. O
    /// `fsync` recusado derruba o processo antes de voltar (ver
    /// [`fsync_recusado_derruba_o_processo`]).
    pub(super) fn fecho_recusado(&self, chave: &str, e: &PhxError) {
        let PhxError::Io(io) = e else {
            return;
        };
        let (db, tab) = chave.split_once('/').unwrap_or((chave, ""));
        self.evento_de_disco(
            crate::saude_do_disco::classificar(io),
            "fecho",
            db,
            tab,
            &e.to_string(),
        );
    }

    pub(super) fn descarregar_sujas_com(&self, dados: &Instancia) {
        // Uma COPIA, e nao a lista drenada -- pedido 451, A1 do DBA.
        //
        // Drenar antes do trabalho tirava das sujas a tabela que ainda nao
        // tinha ido ao disco: um panico no meio do laco perdia as chaves que
        // faltavam, e o reparo da trava -- que comeca por este mesmo fecho --
        // achava as sujas sem elas e drenava as marcas pendentes, apagando o
        // bilhete de um commit cujo dado nao passou por `fsync`. A chave agora
        // so sai quando a tabela dela sincronizou (`tirar_das_sujas`, a cada
        // pedaco), e o que o panico interromper continua na lista.
        //
        // Tirar DEPOIS e seguro mesmo com alguem pondo a mesma chave de volta
        // no meio (o `soltar_cargas_da_ligacao` faz isso sem a trava): toda
        // escrita numa tabela exige esta trava, que esta na mao de quem
        // sincroniza -- o que a chave de volta representa ja estava no
        // `fsync` que acabou de acontecer.
        let lista: Vec<String> = match self.sujas.lock() {
            Ok(s) => s.iter().cloned().collect(),
            Err(_) => return,
        };
        // Lista vazia NAO volta daqui: segue ate a drenagem das marcas. Quem
        // chama pode ter sincronizado a unica tabela suja por conta propria
        // -- o fecho da janela numa tabela so, e o `bulkinsert(false)` -- e
        // ai o que falta nao e `fsync` nenhum, e apagar a marca do commit que
        // esperava por ele. Voltar aqui era o pedido 254: a marca de um
        // commit ja duravel ficava no disco para sempre, e o relogio de
        // fundo nao a alcancava porque ele tambem volta sem tabela suja.
        let mut faltaram = Vec::new();
        for pedaco in lista.chunks(FIOS_DO_FECHO) {
            let mut chaves: Vec<String> = Vec::with_capacity(pedaco.len());
            let mut abertas: Vec<Table> = Vec::with_capacity(pedaco.len());
            // Chave sem `/` nao e tabela nenhuma: sai da lista, como saia
            // quando a lista era drenada.
            let mut sem_tabela: Vec<String> = Vec::new();
            for chave in pedaco {
                #[cfg(test)]
                if self.fecho_de_teste(chave) {
                    faltaram.push(chave.clone());
                    continue;
                }
                let Some((db, tab)) = chave.split_once('/') else {
                    sem_tabela.push(chave.clone());
                    continue;
                };
                match dados
                    .abrir_database(db)
                    .and_then(|d| d.abrir_qualificada(tab))
                {
                    // O indice adiado de uma carga que saiu SEM o
                    // `bulkinsert(false)` -- conexao que caiu, reserva que
                    // venceu (pedido 324). A reserva ja nao esta la, e o
                    // indice suspenso recusaria toda busca ate o proximo
                    // arranque. Reconstroi aqui, antes do `fsync` que baixa o
                    // byte 52. O portao e um `bool` do punho ja aberto: sem
                    // carga adiada, este fecho custa o que custava.
                    Ok(mut t) if t.indice_suspenso() && !self.reservada(db, tab) => {
                        match t.reindexar() {
                            Ok(_) => {
                                chaves.push(chave.clone());
                                abertas.push(t);
                            }
                            // A marca fica no disco: a tabela continua
                            // recusando dizendo que ha carga, e o arranque
                            // reconstroi. A chave fica para a proxima passada.
                            Err(_) => faltaram.push(chave.clone()),
                        }
                    }
                    Ok(t) => {
                        chaves.push(chave.clone());
                        abertas.push(t);
                    }
                    // Nao abriu: nao ha o que sincronizar, e a marca desta
                    // tabela tem de FICAR. A proxima passada tenta de novo.
                    Err(_) => faltaram.push(chave.clone()),
                }
            }
            self.tirar_das_sujas(&sem_tabela);
            // Uma tabela so nao tem com quem se sobrepor: sem fio nenhum, o
            // caminho de K=1 continua sendo exatamente o de antes.
            if abertas.len() == 1 {
                match abertas[0].sincronizar() {
                    Ok(()) => self.tirar_das_sujas(&chaves),
                    Err(e) => {
                        self.fecho_recusado(&chaves[0], &e);
                        faltaram.push(chaves.remove(0));
                    }
                }
                continue;
            }
            let quebrados: Vec<(usize, Option<PhxError>)> = std::thread::scope(|escopo| {
                let fios: Vec<_> = abertas
                    .iter_mut()
                    .map(|t| escopo.spawn(move || t.sincronizar()))
                    .collect();
                // `join` devolve `Err` quando o fio entrou em panico, e panico
                // aqui NAO pode virar marca apagada: um `unwrap` derrubaria a
                // thread que segura a trava de dados, e o caminho seguro e o
                // mesmo do erro de E/S -- a chave volta para as sujas e a marca
                // fica pendurada.
                fios.into_iter()
                    .enumerate()
                    .filter_map(|(i, f)| match f.join() {
                        Ok(Ok(())) => None,
                        Ok(Err(e)) => Some((i, Some(e))),
                        Err(_) => Some((i, None)),
                    })
                    .collect()
            });
            let mut quebradas = vec![false; chaves.len()];
            for (i, erro) in quebrados {
                if let Some(e) = &erro {
                    self.fecho_recusado(&chaves[i], e);
                }
                quebradas[i] = true;
                faltaram.push(chaves[i].clone());
            }
            let sincronizadas: Vec<String> = chaves
                .into_iter()
                .zip(quebradas)
                .filter(|(_, quebrou)| !quebrou)
                .map(|(chave, _)| chave)
                .collect();
            self.tirar_das_sujas(&sincronizadas);
        }
        if !faltaram.is_empty() {
            // Alguma tabela nao sincronizou: as marcas FICAM. Apaga-las agora
            // seria jogar fora o bilhete de um dado que pode nao estar no
            // disco -- e o bilhete e a unica coisa que o traz de volta.
            return;
        }
        // O disco esta em dia: as marcas dos commits que esperavam por ele
        // podem sair. **Esta e a ordem que faz o group commit ser seguro**, e
        // ela nao se inverte.
        //
        // E a ordem sozinha NAO e o que faz -- este comentario dizia menos do
        // que a garantia, e a matriz de queda da secao 12.6 do
        // `docs/CONCORRENCIA.md` mediu a diferenca. O invariante de verdade e:
        // *uma marca so sai quando toda tabela que ela nomeia teve um `fsync`
        // POSTERIOR a ultima escrita daquela transacao*. O laco entrega isso
        // com uma regra mais barata -- apaga todas quando todas sincronizaram
        // -- e as duas so sao a mesma coisa porque **ninguem escreve no meio**:
        // o encontro inteiro acontece sob uma tomada so da trava de dados.
        //
        // Por isso o `join` acima acontece ANTES daqui, e por isso soltar a
        // trava entre uma tabela e a seguinte esta recusado: outro escritor
        // entraria, sujaria uma tabela que ja passou e penduraria a marca dele
        // -- e esta drenagem a apagaria. Uma queda de energia depois disso
        // perde um commit confirmado, sem bilhete nenhum, e a bateria de
        // `SIGKILL` NAO acusaria (pagina suja no cache do nucleo sobrevive a
        // processo morto -- pedido 186).
        let pendentes: Vec<PathBuf> = match self.marcas_pendentes.lock() {
            Ok(mut m) => m.drain(..).collect(),
            Err(_) => return,
        };
        for marca in pendentes {
            let _ = std::fs::remove_file(marca);
        }
    }

    /// O REPARO da trava de dados, rodado pelo `TravaMedida::drop` no
    /// desenrolar de um panico, com a trava ainda na mao -- pedido 451.
    ///
    /// # O desenho, que nao e desta frente
    ///
    /// E a H4 do DBA (`docs/propostas/parecer-dba-451-448-2026-09-24.md`): o
    /// panico vira queda, e a queda se cura na hora, pela mao que caiu. A
    /// convergencia dos quatro motores e no COMPORTAMENTO (10 de 10: descartar
    /// o estado e passar pela recuperacao antes do proximo uso); o MEIO deles
    /// desfaz, e aqui a ordem de digitacao proibe -- entao anda para a frente,
    /// como no 426. A revisao adversaria do mesmo DBA
    /// (`docs/propostas/parecer-dba-451-2026-09-24.md`) apertou quatro pontos,
    /// e cada um esta nomeado abaixo.
    ///
    /// # O que ele faz, e em que ordem
    ///
    /// 1. `descarregar_sujas_com` -- sincroniza as tabelas da janela e so
    ///    entao apaga as marcas pendentes, a ordem de sempre do group commit.
    ///    A chave so sai das sujas depois do `fsync` dela (A1): o panico que
    ///    interrompeu um fecho nao apaga bilhete de dado que nao foi ao disco;
    /// 2. completa a marca EM VOO desta tomada, e so ela (M1), pelo
    ///    [`crate::transacao::completar_marca_em_voo`] -- o mesmo motor do braco de
    ///    erro do `COMMIT`, em O(1). O `.ndx` que a queda deixou para tras e
    ///    reconstruido por ser tabela nomeada na marca, e o `COMMIT` que
    ///    morreu no meio da passada sai inteiro antes de o `AoSair` soltar as
    ///    travas dele. As outras marcas do disco nao sao deste panico: a
    ///    pendente espera o `fsync`, a do braco de erro ja foi tratada, a
    ///    parada espera a chave -- completa-las reaplicaria `Atualizar` sem
    ///    condicao;
    /// 3. solta TODAS as copias residentes -- elas sao anotadas depois do
    ///    disco, e a da operacao que morreu ficou atras dele;
    /// 4. conta o reparo, e so ai a trava volta a atender.
    ///
    /// # O que ele NAO repara: a escrita solta de UMA tabela
    ///
    /// A tabela que a operacao interrompida tocava FORA de transacao nao entra
    /// em marca nenhuma e nao e reconstruida aqui. Num `inserir`, num
    /// `atualizar` e num `excluir` o `.ndx` fica com o byte 52 em 1 (pedido
    /// 456) e recusa, nomeando o indice, ate o `reindexar` -- o que ele ja faz
    /// depois de um `SIGKILL` no mesmo ponto. E ali basta: a linha e uma so.
    ///
    /// **A cascata do `ao_alterar` solta ENTRA em marca desde o pedido 540**
    /// (`atualizar_com_a_marca`): a mae e cada filha estao na marca EM VOO, e
    /// o passo 2 as completa. Antes dela, o panico entre duas filhas deixava as
    /// seguintes na chave velha -- o 490 so conseguia fazer a filha RECUSAR
    /// ate um `reindexar`, que a reconstruia com a orfa dentro.
    ///
    /// # Por que TODOS os residentes, e nao os da tabela tocada
    ///
    /// Porque fora de transacao nao ha intencao escrita dizendo qual tabela a
    /// operacao morta tocava -- saber isso exigiria cada caminho de escrita se
    /// registrar, que e a H3 de novo. Soltar a mais custa uma recarga pedida
    /// (`memoria_carregar`), e a leitura da memoria recusa dizendo isso;
    /// soltar a menos serve uma copia que o disco desmente.
    ///
    /// # O piso: H5 -- e a thread de SERVICO (A2)
    ///
    /// Reparo que nao se pode afirmar vira QUEDA (`abort`), o meio do InnoDB:
    /// o supervisor sobe o processo (`Restart=on-failure`, MANUAL.txt) e o
    /// arranque repara com o servidor fechado para o mundo. Servir de estado
    /// incerto e pior que cair. Um panico DENTRO do reparo e panico duplo, e o
    /// Rust aborta sozinho -- o mesmo piso, pelo outro caminho.
    ///
    /// E a thread de servico vai direto ao piso. O reparo cura a trava e NAO
    /// a thread: a de atendimento que morre leva a conexao dela, e o cliente
    /// ve; a de servico (o relogio da janela, a replicacao, o backup, os
    /// jobs) morre calada, e o que ela fazia para sem ninguem saber. Cair e o
    /// que o postmaster do PostgreSQL faz quando um processo auxiliar morre --
    /// derruba todos e sobe de novo, pela recuperacao -- e o que o InnoDB faz
    /// com o `ut_a` de uma thread de fundo. O `catch_unwind` por volta de cada
    /// laco foi a outra saida oferecida, e ficou de fora: seria a mesma
    /// decisao escrita em cada laco que toma a trava, e o laco de amanha que
    /// a esquecesse voltaria a morrer calado. A familia vem de quem subiu a
    /// thread (`telemetria::subir` e `rodar_em_filha`, os unicos `spawn` do
    /// servidor), e nao de uma lista de nomes.
    ///
    /// O trabalho que uma thread de servico MANDA FAZER -- a corrida de um
    /// job, o backup agendado -- roda numa filha de familia `corrida`
    /// (pedido 502): a morte dela e vista pelo `join` do relogio, que a anota
    /// como corrida que falhou. Por isso ela repara em vez de abortar: abortar
    /// ali fazia o job rodar de novo no arranque, e abortar de novo.
    pub(super) fn reparar_a_trava(&self, dados: &Instancia, marca_em_voo: Option<&MarcaEmVoo>) {
        let n = self.panicos_na_trava.fetch_add(1, Ordering::SeqCst) + 1;
        if crate::telemetria::familia_desta_thread() == Some("servico") {
            let fio = std::thread::current().name().unwrap_or("?").to_string();
            dizer_no_diagnostico(&format!(
                "PHXSQL: panico {n} com a trava de dados na mao na thread de \
                 SERVICO {fio}. O reparo curaria a trava e nao a thread, que \
                 morreria calada. O processo vai ABORTAR: o supervisor sobe de \
                 novo, com todas as threads, e o arranque repara com a porta \
                 fechada (pedido 451, A2)."
            ));
            std::process::abort();
        }
        let comeco = Instant::now();
        match self.reparo_da_trava(dados, marca_em_voo) {
            Ok(feito) => {
                // So DEPOIS do reparo inteiro: e este numero que faz o
                // `depois_do_veneno` deixar a trava atender de novo.
                self.reparos_da_trava.fetch_add(1, Ordering::SeqCst);
                dizer_no_diagnostico(&format!(
                    "PHXSQL Reparo da trava de dados -- panico {n} com a trava na mao \
                     (pedido 451)\n{feito}\x20 tempo ......................... {} ms\n\
                     \x20 a trava volta a atender. A tabela que a operacao interrompida \
                     gravava FORA de transacao pode ter ficado com o indice para tras: \
                     se ela recusar mandando reparar, rode o `reindexar`. A cascata \
                     solta tem marca, e a marca dela foi completada acima (pedido 540).",
                    comeco.elapsed().as_millis()
                ));
            }
            Err(motivo) => {
                dizer_no_diagnostico(&format!(
                    "PHXSQL: o reparo da trava de dados FALHOU depois do panico {n} \
                     ({motivo}). O processo vai ABORTAR em vez de servir de estado \
                     incerto: o supervisor sobe de novo e o arranque repara com a \
                     porta fechada (pedido 451, H5)."
                ));
                std::process::abort();
            }
        }
    }

    /// So nos testes: o panico da thread de pulso, enquanto houver algum
    /// pedido em `panicos_no_pulso_de_teste` -- pedido 452.
    #[cfg(test)]
    pub(super) fn panico_no_pulso_de_teste(&self) {
        let pedido = &self.panicos_no_pulso_de_teste;
        let armado = pedido
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| match n {
                0 => None,
                u32::MAX => Some(u32::MAX),
                n => Some(n - 1),
            })
            .is_ok();
        if armado {
            self.panicos_no_pulso_dados.fetch_add(1, Ordering::SeqCst);
            panic!("panico de teste na thread de pulso (pedido 452)");
        }
    }

    /// So nos testes: o panico de teste pedido para ESTA operacao, na thread
    /// que a atende. Ver `panico_de_teste_na_op`.
    #[cfg(test)]
    pub(super) fn armar_panico_de_teste(&self, op: &str) {
        let Ok(mut pedido) = self.panico_de_teste_na_op.lock() else {
            return;
        };
        if pedido.as_ref().is_none_or(|(o, _)| o != op) {
            return;
        }
        let armado = pedido.take();
        // A trava do campo sai ANTES do panico: cair com ela na mao a
        // envenenaria, e o teste seguinte leria veneno em vez de arma.
        drop(pedido);
        match armado {
            Some((_, PanicoDeTeste::NoMotor(ponto))) => {
                phxsql_store::ndx::panico_de_teste::armar(ponto);
            }
            Some((_, PanicoDeTeste::PausaNoMotor(ponto, n, aviso))) => {
                phxsql_store::ndx::panico_de_teste::armar_pausa(ponto, n, aviso);
            }
            Some((_, PanicoDeTeste::ForaDaTrava)) => {
                panic!("panico de teste FORA da trava, no despachar de {op}");
            }
            Some((_, PanicoDeTeste::Aqui)) => {
                panic!("panico de teste em {op}");
            }
            Some((_, PanicoDeTeste::Pausa(quanto, aviso))) => {
                eprintln!("{aviso}");
                std::thread::sleep(quanto);
            }
            None => {}
        }
    }

    /// So nos testes: o gancho do fecho da janela para `chave`. Entra em
    /// panico se armado para ela e para esta thread; devolve `true` quando a
    /// tabela deve contar como NAO sincronizada. Ver `panico_no_fecho_de_teste`.
    #[cfg(test)]
    fn fecho_de_teste(&self, chave: &str) -> bool {
        let fio = std::thread::current().name().unwrap_or("").to_string();
        if let Ok(mut armado) = self.panico_no_fecho_de_teste.lock() {
            let dispara = armado.as_ref().is_some_and(|(alvo, prefixo)| {
                (alvo.is_empty() || alvo == chave) && fio.starts_with(prefixo.as_str())
            });
            if dispara {
                *armado = None;
                drop(armado);
                panic!("panico de teste no fecho da janela, em {chave} (thread {fio})");
            }
        }
        self.fecho_falha_de_teste
            .lock()
            .map(|f| f.as_deref() == Some(chave))
            .unwrap_or(false)
    }

    /// **Pedido 536:** a tabela que sumiu pelo nome -- excluida ou renomeada
    /// -- sai das sujas pelo nome velho. A chave das sujas e o nome: depois do
    /// `excluir_tabela` ou do `renomear_tabela` ela nao abre mais, e o fecho
    /// segurava TODAS as marcas de COMMIT por ela (o lado seguro dele) ate o
    /// processo cair -- e no arranque a marca reaplicava `Atualizar` velho por
    /// cima do que foi escrito depois.
    ///
    /// Renomeada, a chave MUDA de nome, e o `fsync` fica com o fecho seguinte,
    /// que abre a tabela pelo nome novo: a marca continua esperando por um
    /// `fsync` posterior a escrita, como o invariante do
    /// `descarregar_sujas_com` pede. Excluida, a chave so sai -- o dado que a
    /// marca esperava levar ao disco acabou de ser apagado. Nenhum `fsync`
    /// entra aqui com a trava na mao: a catraca `alcancam-fsync-2` do mapa da
    /// trava mediu os dois caminhos e nao sobe. Quem chama segura a trava de
    /// dados, e ninguem suja a tabela entre a troca de nome e esta linha.
    pub(super) fn renomear_nas_sujas(&self, database: &str, tabela: &str, destino: Option<&str>) {
        let Ok(mut sujas) = self.sujas.lock() else {
            return;
        };
        if sujas.remove(&format!("{database}/{tabela}")) {
            if let Some(d) = destino {
                sujas.insert(format!("{database}/{d}"));
            }
        }
    }

    /// Tira das sujas as chaves que o fecho ja levou ao disco -- e so elas.
    /// Ver o A1 no `descarregar_sujas_com`.
    fn tirar_das_sujas(&self, chaves: &[String]) {
        if chaves.is_empty() {
            return;
        }
        if let Ok(mut s) = self.sujas.lock() {
            for chave in chaves {
                s.remove(chave);
            }
        }
    }

    /// O corpo do reparo: `Ok` com as linhas do relatorio, `Err` com o motivo
    /// de ele nao poder ser afirmado. Ver [`Servidor::reparar_a_trava`].
    pub(super) fn reparo_da_trava(
        &self,
        dados: &Instancia,
        marca_em_voo: Option<&MarcaEmVoo>,
    ) -> std::result::Result<String, String> {
        #[cfg(test)]
        if self.reparo_falha_de_teste.load(Ordering::SeqCst) {
            return Err("falha de teste forcada".into());
        }
        #[cfg(test)]
        if self.reparo_panica_de_teste.load(Ordering::SeqCst) {
            panic!("panico de teste DENTRO do reparo da trava");
        }
        let mut feito = String::new();
        // 1. A janela de durabilidade, como o fecho de sempre. Tabela que nao
        //    sincroniza continua suja e segura as marcas pendentes -- e o que o
        //    fecho de sempre faz com ela, e nao e falha do reparo: a proxima
        //    passada tenta de novo, e nenhuma marca sai antes do disco.
        //
        //    As sujas ENVENENADAS sao falha: o fecho volta calado sem elas, e
        //    daqui em diante nenhuma gravacao se anotaria para o `fsync` -- a
        //    janela inteira deixaria de valer, e isso nao se afirma de pe.
        if self.sujas.is_poisoned() {
            return Err("a trava das tabelas sujas esta envenenada: a janela de \
                 durabilidade nao se afirma"
                .into());
        }
        let sujas = self.sujas.lock().map(|s| s.len()).unwrap_or(0);
        self.descarregar_sujas_com(dados);
        let ficaram = self.sujas.lock().map(|s| s.len()).unwrap_or(0);
        feito.push_str(&format!(
            "\x20 tabelas da janela sincronizadas  {}{}\n",
            sujas.saturating_sub(ficaram),
            if ficaram > 0 {
                format!("   ({ficaram} continuam devendo ao disco, e as marcas pendentes ficam)")
            } else {
                String::new()
            }
        ));
        // 2. A marca EM VOO, e so ela. Completa-la com operacao impossivel,
        //    ou parada por falta da chave, deixa a marca no disco -- e o
        //    `AoSair` vai soltar as travas da transacao dela logo depois deste
        //    reparo, deixando outro escritor tomar o rowid que ela reservou. O
        //    arranque, com nada congelado e a porta fechada, e quem a resolve;
        //    entao a resposta e cair, e nao seguir. E o mesmo criterio de
        //    «pendente» do braco de erro do `COMMIT`.
        //
        //    E a marca que ja estava gravada e nao se releu (M4) tambem: sem o
        //    `gravada`, a falha de leitura contava como «nao confere», a marca
        //    saia do disco e a trava voltava a atender com a transacao
        //    confirmada pela metade. Com ele a marca fica, a falha vai para as
        //    impossiveis, e o arranque -- com descritores novos -- a completa.
        //
        //    Pedido 700: a marca do grupo do BIDIRECIONAL nao se completa
        //    aqui. O `completar_marca_em_voo` e o motor do rowid e da posicao,
        //    e a do bidirecional casa pela chave contra o mapa de toques --
        //    reaplica-la pelo rowid gravaria por cima de linha alheia sempre
        //    que as posicoes dos dois lados coincidissem. O motor dela e o do
        //    arranque (`completar_marcas_do_bidi`), com a porta fechada; entao
        //    o reparo nao se afirma, e o processo cai com a marca no disco
        //    (H5), como a thread de servico ja cairia.
        if let Some(bidi) = marca_em_voo.filter(|m| crate::transacao::e_marca_do_bidi(&m.caminho)) {
            return Err(format!(
                "a marca em voo {} e de um grupo do bidirecional: ela se completa \
                 pela chave, no arranque, e fica no disco",
                bidi.caminho.display()
            ));
        }
        if let Some(em_voo) = marca_em_voo {
            let caminho = &em_voo.caminho;
            #[cfg(test)]
            if self.marca_ilegivel_de_teste.load(Ordering::SeqCst) {
                crate::transacao::falhar_a_proxima_leitura_de_teste();
            }
            let r = crate::transacao::completar_marca_em_voo(
                dados,
                &em_voo.database,
                caminho,
                em_voo.gravada,
            );
            if r.houve() {
                feito.push_str(&r.texto(&self.config.base));
                feito.push('\n');
            }
            if !r.impossiveis.is_empty()
                || !r.paradas.is_empty()
                || (em_voo.gravada && r.completadas == 0)
            {
                return Err(format!(
                    "a marca em voo {} nao se completou: {}",
                    caminho.display(),
                    r.impossiveis
                        .iter()
                        .chain(r.paradas.iter())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
        }
        // 3. Os residentes. A trava deles envenenada ja recusa todo leitor da
        //    memoria (pedido 458), entao nao ha copia servida para soltar.
        let soltas = match self.residentes.lock() {
            Ok(mut m) => {
                let n = m.len();
                m.clear();
                n
            }
            Err(_) => 0,
        };
        feito.push_str(&format!("\x20 copias residentes soltas ...... {soltas}\n"));
        Ok(feito)
    }

    /// Alguma tabela desta transacao continua devendo ao disco?
    fn tabelas_ainda_sujas(&self, database: &str, escritas: &[crate::transacao::Escrita]) -> bool {
        let Ok(sujas) = self.sujas.lock() else {
            // Trava envenenada: o lado seguro e SEGURAR a marca. Uma marca a
            // mais custa uma varredura no proximo arranque; uma marca a menos
            // custa o dado.
            return true;
        };
        escritas
            .iter()
            .any(|e| sujas.contains(&format!("{database}/{}", e.tabela)))
    }

    /// Ha quanto o disco esta devendo, para quem quiser mostrar.
    pub fn pendentes_de_gravacao(&self) -> u64 {
        self.janela.pendente()
    }
}

/// A sentinela do pedido 509, na raiz da instancia.
///
/// # O buraco que ela fecha
///
/// O `fsync` recusado derruba o processo -- mas o supervisor que o sobe de
/// novo no MESMO boot encontra o cache do nucleo devolvendo o que o disco
/// perdeu: medido pelo papel C (3/3), o arranque le as 5.000 linhas do
/// cache, a recuperacao as da por aplicadas, a marca sai, e depois de
/// remontar o `.log` esta ilegivel e o `.ndx` com CRC invalido. Nenhum dos
/// quatro motores de referencia trata o caso; a saida (a) -- nao subir no
/// mesmo boot -- foi decisao do dono (SLA), em 30/09/2026.
///
/// # O formato
///
/// Texto, tres linhas `chave=valor`: `boot_id` (o de
/// `/proc/sys/kernel/random/boot_id`, ou vazio onde nao ha), `caminho` (o
/// arquivo cujo `fsync` foi recusado) e `quando_ms`. Ver `docs/FORMATO.md`.
///
/// Gravada SEM `fsync`, de proposito: quem precisa le-la e o arranque do
/// MESMO boot, pelo mesmo cache. Depois de reiniciar, perde-la nao custa nada
/// -- o cache que mentia tambem se foi.
pub(super) const SENTINELA_509: &str = ".fsync-recusado";

/// O identificador deste boot. `None` onde o sistema nao o diz.
pub(super) fn boot_id() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn gravar_sentinela_509(caminho: &Path, e: &std::io::Error) {
    let bases = BASES_DA_SENTINELA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let texto = format!(
        "boot_id={}\ncaminho={}\nerro={e}\nquando_ms={}\n",
        boot_id().unwrap_or_default(),
        caminho.display(),
        crate::agora_ms()
    );
    // Em TODAS as raizes deste processo: a recusa pode ter sido fora de
    // qualquer uma (o diretorio do config), e o lado seguro e nenhuma subir.
    for b in bases {
        let _ = std::fs::write(b.join(SENTINELA_509), &texto);
    }
}

/// O gancho do processo para o disco que recusa -- o `abort`.
///
/// Serve a DUAS quedas pelo mesmo motor (`sincronia::Queda`): o `fsync`
/// recusado do 509, e o `.log` que falhou depois de a linha estar no `.reg`
/// (pedido 498, decisao do dono de 30/09/2026: derrubar e completar, como o
/// PANIC do PostgreSQL na falha de escrita do WAL). So a primeira grava a
/// sentinela: ali o cache do nucleo mente no mesmo boot. Na segunda o disco
/// so encheu -- a marca do evento devido ficou no cabecalho do `.log`, e a
/// primeira abertura da tabela depois de subir completa o evento pela linha.
pub(super) fn fsync_recusado_derruba_o_processo(
    queda: phxsql_store::sincronia::Queda,
    caminho: &Path,
    e: &std::io::Error,
) {
    if queda == phxsql_store::sincronia::Queda::DiarioSemEvento {
        dizer_no_diagnostico(&format!(
            "PHXSQL: o diario {} nao gravou o evento de uma linha que JA esta no \
             .reg ({e}). Linha sem diario e replica divergindo e cascata pulada, \
             entao o processo vai ABORTAR em vez de seguir de pe, como o PostgreSQL \
             (PANIC) na falha de escrita do WAL. Libere espaco e suba de novo: a \
             primeira abertura da tabela completa o evento pela marca no cabecalho \
             do .log (pedido 498).",
            caminho.display()
        ));
        std::process::abort();
    }
    gravar_sentinela_509(caminho, e);
    dizer_no_diagnostico(&format!(
        "PHXSQL: o fsync de {} foi RECUSADO ({e}). Depois disso o nucleo pode ter \
         descartado o que nao foi ao disco, e o proximo fsync responderia Ok sem \
         o dado estar la. O processo vai ABORTAR em vez de confirmar mais nada \
         sobre este disco, como o PostgreSQL (PANIC) e o InnoDB (ib::fatal). \
         Antes de subir de novo, remonte o volume ou reinicie a maquina: no mesmo \
         boot o cache do nucleo devolve o que o disco perdeu, e o arranque nao \
         tem como ver a diferenca (pedido 509).",
        caminho.display()
    ));
    std::process::abort();
}

/// Uma linha no erro padrao que NUNCA entra em panico.
///
/// O `eprintln!` entra em panico se o erro padrao estiver fechado -- e o
/// reparo da trava roda no desenrolar de um panico, onde um segundo panico
/// aborta o processo. Abortar por nao conseguir ESCREVER o relatorio de um
/// reparo que deu certo seria derrubar todas as conexoes por um log.
fn dizer_no_diagnostico(texto: &str) {
    let _ = writeln!(std::io::stderr(), "{texto}");
}

/// So nos testes: o panico que o `panico_de_teste_na_op` pede -- pedido 451.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum PanicoDeTeste {
    /// O panico do motor, armado nesta thread: no meio de uma escrita de
    /// verdade, com a trava de dados na mao.
    NoMotor(phxsql_store::ndx::panico_de_teste::Ponto),
    /// A PAUSA sem fim do motor na `n`-esima passagem pelo ponto, com a trava
    /// na mao, dizendo o texto dado no erro padrao -- o processo parado que a
    /// prova de `SIGKILL` mata (pedido 540).
    PausaNoMotor(phxsql_store::ndx::panico_de_teste::Ponto, u32, &'static str),
    /// O panico ja no `despachar`, FORA da trava -- o que o `AoSair` da
    /// conexao desenrola tomando a trava de novo (M2).
    ForaDaTrava,
    /// O panico no proprio gancho, onde quer que ele esteja -- no backup
    /// agendado, com a ficha da copia na mao (pedidos 502 e 513).
    Aqui,
    /// A PAUSA no proprio gancho, pelo tempo dado, dizendo o texto no erro
    /// padrao -- a copia LENTA do backup, com a ficha dela na mao (pedido
    /// 513). Com prazo, e nao sem fim: o servidor de teste tem de terminar
    /// sozinho mesmo se ninguem o matar.
    Pausa(Duration, &'static str),
}

/// O pedido minimo que `abrir_travada`, `gatilhos_para` e `residente_mut`
/// esperam. Montado uma vez, e nao repetido em cinco lugares.
pub(super) fn pedido_da_tabela(database: &str, tabela: &str) -> Json {
    Json::objeto(vec![
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
    ])
}

pub(super) struct AoSair<F: FnMut()>(pub(super) F);

impl<F: FnMut()> Drop for AoSair<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
