//! A conferencia da chave estrangeira, e o achado que so a MEDICAO trouxe.
//!
//! Os testes de uma tabela so passavam todos. O medidor `custo-da-fk` caiu na
//! primeira rodada -- e o motivo nao era do medidor: a conferencia abre a MAE
//! num SEGUNDO descritor, e um segundo descritor sobre tabela com escrita
//! pendente le um indice que ainda nao foi para o disco. O store recusa, e
//! recusa certo: ler seria pior.
//!
//! Isso alcanca o servidor, e nao so o medidor. O caminho do `commit` mantem
//! VARIAS tabelas abertas ao mesmo tempo (o mapa `abertas`), entao mae e filha
//! na mesma transacao caem exatamente aqui.
//!
//! *Interface -- e garantia -- so se prova exercitando.*

mod comum;
use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn dir(nome: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("fk-{nome}"))
}

fn mae(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn filha(d: &std::path::Path, conferindo: bool) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            // O indice da COLUNA DA CHAVE, e nao so o da primaria. A chave
            // conferida precisa de indice dos DOIS lados, e por motivos
            // diferentes: na mae para responder «existe este pai?» ao gravar a
            // filha, e na FILHA para responder «alguem aponta para esta
            // linha?» ao apagar a mae. Sem este, cada exclusao de mae varreria
            // a tabela de filhas inteira -- e o motor recusa em vez de esconder
            // esse custo dentro de um `excluir` que parece barato.
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(conferindo)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A mae ABERTA e ja gravada e vista normalmente.
///
/// Este e o caso comum -- a mae ja existe quando a filha entra -- e ele tinha
/// de ficar num teste proprio, porque e ele que impede o teste seguinte de
/// "passar" declarando que a conferencia nunca funciona.
#[test]
fn a_mae_aberta_e_ja_gravada_e_vista() {
    let d = dir("mae-aberta");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    // Continua ABERTA de proposito: o que impede a leitura nao e o descritor,
    // e a escrita pendente.
    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)])
        .expect("a mae gravada e aberta nao foi vista");
}

/// O LIMITE, medido e dito com todas as letras: a conferencia le o que ja foi
/// gravado.
///
/// A mae inserida e NAO sincronizada nao esta visivel -- e isso alcanca o
/// servidor, porque o `commit` mantem varias tabelas abertas ao mesmo tempo:
/// mae e filha na mesma transacao caem aqui. **E o mesmo buraco do
/// read-your-own-writes** (SP000006 do roteiro), e nao um defeito a parte:
/// quem nao enxerga a propria escrita tambem nao enxerga a mae que acabou de
/// inserir.
///
/// O que este teste TRAVA nao e a limitacao -- e a QUALIDADE do recado. Sozinho,
/// o erro cru dizia "indice corrompido: reconstrua", mandando o leitor reparar
/// um arquivo sao.
///
/// A causa CONTINUA na mensagem, e de proposito: e ela que diz qual arquivo e
/// qual guarda recusou. Por isso o teste afirma que a explicacao vem DEPOIS da
/// causa, e nao que a causa sumiu -- jogar a causa fora trocaria um recado ruim
/// por um recado cego.
///
/// **O que mudou em 03/09/2026, e por que a intencao deste teste sobreviveu
/// inteira:** a causa era o erro CRU embrulhado em parenteses, e ele carregava
/// o imperativo «reconstrua com `reparar indice`» -- mandando reparar um
/// arquivo intacto, o que era a primeira metade do recado contradizendo a
/// segunda. Hoje a mensagem nomeia o `.ndx` a partir do DADO (`diretorio` +
/// `nome`, nunca recortado do texto do erro) e diz que o arquivo esta sao.
/// Entao a marca da causa neste teste deixou de ser a palavra «reconstrua» e
/// passou a ser o proprio caminho do indice: o teste afirma a MESMA coisa, com
/// a agulha que a mensagem nova oferece.
#[test]
fn a_mae_nao_gravada_recusa_dizendo_por_que() {
    let d = dir("mae-pendente");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    // SEM sincronizar, de proposito.
    let mut f = filha(&d, true);
    let e = f
        .inserir(&[Value::Int(10), Value::Int(1)])
        .expect_err("a mae pendente foi vista -- o limite caducou, atualize o docs");
    let txt = e.to_string();
    assert!(
        txt.contains("mesma transacao") || txt.contains("ja foi gravado"),
        "o recado nao explica o limite: {txt}"
    );
    let (i_causa, i_expl) = (
        txt.find(".ndx").unwrap_or(usize::MAX),
        txt.find("ja foi gravado").unwrap_or(0),
    );
    assert!(
        i_expl > i_causa,
        "a causa ficou por ultimo e vira a ultima palavra do recado: {txt}"
    );
    assert!(
        !txt.contains("reconstrua") && !txt.contains("reparar indice"),
        "voltou a mandar reparar um arquivo sao: {txt}"
    );
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "familia errada: {txt}"
    );
}

