//! **Pedido 448: a pre-conferencia da lista inteira ANTES da marca.**
//!
//! Cada prova confere o DISCO, e nao so o veredito -- esta casa ja pagou
//! por prova que conferia depois do dano: a contagem viva e os slots das
//! duas tabelas, as marcas `*.tx`, e de novo depois de reiniciar. O
//! retrato de antes da transacao tem de ser o retrato de depois da recusa.
use super::*;

/// (clientes vivos, pedidos vivos, slots de clientes, slots de
/// pedidos, marcas no disco). Slot conta tambem a linha que nasceu e
/// morreu: e ele que acusa «gravou e depois nao apareceu».
fn retrato(s: &Arc<Servidor>, ses: &Sessao, dir: &std::path::Path) -> (u64, u64, u64, u64, usize) {
    (
        quantas(s, ses, "clientes"),
        quantas(s, ses, "pedidos"),
        slots(s, ses, "clientes"),
        slots(s, ses, "pedidos"),
        marcas_em(&dir.join("loja")),
    )
}

fn inserir(s: &Arc<Servidor>, ses: &Sessao, tabela: &str, linha: &str) {
    pede(
        s,
        ses,
        &format!(r#""op":"inserir","database":"loja","tabela":"{tabela}","linha":{linha}"#),
    )
    .unwrap();
}

/// A recusa que o 448 promete: erro do DADO, `repetir` falso, a
/// posicao e a tabela nomeadas, a transacao encerrada -- e o disco
/// igual ao de antes, inclusive depois de reiniciar.
fn confere_a_recusa(
    s: Arc<Servidor>,
    ses: &Sessao,
    dir: &std::path::Path,
    antes: (u64, u64, u64, u64, usize),
    posicao: &str,
    tabela: &str,
    nome_do_erro: &str,
) {
    let r = pede(&s, ses, r#""op":"commit""#);
    let depois = retrato(&s, ses, dir);
    let e = match r {
        Ok(j) => panic!(
            "o COMMIT tinha de recusar ANTES da marca, e respondeu {} \
                     -- (clientes, pedidos, slots c, slots p, marcas) antes {antes:?}, \
                     depois {depois:?}",
            j.escrever()
        ),
        Err(e) => e,
    };
    let texto = e.to_string();
    assert_eq!(
        depois, antes,
        "(clientes, pedidos, slots c, slots p, marcas): a recusa deixou coisa \
                 gravada -- {texto}"
    );
    assert_eq!(e.nome(), nome_do_erro, "{texto}");
    assert!(
        !e.adianta_repetir(),
        "recusa do dado mandando repetir: {texto}"
    );
    assert!(
        texto.contains(posicao) && texto.contains(tabela),
        "a recusa tinha de nomear a posicao ({posicao}) e a tabela ({tabela}): {texto}"
    );
    assert!(
        texto.contains("ANTES da marca"),
        "a recusa tinha de dizer que nada foi gravado: {texto}"
    );
    let estado = pede(&s, ses, r#""op":"transacao""#).unwrap();
    assert_eq!(
        estado.texto_ou("transaction_state", ""),
        "IDLE",
        "a transacao recusada no COMMIT tinha de sair"
    );
    drop(s);
    let s = servidor(dir);
    assert_eq!(
        retrato(&s, ses, dir),
        antes,
        "o arranque mudou o passado depois de uma recusa sem marca"
    );
}

/// **A prova do pedido:** `[mae, filha-orfa, outra]`. Hoje a mae fica
/// gravada e a passada para na filha; com a pre-conferencia, zero.
/// Substitui `o_erro_do_dado_no_meio_da_lista_...`, que aceitava a
/// metade -- e muda de lado conscientemente, como o parecer manda.
#[test]
fn a_filha_orfa_no_meio_recusa_antes_da_marca_com_zero_gravado() {
    let dir = dir_temp("448-meio");
    let s = servidor(&dir);
    let ses = sessao(4481);
    base_com_fk(&s, &ses);
    let antes = retrato(&s, &ses, &dir);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":1,"cliente_id":2}"#);
    inserir(&s, &ses, "clientes", r#"{"id":2,"nome":"Bia"}"#);
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 2 de 3",
        "pedidos",
        "INTEGRIDADE",
    );
}

/// **Medicao 1 do parecer:** `[inserir filha->M, excluir M]`, com M ja
/// no disco. A exclusao tem de enxergar a filha do PREFIXO.
#[test]
fn inserir_a_filha_e_excluir_a_mae_recusa_antes_da_marca() {
    let dir = dir_temp("448-filha-e-exclui");
    let s = servidor(&dir);
    let ses = sessao(4482);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    let antes = retrato(&s, &ses, &dir);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1"#,
    )
    .unwrap();
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 2 de 2",
        "clientes",
        "INTEGRIDADE",
    );
}

/// **Buraco (a) da sobreposicao:** `[atualizar M K->K', inserir
/// filha->K]`. A linha do disco cuja chave o prefixo ALTEROU nao pode
/// continuar sendo achada pela chave velha.
#[test]
fn alterar_a_chave_da_mae_e_apontar_para_a_velha_recusa() {
    let dir = dir_temp("448-chave-velha");
    let s = servidor(&dir);
    let ses = sessao(4483);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    let antes = retrato(&s, &ses, &dir);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":5,"nome":"Ana"}"#,
    )
    .unwrap();
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 2 de 2",
        "pedidos",
        "INTEGRIDADE",
    );
}

