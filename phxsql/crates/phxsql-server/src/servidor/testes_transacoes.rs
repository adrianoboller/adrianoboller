//! As transacoes pelo PROTOCOLO: `BEGIN`, o conjunto de escrita, o `COMMIT`,
//! o `ROLLBACK`, os `SAVEPOINT`, o escopo declarado e as travas.
//!
//! # O teste que mais importa neste modulo
//!
//! Nao e nenhum dos que exercitam o recurso novo: e o
//! `sem_transacao_nada_muda`. Quem nunca manda `BEGIN` tem de ver EXATAMENTE
//! o comportamento de hoje -- inclusive a mensagem literal do `inserir_lote`
//! sobre as linhas gravadas antes do erro. Guarda nova entra pedida.
use super::*;

fn dir_temp(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("tx-{rotulo}"))
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
    Servidor::novo(c).unwrap()
}

/// Uma sessao com LIGACAO, que e o que a transacao exige: sem id de
/// conexao a transacao nao teria a que morrer amarrada.
fn sessao(ligacao: u64) -> Sessao {
    Sessao {
        ligacao,
        ip: "127.0.0.1".into(),
        ..Sessao::default()
    }
}

fn pede(s: &Arc<Servidor>, ses: &Sessao, corpo: &str) -> Result<Json> {
    let mut copia = Sessao {
        ligacao: ses.ligacao,
        ip: ses.ip.clone(),
        usuario: ses.usuario.clone(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut copia,
        "127.0.0.1",
    );
    r
}

/// Um banco com `clientes` e `pedidos`, e nada dentro.
fn base(s: &Arc<Servidor>, ses: &Sessao) {
    pede(s, ses, r#""op":"criar_database","database":"loja""#).unwrap();
    for tabela in ["clientes", "pedidos"] {
        pede(
            s,
            ses,
            &format!(
                r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
                       "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                                  {{"nome":"nome","tipo":"Str(40)"}}],
                       "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
            ),
        )
        .unwrap();
    }
}

fn quantas(s: &Arc<Servidor>, ses: &Sessao, tabela: &str) -> u64 {
    let r = pede(
        s,
        ses,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":1000"#),
    )
    .unwrap();
    r.campo("linhas")
        .and_then(Json::lista)
        .map(|l| l.len() as u64)
        .unwrap_or(0)
}

fn slots(s: &Arc<Servidor>, ses: &Sessao, tabela: &str) -> u64 {
    let r = pede(
        s,
        ses,
        &format!(r#""op":"esquema","database":"loja","tabela":"{tabela}""#),
    )
    .unwrap();
    r.campo("slots").and_then(Json::inteiro).unwrap_or(0) as u64
}

// ------------------------------------------------- o comportamento VELHO

/// **O teste que mais importa.** Quem nunca abre transacao ve exatamente o
/// servidor de sempre: insere, atualiza, exclui, e o `inserir_lote` que
/// para no erro continua dizendo o que gravou antes de parar.
#[test]
fn sem_transacao_nada_muda() {
    let dir = dir_temp("velho");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);

    let r = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    assert_eq!(r.campo("rowid").and_then(Json::inteiro), Some(1));
    // Gravou de VERDADE, sem commit nenhum: e o autocommit de sempre.
    assert_eq!(quantas(&s, &ses, "clientes"), 1);

    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "linha":{"id":1,"nome":"Ana Maria"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1"#,
    )
    .unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 0);

    // O lote que para no erro: a mensagem de sempre, com o que gravou.
    let r = pede(
        &s,
        &ses,
        r#""op":"inserir_lote","database":"loja","tabela":"pedidos",
               "linhas":[{"id":1,"nome":"a"},{"id":1,"nome":"b"}]"#,
    )
    .unwrap();
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(1));
    assert!(!r
        .campo("erros")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .is_empty());

    // E o estado da transacao desta conexao e IDLE, que e resposta e nao erro.
    let r = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    assert_eq!(r.texto_ou("transaction_state", ""), "IDLE");
    // Nenhuma trava viva no servidor inteiro.
    assert_eq!(s.travas.travar().quantas(), 0);
}

// ------------------------------------ GAP 242: CHECK recusa NA INSTRUCAO

/// **CHECK dentro de transacao recusa NA INSTRUCAO, nao derruba o COMMIT.**
///
/// Ate o pedido 242 o `empilhar` conferia unicidade e o gatilho BEFORE, mas
/// NAO julgava padrao/calculada/CHECK -- quem julgava era o `t.inserir` do
/// COMMIT. Uma instrucao do MEIO com CHECK violado passava batido no
/// empilhar e so estourava no COMMIT, derrubando a transacao inteira -- as
/// linhas boas junto. Os tres motores maduros recusam CHECK NA INSTRUCAO
/// (aceite automatico).
///
/// PROVA REAL nos dois sentidos: com o defeito reposto (tirar o
/// `julgar_regras_de_escrita` do `empilhar`), o `inserir` proibido volta
/// `Ok` -- e este teste reprova no `unwrap_err` -- e o estrago aparece so no
/// COMMIT. Com o conserto, o `inserir` recusa AQUI, a transacao segue
/// `ACTIVE`, a proxima linha boa empilha, e o COMMIT grava so as duas boas.
#[test]
fn check_dentro_de_transacao_recusa_na_instrucao_e_nao_derruba_o_commit() {
    let dir = dir_temp("check-na-instrucao");
    let s = servidor(&dir);
    let ses = sessao(42);
    pede(&s, &ses, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"criar_tabela","database":"loja","tabela":"caixa",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"v","tipo":"Int8","check":"v > 0"}],
               "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();

    pede(&s, &ses, r#""op":"begin""#).unwrap();

    // 1. Linha boa -- empilha.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"caixa","linha":{"id":1,"v":10}"#,
    )
    .unwrap();

    // 2. Linha do MEIO que viola o CHECK -- recusa AQUI, na instrucao.
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"caixa","linha":{"id":2,"v":-5}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("CHECK da coluna v"), "{e}");

    // A transacao NAO caiu: um CHECK violado e erro de INSTRUCAO (2xxx),
    // como uma chave duplicada -- segue `ACTIVE`.
    let est = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    assert_eq!(
        est.texto_ou("transaction_state", ""),
        "ACTIVE",
        "{}",
        est.escrever()
    );

    // 3. Outra linha boa empilha DEPOIS do erro -- a prova de que a
    //    transacao sobreviveu a instrucao recusada.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"caixa","linha":{"id":3,"v":20}"#,
    )
    .unwrap();

    // 4. COMMIT grava as DUAS boas -- e nao cai por causa da do meio, que
    //    nunca chegou a empilhar.
    pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(
        quantas(&s, &ses, "caixa"),
        2,
        "id 1 e id 3 gravados; id 2 nao"
    );
    let r = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"loja","tabela":"caixa","max":100"#,
    )
    .unwrap();
    let ids: Vec<i64> = r
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect();
    assert_eq!(ids, vec![1, 3]);
}

// ------------------------------------ SP000006: read-your-own-writes

/// A transacao enxerga o que ela mesma ALTEROU, EXCLUIU e INSERIU.
///
/// # Por que os quatro caminhos num teste so
///
/// Porque o defeito que este teste existe para pegar nao e «o `ler` nao
/// mostra»: e o `ler` mostrar e o `varrer` nao, ou a lista mostrar e a
/// CONTA continuar a do disco. Uma tela que diz «5 de 4 linhas» nao tem
/// como estar certa, e quem olha nao sabe qual dos dois numeros mentiu.
///
/// Por isso a sobreposicao mora num lugar so, e por isso a prova pergunta
/// pelos quatro caminhos na mesma transacao.
#[test]
fn a_transacao_enxerga_o_que_ela_mesma_escreveu() {
    let dir = dir_temp("ryow");
    let s = servidor(&dir);
    let ses = sessao(21);
    base(&s, &ses);
    for i in 1..=3 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
    }
    assert_eq!(quantas(&s, &ses, "clientes"), 3);

    pede(&s, &ses, r#""op":"begin""#).unwrap();

    // 1. INSERIR -- a linha nova aparece, e aparece no FIM, que e onde o
    //    commit vai grava-la: a ordem de digitacao e sagrada tambem aqui.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{"id":4,"nome":"nova"}"#,
    )
    .unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 4);
    let r = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )
    .unwrap();
    // A CONTA se confere AQUI, e nao depois do excluir, e a diferenca
    // custou uma sabotagem: la embaixo o disco tem 3 e a transacao tambem
    // tem 3 (uma a mais, uma escondida), entao a asercao passava com a
    // conta do disco -- teste que passa por engano e pior que teste que
    // falta. Neste ponto os dois numeros DIVERGEM: 3 no disco, 4 aqui.
    assert_eq!(
        r.campo("visiveis").and_then(Json::inteiro),
        Some(4),
        "«4 linhas» na lista e «3» na conta seria a tela mentindo sobre si mesma"
    );
    let ultima = r
        .campo("linhas")
        .and_then(Json::lista)
        .and_then(|l| l.last())
        .and_then(|j| j.campo("nome"))
        .and_then(Json::texto)
        .unwrap_or("")
        .to_string();
    assert_eq!(ultima, "nova", "a linha empilhada tem de sair no fim");

    // 2. LER pelo rowid previsto devolve a linha empilhada.
    let r = pede(
        &s,
        &ses,
        r#""op":"ler","database":"loja","tabela":"clientes","rowid":4"#,
    )
    .unwrap();
    assert_eq!(r.texto_ou("nome", ""), "nova");

    // 3. ALTERAR uma linha do disco -- a leitura devolve o valor NOVO, e o
    //    contador nao se mexe: alterar nao cria nem apaga linha.
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":2,
               "valores":{"id":2,"nome":"MUDADA"}"#,
    )
    .unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"ler","database":"loja","tabela":"clientes","rowid":2"#,
    )
    .unwrap();
    assert_eq!(r.texto_ou("nome", ""), "MUDADA");
    assert_eq!(quantas(&s, &ses, "clientes"), 4);

    // 4. EXCLUIR (suave) some da lista E da conta, na mesma passada.
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,
               "motivo":"prova""#,
    )
    .unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 3);
    let r = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("visiveis").and_then(Json::inteiro),
        Some(3),
        "a conta tem de descer junto com a exclusao suave"
    );

    // E o vizinho continua vendo o disco: tres linhas, nenhuma mudanca.
    let outra = sessao(22);
    assert_eq!(quantas(&s, &outra, "clientes"), 3);
    let r = pede(
        &s,
        &outra,
        r#""op":"ler","database":"loja","tabela":"clientes","rowid":2"#,
    )
    .unwrap();
    assert_eq!(
        r.texto_ou("nome", ""),
        "c2",
        "isolamento: o vizinho ve o disco"
    );

    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 3);
}