/// Com a chave DESLIGADA nada disso acontece -- o caminho antigo continua
/// intacto, inclusive com a mae aberta.
#[test]
fn sem_conferir_a_mae_aberta_nao_muda_nada() {
    let d = dir("mae-aberta-off");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    let mut f = filha(&d, false);
    f.inserir(&[Value::Int(10), Value::Int(999)])
        .expect("o caminho sem conferencia mudou");
}

// ---------------------------------------------------------------------------
// A REGRA PRIMORDIAL: nunca se mata o pai que tem filhos
// ---------------------------------------------------------------------------

/// O coracao da regra: a mae com filha NAO sai.
///
/// Prova real: tirar a chamada de `conferir_filhas` do `excluir_de_vez` faz
/// este teste passar a apagar -- e e exatamente o que ele existe para impedir.
#[test]
fn a_mae_com_filha_nao_pode_ser_apagada() {
    let d = dir("mae-com-filha");
    let mut m = mae(&d);
    let rowid = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    let e = m
        .excluir_de_vez(rowid, "tentando")
        .expect_err("a mae com filha foi apagada -- a regra primordial caiu");
    let txt = e.to_string();
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "familia errada: {txt}"
    );
    assert!(txt.contains("pedidos"), "nao diz ONDE esta a filha: {txt}");
    assert!(
        txt.contains("apague as filhas antes"),
        "nao diz o que fazer: {txt}"
    );
}

