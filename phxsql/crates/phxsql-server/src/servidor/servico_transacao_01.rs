//! A transacao: begin, escopo, empilhar, travas por linha, commit e
//! rollback.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// As operacoes de CONTROLE de transacao. Elas passam sempre, inclusive com a
/// transacao em `ABORT_ONLY` -- que so sai pelo `rollback`.
pub(crate) const OPS_DE_TRANSACAO: &[&str] = &[
    "begin",
    "start_transaction",
    "begin_transaction",
    "commit",
    "rollback",
    "savepoint",
    "rollback_para",
    "rollback_to_savepoint",
    "release_savepoint",
    "transacao",
    "transacoes",
];

/// O que uma transacao aberta EMPILHA em vez de gravar.
///
/// Lista curta e escrita a mao de proposito: o que nao esta aqui e escrita e
/// nao passa (`dentro_da_transacao` recusa nomeando), e o que nao e escrita
/// passa direto como leitura. Uma operacao de escrita nova nasce RECUSADA
/// dentro de transacao ate alguem decidir o contrario -- o mesmo principio da
/// lista de permissao do spare.
///
/// `inserir_lote` NAO entra, e a ausencia e deliberada: ele ja e uma operacao
/// atomica sozinho -- roda inteiro dentro de UMA tomada da trava -- e
/// empilha-lo linha a linha estouraria o teto de memoria da transacao com uma
/// carga de cinco mil linhas que nao precisava de transacao nenhuma.
pub(crate) const OPS_EMPILHAVEIS: &[&str] = &["inserir", "atualizar", "excluir", "restaurar"];

impl Servidor {
    // ================================================== transacoes
    //
    // `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`. O desenho inteiro esta em
    // `docs/TRANSACOES.md` e a maquina de estados em `crate::transacao`; aqui
    // fica so o que amarra as duas ao protocolo.

    /// Os tres prazos e os dois modos que valem para esta abertura.
    ///
    /// # Por que parametros NOMEADOS, e nao posicionais
    ///
    /// `Transaction(a, b, c, 5s)` nao estende -- entrou o segundo prazo, nao ha
    /// onde ele caiba sem quebrar quem ja escreveu -- e confunde tabela com
    /// duracao na mesma lista. Aqui cada coisa tem nome, e o que nao vier no
    /// pedido cai no padrao do `config.json`.
    fn abertura_do_pedido(&self, p: &Json) -> Result<crate::transacao::Abertura> {
        let r = &self.config.recursos;
        let modo = crate::travas::Modo::de_texto(p.texto_ou("lock_mode", p.texto_ou("modo", "")))
            .ok_or_else(|| {
            PhxError::Esquema("lock_mode aceita AUTO, ROW, TABLE ou EXCLUSIVE".into())
        })?;
        let escopo_modo = crate::travas::EscopoModo::de_texto(
            p.texto_ou("scope_mode", p.texto_ou("escopo_modo", "")),
        )
        .ok_or_else(|| PhxError::Esquema("scope_mode aceita DYNAMIC ou STRICT".into()))?;
        // O TETO do prazo da transacao e o do `config.json` -- pedido 607.
        //
        // O cliente pede MENOS e vale; pede mais e vale o teto, calado, como o
        // `limite` do `dblink_consultar` contra o `max_linhas`. Sem teto, um
        // `timeout_ms` de 10^12 abria uma transacao de 31 anos, e com `SCOPE
        // EXCLUSIVE` ela segurava a tabela contra todo mundo ate la.
        //
        // A regua, e por que ela nao decide sozinha: nenhum dos tres maduros
        // poe teto de servidor no prazo que a sessao pede -- o
        // `innodb_lock_wait_timeout` nasce 50 s e aceita ate 1.073.741.824 s,
        // o `lock_wait_timeout` do MySQL nasce 1 ano, o `lock_timeout` e o
        // `transaction_timeout` do PostgreSQL nascem 0 (sem prazo). So que la
        // a sessao que muda o prazo ja provou quem e e tem direito na tabela;
        // aqui o prazo vinha de quem so tinha o token. O que decide e a regra
        // escrita no proprio campo: «transacao sem prazo nenhum e exatamente
        // a que trava a tabela para sempre», e o `transacao_prazo_min` e o
        // prazo que o DONO DO SERVIDOR escreveu. Teto: 5 min de fabrica.
        //
        // O `LOCK TIMEOUT` nao ganha teto proprio: a espera ja e cortada pelo
        // prazo da transacao (`esperar_trava`, `estourou_a_transacao`), entao
        // nunca passa deste. Limita-la aqui de novo seria a mesma decisao
        // escrita duas vezes.
        let teto_ms = r.transacao_prazo_min as i64 * 60_000;
        Ok(crate::transacao::Abertura {
            transacao_ms: duracao_ms(p, "timeout", teto_ms)?.min(teto_ms),
            lock_ms: duracao_ms(p, "lock_timeout", r.transacao_lock_timeout_ms as i64)?,
            statement_ms: duracao_ms(p, "statement_timeout", r.transacao_statement_ms as i64)?,
            modo,
            escopo_modo,
            // Opt-in, no estilo do `"versao"`: quem manda ganha a leitura
            // repetivel pela trava; quem nao manda fica como sempre. Aceita o
            // nome em portugues e o do padrao.
            leitura_repetivel: p
                .booleano_ou("leitura_repetivel", p.booleano_ou("repeatable_read", false)),
        })
    }

    /// O PORTAO das transacoes, e ele vem antes de qualquer trabalho.
    ///
    /// Devolve `None` quando este pedido nao tem nada a ver com transacao --
    /// que e o caso do servidor inteiro hoje, e por isso a primeira linha e um
    /// `load` atomico e nada mais.
    ///
    /// # Um portao so, e nao espalhado
    ///
    /// A alternativa seria cada `op_inserir`/`op_atualizar`/`op_excluir`
    /// perguntar por conta se ha transacao aberta. E exatamente o antipadrao
    /// que ja produziu a porta dos fundos do `juntar` e do `unir`: a operacao
    /// que alguem esquecer grava direto no disco no meio de uma transacao, e
    /// ninguem acha isso por leitura.
    pub(super) fn dentro_da_transacao(
        &self,
        op: &str,
        p: &Json,
        sessao: &Sessao,
    ) -> Option<Result<Json>> {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 || sessao.ligacao == 0 {
            return None;
        }
        // A partir daqui ja se sabe que EXISTE transacao em algum lugar; falta
        // saber se e nesta conexao.
        let estado = {
            let t = self.transacoes.travar();
            t.de(sessao.ligacao)?.estado
        };

        // As operacoes de controle passam sempre -- inclusive em ABORT_ONLY,
        // que so sai pelo `rollback`.
        if OPS_DE_TRANSACAO.contains(&op) {
            return None;
        }

        // PEDIDO 262, etapa 1: escrita com a transacao em `COMMITTING` so vem
        // de um gatilho AFTER disparado no COMMIT -- e a mesma sessao, e nao
        // ha outra porta. Ela caia no `empilhar` de uma lista que o COMMIT ja
        // tinha tirado, depois de a marca selada, e ia ao chao com o descarte
        // da transacao: calada no sucesso, barulhenta so no erro.
        //
        // Trocar o estado para `Ativa` mediria ZERO (parecer do 262, §2): a
        // escrita cairia na mesma lista vazia e sumiria do mesmo jeito. Gravar
        // de verdade exige rodar o AFTER antes da marca (a etapa 2). Ate la, a
        // recusa NOMEADA sobe pelo canal que ja existe para o gatilho que
        // falha -- `gatilhos_avisos` na resposta do COMMIT --, que continua
        // `COMMITTED` e com o mesmo `gravadas`: nada que hoje funciona passa a
        // falhar. Vem antes do prazo de proposito: a transacao que esta sendo
        // confirmada nao se estoura pelo corpo do proprio gatilho.
        if estado == crate::transacao::Estado::Confirmando
            && (OPS_EMPILHAVEIS.contains(&op) || OPS_ESCRITA.contains(&op))
        {
            return Some(Err(PhxError::Esquema(format!(
                "este {op} em {} veio de um gatilho AFTER disparado no COMMIT: a \
                 transacao ja esta confirmando e nao aceita escrita nova -- a linha \
                 do gatilho NAO foi gravada (pedido 262)",
                p.texto_ou("tabela", "?")
            ))));
        }

        // O PRAZO DA TRANSACAO, conferido na hora de usar. Estourado, quem
        // encerra e o gestor: `ABORT_ONLY`, travas soltas, lista jogada fora,
        // e a proxima operacao recebe o erro com o numero do prazo. Nenhuma
        // thread e morta -- matar thread deixaria estado interno pela metade.
        let vencida = self
            .transacoes
            .travar()
            .de(sessao.ligacao)
            .is_some_and(|tx| tx.expira_ms <= crate::agora_ms());
        if vencida && estado != crate::transacao::Estado::AbortOnly {
            return Some(Err(self.estourar_prazo(sessao)));
        }

        if estado == crate::transacao::Estado::AbortOnly && Atividade::da_operacao(op).is_some() {
            return Some(Err(PhxError::TransacaoAbortada(
                self.motivo_do_aborto(sessao.ligacao),
            )));
        }

        if OPS_EMPILHAVEIS.contains(&op) {
            self.por_prazo_na_operacao(sessao);
            return Some(self.empilhar(op, p, sessao));
        }

        // O STATEMENT TIMEOUT vale tambem para o que a transacao manda
        // executar sem empilhar -- uma consulta longa no meio dela. Ele e
        // posto no relogio da ATIVIDADE desta conexao, que e a mesma maquina
        // de cancelamento cooperativo do `telemetria_encerrar`: quem para a
        // operacao e o laco dela, num ponto seguro, e nunca uma thread morta.
        self.por_prazo_na_operacao(sessao);

        // Escrita que a transacao nao sabe empilhar NAO passa direto ao disco,
        // e tambem nao confirma a transacao pelas costas de ninguem.
        //
        // O MySQL(R) e o Oracle confirmam a transacao aberta quando chega um
        // DDL, e isso e uma armadilha conhecida: quem escreveu `BEGIN; ...;
        // CREATE TABLE; ROLLBACK` acha que desfez e nao desfez. Recusar e a
        // resposta honesta enquanto o DDL nao for transacional -- e o que
        // falta para ele ser esta escrito no documento.
        if OPS_ESCRITA.contains(&op) {
            return Some(Err(PhxError::Esquema(format!(
                "{op} nao entra em transacao: esta rodada empilha {}. \
                 E ela NAO confirma a transacao aberta por conta propria -- \
                 termine com COMMIT ou ROLLBACK e repita",
                OPS_EMPILHAVEIS.join(", ")
            ))));
        }
        // LEITURA REPETIVEL PELA TRAVA (`docs/SOMBRA.md` §5b): a transacao que
        // PEDIU segura a compartilhada (S) na tabela que vai ler, ate o fim.
        // Entra aqui e nao em cada `op_*`: este portao e o irmao do
        // `despachar` e do `executar_derivado`, entao o `buscar`/`ler` que uma
        // juncao ou um SQL deriva passa por ele tabela a tabela.
        if let Some(r) = self.travar_leitura_repetivel(p, sessao) {
            return Some(r);
        }
        // Leitura passa direto, e isso e o isolamento declarado: a transacao
        // le o CONFIRMADO e nao ve as proprias escritas, porque elas estao em
        // RAM. Esta escrito na §4.3 e a tela diz com todas as letras.
        None
    }

    /// LEITURA REPETIVEL PELA TRAVA (`docs/SOMBRA.md` §5b), no unico lugar.
    ///
    /// A transacao que PEDIU `"leitura_repetivel": true` segura a trava
    /// compartilhada (S) em cada tabela que LE, ate o COMMIT/ROLLBACK -- e o
    /// escritor espera esse leitor. A consistencia sai por exclusao, nao por
    /// versao: sob a S nenhuma escrita entra, entao a releitura devolve o
    /// mesmo estado e nenhuma linha nasce no meio. E a visao de varias
    /// tabelas sai coerente sem instante inventado, porque cada tabela lida
    /// toma a sua S por este mesmo portao.
    ///
    /// Devolve `None` para deixar a leitura seguir (a S foi tomada, ou nao
    /// era o caso), e `Some(Err)` quando a espera pela S estourou o `LOCK
    /// TIMEOUT` -- a recusa e do LEITOR, e o escritor mantem a vazao (§7 do
    /// SOMBRA). Quem nao pediu leitura repetivel nunca chega ao `esperar`.
    fn travar_leitura_repetivel(&self, p: &Json, sessao: &Sessao) -> Option<Result<Json>> {
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if tabela.is_empty() {
            return None;
        }
        let (id, quer, database_tx) = {
            let t = self.transacoes.travar();
            let tx = t.de(sessao.ligacao)?;
            (tx.id, tx.leitura_repetivel, tx.database.clone())
        };
        if !quer {
            return None;
        }
        let database = {
            let d = p.texto_ou("database", "").trim().to_string();
            if d.is_empty() {
                database_tx
            } else {
                d
            }
        };
        if database.is_empty() {
            // Sem database nao ha tabela para abrir: a leitura falha sozinha
            // adiante, e nao ha o que travar.
            return None;
        }
        let chave = crate::carga::chave(&database, &tabela);
        match self.esperar_trava(
            sessao,
            id,
            &chave,
            Alvo::Tabela(crate::travas::Trava::Compartilhada),
            Espera::AteOPrazo,
        ) {
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        }
    }

    fn motivo_do_aborto(&self, ligacao: u64) -> String {
        let padrao = "houve erro de TRANSACAO nesta conexao: a transacao nao \
                      pode ser confirmada. Mande ROLLBACK";
        match self.transacoes.travar().de(ligacao) {
            Some(tx) if !tx.motivo_do_aborto.is_empty() => format!(
                "{}; a transacao nao pode ser confirmada, mande ROLLBACK",
                tx.motivo_do_aborto
            ),
            _ => padrao.to_string(),
        }
    }

    /// Marca a transacao desta conexao como `ABORT_ONLY`.
    ///
    /// Chamado quando um erro de classe TRANSACAO acontece: dali em diante o
    /// `COMMIT` recusa em vez de confirmar trabalho meio invalido.
    pub(super) fn abortar_transacao(&self, ligacao: u64, motivo: &str) {
        if let Some(tx) = self.transacoes.travar().de_mut(ligacao) {
            tx.estado = crate::transacao::Estado::AbortOnly;
            if tx.motivo_do_aborto.is_empty() {
                tx.motivo_do_aborto = motivo.to_string();
            }
        }
    }