/// O COMPORTAMENTO VELHO do caminho que empilha: ele decide contra o
/// **disco**, e a linha pendente recusa NOMEANDO a transacao.
///
/// # Por que este teste entrou junto do conserto de 17/09/2026
///
/// Porque o conserto mexe exatamente aqui. O `empilhar` abria a tabela
/// pela porta que monta a sobreposicao da transacao e a desligava na linha
/// seguinte; hoje abre pela porta que nao a monta
/// (`abrir_travada_sem_sobrepor`), porque montar para jogar fora custava
/// O(pendentes) com a trava de dados na mao. As duas formas deixam o
/// `Table` no mesmo estado -- e e justamente por isso que so' um teste do
/// comportamento VELHO diz que a dispensa continua de pe.
///
/// As duas metades, e as duas importam:
///
/// 1. a chave que ja esta EM DISCO vira `atualizar` -- o upsert enxerga o
///    disco pelo handle sem sobreposicao;
/// 2. a chave que so' esta PENDENTE recusa com a frase da transacao, e nao
///    com «o indice unico ja tem essa chave», que mandaria procurar no
///    lugar errado.
#[test]
fn dentro_da_transacao_o_upsert_decide_contra_o_disco() {
    let dir = dir_temp("upsert-disco");
    let s = servidor(&dir);
    let ses = sessao(31);
    base(&s, &ses);
    // Uma linha JA GRAVADA, fora de transacao: e o disco contra o qual o
    // upsert vai decidir.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();

    pede(&s, &ses, r#""op":"begin","database":"loja""#).unwrap();

    // (1) a chave do DISCO vira `atualizar`, e empilha como tal.
    let r = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{"id":1,"nome":"Ana Maria"},"se_existir":"atualizar""#,
    )
    .unwrap();
    assert_eq!(
        r.campo("acao").and_then(Json::texto),
        Some("atualizar"),
        "o upsert tinha de enxergar a linha do disco: {}",
        r.escrever()
    );

    // (2) uma chave que so' existe NA LISTA, empilhada agora.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":9,"nome":"Bia"}"#,
    )
    .unwrap();
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{"id":9,"nome":"Bia II"},"se_existir":"atualizar""#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("nesta mesma transacao"),
        "a recusa tinha de nomear a transacao, e veio: {e}"
    );

    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    // O rollback devolve o disco como estava: uma linha, com o nome velho.
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
}

/// A dispensa da sobreposicao no caminho que EMPILHA, provada pelo unico
/// lado em que ela aparece de fora: **o `empilhar` le o DISCO**.
///
/// # O caso, e por que e ele
///
/// Excluir de vez e, na mesma transacao, alterar a mesma linha. O
/// `empilhar` le `velha` do disco, acha a linha e empilha a alteracao; a
/// ordem da lista e que decide o resultado no commit, como o
/// `a_ultima_escrita_da_mesma_linha_e_a_que_manda` ja prova.
///
/// Com a sobreposicao LIGADA aqui, o `t.ler(rowid)` enxergaria a exclusao
/// pendente, `velha` viria vazia e a alteracao seria recusada com «rowid 1
/// nao existe» -- um erro sobre uma linha que esta em disco. E o valor de
/// `linha_antiga` que a reaplicacao da recuperacao precisa: ele tem de ser
/// o de ANTES da transacao, e nao o de dentro dela.
///
/// Este e o teste que **discrimina**. O
/// `dentro_da_transacao_o_upsert_decide_contra_o_disco`, ao lado, NAO
/// discrimina -- a recusa do `chave_ja_empilhada` vem antes do `buscar`, e
/// ele passa com a sobreposicao ligada ou desligada. Esta nota esta aqui
/// de proposito: teste que passa por engano e pior que teste que falta, e
/// o jeito de ele nao passar por engano e estar escrito o que ele prova.
#[test]
fn o_empilhar_le_o_disco_e_nao_a_lista_pendente() {
    let dir = dir_temp("empilhar-disco");
    let s = servidor(&dir);
    let ses = sessao(33);
    base(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();

    pede(&s, &ses, r#""op":"begin","database":"loja""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,"fisico":true"#,
    )
    .unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "linha":{"id":1,"nome":"Ana Maria"}"#,
    );
    assert!(
        r.is_ok(),
        "o `empilhar` le o DISCO: a linha esta la, e a alteracao empilha. \
             Veio: {:?}",
        r.err().map(|e| e.to_string())
    );

    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
}

/// A ordem manda: `UPDATE` e depois `DELETE` da mesma linha termina em
/// «sumida», e nao na linha alterada.
///
/// Dentro da transacao as escritas se sobrepoem como se cada uma ja
/// tivesse ido a disco. Guardar a primeira e ignorar a segunda faria o
/// `varrer` mostrar uma linha que o commit vai apagar.
#[test]
fn a_ultima_escrita_da_mesma_linha_e_a_que_manda() {
    let dir = dir_temp("ryow-ordem");
    let s = servidor(&dir);
    let ses = sessao(23);
    base(&s, &ses);
    for i in 1..=2 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
    }
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "valores":{"id":1,"nome":"passou por aqui"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,
               "motivo":"e depois sumiu""#,
    )
    .unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
}

// ------------------------------------------------------- o basico

/// `BEGIN; INSERT; ROLLBACK` nao consome slot, nao consome rowid e nao
/// deixa nada no disco. E a resposta da §3.1 do documento.
#[test]
fn o_rollback_de_um_insert_nao_queima_slot() {
    let dir = dir_temp("rollback");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    let antes = slots(&s, &ses, "clientes");

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    for i in 1..=50 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
    }
    // O PAR QUE PROVA O READ-YOUR-OWN-WRITES (SP000006), e ele so vale
    // junto: a MESMA conexao ja enxerga as 50 linhas, e o disco continua
    // sem nenhuma. Ate esta sprint a primeira asercao era `0` -- a leitura
    // ia ao arquivo e o conjunto de escrita nao existia para ela.
    //
    // Separadas, cada uma passaria pelo motivo errado: so a de cima
    // passaria num motor que gravou tudo na hora (e ai a transacao nao
    // seria transacao), e so a de baixo passa no comportamento anterior,
    // que era o defeito. Sao as duas ou nenhuma.
    assert_eq!(quantas(&s, &ses, "clientes"), 50);
    assert_eq!(slots(&s, &ses, "clientes"), antes);

    // E a visibilidade acaba na CONEXAO. Outra sessao, no mesmo servidor e
    // no mesmo instante, continua vendo o disco -- e a diferenca entre
    // «leio o que eu mesmo escrevi» e «leitura suja», que e o que nenhum
    // motor serio entrega.
    let outra = sessao(8);
    assert_eq!(quantas(&s, &outra, "clientes"), 0);

    let r = pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(r.campo("descartadas").and_then(Json::inteiro), Some(50));
    assert_eq!(r.texto_ou("transaction_state", ""), "ROLLED_BACK");
    // **Zero slot queimado**, que e a regra petrea desta frente.
    assert_eq!(slots(&s, &ses, "clientes"), antes);
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// O INSERT pela camada SQL EMPILHA como qualquer `inserir`: a mesma
/// conexao ja o enxerga, o disco nao, e o ROLLBACK o desfaz sem queimar
/// slot. E o motivo de `VALUES (...), (...)` ser recusado pelo nome:
/// viraria `inserir_lote`, que nao empilha.
#[test]
fn o_insert_pelo_sql_empilha_na_transacao() {
    let dir = dir_temp("sql-empilha");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    let antes = slots(&s, &ses, "clientes");

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"sql","database":"loja",
               "texto":"INSERT INTO clientes (id, nome) VALUES (1, 'c1')""#,
    )
    .unwrap();
    assert_eq!(r.texto_ou("op", ""), "inserir");
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
    assert_eq!(quantas(&s, &sessao(8), "clientes"), 0);
    assert_eq!(slots(&s, &ses, "clientes"), antes);

    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
    assert_eq!(slots(&s, &ses, "clientes"), antes);
}

/// O `COMMIT` aplica a lista inteira, na ordem, com os rowids que a
/// transacao prometeu ao empilhar.
#[test]
fn o_commit_aplica_na_ordem_e_com_o_rowid_prometido() {
    let dir = dir_temp("commit");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    let mut prometidos = Vec::new();
    for i in 1..=20 {
        let r = pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
        assert_eq!(r.campo("empilhada").and_then(Json::booleano), Some(true));
        prometidos.push(r.campo("rowid").and_then(Json::inteiro).unwrap());
    }
    assert_eq!(prometidos, (1..=20).collect::<Vec<i64>>());

    let r = pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(20));
    assert_eq!(r.texto_ou("transaction_state", ""), "COMMITTED");
    assert_eq!(quantas(&s, &ses, "clientes"), 20);
    // O rowid prometido e o rowid gravado.
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"loja","tabela":"clientes","rowid":7"#,
    )
    .unwrap();
    assert_eq!(l.texto_ou("nome", ""), "c7");
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// Um banco com `clientes` e `pedidos(cliente_id -> clientes.id)`, a chave
/// CONFERIDA. Pedidos ganha um indice em `cliente_id` -- a cascata do
/// `ao_alterar` exige indice dos dois lados.
fn base_com_fk(s: &Arc<Servidor>, ses: &Sessao) {
    pede(s, ses, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(40)"}],
               "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    pede(
        s,
        ses,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"cliente_id","tipo":"Int8"}],
               "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true},
                          {"nome":"por_cliente","colunas":["cliente_id"]}],
               "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cliente_id"],
                                       "tabela_ref":"clientes","colunas_ref":["id"],
                                       "verificar":true,"ao_alterar":"cascata"}]"#,
    )
    .unwrap();
}

/// **PROVA REAL P0.** Dentro de UMA transacao, o pai empilhado tem de ser
/// visivel a conferencia da FK da filha. `BEGIN; INSERT pai; INSERT filha
/// (-> pai); COMMIT` tem de gravar os dois -- e com o defeito reposto
/// (`conferir_fks` lendo so o disco) o COMMIT recusa a filha, porque a mae
/// aberta num segundo descritor nao ve o pai que ainda esta na lista.
#[test]
fn p0_pai_empilhado_e_visivel_a_fk_da_filha_no_mesmo_commit() {
    let dir = dir_temp("p0-pai-empilhado");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":1}"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#).expect(
        "o COMMIT recusou a filha cujo pai foi inserido na MESMA transacao: \
             a conferencia de FK nao enxerga o pai empilhado (P0)",
    );
    assert_eq!(r.texto_ou("transaction_state", ""), "COMMITTED");
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
    assert_eq!(quantas(&s, &ses, "pedidos"), 1);
}

/// **O comportamento VELHO nao muda.** FORA de transacao ninguem empresta
/// mae nenhuma: a conferencia le o disco como sempre. A filha orfa e
/// recusada, e a filha com o pai JA no disco entra -- byte por byte o
/// servidor de sempre. E o `sem_transacao_nada_muda` desta rodada: guarda
/// nova entra pedida, nao imposta.
#[test]
fn p0_fora_de_transacao_a_fk_le_o_disco() {
    let dir = dir_temp("p0-fora-da-tx");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);

    // Filha orfa, sem transacao: recusada, como sempre.
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":1}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "INTEGRIDADE", "{e}");

    // Com o pai JA no disco, a filha entra -- o caminho de disco, intocado.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":1}"#,
    )
    .unwrap();
    assert_eq!(quantas(&s, &ses, "pedidos"), 1);
}