/// **Buraco (a), o outro lado:** `[atualizar M K->K' com cascata,
/// excluir M]`. As filhas movidas para K' sao linhas do disco com
/// troca pendente -- e a exclusao da mae tem de acha-las pela chave NOVA.
#[test]
fn alterar_a_chave_com_cascata_e_excluir_a_mae_recusa() {
    let dir = dir_temp("448-cascata-e-exclui");
    let s = servidor(&dir);
    let ses = sessao(4484);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    let antes = retrato(&s, &ses, &dir);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":5,"nome":"Ana"}"#,
    )
    .unwrap();
    assert_eq!(
        r.inteiro_ou("linhas", 0),
        2,
        "a cascata tinha de entrar na lista: {}",
        r.escrever()
    );
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1"#,
    )
    .unwrap();
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 3 de 3",
        "clientes",
        "INTEGRIDADE",
    );
}

/// **Buraco (b) da sobreposicao:** `[excluir_suave M, inserir
/// filha->M]`. A conferencia de «mae viva» nao pode ler por baixo da
/// marca pendente.
#[test]
fn excluir_suave_a_mae_e_inserir_a_filha_recusa() {
    let dir = dir_temp("448-suave-e-filha");
    let s = servidor(&dir);
    let ses = sessao(4485);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    let antes = retrato(&s, &ses, &dir);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1"#,
    )
    .unwrap();
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 2 de 2",
        "pedidos",
        "INTEGRIDADE",
    );
}

/// **Unicidade de novo, no COMMIT:** `[atualizar R: chave -> K,
/// inserir K]`. O `empilhar` so guarda as chaves das insercoes, e o
/// `atualizar` que toma a chave passava para a passada.
#[test]
fn a_chave_unica_tomada_no_prefixo_recusa_antes_da_marca() {
    let dir = dir_temp("448-unica");
    let s = servidor(&dir);
    let ses = sessao(4486);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    let antes = retrato(&s, &ses, &dir);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":7,"nome":"Ana"}"#,
    )
    .unwrap();
    inserir(&s, &ses, "clientes", r#"{"id":7,"nome":"Bia"}"#);
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 2 de 2",
        "clientes",
        "DUPLICADO",
    );
}