    /// `begin` / `start_transaction` / `begin_transaction`, com o escopo e os
    /// prazos declarados na abertura.
    ///
    /// ```text
    /// BEGIN TRANSACTION
    ///   SCOPE (clientes, pedidos, pediditens, estoque)
    ///   TIMEOUT 5s
    ///   LOCK TIMEOUT 500ms
    ///   LOCK MODE AUTO;
    /// ```
    ///
    /// # O que a declaracao previa COMPRA
    ///
    /// Ela e o que paga pela volta da espera. Travar por linha desfaz o
    /// conflito artificial entre dois caixas em pedidos diferentes, mas traz
    /// de volta a possibilidade de ciclo -- e com as tabelas conhecidas aqui,
    /// as travas de tabela sao tomadas SEMPRE na mesma ordem canonica, o que
    /// mata o ciclo classico entre tabelas. Ver `crate::travas`.
    pub(super) fn op_begin(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // Pela mesma razao do `BULKINSERT`, e com o mesmo tamanho de estrago:
        // HTTP nao tem conexao para cair, entao a primeira rede de protecao
        // contra transacao orfa nao existiria. A tela nao fica sem
        // atomicidade -- ela manda as operacoes num pedido so.
        if sessao.ligacao == 0 {
            return Err(PhxError::Esquema(
                "transacao so vale pela porta de dados: HTTP nao tem conexao \
                 para ela morrer amarrada, e uma aba fechada deixaria tabelas \
                 presas ate o prazo. Pela tela, mande as operacoes num pedido \
                 so"
                .into(),
            ));
        }
        self.limpar_transacoes_vencidas();
        let abertura = self.abertura_do_pedido(p)?;
        let database = p.texto_ou("database", "").trim().to_string();
        let declaradas = lista_do_escopo(p);
        if !declaradas.is_empty() && database.is_empty() {
            return Err(PhxError::Esquema(
                "SCOPE precisa do \"database\": uma transacao abrange UM \
                 database, e a marca de recuperacao mora dentro dele"
                    .into(),
            ));
        }
        let agora = crate::agora_ms();
        let id = {
            let mut t = self.transacoes.travar();
            t.abrir(sessao.ligacao, sessao.login(), &sessao.ip, agora, &abertura)?
        };
        // Depois de a transacao estar no mapa: o portao le este contador, e
        // incrementa-lo antes deixaria uma janela em que ele diz "ha
        // transacao" e o mapa ainda nao tem nenhuma.
        self.transacoes_abertas.fetch_add(1, Ordering::SeqCst);

        // Daqui para baixo, qualquer falha DESFAZ a transacao recem-aberta:
        // deixa-la meio aberta seguraria travas que ninguem sabe que existem.
        if let Err(e) = self.declarar_escopo(id, &database, &declaradas, sessao) {
            self.descartar_transacao(sessao.ligacao);
            return Err(e);
        }
        let ficha = {
            let t = self.transacoes.travar();
            t.de(sessao.ligacao).ok_or_else(sem_transacao)?.ficha(agora)
        };
        Ok(self.juntar_travas(ficha, agora))
    }

    /// Calcula o escopo EFETIVO e toma as travas de tabela em ordem canonica.
    ///
    /// Sem `SCOPE` declarado nao ha nada a fazer aqui: as tabelas entram uma a
    /// uma, na primeira escrita de cada -- e e por isso que **quem nunca
    /// declarou escopo nao sente diferenca nenhuma**.
    fn declarar_escopo(
        &self,
        id: u64,
        database: &str,
        declaradas: &[String],
        sessao: &Sessao,
    ) -> Result<()> {
        if declaradas.is_empty() {
            return Ok(());
        }
        let modo = self.modo_da_transacao(sessao)?;
        // O direito em CADA declarada, e ANTES de perguntar se ela existe --
        // pedido 607. O `SCOPE` nomeia tabela num campo que o portao 3 nao le
        // (o mesmo furo do `juntar`, do `unir` e do `pivotar`), entao paga a
        // conferencia propria, com a recusa do portao. E a ordem e a defesa
        // contra o oraculo: quem nao tem direito ouve a MESMA recusa para a
        // tabela que existe e para a que nao existe, e nao aprende o catalogo.
        if let Some(u) = &sessao.usuario {
            let pede = direitos_da_trava(modo.na_tabela());
            for nome in declaradas {
                if !pede.iter().any(|a| u.pode_em(database, nome, *a)) {
                    return Err(self.recusa_sem_direito(u, pede[0], database, nome));
                }
            }
        }
        // A tabela declarada TEM de existir, e o erro sai aqui em vez de mais
        // tarde. Sem esta conferencia, `SCOPE (pediditens)` escrito com um
        // erro de digitacao era aceito calado: a trava ia para uma chave que
        // nao aponta para nada, e o `STRICT` recusava depois a tabela CERTA,
        // dizendo que ela nao estava no escopo. O engano ficava a duas
        // mensagens de distancia da causa.
        {
            let trava = self.travar_dados()?;
            let db = trava.abrir_database(database)?;
            for nome in declaradas {
                if db.abrir_qualificada(nome).is_err() {
                    return Err(PhxError::NaoEncontrado(format!(
                        "{database}.{nome} esta no SCOPE e nao existe"
                    )));
                }
            }
        }
        let efetivas = self.escopo_efetivo(database, declaradas, sessao)?;
        {
            let mut t = self.transacoes.travar();
            let tx = t.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
            tx.database = database.to_string();
            tx.declaradas = declaradas
                .iter()
                .map(|n| crate::carga::chave(database, n))
                .collect();
            crate::travas::em_ordem_canonica(&mut tx.declaradas);
            tx.efetivas = efetivas.clone();
        }
        // ORDEM CANONICA, e e ela que mata o ciclo entre tabelas: A e B pedem
        // `estoque` e `pedidos` na MESMA sequencia, entao nunca ha uma
        // segurando o que a outra quer enquanto quer o que a outra segura.
        for chave in &efetivas {
            self.esperar_trava(
                sessao,
                id,
                chave,
                Alvo::Tabela(modo.na_tabela()),
                Espera::AteOPrazo,
            )?;
        }
        Ok(())
    }

    /// O escopo EFETIVO: o declarado mais o que as dependencias do catalogo
    /// alcancam. Devolve `(efetivo, como cada uma chegou)`.
    ///
    /// # A chave estrangeira NAO entra -- e desde a SP000057 isso e um GAP,
    /// nao mais uma constatacao
    ///
    /// O texto que estava aqui dizia que somar as tabelas apontadas por chave
    /// estrangeira nao adiantava nada, porque *o motor DECLARA a chave e nao a
    /// IMPOE*. Isso deixou de ser verdade em duas etapas, e o comentario ficou
    /// para tras nas duas: a SP000008 ligou `conferir_fks` e `conferir_filhas`
    /// (que so LEEM as irmas), e a SP000057 ligou o `ao_alterar`, que
    /// **ESCREVE** nelas -- `Table::planejar_ao_alterar` leva a alteracao da
    /// chave da mae ate as filhas, e a cadeia segue ate a neta.
    ///
    /// O teste citado no texto velho -- `a_chave_e_declarada_mas_ainda_nao_e_-
    /// imposta_na_gravacao` -- ja nao existe. Um comentario que aponta para um
    /// teste apagado nao trava nada; e por isso que ele esta sendo reescrito
    /// em vez de corrigido.
    ///
    /// **A consequencia, dita com todas as letras:** uma transacao que declara
    /// so a mae e altera a coluna referenciada dela grava na FILHA sem ter
    /// declarado a filha. A trava global de dados serializa a escrita, entao
    /// nao ha corrida; o que falta e o ESCOPO -- a filha nao esta na lista de
    /// travas da transacao, e um desfazer nao a alcanca. Somar o fecho da FK
    /// aqui e a correcao, e ela e decisao da frente que manda na concorrencia:
    /// aumentar escopo efetivo muda quem espera por quem, e isso se decide
    /// medindo, nao de passagem. O lugar continua sendo este e o
    /// `escopo_por_gatilho` ao lado.
    ///
    /// # O GATILHO entra -- e, dentro da transacao, ainda NAO grava
    ///
    /// O corpo de um gatilho grava noutra tabela com `INSERT INTO`. FORA de
    /// transacao isso acontece de verdade: o `rodar_gatilhos_depois` executa
    /// depois da escrita. DENTRO de uma, o AFTER roda no `COMMIT`, depois da
    /// marca, e a escrita dele nao tem mais lista onde entrar: ate o pedido
    /// 262 ela ia para a lista ja esvaziada e sumia calada; desde a etapa 1
    /// ela e RECUSADA com nome e chega ao cliente em `gatilhos_avisos`. Gravar
    /// de verdade e a etapa 2 (o AFTER antes da marca, que depende da
    /// numeracao no `INSERT` do `docs/AUTONUMBER.md` §B.4). Este texto dizia
    /// «acontece de verdade» sem a ressalva, e era falso dentro da transacao.
    ///
    /// O alvo de cada `INSERT` do corpo entra no escopo efetivo mesmo assim --
    /// a trava e tomada para a escrita que a etapa 2 vai fazer --, e o fecho
    /// e transitivo: o gatilho de `auditoria` pode ter gatilho.
    fn escopo_efetivo(
        &self,
        database: &str,
        declaradas: &[String],
        _sessao: &Sessao,
    ) -> Result<Vec<String>> {
        let mut efetivas: Vec<String> = declaradas
            .iter()
            .map(|n| crate::carga::chave(database, n))
            .collect();
        // Quem ja esta dentro nao entra de novo -- e o conjunto tambem serve
        // de marca de visita, para o fecho transitivo nao girar num gatilho
        // que grava na propria tabela.
        let mut dentro: std::collections::HashSet<String> = efetivas.iter().cloned().collect();
        // Fila de trabalho com o nome ORIGINAL (nao a chave), porque e assim
        // que o cadastro de gatilhos guarda a tabela.
        let mut fila: Vec<String> = declaradas.to_vec();
        // Teto de voltas: um gatilho que grava na propria tabela e legitimo, e
        // sem teto o fecho transitivo de um ciclo nao termina.
        let mut voltas = 0;
        while let Some(tabela) = fila.pop() {
            voltas += 1;
            if voltas > 256 {
                break;
            }
            for alvo in self.escopo_por_gatilho(database, &tabela)? {
                let chave = crate::carga::chave(database, &alvo);
                if !dentro.insert(chave.clone()) {
                    continue;
                }
                efetivas.push(chave);
                fila.push(alvo);
            }
        }
        crate::travas::em_ordem_canonica(&mut efetivas);
        Ok(efetivas)
    }

    /// As tabelas em que os gatilhos DESTA tabela gravam.
    ///
    /// Sai do programa ja compilado, e nao do texto do corpo: comparar texto
    /// para descobrir onde um gatilho grava e a mesma armadilha de resolver
    /// texto de tela por frase -- quebra calado no dia em que alguem escrever
    /// o mesmo `INSERT` com outro espacamento.
    fn escopo_por_gatilho(&self, database: &str, tabela: &str) -> Result<Vec<String>> {
        if !self.ha_gatilhos.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        let r = self.rotinas.tomar("rotinas")?;
        let mut alvos = Vec::new();
        for evento in [
            phxsql_sql::rotina::Evento::Inserir,
            phxsql_sql::rotina::Evento::Atualizar,
            phxsql_sql::rotina::Evento::Excluir,
        ] {
            let (antes, depois) = r.gatilhos_de(database, tabela, evento);
            for g in antes.iter().chain(depois.iter()) {
                let Ok(programa) = &g.programa else { continue };
                tabelas_gravadas(programa, &mut alvos);
            }
        }
        alvos.sort();
        alvos.dedup();
        Ok(alvos)
    }

    fn modo_da_transacao(&self, sessao: &Sessao) -> Result<crate::travas::Modo> {
        let t = self.transacoes.travar();
        Ok(t.de(sessao.ligacao).ok_or_else(sem_transacao)?.modo)
    }

    /// A transacao desta conexao, ou o erro que diz que nao ha nenhuma.
    fn exigir_transacao(&self, sessao: &Sessao) -> Result<u64> {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 || sessao.ligacao == 0 {
            return Err(sem_transacao());
        }
        let t = self.transacoes.travar();
        match t.de(sessao.ligacao) {
            Some(tx) => Ok(tx.id),
            None => Err(sem_transacao()),
        }
    }

    /// `transacao`: o estado da transacao DESTA conexao.
    ///
    /// Responde tambem quando nao ha nenhuma -- `IDLE` e uma resposta, e nao
    /// um erro. E o que permite um cliente perguntar "estou em transacao?"
    /// sem tratar excecao.
    pub(super) fn op_transacao(&self, sessao: &Sessao) -> Result<Json> {
        let agora = crate::agora_ms();
        if self.transacoes_abertas.load(Ordering::Relaxed) > 0 && sessao.ligacao != 0 {
            let ficha = {
                let t = self.transacoes.travar();
                t.de(sessao.ligacao).map(|tx| tx.ficha(agora))
            };
            if let Some(f) = ficha {
                return Ok(self.juntar_travas(f, agora));
            }
        }
        Ok(Json::objeto(vec![
            ("transaction_id", Json::Nulo),
            (
                "transaction_state",
                Json::texto_de(crate::transacao::Estado::Ociosa.nome()),
            ),
            (
                "transaction_isolation",
                Json::texto_de(crate::transacao::NIVEL_DE_ISOLAMENTO),
            ),
            ("linhas", Json::de_u64(0)),
        ]))
    }