/// **A petrea «so existe filho se o pai existir primeiro» continua de pe.**
/// O conserto do P0 reusa o handle da mae, e o handle so ve o pai se ele
/// foi aplicado ANTES da filha na passada. Empilhar a filha ANTES do pai e
/// resolver no commit -- o `DEFERRABLE` do PostgreSQL -- NAO entra: a
/// passada aplica a filha primeiro, a mae ainda nao esta aberta, e a
/// conferencia cai no disco vazio e recusa. Filha antes do pai continua
/// sendo erro.
#[test]
fn p0_filha_antes_do_pai_no_commit_ainda_recusa() {
    let dir = dir_temp("p0-filha-antes");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    // A filha empilha primeiro (empilhar nao confere FK), depois o pai.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":1}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    assert_eq!(
        e.nome(),
        "INTEGRIDADE",
        "filha antes do pai tinha de ser recusada no commit: {e}"
    );
}

/// **PROVA REAL P0 na RECUPERACAO.** Uma queda no meio de um commit de
/// pai+filha deixa a marca `.tx` no disco e nada aplicado. Ao reabrir, a
/// recuperacao reaplica a marca -- e a filha, reaberta, tem de enxergar o
/// pai que a MESMA marca acabou de reaplicar. Com o defeito reposto
/// (`aplicar_uma` chamando `inserir` puro), a recuperacao batia na guarda
/// de visibilidade do `.ndx` da mae e o commit ficava pela metade, em
/// `operacoes IMPOSSIVEIS`.
#[test]
fn p0_recuperacao_completa_pai_e_filha_no_mesmo_commit() {
    let dir = dir_temp("p0-recuperar");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);

    // A marca a mao, na ORDEM pai-depois-filha, como a passada teria
    // empilhado. Nada aplicado no disco: a intencao inteira mora na marca.
    let escritas = vec![
        crate::transacao::Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: crate::transacao::Acao::Inserir,
            rowid: 1,
            linha: vec![Value::Int(1), Value::Str("Ana".into()), Value::Bool(false)],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        },
        crate::transacao::Escrita {
            database: "loja".into(),
            tabela: "pedidos".into(),
            acao: crate::transacao::Acao::Inserir,
            rowid: 1,
            linha: vec![Value::Int(1), Value::Int(1), Value::Bool(false)],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        },
    ];
    crate::transacao::gravar_marca(&dir.join("loja"), 77, crate::agora_ms(), &escritas).unwrap();
    drop(s);

    // O arranque acha a marca e COMPLETA o commit -- pai E filha.
    let s = servidor(&dir);
    assert_eq!(
        quantas(&s, &ses, "clientes"),
        1,
        "a recuperacao nao reaplicou o pai"
    );
    assert_eq!(
        quantas(&s, &ses, "pedidos"),
        1,
        "a recuperacao nao completou a filha -- P0 na recuperacao"
    );
    // A marca saiu: o commit foi completado sem sobrar `IMPOSSIVEL`.
    assert!(
        marcas(&dir).is_empty(),
        "a marca ficou pendurada -- a recuperacao nao completou: {:?}",
        marcas(&dir)
    );
}

/// Le o `cliente_id` da unica linha de `pedidos`, pela visao de `ses`.
fn cliente_id_do_pedido(s: &Arc<Servidor>, ses: &Sessao) -> i64 {
    let r = pede(
        s,
        ses,
        r#""op":"varrer","database":"loja","tabela":"pedidos","max":10"#,
    )
    .unwrap();
    r.campo("linhas")
        .and_then(Json::lista)
        .and_then(|l| l.first().cloned())
        .and_then(|linha| linha.campo("cliente_id").and_then(Json::inteiro))
        .unwrap()
}

/// **PROVA REAL do ACID-C -- FECHADO.** A cascata do `ao_alterar` entra
/// INTEIRA no conjunto de escrita da transacao (super-journal do SQLite): a
/// filha vira uma escrita propria da lista, entao o read-your-own-writes a
/// alcanca, o `COMMIT` a conta, a marca `.tx` a descreve e o `ROLLBACK` a
/// desfaz. Ver `docs/ACID.md` §2.4/§3.3/§4.4.
///
/// * dentro da transacao, apos empilhar a alteracao da chave da mae de 1
///   para 2, a MAE volta `id:2` E a FILHA volta `cliente_id:2` -- a
///   sobreposicao le a escrita da cascata como qualquer outra (§4.4);
/// * o `COMMIT` responde `gravadas:2` -- a mae e a filha, as duas na lista
///   e as duas na marca (§3.3).
///
/// A prova de que ela PEGA: revertido o `empilhar` para nao expandir a
/// cascata (a mae voltaria a cascatear so no commit), as duas asserts
/// falham -- e a prova por queda vive no
/// `acidc_a_marca_v3_recupera_a_cascata_achatada` e no rollback logo abaixo.
#[test]
fn acidc_a_cascata_entra_no_conjunto_de_escrita_da_transacao() {
    let dir = dir_temp("acidc-cascata");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":10,"cliente_id":1}"#,
    )
    .unwrap();

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "linha":{"id":2,"nome":"Ana"}"#,
    )
    .unwrap();
    // (VERMELHO) o read-your-own-writes DEVERIA alcancar a cascata: a mae
    // agora e id=2, e a filha da mesma transacao deveria acompanhar.
    assert_eq!(
        cliente_id_do_pedido(&s, &ses),
        2,
        "read-your-own-writes nao alcanca a cascata (ACID-C, §4.4)"
    );
    let r = pede(&s, &ses, r#""op":"commit""#).unwrap();
    // (VERMELHO) a transacao alterou DUAS tabelas; o commit deveria contar
    // as duas, e a marca deveria descrever as duas.
    assert_eq!(
        r.campo("gravadas").and_then(Json::inteiro),
        Some(2),
        "a cascata da filha nao entra no conjunto de escrita (ACID-C, §3.3)"
    );
}

/// Os eventos da auditoria, na ordem de digitacao.
fn eventos_da_auditoria(s: &Arc<Servidor>, ses: &Sessao) -> Vec<String> {
    let r = pede(
        s,
        ses,
        r#""op":"varrer","database":"loja","tabela":"auditoria","max":100"#,
    )
    .unwrap();
    r.campo("linhas")
        .and_then(Json::lista)
        .map(|l| {
            l.iter()
                .map(|x| x.texto_ou("evento", "").to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// **Pedido 562: a mesma cascata do `ao_alterar` da o MESMO resultado de
/// gatilho dentro e fora da transacao.** A `INTEGRIDADE.md` §7.3 decidiu
/// que a cascata nao dispara gatilho; a alteracao solta ja descartava o
/// AFTER do elo, e o COMMIT o rodava -- a auditoria ficava com `filha` so
/// quando havia `BEGIN`. Os dois caminhos passam pela mesma passada
/// (`aplicar_conjunto`), e e nela que a decisao mora agora.
///
/// Prova real: com o `!e.elo_da_cascata` tirado do `dispara` da passada, o
/// COMMIT dispara o `audita_filha` (o aviso do 262 o nomeia) e o teste cai.
#[test]
fn a_cascata_dispara_os_mesmos_gatilhos_dentro_e_fora_da_transacao() {
    let dir = dir_temp("562-gatilho-do-elo");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"criar_tabela","database":"loja","tabela":"auditoria",
               "colunas":[{"nome":"evento","tipo":"Str(60)"}]"#,
    )
    .unwrap();
    for texto in [
        "CREATE TRIGGER audita_mae AFTER UPDATE ON clientes FOR EACH ROW \
             INSERT INTO auditoria (evento) VALUES ('mae')",
        "CREATE TRIGGER audita_filha AFTER UPDATE ON pedidos FOR EACH ROW \
             INSERT INTO auditoria (evento) VALUES (CONCAT('filha ', NEW.id))",
    ] {
        let corpo = Json::objeto(vec![
            ("op", Json::texto_de("sql")),
            ("database", Json::texto_de("loja")),
            ("texto", Json::texto_de(texto)),
        ])
        .escrever();
        // `pede` embrulha o corpo em `{"token":..., <corpo>}`: tiro as chaves.
        pede(&s, &ses, &corpo[1..corpo.len() - 1]).unwrap();
    }
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":10,"cliente_id":1}"#,
    )
    .unwrap();

    // Fora da transacao: a mae troca a chave, a filha acompanha pela
    // cascata, e so o AFTER da mae dispara.
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "linha":{"id":2,"nome":"Ana"}"#,
    )
    .unwrap();
    assert_eq!(cliente_id_do_pedido(&s, &ses), 2);
    let solta = eventos_da_auditoria(&s, &ses);
    assert_eq!(solta, vec!["mae".to_string()], "alteracao solta");

    // Dentro da transacao: a MESMA cascata, e o mesmo resultado.
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "linha":{"id":3,"nome":"Ana"}"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(2));
    assert_eq!(cliente_id_do_pedido(&s, &ses), 3);
    // O AFTER que roda no COMMIT nao grava noutra tabela (pedido 262) e
    // vira aviso nomeando o gatilho -- e o aviso e o que diz QUAIS
    // dispararam. O da mae dispara nos dois caminhos; o da filha, em
    // nenhum.
    let avisos = r
        .campo("gatilhos_avisos")
        .map(Json::escrever)
        .unwrap_or_default();
    assert!(avisos.contains("audita_mae"), "{}", r.escrever());
    assert!(
        !avisos.contains("audita_filha"),
        "o COMMIT disparou o AFTER do elo da cascata, que a solta nao dispara: {}",
        r.escrever()
    );
    assert_eq!(eventos_da_auditoria(&s, &ses), solta);
}

/// **O `ROLLBACK` alcanca a cascata, porque ela e escrita da lista.** Antes
/// do ACID-C a cascata so acontecia no `aplicar_conjunto`, entao o
/// `ROLLBACK` -- que so joga fora a lista -- nao a alcancava. Agora ela E a
/// lista: desfazer a lista desfaz a cascata. Ver `docs/ACID.md` §2.4.
#[test]
fn acidc_o_rollback_desfaz_a_cascata() {
    let dir = dir_temp("acidc-rollback");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":10,"cliente_id":1}"#,
    )
    .unwrap();

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "linha":{"id":2,"nome":"Ana"}"#,
    )
    .unwrap();
    // Dentro da transacao a filha ja acompanha (read-your-own-writes).
    assert_eq!(cliente_id_do_pedido(&s, &ses), 2);
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    // Desfeito nos DOIS: a chave da mae voltou a 1, e a filha com ela.
    assert_eq!(
        cliente_id_do_pedido(&s, &ses),
        1,
        "o ROLLBACK nao desfez a cascata da filha (ACID-C)"
    );
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
}