/// **Concorrencia (prova 8):** T1 empilha a filha de M; uma sessao comum
/// apaga M -- nada no disco aponta para M, entao passa. O COMMIT de T1
/// tem de recusar com zero aplicado, e nao gravar o que vinha antes.
#[test]
fn a_mae_apagada_por_outra_sessao_recusa_o_commit_com_zero_gravado() {
    let dir = dir_temp("448-concorrencia");
    let s = servidor(&dir);
    let ses = sessao(4487);
    let outra = sessao(4488);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    inserir(&s, &ses, "clientes", r#"{"id":2,"nome":"Bia"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    pede(
        &s,
        &outra,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,"fisico":true"#,
    )
    .expect("a sessao comum tinha de conseguir apagar a mae sem filha no disco");
    let antes = retrato(&s, &outra, &dir);
    confere_a_recusa(
        s,
        &ses,
        &dir,
        antes,
        "escrita 2 de 2",
        "pedidos",
        "INTEGRIDADE",
    );
}

/// **Medicao 2 do parecer (armadilha 3):** a filha FORA do plano do
/// `ao_alterar`, redirecionada para a chave velha entre o `empilhar` e
/// o COMMIT, nao pode ficar orfa. A pre-conferencia replaneja a arvore
/// contra o disco e o prefixo, acha a filha que a lista nao leva, e
/// recusa.
#[test]
fn a_filha_redirecionada_fora_do_plano_nao_fica_orfa() {
    let dir = dir_temp("448-fora-do-plano");
    let s = servidor(&dir);
    let ses = sessao(4489);
    let outra = sessao(4490);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    inserir(&s, &ses, "clientes", r#"{"id":2,"nome":"Bia"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":11,"cliente_id":2}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":5,"nome":"Ana"}"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("linhas", 0), 2, "{}", r.escrever());
    // A sessao comum aponta a filha R (rowid 2, fora do plano) para a
    // chave VELHA da mae -- que no disco ainda existe.
    pede(
        &s,
        &outra,
        r#""op":"atualizar","database":"loja","tabela":"pedidos","rowid":2,
                   "linha":{"id":11,"cliente_id":1}"#,
    )
    .expect("a sessao comum tinha de conseguir redirecionar a filha fora do plano");
    let antes = retrato(&s, &outra, &dir);
    let r = pede(&s, &ses, r#""op":"commit""#);
    let veredito = match &r {
        Ok(j) => j.escrever(),
        Err(e) => e.to_string(),
    };
    // O que importa e o DADO: nenhuma filha pode apontar para mae que
    // nao existe, qualquer que tenha sido o veredito.
    let ids: Vec<i64> = pede(
        &s,
        &outra,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .filter_map(|l| l.campo("id").and_then(Json::inteiro))
    .collect();
    let orfas = pede(
        &s,
        &outra,
        r#""op":"varrer","database":"loja","tabela":"pedidos","max":100"#,
    )
    .unwrap()
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .filter_map(|l| l.campo("cliente_id").and_then(Json::inteiro))
    .filter(|c| !ids.contains(c))
    .count();
    assert_eq!(
        (orfas, r.is_err(), retrato(&s, &outra, &dir)),
        (0, true, antes),
        "(filhas orfas, recusou?, retrato): a filha fora do plano ficou orfa ou \
                 a recusa gravou -- COMMIT: {veredito}"
    );
    assert!(
        veredito.contains("ANTES da marca") && veredito.contains("pedidos"),
        "a recusa tinha de nomear a filha que a lista nao leva: {veredito}"
    );
}

/// **A cascata IMPLICITA numa tabela que a lista nao tocou continua
/// valendo** -- o comportamento velho. A alteracao da chave foi
/// empilhada sem filha nenhuma (a lista nao leva cascata), e outra
/// sessao pos uma filha na chave velha antes do COMMIT: a passada
/// cascateia por conta propria, e a pre-conferencia nao pode recusar.
#[test]
fn a_cascata_implicita_numa_tabela_fora_da_lista_continua_valendo() {
    let dir = dir_temp("448-implicita-fora");
    let s = servidor(&dir);
    let ses = sessao(4493);
    let outra = sessao(4494);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":5,"nome":"Ana"}"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("linhas", 0), 1, "{}", r.escrever());
    inserir(&s, &outra, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    let r = pede(&s, &ses, r#""op":"commit""#).expect("transacao valida recusada");
    assert_eq!(r.texto_ou("transaction_state", ""), "COMMITTED");
    let pedidos = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"loja","tabela":"pedidos","max":10"#,
    )
    .unwrap();
    let maes: Vec<i64> = pedidos
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .filter_map(|l| l.campo("cliente_id").and_then(Json::inteiro))
        .collect();
    assert_eq!(maes, vec![5], "a cascata implicita nao acompanhou a mae");
}

/// **Excluir a filha e DEPOIS a mae, na mesma lista, e valido.** A
/// exclusao da mae tem de descontar a filha que o prefixo apagou -- e
/// o comportamento que nao pode quebrar.
#[test]
fn excluir_a_filha_e_depois_a_mae_na_mesma_lista_confirma() {
    let dir = dir_temp("448-filha-e-mae");
    let s = servidor(&dir);
    let ses = sessao(4491);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"pedidos","rowid":1,"fisico":true"#,
    )
    .unwrap();
    pede(
        &s,
        &ses,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,"fisico":true"#,
    )
    .unwrap();
    let r = pede(&s, &ses, r#""op":"commit""#);
    let veredito = match &r {
        Ok(j) => j.escrever(),
        Err(e) => e.to_string(),
    };
    let r = r.unwrap_or_else(|_| panic!("transacao valida recusada: {veredito}"));
    assert_eq!(
        r.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{veredito}"
    );
    assert!(r.campo("aviso").is_none(), "{veredito}");
    // A marca de um COMMIT bom espera a janela de durabilidade fechar
    // (o group commit); fechada a janela, ela tem de sair.
    s.descarregar_sujas();
    assert_eq!(
        retrato(&s, &ses, &dir),
        (0, 0, 1, 1, 0),
        "a transacao valida nao saiu inteira: {veredito}"
    );
}

/// **O comportamento VELHO, e o teste que mais importa:** mae nova e
/// filha na ordem certa, e a chave da mae alterada com a cascata na
/// lista -- COMMITTED e inteira, sem marca sobrando.
#[test]
fn a_ordem_certa_continua_committed_e_inteira() {
    let dir = dir_temp("448-velho");
    let s = servidor(&dir);
    let ses = sessao(4492);
    base_com_fk(&s, &ses);
    inserir(&s, &ses, "clientes", r#"{"id":1,"nome":"Ana"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":10,"cliente_id":1}"#);
    pede(&s, &ses, r#""op":"begin""#).unwrap();
    inserir(&s, &ses, "clientes", r#"{"id":2,"nome":"Bia"}"#);
    inserir(&s, &ses, "pedidos", r#"{"id":11,"cliente_id":2}"#);
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "linha":{"id":5,"nome":"Ana"}"#,
    )
    .unwrap();
    inserir(&s, &ses, "pedidos", r#"{"id":12,"cliente_id":5}"#);
    let r = pede(&s, &ses, r#""op":"commit""#).expect("transacao valida recusada");
    assert_eq!(r.texto_ou("transaction_state", ""), "COMMITTED");
    assert_eq!(r.inteiro_ou("gravadas", 0), 5, "{}", r.escrever());
    assert!(r.campo("aviso").is_none(), "{}", r.escrever());
    s.descarregar_sujas();
    assert_eq!(retrato(&s, &ses, &dir), (2, 3, 2, 3, 0));
    let pedidos = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"loja","tabela":"pedidos","max":100"#,
    )
    .unwrap();
    let maes: Vec<i64> = pedidos
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .filter_map(|l| l.campo("cliente_id").and_then(Json::inteiro))
        .collect();
    assert_eq!(maes, vec![5, 2, 5], "a cascata nao acompanhou a mae");
}