    /// `transacoes`: todas as abertas, para quem administra e para a tela.
    pub(super) fn op_transacoes(&self) -> Result<Json> {
        self.limpar_transacoes_vencidas();
        let agora = crate::agora_ms();
        let fichas = {
            let t = self.transacoes.travar();
            t.todas(agora)
        };
        let quantas = fichas.len();
        let fichas: Vec<Json> = fichas
            .into_iter()
            .map(|f| self.juntar_travas(f, agora))
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(quantas as u64)),
            ("transacoes", Json::Lista(fichas)),
            (
                "transaction_isolation",
                Json::texto_de(crate::transacao::NIVEL_DE_ISOLAMENTO),
            ),
        ]))
    }

    /// `savepoint <nome>`.
    pub(super) fn op_savepoint(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = nome_do_ponto(p)?;
        self.exigir_transacao(sessao)?;
        let mut t = self.transacoes.travar();
        let tx = t.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
        if !tx.estado.aceita_trabalho() {
            return Err(PhxError::TransacaoAbortada(
                "a transacao esta em ABORT_ONLY: so o ROLLBACK passa".into(),
            ));
        }
        // Nome repetido DESTROI o ponto anterior e cria um novo, que e o que o
        // SQL manda: quem repete o nome quer marcar aqui, e nao empilhar dois
        // pontos com a mesma etiqueta -- dois pontos iguais fariam o
        // `ROLLBACK TO` voltar para um lugar que quem escreveu nao escolheu.
        tx.pontos.retain(|x| x.nome != nome);
        let ate = tx.escritas.len();
        tx.pontos.push(crate::transacao::Ponto {
            nome: nome.clone(),
            ate,
        });
        Ok(Json::objeto(vec![
            ("savepoint", Json::texto_de(&nome)),
            ("linhas", Json::de_u64(ate as u64)),
            ("transaction_id", Json::de_u64(tx.id)),
        ]))
    }

    /// `rollback_para` / `ROLLBACK TO SAVEPOINT <nome>`.
    ///
    /// Trunca a lista de escrita naquele ponto e a transacao **continua
    /// aberta**. Num desenho em que tudo esta em RAM isso e quase de graca --
    /// e a ideia e do capitulo que o dono mandou: nao se copia a transacao,
    /// guarda-se um INDICE na lista.
    pub(super) fn op_rollback_para(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = nome_do_ponto(p)?;
        self.exigir_transacao(sessao)?;
        let (descartadas, restantes, id) = {
            let mut t = self.transacoes.travar();
            let tx = t.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
            // ABORT_ONLY NAO se conserta voltando a um ponto, e a diferenca
            // para o PostgreSQL(R) e deliberada: la TODO erro aborta a
            // transacao, e o SAVEPOINT existe justamente para resgatar quem
            // errou uma instrucao. Aqui erro de INSTRUCAO nao aborta nada --
            // a transacao continua ACTIVE --, entao so chega a ABORT_ONLY o
            // que poe em duvida o proprio conjunto de escrita. Voltar a um
            // ponto nao desfaz essa duvida.
            if tx.estado == crate::transacao::Estado::AbortOnly {
                return Err(PhxError::TransacaoAbortada(
                    "a transacao esta em ABORT_ONLY: voltar a um SAVEPOINT nao \
                     a conserta, porque o que a abortou nao foi uma instrucao. \
                     Mande ROLLBACK"
                        .into(),
                ));
            }
            let ate = tx
                .pontos
                .iter()
                .find(|x| x.nome == nome)
                .map(|x| x.ate)
                .ok_or_else(|| {
                    PhxError::NaoEncontrado(format!("nao ha SAVEPOINT {nome:?} nesta transacao"))
                })?;
            let descartadas = tx.escritas.len().saturating_sub(ate);
            tx.escritas.truncate(ate);
            // A aresta do COMMIT barrado falava da lista que acabou de ser
            // cortada (R1 da re-checagem do papel C): o elo que precisava da
            // outra transacao pode ter saido junto, e a aresta velha faria a
            // outra ceder num ciclo que nao existe mais. O proximo COMMIT
            // desta a refaz, se ainda for barrado.
            tx.commit_barrado_por = None;
            // Os pontos criados DEPOIS deste somem; o proprio fica, e e o que
            // o SQL manda -- da para voltar ao mesmo ponto duas vezes.
            tx.pontos.retain(|x| x.ate <= ate);
            (descartadas, ate, tx.id)
        };
        // As chaves unicas empilhadas TEM de acompanhar o truncamento: sem
        // isto, a chave de uma linha descartada continuaria barrando a proxima
        // igual a ela, e o SAVEPOINT deixaria de desfazer de verdade.
        self.refazer_chaves_da_transacao(sessao)?;
        Ok(Json::objeto(vec![
            ("savepoint", Json::texto_de(&nome)),
            ("descartadas", Json::de_u64(descartadas as u64)),
            ("linhas", Json::de_u64(restantes as u64)),
            ("transaction_id", Json::de_u64(id)),
            (
                "transaction_state",
                Json::texto_de(crate::transacao::Estado::Ativa.nome()),
            ),
        ]))
    }

    /// `release_savepoint`: tira o ponto e os criados depois dele, sem tocar
    /// no trabalho -- as escritas continuam empilhadas.
    pub(super) fn op_release_savepoint(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = nome_do_ponto(p)?;
        self.exigir_transacao(sessao)?;
        let mut t = self.transacoes.travar();
        let tx = t.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
        let ate = tx
            .pontos
            .iter()
            .find(|x| x.nome == nome)
            .map(|x| x.ate)
            .ok_or_else(|| {
                PhxError::NaoEncontrado(format!("nao ha SAVEPOINT {nome:?} nesta transacao"))
            })?;
        let antes = tx.pontos.len();
        tx.pontos.retain(|x| x.ate < ate);
        Ok(Json::objeto(vec![
            ("savepoint", Json::texto_de(&nome)),
            ("liberados", Json::de_u64((antes - tx.pontos.len()) as u64)),
            ("linhas", Json::de_u64(tx.escritas.len() as u64)),
            ("transaction_id", Json::de_u64(tx.id)),
        ]))
    }

    /// Recalcula o conjunto de chaves unicas empilhadas.
    fn refazer_chaves_da_transacao(&self, sessao: &Sessao) -> Result<()> {
        // O esquema de cada tabela e preciso para saber as colunas de cada
        // indice, entao a trava de dados entra -- e entra ANTES da trava das
        // transacoes, que e a ordem unica deste servidor.
        let escritas: Vec<crate::transacao::Escrita> = {
            let t = self.transacoes.travar();
            match t.de(sessao.ligacao) {
                Some(tx) => tx.escritas.clone(),
                None => return Ok(()),
            }
        };
        let mut chaves: HashMap<usize, Vec<(String, String)>> = HashMap::new();
        if !escritas.is_empty() {
            let trava = self.travar_dados()?;
            let mut abertas: HashMap<String, Table> = HashMap::new();
            for (i, e) in escritas.iter().enumerate() {
                if e.acao != crate::transacao::Acao::Inserir {
                    continue;
                }
                let chave = format!("{}/{}", e.database, e.tabela);
                let t = match abertas.entry(chave) {
                    std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
                    std::collections::hash_map::Entry::Vacant(v) => {
                        let ped = pedido_da_tabela(&e.database, &e.tabela);
                        match self.abrir_travada(&trava, &ped, sessao) {
                            Ok(t) => v.insert(t),
                            Err(_) => continue,
                        }
                    }
                };
                if let Ok(c) = chaves_unicas(t, &e.linha) {
                    chaves.insert(i, c);
                }
            }
        }
        let mut t = self.transacoes.travar();
        if let Some(tx) = t.de_mut(sessao.ligacao) {
            tx.refazer_chaves(&chaves);
        }
        Ok(())
    }

    /// Empilha uma escrita no conjunto da transacao. **Nao toca em disco.**
    ///
    /// # Por que a conferencia acontece AQUI, e nao no `COMMIT`
    ///
    /// Porque a passada de commit nao pode falhar no meio: se ela falhar, uma
    /// parte da transacao ja esta gravada e a outra nao. Entao tudo o que pode
    /// ser conferido antes e conferido antes -- a linha converte, os gatilhos
    /// BEFORE rodam, a unicidade e testada contra o indice E contra as chaves
    /// que esta mesma transacao ja empilhou. Sobrando so falha de E/S no
    /// `COMMIT`, a recuperacao tem uma resposta unica e simples.
    fn empilhar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::transacao::Acao;
        let database = p.texto_ou("database", "").trim().to_string();
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        // As chaves unicas desta linha, guardadas so DEPOIS de a escrita
        // entrar na lista. Ver a nota no ramo do `Inserir`.
        let mut chaves_novas: Vec<(String, String)> = Vec::new();
        // O upsert, lido aqui pelo mesmo motivo do `op_inserir`: `se_existir`
        // invalido e erro do pedido, e recusar depois da trava seguraria todo
        // mundo por causa de uma palavra digitada errada.
        let se_existir = match op {
            "inserir" => crate::upsert::SeExistir::de_texto(p.texto_ou("se_existir", ""))?,
            _ => None,
        };
        let atualizar = match op {
            "inserir" => crate::upsert::atualizar_do_pedido(p, se_existir)?,
            _ => None,
        };
        let acao = match op {
            "inserir" => Acao::Inserir,
            "atualizar" => Acao::Atualizar,
            "restaurar" => Acao::Restaurar,
            "excluir" if p.booleano_ou("fisico", false) => Acao::ExcluirDeVez,
            _ => Acao::ExcluirSuave,
        };

        // Um database so por transacao. A marca `.tx` mora dentro do
        // diretorio do database, e e isso que a faz viajar junto no backup e
        // na restauracao; uma marca que cobrisse dois deixaria de valer no
        // instante em que alguem restaurasse um deles sozinho.
        {
            let t = self.transacoes.travar();
            let tx = t.de(sessao.ligacao).ok_or_else(sem_transacao)?;
            if !tx.database.is_empty() && !tx.database.eq_ignore_ascii_case(&database) {
                return Err(PhxError::Esquema(format!(
                    "esta transacao ja esta no database {:?} e uma transacao \
                     abrange UM database. Transacao entre databases e two-phase \
                     commit, e e outro projeto",
                    tx.database
                )));
            }
        }

        // Os gatilhos BEFORE rodam AQUI, na instrucao -- e nao no `COMMIT`.
        // Um `SIGNAL` deles e erro de INSTRUCAO: cancela esta operacao e a
        // transacao continua `ACTIVE`, exatamente como uma chave duplicada.
        // Deixa-los para o commit faria um `SIGNAL` derrubar a passada no
        // meio, que e a unica coisa que este desenho nao pode ter.
        let evento = match acao {
            Acao::Inserir => phxsql_sql::rotina::Evento::Inserir,
            Acao::Atualizar => phxsql_sql::rotina::Evento::Atualizar,
            _ => phxsql_sql::rotina::Evento::Excluir,
        };
        let (antes, depois) = self.gatilhos_para(p, evento)?;
        Self::conferir_gatilhos_compilam(&antes, &depois)?;
        // O upsert que VIRAR `atualizar` roda o BEFORE UPDATE na instrucao,
        // sobre a linha mesclada -- o irmao do `op_inserir`, que chama as
        // mesmas funcoes na mesma ordem. Lidos aqui, antes da trava, pelo
        // mesmo motivo dos de cima; o AFTER UPDATE ja roda no COMMIT pela
        // acao empilhada, e so entra na conferencia de que compila.
        let (antes_upd, depois_upd) = match se_existir {
            Some(crate::upsert::SeExistir::Atualizar) => {
                self.gatilhos_para(p, phxsql_sql::rotina::Evento::Atualizar)?
            }
            _ => (Vec::new(), Vec::new()),
        };
        Self::conferir_gatilhos_compilam(&antes_upd, &depois_upd)?;

        // AS TRAVAS VEM ANTES DA TRAVA DE DADOS, e a ordem foi corrigida por
        // uma corrida que a revisao achou.
        //
        // Tomando-as depois, o rowid previsto era calculado com a trava de
        // dados na mao e a trava do FIM DA TABELA so era pedida DEPOIS de a
        // trava de dados sair -- e nessa fresta uma escrita comum de outra
        // conexao podia anexar. O commit descobriria a divergencia
        // (`saiu != e.rowid`), e ate ai tudo bem; o estrago vinha depois: a
        // recuperacao encontraria o slot ocupado pela linha do OUTRO e o
        // trataria como «ja aplicado», descartando a nossa em silencio.
        //
        // O que a trava precisa saber sai do PEDIDO, e nao do esquema: a
        // chave da tabela e o rowid alvo (ou o fim, no anexar). Entao ela cabe
        // aqui, antes de qualquer arquivo abrir -- e com o fim travado o
        // `slots()` nao se move mais debaixo do calculo.
        //
        // Uma instrucao que falhar DEPOIS disto (chave duplicada, por
        // exemplo) deixa a trava com a transacao ate o fim dela. E de
        // proposito: a transacao anunciou a intencao de escrever ali, e
        // devolver a trava por causa de uma instrucao recusada abriria a
        // mesma fresta pela outra ponta.
        let chave = crate::carga::chave(&database, &tabela);
        let rowid_pedido = match acao {
            Acao::Inserir => crate::travas::FIM_DA_TABELA,
            _ => self.rowid(p)?,
        };
        self.travar_para_empilhar(sessao, &chave, rowid_pedido)?;

        let trava = self.travar_dados()?;
        // DISPENSA REGISTRADA da sobreposicao, e ela e da PORTA: este e o
        // caminho que EMPILHA, e ele ja sabe o que esta pendente por conta
        // propria (`chave_ja_empilhada`, `nasceu_aqui`). O motivo inteiro
        // esta no `abrir_travada_sem_sobrepor` -- e ele virou porta porque
        // abrir pela de sempre e desligar depois montava o mapa da transacao
        // sob a trava para apaga-lo na linha seguinte.
        let mut t = self.abrir_travada_sem_sobrepor(&trava, p, sessao)?;

        // Pedido 426, D3: a tabela LIGADA por chave a uma que esta sendo
        // reescrita recusa AQUI, na instrucao, com zero aplicado -- e nao no
        // COMMIT, depois da marca, quando a conferencia da chave abrisse a
        // congelada. A propria tabela congelada ja recusou uma linha acima,
        // no `abrir`.
        if let Some(recado) = self.congelada_no_alcance(
            &trava,
            &database,
            phxsql_store::catalogo::separar_qualificado(&tabela)
                .0
                .as_deref(),
            t.diretorio(),
            &[t.nome().to_string()],
        )? {
            return Err(PhxError::EmMigracao(recado));
        }

        // A particao alfanumerica fica de fora, e o motivo e o rowid.
        //
        // Numa tabela por letra o slot da linha depende do BALDE em que ela
        // cai, e o balde sai de uma regra que mora dentro do `Table`.
        // Reproduzir essa regra aqui seria uma segunda implementacao dela --
        // e a segunda e sempre a que envelhece. Sem o rowid alvo, a marca
        // `.tx` nao consegue ser idempotente e a recuperacao deixa de ser
        // exata. Recusa fundamentada, e nao esquecimento.
        if acao == Acao::Inserir && t.esquema().paginacao().modo.por_letra() {
            return Err(PhxError::Esquema(format!(
                "{database}.{tabela} e particionada por letra, e ali o slot da \
                 linha depende do balde: o rowid alvo nao e previsivel fora do \
                 motor, e sem ele a marca de recuperacao nao e idempotente. \
                 Insira nesta tabela fora de transacao"
            )));
        }

        // A linha que a alteracao muda, como a transacao a ve -- ver
        // `linha_na_transacao`. So o ramo do `atualizar` a preenche.
        let mut vista_da_alteracao: Option<Vec<Value>> = None;
        let escrita = match acao {
            Acao::Inserir => {
                let valores_json = p
                    .campo("valores")
                    .or_else(|| p.campo("linha"))
                    .cloned()
                    .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
                let mut linha = json_para_linha(&valores_json, t.esquema())?;
                if !antes.is_empty() {
                    self.rodar_gatilhos_antes(&antes, Some(&mut linha), None, t.esquema())?;
                }

                // O UPSERT DENTRO DE UMA TRANSACAO: **ele empilha como a op
                // que ele VIROU** -- `inserir` quando a chave nao existe,
                // `atualizar` quando existe --, e a resposta diz qual.
                //
                // A alternativa seria empilhar sempre como `inserir` e deixar
                // o commit descobrir a duplicata: a transacao inteira cairia
                // no fim por causa de uma linha que o pedido mandava
                // justamente sobrescrever. Empilhar a op certa e o que faz o
                // `se_existir` querer dizer aqui o mesmo que fora.
                //
                // A decisao e contra o DISCO, porque este caminho chama
                // `ver_so_o_disco`. A chave que esta transacao JA empilhou
                // RECUSA nomeando: a linha pendente nao tem rowid em disco
                // para atualizar, e escolher qualquer um dos dois caminhos
                // seria adivinhar.
                if let Some(modo) = se_existir {
                    let indice =
                        crate::upsert::indice_do_upsert(t.esquema(), p.texto_ou("indice", ""))?;
                    let valores = valores_do_indice(t.esquema(), &indice, &linha);
                    let tem_chave = !valores.is_empty() && !valores.iter().any(Value::e_null);
                    if tem_chave {
                        let em_texto = chaves_unicas(&t, &linha)?
                            .into_iter()
                            .find(|(i, _)| *i == indice)
                            .map(|(_, c)| c);
                        if let Some(c) = &em_texto {
                            let reg = self.transacoes.travar();
                            let tx = reg.de(sessao.ligacao).ok_or_else(sem_transacao)?;
                            if tx.chave_ja_empilhada(&tabela, &indice, c) {
                                return Err(PhxError::Duplicado(format!(
                                    "o indice unico {indice} ja recebeu essa chave \
                                     nesta mesma transacao, e o \"se_existir\" decide \
                                     contra o DISCO: a linha pendente ainda nao tem \
                                     rowid para atualizar. Confirme a transacao \
                                     antes, ou mande a linha uma vez so"
                                )));
                            }
                        }
                        if let Some(rowid) = t.buscar(&indice, &valores)?.first().copied() {
                            if modo == crate::upsert::SeExistir::Ignorar {
                                // Nada a empilhar: a linha ja esta la e o
                                // pedido disse para nao mexer nela.
                                return Ok(Json::objeto(vec![
                                    ("empilhada", Json::Bool(false)),
                                    ("rowid", Json::de_u64(rowid)),
                                    ("ignorada", Json::Bool(true)),
                                ]));
                            }
                            // A trava da LINHA, que a de cima nao pegou: ali o
                            // alvo era o fim da tabela, porque ainda nao se
                            // sabia que isto viraria uma alteracao. A do fim
                            // fica com a transacao ate o fim dela, e o preco e
                            // conservador de propósito -- soltar uma trava ja
                            // tomada abriria a fresta que o comentario de cima
                            // descreve.
                            self.travar_para_empilhar(sessao, &chave, rowid)?;
                            let velha = t.ler(rowid)?.unwrap_or_default();
                            // A linha como a TRANSACAO a ve (pedido 492): a
                            // `velha` do disco continua sendo a `linha_antiga`
                            // da marca, mas a alteracao parte desta.
                            let vista = self
                                .linha_na_transacao(&mut t, &database, &tabela, rowid, sessao)?
                                .unwrap_or_else(|| velha.clone());
                            // A coluna de sistema do softdeleted vem da linha
                            // atual quando o pedido nao a mandou -- a MESMA
                            // guarda do `atualizar`, porque isto virou um.
                            if crate::valores::herda_a_marca(&valores_json, t.esquema()) {
                                crate::valores::herdar_a_marca(&mut linha, &vista, t.esquema());
                            }
                            // Com `atualizar`, o que empilha e a linha ATUAL com
                            // o SET por cima -- a mesma regra do `op_inserir`,
                            // porque isto virou um `atualizar` e o `VALUES`
                            // nao e para a linha que ja existe.
                            if let Some(set) = atualizar {
                                linha = crate::upsert::mesclar(&vista, set, t.esquema())?;
                            }
                            // O BEFORE UPDATE do ramo que o upsert virou, sobre
                            // a linha como VAI FICAR -- depois da mescla -- e
                            // antes de julgar as regras: a ordem do ramo
                            // `Acao::Atualizar` logo abaixo. Sem isto o unico
                            // BEFORE desta instrucao era o de INSERT, sobre a
                            // linha crua (G4-MOTOR, pedido 245). O OLD e a
                            // `vista`, a linha que a transacao ve, pela mesma
                            // razao do ramo irmao (pedido 538).
                            if !antes_upd.is_empty() {
                                self.rodar_gatilhos_antes(
                                    &antes_upd,
                                    Some(&mut linha),
                                    Some(&vista),
                                    t.esquema(),
                                )?;
                            }
                            // GAP 242: julga padrao/calculada/CHECK do esquema
                            // sobre a linha ja mesclada -- este upsert virou um
                            // `atualizar`, e um CHECK violado recusa AQUI, na
                            // instrucao, em vez de derrubar a transacao no
                            // COMMIT.
                            t.julgar_regras_de_escrita(&linha, false)?;
                            let nova = crate::transacao::Escrita {
                                database: database.clone(),
                                tabela: tabela.clone(),
                                acao: Acao::Atualizar,
                                rowid,
                                linha,
                                linha_antiga: velha,
                                motivo: String::new(),
                                cascata_na_lista: false,
                                elo_do_empilhar: false,
                                elo_da_cascata: false,
                            };
                            // ACID-C: o upsert que virou `atualizar` cascateia
                            // como qualquer alteracao. Planeja com a mae aberta
                            // e a trava na mao (disco estavel), contra o que a
                            // transacao ve (pedido 515), e so entao decide o
                            // caminho.
                            let plano = if nova.linha_antiga.is_empty() {
                                Vec::new()
                            } else {
                                self.planejar_cascata_empilhada(
                                    &trava,
                                    &mut t,
                                    &database,
                                    &tabela,
                                    &vista,
                                    &nova.linha,
                                    sessao,
                                )?
                            };
                            // A trava de dados sai ANTES da do registro de
                            // transacoes, como no caminho de sempre: a ordem
                            // entre as duas e o que a `COM_A_TRAVA` cobra.
                            drop(t);
                            drop(trava);
                            if plano.is_empty() {
                                return self.empilhar_escrita(
                                    sessao,
                                    nova,
                                    &database,
                                    &tabela,
                                    chave,
                                    &[],
                                    Some("atualizada"),
                                );
                            }
                            return self.empilhar_atualizar_com_cascata(
                                sessao,
                                nova,
                                &database,
                                &tabela,
                                chave,
                                &[],
                                plano,
                                Some("atualizada"),
                            );
                        }
                    }
                }

                // GAP 242: as regras do esquema -- padrao, calculada e CHECK --
                // sao JULGADAS aqui, na instrucao, ao lado da unicidade e do
                // gatilho BEFORE que ja rodam neste `empilhar`. Um CHECK violado
                // recusa ESTA instrucao e deixa a transacao `ACTIVE`, exatamente
                // como uma chave duplicada -- em vez de derrubar tudo no COMMIT.
                // So JULGA: a linha crua e a que empilha, e o COMMIT reaplica as
                // regras com o numero da `Sequence`/`rownum` ja gerado. Por
                // isso um CHECK que dependa de `Sequence`/`rownum` NAO e pego
                // aqui (o numero ainda e nulo, e a regra do SQL deixa nulo
                // passar) -- so no COMMIT. Nao se forca o numero no empilhar.
                t.julgar_regras_de_escrita(&linha, true)?;

                // A unicidade contra o INDICE, que e a mesma conferencia que o
                // `inserir` de hoje faz antes de gravar byte nenhum.
                t.conferir_unicidade(&linha, None)?;
                let chaves = chaves_unicas(&t, &linha)?;
                // E contra o que esta transacao JA empilhou -- o indice em
                // disco nao sabe das linhas que ainda estao na lista, e duas
                // iguais dentro da mesma transacao passariam pela conferencia
                // de cima e quebrariam a passada no meio.
                //
                // So PERGUNTA aqui; guardar so acontece la embaixo, depois de
                // a escrita estar empilhada de verdade. Guardar antes deixava
                // a chave de uma escrita recusada pela TRAVA na lista, e a
                // tentativa seguinte da mesma linha era acusada de duplicada
                // por si mesma.
                {
                    let reg = self.transacoes.travar();
                    let tx = reg.de(sessao.ligacao).ok_or_else(sem_transacao)?;
                    let posicao = tx.escritas.len() + 1;
                    for (indice, chave) in &chaves {
                        if tx.chave_ja_empilhada(&tabela, indice, chave) {
                            return Err(PhxError::Duplicado(format!(
                                "o indice unico {indice} ja recebeu essa chave \
                                 nesta mesma transacao; esta seria a linha \
                                 {posicao} da lista"
                            )));
                        }
                    }
                }
                chaves_novas = chaves;
                let rowid = {
                    let reg = self.transacoes.travar();
                    let tx = reg.de(sessao.ligacao).ok_or_else(sem_transacao)?;
                    t.slots() + 1 + tx.insercoes_em(&tabela)
                };
                crate::transacao::Escrita {
                    database: database.clone(),
                    tabela: tabela.clone(),
                    acao,
                    rowid,
                    linha,
                    linha_antiga: Vec::new(),
                    motivo: String::new(),
                    // Insercao nao cascateia: cascata e do `ao_alterar`.
                    cascata_na_lista: false,
                    elo_do_empilhar: false,
                    elo_da_cascata: false,
                }
            }
            Acao::Atualizar => {
                let rowid = self.rowid(p)?;
                let valores_json = p
                    .campo("valores")
                    .or_else(|| p.campo("linha"))
                    .cloned()
                    .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
                let nasceu_aqui = {
                    let reg = self.transacoes.travar();
                    reg.de(sessao.ligacao)
                        .ok_or_else(sem_transacao)?
                        .nasceu_aqui(&tabela, rowid)
                };
                let velha = if nasceu_aqui { None } else { t.ler(rowid)? };
                if velha.is_none() && !nasceu_aqui {
                    return Err(PhxError::NaoEncontrado(format!(
                        "rowid {rowid} nao existe em {database}.{tabela}"
                    )));
                }
                // A linha como a TRANSACAO a ve (pedido 492). A `velha` do
                // disco decide se a linha existe e e a `linha_antiga` da marca;
                // a marca herdada e o plano da cascata partem desta. Lida do
                // disco, `[excluir suave M, atualizar M]` ressuscitava M so
                // dentro de transacao -- e a nascida aqui, sem `velha`, nao
                // herdava marca nenhuma.
                let vista = self
                    .linha_na_transacao(&mut t, &database, &tabela, rowid, sessao)?
                    .or_else(|| velha.clone());
                let mut linha = json_para_linha(&valores_json, t.esquema())?;
                // A MESMA guarda do `op_atualizar`: quem nao mandou a coluna
                // de sistema nao esta ressuscitando linha excluida.
                if let Some(atual) = vista.as_deref() {
                    if crate::valores::herda_a_marca(&valores_json, t.esquema()) {
                        crate::valores::herdar_a_marca(&mut linha, atual, t.esquema());
                    }
                }
                vista_da_alteracao = vista;
                // O OLD do BEFORE UPDATE e a linha como a TRANSACAO a ve, e
                // nao a do disco (pedido 538): um gatilho de delta de estoque
                // na transacao 5->3->1 dava -4, onde o PostgreSQL 16 e o
                // MySQL 8.0 dao -2 -- o OLD da segunda instrucao e 3, que so
                // a lista sabe. A nascida aqui, que nao tem `velha`, passa a
                // ter OLD tambem: a propria insercao pendente.
                if !antes.is_empty() {
                    self.rodar_gatilhos_antes(
                        &antes,
                        Some(&mut linha),
                        vista_da_alteracao.as_deref(),
                        t.esquema(),
                    )?;
                }
                // GAP 242: calculada e CHECK do esquema julgadas na instrucao
                // (o `atualizar` nao aplica padrao -- por isso `insercao=false`),
                // ao lado do gatilho BEFORE. CHECK violado recusa AQUI, nao no
                // COMMIT.
                t.julgar_regras_de_escrita(&linha, false)?;
                crate::transacao::Escrita {
                    database: database.clone(),
                    tabela: tabela.clone(),
                    acao,
                    rowid,
                    linha,
                    // De graca: a `velha` ja foi lida do disco aqui em cima
                    // para a guarda do softdeleted. E ela vem do DISCO porque
                    // este caminho chama `ver_so_o_disco` -- que e o valor de
                    // antes de que a reaplicacao precisa.
                    linha_antiga: velha.unwrap_or_default(),
                    motivo: String::new(),
                    // O portao adiante decide: se esta alteracao cascatear, a
                    // mae vira `true` no `empilhar_atualizar_com_cascata`.
                    cascata_na_lista: false,
                    elo_do_empilhar: false,
                    elo_da_cascata: false,
                }
            }
            _ => {
                let rowid = self.rowid(p)?;
                let motivo = p.texto_ou("motivo", "").trim().to_string();
                let nasceu_aqui = {
                    let reg = self.transacoes.travar();
                    reg.de(sessao.ligacao)
                        .ok_or_else(sem_transacao)?
                        .nasceu_aqui(&tabela, rowid)
                };
                let velha = if nasceu_aqui { None } else { t.ler(rowid)? };
                if !nasceu_aqui && velha.is_none() && acao != Acao::Restaurar {
                    return Err(PhxError::NaoEncontrado(format!(
                        "rowid {rowid} nao existe em {database}.{tabela}"
                    )));
                }
                // O irmao do BEFORE UPDATE (pedido 538): o OLD do BEFORE
                // DELETE e a linha que a transacao ve -- a alterada antes na
                // lista, e a nascida aqui, que pelo disco nem disparava o
                // gatilho. A excluida de vez pela propria lista nao tem linha,
                // e nao dispara: e o que o DELETE de zero linhas faz nos
                // outros motores. So com gatilho: sem ele a volta na lista e
                // trabalho para jogar fora.
                if !antes.is_empty() {
                    let vista =
                        self.linha_na_transacao(&mut t, &database, &tabela, rowid, sessao)?;
                    if let Some(l) = vista.as_deref() {
                        self.rodar_gatilhos_antes(&antes, None, Some(l), t.esquema())?;
                    }
                }
                // Exclusao suave numa tabela sem a coluna de sistema so tem o
                // caminho fisico -- a mesma regra do `op_excluir`.
                let acao =
                    if acao == Acao::ExcluirSuave && t.esquema().coluna_softdeleted().is_none() {
                        Acao::ExcluirDeVez
                    } else {
                        acao
                    };
                crate::transacao::Escrita {
                    database: database.clone(),
                    tabela: tabela.clone(),
                    acao,
                    rowid,
                    linha: Vec::new(),
                    // Nao ha cascata no excluir -- `ao_excluir` so aceita
                    // `restringir` --, entao nao ha o que replanejar.
                    linha_antiga: Vec::new(),
                    motivo,
                    cascata_na_lista: false,
                    elo_do_empilhar: false,
                    elo_da_cascata: false,
                }
            }
        };
        // ACID-C: so o `atualizar` que mexe em chave conferida cascateia. O
        // `planejar_cascata_para_lista` sai de graca (`is_empty()`) no resto --
        // insercao, exclusao, ou alteracao que nao toca chave --, e por isso o
        // portao vem ANTES de qualquer trabalho de cascata. A mae ainda esta
        // aberta e a trava de dados na mao, entao o plano le o disco estavel --
        // e, por cima dele, o que a transacao ja pediu (pedido 515).
        let plano_cascata = if escrita.acao == crate::transacao::Acao::Atualizar
            && !escrita.linha_antiga.is_empty()
        {
            let antes = vista_da_alteracao
                .as_deref()
                .unwrap_or(&escrita.linha_antiga);
            self.planejar_cascata_empilhada(
                &trava,
                &mut t,
                &database,
                &tabela,
                antes,
                &escrita.linha,
                sessao,
            )?
        } else {
            Vec::new()
        };
        drop(t);
        drop(trava);
        if plano_cascata.is_empty() {
            return self.empilhar_escrita(
                sessao,
                escrita,
                &database,
                &tabela,
                chave,
                &chaves_novas,
                None,
            );
        }
        self.empilhar_atualizar_com_cascata(
            sessao,
            escrita,
            &database,
            &tabela,
            chave,
            &chaves_novas,
            plano_cascata,
            None,
        )
    }

    /// Poe uma escrita ja decidida na lista da transacao, e responde.
    ///
    /// Saiu do fim do `empilhar` quando o upsert passou a ter uma SEGUNDA
    /// decisao possivel -- «isto virou um atualizar» --, e nao duas listas:
    /// duplicar o teto do conjunto de escrita, o registro do database e o da
    /// tabela daria duas versoes da mesma contabilidade, e a que alguem
    /// esquecesse de atualizar seria a que deixa passar do teto.
    ///
    /// A trava de dados JA SAIU quando esta funcao e chamada -- os dois
    /// chamadores a soltam antes, porque a ordem entre ela e a do registro de
    /// transacoes e o que a `COM_A_TRAVA` cobra.
    #[allow(clippy::too_many_arguments)]
    fn empilhar_escrita(
        &self,
        sessao: &Sessao,
        escrita: crate::transacao::Escrita,
        database: &str,
        tabela: &str,
        chave: String,
        chaves_novas: &[(String, String)],
        marca: Option<&'static str>,
    ) -> Result<Json> {
        let teto = self.config.recursos.transacao_max_linhas as usize;
        let mut t = self.transacoes.travar();
        let tx = t.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
        if teto > 0 && tx.escritas.len() >= teto {
            // Erro de TRANSACAO, e nao de instrucao: o conjunto de escrita
            // esta no teto e nao ha instrucao a corrigir. **Nunca engolido, e
            // nunca vazado para disco pelas costas.**
            tx.estado = crate::transacao::Estado::AbortOnly;
            tx.motivo_do_aborto = format!(
                "o conjunto de escrita chegou ao teto de {teto} linhas \
                 (recursos.transacao_max_linhas)"
            );
            return Err(PhxError::TransacaoAbortada(tx.motivo_do_aborto.clone()));
        }
        if tx.database.is_empty() {
            tx.database = database.to_string();
        }
        if !tx.tabelas.contains(&chave) {
            tx.tabelas.push(chave);
        }
        let rowid = escrita.rowid;
        let acao_nome = escrita.acao.nome();
        tx.escritas.push(escrita);
        for (indice, chave) in chaves_novas {
            tx.guardar_chave(tabela, indice, chave);
        }
        let mut resposta = vec![
            ("empilhada", Json::Bool(true)),
            ("acao", Json::texto_de(acao_nome)),
            ("rowid", Json::de_u64(rowid)),
            ("linhas", Json::de_u64(tx.escritas.len() as u64)),
            ("transaction_id", Json::de_u64(tx.id)),
            ("transaction_state", Json::texto_de(tx.estado.nome())),
        ];
        // A marca so aparece quando ha o que dizer -- a mesma decisao do
        // `op_inserir`: um campo novo em toda resposta mudaria a forma dela
        // para todo cliente que ja existe.
        if let Some(m) = marca {
            resposta.push((m, Json::Bool(true)));
        }
        Ok(Json::objeto(resposta))
    }

    /// ACID-C: poe a mae e a cascata INTEIRA do `ao_alterar` no conjunto de
    /// escrita, no molde do super-journal do SQLite. Ver `docs/ACID.md` §2.4.
    ///
    /// # As tres fases, e por que sao tres
    ///
    /// 1. **Descobrir** (ja feito pelo chamador, com a mae aberta e a trava de
    ///    dados na mao): `planejar_cascata_para_lista` diz QUAIS filhas a
    ///    cascata toca. E o retrato do disco, estavel porque a trava esta na mao.
    /// 2. **Travar** cada tabela filha, aqui, FORA da trava de dados -- a espera
    ///    por trava nunca segura o servidor inteiro (licao do comboio), e o
    ///    escopo se expande DINAMICAMENTE como ja acontece com o gatilho. Em
    ///    `STRICT` uma filha nao declarada e recusada nomeando a tabela: guarda
    ///    nova entra pedida.
    /// 3. **Replanejar** sob a trava de dados, agora com as filhas travadas.
    ///    Sem esta fase, uma escrita de outra conexao na fresta entre 1 e 2
    ///    deixaria a lista com um retrato velho da filha.
    ///
    /// # A trava e da LINHA de cada filha, e nao so da tabela (pedido 537)
    ///
    /// A fase 2 travava a tabela filha e o FIM dela -- o que impede filha NOVA
    /// de nascer, e nada mais. A linha da filha continuava livre: T2 gravava
    /// `x=1` nela, solta ou em transacao, e o COMMIT de T1 regravava a linha
    /// inteira que o plano tinha visto, `x=0` -- update perdido, medido pelo
    /// papel C na base e depois do lote do 515. O comentario daqui dizia que
    /// o retrato «nao muda mais ate o commit», e era isso que ninguem
    /// conferia. Agora cada linha do plano e travada pelo MESMO caminho de
    /// toda escrita da transacao (`travar_para_empilhar`, que espera o `LOCK
    /// TIMEOUT` fora da trava de dados), como o COMMIT faz com o elo que so
    /// ele descobre (516). A linha que so aparece no plano da fase 3 -- uma
    /// filha que passou a apontar para a chave na fresta -- e travada numa
    /// volta seguinte, e o plano se refaz; volta sem fim recusa a instrucao.
    ///
    /// E a trava nao e a unica rede: o elo leva `elo_do_empilhar`, e o COMMIT
    /// o refaz sobre a linha ATUAL antes da marca, levando so a chave
    /// (`refazer_o_elo`). A trava faz o outro esperar; o refazer garante que
    /// o que escapar da trava -- a cascata SOLTA de outra mae da mesma filha
    /// nao pergunta por trava de linha nenhuma -- nao volta a ser apagado.
    ///
    /// A mae ja esta reservada desde o topo do `empilhar`, entao a
    /// `linha_antiga` dela continua valendo entre as fases.
    #[allow(clippy::too_many_arguments)]
    fn empilhar_atualizar_com_cascata(
        &self,
        sessao: &Sessao,
        mut mae: crate::transacao::Escrita,
        database: &str,
        tabela: &str,
        chave: String,
        chaves_novas: &[(String, String)],
        plano_fase1: Vec<phxsql_store::table::EscritaDaCascata>,
        marca: Option<&'static str>,
    ) -> Result<Json> {
        // Quantas vezes o plano pode achar filha que a volta anterior nao
        // travou. Uma e o caso comum (nada mudou na fresta); cada volta a
        // mais exige OUTRA conexao repontando filha para a chave velha no
        // meio de milissegundos.
        const VOLTAS: usize = 4;
        // A mae reserva a propria tabela como toda escrita empilhada faz.
        let _ = &chave;
        let mut tabelas_travadas: Vec<String> = Vec::new();
        let mut linhas_travadas: std::collections::HashSet<(String, u64)> =
            std::collections::HashSet::new();
        let mut a_travar: Vec<(String, u64)> = plano_fase1
            .iter()
            .map(|e| (e.tabela.clone(), e.rowid))
            .collect();
        let mut volta = 0;
        let plano = loop {
            // FASE 2 -- travar cada tabela filha distinta (o fim dela) e cada
            // linha do plano, fora da trava de dados.
            a_travar.sort();
            a_travar.dedup();
            for (filha, rowid) in &a_travar {
                let chave_filha = crate::carga::chave(database, filha);
                if !tabelas_travadas.contains(&chave_filha) {
                    self.travar_para_empilhar(sessao, &chave_filha, crate::travas::FIM_DA_TABELA)?;
                    tabelas_travadas.push(chave_filha.clone());
                }
                self.travar_para_empilhar(sessao, &chave_filha, *rowid)?;
                linhas_travadas.insert((chave_filha, *rowid));
            }
            // FASE 3 -- replanejar com as filhas travadas.
            let plano = {
                let trava = self.travar_dados()?;
                let ped = pedido_da_tabela(database, tabela);
                // O IRMAO do `empilhar`: mesma dispensa, mesma porta. Ele chama
                // as mesmas funcoes na mesma ordem, e um conserto que entrasse
                // so la deixaria a fase 3 da cascata pagando o mapa que
                // ninguem le.
                let mut t = self.abrir_travada_sem_sobrepor(&trava, &ped, sessao)?;
                // O MESMO plano da fase 1, contra o que a transacao ve (pedido
                // 515): a mae como a lista a deixou, e cada filha com o que a
                // lista ja pediu nela. Refeito aqui porque o disco pode ter
                // mudado na fresta -- a lista desta sessao, nao.
                let antes = self
                    .linha_na_transacao(&mut t, database, tabela, mae.rowid, sessao)?
                    .unwrap_or_else(|| mae.linha_antiga.clone());
                let plano = self.planejar_cascata_empilhada(
                    &trava, &mut t, database, tabela, &antes, &mae.linha, sessao,
                )?;
                drop(t);
                drop(trava);
                plano
            };
            a_travar = plano
                .iter()
                .filter(|e| {
                    !linhas_travadas.contains(&(crate::carga::chave(database, &e.tabela), e.rowid))
                })
                .map(|e| (e.tabela.clone(), e.rowid))
                .collect();
            if a_travar.is_empty() {
                break plano;
            }
            volta += 1;
            if volta >= VOLTAS {
                let (filha, rowid) = &a_travar[0];
                return Err(PhxError::EmTransacao(format!(
                    "a cascata desta alteracao achou filha nova a cada volta ({VOLTAS} \
                     voltas; a ultima foi {filha} rowid {rowid}): outra conexao esta \
                     repontando filhas para esta chave agora. NADA desta instrucao foi \
                     empilhado; mande de novo"
                )));
            }
        };
        // A mae aplica ACHATADA -- os elos ja sao escritas da lista.
        mae.cascata_na_lista = true;
        let mut grupo = Vec::with_capacity(1 + plano.len());
        grupo.push(mae);
        for elo in plano {
            grupo.push(crate::transacao::Escrita {
                database: database.to_string(),
                tabela: elo.tabela,
                acao: crate::transacao::Acao::Atualizar,
                rowid: elo.rowid,
                linha: elo.linha,
                linha_antiga: elo.linha_antiga,
                motivo: String::new(),
                cascata_na_lista: true,
                // O retrato da filha e o de AGORA; o COMMIT o refaz sobre a
                // linha de entao, levando so a chave (pedido 537).
                elo_do_empilhar: true,
                elo_da_cascata: true,
            });
        }
        self.empilhar_grupo(sessao, grupo, database, tabela, chaves_novas, marca)
    }

    /// Empilha um GRUPO de escritas de uma vez -- a mae de uma cascata e cada
    /// elo dela. Ou entra inteiro, ou nao entra: meia cascata na lista seria o
    /// mesmo estrago que o ACID-C existe para fechar. O teto conta o TOTAL, e
    /// cada tabela tocada entra na reserva.
    ///
    /// A trava de dados JA SAIU quando esta funcao e chamada -- a ordem entre
    /// ela e a do registro de transacoes e o que a `COM_A_TRAVA` cobra, a mesma
    /// do `empilhar_escrita`.
    fn empilhar_grupo(
        &self,
        sessao: &Sessao,
        grupo: Vec<crate::transacao::Escrita>,
        database: &str,
        tabela: &str,
        chaves_novas: &[(String, String)],
        marca: Option<&'static str>,
    ) -> Result<Json> {
        let teto = self.config.recursos.transacao_max_linhas as usize;
        let mut t = self.transacoes.travar();
        let tx = t.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
        if teto > 0 && tx.escritas.len() + grupo.len() > teto {
            tx.estado = crate::transacao::Estado::AbortOnly;
            tx.motivo_do_aborto = format!(
                "o conjunto de escrita chegou ao teto de {teto} linhas \
                 (recursos.transacao_max_linhas); a cascata do ao_alterar conta \
                 cada filha"
            );
            return Err(PhxError::TransacaoAbortada(tx.motivo_do_aborto.clone()));
        }
        if tx.database.is_empty() {
            tx.database = database.to_string();
        }
        let mae_rowid = grupo.first().map_or(0, |e| e.rowid);
        let mae_acao = grupo.first().map_or("atualizar", |e| e.acao.nome());
        let elos = grupo.len().saturating_sub(1);
        for e in &grupo {
            let ch = crate::carga::chave(database, &e.tabela);
            if !tx.tabelas.contains(&ch) {
                tx.tabelas.push(ch);
            }
        }
        for e in grupo {
            tx.escritas.push(e);
        }
        for (indice, chave) in chaves_novas {
            tx.guardar_chave(tabela, indice, chave);
        }
        let mut resposta = vec![
            ("empilhada", Json::Bool(true)),
            ("acao", Json::texto_de(mae_acao)),
            ("rowid", Json::de_u64(mae_rowid)),
            ("linhas", Json::de_u64(tx.escritas.len() as u64)),
            // Quantos elos da cascata entraram junto -- read-your-own-writes ja
            // os mostra, e o COMMIT ja os conta.
            ("cascata", Json::de_u64(elos as u64)),
            ("transaction_id", Json::de_u64(tx.id)),
            ("transaction_state", Json::texto_de(tx.estado.nome())),
        ];
        if let Some(m) = marca {
            resposta.push((m, Json::Bool(true)));
        }
        Ok(Json::objeto(resposta))
    }

    /// As travas que UMA escrita empilhada precisa, na ordem certa.
    ///
    /// # O escopo declarado manda aqui
    ///
    /// Tabela fora do escopo: em `STRICT` a escrita e recusada nomeando a
    /// tabela e o escopo; em `DYNAMIC` -- o padrao -- ela ENTRA, a trava e
    /// tomada na hora e a expansao fica anotada, para a ficha de diagnostico
    /// mostrar o que entrou sem ninguem pedir.
    ///
    /// **A expansao dinamica reintroduz a possibilidade de ciclo entre
    /// tabelas**, e nao adianta fingir que nao: a ordem canonica so vale para
    /// o que foi declarado na abertura. Quem quer a garantia declara o escopo
    /// inteiro; quem nao declara fica com o `LOCK TIMEOUT` de rede embaixo. E
    /// esse e exatamente o preco do `DYNAMIC`, dito antes de alguem descobrir.
    fn travar_para_empilhar(&self, sessao: &Sessao, chave: &str, rowid: u64) -> Result<()> {
        // Esperando ate o prazo, a barrada nunca volta: vira o erro do LOCK
        // TIMEOUT la dentro.
        self.travar_para_escrever(sessao, chave, rowid, Espera::AteOPrazo)
            .map(|_| ())
    }

    /// O corpo do [`Servidor::travar_para_empilhar`], com a espera escolhida.
    ///
    /// `Espera::Nenhuma` e a do COMMIT (pedido 516): o elo da cascata que so
    /// a pre-conferencia descobre -- a filha que nasceu na chave velha depois
    /// do `empilhar` -- escreve numa linha que a transacao nunca travou, e
    /// passava por cima da leitura repetivel de outra transacao. Ele toma a
    /// trava pelo mesmo caminho de toda escrita da transacao (escopo, tabela,
    /// linha); o que muda e so que o COMMIT esta com a trava de dados na mao
    /// e nao pode esperar ali -- a doenca dos 29.456 ms.
    pub(super) fn travar_para_escrever(
        &self,
        sessao: &Sessao,
        chave: &str,
        rowid: u64,
        espera: Espera,
    ) -> Result<Option<crate::travas::Barrada>> {
        let (id, modo, escopo_modo, no_escopo, declarou) = {
            let t = self.transacoes.travar();
            let tx = t.de(sessao.ligacao).ok_or_else(sem_transacao)?;
            (
                tx.id,
                tx.modo,
                tx.escopo_modo,
                tx.efetivas.iter().any(|c| c == chave),
                !tx.declaradas.is_empty(),
            )
        };
        if declarou && !no_escopo {
            if escopo_modo == crate::travas::EscopoModo::Estrito {
                return Err(PhxError::Esquema(format!(
                    "{chave} nao esta no SCOPE desta transacao e o SCOPE MODE e \
                     STRICT. Declare-a na abertura, ou abra com SCOPE MODE \
                     DYNAMIC"
                )));
            }
            let mut t = self.transacoes.travar();
            if let Some(tx) = t.de_mut(sessao.ligacao) {
                tx.efetivas.push(chave.to_string());
                crate::travas::em_ordem_canonica(&mut tx.efetivas);
                tx.expandidas.push(chave.to_string());
            }
        }
        // Tabela primeiro, linha depois -- e sempre nessa ordem, que e o que a
        // hierarquia de intencao existe para dar: quem quer a tabela inteira ve
        // a intencao de quem esta nas linhas sem varrer linha por linha.
        if let Some(b) =
            self.esperar_trava(sessao, id, chave, Alvo::Tabela(modo.na_tabela()), espera)?
        {
            return Ok(Some(b));
        }
        if modo.trava_linha() {
            // `rowid` ja vem sendo o FIM DA TABELA quando a acao e anexar: o
            // proximo slot e `slots() + 1` e ele e um so, entao duas
            // transacoes que anexam disputam o MESMO lugar -- e disputam de
            // verdade, porque o rowid e o endereco.
            return self.esperar_trava(sessao, id, chave, Alvo::Linha(rowid), espera);
        }
        Ok(None)
    }

    /// Toma uma trava, esperando no maximo o `LOCK TIMEOUT` desta transacao.
    ///
    /// # A espera acontece FORA da trava de dados, e isso e a peca
    ///
    /// Esperar com a trava global na mao seria a doenca que este projeto ja
    /// mediu em 29.456 ms: uma transacao esperando outra pararia o servidor
    /// inteiro, e nao por engano de implementacao -- por desenho. Aqui a
    /// espera solta tudo entre uma tentativa e a proxima.
    ///
    /// # Por que enquete, e nao variavel de condicao
    ///
    /// Porque a espera e limitada e curta por definicao (o padrao sao 500 ms),
    /// e uma variavel de condicao precisaria acordar por tabela e por linha
    /// para nao acordar todo mundo a cada solta. O custo desta e um `lock` sem
    /// disputa a cada 2 ms -- **13,2 ns** medidos em `DESEMPENHO.md` --, e
    /// quem espera ja esta parado de qualquer forma. Trocar isto por condvar e
    /// otimizar o caminho de quem ja perdeu.
    ///
    /// # `Espera::Nenhuma`: uma tentativa, e a recusa diz por que
    ///
    /// E a do COMMIT, que esta com a trava de dados na mao (pedido 516). A
    /// tentativa e a MESMA do laco -- `pegar_tabela`/`pegar_linha`, com o
    /// mesmo recado de quem barrou --; o que ela nao faz e dormir, nem anotar
    /// espera na ficha, nem estourar o prazo da transacao, que a lista do
    /// COMMIT ja nao esta mais nela para jogar fora.
    fn esperar_trava(
        &self,
        sessao: &Sessao,
        id: u64,
        chave: &str,
        alvo: Alvo,
        espera: Espera,
    ) -> Result<Option<crate::travas::Barrada>> {
        let (prazo_ms, expira_ms) = {
            let t = self.transacoes.travar();
            let tx = t.de(sessao.ligacao).ok_or_else(sem_transacao)?;
            (tx.lock_timeout_ms, tx.expira_ms)
        };
        let comeco = Instant::now();
        let mut ultima: Option<crate::travas::Barrada>;
        loop {
            {
                let mut tr = self.travas.travar();
                let r = match alvo {
                    Alvo::Tabela(t) => tr.pegar_tabela(chave, id, t),
                    Alvo::Linha(rowid) => tr.pegar_linha(chave, id, rowid),
                };
                match r {
                    Ok(()) if espera == Espera::Nenhuma => return Ok(None),
                    Ok(()) => {
                        drop(tr);
                        self.anotar_espera(sessao, "");
                        return Ok(None);
                    }
                    Err(b) => ultima = Some(b),
                }
            }
            if espera == Espera::Nenhuma {
                // Quem barrou volta INTEIRO, e nao numa frase: o COMMIT precisa
                // do id dele para o desempate do ciclo (C1 do 516).
                return Ok(ultima);
            }
            // O prazo da TRANSACAO tambem corta a espera: nao adianta esperar
            // 500 ms por uma trava se a transacao morre em 50.
            let estourou_o_lock = prazo_ms <= 0 || comeco.elapsed().as_millis() as i64 >= prazo_ms;
            let estourou_a_transacao = crate::agora_ms() >= expira_ms;
            if estourou_o_lock || estourou_a_transacao {
                self.anotar_espera(sessao, "");
                let b = ultima.expect("so se sai do laco com uma barrada na mao");
                let recado = {
                    let t = self.transacoes.travar();
                    t.recado_da_barrada(&b, crate::agora_ms())
                };
                if estourou_a_transacao {
                    return Err(self.estourar_prazo(sessao));
                }
                return Err(PhxError::EmTransacao(format!(
                    "{recado}. Esperei o LOCK TIMEOUT de {prazo_ms} ms e desisti"
                )));
            }
            if let Some(b) = &ultima {
                let onde = match b.rowid {
                    Some(crate::travas::FIM_DA_TABELA) => format!("o fim de {}", b.tabela),
                    Some(r) => format!("{} rowid {r}", b.tabela),
                    None => b.tabela.clone(),
                };
                self.anotar_espera(sessao, &format!("{onde} (transacao {})", b.transacao));
            }
            // Dois milissegundos: curto o bastante para nao somar ao tempo de
            // quem consegue a trava logo, e longo o bastante para nao virar
            // espera ativa.
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Anota (ou limpa) o que esta transacao espera, para a ficha.
    fn anotar_espera(&self, sessao: &Sessao, o_que: &str) {
        if let Some(tx) = self.transacoes.travar().de_mut(sessao.ligacao) {
            tx.esperando = o_que.to_string();
        }
    }

    /// O prazo da transacao estourou: **quem encerra e o gestor, nunca uma
    /// thread morta.**
    ///
    /// Matar a thread deixaria estado interno pela metade -- travas presas,
    /// conjunto de escrita orfao, o contador de transacoes abertas errado. Em
    /// vez disso a transacao vai para `ABORT_ONLY`, solta as travas, joga a
    /// lista fora e passa a recusar operacao nova com um erro que traz o
    /// numero do prazo. A conexao continua viva e o cliente descobre pelo
    /// erro, que e o que ele sabe tratar.
    fn estourar_prazo(&self, sessao: &Sessao) -> PhxError {
        let r = self.abortar_soltando(sessao.ligacao, |tx| {
            format!(
                "o TIMEOUT da transacao ({} ms) estourou",
                tx.expira_ms - tx.desde_ms
            )
        });
        match r {
            Ok((id, prazo)) => PhxError::TransacaoAbortada(format!(
                "a transacao {id} passou do TIMEOUT de {prazo} ms e foi revertida; \
                 as travas dela ja sairam. Mande ROLLBACK para fechar"
            )),
            Err(e) => e,
        }
    }

    /// O gestor encerra a transacao de `ligacao`: `ABORT_ONLY`, lista fora,
    /// travas soltas JA. Devolve `(id, prazo em ms)`.
    ///
    /// Uma porta para as duas causas -- o prazo estourado e o ciclo de
    /// `COMMIT`s que cede pela mais nova (C1 do 516) --, porque as duas pedem
    /// o mesmo: segurar tabela alheia depois de a transacao nao poder mais
    /// confirmar e exatamente o que cada uma existe para impedir. O `motivo`
    /// so entra se a transacao ainda nao tinha um.
    pub(super) fn abortar_soltando(
        &self,
        ligacao: u64,
        motivo: impl FnOnce(&crate::transacao::Transacao) -> String,
    ) -> Result<(u64, i64)> {
        let (id, prazo) = {
            let mut t = self.transacoes.travar();
            let tx = t.de_mut(ligacao).ok_or_else(sem_transacao)?;
            tx.estado = crate::transacao::Estado::AbortOnly;
            tx.escritas.clear();
            tx.pontos.clear();
            tx.esperando.clear();
            tx.commit_barrado_por = None;
            if tx.motivo_do_aborto.is_empty() {
                tx.motivo_do_aborto = motivo(tx);
            }
            (tx.id, tx.expira_ms - tx.desde_ms)
        };
        self.travas.travar().soltar_tudo(id);
        Ok((id, prazo))
    }

    /// Poe o `STATEMENT TIMEOUT` desta transacao no relogio da operacao.
    ///
    /// # Onde ele morde, e onde nao morde
    ///
    /// Ele e conferido nos PONTOS DE CANCELAMENTO que ja existem -- o
    /// `Atividade::siga`, chamado entre duas unidades de trabalho seguras
    /// pelos lacos longos (a conversao de uma carga, a exportacao). Uma
    /// insercao de UMA linha nao tem ponto de cancelamento no meio, e nao
    /// poderia ter: parar entre gravar o slot e manter o indice deixaria os
    /// dois discordando. **Isso esta dito no documento em vez de escondido**,
    /// e e a diferenca entre um prazo que faz o que promete e um campo de
    /// configuracao que mente.
    fn por_prazo_na_operacao(&self, sessao: &Sessao) {
        let prazo = self
            .transacoes
            .travar()
            .de(sessao.ligacao)
            .map(|tx| tx.statement_timeout_ms)
            .unwrap_or(0);
        if prazo <= 0 {
            return;
        }
        if let Some(a) = crate::telemetria::corrente() {
            a.definir_prazo(crate::agora_ms() + prazo);
        }
    }

    /// Acrescenta a uma ficha de transacao o que o gestor de travas SABE.
    ///
    /// As duas metades vem de registros diferentes de proposito: a transacao
    /// sabe o que declarou, o gestor sabe o que ela conseguiu. Juntar as duas
    /// fontes numa so faria uma passar a acreditar na outra, e a ficha
    /// deixaria de ser medida -- que e a regra do relatorio de recuperacao
    /// aplicada aqui.
    fn juntar_travas(&self, ficha: Json, _agora: i64) -> Json {
        let Json::Objeto(mut pares) = ficha else {
            return ficha;
        };
        let id = pares
            .iter()
            .find(|(k, _)| k == "transaction_id")
            .and_then(|(_, v)| v.inteiro())
            .unwrap_or(0)
            .max(0) as u64;
        let travadas = self.travas.travar().ficha(id);
        let mut linhas_travadas = 0u64;
        let lista: Vec<Json> = travadas
            .iter()
            .map(|(tabela, trava, linhas)| {
                linhas_travadas += *linhas;
                Json::objeto(vec![
                    ("tabela", Json::texto_de(tabela)),
                    ("trava", Json::texto_de(*trava)),
                    ("linhas", Json::de_u64(*linhas)),
                ])
            })
            .collect();
        pares.push(("travas".to_string(), Json::Lista(lista)));
        pares.push(("linhas_travadas".to_string(), Json::de_u64(linhas_travadas)));
        Json::Objeto(pares)
    }

    /// Uma escrita COMUM -- sem `BEGIN` nenhum -- esbarra numa trava de
    /// transacao?
    ///
    /// **Ela nao espera**, e isso e deliberado: um pedido solto nao declarou
    /// `LOCK TIMEOUT` nenhum, e inventar uma espera para ele mudaria o tempo
    /// de resposta de todo cliente que ja existe. Recebe `EM_TRANSACAO` com
    /// `repetir: true`, que e o que separa «espere» de «voce nao pode».
    pub(super) fn barrado_por_travas(
        &self,
        op: &str,
        pedido: &Json,
        sessao: &Sessao,
    ) -> Option<String> {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 {
            return None;
        }
        let db = pedido.texto_ou("database", "");
        if db.is_empty() {
            return None;
        }
        // **Quem TEM transacao aberta nao passa por aqui.** Este portao e o da
        // escrita comum, e a diferenca entre os dois e a espera: uma transacao
        // declarou `LOCK TIMEOUT` e tem direito de esperar por ele, no
        // `esperar_trava`; um pedido solto nao declarou nada e recusa na hora.
        // Deixar os dois passarem por aqui tiraria da transacao a espera que
        // ela pediu -- e o erro dela sairia sem sequer dizer LOCK TIMEOUT.
        if self.transacoes.travar().de(sessao.ligacao).is_some() {
            return None;
        }
        let meu = 0u64;
        let travas = self.travas.travar();
        // O campo que este portao le e o furo: `tabela` cobre quase tudo, e
        // `destino` cobre as duas que gravam noutra tabela sem dize-lo ali
        // (`duplicar_tabela` e `copiar_tabela`).
        for campo in ["tabela", "destino"] {
            let tab = pedido.texto_ou(campo, "");
            if tab.is_empty() {
                continue;
            }
            let chave = crate::carga::chave(db, tab);
            // O que esta escrita pretende, e nao "a tabela inteira" para todo
            // mundo: um `atualizar` no rowid 7 nao tem nada a ver com a linha 9
            // que uma transacao segura.
            let barrada = match op {
                "atualizar" | "excluir" | "restaurar" => {
                    let rowid = pedido.campo("rowid").and_then(Json::inteiro).unwrap_or(0);
                    travas.conflito_de_linha(&chave, meu, rowid.max(0) as u64)
                }
                // Anexar disputa o FIM da tabela: o proximo slot e um so.
                "inserir" | "inserir_lote" | "importar" | "carga" | "duplicar_tabela"
                | "copiar_tabela" => {
                    travas.conflito_de_linha(&chave, meu, crate::travas::FIM_DA_TABELA)
                }
                // O resto mexe na tabela toda -- estrutura, indice, lixeira.
                _ => travas.conflito_de_tabela(&chave, meu, crate::travas::Trava::Exclusiva),
            };
            if let Some(b) = barrada {
                let agora = crate::agora_ms();
                let t = self.transacoes.travar();
                return Some(t.recado_da_barrada(&b, agora));
            }
        }
        None
    }

    /// Uma REESCRITA (`migrar_esquema`, `acrescentar_coluna`) vai congelar `t`:
    /// alguma transacao viva alcanca esta tabela? Devolve o recado de quem
    /// segura, ou `None` para a reescrita seguir.
    ///
    /// # Por que aqui dentro, com a trava global na mao, e nao no portao
    ///
    /// Pedido 426. O portao das travas de transacao (`barrado_por_travas`)
    /// roda FORA da trava global, entre o pedido chegar e a operacao comecar.
    /// Uma transacao que empilhasse na fresta entre os dois ficava por baixo
    /// do congelamento, e o `COMMIT` dela batia nele DEPOIS da marca -- uma
    /// tabela gravada e a outra nao. Com a trava global na mao nenhuma
    /// transacao toma trava nova entre esta pergunta e o `congelar`: toda
    /// escrita empilhada toma a trava da tabela ANTES da trava global
    /// (`empilhar`), e a que vier depois do congelamento esbarra nele na
    /// propria instrucao.
    ///
    /// # Por que o COMPONENTE da chave, e nao so a tabela
    ///
    /// Porque o portao velho olhava so o campo `tabela` do pedido, e o
    /// `COMMIT` abre para gravar tambem a mae (conferencia da chave), a filha
    /// (conferencia do `excluir`) e a neta (cascata). Medido pelo soquete no
    /// HEAD: transacao escrevendo na FILHA, `migrar_esquema` na MAE -- sem
    /// corrida nenhuma, a migracao congelava a mae e o COMMIT saia pela metade.
    /// Ver [`crate::transacao::componente_de_chave`].
    ///
    /// # Quem cede e a reescrita -- e ela cede na hora
    ///
    /// Os quatro motores convergem em que quem paga e o DDL, nunca a
    /// transacao (`docs/propostas/commit-contra-ddl-4-motores.md`, D5). A
    /// reescrita recusa com `EM_TRANSACAO` e nada reescrito, em vez de esperar:
    /// e a regra desta casa para a escrita sem `BEGIN` (`barrado_por_travas`),
    /// que nao declarou `LOCK TIMEOUT` nenhum, e uma espera aqui seria dormir
    /// com a trava global na mao ou soltar e tomar de volta num laco.
    ///
    /// O primeiro `if` e um `load` atomico: sem transacao aberta em lugar
    /// nenhum -- o servidor de sempre -- nada aqui custa.
    pub(super) fn transacao_na_vizinhanca(
        &self,
        dados: &Instancia,
        database: &str,
        tabela: &str,
        t: &Table,
    ) -> Result<Option<String>> {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 {
            return Ok(None);
        }
        let (schema, _) = phxsql_store::catalogo::separar_qualificado(tabela);
        let db = dados.abrir_database(database)?;
        let todas = db.tabelas(schema.as_deref())?;
        let alcance =
            crate::transacao::componente_de_chave(t.diretorio(), &todas, &[t.nome().to_string()]);
        // A chave da trava e o nome como a ESCRITA o mandou: qualificado no
        // pedido do cliente, simples no elo da cascata. As duas formas entram.
        let mut chaves = Vec::with_capacity(alcance.len() * 2);
        for nome in &alcance {
            chaves.push((
                nome.clone(),
                crate::carga::chave(
                    database,
                    &phxsql_store::catalogo::qualificar(schema.as_deref(), nome),
                ),
            ));
            if schema.is_some() {
                chaves.push((nome.clone(), crate::carga::chave(database, nome)));
            }
        }
        let barrada = {
            let travas = self.travas.travar();
            chaves.iter().find_map(|(nome, chave)| {
                travas
                    .conflito_de_tabela(chave, 0, crate::travas::Trava::Exclusiva)
                    .map(|b| (nome.clone(), b))
            })
        };
        let Some((nome, b)) = barrada else {
            return Ok(None);
        };
        let recado = {
            let reg = self.transacoes.travar();
            reg.recado_da_barrada(&b, crate::agora_ms())
        };
        let ligacao = if nome.eq_ignore_ascii_case(t.nome()) {
            String::new()
        } else {
            format!(
                " -- {nome} e ligada a {} por chave estrangeira, e o COMMIT dela abre as duas",
                t.nome()
            )
        };
        Ok(Some(format!(
            "{} nao sera reescrita agora: {recado}{ligacao}. Quem cede e a \
             reescrita, nunca a transacao -- nada foi reescrito; repita depois \
             do COMMIT ou do ROLLBACK dela",
            t.nome()
        )))
    }

    /// Alguma tabela que o `COMMIT` destas pode abrir para gravar esta
    /// CONGELADA agora? Devolve o recado do congelamento, ou `None`.
    ///
    /// `diretorio` e o de uma tabela ja ABERTA (`Table::diretorio`), e nao o
    /// do catalogo: o congelamento guarda o caminho resolvido, e comparar com
    /// o cru erraria calado com a base relativa no `config.json`.
    ///
    /// Os dois que chamam, e por que sao dois:
    ///
    /// * o `empilhar`, na INSTRUCAO -- «repita» so se diz sobre o que aplicou
    ///   zero, e sempre na instrucao (`commit-contra-ddl-4-motores.md`, D3). E
    ///   o 1412 `ER_TABLE_DEF_CHANGED` do MySQL, no mesmo lugar;
    /// * o `op_commit`, ANTES da marca -- a rede que sobra para o congelamento
    ///   que nasceu depois da instrucao por um caminho que ninguem previu. Ali
    ///   a recusa devolve a lista a transacao, que continua ativa.
    ///
    /// O portao vem antes do trabalho: com nada congelado -- sempre, menos
    /// durante uma reescrita -- e um `load` atomico e volta.
    pub(super) fn congelada_no_alcance(
        &self,
        dados: &Instancia,
        database: &str,
        schema: Option<&str>,
        diretorio: &Path,
        nomes: &[String],
    ) -> Result<Option<String>> {
        if phxsql_store::congelamento::quantas() == 0 {
            return Ok(None);
        }
        let todas = dados.abrir_database(database)?.tabelas(schema)?;
        for nome in crate::transacao::componente_de_chave(diretorio, &todas, nomes) {
            match phxsql_store::congelamento::conferir(diretorio, &nome) {
                Ok(()) => {}
                Err(PhxError::EmMigracao(m)) => return Ok(Some(m)),
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }

    /// `rollback`: joga a lista fora. **Zero bytes de trabalho.**
    pub(super) fn op_rollback(&self, sessao: &Sessao) -> Result<Json> {
        self.exigir_transacao(sessao)?;
        let (id, descartadas) = self.descartar_transacao(sessao.ligacao);
        Ok(Json::objeto(vec![
            ("transaction_id", Json::de_u64(id)),
            (
                "transaction_state",
                Json::texto_de(crate::transacao::Estado::Revertida.nome()),
            ),
            ("descartadas", Json::de_u64(descartadas)),
        ]))
    }

    /// Tira a transacao do registro e devolve `(id, linhas descartadas)`.
    ///
    /// E o mesmo caminho do `ROLLBACK`, da queda da conexao e do prazo -- tres
    /// portas para uma saida so, porque tres implementacoes de "desfazer"
    /// seriam tres chances de esquecer o contador.
    pub(super) fn descartar_transacao(&self, ligacao: u64) -> (u64, u64) {
        let saiu = self.transacoes.travar().tirar(ligacao);
        match saiu {
            Some(tx) => {
                // As travas saem JUNTO, e no mesmo lugar: uma segunda funcao
                // para soltar travas seria uma segunda chance de esquecer, e
                // trava esquecida trava a tabela para sempre.
                self.travas.travar().soltar_tudo(tx.id);
                self.transacoes_abertas.fetch_sub(1, Ordering::SeqCst);
                (tx.id, tx.escritas.len() as u64)
            }
            None => (0, 0),
        }
    }

    /// A queda da conexao desfaz a transacao dela -- a PRIMEIRA rede.
    ///
    /// A segunda e o prazo, e uma so nao basta: soquete meio-morto existe, e e
    /// justamente o caso em que esta nao pega.
    pub(super) fn soltar_transacao_da_ligacao(&self, ligacao: u64) {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 {
            return;
        }
        self.descartar_transacao(ligacao);
    }

    /// As transacoes que passaram do prazo -- a SEGUNDA rede.
    ///
    /// Conferida na hora de usar, e nao por um relogio de fundo: uma transacao
    /// vencida que ninguem consultou nao atrapalha ninguem, e a primeira
    /// consulta a limpa. E a mesma decisao da reserva de carga.
    fn limpar_transacoes_vencidas(&self) {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 {
            return;
        }
        let vencidas = self.transacoes.travar().vencidas(crate::agora_ms());
        for l in vencidas {
            // **Ela nao SUMA: vira `ABORT_ONLY` e espera o dono.**
            //
            // Descartar aqui soltava as travas -- que e o que importa para os
            // outros -- e tirava do dono a resposta: a proxima operacao dele
            // receberia «esta conexao nao tem transacao aberta», sem o numero
            // do prazo dentro. Quem varre e OUTRA conexao, e ela nao pode
            // apagar a explicacao de quem estourou.
            //
            // O que fica e uma entrada sem travas e sem conjunto de escrita:
            // ela custa nada, e sai no `ROLLBACK`, no `COMMIT` ou na queda da
            // conexao, como qualquer outra.
            let _ = self.estourar_prazo(&Sessao {
                ligacao: l,
                ..Sessao::default()
            });
        }
    }

    /// `commit`: a lista inteira aplicada numa passada so.
    ///
    /// # A resposta ao contrato, ponto a ponto
    ///
    /// *«Se o computador perder energia exatamente nesta instrucao, depois de
    /// reiniciar o banco conseguira determinar de forma inequivoca se esta
    /// transacao foi COMMITTED ou ABORTED?»*
    ///
    /// | onde a energia cai | o que o banco sabe dizer |
    /// |---|---|
    /// | antes de a marca estar sincronizada | ABORTED. Nao ha marca e nenhum byte de dado foi tocado |
    /// | durante o `fsync` da marca | ABORTED. Ela fica truncada, o CRC nao confere, e marca que nao confere e commit que nunca comecou |
    /// | depois da marca e no meio da passada | COMMITTED. A marca esta inteira no disco e a recuperacao completa o que falta, andando para a frente |
    /// | depois da passada e antes do `unlink` | COMMITTED. A recuperacao reaplica e acha tudo ja certo -- custa uma varredura da marca, e e seguro |
    /// | depois do `unlink` | COMMITTED, e sem trabalho nenhum |
    ///
    /// O `fsync` da marca e o **ponto de compromisso**: antes dele a transacao
    /// nao aconteceu; depois dele ela aconteceu, mesmo que o arquivo de dado
    /// ainda nao saiba.
    ///
    /// # A resposta ao cliente segue o MESMO ponto -- pedido 426
    ///
    /// Antes da marca, a recusa e erro, e o `repetir` do tipo do erro diz a
    /// verdade: nada foi aplicado. Depois da marca a transacao ACONTECEU, e
    /// quem quebrar a passada no meio recebe `COMMITTED`, com o aviso do que
    /// houve -- a nao ser que a marca SAIA do disco, e os dois casos em que
    /// ela sai estao na tabela de [`Servidor::depois_da_marca`]. Um `4006`
    /// com `repetir: true` depois de meia passada mandava o cliente duplicar
    /// o que ja estava gravado; nenhuma linha daquela tabela faz isso.
    pub(super) fn op_commit(&self, sessao: &Sessao) -> Result<Json> {
        self.exigir_transacao(sessao)?;
        let inicio = Instant::now();
        // ORDEM UNICA DAS TRAVAS: dados primeiro, transacoes depois. Sempre.
        // Duas ordens diferentes em dois pontos e o abraco mortal classico, e
        // este servidor ja pagou tres vezes por uma trava tomada duas vezes.
        let mut trava = self.travar_dados()?;
        let tirada = {
            let mut reg = self.transacoes.travar();
            let tx = reg.de_mut(sessao.ligacao).ok_or_else(sem_transacao)?;
            match tx.estado {
                crate::transacao::Estado::AbortOnly => {
                    return Err(PhxError::TransacaoAbortada(format!(
                        "{}; a transacao nao pode ser confirmada -- mande ROLLBACK",
                        if tx.motivo_do_aborto.is_empty() {
                            "houve erro de TRANSACAO".to_string()
                        } else {
                            tx.motivo_do_aborto.clone()
                        }
                    )))
                }
                crate::transacao::Estado::Ativa => {}
                outro => {
                    return Err(PhxError::Esquema(format!(
                        "a transacao esta em {} e nao aceita COMMIT",
                        outro.nome()
                    )))
                }
            }
            // PEDIDO 539: o PRAZO DA TRANSACAO vale para o COMMIT. O portao
            // deixa passar as operacoes de controle -- o ROLLBACK tem de sair
            // sempre --, e a varredura so roda no `begin` e no `transacoes`;
            // entao o COMMIT tardio gravava ou nao conforme uma TERCEIRA
            // conexao tivesse aberto transacao no meio (medido pelo papel C:
            // COMMITTED 600 ms depois de um prazo de 200 ms). Conferido aqui,
            // com a trava de dados na mao e antes de a lista sair do registro,
            // porque a espera pela trava pode ser longa (um backup) e o que
            // conta e o instante que decide a marca. Vale tambem para a lista
            // vazia: a resposta nao pode depender de quem varreu.
            if tx.expira_ms <= crate::agora_ms() {
                None
            } else {
                tx.estado = crate::transacao::Estado::Confirmando;
                // A aresta do grafo de espera e deste COMMIT, e nao do
                // anterior: se ele for barrado de novo, ela volta la embaixo,
                // com quem barrou agora (C1 do 516).
                tx.commit_barrado_por = None;
                // `take` e nao `clone`: depois de a marca estar no disco, a
                // lista em RAM deixou de ser a verdade -- a marca e. E cinco
                // mil linhas clonadas seriam cinco mil linhas de RAM para
                // jogar fora.
                Some((tx.id, tx.database.clone(), std::mem::take(&mut tx.escritas)))
            }
        };
        // A MESMA porta do prazo estourado na instrucao e na varredura
        // (`estourar_prazo` -> `abortar_soltando`): `ABORT_ONLY`, lista fora,
        // travas soltas ja. E e ela que da teto ao desempate do 516: a mais
        // velha de um ciclo, mandada repetir, para no proprio prazo.
        let Some((id, database, mut escritas)) = tirada else {
            drop(trava);
            return Err(self.estourar_prazo(sessao));
        };

        let quantas = escritas.len();
        if quantas == 0 {
            drop(trava);
            self.descartar_transacao(sessao.ligacao);
            return Ok(Json::objeto(vec![
                ("transaction_id", Json::de_u64(id)),
                (
                    "transaction_state",
                    Json::texto_de(crate::transacao::Estado::Confirmada.nome()),
                ),
                ("gravadas", Json::de_u64(0)),
                ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
            ]));
        }

        // ANTES da marca: toda recusa daqui DEVOLVE a lista e o estado. Nada
        // foi aplicado, a transacao continua `ACTIVE`, e o cliente pode mandar
        // `COMMIT` de novo -- e o `SQLITE_BUSY` no `COMMIT`, o unico dos quatro
        // motores em que o `COMMIT` pode falhar, e la a transacao tambem fica
        // ativa (`commit-contra-ddl-4-motores.md` §2). Antes, o `?` do
        // `abrir_database` deixava a transacao presa em `COMMITTING` com a
        // lista jogada fora.
        let dir = match self.preparar_a_marca(&trava, &database, &escritas) {
            Ok(d) => d,
            Err(e) => {
                drop(trava);
                self.devolver_a_lista(sessao.ligacao, escritas);
                return Err(e);
            }
        };
        // A LISTA INTEIRA se confere aqui, ainda antes da marca e com a mesma
        // trava que a passada vai usar -- pedido 448. Fica FORA do
        // `preparar_a_marca` de proposito: aquele volta cedo quando nada esta
        // congelado, e a conferencia so rodaria em dia de migracao.
        //
        // Recusa do DADO com zero aplicado e o que o PostgreSQL faz com
        // restricao que falha no COMMIT: a transacao termina, com o erro, e
        // `repetir` e falso -- repetir a mesma lista daria o mesmo erro.
        // Qualquer outra quebra aqui (E/S, tabela congelada) nao julgou lista
        // nenhuma, e ela volta como no `preparar_a_marca`.
        #[cfg(test)]
        let pre_conferir = !self.pre_conferencia_desligada.load(Ordering::SeqCst);
        #[cfg(not(test))]
        let pre_conferir = true;
        let conferida = if pre_conferir {
            self.pre_conferir_a_lista(&trava, &database, &mut escritas, sessao, |i, elos| {
                self.travar_os_elos(sessao, &database, i, elos)
            })
        } else {
            Ok(Vec::new())
        };
        let elos = match conferida {
            Ok(elos) => elos,
            Err(recusa) => {
                drop(trava);
                // A mais nova de um ciclo de COMMITs barrados CEDE (C1 do
                // 516): encerrada pelo gestor, com as travas soltas agora, e
                // nao devolvida -- devolver a lista com as travas seria manter
                // o ciclo que o desempate existe para quebrar.
                if let PhxError::TransacaoAbortada(m) = &recusa.erro {
                    let _ = self.abortar_soltando(sessao.ligacao, |_| m.clone());
                    return Err(recusa.erro);
                }
                // A LISTA INTEIRA recusada (pedido 685, acima do teto): nenhuma
                // escrita e a culpada, e a mensagem ja diz que nada foi gravado
                // e que a transacao terminou. Repetir daria o mesmo tamanho.
                if recusa.posicao >= escritas.len() {
                    self.descartar_transacao(sessao.ligacao);
                    return Err(recusa.erro);
                }
                if matches!(recusa.erro.codigo() / 1000, 2 | 3) {
                    let w = &escritas[recusa.posicao];
                    self.descartar_transacao(sessao.ligacao);
                    let na_cascata = match &recusa.elo {
                        Some((tabela, rowid)) => {
                            format!(", no elo da cascata que ela leva a {tabela} rowid {rowid}")
                        }
                        None => String::new(),
                    };
                    return Err(com_nota(
                        recusa.erro,
                        &format!(
                            "o COMMIT recusou a escrita {} de {quantas} ({} em {}, rowid \
                             {}{na_cascata}) ANTES da marca: nada desta transacao foi \
                             gravado, e ela terminou",
                            recusa.posicao + 1,
                            w.acao.nome(),
                            w.tabela,
                            w.rowid
                        ),
                    ));
                }
                self.devolver_a_lista(sessao.ligacao, escritas);
                return Err(recusa.erro);
            }
        };
        // UM planejador so (achado A1 da revisao do DBA): a cascata que a
        // pre-conferencia planejou entra na lista AGORA, antes da marca, como
        // elo achatado -- e a passada e a recuperacao aplicam cada alteracao
        // SEM replanejar. A passada replanejava abrindo a filha por um
        // segundo descritor, e a guarda do `.ndx` sujo pela propria passada
        // recusava mesmo com o plano vazio: `[inserir pedido->A, alterar a
        // chave de B]`, com B sem filha, saia pela metade e «DEFEITO DO
        // MOTOR». A marca v3 ja carrega elo achatado; o formato nao muda.
        let escritas = if pre_conferir {
            costurar_os_elos(escritas, elos)
        } else {
            escritas
        };
        let carimbo = crate::agora_ms();
        // A marca fica EM VOO na propria trava desde ANTES de ir ao disco --
        // pedido 451 (M1): um panico daqui ate o destino dela estar decidido
        // e reparado completando ESTA marca, e so ela. Antes da gravacao e nao
        // depois: um panico entre o `fsync` e o retorno deixaria uma marca
        // inteira que ninguem sabia que existia.
        trava.marca_em_voo = Some(MarcaEmVoo {
            database: database.clone(),
            caminho: crate::transacao::caminho_da_marca(&dir, id),
            gravada: false,
        });
        // A marca vai ao disco e e SINCRONIZADA antes de a passada tocar em
        // qualquer arquivo de dado. A ordem inversa tem uma janela em que o
        // trabalho existe pela metade e nao ha intencao nenhuma no disco para
        // completa-lo -- e essa janela nao tem conserto depois.
        let marca = match crate::transacao::gravar_marca(&dir, id, carimbo, &escritas) {
            Ok(c) => c,
            Err(e) => {
                drop(trava);
                self.abortar_transacao(
                    sessao.ligacao,
                    &format!("nao consegui gravar a marca de commit: {e}"),
                );
                return Err(e);
            }
        };
        // Daqui em diante a marca esta INTEIRA e sincronizada: se ela nao se
        // reler no reparo, e a leitura que falhou, e nao o commit (M4).
        if let Some(em_voo) = trava.marca_em_voo.as_mut() {
            em_voo.gravada = true;
        }
        self.depois_da_marca(trava, sessao, id, &database, escritas, &marca, inicio)
    }
}

/// O erro de um gatilho, com o nome de quem errou — MENOS o `SIGNAL`.
///
/// O `SIGNAL` passa intacto de proposito: a MESSAGE_TEXT e a mensagem que o
/// dono do banco escreveu para quem esbarrar na regra, e embrulha-la em
/// "gatilho x:" esconderia a frase dele atras da nossa.
/// O que uma espera de trava esta tentando pegar.
#[derive(Debug, Clone, Copy)]
pub(super) enum Alvo {
    Tabela(crate::travas::Trava),
    Linha(u64),
}

/// Quanto uma trava pedida espera por quem a segura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Espera {
    /// O `LOCK TIMEOUT` da transacao, FORA da trava de dados -- toda instrucao.
    AteOPrazo,
    /// Uma tentativa so: o COMMIT, que esta DENTRO da trava de dados (pedido
    /// 516). Ver `Servidor::esperar_trava`.
    Nenhuma,
}

/// Que direito na tabela a trava do `SCOPE` pede -- pedido 607. Basta um dos
/// da lista.
///
/// A regua dos motores, e o numero: o PostgreSQL (peso 4) pede, no `LOCK
/// TABLE`, `INSERT`/`UPDATE`/`DELETE`/`TRUNCATE` para o `ROW EXCLUSIVE` (a
/// nossa intencao, IX) e `UPDATE`/`DELETE`/`TRUNCATE` para os modos que barram
/// escrita alheia (a nossa X). MySQL (2) e MariaDB (3) pedem `SELECT` MAIS o
/// privilegio proprio `LOCK TABLES`, que aqui nao existe: tomar so a metade
/// `SELECT` deixaria a regra mais frouxa que a dos dois, e o leitor de uma
/// tabela travaria a escrita de todo mundo nela. SQLite (1) nao tem direito
/// por tabela. Sobra a regra do PostgreSQL, a unica que se escreve inteira no
/// nosso modelo. A compartilhada (S) da leitura repetivel nao nasce do
/// `SCOPE`: entra pela leitura, que ja passa pelo portao.
fn direitos_da_trava(t: crate::travas::Trava) -> &'static [Atividade] {
    use crate::travas::Trava;
    match t {
        Trava::Compartilhada => &[Atividade::Ler],
        Trava::Intencao => &[Atividade::Inserir, Atividade::Alterar, Atividade::Excluir],
        Trava::Exclusiva => &[Atividade::Alterar, Atividade::Excluir],
    }
}

/// A lista de tabelas do `SCOPE`, venha ela como lista ou como texto.
pub(super) fn lista_do_escopo(p: &Json) -> Vec<String> {
    let campo = p.campo("scope").or_else(|| p.campo("escopo"));
    match campo {
        Some(Json::Lista(itens)) => itens
            .iter()
            .filter_map(Json::texto)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        // Texto tambem serve: `"scope": "clientes, pedidos"` e o que sai de um
        // cliente que so tem string para oferecer.
        Some(Json::Texto(t)) => t
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// As tabelas em que um programa de rotina GRAVA, recursivamente.
///
/// Sai da arvore ja compilada, e nao do texto do corpo: procurar `INSERT INTO`
/// por comparacao de texto quebra calado no dia em que alguem escrever o mesmo
/// comando com outro espacamento -- a mesma armadilha de resolver texto de
/// tela comparando a frase.
fn tabelas_gravadas(programa: &[phxsql_sql::rotina::Instrucao], alvos: &mut Vec<String>) {
    use phxsql_sql::rotina::Instrucao;
    for i in programa {
        match i {
            Instrucao::Inserir { alvo, .. } => {
                if !alvo.tabela.is_empty() {
                    alvos.push(alvo.tabela.clone());
                }
            }
            Instrucao::Se { ramos, senao } => {
                for (_, corpo) in ramos {
                    tabelas_gravadas(corpo, alvos);
                }
                tabelas_gravadas(senao, alvos);
            }
            Instrucao::Enquanto { corpo, .. } => tabelas_gravadas(corpo, alvos),
            _ => {}
        }
    }
}

/// A recusa de quem mandou `COMMIT` sem ter aberto nada.
///
/// Nao e um erro de transacao: e um pedido fora de hora, e por isso e da
/// familia do esquema. A mensagem diz o que fazer, porque a causa mais comum
/// e um cliente que reconectou -- e a transacao morre com a conexao.
pub(super) fn sem_transacao() -> PhxError {
    PhxError::Esquema(
        "esta conexao nao tem transacao aberta; comece com {\"op\":\"begin\"} \
         (ou BEGIN / START TRANSACTION pelo SQL). Lembre que a transacao \
         pertence a CONEXAO: reconectar desfaz a que estava aberta"
            .into(),
    )
}

/// O nome de um `SAVEPOINT`, conferido.
///
/// Nome nao e dado: ele vira etiqueta dentro do servidor e volta na resposta.
/// Aceitar qualquer coisa deixaria passar uma quebra de linha, que e como uma
/// linha forjada aparece num log -- o mesmo cuidado do `de_uma_linha`.
fn nome_do_ponto(p: &Json) -> Result<String> {
    let nome = p
        .texto_ou("nome", p.texto_ou("savepoint", ""))
        .trim()
        .to_string();
    if nome.is_empty() {
        return Err(PhxError::Esquema("informe \"nome\" do SAVEPOINT".into()));
    }
    if nome.len() > 64 || !nome.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(PhxError::Esquema(format!(
            "nome de SAVEPOINT invalido: {nome:?}. Use ate 64 letras, numeros \
             ou sublinhado"
        )));
    }
    Ok(nome)
}

/// As chaves UNICAS desta linha, como `(indice, chave em texto)`.
///
/// # Por que a chave vira texto
///
/// Porque ela e comparada com as das outras linhas EMPILHADAS, e a comparacao
/// tem de ser exata e barata. O texto e o hexadecimal da chave CODIFICADA pelo
/// store -- a mesma que o `.ndx` compara --, e nao mais o `Debug` dos valores
/// crus: com a chave do store, o indice por expressao compara o que a
/// expressao da, e nao a coluna de onde ela saiu.
///
/// # E quem decide o que entra e o store (pedido 448, achado A4)
///
/// A regra do NULL -- chave com componente NULL nao colide -- era escrita aqui
/// E no store, e as duas divergiam: o store dava DUPLICADO no segundo NULL e
/// esta funcao o pulava. Hoje `Table::chaves_unicas` responde pelas duas
/// (`participa_da_unicidade`). A sequencia e o rownum, que o motor so preenche
/// na gravacao, continuam de fora pelo mesmo motivo de antes: ao empilhar eles
/// ainda sao NULL, e NULL nao colide.
fn chaves_unicas(t: &Table, linha: &[Value]) -> Result<Vec<(String, String)>> {
    Ok(t.chaves_unicas(linha)?
        .into_iter()
        .map(|(indice, k)| (indice, bytes_para_hex(&k)))
        .collect())
}