/// **PROVA POR QUEDA: a marca v3 recupera a cascata ACHATADA, sem duplicar.**
/// A marca de um commit pai+cascata escrito a mao, na ORDEM da passada
/// (mae primeiro, filha depois), as duas com `cascata_na_lista: true`. O
/// arranque acha a marca e completa as DUAS sem re-cascatear -- reaplicar a
/// mae com cascata gravaria a filha uma segunda vez. Ver `docs/ACID.md`
/// §2.4 e a marca v3 em `docs/FORMATO.md`.
#[test]
fn acidc_a_marca_v3_recupera_a_cascata_achatada() {
    let dir = dir_temp("acidc-recuperacao");
    let s = servidor(&dir);
    let ses = sessao(7);
    base_com_fk(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":10,"cliente_id":1}"#,
    )
    .unwrap();
    // A marca que a passada teria deixado ao trocar a chave da mae de 1 para
    // 2: a mae e a filha, ACHATADAS, as duas `cascata_na_lista: true`. Nada
    // aplicado ainda -- a intencao inteira mora na marca.
    let escritas = vec![
        crate::transacao::Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: crate::transacao::Acao::Atualizar,
            rowid: 1,
            linha: vec![Value::Int(2), Value::Str("Ana".into()), Value::Bool(false)],
            linha_antiga: vec![Value::Int(1), Value::Str("Ana".into()), Value::Bool(false)],
            motivo: String::new(),
            cascata_na_lista: true,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        },
        crate::transacao::Escrita {
            database: "loja".into(),
            tabela: "pedidos".into(),
            acao: crate::transacao::Acao::Atualizar,
            rowid: 1,
            linha: vec![Value::Int(10), Value::Int(2), Value::Bool(false)],
            linha_antiga: vec![Value::Int(10), Value::Int(1), Value::Bool(false)],
            motivo: String::new(),
            cascata_na_lista: true,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        },
    ];
    crate::transacao::gravar_marca(&dir.join("loja"), 55, crate::agora_ms(), &escritas).unwrap();
    drop(s);

    // O arranque completa o commit -- mae E filha -- pela marca achatada.
    let s = servidor(&dir);
    assert_eq!(
        cliente_id_do_pedido(&s, &ses),
        2,
        "a recuperacao nao completou a cascata achatada (ACID-C)"
    );
    // Um segundo arranque nao duplica: a marca ja sumiu, e a reaplicacao e
    // idempotente pelo rowid.
    drop(s);
    let s = servidor(&dir);
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
    assert_eq!(quantas(&s, &ses, "pedidos"), 1);
    assert_eq!(cliente_id_do_pedido(&s, &ses), 2);
}

fn marcas(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir.join("loja"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".tx"))
                .collect()
        })
        .unwrap_or_default()
}

/// **A ordem do group commit, e ela nao se inverte.**
///
/// Com a janela de durabilidade aberta -- o padrao -- a marca FICA depois
/// do commit: ela e o bilhete que traz o dado de volta se a energia cair
/// antes de o `fsync` acontecer. Ela so sai quando a janela fecha, e sai
/// DEPOIS do `fsync`, nunca antes.
///
/// Com `durabilidade: por_operacao` a janela fecha em toda gravacao, e a
/// marca sai no proprio commit.
///
/// A janela fica aberta por CONFIGURACAO, e nao pelos 200 ms de fabrica:
/// o relogio dela comeca a contar quando o servidor nasce, e numa maquina
/// ocupada criar o banco e as duas tabelas ja passava dos 200 ms -- a
/// janela fechava no proprio commit, a marca saia na hora e a primeira
/// asercao caia sem defeito nenhum (medido em 16/09/2026: 2 quedas em 15
/// corridas com outra suite rodando ao lado). Teste que depende do
/// relogio de parede passa ou cai conforme a maquina, e nao conforme o
/// codigo.
#[test]
fn a_marca_espera_o_fsync_e_so_entao_e_apagada() {
    let dir = dir_temp("marca");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.recursos.lote_milissegundos = 3_600_000;
    let s = Servidor::novo(c).unwrap();
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a marca tem de ESPERAR o fsync -- apaga-la antes joga fora o \
             bilhete de um dado que pode nao estar no disco"
    );
    // A janela fecha, e a marca sai junto.
    s.descarregar_sujas();
    assert!(
        marcas(&dir).is_empty(),
        "a marca tem de sair quando a janela fecha: {:?}",
        marcas(&dir)
    );
}

/// Com `por_operacao` nao ha janela: a marca sai no proprio commit.
#[test]
fn sem_janela_a_marca_sai_no_commit() {
    let dir = dir_temp("marca-sem-janela");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.recursos.durabilidade = crate::config::Durabilidade::PorOperacao;
    let s = Servidor::novo(c).unwrap();
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert!(marcas(&dir).is_empty(), "{:?}", marcas(&dir));
}

/// **Pedido 254**: a marca do COMMIT feito DENTRO de uma tabela reservada
/// sai no `bulkinsert(false)`.
///
/// Com a tabela reservada a janela nao fecha no commit -- e o segundo
/// ganho da reserva --, entao a marca fica pendente esperando o `fsync`,
/// que e o do `bulkinsert(false)`. Ele sincronizava a tabela e a tirava
/// das sujas por conta propria, sem passar pela drenagem do fecho: a
/// marca de um commit ja duravel ficava no disco para sempre, e toda
/// tomada chutada depois do «ok» fazia o arranque reportar «achadas 1 /
/// ja aplicadas N» para um commit que ja tinha acabado. Medido pela
/// bancada `chutar-a-tomada.py` em 16/09/2026, sem queda: a marca
/// continuava la 300 ms depois do «ok».
#[test]
fn a_marca_do_commit_na_reserva_sai_no_bulkinsert_false() {
    let dir = dir_temp("marca-na-reserva");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"bulkinsert","database":"loja","tabela":"clientes","ligado":true"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"commit""#).unwrap();
    // O cenario do pedido: reservada, a janela nao fechou e a marca esta
    // pendurada -- e assim que tem de estar, ate o fsync acontecer.
    assert_eq!(
        marcas(&dir).len(),
        1,
        "com a tabela reservada a marca tem de ESPERAR o fsync do \
             bulkinsert(false)"
    );
    let r = pede(
        &s,
        &ses,
        r#""op":"bulkinsert","database":"loja","tabela":"clientes","ligado":false"#,
    )
    .unwrap();
    assert_eq!(r.campo("sincronizada").and_then(Json::booleano), Some(true));
    assert!(
        marcas(&dir).is_empty(),
        "o bulkinsert(false) sincronizou a tabela e a marca do commit \
             ficou no disco: {:?}",
        marcas(&dir)
    );
    assert!(
        s.marcas_pendentes.lock().unwrap().is_empty(),
        "a marca saiu do disco mas ficou na lista de pendentes"
    );
    assert!(s.sujas.lock().unwrap().is_empty());
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
}

/// **O irmao do pedido 254**, achado procurando quem mais tira tabela
/// das sujas sem passar pela drenagem: nenhuma reserva, `por_lote`, UMA
/// tabela so.
///
/// O commit deixa a marca pendente (janela aberta). A gravacao que fecha
/// a janela sincroniza a tabela, a tira das sujas e chama o fecho -- que
/// voltava antes de drenar as marcas porque a lista de sujas JA estava
/// vazia. A marca de um commit ja duravel ficava pendurada ate a proxima
/// janela que fechasse com DUAS tabelas sujas, ou para sempre; e o
/// relogio de fundo nao a alcancava, porque ele tambem volta quando nao
/// ha tabela suja. O `bulkinsert(false)` e este fecho chamam as mesmas
/// funcoes na mesma ordem -- sincronizar, tirar das sujas, drenar --, e
/// por isso o conserto de um sem o outro seria meio conserto.
#[test]
fn a_janela_que_fecha_numa_tabela_so_leva_a_marca_junto() {
    let dir = dir_temp("marca-janela-de-uma");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    // A janela fecha pela CONTAGEM, nunca pelo relogio: um teste que
    // dependesse dos 200 ms de fabrica passaria ou cairia conforme a
    // maquina estivesse ocupada.
    c.recursos.lote_operacoes = 4;
    c.recursos.lote_milissegundos = 3_600_000;
    let s = Servidor::novo(c).unwrap();
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a primeira gravacao da janela nao a fecha: a marca tem de esperar"
    );
    // Tres gravacoes soltas na MESMA tabela: a quarta operacao da janela
    // (`lote_operacoes = 4`) e a que fecha, e ela e a unica suja.
    for id in 2..=4 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id},"nome":"b"}}"#
            ),
        )
        .unwrap();
    }
    assert!(
        s.sujas.lock().unwrap().is_empty(),
        "a janela nao fechou: o cenario nao e o do teste"
    );
    assert!(
        marcas(&dir).is_empty(),
        "a janela fechou numa tabela so e a marca do commit ficou \
             pendurada: {:?}",
        marcas(&dir)
    );
    assert!(s.marcas_pendentes.lock().unwrap().is_empty());
    assert_eq!(quantas(&s, &ses, "clientes"), 4);
}

// -------------------------------------------------------- os savepoints

/// `ROLLBACK TO SAVEPOINT` trunca a lista e a transacao **continua
/// aberta** -- e o que torna o ponto quase de graca neste desenho.
#[test]
fn o_savepoint_trunca_a_lista_e_a_transacao_segue() {
    let dir = dir_temp("savepoint");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    for i in 1..=3 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
    }
    pede(&s, &ses, r#""op":"savepoint","nome":"p1""#).unwrap();
    for i in 4..=6 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
    }
    let r = pede(&s, &ses, r#""op":"rollback_para","nome":"p1""#).unwrap();
    assert_eq!(r.campo("descartadas").and_then(Json::inteiro), Some(3));
    assert_eq!(r.campo("linhas").and_then(Json::inteiro), Some(3));
    assert_eq!(r.texto_ou("transaction_state", ""), "ACTIVE");

    // A transacao continua ACEITANDO trabalho, e a chave descartada volta
    // a caber: sem refazer o conjunto de chaves, o `id 4` continuaria
    // barrado por uma linha que ninguem vai gravar.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":4,"nome":"outro"}"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(4));
    assert_eq!(quantas(&s, &ses, "clientes"), 4);
}

#[test]
fn release_savepoint_tira_o_ponto_e_deixa_o_trabalho() {
    let dir = dir_temp("release");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"savepoint","nome":"p1""#).unwrap();
    let r = pede(&s, &ses, r#""op":"release_savepoint","nome":"p1""#).unwrap();
    assert_eq!(r.campo("linhas").and_then(Json::inteiro), Some(1));
    // O ponto sumiu, e voltar a ele agora e erro.
    let e = pede(&s, &ses, r#""op":"rollback_para","nome":"p1""#).unwrap_err();
    assert_eq!(e.nome(), "NAO_ENCONTRADO");
    // E o trabalho continua la.
    pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
}

// ------------------------------------------------- as classes de erro

/// Erro de INSTRUCAO cancela a instrucao e a transacao segue `ACTIVE`;
/// erro de TRANSACAO leva a `ABORT_ONLY` e o `COMMIT` recusa.
#[test]
fn erro_de_instrucao_nao_derruba_a_transacao() {
    let dir = dir_temp("classes");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"ja existe"}"#,
    )
    .unwrap();

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    // Chave duplicada: erro de INSTRUCAO.
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"x"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "DUPLICADO");
    assert_eq!(
        crate::transacao::ClasseDoErro::do_erro(&e),
        crate::transacao::ClasseDoErro::Instrucao
    );
    // A transacao continua viva, e aceita trabalho.
    let r = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    assert_eq!(r.texto_ou("transaction_state", ""), "ACTIVE");
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"ok"}"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(1));
}

