//! A replicacao bidirecional e seus tipos.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;
use phxsql_store::table::Pendente;

/// A identidade de UMA tabela no bidirecional, emprestada -- o que o
/// [`Servidor::aplicar_tabela_bidi`] precisa, venha da rodada ou da marca do
/// arranque (pedido 698).
#[derive(Clone, Copy)]
pub(super) struct AlvoBidi<'a> {
    nome: &'a str,
    chave_tab: &'a str,
    indice: &'a str,
    pos_chave: &'a [usize],
    /// O slot vivo cuja inclusao nunca chegou ao diario daqui -- pedido 700.
    /// So o arranque que completa a marca o acha; na rodada e `None`.
    orfa: Option<u64>,
}

/// A tabela `b/c` casada por `porId`, a dos testes do `aplicar_por_chave`.
#[cfg(test)]
pub(super) const ALVO_DE_TESTE: AlvoBidi<'static> = AlvoBidi {
    nome: "c",
    chave_tab: "b/c",
    indice: "porId",
    pos_chave: &[0],
    orfa: None,
};

/// Uma tabela de um grupo do bidirecional: quem, e o que aplicar. Ver
/// [`Servidor::aplicar_itens_bidi`].
struct ItemBidi<'a> {
    nome: &'a str,
    chave_tab: &'a str,
    /// O indice e as colunas da chave, ja conferidos pela rodada; `None` no
    /// arranque (pedido 698), que os tira do esquema.
    identidade: Option<(&'a str, &'a [usize])>,
    eventos: &'a [crate::replica::EventoRecebido],
}