/// A outra metade, sem a qual a de cima passaria com um portao que recusa TODA
/// exclusao -- e um portao assim tornaria o banco inutil.
#[test]
fn a_mae_sem_filha_sai_normalmente() {
    let d = dir("mae-sem-filha");
    let mut m = mae(&d);
    let com = m.inserir(&[Value::Int(1)]).unwrap();
    let sem = m.inserir(&[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    // A linha 2 nao tem filha, e sai.
    assert!(
        m.excluir_de_vez(sem, "sem filha").unwrap(),
        "a linha sem filha foi barrada"
    );
    // A 1 tem, e nao sai -- as duas afirmacoes na MESMA tabela, senao a de
    // cima poderia estar passando por a tabela inteira estar trancada.
    assert!(m.excluir_de_vez(com, "com filha").is_err());
}

/// A filha que aponta para OUTRA mae nao tranca esta linha.
///
/// Sem este teste, um portao que respondesse "ha alguma filha nesta tabela?"
/// -- em vez de "ha filha DESTA linha?" -- passaria nos dois de cima.
#[test]
fn filha_de_outra_linha_nao_tranca_esta() {
    let d = dir("filha-de-outra");
    let mut m = mae(&d);
    let um = m.inserir(&[Value::Int(1)]).unwrap();
    m.inserir(&[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(2)]).unwrap();
    f.sincronizar().unwrap();

    assert!(
        m.excluir_de_vez(um, "so a 2 tem filha").unwrap(),
        "a linha 1 foi barrada por uma filha que aponta para a 2"
    );
}

/// Chave que NAO pediu conferencia nao tranca ninguem.
///
/// E o teste do comportamento VELHO, o que mais importa numa guarda nova:
/// quem nunca pediu a garantia continua apagando como sempre apagou.
#[test]
fn sem_conferir_a_mae_com_filha_sai_como_sempre() {
    let d = dir("mae-sem-conferir");
    let mut m = mae(&d);
    let rowid = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d, false);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    assert!(
        m.excluir_de_vez(rowid, "sem conferencia").unwrap(),
        "a guarda nova quebrou quem nunca pediu nada"
    );
}

// ---------------------------------------------------------------------------
// A METADE QUE FALTAVA: indice na FILHA (achado do QA-PDCA)
//
// Os nove testes acima usam todos `filha()`, que SEMPRE cria o indice
// "porCliente" -- nenhum deles passa pelo `else` de `indice_que_cobre` em
// `conferir_filhas` (`table.rs`). A regra petrea diz "sem um deles o motor
// recusa dizendo qual falta"; ate aqui so a metade "sem indice na mae"
// (`sem_indice_na_mae_a_recusa_diz_qual_indice_falta`, em
// `phxsql-server/src/servidor.rs`) tinha prova. Esta filha SEM indice cobre a
// outra metade.
// ---------------------------------------------------------------------------

/// A filha SEM o indice da coluna da chave -- so o `porId` da chave primaria.
/// Existe so para os dois testes abaixo; os outros continuam usando `filha()`.
fn filha_sem_indice(d: &std::path::Path, conferindo: bool) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            // SEM o "porCliente" (indice da coluna 1) -- e o ponto do teste.
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(conferindo)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A outra metade da regra: sem indice na FILHA, a exclusao da mae RECUSA
/// dizendo qual indice falta -- em vez de varrer a tabela de filhas inteira a
/// cada exclusao, o custo que o comentario de `conferir_filhas` recusa
/// esconder dentro de uma operacao que parece barata.
///
/// Nao precisa de linha nenhuma em `pedidos`: `indice_que_cobre` olha o
/// ESQUEMA da filha, nao os dados -- a recusa acontece antes de haver
/// qualquer filha de verdade para procurar.
///
/// Prova real: troque o `let Some(indice) = indice_que_cobre(...) else { ...
/// }` de `conferir_filhas` por um indice fixo (`"porId"`, que existe e nao
/// cobre `cliente_id`) e este teste vira "a mae saiu -- recusou a exclusao
/// sem checar filha nenhuma", que e o buraco que ele existe para fechar.
#[test]
fn sem_indice_na_filha_a_recusa_diz_qual_indice_falta() {
    let d = dir("filha-sem-ndx");
    let mut m = mae(&d);
    let rowid = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha_sem_indice(&d, true);
    f.sincronizar().unwrap();

    let e = m
        .excluir_de_vez(rowid, "sem indice na filha")
        .expect_err("a exclusao passou sem indice na filha -- ia varrer a tabela inteira");
    let txt = e.to_string();
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "familia errada: {txt}"
    );
    assert!(txt.contains("indice"), "o erro nao diz o que falta: {txt}");
    assert!(
        txt.contains("pedidos"),
        "o erro nao diz ONDE falta o indice: {txt}"
    );
    assert!(
        txt.contains("crie o indice na filha"),
        "o erro nao diz o que fazer: {txt}"
    );
}

/// Comportamento VELHO: sem `verificar`, a mae sai mesmo sem indice na
/// filha -- quem nunca pediu a garantia nao paga o portao novo. E o par que
/// impede o teste de cima de estar escondendo um portao que recusa toda
/// exclusao, indice ou nao.
#[test]
fn sem_conferir_a_mae_sai_mesmo_sem_indice_na_filha() {
    let d = dir("filha-sem-ndx-off");
    let mut m = mae(&d);
    let rowid = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    let mut f = filha_sem_indice(&d, false);
    f.sincronizar().unwrap();

    assert!(
        m.excluir_de_vez(rowid, "sem indice, sem conferencia")
            .unwrap(),
        "a guarda nova quebrou quem nunca pediu nada"
    );
}

// ---------------------------------------------------------------------------
// EXISTIR nao e ESTAR VIVA: a orfa por construcao
// ---------------------------------------------------------------------------
//
// A conferencia perguntava «esta linha existe?», e a mae excluida de forma
// SUAVE continua no `.reg` com a chave dela no indice -- entao um pedido novo
// nascia apontando para um cliente que a tela nao mostra mais.
//
// E o outro lado do tempo da mesma petrea: o `excluir_suave` da mae ja confere
// as filhas «porque pai logicamente morto deixa filha apontando para linha que
// a tela nao mostra mais». Sem esta metade a casa fechava a porta e deixava a
// janela -- nao dava para MATAR a mae com filha, mas dava para NASCER filha de
// mae morta.