/// O teto de linhas e erro de TRANSACAO: `ABORT_ONLY`, e o `COMMIT`
/// **recusa** em vez de confirmar trabalho meio invalido.
#[test]
fn o_teto_leva_a_abort_only_e_o_commit_recusa() {
    let dir = dir_temp("teto");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.recursos.transacao_max_linhas = 3;
    let s = Servidor::novo(c).unwrap();
    let ses = sessao(7);
    base(&s, &ses);

    pede(&s, &ses, r#""op":"begin""#).unwrap();
    for i in 1..=3 {
        pede(
            &s,
            &ses,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes",
                       "linha":{{"id":{i},"nome":"c{i}"}}"#
            ),
        )
        .unwrap();
    }
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":4,"nome":"x"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA");
    assert!(e.to_string().contains("teto"), "{e}");

    // Em ABORT_ONLY o COMMIT recusa, e a recusa DIZ o que fazer.
    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA");
    assert!(e.to_string().contains("ROLLBACK"), "{e}");
    // Em ABORT_ONLY ate a LEITURA recusa, como no PostgreSQL(R): a
    // transacao esta suja e nao serve para mais nada.
    assert_eq!(
        pede(
            &s,
            &ses,
            r#""op":"varrer","database":"loja","tabela":"clientes""#
        )
        .unwrap_err()
        .nome(),
        "TRANSACAO_ABORTADA"
    );
    // E o ROLLBACK passa, que e o unico caminho que sobra.
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    // Nada foi gravado.
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
    let r = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    assert_eq!(r.texto_ou("transaction_state", ""), "IDLE");
}

/// Em `ABORT_ONLY`, voltar a um `SAVEPOINT` NAO conserta -- e a diferenca
/// deliberada para o PostgreSQL(R), com o motivo na resposta.
#[test]
fn o_savepoint_nao_resgata_uma_transacao_abortada() {
    let dir = dir_temp("resgate");
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.recursos.transacao_max_linhas = 1;
    let s = Servidor::novo(c).unwrap();
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"savepoint","nome":"p1""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"b"}"#,
    )
    .unwrap_err();
    let e = pede(&s, &ses, r#""op":"rollback_para","nome":"p1""#).unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA");
    assert!(e.to_string().contains("ROLLBACK"), "{e}");
}

// ------------------------------------------------------------- as travas