/// O que os itens de um grupo do bidi deram: os eventos que entraram e as
/// paradas nominais, pelo indice no grupo.
type FeitoBidi = (u64, Vec<(usize, (u64, &'static str, String))>);

/// A quebra no meio de um grupo do bidi -- pedido 722. `reter` = parte do
/// grupo entrou e a completacao na hora nao fechou: a marca FICA no disco
/// para o arranque, e quem chama a tira da lista que a rodada apaga.
struct ParouNoGrupo {
    erro: PhxError,
    reter: bool,
}

/// O que a fase 3 de um alcance de tabela BIDIRECIONAL devolveu.
#[derive(Default)]
struct LoteBidi {
    /// Eventos que entraram no `.reg` daqui.
    aplicados: u64,
    /// O evento que PAROU o par: a posicao dele no diario da origem, o motivo
    /// (em chave) e o detalhe ja redigido. `None` = o lote inteiro passou.
    parou_em: Option<(u64, &'static str, String)>,
    /// O erro que quebrou o lote no meio -- pedido 722. Vem DENTRO do lote, e
    /// nao pelo `?`, para quem chama saber quantos entraram antes dele: com
    /// `aplicados > 0` o grupo esta pela metade e tem de se completar.
    erro: Option<PhxError>,
}

/// Uma tabela no alcance BIDIRECIONAL de um database -- pedido 681. O que o
/// [`crate::replica::Juntador`] nao sabe: a identidade por chave e a posicao
/// consumida, que no bidirecional e estado proprio (o diario daqui mistura
/// escrita local com aplicada).
struct FilaBidi {
    no: crate::replica::NoSource,
    indice: String,
    pos_chave: Vec<usize>,
    chave_tab: String,
    chave_pos: String,
    /// A posicao consumida no comeco do alcance, e a de agora.
    inicio: u64,
    desde: u64,
    /// Algum grupo passou por ela: vai ao `fsync` no fim, mesmo saindo pelo
    /// erro.
    tocada: bool,
    /// O par parou nela neste alcance: a posicao nao anda mais.
    parada: bool,
    /// A vez dela num grupo: a mae antes da filha ([`ordem_das_maes`]).
    vez: usize,
}

/// Quantos eventos um `replicar` devolve quando o pedido nao diz -- ou diz
/// zero, ou diz um numero negativo.
///
/// Zero NAO e "sem limite" aqui, e isso e decisao. No store, `limite == 0`
/// quer dizer "todos os eventos" -- e o contrato que a CLI, o PITR e os
/// testes usam --, e era exatamente isso que vazava pelo fio (revisao SEC de
/// 17/09/2026, A2): `"max":0` fazia o source caminhar o diario INTEIRO,
/// decifrando cada imagem, com a trava global na mao, e o
/// `TETO_DO_LOTE_SERVIDO` so cortava a resposta depois. Pelo fio, zero vale o
/// padrao, que e o lote que a propria replica pede.
pub(super) const LOTE_PADRAO_DE_REPLICACAO: u64 = 500;

/// Quanto dura uma fatia da absorcao do diario local sob a trava de LEITURA
/// (pedido 330, decisao do J de 01/10/2026). E o teto da espera de um
/// escritor durante a primeira rodada do bidirecional, mais o lote em que a
/// fatia vence -- um lote de 500 a ~2,4 us por evento (release) sao ~1,2 ms.
/// Mais curta, a troca de trava passa a pesar; mais longa, o escritor sente.
const FATIA_DA_PRE_ABSORCAO: Duration = Duration::from_millis(10);

/// Quanto a pre-absorcao espera, entre duas fatias, o escritor da fila pegar
/// a ficha exclusiva (pedido 623). Nao e a espera esperada -- essa e a de um
/// escritor acordado ser escalado, microssegundos --, e a vida do laco se o
/// escritor nunca entrar por um motivo que a fila nao ve. Vencido, a fatia
/// seguinte anda e conta `fatias_que_furaram_a_fila`.
const TETO_DA_VEZ_CEDIDA: Duration = Duration::from_secs(5);

impl Servidor {
    /// Uma passada bidirecional: puxa do outro lado e aplica POR CHAVE.
    ///
    /// E o modo multi-master. Difere da replica fiel em tres pontos, e os
    /// tres estao em `docs/REPLICACAO.md`: a identidade e a chave unica (o
    /// rowid e local de cada servidor), o conflito e "mais recente vence"
    /// pelo carimbo do nascimento da escrita, e a posicao consumida vira
    /// estado proprio -- o diario local mistura escrita local com aplicada e
    /// deixa de ser a contagem da origem.
    pub(super) fn rodada_bidirecional(
        &self,
        mut cliente: crate::replica::Cliente,
        origem: &crate::config::Origem,
    ) -> Result<u64> {
        let meu_id = self.config.replicacao.id_servidor.trim().to_string();
        let meu_hash = self.config.replicacao.numero();
        // O mesmo filtro da replica fiel, e pela mesma razao: o bidirecional
        // casa linha por CHAVE, mas duas origens no mesmo database ainda
        // disputam a mesma tabela daqui -- e a posicao consumida e por
        // "origem|database/tabela", entao cada uma acharia que a outra nunca
        // escreveu.
        let anunciados = if origem.databases.is_empty() {
            cliente.databases()?
        } else {
            origem.databases.clone()
        };
        let databases = self.so_os_databases_desta_origem(&origem.nome, anunciados);

        let mut aplicados = 0u64;
        for database in databases {
            let p = crate::replica::posicao(&mut cliente, &database)?;
            if !p.com_imagem {
                return Err(PhxError::Esquema(format!(
                    "o outro lado ({}) esta sem replicacao.imagem_da_linha: no \
                     bidirecional a chave mora dentro da imagem",
                    origem.nome
                )));
            }
            // A identidade e obrigatoria DOS DOIS lados: sem o id do outro nao
            // ha supressao de origem confiavel, e sem supressao ha laco.
            let id_dele = p.id_servidor.trim().to_string();
            if id_dele.is_empty() {
                return Err(PhxError::Esquema(format!(
                    "o outro lado ({}) nao informa id_servidor: o bidirecional \
                     exige replicacao.id_servidor nos dois servidores",
                    origem.nome
                )));
            }
            if id_dele == meu_id {
                return Err(PhxError::Esquema(format!(
                    "os dois servidores usam o MESMO id_servidor ({meu_id}): \
                     cada um precisa do seu, senao a supressao de origem \
                     descarta tudo"
                )));
            }
            // A colisao de numero falharia calada, suprimindo eventos de um
            // servidor inocente -- e nao so entre os dois do par: contra TODO
            // par que este no ja viu (pedido 329). O numero do outro e o que
            // ele diz no `posicao`; um source de antes do campo nao diz, e ai
            // vale o hash do id, que e o numero que ele de fato usa.
            // Pedido 496, F7 (4b): o irmao da poda da replica fiel.
            self.anotar_estado(&origem.nome, |e| e.podar_atrasos(&database, &p.tabelas));
            let hash_dele = bidirecional::numero_do_servidor(p.numero_servidor, &id_dele);
            self.conferir_numero_do_par(hash_dele, &id_dele)?;
            aplicados += self.alcancar_database_bidi(
                &mut cliente,
                &database,
                p.tabelas,
                origem,
                &meu_id,
                meu_hash,
                hash_dele,
            )?;
        }
        Ok(aplicados)
    }

    /// Abre a tabela do lado de ca e prepara o confronto por chave.
    ///
    /// Fase 1 do bidirecional: toma a trava, garante a tabela, confere que ela
    /// tem chave unica e absorve o diario local no mapa de toques. Solta a
    /// trava ao voltar. `None` = nao ha o que replicar nesta tabela.
    pub(super) fn abrir_para_bidi(
        &self,
        database: &str,
        no: &crate::replica::NoSource,
        origem: &crate::config::Origem,
        meu_hash: u16,
    ) -> Result<Option<bidirecional::Identidade>> {
        // Pedido 330: o grosso do diario local entra no mapa ANTES, sob a
        // trava de leitura e em fatias; a exclusiva de baixo so ve a cauda.
        self.pre_absorver_sob_leitura(
            database,
            &no.nome,
            &format!("{database}/{}", no.nome),
            meu_hash,
        )?;
        // Pedido 589: o que nasceu sob a trava vai ao disco depois de ela
        // soltar -- a trava e da funcao de dentro, e solta ao ela voltar.
        let (saida, pendente) = self.abrir_para_bidi_sob_a_trava(database, no, origem, meu_hash)?;
        pendente.levar_ao_disco()?;
        Ok(saida)
    }

    /// O [`Self::abrir_para_bidi`] com a trava na mao, devolvendo o `fsync`
    /// do que criou (vazio quando a tabela ja existia).
    fn abrir_para_bidi_sob_a_trava(
        &self,
        database: &str,
        no: &crate::replica::NoSource,
        origem: &crate::config::Origem,
        meu_hash: u16,
    ) -> Result<(Option<bidirecional::Identidade>, PorSincronizar)> {
        let trava = self.travar_dados()?;
        let (tabela, pendente) =
            garantir_tabela_da_replica(&trava, database, no, &self.ledger_marcado_recebido)?;
        let Some(mut tabela) = tabela else {
            return Ok((None, pendente));
        };
        let chave_tab = format!("{database}/{}", no.nome);
        let Some((indice, pos_chave)) = bidirecional::chave_unica(tabela.esquema()) else {
            // A recusa com o motivo escrito: sem chave unica nao ha
            // identidade entre servidores, e adivinhar pela posicao ou pelo
            // rowid gravaria a linha de alguem por cima da de outro. A chave
            // que aceita nulo tambem nao serve (pedido 517), e a frase diz
            // qual coluna tornar obrigatoria.
            let motivo = bidirecional::por_que_sem_chave(tabela.esquema(), &no.nome);
            self.anotar_estado(&origem.nome, |e| {
                e.recusas.insert(chave_tab.clone(), motivo.clone());
            });
            return Ok((None, pendente));
        };
        // A tabela serve: se ela ja esteve recusada, o recado sai. Recado que
        // sobrevive ao conserto vira configuracao que mente -- alguem criou o
        // indice unico e continuaria lendo que a tabela nao replica.
        self.anotar_estado(&origem.nome, |e| {
            e.recusas.remove(&chave_tab);
        });
        // O diario local que ainda nao passou pelo mapa de toques -- inclui a
        // escrita local desde a ultima rodada, que e quem disputa o conflito.
        self.absorver_diario_local(&mut tabela, &chave_tab, &pos_chave, meu_hash, None, true)?;
        Ok((Some((indice, pos_chave)), pendente))
    }

    /// Aplica numa tabela os eventos dela que vieram num grupo do
    /// bidirecional, com a trava ja na mao de quem chama -- fase 3, por
    /// [`Self::aplicar_grupo_bidi`] (pedido 681).
    ///
    /// Recebe a identidade solta, e nao a `FilaBidi`, porque o arranque a
    /// chama tambem, completando a marca do grupo (pedido 698) sem rodada
    /// nenhuma -- pelo MESMO motor, e nao por um segundo aplicador.
    fn aplicar_tabela_bidi(
        &self,
        db: &phxsql_store::catalogo::Database,
        alvo: AlvoBidi<'_>,
        eventos: &[crate::replica::EventoRecebido],
        meu_hash: u16,
        hash_dele: u16,
    ) -> Result<LoteBidi> {
        // No multi as duas imagens sao obrigatorias -- a chave mora nela, e
        // exclusao tambem viaja por chave --, e quem as liga e a politica do
        // diario herdada do `Database` (`politica_do_diario`, pedido 564).
        let mut tabela = db.abrir_qualificada(alvo.nome)?;
        // O irmao da replica fiel (pedido 300 §2.7): o `inserir_replicado` e o
        // `atualizar_replicado` tambem nao julgam, e passam pela MESMA
        // conferencia que conta.
        tabela.contar_orfas();
        let saida = self.aplicar_eventos_bidi(&mut tabela, alvo, eventos, meu_hash, hash_dele);
        self.anotar_orfas(alvo.chave_tab, tabela.orfas_contadas());
        saida
    }

    /// So em `debug`: o gancho `PHXSQL_TESTE_FALHAR_NO_EVENTO` do pedido 722
    /// cai neste evento? `<tabela>:<rowid>` cai sempre; `<tabela>:<rowid>:uma`,
    /// so na primeira vez do processo.
    #[cfg(debug_assertions)]
    fn falhar_no_evento_de_teste(tabela: &str, rowid: u64) -> bool {
        static JA_CAIU: AtomicBool = AtomicBool::new(false);
        let Some(v) = gancho_de_teste_em_texto("PHXSQL_TESTE_FALHAR_NO_EVENTO") else {
            return false;
        };
        let alvo = format!("{tabela}:{rowid}");
        match v.strip_suffix(":uma") {
            Some(uma) => uma == alvo && !JA_CAIU.swap(true, Ordering::SeqCst),
            None => v == alvo,
        }
    }

    /// O laco de [`Self::aplicar_tabela_bidi`], separado para a contagem de
    /// orfas ser colhida tambem quando ele sai pelo erro.
    fn aplicar_eventos_bidi(
        &self,
        tabela: &mut Table,
        alvo: AlvoBidi<'_>,
        eventos: &[crate::replica::EventoRecebido],
        meu_hash: u16,
        hash_dele: u16,
    ) -> Result<LoteBidi> {
        let mut orfa = alvo.orfa;
        let mut saida = LoteBidi::default();
        for e in eventos {
            // Cinto e suspensorio: o source ja suprimiu pelo `para`, e
            // ainda assim um evento com a MINHA origem nao se aplica --
            // um source antigo, que ignora o campo, reabriria o laco.
            if e.origem == meu_hash {
                continue;
            }
            // So em `debug`, pedido 700: o N-esimo evento tentado morre DENTRO
            // da inclusao, com o slot no `.reg` e o evento fora do diario --
            // o mesmo gancho do grupo da replica fiel (699).
            #[cfg(debug_assertions)]
            {
                static TENTADOS: AtomicU64 = AtomicU64::new(0);
                let vez = TENTADOS.fetch_add(1, Ordering::SeqCst) + 1;
                let parar_no_reg: Option<u64> = std::env::var("PHXSQL_TESTE_PARAR_NO_REG")
                    .ok()
                    .and_then(|v| v.parse().ok());
                if parar_no_reg == Some(vez) {
                    phxsql_store::ndx::panico_de_teste::armar_gancho(
                        phxsql_store::ndx::panico_de_teste::Ponto::InserirDepoisDoContador,
                        || sigkill_de_teste("teste: bidirecional parado entre o .reg e o diario"),
                    );
                }
            }
            // So em `debug`, pedido 722: o evento `<tabela>:<rowid de la>`
            // falha com erro do DADO no lugar de se aplicar -- sempre, como um
            // erro de dado de verdade bate no mesmo evento em toda tentativa.
            // Com `:uma` no fim, falha SO a primeira vez: o erro passageiro,
            // que a completacao na hora atravessa.
            #[cfg(debug_assertions)]
            if Self::falhar_no_evento_de_teste(alvo.nome, e.rowid) {
                eprintln!(
                    "teste: erro de dado injetado no evento {}:{}",
                    alvo.nome, e.rowid
                );
                saida.erro = Some(PhxError::Duplicado(format!(
                    "teste: erro de dado injetado no evento {}:{}",
                    alvo.nome, e.rowid
                )));
                break;
            }
            let aplicacao = match self.aplicar_por_chave(tabela, alvo, e, hash_dele, &mut orfa) {
                Ok(a) => a,
                Err(erro) => {
                    saida.erro = Some(erro);
                    break;
                }
            };
            match aplicacao {
                bidirecional::Aplicacao::Aplicado => {
                    saida.aplicados += 1;
                    // So em `debug`: a prova do 698 mata o PROCESSO depois do
                    // N-esimo evento aplicado por chave, com parte do grupo no
                    // disco -- o mesmo esquema do grupo da replica fiel.
                    #[cfg(debug_assertions)]
                    {
                        static APLICADOS: AtomicU64 = AtomicU64::new(0);
                        let feitos = APLICADOS.fetch_add(1, Ordering::SeqCst) + 1;
                        let parar_em: Option<u64> = std::env::var("PHXSQL_TESTE_PARAR_NO_GRUPO")
                            .ok()
                            .and_then(|v| v.parse().ok());
                        if parar_em == Some(feitos) {
                            sigkill_de_teste("teste: parado no meio do grupo do bidirecional");
                        }
                    }
                }
                bidirecional::Aplicacao::Ignorado => {}
                // O lote PARA aqui, e os eventos seguintes ficam para depois
                // do `replicacao_pular`: seguir aplicaria uma alteracao de uma
                // linha que o conflito deixou de fora, e a divergencia se
                // espalharia em vez de ficar num ponto que se sabe nomear.
                bidirecional::Aplicacao::Conflito(c) => {
                    let detalhe = Self::detalhe_do_conflito(&c);
                    saida.parou_em = Some((e.posicao, "conflito_de_unicidade", detalhe));
                    break;
                }
                // Pedido 330 (b): o teto esqueceu o toque e o evento nao passa
                // do piso. Para pelo mesmo motivo do conflito -- a posicao nao
                // anda, e nenhum lado e escolhido calado.
                bidirecional::Aplicacao::Esquecida(chave) => {
                    let detalhe = self.detalhe_do_esquecido(&chave);
                    saida.parou_em = Some((e.posicao, "toque_esquecido_pelo_teto", detalhe));
                    break;
                }
            }
        }
        // Um `fsync` por alcance, e nao por lote -- ver a nota em
        // `aplicar_lote_da_replica`, onde a conta esta medida.
        Ok(saida)
    }

    /// A posicao consumida de `origem|db/tab` no mapa do bidirecional; zero
    /// quando nunca andou.
    fn posicao_bidi(&self, chave_pos: &str) -> u64 {
        self.posicoes_bidi
            .lock()
            .ok()
            .and_then(|p| p.get(chave_pos).copied())
            .unwrap_or(0)
    }

    /// Fase 1 do bidirecional para UMA tabela: o portao da parada, a tabela
    /// aberta com a identidade, e a posicao consumida. `None` = nada a fazer
    /// nesta tabela nesta rodada.
    fn preparar_para_bidi(
        &self,
        database: &str,
        no: crate::replica::NoSource,
        origem: &crate::config::Origem,
        meu_hash: u16,
    ) -> Result<Option<FilaBidi>> {
        let chave_tab = format!("{database}/{}", no.nome);
        let chave_pos = format!("{}|{}", origem.nome, chave_tab);
        // O PORTAO VEM ANTES DO TRABALHO: a tabela parada sai daqui sem tomar
        // a trava de dados, sem absorver o diario local e sem uma unica ida e
        // volta de rede. E a licao do Profiler aplicada a uma parada: quem
        // pergunta se esta ligado DEPOIS de trabalhar paga o trabalho a cada
        // rodada, e aqui isso seria o laco apertado que o pedido 292 existe
        // para matar. Ver `bidirecional::ParadaDaTabela`.
        if self.esta_parada(&origem.nome, &chave_tab) {
            // Pedido 496, F7: a parada com a origem gravando e justamente a
            // tabela que o atraso tem de ver, e a amostra nao e trabalho que
            // o portao poupa -- a posicao e uma busca no mapa em memoria, e o
            // `no.eventos` veio no `posicao`.
            let consumida = self.posicao_bidi(&chave_pos);
            self.amostrar_atraso(&origem.nome, database, &no.nome, no.eventos, consumida);
            return Ok(None);
        }
        // A recusada (sem chave unica) NAO se amostra: ela nao replica por
        // decisao de modelagem, ja diz por que em `recusas`, e um atraso que
        // cresce para sempre ali seria o mesmo recado em forma de alarme.
        let Some((indice, pos_chave)) = self.abrir_para_bidi(database, &no, origem, meu_hash)?
        else {
            return Ok(None);
        };
        let desde = self.posicao_bidi(&chave_pos);
        // O mesmo motor da replica fiel, no mesmo ponto: o comeco do alcance.
        self.amostrar_atraso(&origem.nome, database, &no.nome, no.eventos, desde);
        if desde >= no.eventos {
            return Ok(None);
        }
        Ok(Some(FilaBidi {
            no,
            indice,
            pos_chave,
            chave_tab,
            chave_pos,
            inicio: desde,
            desde,
            tocada: false,
            parada: false,
            vez: 0,
        }))
    }

    /// Traz um DATABASE inteiro do outro lado, casando pela chave, e aplica
    /// cada transacao de la inteira ou nada -- pedido 681, o irmao do 676.
    ///
    /// # Por que por database, e nao mais tabela a tabela
    ///
    /// O diario e por tabela, e a venda de la (venda, itens, pagamento) e um
    /// commit so. Tabela a tabela, um leitor daqui via os itens sem a venda
    /// entre um alcance e o seguinte, e o fio que caisse no meio deixava assim
    /// ate voltar. A decisao de QUANDO uma transacao esta inteira e a do pull,
    /// e mora num lugar so: o [`crate::replica::Juntador`]. O que muda aqui e
    /// so o COMO aplicar -- por chave, com o «mais recente vence» e a parada
    /// do par, que e decisao diferente do rowid da replica fiel e por isso
    /// nao passa pelo `aplicar_grupo_da_replica`.
    ///
    /// # As tres fases, como antes
    ///
    /// Puxar FORA da trava (o abraco mortal dos dois lados, medido: 33,0 s
    /// contra 2,4 s), e aplicar cada grupo sob UMA tomada. Partidas assim
    /// desde o pedido do laco do bidirecional, e o motivo continua: aqui os
    /// DOIS lados rodam este laco.
    #[allow(
        clippy::too_many_arguments,
        reason = "o alcance de um database junta as identidades dos dois lados"
    )]
    fn alcancar_database_bidi(
        &self,
        cliente: &mut crate::replica::Cliente,
        database: &str,
        nos: Vec<crate::replica::NoSource>,
        origem: &crate::config::Origem,
        meu_id: &str,
        meu_hash: u16,
        hash_dele: u16,
    ) -> Result<u64> {
        let mut filas: Vec<FilaBidi> = Vec::new();
        for no in nos {
            if let Some(f) = self.preparar_para_bidi(database, no, origem, meu_hash)? {
                filas.push(f);
            }
        }
        if filas.is_empty() {
            return Ok(0);
        }
        let vezes = ordem_das_maes(&filas.iter().map(|f| &f.no).collect::<Vec<_>>());
        for (f, vez) in filas.iter_mut().zip(vezes) {
            f.vez = vez;
        }
        let alvos: Vec<(u64, u64)> = filas.iter().map(|f| (f.desde, f.no.eventos)).collect();
        let mut juntador =
            crate::replica::Juntador::novo(&alvos, phxsql_store::log::teto_da_transacao());
        let mut aplicados = 0u64;
        // As marcas dos grupos (pedido 698): saem depois do `fsync` do fim.
        let mut marcas: Vec<PathBuf> = Vec::new();
        let saida = loop {
            match juntador.passo() {
                crate::replica::Passo::Puxar { fila, desde } => {
                    // FORA da trava -- ver a nota da funcao.
                    match crate::replica::puxar_lote(
                        cliente,
                        database,
                        &filas[fila].no.nome,
                        desde,
                        Some((meu_id, meu_hash)),
                    ) {
                        Ok(lote) => juntador.receber_ate(fila, lote.eventos, lote.ate),
                        Err(e) => break Err(e),
                    }
                }
                crate::replica::Passo::Aplicar { grupo, .. } => {
                    for (i, _) in &grupo {
                        filas[*i].tocada = true;
                    }
                    let (n, paradas) = match self.aplicar_grupo_bidi(
                        database,
                        &filas,
                        grupo,
                        meu_hash,
                        hash_dele,
                        &mut marcas,
                    ) {
                        Ok(x) => x,
                        Err(e) => break Err(e),
                    };
                    aplicados += n;
                    // O conflito de unicidade PARA o par nesta tabela, e a
                    // posicao NAO anda -- e a mesma decisao escrita no
                    // `worker.c` do PostgreSQL: nao avancar a origem e o que
                    // impede perder o evento. Reaplicar os que entraram antes
                    // dele, quando o par for solto, e inofensivo: o casamento
                    // e por chave e a regra e "mais recente vence".
                    for (i, (posicao, motivo, detalhe)) in paradas {
                        juntador.largar(i);
                        let f = &filas[i];
                        self.parar_o_par(
                            &origem.nome,
                            &f.chave_tab,
                            &f.chave_pos,
                            posicao,
                            motivo,
                            detalhe,
                        );
                        filas[i].tocada = true;
                        filas[i].parada = true;
                    }
                    // Aqui a posicao anda so na MEMORIA deste laco. Ela vai ao
                    // mapa compartilhado e ao disco no fim do alcance, depois
                    // do `fsync` do dado -- ver a nota abaixo.
                    for (i, f) in filas.iter_mut().enumerate() {
                        if f.parada {
                            continue;
                        }
                        f.desde = juntador.consumido(i);
                        self.anotar_estado(&origem.nome, |est| {
                            est.posicoes.insert(f.chave_tab.clone(), f.desde);
                        });
                    }
                }
                crate::replica::Passo::Fim => break Ok(()),
            }
        };
        // O que passou so por eventos suprimidos (nasceram aqui) tambem anda.
        if saida.is_ok() {
            for (i, f) in filas.iter_mut().enumerate() {
                if !f.parada {
                    f.desde = juntador.consumido(i);
                }
            }
        }
        if juntador.em_pedacos > 0 {
            let partidas = juntador.em_pedacos;
            self.anotar_estado(&origem.nome, |e| e.transacoes_em_pedacos += partidas);
        }
        // Pedido 535: DADO DURAVEL PRIMEIRO, POSICAO DEPOIS.
        //
        // A posicao ia ao disco a cada lote, num `write` sem `fsync`, e o dado
        // so no `fsync` do fim do alcance: nada ordenava os dois, e a posicao
        // podia chegar primeiro (escrita de fundo, ou o diario do ext4 puxado
        // pelo `fsync` de outro arquivo). Na queda, os eventos entre o dado
        // perdido e a posicao gravada nunca mais eram pedidos.
        //
        // Os tres maduros convergem no COMPORTAMENTO -- a posicao consumida
        // nunca fica a frente do dado duravel --, e o fazem pelo MEIO da
        // transacao: a origem de replicacao do PostgreSQL avanca no registro
        // de commit, o `mysql.slave_relay_log_info` do MySQL e o
        // `gtid_slave_pos` da MariaDB sao tabelas InnoDB gravadas no mesmo
        // commit. Aceite do comportamento, e nao do meio: aqui a aplicacao e
        // por chave com «mais recente vence», entao reaplicar e inofensivo, e
        // basta que a posicao nunca passe o dado. Gravar a posicao DENTRO da
        // tabela seria mudar o formato do `.reg` para comprar uma atomicidade
        // de que a idempotencia ja nos dispensa.
        //
        // O mapa compartilhado so recebe a posicao DEPOIS do `fsync`, e nao a
        // cada lote: ele e gravado INTEIRO, e um alcance de outra origem que o
        // gravasse no meio deste levaria ao disco a posicao desta tabela a
        // frente do dado dela -- o mesmo defeito, por um irmao.
        //
        // Sincroniza tambem quando nada se aplicou mas a posicao andou: um
        // alcance anterior que caiu no meio (erro de rede depois de aplicar)
        // deixou dado sem `fsync`, e o lote repetido agora chega todo
        // «ignorado» -- gravar a posicao sem o `fsync` passaria por cima dele.
        //
        // E vale tambem na saida pelo erro (o fio que caiu no meio, 681): o
        // que ja foi aplicado eram transacoes inteiras, e a posicao delas so
        // vai ao disco depois do `fsync` delas.
        for f in &filas {
            if f.tocada || f.desde > f.inicio {
                self.sincronizar_replicada(database, &f.no.nome)?;
            }
        }
        // O dado no disco, entao o bilhete sai -- a ordem do group commit.
        Self::soltar_marcas_da_replica(marcas);
        if filas.iter().any(|f| f.desde > f.inicio) {
            // A troca duravel (temporario, `fsync`, `rename`, `fsync` da
            // pasta) roda FORA da trava global de dados -- so com a do mapa
            // de posicoes, que nenhuma escrita de cliente toma.
            let mut p = self
                .posicoes_bidi
                .lock()
                .map_err(|_| PhxError::Io(std::io::Error::other("posicoes_bidi envenenado")))?;
            for f in filas.iter().filter(|f| f.desde > f.inicio) {
                p.insert(f.chave_pos.clone(), f.desde);
            }
            bidirecional::gravar_posicoes(&self.config.base.join("replicacao-posicoes.json"), &p)?;
        }
        saida.map(|()| aplicados)
    }

    /// Um grupo do [`crate::replica::Juntador`] no bidirecional, sob UMA
    /// tomada da trava: a mae antes da filha, e cada tabela pelo casamento por
    /// chave. Devolve quantos eventos entraram e as tabelas que PARARAM o par
    /// (conflito de unicidade ou toque esquecido), com onde e por que.
    ///
    /// # A quebra no meio -- pedido 722
    ///
    /// Vale aqui a decisao do dono no 685: a venda chega inteira ou nao chega.
    /// Desfazer devolveria slot (ordem de digitacao), entao, como no grupo da
    /// replica fiel desde o 713, so se anda para a FRENTE: o erro com parte
    /// do grupo aplicada completa o resto na hora, com a mesma trava, pelo
    /// corpo da completacao do arranque, e o que nem assim fecha deixa a
    /// marca no disco e fora da lista da rodada. Este comentario dizia que a
    /// meia transacao era de proposito «como no grupo da replica fiel»: a
    /// paridade era a razao, e o 713 a acabou.
    ///
    /// A parada NOMINAL (conflito de unicidade, toque esquecido) continua por
    /// tabela, com a posicao parada no evento -- o que entrou antes dela na
    /// mesma transacao fica. Fechar isso pede a pre-conferencia do grupo
    /// inteiro antes do primeiro evento (E7 do desenho unico), e e o resto do
    /// pedido 722.
    ///
    /// # A queda do PROCESSO no meio -- pedido 698
    ///
    /// O irmao do 682: o grupo grava a MESMA marca da replica fiel (o mesmo
    /// [`crate::transacao::gravar_marca_do_bidi`], o mesmo formato, o mesmo
    /// selo), sincronizada ANTES da trava, e o arranque a completa pelo MESMO
    /// motor que aplica aqui ([`Self::completar_marcas_do_bidi`]). Ela entra
    /// em `marcas` antes do primeiro evento e sai depois do `fsync` do
    /// alcance, como a do grupo da replica.
    #[allow(
        clippy::type_complexity,
        reason = "a parada carrega posicao, motivo e detalhe, como `LoteBidi`"
    )]
    fn aplicar_grupo_bidi(
        &self,
        database: &str,
        filas: &[FilaBidi],
        mut grupo: Vec<(usize, Vec<crate::replica::EventoRecebido>)>,
        meu_hash: u16,
        hash_dele: u16,
        marcas: &mut Vec<PathBuf>,
    ) -> Result<(u64, Vec<(usize, (u64, &'static str, String))>)> {
        // A vez das maes, a mesma do grupo da replica fiel (`ordem_das_maes`).
        grupo.sort_by_key(|(i, _)| filas[*i].vez);
        // O bilhete antes da trava: o `fsync` dele com a trava global na mao
        // pararia o servidor pelo tempo de um disco (catraca `alcancam-fsync`).
        let marca = self.marcar_o_grupo_bidi(database, filas, &grupo, hash_dele)?;
        if let Some(m) = &marca {
            marcas.push(m.clone());
        }
        // So em `debug`, pedido 723: o processo morre com a marca do grupo no
        // disco e NENHUM evento dele aplicado -- a copia fria que o palco tem
        // de aceitar, tirando a marca.
        #[cfg(debug_assertions)]
        if marca.is_some()
            && gancho_de_teste("PHXSQL_TESTE_PARAR_DEPOIS_DA_MARCA_DO_BIDI") == Some(1)
        {
            sigkill_de_teste("teste: bidirecional parado depois da marca do grupo");
        }
        let itens: Vec<ItemBidi<'_>> = grupo
            .iter()
            .map(|(i, eventos)| {
                let f = &filas[*i];
                ItemBidi {
                    nome: &f.no.nome,
                    chave_tab: &f.chave_tab,
                    identidade: Some((&f.indice, &f.pos_chave)),
                    eventos,
                }
            })
            .collect();
        let (n, paradas) = match self.aplicar_itens_bidi(
            database,
            marca.as_deref(),
            &itens,
            meu_hash,
            hash_dele,
        ) {
            Ok(x) => x,
            Err(ParouNoGrupo { erro, reter }) => {
                // A marca do grupo pela metade que nao se completou FICA
                // no disco para o arranque: a rodada a apagaria depois do
                // `fsync`, e ela e o unico bilhete da metade que falta (I12).
                if reter {
                    if let Some(m) = &marca {
                        marcas.retain(|x| x != m);
                    }
                }
                return Err(erro);
            }
        };
        let paradas = paradas
            .into_iter()
            .map(|(k, p)| (grupo[k].0, p))
            .collect::<Vec<_>>();
        // Grupo em que nada entrou nao tem o que trazer de volta.
        if n == 0 {
            if let Some(m) = &marca {
                let _ = std::fs::remove_file(m);
                marcas.retain(|x| x != m);
            }
        }
        Ok((n, paradas))
    }

    /// Grava e sincroniza a marca do grupo do bidirecional (pedido 698), SEM a
    /// trava de dados. `None` quando o grupo nao tem evento a aplicar.
    ///
    /// A origem de cada evento vai RESOLVIDA (zero vira `hash_dele`, como o
    /// `aplicar_por_chave` faz): o arranque que a completa nao conversa com o
    /// outro lado, e nao saberia de quem era o zero.
    fn marcar_o_grupo_bidi(
        &self,
        database: &str,
        filas: &[FilaBidi],
        grupo: &[(usize, Vec<crate::replica::EventoRecebido>)],
        hash_dele: u16,
    ) -> Result<Option<PathBuf>> {
        let mut eventos = Vec::new();
        for (i, lista) in grupo {
            let f = &filas[*i];
            for e in lista {
                eventos.push(crate::transacao::EventoDoGrupo {
                    tabela: &f.no.nome,
                    operacao: e.operacao,
                    rowid: e.rowid,
                    carimbo_ms: e.carimbo_ms,
                    origem: if e.origem == 0 { hash_dele } else { e.origem },
                    posicao: e.posicao,
                    imagem: &e.imagem,
                });
            }
        }
        if eventos.is_empty() {
            return Ok(None);
        }
        let dir = self.config.base.join(database);
        let id = self.transacoes.travar().numero_de_marca();
        crate::transacao::gravar_marca_do_bidi(&dir, id, crate::agora_ms(), &eventos).map(Some)
    }

    /// Completa, no arranque, as marcas de grupo do bidirecional que um
    /// processo anterior deixou -- pedido 698.
    ///
    /// # Pelo MESMO motor, e nao por um segundo aplicador
    ///
    /// A marca da replica fiel se completa no store, pelo rowid e pela
    /// posicao. A do bidirecional casa pela CHAVE, com o «mais recente
    /// vence», e isso mora aqui: o mapa de toques e o servidor. Entao cada
    /// tabela da marca passa pela mesma absorcao do diario local e pelo
    /// mesmo [`Self::aplicar_tabela_bidi`] da rodada. A idempotencia sai de
    /// graca dele: o evento que ja tinha entrado antes da queda esta no
    /// diario daqui com o carimbo e a origem de la, a absorcao o poe no mapa,
    /// e reaplicar empata -- o remoto nao vence o proprio toque.
    ///
    /// # Quando
    ///
    /// No `Servidor::novo`, antes de a porta abrir e de qualquer rodada: e o
    /// que impede um leitor de ver a venda pela metade entre o arranque e a
    /// rodada seguinte, ou para sempre com o outro lado fora do ar. O `fsync`
    /// das tabelas e a saida da marca vem DEPOIS de soltar a trava, na ordem
    /// do group commit.
    ///
    /// A marca que nao se le por E/S, ou cifrada sem a chave, FICA, com o
    /// caminho no log; a que nao confere e um grupo que nunca comecou, e sai.
    pub(super) fn completar_marcas_do_bidi(&self) {
        // Pelo diretorio, e nao pela trava: a marca nasce em
        // `config.base/<database>` (`marcar_o_grupo_bidi`), e quem a grava
        // confere, com a trava, que e o do database aberto.
        let Ok(entradas) = std::fs::read_dir(&self.config.base) else {
            return;
        };
        let mut dirs: Vec<PathBuf> = entradas
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            let Some(database) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string)
            else {
                continue;
            };
            let caminhos = crate::transacao::marcas_do_bidi_em(&dir);
            for caminho in caminhos {
                let marca = match crate::transacao::ler_marca(&caminho) {
                    Ok(crate::transacao::Leitura::Aberta(m)) => m,
                    Ok(crate::transacao::Leitura::NaoConfere) => {
                        let _ = std::fs::remove_file(&caminho);
                        continue;
                    }
                    Ok(crate::transacao::Leitura::SemChave(motivo)) => {
                        eprintln!(
                            "AVISO: a marca do bidirecional {} esta PARADA sem a chave \
                             ({motivo}); ela NAO foi apagada",
                            caminho.display()
                        );
                        continue;
                    }
                    Err(e) => {
                        eprintln!(
                            "AVISO: a marca do bidirecional {} nao se leu ({e}); ela \
                             NAO foi apagada",
                            caminho.display()
                        );
                        continue;
                    }
                };
                match self.completar_grupo_bidi(&database, &marca) {
                    Ok((n, tabelas)) => {
                        let no_disco = tabelas
                            .iter()
                            .all(|t| self.sincronizar_replicada(&database, t).is_ok());
                        if no_disco {
                            let _ = std::fs::remove_file(&caminho);
                        }
                        eprintln!(
                            "bidirecional: o grupo da marca {} foi completado no arranque \
                             ({n} evento(s) entraram agora){}",
                            caminho.display(),
                            if no_disco {
                                ""
                            } else {
                                "; o fsync falhou e a marca fica"
                            }
                        );
                    }
                    Err(e) => eprintln!(
                        "AVISO: o grupo da marca do bidirecional {} nao se completou \
                         ({e}); ela NAO foi apagada",
                        caminho.display()
                    ),
                }
            }
        }
    }

    /// O corpo do [`Self::completar_marcas_do_bidi`] para UMA marca, com a
    /// trava na mao: devolve quantos eventos entraram e as tabelas tocadas.
    /// O par que parou (conflito, toque esquecido) para a tabela aqui como
    /// na rodada, e a rodada seguinte o para de novo, nominal.
    fn completar_grupo_bidi(
        &self,
        database: &str,
        marca: &crate::transacao::Marca,
    ) -> Result<(u64, Vec<String>)> {
        let meu_hash = self.config.replicacao.numero();
        let (fatias, chaves) = Self::fatias_da_marca_bidi(database, marca);
        let itens = Self::itens_das_fatias(&fatias, &chaves);
        // A origem vai resolvida na marca: nenhum evento chega com zero, e o
        // `hash_dele` nao e consultado.
        let (n, paradas) = self
            .aplicar_itens_bidi(database, None, &itens, meu_hash, 0)
            .map_err(|p| p.erro)?;
        Self::avisar_paradas_da_completacao(&chaves, &paradas);
        Ok((n, fatias.into_iter().map(|(nome, _)| nome).collect()))
    }

    /// As fatias da marca do bidi -- uma por tabela, na ordem em que o grupo
    /// as aplicou -- e a chave de cada uma no mapa de toques.
    #[allow(clippy::type_complexity, reason = "nome e eventos de cada fatia")]
    fn fatias_da_marca_bidi(
        database: &str,
        marca: &crate::transacao::Marca,
    ) -> (
        Vec<(String, Vec<crate::replica::EventoRecebido>)>,
        Vec<String>,
    ) {
        // A marca guarda as tabelas na vez das maes, contiguas: uma fatia por
        // tabela, na ordem em que o grupo as aplicou.
        let mut fatias: Vec<(String, Vec<crate::replica::EventoRecebido>)> = Vec::new();
        for op in &marca.operacoes {
            let Some(ev) = &op.replica else { continue };
            let e = crate::replica::EventoRecebido {
                operacao: ev.operacao,
                rowid: op.rowid,
                versao: 0,
                imagem: ev.imagem.clone(),
                carimbo_ms: ev.carimbo_ms,
                origem: ev.origem,
                posicao: ev.posicao,
                tx: 0,
            };
            match fatias.last_mut() {
                Some((nome, lista)) if *nome == op.tabela => lista.push(e),
                _ => fatias.push((op.tabela.clone(), vec![e])),
            }
        }
        let chaves = fatias
            .iter()
            .map(|(nome, _)| format!("{database}/{nome}"))
            .collect();
        (fatias, chaves)
    }

    /// Os itens da completacao: sem identidade, que sai do esquema e do
    /// diario local absorvido (ver [`Self::aplicar_itens_bidi`]).
    fn itens_das_fatias<'a>(
        fatias: &'a [(String, Vec<crate::replica::EventoRecebido>)],
        chaves: &'a [String],
    ) -> Vec<ItemBidi<'a>> {
        fatias
            .iter()
            .zip(chaves)
            .map(|((nome, eventos), chave_tab)| ItemBidi {
                nome,
                chave_tab,
                identidade: None,
                eventos,
            })
            .collect()
    }

    #[allow(clippy::type_complexity, reason = "como `LoteBidi`")]
    fn avisar_paradas_da_completacao(
        chaves: &[String],
        paradas: &[(usize, (u64, &'static str, String))],
    ) {
        for (k, (posicao, motivo, _)) in paradas {
            eprintln!(
                "AVISO: o grupo do bidirecional parou em {} na completacao ({motivo}, \
                 posicao {posicao} da origem); a rodada seguinte o para de novo, nominal",
                chaves[*k]
            );
        }
    }

    /// O corpo de um grupo do bidirecional com a trava na mao, UM so para a
    /// rodada ([`Self::aplicar_grupo_bidi`]) e para o arranque que completa a
    /// marca ([`Self::completar_grupo_bidi`], pedido 698). Devolve quantos
    /// eventos entraram e as paradas, pelo indice em `itens`.
    ///
    /// O item sem `identidade` e o do arranque: o processo acabou de nascer e
    /// o mapa de toques esta vazio, entao a identidade sai do esquema e o
    /// diario local entra no mapa ANTES -- senao o «mais recente vence» nao
    /// veria o que ja tinha entrado antes da queda, e a idempotencia (o evento
    /// reaplicado empata com o proprio toque) nao existiria.
    #[allow(
        clippy::type_complexity,
        reason = "a parada carrega posicao, motivo e detalhe, como `LoteBidi`"
    )]
    fn aplicar_itens_bidi(
        &self,
        database: &str,
        marca: Option<&Path>,
        itens: &[ItemBidi<'_>],
        meu_hash: u16,
        hash_dele: u16,
    ) -> std::result::Result<FeitoBidi, ParouNoGrupo> {
        let nada = |erro| ParouNoGrupo { erro, reter: false };
        let mut trava = self.travar_dados().map_err(nada)?;
        let db = trava.abrir_database(database).map_err(nada)?;
        if let Some(m) = marca {
            if m.parent() != Some(db.caminho()) {
                return Err(nada(PhxError::Corrompido(format!(
                    "a marca do grupo do bidirecional de {database} foi gravada em {} \
                     e o database esta em {}",
                    m.display(),
                    db.caminho().display()
                ))));
            }
            // EM VOO, como a do grupo da replica fiel (pedido 700): o panico no
            // meio do grupo chega ao reparo da trava sabendo que ha um grupo
            // pela metade -- e o reparo, que nao casa pela chave, derruba o
            // processo para o arranque completa-lo antes de a porta abrir.
            trava.marca_em_voo = Some(MarcaEmVoo {
                database: database.to_string(),
                caminho: m.to_path_buf(),
                gravada: true,
            });
        }
        // Pedido 722, a E7: o grupo da RODADA se ensaia inteiro antes do
        // primeiro evento -- a parada nominal (conflito de unicidade, toque
        // esquecido) e deterministica e bateria de novo em qualquer
        // completacao, entao ela tem de acontecer ANTES de algo entrar. Se
        // algum evento pararia, nenhum entra, e o par para no comeco do grupo
        // em cada tabela dele: a parada passa a ser por TRANSACAO. A
        // completacao do arranque (sem identidade) nao ensaia: ali o grupo ja
        // comecou, e so se anda para a frente.
        if marca.is_some() && itens.iter().all(|it| it.identidade.is_some()) {
            if let Some(parada) = self.ensaiar_o_grupo_bidi(&db, itens, meu_hash, hash_dele) {
                trava.marca_em_voo = None;
                return Ok((0, Self::paradas_do_grupo(itens, parada)));
            }
        }
        let (n, paradas, parou) = self.aplicar_itens_bidi_sob(&db, itens, meu_hash, hash_dele);
        let Some(erro) = parou else {
            trava.marca_em_voo = None;
            return Ok((n, paradas));
        };
        // Pedido 722: nada entrou, ou nao ha marca (a propria completacao do
        // arranque): o erro volta como sempre, e a marca da rodada sai com a
        // lista. Parte entrou: para a FRENTE, ja, com a mesma trava, pelo
        // corpo da completacao -- o mesmo aplicador, casando pela chave, e o
        // «mais recente vence» faz o que ja entrou empatar com o proprio toque.
        let Some(m) = marca.filter(|_| n > 0) else {
            trava.marca_em_voo = None;
            return Err(nada(erro));
        };
        let completado = match crate::transacao::ler_marca(m) {
            Ok(crate::transacao::Leitura::Aberta(lida)) => {
                let (fatias, chaves) = Self::fatias_da_marca_bidi(database, &lida);
                let itens = Self::itens_das_fatias(&fatias, &chaves);
                let (k, mais, parou) = self.aplicar_itens_bidi_sob(&db, &itens, meu_hash, 0);
                Self::avisar_paradas_da_completacao(&chaves, &mais);
                match parou {
                    None => Ok(k),
                    Some(e) => Err(e),
                }
            }
            Ok(_) => Err(PhxError::Corrompido(format!(
                "a marca {} voltou incompleta na releitura",
                m.display()
            ))),
            Err(e) => Err(e),
        };
        trava.marca_em_voo = None;
        match completado {
            Ok(k) => {
                eprintln!(
                    "bidirecional: {database}: o grupo parou no meio ({erro}) e foi \
                     completado pela marca, com a mesma trava (pedido 722)"
                );
                Ok((n + k, paradas))
            }
            Err(e) => Err(ParouNoGrupo {
                erro: com_nota(
                    erro,
                    &format!(
                        "o grupo do bidirecional parou no meio e nao se completou agora \
                         ({e}); a marca {} fica no disco e o arranque a completa \
                         (pedido 722)",
                        m.display()
                    ),
                ),
                reter: true,
            }),
        }
    }

    /// O ENSAIO A SECO do grupo do bidirecional -- pedido 722, a E7 do desenho
    /// unico. Responde «algum evento deste grupo PARARIA o par?» sem gravar
    /// nada: o mesmo «mais recente vence» do [`Self::aplicar_por_chave`], com
    /// os toques dos eventos anteriores do MESMO grupo por cima do mapa, e a
    /// unicidade conferida contra o disco mais o que o grupo ja teria escrito
    /// (a sobreposicao do `Table`, a mesma da pre-conferencia do COMMIT) --
    /// inclusive nos indices secundarios, que e onde o conflito costuma
    /// morar. Devolve o item e a parada `(posicao, motivo, detalhe)`.
    ///
    /// Ensaio, e nao garantia: o que ele nao alcanca (o teto de toques que
    /// esquece no meio do grupo, a linha que a gravacao completa diferente da
    /// imagem) cai no aplicador como antes, que para a tabela -- o cinto. E o
    /// erro que nao e parada nominal (E/S, imagem que nao abre) nao se decide
    /// aqui: o ensaio desiste e deixa o aplicador dize-lo, pelo `parou`.
    #[allow(clippy::type_complexity, reason = "como `LoteBidi`")]
    fn ensaiar_o_grupo_bidi(
        &self,
        db: &phxsql_store::catalogo::Database,
        itens: &[ItemBidi<'_>],
        meu_hash: u16,
        hash_dele: u16,
    ) -> Option<(usize, (u64, &'static str, String))> {
        for (k, it) in itens.iter().enumerate() {
            let (indice, pos_chave) = it.identidade?;
            match self.ensaiar_a_tabela_bidi(db, it, indice, pos_chave, meu_hash, hash_dele) {
                Ok(Some(p)) => return Some((k, p)),
                Ok(None) => {}
                // O ensaio nao sabe julgar: o aplicador fala.
                Err(_) => return None,
            }
        }
        None
    }

    /// O ensaio de UMA tabela do grupo. Ver [`Self::ensaiar_o_grupo_bidi`].
    fn ensaiar_a_tabela_bidi(
        &self,
        db: &phxsql_store::catalogo::Database,
        it: &ItemBidi<'_>,
        indice: &str,
        pos_chave: &[usize],
        meu_hash: u16,
        hash_dele: u16,
    ) -> Result<Option<(u64, &'static str, String)>> {
        let mut t = db.abrir_qualificada(it.nome)?;
        let mut locais: HashMap<String, Toque> = HashMap::new();
        let mut nascidas = 0u64;
        for e in it.eventos {
            if e.origem == meu_hash || e.imagem.is_empty() {
                continue;
            }
            let (valores, chave, antiga) =
                Self::identidades_do_evento(&mut t, e.operacao, &e.imagem, pos_chave)?;
            let origem_ev = if e.origem == 0 { hash_dele } else { e.origem };
            let agora = crate::agora_ms();
            let carimbo = if bidirecional::carimbo_alem_da_folga(e.carimbo_ms, agora) {
                agora
            } else {
                e.carimbo_ms
            };
            let (vence, vence_na_antiga) = {
                let guarda = self.toques_bidi.tomar("toques_bidi")?;
                let mapa = guarda.get(it.chave_tab);
                let decide = |c: &str| match locais.get(c) {
                    Some(l) if bidirecional::remoto_vence(carimbo, origem_ev, l) => {
                        bidirecional::Decisao::Vence
                    }
                    Some(_) => bidirecional::Decisao::Perde,
                    None => match mapa {
                        Some(m) => m.decidir(c, carimbo, origem_ev),
                        None => bidirecional::Decisao::Vence,
                    },
                };
                (decide(&chave), antiga.as_ref().map(|(_, c)| decide(c)))
            };
            if vence == bidirecional::Decisao::NaoSei
                || vence_na_antiga == Some(bidirecional::Decisao::NaoSei)
            {
                let chave_dita = Self::chave_dita(&t, pos_chave, &valores);
                return Ok(Some((
                    e.posicao,
                    "toque_esquecido_pelo_teto",
                    self.detalhe_do_esquecido(&chave_dita),
                )));
            }
            if vence != bidirecional::Decisao::Vence {
                continue;
            }
            let vence_na_antiga = vence_na_antiga == Some(bidirecional::Decisao::Vence);
            let tupla = bidirecional::tupla(&valores, pos_chave);
            match e.operacao {
                Operacao::Inclusao | Operacao::Alteracao => {
                    let achadas = t.buscar(indice, &tupla)?;
                    let achada =
                        Self::uma_linha_so(&t, it.chave_tab, indice, pos_chave, &valores, achadas)?;
                    let velha = match &antiga {
                        Some((tv, _)) if vence_na_antiga => {
                            let achadas = t.buscar(indice, tv)?;
                            Self::uma_linha_so(
                                &t,
                                it.chave_tab,
                                indice,
                                pos_chave,
                                &valores,
                                achadas,
                            )?
                        }
                        _ => None,
                    };
                    // A mesma escolha do aplicador: a linha da chave nova, ou
                    // a da chave velha que muda de chave; a velha que sobra
                    // quando as duas existem sai.
                    let proprio = achada.or(velha);
                    if let (Some(v), Some(_)) = (velha, achada) {
                        let _ = t.sobrepor_mais(v, Pendente::Exclusao);
                    }
                    if let Err(PhxError::Duplicado(qual)) = t.conferir_unicidade(&valores, proprio)
                    {
                        let alvo = AlvoBidi {
                            nome: it.nome,
                            chave_tab: it.chave_tab,
                            indice,
                            pos_chave,
                            orfa: None,
                        };
                        let c = self.contar_o_conflito(&mut t, alvo, e, &valores, &qual)?;
                        return Ok(Some((
                            e.posicao,
                            "conflito_de_unicidade",
                            Self::detalhe_do_conflito(&c),
                        )));
                    }
                    let _ = match proprio {
                        Some(r) => t.sobrepor_mais(r, Pendente::Alteracao(&valores, &[])),
                        None => {
                            nascidas += 1;
                            t.sobrepor_mais(t.slots() + nascidas, Pendente::Insercao(&valores))
                        }
                    };
                }
                Operacao::Exclusao => {
                    let achadas = t.buscar(indice, &tupla)?;
                    if let Some(r) =
                        Self::uma_linha_so(&t, it.chave_tab, indice, pos_chave, &valores, achadas)?
                    {
                        let _ = t.sobrepor_mais(r, Pendente::Exclusao);
                    }
                }
            }
            let toque = Toque {
                carimbo,
                origem: origem_ev,
                excluido: e.operacao == Operacao::Exclusao,
            };
            if let Some((_, velha)) = antiga.filter(|_| vence_na_antiga) {
                locais.insert(
                    velha,
                    Toque {
                        excluido: true,
                        ..toque
                    },
                );
            }
            locais.insert(chave, toque);
        }
        Ok(None)
    }

    /// A parada de UM evento vira a parada do grupo inteiro: cada tabela dele
    /// para na posicao do PRIMEIRO evento dela no grupo -- nenhum entrou, e a
    /// rodada seguinte pede de novo dali, depois do `replicacao_pular`.
    #[allow(clippy::type_complexity, reason = "como `LoteBidi`")]
    fn paradas_do_grupo(
        itens: &[ItemBidi<'_>],
        (culpado, (_, motivo, detalhe)): (usize, (u64, &'static str, String)),
    ) -> Vec<(usize, (u64, &'static str, String))> {
        itens
            .iter()
            .enumerate()
            .filter_map(|(k, it)| {
                let inicio = it.eventos.first()?.posicao;
                let dito = if k == culpado {
                    detalhe.clone()
                } else {
                    format!(
                        "a transacao parou em {}: {detalhe}",
                        itens[culpado].chave_tab
                    )
                };
                Some((k, (inicio, motivo, dito)))
            })
            .collect()
    }

    /// O laco de [`Self::aplicar_itens_bidi`] com a trava ja na mao. Toda
    /// quebra -- a tabela que nao abre, a absorcao, o aplicador -- cai em
    /// `parou` e o laco para ali (pedido 722): eram saidas pelo `?`, que
    /// largavam a marca da metade na lista que a rodada apaga.
    #[allow(clippy::type_complexity, reason = "como `LoteBidi`")]
    fn aplicar_itens_bidi_sob(
        &self,
        db: &phxsql_store::catalogo::Database,
        itens: &[ItemBidi<'_>],
        meu_hash: u16,
        hash_dele: u16,
    ) -> (
        u64,
        Vec<(usize, (u64, &'static str, String))>,
        Option<PhxError>,
    ) {
        let mut n = 0u64;
        let mut paradas = Vec::new();
        for (k, it) in itens.iter().enumerate() {
            let feito = (|| -> Result<LoteBidi> {
                let resolvida: (String, Vec<usize>);
                let mut orfa = None;
                let (indice, pos_chave) = match it.identidade {
                    Some(x) => x,
                    None => {
                        let mut t = db.abrir_qualificada(it.nome)?;
                        let Some(chave) = bidirecional::chave_unica(t.esquema()) else {
                            return Err(PhxError::Esquema(format!(
                                "{} perdeu a chave unica depois da queda: o grupo do \
                                 bidirecional casa por ela e nao se completa sem ela",
                                it.chave_tab
                            )));
                        };
                        self.absorver_diario_local(
                            &mut t,
                            it.chave_tab,
                            &chave.1,
                            meu_hash,
                            None,
                            true,
                        )?;
                        orfa = self.slot_sem_inclusao_no_diario(&mut t, it.chave_tab)?;
                        Self::adotar_o_id_do_grupo(&mut t, it.eventos)?;
                        resolvida = chave;
                        (resolvida.0.as_str(), resolvida.1.as_slice())
                    }
                };
                let alvo = AlvoBidi {
                    nome: it.nome,
                    chave_tab: it.chave_tab,
                    indice,
                    pos_chave,
                    orfa,
                };
                self.aplicar_tabela_bidi(db, alvo, it.eventos, meu_hash, hash_dele)
            })();
            match feito {
                Ok(feito) => {
                    n += feito.aplicados;
                    if let Some(e) = feito.erro {
                        return (n, paradas, Some(e));
                    }
                    if let Some(p) = feito.parou_em {
                        paradas.push((k, p));
                    }
                }
                Err(e) => return (n, paradas, Some(e)),
            }
        }
        (n, paradas, None)
    }

    /// O slot vivo do fim do `.reg` cuja inclusao nunca chegou ao diario daqui
    /// -- pedido 700. So depois de absorver o diario INTEIRO no mapa, que e o
    /// que o arranque faz.
    ///
    /// O rowid nasce sempre no fim, e a inclusao grava o slot antes do
    /// evento: a queda entre os dois so deixa orfa a ULTIMA linha, e ela e
    /// orfa se nenhum evento do diario a nomeia.
    fn slot_sem_inclusao_no_diario(&self, t: &mut Table, chave_tab: &str) -> Result<Option<u64>> {
        let ultimo = t.slots();
        let nomeado = self
            .toques_bidi
            .tomar("toques_bidi")?
            .get(chave_tab)
            .map_or(0, |m| m.maior_rowid);
        if ultimo == 0 || ultimo <= nomeado {
            return Ok(None);
        }
        Ok(t.ler_sem_externos(ultimo)?.map(|_| ultimo))
    }

    /// O resto do grupo leva o id de transacao que a primeira metade levou
    /// antes da queda -- pedido 701 (b), o irmao da marca da replica fiel.
    ///
    /// Aqui nao ha posicao local na marca: o evento que ja entrou se reconhece
    /// pelo carimbo e pela origem de la, que o `aplicar_por_chave` grava no
    /// diario daqui. Se o ULTIMO evento da tabela e um do grupo, o id dele e
    /// o da unidade que a queda interrompeu. Nao sendo (nada entrou, ou o
    /// carimbo foi trocado pelo relogio daqui), o resto leva um id novo -- e
    /// chega a replica encadeada como uma transacao so, que e o que faltava.
    fn adotar_o_id_do_grupo(
        t: &mut Table,
        eventos: &[crate::replica::EventoRecebido],
    ) -> Result<()> {
        let total = t.eventos()?;
        if total == 0 {
            return Ok(());
        }
        if let Some(ultimo) = t.diario(total - 1, 1)?.first() {
            if eventos
                .iter()
                .any(|e| e.carimbo_ms == ultimo.carimbo && e.origem == ultimo.origem)
            {
                phxsql_store::log::adotar_tx_na_unidade(ultimo.tx);
            }
        }
        Ok(())
    }

    /// Poe no mapa de toques os eventos locais que ele ainda nao viu.
    ///
    /// O mapa e por chave, e a chave mora na imagem -- evento sem imagem
    /// (gravado antes de o modo multi ligar) nao entra no confronto, e isso
    /// esta documentado: o bidirecional comeca a valer do momento em que as
    /// imagens comecam.
    ///
    /// # Um corpo so, para as duas fichas (pedido 330)
    ///
    /// Recebe a tabela aberta por qualquer uma das duas: a COMPARTILHADA, nas
    /// fatias de [`Self::pre_absorver_sob_leitura`], e a EXCLUSIVA, na cauda
    /// que sobra para [`Self::abrir_para_bidi_sob_a_trava`]. `prazo` corta a
    /// fatia depois do lote em que ele vence -- um lote inteiro sempre entra,
    /// senao uma fatia curta demais nunca andaria. `sob_a_exclusiva` so conta
    /// onde a absorcao aconteceu, para a prova e para o painel.
    ///
    /// Devolve quantos eventos do diario ainda faltam absorver.
    pub(super) fn absorver_diario_local<T: DiarioLegivel>(
        &self,
        tabela: &mut T,
        chave_tab: &str,
        pos_chave: &[usize],
        meu_hash: u16,
        prazo: Option<Instant>,
        sob_a_exclusiva: bool,
    ) -> Result<u64> {
        let total = tabela.eventos()?;
        let teto = self.config.replicacao.teto_de_toques;
        let mut guarda = self.toques_bidi.tomar("toques_bidi")?;
        let mapa = guarda.entry(chave_tab.to_string()).or_default();
        // O mapa e da VIDA da tabela, e nao do nome (pedido 620). Apagada e
        // recriada, ou restaurada de um backup, ela tem outro `.log` no mesmo
        // caminho: com o `vistos` velho, os eventos locais novos ate ele nunca
        // entravam no mapa, e o «mais recente vence» deixava um evento remoto
        // MAIS VELHO sobrescrever a escrita local nova. A prova de que e a
        // mesma vida e a ancora da marca, conferida no `.log` de agora; sem
        // ela, ou com o diario menor que o `vistos`, o mapa recomeca -- custa
        // uma varredura, e reconstruir do diario daqui e sempre certo.
        if mapa.vistos > 0 {
            let vistos = mapa.vistos;
            let mesma_vida = vistos <= total
                && mapa
                    .marca
                    .is_some_and(|m| m.evento == vistos && tabela.marca_do_diario_confere(&m));
            if !mesma_vida {
                mapa.recomecar_a_vida();
            }
        }
        // A marca da rodada anterior: sem ela, a tabela recem-aberta caminha
        // do comeco do volume ate `vistos` para ler um evento so -- 507-517
        // ms por rodada num diario de 1 M, com a trava na mao (ver
        // `MapaDeToques::marca`).
        if mapa.vistos < total {
            tabela.definir_marca_do_diario(mapa.marca);
        }
        // Em LOTES, com os mesmos dois tetos do `replicar`, e nao o diario
        // inteiro de uma vez: a primeira rodada do bidirecional numa tabela
        // que engordou carregava todo o diario local com imagens para a RAM
        // (irmao do A2 da revisao SEC de 17/09/2026 -- nao e entrada de
        // atacante, e crescimento). A `Table` fica aberta entre os lotes,
        // entao a marca do diario faz cada lote continuar de onde o anterior
        // parou, sem recomecar a varredura.
        while mapa.vistos < total {
            let lote = tabela.diario_com_imagem_ate(
                mapa.vistos,
                LOTE_PADRAO_DE_REPLICACAO,
                TETO_DO_LOTE_SERVIDO,
            )?;
            if lote.is_empty() {
                break;
            }
            if sob_a_exclusiva {
                mapa.absorvidos_sob_a_exclusiva += lote.len() as u64;
            }
            for (ev, imagem) in lote {
                mapa.vistos += 1;
                mapa.maior_rowid = mapa.maior_rowid.max(ev.rowid);
                if imagem.is_empty() {
                    continue;
                }
                let (_, chave, antiga) =
                    Self::identidades_do_evento(tabela, ev.operacao, &imagem, pos_chave)?;
                let toque = Toque {
                    carimbo: ev.carimbo,
                    // Escrita local guarda o hash do PROPRIO servidor, para
                    // o empate desempatar pela mesma conta nos dois lados.
                    origem: if ev.origem == 0 { meu_hash } else { ev.origem },
                    excluido: ev.operacao == Operacao::Exclusao,
                };
                // A troca de chave deixa uma LAPIDE na chave antiga: sem ela,
                // uma alteracao mais velha do outro lado naquela chave nao
                // acharia linha aqui e a ressuscitaria como linha nova.
                if let Some((_, velha)) = antiga {
                    mapa.tocar(
                        velha,
                        Toque {
                            excluido: true,
                            ..toque
                        },
                        teto,
                    );
                }
                mapa.tocar(chave, toque, teto);
            }
            // Guardada a cada lote, e nao no fim: um erro no meio da rodada
            // deixa `vistos` andado, e a marca tem de andar junto -- ela so
            // serve para posicao DEPOIS dela, e a de um lote atras ainda vale.
            mapa.marca = tabela.marca_do_diario();
            // O prazo DEPOIS do lote, e nao antes (pedido 623): conferido no
            // topo do laco, a fatia que chegava aqui com o prazo ja vencido --
            // abrir a tabela, esperar o `toques_bidi` de um `replicacao_estado`,
            // um nucleo tomado -- saia sem lote nenhum, a falta nao encurtava,
            // e a pre-absorcao entregava o resto a EXCLUSIVA. Medido sob carga
            // de `fsync` e CPU (01/10/2026): 211.060 eventos sob a exclusiva e
            // 3,3 s de escritor parado, numa fatia de 13 ms que nao andou.
            if prazo.is_some_and(|p| Instant::now() >= p) {
                break;
            }
        }
        Ok(total.saturating_sub(mapa.vistos))
    }

    /// A primeira rodada depois do arranque, fora da trava EXCLUSIVA -- pedido
    /// 330.
    ///
    /// # O que ela compra
    ///
    /// O mapa de toques e estado de processo: processo novo, `vistos = 0`, e o
    /// diario local inteiro passa pelo mapa na primeira rodada de cada tabela.
    /// Medido (`--example custo-da-absorcao-do-bidi`, release, 01/10/2026):
    /// **2,26-2,63 us por evento**, 2,3-2,5 s num diario de 1 M -- e isso
    /// acontecia sob `travar_dados()`, com o servidor parado para escrita E
    /// leitura. «Ao subir» (a promessa do pedido 325) e justamente o instante
    /// em que esse custo e pago por inteiro.
    ///
    /// Aqui a absorcao anda sob a trava de LEITURA, em fatias de
    /// [`FATIA_DA_PRE_ABSORCAO`]: leitor nao espera, e escritor espera no
    /// maximo uma fatia (mais o lote em que ela vence) -- porque, entre uma
    /// fatia e a seguinte, a absorcao CEDE a vez a quem esta na fila (pedido
    /// 623). Sem ceder, a promessa era do papel: o `RwLock` deixa o leitor em
    /// laco retomar a leitura antes de o escritor acordado ser escalado, e o
    /// escritor esperava dezenas de fatias. Termina quando falta
    /// menos de um lote -- a cauda vai sob a exclusiva, no mesmo corpo, junto
    /// do que alguem escreveu entre as fatias -- ou quando uma fatia nao
    /// encurtou a falta (quem escreve anda mais rapido que quem absorve), e
    /// ai a exclusiva leva o resto, como antes.
    ///
    /// Nada aqui decide o confronto: o mapa so recebe o que o diario daqui ja
    /// gravou, e a ordem e a do diario. A exclusiva continua sendo a unica que
    /// aplica evento remoto, e ela absorve a cauda ANTES de aplicar.
    fn pre_absorver_sob_leitura(
        &self,
        database: &str,
        nome: &str,
        chave_tab: &str,
        meu_hash: u16,
    ) -> Result<()> {
        let mut falta_antes = u64::MAX;
        // A fila de escritores no fim da fatia anterior (pedido 623).
        let mut cedida: Option<crate::retrato::Fila> = None;
        // O teto de voltas e o proprio progresso: cada volta encurta a falta
        // ou o laco sai. Nenhuma volta repete sem andar.
        loop {
            let trava = self.travar_dados_para_ler()?;
            if let Some(foto) = cedida.take() {
                let vez = self.retrato.depois_de_ceder(foto);
                if vez.entraram > 0 || vez.furou {
                    let mut guarda = self.toques_bidi.tomar("toques_bidi")?;
                    let mapa = guarda.entry(chave_tab.to_string()).or_default();
                    mapa.escritores_entre_as_fatias += vez.entraram;
                    mapa.fatias_que_furaram_a_fila += u64::from(vez.furou);
                }
            }
            // `abrir_diario_para_ler`, e nao `abrir_para_ler`: a tabela escrita
            // desde o ultimo fecho da janela tem o cabecalho do `.log` atras
            // do arquivo, e a abertura de leitura comum recusa pedindo cura --
            // medido em 01/10/2026, era a recusa de toda fatia sob carga, e a
            // absorcao inteira voltava para a exclusiva. A do diario conta com
            // a cauda na memoria, sem gravar.
            //
            // Tabela que ainda nao existe, que precisaria escrever para abrir
            // por outro motivo, ou sem chave: a exclusiva cria, cura ou recusa
            // com o motivo.
            let Ok(Aberta::Pronta(mut t)) = trava.abrir_diario_para_ler(database, nome) else {
                return Ok(());
            };
            let Some((_, pos_chave)) = bidirecional::chave_unica(t.esquema_do_diario()) else {
                return Ok(());
            };
            let prazo = Instant::now() + FATIA_DA_PRE_ABSORCAO;
            let falta = self.absorver_diario_local(
                &mut t,
                chave_tab,
                &pos_chave,
                meu_hash,
                Some(prazo),
                false,
            )?;
            // Com a leitura AINDA na mao: so assim todo escritor contado esta
            // esperando, e nenhum dentro.
            let foto = self.retrato.fila();
            drop(t);
            drop(trava);
            if falta < LOTE_PADRAO_DE_REPLICACAO || falta >= falta_antes {
                return Ok(());
            }
            falta_antes = falta;
            // Cede a vez (pedido 623): pedir a leitura de novo agora a levaria
            // antes de o escritor acordado ser escalado, e ele voltaria a
            // dormir -- ver `crate::retrato`, «O leitor que CEDE a vez».
            self.retrato.ceder(foto, TETO_DA_VEZ_CEDIDA);
            cedida = Some(foto);
        }
    }

    /// Aplica UM evento remoto pela chave, com "mais recente vence".
    ///
    /// Devolve `false` quando o toque local venceu -- que nao e erro, e o
    /// conflito fazendo o trabalho dele.
    ///
    /// `orfa` e o slot vivo sem inclusao no diario daqui (pedido 700, ver
    /// [`AlvoBidi::orfa`]): o evento cuja chave o acha COMPLETA a inclusao em
    /// vez de gravar por cima, e a orfa se gasta.
    pub(super) fn aplicar_por_chave(
        &self,
        tabela: &mut Table,
        alvo: AlvoBidi<'_>,
        e: &crate::replica::EventoRecebido,
        hash_dele: u16,
        orfa: &mut Option<u64>,
    ) -> Result<bidirecional::Aplicacao> {
        let AlvoBidi {
            chave_tab,
            indice,
            pos_chave,
            ..
        } = alvo;
        if e.imagem.is_empty() {
            return Err(PhxError::Esquema(format!(
                "evento de {} sem imagem no bidirecional: o outro lado precisa \
                 de replicacao.imagem_da_linha ligada (e de papel multi, que \
                 poe imagem tambem na exclusao)",
                e.operacao.nome()
            )));
        }
        let (valores, chave, antiga) =
            Self::identidades_do_evento(tabela, e.operacao, &e.imagem, pos_chave)?;
        // Um source antigo manda origem zero; zero aqui significaria "meu",
        // que e a leitura errada para um evento que veio DELE.
        let origem_ev = if e.origem == 0 { hash_dele } else { e.origem };
        // O carimbo com que o evento DISPUTA e com que ele entra no diario e
        // no toque daqui. Alem da folga do futuro, vale o relogio local e a
        // troca e contada -- nunca recusada, porque recusar pararia o par.
        // Ver `bidirecional::carimbo_alem_da_folga` (revisao SEC, A9).
        let carimbo = {
            let agora = crate::agora_ms();
            if bidirecional::carimbo_alem_da_folga(e.carimbo_ms, agora) {
                if let Ok(mut guarda) = self.toques_bidi.lock() {
                    let m = guarda.entry(chave_tab.to_string()).or_default();
                    m.carimbos_do_futuro += 1;
                    // Uma linha no log por tabela, e nao por evento: um par
                    // mentindo o carimbo nao pode encher o log deste lado.
                    if m.carimbos_do_futuro == 1 {
                        eprintln!(
                            "CARIMBO DO FUTURO em {chave_tab}: evento com carimbo {} \
                             mais de {} ms a frente do relogio local ({agora}); entrou \
                             com o relogio daqui. Confira o NTP do outro lado -- e se \
                             o NTP esta certo, o outro lado mente",
                            e.carimbo_ms,
                            bidirecional::FOLGA_DO_CARIMBO_MS
                        );
                    }
                }
                agora
            } else {
                e.carimbo_ms
            }
        };

        // A colisao e detectada ANTES de decidir quem vence, e de proposito: os
        // dois lados perdem uma linha, cada um no seu `.reg`, e a deteccao tem
        // de disparar dos dois lados -- inclusive quando o LOCAL vence e o
        // evento remoto e descartado (a linha remota morre no outro `.reg`).
        // Ver `bidirecional::colisao_de_criacao`.
        let (vence, colisao, vence_na_antiga) = {
            let guarda = self.toques_bidi.tomar("toques_bidi")?;
            let mapa = guarda.get(chave_tab);
            // Pedido 330 (b): a decisao e do MAPA, e nao de um `get` aqui --
            // so ele sabe se a chave ausente nunca foi tocada ou foi
            // esquecida pelo teto, e so ele sabe o piso.
            let decide = |c: &str| match mapa {
                Some(m) => m.decidir(c, carimbo, origem_ev),
                None => bidirecional::Decisao::Vence,
            };
            let local = mapa.and_then(|m| m.toque(&chave));
            let colisao = bidirecional::colisao_de_criacao(e.operacao, origem_ev, local.as_ref());
            // A chave ANTIGA disputa pelo mesmo «mais recente vence»: uma
            // escrita daqui na chave velha, mais nova que a troca de la, nao
            // pode ser apagada por ela.
            let na_antiga = antiga.as_ref().map(|(_, c)| decide(c));
            (decide(&chave), colisao, na_antiga)
        };
        // A chave esquecida sem resposta PARA o par -- inclusive a antiga da
        // troca de chave, que decide se a linha velha sai. Ver
        // `bidirecional::Decisao::NaoSei`.
        if vence == bidirecional::Decisao::NaoSei
            || vence_na_antiga == Some(bidirecional::Decisao::NaoSei)
        {
            return Ok(bidirecional::Aplicacao::Esquecida(Self::chave_dita(
                tabela, pos_chave, &valores,
            )));
        }
        let vence = vence == bidirecional::Decisao::Vence;
        let vence_na_antiga = vence_na_antiga == Some(bidirecional::Decisao::Vence);
        if colisao {
            // Nao para o laco (isso travaria o par para sempre -- ver
            // `Table::inserir_replicado`); torna o defeito VISIVEL. O contador
            // vai para `replicacao_estado`, e a linha vai ao log do processo.
            if let Ok(mut guarda) = self.toques_bidi.lock() {
                guarda.entry(chave_tab.to_string()).or_default().colisoes += 1;
            }
            // A chave sai REDIGIDA aqui pelo mesmo motivo que no grito do
            // conflito logo abaixo: este e o IRMAO -- as duas linhas imprimem
            // a mesma `chave` canonica, e uma primaria de CPF marcada como
            // dado pessoal vazaria por qualquer uma das duas. Consertar so a
            // de baixo deixaria a porta aberta nesta.
            eprintln!(
                "COLISAO DE SEQUENCE em {chave_tab}: chave {} criada em dois nos \
                 da MESMA faixa; \"mais recente vence\" apaga uma linha. Declare faixas \
                 disjuntas (inicio/passo) -- ver docs/AUTONUMBER.md, defeito (a)",
                Self::chave_dita(tabela, pos_chave, &valores)
            );
        }
        if !vence {
            return Ok(bidirecional::Aplicacao::Ignorado);
        }

        let tupla = bidirecional::tupla(&valores, pos_chave);
        // A escrita vai num bloco proprio porque a recusa por chave duplicada
        // NAO pode subir pelo `?`: ela e tratada logo abaixo. Ver o comentario
        // da recusa, e `bidirecional::MapaDeToques::recusas_por_unicidade`.
        let escrita = (|| -> Result<()> {
            match e.operacao {
                Operacao::Inclusao | Operacao::Alteracao => {
                    let achadas = tabela.buscar(indice, &tupla)?;
                    let achada = Self::uma_linha_so(
                        tabela, chave_tab, indice, pos_chave, &valores, achadas,
                    )?;
                    // Pedido 700: a chave achou a linha cuja inclusao a queda
                    // deixou fora do diario -- e a deste evento, que estava
                    // sendo incluida quando o processo morreu (o arranque so
                    // chega aqui pelo grupo da marca, e a chave sem toque e a
                    // que o diario nunca viu). Gravar por cima mandaria a
                    // replica encadeada a ALTERACAO de uma linha que ela nunca
                    // viu nascer; a pergunta e a do `slot_ja_consumido` da
                    // replica fiel, e a resposta tambem: completa so o evento,
                    // do payload que esta no disco.
                    if achada.is_some() && achada == *orfa {
                        let rowid = achada.unwrap_or_default();
                        tabela.forcar_proximo_evento(carimbo, origem_ev);
                        tabela.completar_o_diario_da_inclusao(rowid, &[])?;
                        *orfa = None;
                        return Ok(());
                    }
                    // A linha da chave ANTIGA, quando a alteracao trocou a
                    // chave e a troca venceu la -- ver `anexar_o_antes` no
                    // store. Sem isto, buscar so pela chave nova nao achava
                    // nada, inseria, e a linha antiga FICAVA: uma alteracao
                    // virava duas linhas deste lado.
                    let velha = match &antiga {
                        Some((t, _)) if vence_na_antiga => {
                            let achadas = tabela.buscar(indice, t)?;
                            Self::uma_linha_so(
                                tabela, chave_tab, indice, pos_chave, &valores, achadas,
                            )?
                        }
                        _ => None,
                    };
                    // O evento local nasce com o carimbo e a origem do
                    // NASCIMENTO da escrita -- e o que faz o conflito ser justo
                    // e o evento nao voltar para de onde veio.
                    tabela.forcar_proximo_evento(carimbo, origem_ev);
                    if let (None, Some(rowid)) = (achada, velha) {
                        // A troca de chave chega como ALTERACAO da linha que
                        // ja estava aqui: o rowid e o rownum daqui ficam, que e
                        // a ordem de digitacao deste servidor. Excluir e
                        // inserir de novo poria a linha no fim da fila.
                        tabela.atualizar_replicado(rowid, &valores)?;
                        return Ok(());
                    }
                    if let Some(rowid) = velha {
                        // As duas chaves ocupadas aqui: a nova recebe a linha
                        // logo abaixo, e a antiga sai -- e a exclusao da
                        // antiga que o SQLite registra, chegando pelo mesmo
                        // motor que a exclusao replicada usa.
                        tabela.excluir_de_vez_replicado(
                            rowid,
                            "replicacao bidirecional: troca de chave",
                        )?;
                        tabela.forcar_proximo_evento(carimbo, origem_ev);
                    }
                    match achada {
                        // O rowid e o rownum sao LOCAIS: `atualizar` mantem os
                        // daqui, `inserir` numera na ordem de chegada daqui. A
                        // ordem de digitacao de cada servidor e sagrada NELE.
                        // Sem julgar: o outro lado ja aceitou esta escrita, e a
                        // replicacao anda por TABELA -- a filha chega antes da
                        // mae. Conferindo aqui, o erro subia pelo `?` do laco, a
                        // posicao nunca andava, e o mesmo lote voltava para
                        // sempre: o par de servidores PARADO. Ver
                        // `Table::inserir_replicado`.
                        Some(rowid) => tabela.atualizar_replicado(rowid, &valores)?,
                        None => {
                            tabela.inserir_replicado(&valores)?;
                        }
                    }
                }
                Operacao::Exclusao => {
                    let achadas = tabela.buscar(indice, &tupla)?;
                    let achada = Self::uma_linha_so(
                        tabela, chave_tab, indice, pos_chave, &valores, achadas,
                    )?;
                    // Sem linha nao ha o que excluir: ela ja saiu daqui, ou
                    // nunca chegou. O toque gravado abaixo vira a lapide em
                    // memoria que impede uma alteracao MAIS VELHA de
                    // ressuscita-la.
                    if let Some(rowid) = achada {
                        tabela.forcar_proximo_evento(carimbo, origem_ev);
                        tabela.excluir_de_vez_replicado(rowid, "replicacao bidirecional")?;
                    }
                }
            }
            Ok(())
        })();
        // Chave duplicada num indice unico NAO sobe pelo `?` (pedido 292): ela
        // PARA o par naquela tabela, marcada, contada e gritada com o que o
        // PostgreSQL grita -- indice, valor da chave, a linha daqui e a de la.
        //
        // O casamento entre servidores usa UMA chave, e a unicidade dos outros
        // indices continua valendo na gravacao -- entao o evento que colide
        // com a linha de OUTRA chave e recusado aqui. Subindo pelo `?`, `desde`
        // nunca andava e o MESMO lote voltava para sempre, calado. Agora quem
        // chama recebe o conflito ANALISADO e decide: marca a tabela como
        // parada e nao anda a posicao, que e a decisao do `worker.c` do
        // PostgreSQL (nao avancar a origem e o que impede perder o evento). O
        // caminho de volta e humano: `replicacao_pular`.
        //
        // O erro que NAO for duplicidade continua subindo: disco, imagem
        // corrompida e trava envenenada continuam parando a rodada, que e o
        // comportamento de sempre.
        if let Err(PhxError::Duplicado(qual)) = &escrita {
            let conflito = self.contar_o_conflito(tabela, alvo, e, &valores, qual)?;
            return Ok(bidirecional::Aplicacao::Conflito(Box::new(conflito)));
        }
        escrita?;

        if let Ok(mut guarda) = self.toques_bidi.lock() {
            let teto = self.config.replicacao.teto_de_toques;
            let mapa = guarda.entry(chave_tab.to_string()).or_default();
            let toque = Toque {
                carimbo,
                origem: origem_ev,
                excluido: e.operacao == Operacao::Exclusao,
            };
            // A chave antiga vira LAPIDE, pelo mesmo motivo do
            // `absorver_diario_local`: uma alteracao mais velha nela nao pode
            // ressuscitar a linha que a troca levou embora.
            if let Some((_, velha)) = antiga.filter(|_| vence_na_antiga) {
                mapa.tocar(
                    velha,
                    Toque {
                        excluido: true,
                        ..toque
                    },
                    teto,
                );
            }
            mapa.tocar(chave, toque, teto);
        }
        Ok(bidirecional::Aplicacao::Aplicado)
    }

    /// A linha que a chave do casamento achou -- uma ou nenhuma, nunca «a
    /// primeira de varias».
    ///
    /// Cinto e suspensorio do pedido 517: a chave e unica e obrigatoria
    /// (`bidirecional::chave_unica`), entao a busca acha uma linha ou nenhuma.
    /// Se achar duas, a identidade nao identifica -- e escolher a primeira
    /// apagaria ou sobrescreveria a de outro servidor, que era o `first()` de
    /// antes. Para, nomeando, em vez de escolher.
    fn uma_linha_so(
        tabela: &Table,
        chave_tab: &str,
        indice: &str,
        pos_chave: &[usize],
        valores: &[Value],
        achadas: Vec<u64>,
    ) -> Result<Option<u64>> {
        if achadas.len() > 1 {
            return Err(PhxError::Corrompido(format!(
                "a chave {} do indice {indice} casa {} linhas em {chave_tab}: o \
                 bidirecional nao escolhe uma por ordem, porque a escolhida seria a de \
                 outro servidor -- ver docs/REPLICACAO.md, pedido 517",
                Self::chave_dita(tabela, pos_chave, valores),
                achadas.len()
            )));
        }
        Ok(achadas.first().copied())
    }

    /// A chave do casamento pronta para IR A LOG -- redigida pelo esquema.
    ///
    /// A `chave` canonica que o mapa de toques usa e o dado cru, e serve para
    /// casar linha; para dizer, ela passa por `valor_redigido`. So se monta
    /// quando ha grito: no caminho sa ninguem paga nada.
    fn chave_dita(tabela: &Table, pos_chave: &[usize], valores: &[Value]) -> String {
        // A composta sai coluna a coluna, cada uma pelo `valor_redigido`: a
        // parte marcada como dado pessoal vira tamanho mesmo no meio da tupla.
        pos_chave
            .iter()
            .map(
                |p| match (tabela.esquema().colunas().get(*p), valores.get(*p)) {
                    (Some(c), Some(v)) => bidirecional::valor_redigido(c, v),
                    _ => String::new(),
                },
            )
            .collect::<Vec<_>>()
            .join("|")
    }

    /// O que um evento do bidirecional diz de IDENTIDADE: os valores da linha,
    /// a chave canonica dela e -- quando a alteracao trocou a chave -- a tupla
    /// e a chave de ANTES, lidas do rabo da imagem (`anexar_o_antes`).
    ///
    /// UM motor para quem aplica o evento remoto e para quem absorve o diario
    /// local: as duas pontas fazem a mesma pergunta, e duas leituras do rabo
    /// divergiriam no dia em que uma aprendesse um caso.
    #[allow(clippy::type_complexity)]
    fn identidades_do_evento<T: DiarioLegivel>(
        tabela: &mut T,
        operacao: Operacao,
        imagem: &[u8],
        pos_chave: &[usize],
    ) -> Result<(Vec<Value>, String, Option<(Vec<Value>, String)>)> {
        let valores = tabela.valores_da_imagem(imagem)?;
        let chave = bidirecional::chave_da_tupla(&valores, pos_chave);
        let antiga = if operacao == Operacao::Alteracao {
            match tabela.valores_antes_da_imagem(imagem)? {
                Some(antes) => {
                    let tupla = bidirecional::tupla(&antes, pos_chave);
                    let velha = crate::dblink::sincronia::chave_canonica_da_tupla(&tupla);
                    // O rabo vem quando QUALQUER unico mudou; se a identidade
                    // nao mudou, nao ha troca de chave nenhuma a fazer.
                    (velha != chave).then_some((tupla, velha))
                }
                None => None,
            }
        } else {
            None
        };
        Ok((valores, chave, antiga))
    }

    /// Descobre QUAL indice unico recusou o evento, e com que linha.
    ///
    /// # Analisa, nao recorta
    ///
    /// `PhxError::Duplicado` carrega o nome do indice dentro de uma frase
    /// («indice unico porEmail ja tem essa chave»), e recortar a frase quebra
    /// calado no dia em que alguem melhorar a redacao -- texto se resolve por
    /// chave, nunca por comparacao de frase. Entao aqui as chaves unicas sao
    /// percorridas de novo contra o esquema, que e exatamente a conta que a
    /// gravacao ja fez para recusar: a diferenca e que agora ela serve para
    /// DIZER. A frase do erro entra so como ultimo recurso, quando a analise
    /// nao acha indice nenhum (uma corrida, ou um indice parcial cuja
    /// condicao mudou entre a recusa e esta releitura) -- e ai ela vai no
    /// campo `valor`, dita como o que e, em vez de virar um nome inventado.
    ///
    /// # Custo
    ///
    /// Zero no caminho sa: so roda depois de um `Err(Duplicado)`, que agora
    /// para o par -- ou seja, uma vez por parada, e nao uma vez por evento.
    /// O conflito de unicidade CONTADO e gritado -- um corpo so para o
    /// aplicador e o ensaio do grupo (pedido 722): o contador, a analise
    /// (indice, valor redigido, as duas linhas) e a linha do log.
    fn contar_o_conflito(
        &self,
        tabela: &mut Table,
        alvo: AlvoBidi<'_>,
        e: &crate::replica::EventoRecebido,
        valores: &[Value],
        qual: &str,
    ) -> Result<bidirecional::Conflito> {
        let chave_tab = alvo.chave_tab;
        if let Ok(mut guarda) = self.toques_bidi.lock() {
            guarda
                .entry(chave_tab.to_string())
                .or_default()
                .recusas_por_unicidade += 1;
        }
        let conflito =
            self.analisar_conflito(tabela, alvo.indice, alvo.pos_chave, valores, qual)?;
        let chave_dita = Self::chave_dita(tabela, alvo.pos_chave, valores);
        eprintln!(
            "CONFLITO DE UNICIDADE em {chave_tab}: o evento de {} da chave \
             {chave_dita} colide com uma linha daqui. indice={:?} valor={:?} \
             linha daqui={} \
             linha de la={}. A replicacao DESTA tabela neste par PAROU na posicao \
             {}; solte-a com replicacao_pular depois de resolver -- ver \
             docs/REPLICACAO.md §21 e docs/PENDENCIAS.md, pedido 292",
            e.operacao.nome(),
            conflito.indice,
            conflito.valor,
            conflito.linha_daqui,
            conflito.linha_de_la,
            e.posicao
        );
        Ok(conflito)
    }

    /// O detalhe da parada por conflito, o mesmo no aplicador e no ensaio.
    fn detalhe_do_conflito(c: &bidirecional::Conflito) -> String {
        format!(
            "indice {:?}, valor {:?}; a linha daqui e {}, a de la e {}",
            c.indice, c.valor, c.linha_daqui, c.linha_de_la
        )
    }

    /// O detalhe da parada pelo toque esquecido, o mesmo no aplicador e no
    /// ensaio.
    fn detalhe_do_esquecido(&self, chave: &str) -> String {
        format!(
            "chave {chave}: o mapa de toques passou do teto \
             (replicacao.teto_de_toques = {}) e esqueceu o ultimo toque \
             local dela, e o evento nao e mais recente que o mais novo \
             esquecido -- aplicar ou descartar seria escolher um lado as \
             cegas. Suba o teto e reinicie (o mapa se refaz do diario), \
             ou solte o par sabendo que este evento sera pulado",
            self.config.replicacao.teto_de_toques
        )
    }

    fn analisar_conflito(
        &self,
        tabela: &mut Table,
        indice: &str,
        pos_chave: &[usize],
        valores: &[Value],
        qual: &str,
    ) -> Result<bidirecional::Conflito> {
        let mut achado = bidirecional::Conflito {
            linha_de_la: bidirecional::linha_redigida(tabela.esquema(), valores),
            ..Default::default()
        };
        // A linha que o evento ia ESCREVER, pela chave do casamento. Ela nao
        // e conflito com ela mesma: numa alteracao o indice do casamento acha
        // justamente o alvo, e conta-lo como culpado diria o indice errado a
        // quem opera.
        let alvo = tabela
            .buscar(indice, &bidirecional::tupla(valores, pos_chave))
            .ok()
            .and_then(|r| r.first().copied());
        let unicos: Vec<(String, Vec<usize>)> = tabela
            .esquema()
            .indices()
            .iter()
            .filter(|i| i.unico)
            .map(|i| (i.nome.clone(), i.colunas.iter().map(|c| c.coluna).collect()))
            .collect();
        for (nome, colunas) in unicos {
            let Some(chave): Option<Vec<Value>> = colunas
                .iter()
                .map(|c| valores.get(*c).cloned())
                .collect::<Option<Vec<Value>>>()
            else {
                continue;
            };
            // Chave com nulo nao participa de unicidade -- o indice nao a
            // guarda, entao ela nunca pode ter sido a que recusou.
            if chave.iter().any(Value::e_null) {
                continue;
            }
            let Ok(rowids) = tabela.buscar(&nome, &chave) else {
                continue;
            };
            let Some(rowid) = rowids.iter().copied().find(|r| Some(*r) != alvo) else {
                continue;
            };
            achado.indice = nome;
            // A chave sai REDIGIDA coluna a coluna, e nao pelo `para_texto`
            // cru: uma primaria de CPF marcada como dado pessoal iria ao log
            // do processo se copiassemos o `Key (c)=(1)` do PostgreSQL.
            achado.valor = colunas
                .iter()
                .zip(chave.iter())
                .map(|(c, v)| match tabela.esquema().colunas().get(*c) {
                    Some(col) => bidirecional::valor_redigido(col, v),
                    None => String::new(),
                })
                .collect::<Vec<_>>()
                .join("|");
            if let Ok(Some(linha)) = tabela.ler(rowid) {
                achado.linha_daqui = bidirecional::linha_redigida(tabela.esquema(), &linha);
            }
            return Ok(achado);
        }
        // A analise nao achou: diz isso, com a frase do motor ao lado.
        achado.valor = format!("(nao analisado; o motor disse: {qual})");
        Ok(achado)
    }
}