/// A filha NAO nasce apontando para mae excluida de forma suave.
#[test]
fn a_filha_nao_nasce_apontando_para_mae_excluida_suave() {
    let d = dir("mae-morta-inserir");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.excluir_suave(1, "saiu").unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = filha(&d, true);
    let erro = f
        .inserir(&[Value::Int(10), Value::Int(1)])
        .expect_err("a mae esta morta: a filha nao pode nascer apontando para ela");
    let t = erro.to_string();
    assert!(
        matches!(erro, PhxError::Integridade(_)),
        "tinha de ser erro de integridade, e veio: {erro:?}"
    );
    assert!(
        t.contains("EXCLUIDA"),
        "a recusa tem de dizer que a mae existe mas esta excluida, e veio: {t}"
    );
    assert!(
        t.contains("restaure"),
        "a recusa tem de dizer o que fazer, e veio: {t}"
    );
}

/// E o `atualizar` pela mesma porta: apontar uma filha VIVA para uma mae morta
/// e o mesmo estrago com mais cliques.
#[test]
fn o_atualizar_tambem_nao_aponta_a_filha_para_mae_morta() {
    let d = dir("mae-morta-atualizar");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.inserir(&[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();

    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    // A mae 2 morre depois -- e ela nao tem filha, entao o suave passa.
    m.excluir_suave(2, "saiu").unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    let erro = f
        .atualizar(1, &[Value::Int(10), Value::Int(2)])
        .expect_err("apontar para a mae morta tem de ser recusado tambem no atualizar");
    assert!(erro.to_string().contains("EXCLUIDA"), "veio: {erro}");
}

/// A recusa DIZ o conserto, e o conserto funciona: mae restaurada, filha entra.
///
/// Sem este teste o par acima passaria com um portao que recusasse tudo.
#[test]
fn mae_restaurada_volta_a_aceitar_filha() {
    let d = dir("mae-restaurada");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.excluir_suave(1, "saiu").unwrap();
    m.sincronizar().unwrap();

    let mut f = filha(&d, true);
    assert!(f.inserir(&[Value::Int(10), Value::Int(1)]).is_err());
    drop(f);

    m.restaurar(1, "voltou").unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = Table::abrir(&d, "pedidos").unwrap();
    f.inserir(&[Value::Int(10), Value::Int(1)])
        .expect("com a mae de volta, a filha entra");
}

/// O comportamento VELHO nao muda: quem nao pediu conferencia continua gravando.
///
/// E o teste que mais importa numa guarda nova. Chave sem `verificar` fica de
/// fora do portao inteiro, e ficar viva ou morta a mae nao e pergunta que ela
/// faz.
#[test]
fn sem_conferir_a_mae_morta_nao_tranca_nada() {
    let d = dir("mae-morta-sem-conferir");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.excluir_suave(1, "saiu").unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = filha(&d, false);
    f.inserir(&[Value::Int(10), Value::Int(1)])
        .expect("chave sem `verificar` continua como sempre foi");
}

/// Mae VIVA continua entrando, e este e o controle: sem ele, um portao que
/// recusasse toda gravacao passaria pelos quatro testes acima.
#[test]
fn a_mae_viva_continua_aceitando_filha() {
    let d = dir("mae-viva-controle");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)])
        .expect("mae viva: a filha entra");
}

// ---------------------------------------------------------------------------
// A terceira porta: restaurar
// ---------------------------------------------------------------------------
//
// `inserir` e `atualizar` conferem; restaurar era a porta que nao perguntava.
// Nas invariantes de hoje ela nao poderia falhar -- a mae nao morre com filha,
// nem suave --, e a conferencia entra mesmo assim: a orfa por construcao
// sobreviveu versoes porque a porta que faltava era a que ninguem olhava.