/// **O caso que matou o exclusivo-por-padrao.** Dois caixas em pedidos
/// diferentes nao disputam nada; na MESMA linha, disputam.
#[test]
fn dois_caixas_em_linhas_diferentes_nao_se_esbarram() {
    let dir = dir_temp("caixas");
    let s = servidor(&dir);
    let a = sessao(1);
    let b = sessao(2);
    base(&s, &a);
    for i in 1..=4 {
        pede(
            &s,
            &a,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"pedidos",
                       "linha":{{"id":{i},"nome":"p{i}"}}"#
            ),
        )
        .unwrap();
    }

    pede(&s, &a, r#""op":"begin""#).unwrap();
    pede(&s, &b, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &a,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
               "linha":{"id":1,"nome":"do caixa A"}"#,
    )
    .unwrap();
    // Linha DIFERENTE: passa sem esperar nada.
    pede(
        &s,
        &b,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":3,
               "linha":{"id":3,"nome":"do caixa B"}"#,
    )
    .unwrap();
    // MESMA linha: o conflito de verdade, com o nome de quem segura.
    let e = pede(
        &s,
        &b,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
               "linha":{"id":1,"nome":"tambem do B"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "EM_TRANSACAO");
    assert!(e.adianta_repetir(), "EM_TRANSACAO tem de pedir repeticao");
    assert!(e.to_string().contains("LOCK TIMEOUT"), "{e}");

    pede(&s, &a, r#""op":"commit""#).unwrap();
    pede(&s, &b, r#""op":"commit""#).unwrap();
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// `LOCK MODE EXCLUSIVE` cria o conflito que o `AUTO` evita -- e e por
/// isso que ele se PEDE, e nao e o padrao.
#[test]
fn o_modo_exclusivo_barra_ate_linha_diferente() {
    let dir = dir_temp("exclusivo");
    let s = servidor(&dir);
    let a = sessao(1);
    let b = sessao(2);
    base(&s, &a);
    for i in 1..=4 {
        pede(
            &s,
            &a,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"pedidos",
                       "linha":{{"id":{i},"nome":"p{i}"}}"#
            ),
        )
        .unwrap();
    }
    pede(
        &s,
        &a,
        r#""op":"begin","database":"loja","scope":["pedidos"],"lock_mode":"EXCLUSIVE""#,
    )
    .unwrap();
    pede(&s, &b, r#""op":"begin""#).unwrap();
    let e = pede(
        &s,
        &b,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":3,
               "linha":{"id":3,"nome":"do B"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "EM_TRANSACAO");
    pede(&s, &a, r#""op":"rollback""#).unwrap();
    // Solta a exclusiva, o outro entra.
    pede(
        &s,
        &b,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":3,
               "linha":{"id":3,"nome":"agora vai"}"#,
    )
    .unwrap();
    pede(&s, &b, r#""op":"commit""#).unwrap();
}

/// Uma escrita COMUM -- sem `BEGIN` -- respeita a trava de quem tem, e
/// recebe `EM_TRANSACAO` com `repetir: true` em vez de esperar.
#[test]
fn a_escrita_comum_respeita_a_trava_e_nao_espera() {
    let dir = dir_temp("comum");
    let s = servidor(&dir);
    let a = sessao(1);
    let b = sessao(2);
    base(&s, &a);
    pede(
        &s,
        &a,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"nome":"p1"}"#,
    )
    .unwrap();

    pede(&s, &a, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &a,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
               "linha":{"id":1,"nome":"na transacao"}"#,
    )
    .unwrap();
    let comeco = Instant::now();
    let e = pede(
        &s,
        &b,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
               "linha":{"id":1,"nome":"por fora"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "EM_TRANSACAO");
    assert!(
        comeco.elapsed() < Duration::from_millis(200),
        "a escrita comum NAO espera -- ela recusa na hora"
    );
    // Outra linha da mesma tabela continua passando: a trava e de LINHA.
    pede(
        &s,
        &a,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":9,"nome":"x"}"#,
    )
    .unwrap();
    pede(&s, &a, r#""op":"rollback""#).unwrap();
}

// ------------------------------------------------------ o escopo declarado

/// `SCOPE MODE STRICT` recusa a tabela nao declarada; o `DYNAMIC` -- que e
/// o padrao -- a acolhe e ANOTA a expansao.
#[test]
fn estrito_recusa_e_dinamico_expande_avisando() {
    let dir = dir_temp("escopo");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);

    pede(
        &s,
        &ses,
        r#""op":"begin","database":"loja","scope":["clientes"],"scope_mode":"STRICT""#,
    )
    .unwrap();
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"nome":"x"}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("STRICT"), "{e}");
    assert!(e.to_string().contains("SCOPE"), "{e}");
    pede(&s, &ses, r#""op":"rollback""#).unwrap();

    // DYNAMIC e o padrao, e ele expande em vez de recusar.
    pede(
        &s,
        &ses,
        r#""op":"begin","database":"loja","scope":["clientes"]"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"nome":"x"}"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    let expandidas: Vec<&str> = r
        .campo("tabelas_expandidas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(Json::texto)
        .collect();
    assert_eq!(expandidas, vec!["loja/pedidos"]);
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
}

/// A ficha separa DECLARADO de EFETIVO, e o gatilho que grava noutra
/// tabela entra no efetivo -- porque ele alcanca de verdade.
#[test]
fn o_gatilho_entra_no_escopo_efetivo_e_a_fk_nao() {
    let dir = dir_temp("efetivo");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    // `auditoria` recebe do gatilho, e nao e declarada por ninguem.
    pede(
        &s,
        &ses,
        r#""op":"criar_tabela","database":"loja","tabela":"auditoria",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(40)"}]"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"sql","database":"loja",
               "texto":"CREATE TRIGGER reg AFTER INSERT ON clientes FOR EACH ROW BEGIN INSERT INTO auditoria (id, nome) VALUES (NEW.id, 'entrou'); END""#,
    )
    .unwrap();

    pede(
        &s,
        &ses,
        r#""op":"begin","database":"loja","scope":["clientes"]"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    let declaradas: Vec<&str> = r
        .campo("tabelas_declaradas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(Json::texto)
        .collect();
    let efetivas: Vec<&str> = r
        .campo("tabelas_efetivas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(Json::texto)
        .collect();
    assert_eq!(declaradas, vec!["loja/clientes"]);
    assert_eq!(efetivas, vec!["loja/auditoria", "loja/clientes"]);
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
}

/// Vence a transacao desta ligacao SEM dormir: poe o `expira_ms` no
/// passado e devolve o controle.
///
/// A primeira versao destes dois testes abria com `TIMEOUT 1ms` e dormia
/// 30 ms. Ela reprovou de verdade numa rodada carregada, e nao por
/// defeito: com a bateria inteira rodando em paralelo, a PRIMEIRA insercao
/// -- a que precisa passar -- ja chegava depois do milissegundo, e o teste
/// morria na linha errada. Prazo medido em relogio de parede e corrida, e
/// corrida em teste e ruido que gasta a confianca da bateria toda.
///
/// Mover o relogio da transacao prova a MESMA coisa e nao tem corrida: o
/// caminho exercitado continua sendo o de producao (a varredura ve a
/// vencida, o gestor a encerra, o dono recebe o erro com o numero).
fn vencer_agora(s: &Servidor, ligacao: u64) {
    let mut t = s.transacoes.travar();
    t.de_mut(ligacao).unwrap().expira_ms = crate::agora_ms() - 1;
}

/// O prazo da transacao estoura e **quem encerra e o gestor**: a transacao
/// vai para `ABORT_ONLY`, as travas saem, e a proxima operacao recebe o
/// erro com o NUMERO do prazo. Nenhuma thread e morta.
#[test]
fn o_prazo_estourado_reverte_e_solta_as_travas() {
    let dir = dir_temp("prazo");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin","timeout":"10s""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    assert_eq!(s.travas.travar().quantas(), 1);
    vencer_agora(&s, 7);

    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"b"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA");
    assert!(e.to_string().contains("TIMEOUT"), "{e}");
    // As travas ja sairam.
    assert_eq!(s.travas.travar().quantas(), 0);
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
}

/// Pedido 539: o COMMIT depois do prazo NAO grava. Ele e operacao de
/// controle, e o portao deixa as de controle passarem sem olhar o prazo;
/// sem outra conexao varrendo, a lista vencida ia inteira para o disco
/// (medido pelo papel C: COMMITTED 600 ms depois de um prazo de 200 ms).
/// O que se confere e o DADO -- zero linhas -- e a porta: as travas saem
/// e o ROLLBACK fecha, como no prazo estourado na instrucao.
#[test]
fn o_commit_depois_do_prazo_nao_grava() {
    let dir = dir_temp("prazo-commit");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin","timeout":"10s""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    vencer_agora(&s, 7);

    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA", "{e}");
    assert!(e.to_string().contains("TIMEOUT"), "{e}");
    assert!(!e.adianta_repetir(), "{e}");
    // Lido por OUTRA conexao: a desta esta em ABORT_ONLY e so sai pelo
    // ROLLBACK.
    assert_eq!(
        quantas(&s, &sessao(8), "clientes"),
        0,
        "o COMMIT vencido gravou"
    );
    assert_eq!(s.travas.travar().quantas(), 0);
    // O COMMIT de novo continua recusado, e o ROLLBACK fecha.
    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA", "{e}");
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
}

/// Pedido 559, (a): a varredura do prazo NAO mexe na transacao que esta
/// no COMMIT. A varredura roda no `begin` de OUTRA conexao, sem a trava de
/// dados, e o COMMIT roda com ela na mao: sem o filtro, a vencida no meio
/// do COMMIT ia para `ABORT_ONLY` e soltava as travas de quem ainda estava
/// gravando -- outra transacao pegava a linha no meio da passada.
///
/// O instante e montado no registro, e nao por corrida: a lista ja saiu
/// (`COMMITTING`) e o prazo venceu nesse meio -- o mesmo desenho do
/// `vencer_agora`.
#[test]
fn a_varredura_do_prazo_nao_mexe_na_transacao_que_esta_confirmando() {
    let dir = dir_temp("prazo-confirmando");
    let s = servidor(&dir);
    let t1 = sessao(7);
    let outra = sessao(8);
    base(&s, &t1);
    pede(&s, &t1, r#""op":"begin","timeout":"10s""#).unwrap();
    pede(
        &s,
        &t1,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    let travas = s.travas.travar().quantas();
    assert!(travas > 0);
    {
        let mut t = s.transacoes.travar();
        let tx = t.de_mut(7).unwrap();
        tx.estado = crate::transacao::Estado::Confirmando;
        tx.expira_ms = crate::agora_ms() - 1;
    }
    // A varredura: o `begin` de outra conexao.
    pede(&s, &outra, r#""op":"begin""#).unwrap();
    pede(&s, &outra, r#""op":"rollback""#).unwrap();
    let estado = s.transacoes.travar().de(7).unwrap().estado;
    assert_eq!(
        estado,
        crate::transacao::Estado::Confirmando,
        "a varredura abortou a transacao no meio do COMMIT"
    );
    assert_eq!(
        s.travas.travar().quantas(),
        travas,
        "a varredura soltou as travas de quem esta gravando"
    );
    // Fora do instante montado, a transacao sai como sempre.
    s.transacoes.travar().de_mut(7).unwrap().estado = crate::transacao::Estado::Ativa;
    pede(&s, &t1, r#""op":"rollback""#).unwrap();
    assert_eq!(s.travas.travar().quantas(), 0);
}

/// Pedido 559, (b): devolver a lista ao fim de um COMMIT recusado NAO
/// desfaz um `ABORT_ONLY` que chegou no meio. O `devolver_a_lista` punha
/// `ACTIVE` sem condicao: a transacao encerrada pelo gestor -- travas ja
/// soltas -- voltava ativa, com a lista, e o COMMIT seguinte gravava sem
/// trava nenhuma.
#[test]
fn devolver_a_lista_nao_desfaz_o_abort_only() {
    let dir = dir_temp("devolver-abort-only");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin","timeout":"10s""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    // O COMMIT tirou a lista; no meio dele, o gestor encerrou a transacao.
    let escritas = {
        let mut t = s.transacoes.travar();
        let tx = t.de_mut(7).unwrap();
        tx.estado = crate::transacao::Estado::Confirmando;
        std::mem::take(&mut tx.escritas)
    };
    s.abortar_soltando(7, |_| "encerrada no meio do COMMIT (teste)".into())
        .unwrap();
    s.devolver_a_lista(7, escritas);
    {
        let t = s.transacoes.travar();
        let tx = t.de(7).unwrap();
        assert_eq!(
            tx.estado,
            crate::transacao::Estado::AbortOnly,
            "a lista devolvida desfez o ABORT_ONLY"
        );
        assert!(
            tx.escritas.is_empty(),
            "a lista voltou a transacao encerrada"
        );
    }
    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA", "{e}");
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &ses, "clientes"), 0, "gravou sem trava");
}

/// Tabela declarada no `SCOPE` que não existe **recusa na abertura**.
///
/// Sem esta conferência o engano ficava a duas mensagens de distância da
/// causa: a trava ia para uma chave que não aponta para nada, e o `STRICT`
/// recusava depois a tabela CERTA dizendo que ela não estava no escopo.
#[test]
fn escopo_com_tabela_que_nao_existe_recusa_na_abertura() {
    let dir = dir_temp("escopo-errado");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    let e = pede(
        &s,
        &ses,
        r#""op":"begin","database":"loja","scope":["clientes","pediditens"]"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "NAO_ENCONTRADO");
    assert!(e.to_string().contains("pediditens"), "{e}");
    assert!(e.to_string().contains("SCOPE"), "{e}");
    // E a transacao NAO ficou meio aberta: nenhuma trava presa, nenhum
    // contador subido.
    assert_eq!(s.travas.travar().quantas(), 0);
    assert_eq!(s.transacoes_abertas.load(Ordering::SeqCst), 0);
    let r = pede(&s, &ses, r#""op":"transacao""#).unwrap();
    assert_eq!(r.texto_ou("transaction_state", ""), "IDLE");
}

/// A transação vencida **não some** quando OUTRA conexão varre: ela vira
/// `ABORT_ONLY`, solta as travas, e espera o dono para lhe dizer o número
/// do prazo.
///
/// Descartá-la ali soltava as travas — que é o que importa para os outros
/// — e tirava do dono a resposta: a próxima operação dele receberia «esta
/// conexão não tem transação aberta», sem o prazo dentro.
#[test]
fn a_vencida_varrida_por_outro_ainda_explica_ao_dono() {
    let dir = dir_temp("varrida");
    let s = servidor(&dir);
    let dono = sessao(1);
    let outro = sessao(2);
    base(&s, &dono);
    pede(&s, &dono, r#""op":"begin","timeout":"10s""#).unwrap();
    pede(
        &s,
        &dono,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    assert_eq!(s.travas.travar().quantas(), 1);
    vencer_agora(&s, 1);

    // OUTRA conexao varre -- e e o `begin` dela que chama a varredura.
    pede(&s, &outro, r#""op":"begin""#).unwrap();
    // As travas do vencido sairam: e o que importa para quem esperava.
    assert_eq!(
        s.travas.travar().quantas(),
        0,
        "a varredura tem de soltar as travas da vencida"
    );
    // E o DONO ainda recebe a explicacao, com o numero do prazo.
    let e = pede(
        &s,
        &dono,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"b"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "TRANSACAO_ABORTADA");
    assert!(e.to_string().contains("TIMEOUT"), "{e}");
    pede(&s, &dono, r#""op":"rollback""#).unwrap();
    pede(&s, &outro, r#""op":"rollback""#).unwrap();
    assert_eq!(quantas(&s, &dono, "clientes"), 0);
}

/// Os tres sinonimos de abertura, e a abertura declarada inteira pelo SQL.
#[test]
fn o_sql_abre_com_escopo_e_prazos() {
    let dir = dir_temp("sql");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    let r = pede(
        &s,
        &ses,
        r#""op":"sql","database":"loja",
               "texto":"BEGIN TRANSACTION SCOPE (clientes, pedidos) TIMEOUT 5s LOCK TIMEOUT 500ms LOCK MODE AUTO""#,
    )
    .unwrap();
    let dentro = r.campo("resultado").unwrap();
    assert_eq!(dentro.texto_ou("transaction_state", ""), "ACTIVE");
    assert_eq!(dentro.texto_ou("lock_mode", ""), "AUTO");
    assert_eq!(
        dentro.campo("lock_timeout_ms").and_then(Json::inteiro),
        Some(500)
    );
    let efetivas = dentro
        .campo("tabelas_efetivas")
        .and_then(Json::lista)
        .unwrap();
    assert_eq!(efetivas.len(), 2);

    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"sql","texto":"COMMIT""#).unwrap();
    assert_eq!(
        r.campo("resultado")
            .unwrap()
            .texto_ou("transaction_state", ""),
        "COMMITTED"
    );
    assert_eq!(quantas(&s, &ses, "clientes"), 1);
}

/// DDL dentro de transacao **recusa**, e nao confirma pelas costas.
///
/// O MySQL(R) e o Oracle confirmam a transacao aberta quando chega um DDL,
/// e quem escreveu `BEGIN; …; CREATE TABLE; ROLLBACK` acha que desfez sem
/// ter desfeito. Recusar e a resposta honesta enquanto o DDL nao for
/// transacional.
#[test]
fn o_ddl_recusa_em_vez_de_confirmar_pelas_costas() {
    let dir = dir_temp("ddl");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    let e = pede(
        &s,
        &ses,
        r#""op":"excluir_tabela","database":"loja","tabela":"pedidos""#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("nao entra em transacao"), "{e}");
    assert!(e.to_string().contains("ROLLBACK"), "{e}");
    // A transacao continua viva e o trabalho dela continua la.
    let r = pede(&s, &ses, r#""op":"commit""#).unwrap();
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(1));
    // E a tabela NAO foi apagada.
    assert!(pede(
        &s,
        &ses,
        r#""op":"esquema","database":"loja","tabela":"pedidos""#
    )
    .is_ok());
}

/// A queda da conexao desfaz a transacao -- a PRIMEIRA rede.
#[test]
fn a_queda_da_ligacao_desfaz_e_solta() {
    let dir = dir_temp("queda");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    assert_eq!(s.travas.travar().quantas(), 1);
    // E o que o laco da conexao faz na saida, por qualquer caminho.
    s.soltar_transacao_da_ligacao(7);
    assert_eq!(s.travas.travar().quantas(), 0);
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
    assert_eq!(s.transacoes_abertas.load(Ordering::SeqCst), 0);
}

/// Duas transacoes que anexam na mesma tabela disputam o FIM dela -- e
/// disputam de verdade, porque o proximo slot e um so.
#[test]
fn duas_transacoes_que_anexam_disputam_o_fim_da_tabela() {
    let dir = dir_temp("fim");
    let s = servidor(&dir);
    let a = sessao(1);
    let b = sessao(2);
    base(&s, &a);
    pede(&s, &a, r#""op":"begin""#).unwrap();
    pede(&s, &b, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &a,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    let e = pede(
        &s,
        &b,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"b"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "EM_TRANSACAO");
    assert!(e.to_string().contains("fim de"), "{e}");
    // A solta, B entra e o rowid dele e o seguinte -- nao o mesmo.
    pede(&s, &a, r#""op":"commit""#).unwrap();
    let r = pede(
        &s,
        &b,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"b"}"#,
    )
    .unwrap();
    assert_eq!(r.campo("rowid").and_then(Json::inteiro), Some(2));
    pede(&s, &b, r#""op":"commit""#).unwrap();
    assert_eq!(quantas(&s, &a, "clientes"), 2);
}

/// **A fresta que a revisao achou, fechada pelos dois lados.**
///
/// Uma escrita COMUM nao pode anexar enquanto uma transacao segura o fim
/// da tabela. Se pudesse, o rowid que a transacao prometeu passaria a ser
/// de outra linha — e o estrago nao seria o erro no `COMMIT` (esse e
/// visivel): seria a RECUPERACAO encontrar o slot ocupado pela linha do
/// outro, trata-lo como "ja aplicado" e descartar a nossa em silencio.
///
/// Por isso a trava do fim e tomada **antes** da trava de dados, e nao
/// depois de o rowid ser calculado.
#[test]
fn escrita_comum_nao_anexa_enquanto_a_transacao_segura_o_fim() {
    let dir = dir_temp("fresta");
    let s = servidor(&dir);
    let a = sessao(1);
    let b = sessao(2);
    base(&s, &a);

    pede(&s, &a, r#""op":"begin""#).unwrap();
    let r = pede(
        &s,
        &a,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"da tx"}"#,
    )
    .unwrap();
    let prometido = r.campo("rowid").and_then(Json::inteiro).unwrap();

    // A escrita comum, sem BEGIN nenhum, tem de ser BARRADA -- e sem
    // esperar, porque ela nao declarou prazo nenhum.
    let e = pede(
        &s,
        &b,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":9,"nome":"por fora"}"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "EM_TRANSACAO");
    assert!(e.to_string().contains("fim de"), "{e}");
    // O lote tambem anexa, e tambem e barrado.
    assert_eq!(
        pede(
            &s,
            &b,
            r#""op":"inserir_lote","database":"loja","tabela":"clientes",
                   "linhas":[{"id":10,"nome":"lote"}]"#,
        )
        .unwrap_err()
        .nome(),
        "EM_TRANSACAO"
    );

    // O commit encontra o slot que prometeu, e nao o de outro.
    pede(&s, &a, r#""op":"commit""#).unwrap();
    let l = pede(
        &s,
        &a,
        &format!(r#""op":"ler","database":"loja","tabela":"clientes","rowid":{prometido}"#),
    )
    .unwrap();
    assert_eq!(l.texto_ou("nome", ""), "da tx");
    // E agora que a trava saiu, a escrita comum passa.
    pede(
        &s,
        &b,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":9,"nome":"agora vai"}"#,
    )
    .unwrap();
}

/// Uma transacao abrange UM database, e a recusa diz por que.
#[test]
fn duas_bases_na_mesma_transacao_recusam() {
    let dir = dir_temp("duasbases");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"criar_database","database":"outra""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"criar_tabela","database":"outra","tabela":"t",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
    )
    .unwrap();
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"a"}"#,
    )
    .unwrap();
    let e = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"outra","tabela":"t","linha":{"id":1}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("two-phase commit"), "{e}");
    pede(&s, &ses, r#""op":"rollback""#).unwrap();
}

/// A recuperacao completa um commit que morreu no meio -- e ela e
/// idempotente, entao rodar de novo nao duplica nada.
#[test]
fn a_recuperacao_completa_o_commit_e_nao_duplica() {
    let dir = dir_temp("recuperar");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    // A marca escrita a mao, como se a passada tivesse morrido logo depois
    // de sincroniza-la: nada de dado no disco, e a intencao inteira la.
    let escritas: Vec<crate::transacao::Escrita> = (1..=5)
        .map(|i| crate::transacao::Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: crate::transacao::Acao::Inserir,
            rowid: i,
            linha: vec![
                Value::Int(i as i64),
                Value::Str(format!("c{i}")),
                Value::Bool(false),
            ],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        })
        .collect();
    crate::transacao::gravar_marca(&dir.join("loja"), 42, crate::agora_ms(), &escritas).unwrap();
    drop(s);

    // O arranque acha a marca e completa o commit.
    let s = servidor(&dir);
    assert_eq!(quantas(&s, &ses, "clientes"), 5);
    // A marca sumiu, e um segundo arranque nao duplica nada.
    drop(s);
    let s = servidor(&dir);
    assert_eq!(quantas(&s, &ses, "clientes"), 5);
}

/// Marca com o CRC quebrado e um commit que NUNCA COMECOU: ela e
/// descartada, e o disco continua como estava.
#[test]
fn marca_que_nao_confere_e_commit_que_nunca_comecou() {
    let dir = dir_temp("marca-ruim");
    let s = servidor(&dir);
    let ses = sessao(7);
    base(&s, &ses);
    let escritas = vec![crate::transacao::Escrita {
        database: "loja".into(),
        tabela: "clientes".into(),
        acao: crate::transacao::Acao::Inserir,
        rowid: 1,
        linha: vec![Value::Int(1), Value::Str("a".into()), Value::Bool(false)],
        linha_antiga: Vec::new(),
        motivo: String::new(),
        cascata_na_lista: false,
        elo_do_empilhar: false,
        elo_da_cascata: false,
    }];
    let caminho =
        crate::transacao::gravar_marca(&dir.join("loja"), 7, crate::agora_ms(), &escritas).unwrap();
    let mut b = std::fs::read(&caminho).unwrap();
    let meio = b.len() - 6;
    b[meio] ^= 0xFF;
    std::fs::write(&caminho, &b).unwrap();
    drop(s);

    let s = servidor(&dir);
    assert_eq!(quantas(&s, &ses, "clientes"), 0);
    assert!(!caminho.exists(), "a marca ruim tem de sair do disco");
}

// ------------------------------------- pedido 426: DEPOIS da marca

/// As marcas `.tx` que sobraram no diretorio do database.
fn marcas_em(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with(crate::transacao::PREFIXO)
                        && n.ends_with(crate::transacao::EXTENSAO)
                })
                .count()
        })
        .unwrap_or(0)
}

/// BEGIN, uma linha em `clientes` e uma em `pedidos`, e o COMMIT com a
/// passada quebrando antes da SEGUNDA escrita -- ja DEPOIS da marca, com
/// `clientes` gravada. Devolve a resposta como o cliente a le: o corpo do
/// `Ok`, ou a resposta de erro montada pelo mesmo `resposta_erro` do
/// protocolo (e ela que carrega o `repetir`).
fn commit_que_quebra(s: &Arc<Servidor>, ses: &Sessao, congelando: bool) -> Json {
    base(s, ses);
    pede(s, ses, r#""op":"begin","database":"loja""#).unwrap();
    pede(
        s,
        ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        s,
        ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"nome":"p"}"#,
    )
    .unwrap();
    s.passada_quebra_congela.store(congelando, Ordering::SeqCst);
    s.passada_quebra_na_escrita.store(2, Ordering::SeqCst);
    let r = pede(s, ses, r#""op":"commit""#);
    s.passada_quebra_na_escrita.store(0, Ordering::SeqCst);
    match r {
        Ok(j) => j,
        Err(e) => s.resposta_erro("commit", &e, 0),
    }
}

/// **426 (a)** -- a passada que quebra DEPOIS da marca nao deixa uma
/// tabela com a linha e a outra sem. A marca e o ponto de compromisso: o
/// que falta se completa para a FRENTE, na hora, com o mesmo codigo da
/// recuperacao do arranque.
#[test]
fn a_passada_que_quebra_depois_da_marca_nao_deixa_meia_transacao() {
    let dir = dir_temp("426-a");
    let s = servidor(&dir);
    let ses = sessao(4261);
    let r = commit_que_quebra(&s, &ses, false);
    let (c, p) = (quantas(&s, &ses, "clientes"), quantas(&s, &ses, "pedidos"));
    assert_eq!(
        (c, p),
        (1, 1),
        "(a) ATOMICIDADE: clientes com {c} linha(s), pedidos com {p} -- a \
             transacao confirmada ficou pela metade. COMMIT: {}",
        r.escrever()
    );
}

/// **426 (b)** -- nenhuma resposta de `COMMIT` manda repetir tendo gravado.
/// E a assertiva que faltava na prova (`commit-contra-ddl-4-motores.md`
/// §5 item 9), medida contra o DISCO e nao contra o campo `gravadas`: a
/// resposta de erro nem traz `gravadas`, e o estrago estava justamente
/// ali.
#[test]
fn o_commit_que_quebra_depois_da_marca_nao_manda_repetir() {
    let dir = dir_temp("426-b");
    let s = servidor(&dir);
    let ses = sessao(4262);
    let r = commit_que_quebra(&s, &ses, false);
    let aplicadas = quantas(&s, &ses, "clientes") + quantas(&s, &ses, "pedidos");
    assert!(
        !(r.booleano_ou("repetir", false) && aplicadas > 0),
        "(b) o COMMIT mandou REPETIR com {aplicadas} linha(s) da transacao ja \
             gravada(s) -- quem obedece duplica: {}",
        r.escrever()
    );
    assert_eq!(
        r.texto_ou("transaction_state", ""),
        "COMMITTED",
        "(b) depois da marca a transacao esta confirmada, e a resposta tem de \
             dizer isso: {}",
        r.escrever()
    );
}

/// **426 (c)** -- a recuperacao do braco de erro RODA: a marca sai do disco
/// na hora, sem esperar o proximo arranque. Com o defeito, o `travar_dados`
/// do braco de erro batia na propria trava ainda viva, o `if let Ok` engolia
/// a recusa, e a transacao que o cliente viu falhar so aparecia aplicada
/// depois de reiniciar.
#[test]
fn a_recuperacao_do_braco_de_erro_roda_de_verdade() {
    let dir = dir_temp("426-c");
    let s = servidor(&dir);
    let ses = sessao(4263);
    let r = commit_que_quebra(&s, &ses, false);
    let sobraram = marcas_em(&dir.join("loja"));
    assert_eq!(
        sobraram,
        0,
        "(c) {sobraram} marca(s) .tx sobrando depois da resposta: a \
             recuperacao do braco de erro nao rodou. COMMIT: {}",
        r.escrever()
    );
    assert!(
        r.campo("completando").is_none(),
        "a recuperacao completou na hora e a resposta diz que ainda falta: {}",
        r.escrever()
    );
    assert!(
        r.texto_ou("aviso", "").contains("completou na hora"),
        "a passada quebrou e a resposta nao diz: {}",
        r.escrever()
    );
}

/// Uma escrita de `clientes`, para as marcas escritas a mao abaixo.
fn escrita_de_clientes(id: i64) -> Vec<crate::transacao::Escrita> {
    vec![crate::transacao::Escrita {
        database: "loja".into(),
        tabela: "clientes".into(),
        acao: crate::transacao::Acao::Inserir,
        rowid: id as u64,
        linha: vec![Value::Int(id), Value::Str("a".into()), Value::Bool(false)],
        linha_antiga: Vec::new(),
        motivo: String::new(),
        cascata_na_lista: false,
        elo_do_empilhar: false,
        elo_da_cascata: false,
    }]
}

/// **Pedido 503, (1a)**: a marca que o ARRANQUE nao consegue ler (erro de
/// E/S) nao e «commit que nunca comecou». Ela fica no disco e o servidor
/// NAO sobe; com o disco de volta, o arranque a completa. Com o defeito,
/// o erro caia no «descartada» e a transacao confirmada sumia calada.
#[test]
fn marca_que_nao_se_le_no_arranque_fica_e_o_servidor_nao_sobe() {
    let dir = dir_temp("503-1a");
    let s = servidor(&dir);
    let ses = sessao(5031);
    base(&s, &ses);
    let caminho = crate::transacao::gravar_marca(
        &dir.join("loja"),
        9,
        crate::agora_ms(),
        &escrita_de_clientes(1),
    )
    .unwrap();
    drop(s);

    crate::transacao::falhar_a_proxima_leitura_de_teste();
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let e = Servidor::novo(c)
        .err()
        .expect("subiu com uma marca que nao se leu");
    assert!(e.to_string().contains("NAO subiu"), "{e}");
    assert!(caminho.exists(), "a marca que nao se leu foi APAGADA");

    let s = servidor(&dir);
    assert_eq!(
        quantas(&s, &ses, "clientes"),
        1,
        "o arranque nao a completou"
    );
    assert!(!caminho.exists());
}

/// **Pedido 503, (1b)**: no braco de erro do 426, a marca que ESTE
/// processo gravou e que nao se rele fica no disco -- e a resposta que diz
/// «se completa na proxima recuperacao» passa a ser verdade. Com o
/// defeito, a marca saia e a transacao ficava pela metade para sempre.
#[test]
fn braco_do_426_com_marca_que_nao_se_le_deixa_a_marca_para_o_arranque() {
    let dir = dir_temp("503-1b");
    let s = servidor(&dir);
    let ses = sessao(5032);
    crate::transacao::falhar_a_proxima_leitura_de_teste();
    let r = commit_que_quebra(&s, &ses, false);
    assert_eq!(
        marcas_em(&dir.join("loja")),
        1,
        "a marca que nao se releu saiu do disco: {}",
        r.escrever()
    );
    drop(s);
    let s = servidor(&dir);
    let (c, p) = (quantas(&s, &ses, "clientes"), quantas(&s, &ses, "pedidos"));
    assert_eq!((c, p), (1, 1), "o arranque nao completou a transacao");
}

/// O filho do teste de baixo: um processo SEM servidor, entao sem o
/// gancho que derruba -- o caso da biblioteca embutida.
#[test]
#[ignore = "roda so dentro de a_marca_que_nao_se_confirma_sai_do_disco"]
fn filho_503_3() {
    let dir = std::path::PathBuf::from(std::env::var("PHX_503_DIR").unwrap());
    phxsql_store::sincronia::falha_de_teste::armar(
        &dir,
        phxsql_store::sincronia::falha_de_teste::Onde::Fsync,
        1,
    );
    let r = crate::transacao::gravar_marca(&dir, 3, crate::agora_ms(), &escrita_de_clientes(1));
    assert!(r.is_err(), "o fsync recusado da marca respondeu Ok");
    let sobrou = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(sobrou, 0, "a marca que nao se confirmou ficou no disco");
}

/// **Pedido 503, (3)**: o `fsync` da marca que falha nao deixa a marca
/// INTEIRA no disco enquanto o COMMIT responde «abortada» -- o arranque
/// seguinte a aplicaria por cima de gravacao mais nova. Num processo
/// limpo, porque o gancho do servidor derrubaria este.
#[test]
fn a_marca_que_nao_se_confirma_sai_do_disco() {
    let dir = DirTemp::novo("503-3");
    let st = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "servidor::testes_transacoes::filho_503_3",
        ])
        .env("PHX_503_DIR", &dir.0)
        .output()
        .unwrap();
    let saida = String::from_utf8_lossy(&st.stdout);
    assert!(
        st.status.success() && saida.contains("1 passed"),
        "{saida}{}",
        String::from_utf8_lossy(&st.stderr)
    );
}

/// **426, a quebra de ACESSO antes de qualquer byte da lista** (a
/// primeira escrita bate no congelamento): a marca SAI, nada foi
/// aplicado, a transacao volta a `ACTIVE` com a lista -- e ai, e so ai, o
/// `repetir: true` do `EM_MIGRACAO` diz a verdade. O segundo COMMIT grava
/// as duas.
#[test]
fn a_quebra_de_acesso_antes_de_qualquer_byte_devolve_a_transacao() {
    let dir = dir_temp("426-zero");
    let s = servidor(&dir);
    let ses = sessao(4265);
    base(&s, &ses);
    pede(&s, &ses, r#""op":"begin","database":"loja""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"nome":"p"}"#,
    )
    .unwrap();
    s.passada_quebra_na_escrita.store(1, Ordering::SeqCst);
    let e = pede(&s, &ses, r#""op":"commit""#);
    s.passada_quebra_na_escrita.store(0, Ordering::SeqCst);
    let e = e.expect_err("a primeira escrita quebrou: o COMMIT nao aconteceu");
    // Contado por OUTRA conexao: a desta transacao enxerga as proprias
    // escritas pendentes, e mediria a lista em vez do disco.
    let fora = sessao(4275);
    let aplicadas = quantas(&s, &fora, "clientes") + quantas(&s, &fora, "pedidos");
    assert_eq!(aplicadas, 0, "a quebra veio antes de qualquer byte: {e}");
    assert!(
        e.adianta_repetir(),
        "nada aplicado e ACESSO: repetir e verdade -- {e}"
    );
    assert_eq!(
        marcas_em(&dir.join("loja")),
        0,
        "a marca de quem nao aconteceu ficou"
    );
    let r = pede(&s, &ses, r#""op":"commit""#)
        .expect("a transacao tinha de voltar a ACTIVE com a lista");
    assert_eq!(r.campo("gravadas").and_then(Json::inteiro), Some(2));
    assert_eq!(
        quantas(&s, &ses, "clientes") + quantas(&s, &ses, "pedidos"),
        2
    );
}

/// **426, a chave que so se confere na passada: filha ANTES do pai.** A
/// recusa ja existia (`p0_filha_antes_do_pai_no_commit_ainda_recusa`), e
/// conferia so o VEREDITO -- a marca ficava no disco, e o arranque
/// seguinte reaplicava: a filha continuava impossivel, o pai entrava.
/// Uma transacao que o cliente viu RECUSADA aparecia pela metade depois
/// de reiniciar. Com nada aplicado e erro do DADO, a marca sai.
#[test]
fn a_filha_antes_do_pai_nao_deixa_marca_para_o_arranque() {
    let dir = dir_temp("426-p0-marca");
    let s = servidor(&dir);
    let ses = sessao(4266);
    base_com_fk(&s, &ses);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":1}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    assert_eq!(e.nome(), "INTEGRIDADE", "{e}");
    let sobraram = marcas_em(&dir.join("loja"));
    drop(s);
    let s = servidor(&dir);
    let depois = (quantas(&s, &ses, "clientes"), quantas(&s, &ses, "pedidos"));
    assert_eq!(
        (sobraram, depois),
        (0, (0, 0)),
        "(marcas sobrando, (clientes, pedidos) depois do arranque): a transacao \
             RECUSADA apareceu depois de reiniciar"
    );
}

/// **426, o CINTO da passada:** erro do DADO no MEIO da lista, com a
/// pre-conferencia do pedido 448 DESLIGADA -- e o unico jeito de chegar a
/// esta linha da tabela do `depois_da_marca` sem defeito no motor. A parte
/// anterior ja esta gravada e nao se desfaz. O que o cinto garante: a
/// resposta DIZ quantas ficaram, manda nao repetir e diz que e defeito do
/// motor, `repetir` e falso, e o arranque nao muda o passado -- a marca
/// sai, e o pai que vinha DEPOIS da filha orfa nao aparece sozinho.
///
/// Era o `o_erro_do_dado_no_meio_da_lista_...`, e mudou de lado
/// conscientemente: com a pre-conferencia LIGADA a mesma lista recusa com
/// zero gravado (`a_filha_orfa_no_meio_recusa_antes_da_marca_...`).
#[test]
fn sem_a_pre_conferencia_o_cinto_da_passada_diz_o_que_ficou() {
    let dir = dir_temp("426-meio");
    let s = servidor(&dir);
    let ses = sessao(4267);
    base_com_fk(&s, &ses);
    s.pre_conferencia_desligada.store(true, Ordering::SeqCst);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":2}"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"Bia"}"#,
    )
    .unwrap();
    let e = pede(&s, &ses, r#""op":"commit""#).unwrap_err();
    let texto = e.to_string();
    assert_eq!(e.nome(), "INTEGRIDADE", "{texto}");
    assert!(
        !e.adianta_repetir(),
        "parte gravada e mandando repetir: {texto}"
    );
    assert!(
        texto.contains("JA ESTAO gravadas")
            && texto.contains("clientes rowid 1")
            && texto.contains("DEFEITO DO MOTOR"),
        "a resposta tinha de dizer o que ficou gravado, e que e defeito: {texto}"
    );
    let antes = (quantas(&s, &ses, "clientes"), quantas(&s, &ses, "pedidos"));
    let sobraram = marcas_em(&dir.join("loja"));
    drop(s);
    let s = servidor(&dir);
    let depois = (quantas(&s, &ses, "clientes"), quantas(&s, &ses, "pedidos"));
    assert_eq!(
        (sobraram, antes, depois),
        (0, (1, 0), (1, 0)),
        "(marcas, antes, depois do arranque): o arranque mudou o passado"
    );
}

/// **426, o lado que a recuperacao da HORA nao vence** -- a tabela continua
/// congelada. A marca FICA no disco (apaga-la trocaria «completa no proximo
/// arranque» por «perdida para sempre»), a resposta diz `COMMITTED` com
/// `completando: true`, e o arranque completa.
#[test]
fn a_quebra_que_a_recuperacao_nao_vence_fica_na_marca_ate_o_arranque() {
    let dir = dir_temp("426-pendente");
    let s = servidor(&dir);
    let ses = sessao(4264);
    let r = commit_que_quebra(&s, &ses, true);
    let sobraram = marcas_em(&dir.join("loja"));
    // Solta o congelamento ANTES de qualquer outra coisa: o `Servidor` pode
    // ter threads de fundo segurando um `Arc`, e o `drop(s)` sozinho nao
    // garantiria que ele saisse.
    let posse = s.passada_quebra_congelando.lock().unwrap().take();
    assert!(posse.is_some(), "a quebra de teste nao congelou nada");
    drop(posse);
    assert_eq!(
        r.texto_ou("transaction_state", ""),
        "COMMITTED",
        "a marca esta no disco: a transacao esta confirmada. COMMIT: {}",
        r.escrever()
    );
    assert!(
        r.booleano_ou("completando", false),
        "a resposta tinha de dizer que a aplicacao ficou pendente: {}",
        r.escrever()
    );
    assert!(
        !r.booleano_ou("repetir", false),
        "confirmada e mandando repetir: {}",
        r.escrever()
    );
    assert_eq!(
        sobraram, 1,
        "a marca de uma transacao confirmada e pendente saiu do disco -- o \
             arranque nao tem mais como completa-la"
    );
    drop(s);
    let s = servidor(&dir);
    assert_eq!(
        (quantas(&s, &ses, "clientes"), quantas(&s, &ses, "pedidos")),
        (1, 1),
        "o arranque tinha de completar a transacao que ficou na marca"
    );
    assert_eq!(marcas_em(&dir.join("loja")), 0);
}

mod pre_conferencia_448;

mod revisao_do_dba_448;

mod pedido_514;

mod integridade_na_transacao;