/// Uma filha marcada nao volta se a mae dela nao estiver viva.
///
/// O cenario e o que a replica produz legitimamente: la o `aplicar_evento`
/// aplica sem julgar, entao a filha pode estar marcada e a mae pode nao ter
/// chegado. Restaurar a filha ali criaria a orfa VISIVEL.
#[test]
fn a_filha_marcada_nao_volta_sem_mae_viva() {
    let d = dir("restaurar-sem-mae");
    // Um source, para tirar dele a imagem da filha.
    let ds = dir("restaurar-source");
    let mut ms = mae(&ds);
    ms.inserir(&[Value::Int(1)]).unwrap();
    ms.sincronizar().unwrap();
    drop(ms);
    let mut fs = filha(&ds, true).com_imagem_no_diario(true);
    fs.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    fs.sincronizar().unwrap();
    let (e, imagem) = fs.diario_com_imagem(0, 0).unwrap().remove(0);
    drop(fs);

    // A replica: a filha chega, a mae nao.
    mae(&d).sincronizar().unwrap();
    let mut f = filha(&d, true);
    f.aplicar_evento(e.operacao, e.rowid, &imagem).unwrap();
    f.excluir_suave(1, "some").unwrap();
    f.sincronizar().unwrap();

    let erro = f
        .restaurar(1, "volta")
        .expect_err("sem mae viva, a filha nao volta");
    let t = erro.to_string();
    assert!(t.contains("nao pode voltar"), "veio: {t}");
    assert!(t.contains("fk_cliente"), "e tem de dizer qual chave: {t}");
}

/// E o controle: com a mae viva, restaurar continua funcionando como sempre.
#[test]
fn com_mae_viva_restaurar_continua_igual() {
    let d = dir("restaurar-com-mae");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.excluir_suave(1, "some").unwrap();
    assert!(f.restaurar(1, "volta").expect("a mae esta viva"));
}

/// E o comportamento VELHO: chave sem `verificar` nao paga nem a leitura.
#[test]
fn sem_conferir_restaurar_nao_pergunta_nada() {
    let d = dir("restaurar-sem-conferir");
    mae(&d).sincronizar().unwrap();
    let mut f = filha(&d, false);
    f.inserir(&[Value::Int(10), Value::Int(999)]).unwrap();
    f.excluir_suave(1, "some").unwrap();
    assert!(
        f.restaurar(1, "volta")
            .expect("chave sem verificar: como sempre"),
        "restaurar tem de devolver true"
    );
}

/// A mensagem nao pode mandar reparar uma tabela SA.
///
/// # O defeito, e por que o comentario do codigo o escondeu
///
/// A mae escrita e ainda nao sincronizada faz a conferencia recusar, e a recusa
/// esta certa. Errado era o TEXTO: ele embrulhava o erro cru do indice
/// («ficou para tras numa queda... reconstrua com `reparar indice`») e mandava
/// junto com a explicacao correta -- duas metades se contradizendo, e a
/// primeira mandando o operador reparar um arquivo intacto.
///
/// O que fez isso durar: o comentario acima da linha JA dizia «o erro cru manda
/// reconstrua o indice, o que faria o leitor reparar um arquivo sao. Este diz o
/// que houve» -- com o `({e})` logo abaixo. **Envolver nao e substituir**, e
/// comentario que se declara resolvido e o motivo de ninguem olhar de novo.
///
/// # Prova real
///
/// Devolver o `({e})` a mensagem faz este teste cair em `nao pode mandar
/// reparar` -- o texto cru volta a aparecer.
#[test]
fn a_mae_invisivel_nao_manda_reparar_indice_sao() {
    let d = dir("mensagem-sem-reparar");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    // DE PROPOSITO sem sincronizar: e o que levanta a marca de visibilidade.
    let mut f = filha(&d, true);
    let erro = f
        .inserir(&[Value::Int(10), Value::Int(1)])
        .expect_err("a mae ainda nao esta visivel: tinha de recusar");
    let texto = erro.to_string();

    assert!(
        !texto.contains("reparar indice") && !texto.contains("ficou para tras"),
        "nao pode mandar reparar: a tabela esta sa, so nao esta sincronizada -- {texto}"
    );
    assert!(
        texto.contains("confirme a mae antes da filha"),
        "tirou o texto cru mas perdeu a explicacao -- {texto}"
    );
}
