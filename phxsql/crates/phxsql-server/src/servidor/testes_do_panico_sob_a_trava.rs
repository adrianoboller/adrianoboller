//! O panico DENTRO da trava global de dados -- pedido 451, provado pelo
//! SOQUETE.
//!
//! # O que cada teste mede
//!
//! O panico e o do motor (`ndx::panico_de_teste`), no meio de uma escrita de
//! verdade -- o slot e o contador do `.reg` ja gravados, nenhuma chave no
//! `.ndx` -- e na thread de uma conexao de verdade, com a trava de escrita na
//! mao. Quem escolhe a thread e o campo `panico_de_teste_na_op`, que so existe
//! com `cfg(test)`: nenhum pedido do fio arma coisa nenhuma.
//!
//! Cada vermelho descreve o DANO medido, e nao o mecanismo: «o pedido de
//! OUTRA conexao recebeu a trava suja», «a transacao confirmada saiu pela
//! metade», «o processo seguiu servindo». Um vermelho que dissesse «o contador
//! de reparos esta em zero» descreveria o conserto e esconderia o estrago.
//!
//! # Por que `debug_assertions`
//!
//! O gancho do motor so dispara com elas (em `release` ele nao existe), e sem
//! o panico estes testes nao teriam o que provar -- o mesmo corte do
//! `panico-no-meio-da-escrita.rs` do `phxsql-store`.
use super::*;
use phxsql_store::ndx::panico_de_teste::Ponto;

/// A variavel que diz ao processo FILHO onde trabalhar. Ver
/// [`filho_do_panico_451`].
const FILHO: &str = "PHXSQL_TESTE_451_FILHO";

fn config_base(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    // O escape escrito: a cifra do fio nasce exigida (pedido 370), e
    // estes testes conectam em claro porque medem OUTRA coisa.
    c.cifra_fio.exigir = false;
    c
}

/// A porta de dados pelo laco DE PRODUCAO: e a thread da conexao que
/// entra em panico, e o `AoSair` dela (que solta as travas da transacao)
/// tem de rodar como roda em producao.
fn porta_de_dados_de_verdade(s: &Arc<Servidor>) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let s = Arc::clone(s);
    std::thread::spawn(move || s.aceitar_ate_mandarem_parar(&ouvinte));
    porta
}

use crate::apoio_teste::Ligacao;

/// Um pedido desta prova na ligacao dada -- a mesma, quando e a da
/// transacao. O corpo vai com o token numa linha so, e a resposta volta
/// analisada. `None` quando a conexao caiu sem responder, que e o que o
/// pedido que entra em panico produz. A leitura e a do cliente unico de
/// teste (`apoio_teste::Ligacao`): aqui fica so o formato do pedido.
fn falar(l: &mut Ligacao, corpo: &str) -> Option<Json> {
    let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
    let r = l.pedir(&format!("{{\"token\":\"t\",{corpo}}}"))?;
    Some(Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}")))
}

/// Um pedido numa conexao NOVA -- a «outra conexao» da prova.
fn pedir(porta: u16, corpo: &str) -> Option<Json> {
    falar(&mut Ligacao::nova(porta), corpo)
}

/// O resultado, exigindo que o pedido tenha dado certo.
fn ok(r: Option<Json>, o_que: &str) -> Json {
    let r = r.unwrap_or_else(|| panic!("{o_que}: a conexao caiu sem resposta"));
    assert!(r.booleano_ou("ok", false), "{o_que}: {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(r)
}

/// `loja.clientes` (indice unico `porId`) com os ids 1..=5, e
/// `loja.outra` com o id 1 -- a tabela que o panico NAO toca.
fn semear(porta: u16) {
    ok(
        pedir(porta, r#""op":"criar_database","database":"loja""#),
        "criar_database",
    );
    ok(
        pedir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"nome","tipo":"Str(20)"}],
                   "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
        ),
        "criar_tabela clientes",
    );
    ok(
        pedir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"outra",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
        ),
        "criar_tabela outra",
    );
    for id in 1..=5 {
        ok(
            pedir(
                porta,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"clientes",
                           "valores":{{"id":{id},"nome":"C{id}"}}"#
                ),
            ),
            "semear clientes",
        );
    }
    ok(
        pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"outra","valores":{"id":1}"#,
        ),
        "semear outra",
    );
}

/// Os ids vivos que o `.reg` da tabela mostra, pelo `varrer`.
fn ids(porta: u16, tabela: &str) -> Vec<i64> {
    let r = ok(
        pedir(
            porta,
            &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":1000"#),
        ),
        &format!("varrer {tabela}"),
    );
    let mut v: Vec<i64> = r
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect();
    v.sort_unstable();
    v
}

/// Todo id vivo e achado pela chave, UMA vez, e o `verificar` conta tantas
/// chaves no indice quantas linhas no `.reg` -- a pergunta que a tabela
/// tem de responder certo depois de um panico no meio da escrita.
fn conferir_indice(porta: u16, esperados: &[i64]) {
    let vivos = ids(porta, "clientes");
    assert_eq!(vivos, esperados, "as linhas vivas no .reg");
    for id in &vivos {
        let r = ok(
            pedir(
                porta,
                &format!(
                    r#""op":"buscar","database":"loja","tabela":"clientes",
                           "indice":"porId","chave":[{id}]"#
                ),
            ),
            &format!("buscar {id}"),
        );
        assert_eq!(
            r.inteiro_ou("encontrados", -1),
            1,
            "linha viva fora do indice (ou repetida nele): o id {id} esta vivo \
                 no .reg e o indice responde {}",
            r.escrever()
        );
    }
    let v = ok(
        pedir(
            porta,
            r#""op":"verificar","database":"loja","tabela":"clientes""#,
        ),
        "verificar",
    );
    assert_eq!(
        v.inteiro_ou("registros", -1),
        esperados.len() as i64,
        "{}",
        v.escrever()
    );
    assert_eq!(
        v.campo("indices").map(|i| i.inteiro_ou("porId", -1)),
        Some(esperados.len() as i64),
        "o indice nao tem uma chave por linha viva: {}",
        v.escrever()
    );
}

/// As marcas `transacao_*.tx` que sobraram no diretorio da base.
fn marcas(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir.join("loja"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("transacao_") && n.ends_with(".tx"))
        .collect()
}

fn armar(s: &Servidor, op: &str) {
    *s.panico_de_teste_na_op.lock().unwrap() = Some((
        op.into(),
        PanicoDeTeste::NoMotor(Ponto::InserirDepoisDoContador),
    ));
}

/// A janela que so fecha quando alguem manda: sem o relogio de fundo (que
/// estes testes nao sobem) e com o lote e o prazo fora de alcance, toda
/// marca de `COMMIT` fica PENDENTE ate um fecho explicito.
fn janela_parada(c: &mut Config) {
    c.recursos.durabilidade = Durabilidade::PorLote;
    c.recursos.lote_operacoes = 1_000_000;
    c.recursos.lote_milissegundos = 600_000;
}

/// Um `COMMIT` de uma linha na `tabela`, por uma conexao propria.
fn commit_em(porta: u16, tabela: &str, id: i64) {
    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    ok(
        falar(
            &mut tx,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"{tabela}","valores":{{"id":{id}}}"#
            ),
        ),
        "inserir na transacao",
    );
    ok(falar(&mut tx, r#""op":"commit""#), "commit");
}

/// `loja` com as tabelas `a` e `b`, so com o `id`.
fn duas_tabelas(porta: u16) {
    ok(
        pedir(porta, r#""op":"criar_database","database":"loja""#),
        "criar_database",
    );
    for t in ["a", "b"] {
        ok(
            pedir(
                porta,
                &format!(
                    r#""op":"criar_tabela","database":"loja","tabela":"{t}",
                           "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}}]"#
                ),
            ),
            "criar_tabela",
        );
    }
}

/// O `nome` do cliente `id`, pelo `varrer`.
fn nome_do_cliente(porta: u16, id: i64) -> String {
    let r = ok(
        pedir(
            porta,
            r#""op":"varrer","database":"loja","tabela":"clientes","max":1000"#,
        ),
        "varrer clientes",
    );
    r.campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .find(|l| l.inteiro_ou("id", -1) == id)
        .map(|l| l.texto_ou("nome", "").to_string())
        .unwrap_or_default()
}

/// **Pedido 653: o veneno da trava de dados e PERMANENTE de proposito, e
/// ela continua atendendo tomada apos tomada.**
///
/// A razao velha de nao limpar o veneno era a versao (`clear_poison` e de
/// 1.77; a casa prometia 1.75). Com o `rust-version` em 1.89 a pergunta
/// voltou: deve-se limpar? Nao -- quem decide e o par
/// `panicos_na_trava`/`reparos_da_trava`, e o veneno so o aciona. Esta
/// prova segura as duas metades: depois de UM panico reparado, o `RwLock`
/// continua envenenado e mesmo assim vinte pedidos seguidos, de leitura e
/// de escrita, atendem -- e os contadores ficam em 1 e 1.
///
/// O vermelho: com o `depois_do_veneno` exigindo trava limpa (o
/// `trava_de_dados_sem_reparo` sempre), o primeiro pedido depois do
/// panico ja recusa com `SP000010`.
#[test]
fn o_veneno_permanente_continua_recuperando_tomada_apos_tomada() {
    let dir = DirTemp::novo("panico-653-veneno");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear(porta);
    armar(&s, "inserir");
    let morto = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":6,"nome":"C6"}"#,
    );
    assert!(morto.is_none(), "o inserir armado nao caiu no panico");
    assert!(
        s.dados.is_poisoned(),
        "o panico com a trava na mao nao a envenenou -- a prova nao exercita nada"
    );
    for i in 0..20 {
        let id = 100 + i;
        ok(
            pedir(
                porta,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"outra","valores":{{"id":{id}}}"#
                ),
            ),
            &format!("inserir {id} depois do veneno"),
        );
        ok(
            pedir(
                porta,
                r#""op":"varrer","database":"loja","tabela":"outra","max":1"#,
            ),
            "varrer depois do veneno",
        );
    }
    assert!(
        s.dados.is_poisoned(),
        "alguem passou a limpar o veneno: a decisao do 653 mudou sem a prova mudar"
    );
    assert_eq!(
        (
            s.panicos_na_trava.load(Ordering::SeqCst),
            s.reparos_da_trava.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
}

/// **(a) e (b), fora de transacao.** O `inserir` morre com o slot e o
/// contador do `.reg` gravados e nenhuma chave no indice.
///
/// Com o defeito (a H1 de antes): a trava fica envenenada para sempre, e
/// o pedido seguinte de OUTRA conexao, noutra tabela, recebe
/// `[SP000010] ... deixou a trava suja` ate o processo reiniciar.
///
/// Com o conserto: a outra tabela atende na hora; a tabela tocada le pelo
/// `.reg` e RECUSA toda operacao de indice nomeando o indice -- nunca a
/// trava -- ate o `reindexar` (o byte 52 do pedido 456, o mesmo estado de
/// um `SIGKILL` no mesmo ponto); e depois dele cada linha viva esta no
/// indice uma vez, e a chave em voo continua unica.
///
/// # A linha interrompida, e por que ela FICA
///
/// O slot e o contador ja estavam no `.reg` quando o panico veio, e o
/// `.reg` nunca reaproveita slot: desfazer nao existe aqui (a ordem de
/// digitacao), e fora de transacao nao ha marca que diga que a escrita
/// nao aconteceu. Ela anda para a FRENTE, como depois de uma queda no
/// mesmo ponto -- e o que a prova cobra e que ela nao vire FANTASMA: viva
/// no `.reg` e fora do indice, ou dentro dele duas vezes.
#[test]
fn panico_no_meio_do_inserir_nao_fecha_a_base_e_nao_deixa_fantasma() {
    let dir = DirTemp::novo("panico-451-inserir");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear(porta);
    // Uma copia residente, para a prova do terceiro passo do reparo.
    ok(
        pedir(
            porta,
            r#""op":"memoria_carregar","database":"loja","tabela":"clientes""#,
        ),
        "memoria_carregar",
    );

    armar(&s, "inserir");
    let morto = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":6,"nome":"C6"}"#,
    );
    assert!(
        morto.is_none(),
        "o inserir armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    assert!(
        s.panico_de_teste_na_op.lock().unwrap().is_none(),
        "o gancho nao foi armado: o inserir nao passou pelo despachar"
    );

    // (a) O DANO primeiro: OUTRA conexao, OUTRA tabela.
    let outra = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"outra","valores":{"id":2}"#,
    )
    .expect("o inserir seguinte, por OUTRA conexao, caiu sem resposta");
    assert!(
        outra.booleano_ou("ok", false),
        "a trava de dados ficou FECHADA depois do panico: o inserir em OUTRA \
             tabela, por OUTRA conexao, recebeu {}",
        outra.escrever()
    );
    assert_eq!(
        ids(porta, "outra"),
        vec![1, 2],
        "a tabela que o panico nao tocou"
    );

    // (b) A tabela tocada: o dado de antes continua la, lido pelo `.reg`.
    let vivos = ids(porta, "clientes");
    assert_eq!(
        vivos,
        vec![1, 2, 3, 4, 5, 6],
        "o .reg depois do panico: o dado de antes tinha de continuar, e o slot \
             ja gravado da linha interrompida anda para a frente"
    );
    // A copia residente e anotada DEPOIS do disco, e a da operacao que
    // morreu ficou atras dele: servi-la seria mentir sobre o dado.
    let memoria = pedir(
        porta,
        r#""op":"selecionar_memoria","database":"loja","tabela":"clientes""#,
    )
    .expect("o selecionar_memoria caiu sem resposta");
    assert!(
        !memoria.booleano_ou("ok", true) && memoria.escrever().contains("nao esta em memoria"),
        "a copia residente continuou servindo depois do panico, com o .reg em \
             {vivos:?}: {}",
        memoria.escrever()
    );
    // E o indice recusa NOMEANDO-SE, ate o `reindexar` -- nunca a trava.
    let recusa = pedir(
        porta,
        r#""op":"buscar","database":"loja","tabela":"clientes","indice":"porId","chave":[3]"#,
    )
    .expect("o buscar caiu sem resposta");
    let texto = recusa.escrever();
    assert!(
        !recusa.booleano_ou("ok", true),
        "o indice rasgado respondeu como se estivesse em dia: {texto}"
    );
    assert!(
        texto.contains("clientes.ndx"),
        "a recusa nao nomeou o indice: {texto}"
    );
    assert!(
        texto.contains("`reindexar`") && !texto.contains("trava suja"),
        "a recusa nao mandou reconstruir o indice: {texto}"
    );

    ok(
        pedir(
            porta,
            r#""op":"reindexar","database":"loja","tabela":"clientes""#,
        ),
        "reindexar",
    );
    conferir_indice(porta, &[1, 2, 3, 4, 5, 6]);
    // A chave que estava em voo continua UNICA.
    let repetida = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":6,"nome":"de novo"}"#,
    )
    .expect("o inserir repetido caiu sem resposta");
    assert!(
        !repetida.booleano_ou("ok", true),
        "chave duplicada aceita no indice unico depois do reparo: {}",
        repetida.escrever()
    );
    ok(
        pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"clientes",
                   "valores":{"id":7,"nome":"C7"}"#,
        ),
        "inserir depois do reindexar",
    );
    conferir_indice(porta, &[1, 2, 3, 4, 5, 6, 7]);
}

/// **A transacao confirmada, na hora.** O `COMMIT` grava a marca, e a
/// passada morre na PRIMEIRA escrita, com o slot gravado e o indice atras.
///
/// Com o defeito: a base inteira responde a trava suja, e a marca orfa
/// espera o reinicio -- enquanto o `AoSair` ja soltou as travas da
/// transacao, inclusive o fim de tabela que reservava os rowids.
///
/// Com o conserto: o reparo completa a marca ANTES de o `AoSair` rodar, e
/// OUTRA conexao ja ve a transacao inteira, o indice em dia (a recuperacao
/// o reconstroi por ser tabela nomeada na marca), nenhuma marca sobrando e
/// o rowid seguinte sem colidir com os reservados.
#[test]
fn panico_na_passada_do_commit_sai_com_a_transacao_inteira_na_hora() {
    let dir = DirTemp::novo("panico-451-commit");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear(porta);

    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    for id in [10, 11, 12] {
        ok(
            falar(
                &mut tx,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"clientes",
                       "valores":{{"id":{id},"nome":"T{id}"}}"#
                ),
            ),
            "inserir na transacao",
        );
    }
    armar(&s, "commit");
    let morto = falar(&mut tx, r#""op":"commit""#);
    assert!(
        morto.is_none(),
        "o commit armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    drop(tx);

    // O DANO primeiro: OUTRA conexao le a tabela da transacao.
    let lida = pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":1000"#,
    )
    .expect("o varrer seguinte, por OUTRA conexao, caiu sem resposta");
    assert!(
        lida.booleano_ou("ok", false),
        "a trava de dados ficou FECHADA depois do panico no COMMIT: OUTRA \
             conexao recebeu {}",
        lida.escrever()
    );
    assert_eq!(
        ids(porta, "clientes"),
        vec![1, 2, 3, 4, 5, 10, 11, 12],
        "a transacao CONFIRMADA (a marca estava no disco) nao saiu inteira"
    );
    assert!(
        marcas(&dir).is_empty(),
        "a marca do commit completado ficou no disco: {:?}",
        marcas(&dir)
    );
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12]);
    // O rowid seguinte nao colide com os que a transacao reservou.
    let novo = ok(
        pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"clientes",
                   "valores":{"id":13,"nome":"C13"}"#,
        ),
        "inserir depois do reparo",
    );
    assert_eq!(novo.inteiro_ou("rowid", -1), 9, "{}", novo.escrever());
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12, 13]);
}

// O teste de cima serve QUATRO guardas (pedido 655): a trava que nao se
// repara (duas), a marca em voo que fica para o reinicio e a recuperacao que
// nao reconstroi o indice -- e cai igual para as quatro. Os tres de baixo
// repetem o cenario e conferem UMA etapa cada um, na ordem em que o dano
// aparece; cada guarda ganha o teste que so ela derruba, e os outros dois
// ficam de pe como vizinhos.

/// O cenario do de cima ate o panico no `COMMIT`: tres linhas inseridas numa
/// transacao, o `COMMIT` armado para cair na primeira escrita da passada.
fn commit_em_panico_na_passada(nome: &str) -> (DirTemp, Arc<Servidor>, u16) {
    let dir = DirTemp::novo(nome);
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear(porta);
    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    for id in [10, 11, 12] {
        ok(
            falar(
                &mut tx,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"clientes",
                       "valores":{{"id":{id},"nome":"T{id}"}}"#
                ),
            ),
            "inserir na transacao",
        );
    }
    armar(&s, "commit");
    let morto = falar(&mut tx, r#""op":"commit""#);
    assert!(
        morto.is_none(),
        "premissa: o commit armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    drop(tx);
    (dir, s, porta)
}

/// Etapa 1: a trava de dados volta a atender OUTRA conexao. E a etapa das
/// guardas do reparo da trava (451); a marca e o indice nao entram aqui.
#[test]
fn panico_na_passada_do_commit_reabre_a_trava_na_hora() {
    let (_dir, _s, porta) = commit_em_panico_na_passada("panico-451-trava");
    let lida = pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":1000"#,
    )
    .expect("o varrer seguinte, por OUTRA conexao, caiu sem resposta");
    assert!(
        lida.booleano_ou("ok", false),
        "a trava de dados ficou FECHADA depois do panico no COMMIT: {}",
        lida.escrever()
    );
}

/// Etapa 2: a transacao CONFIRMADA sai inteira e a marca dela sai do disco
/// -- o reparo completa a marca em voo na hora, e nao no reinicio.
#[test]
fn panico_na_passada_do_commit_completa_a_marca_na_hora() {
    let (dir, _s, porta) = commit_em_panico_na_passada("panico-451-marca");
    assert_eq!(
        ids(porta, "clientes"),
        vec![1, 2, 3, 4, 5, 10, 11, 12],
        "a transacao CONFIRMADA (a marca estava no disco) nao saiu inteira"
    );
    assert!(
        marcas(&dir).is_empty(),
        "a marca do commit completado ficou no disco: {:?}",
        marcas(&dir)
    );
}

/// Etapa 3: o indice da tabela nomeada na marca sai em dia, e o rowid
/// seguinte nao colide com os que a transacao reservou.
#[test]
fn panico_na_passada_do_commit_deixa_o_indice_em_dia() {
    let (_dir, _s, porta) = commit_em_panico_na_passada("panico-451-indice");
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12]);
    let novo = ok(
        pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"clientes",
                   "valores":{"id":13,"nome":"C13"}"#,
        ),
        "inserir depois do reparo",
    );
    assert_eq!(novo.inteiro_ou("rowid", -1), 9, "{}", novo.escrever());
}

/// `loja.mae(id, codigo)` com `codigo` unico no 5, e `loja.filha` com duas
/// linhas apontando para ele, `ao_alterar` em cascata -- o cenario do
/// pedido 490.
fn semear_cascata(porta: u16) {
    ok(
        pedir(porta, r#""op":"criar_database","database":"loja""#),
        "criar_database",
    );
    ok(
        pedir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"mae",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"codigo","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_codigo","colunas":["codigo"],"unico":true}]"#,
        ),
        "criar_tabela mae",
    );
    ok(
        pedir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"filha",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"cod","tipo":"Int8"}],
                   "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_cod","colunas":["cod"]}],
                   "chaves_estrangeiras":[{"nome":"fk_mae","colunas":["cod"],
                                           "tabela_ref":"mae","colunas_ref":["codigo"]}]"#,
        ),
        "criar_tabela filha",
    );
    ok(
        pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"mae","valores":{"id":1,"codigo":5}"#,
        ),
        "semear mae",
    );
    for id in [10, 11] {
        ok(
            pedir(
                porta,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"filha",
                           "valores":{{"id":{id},"cod":5}}"#
                ),
            ),
            "semear filha",
        );
    }
}

/// Os `cod` da filha, em ordem de digitacao.
fn cods_da_filha(porta: u16) -> Vec<i64> {
    let r = ok(
        pedir(
            porta,
            r#""op":"varrer","database":"loja","tabela":"filha","max":100"#,
        ),
        "varrer filha",
    );
    r.campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| l.inteiro_ou("cod", -1))
        .collect()
}

/// **Pedido 490, DENTRO de transacao -- o que o 451 ja cobre, medido.** A
/// cascata viaja achatada na lista (ACID-C), e a passada morre na mae,
/// com o `.reg` dela ja na chave nova e nenhum elo aplicado: a mae
/// gravada e as duas filhas na chave velha, o estado do meio. O reparo do
/// 451 completa a marca EM VOO pela recuperacao, e OUTRA conexao ja ve a
/// cascata inteira. Dentro de transacao o 490 nao existe; ele e so do
/// caminho sem marca.
#[test]
fn panico_no_meio_da_cascata_na_transacao_sai_com_a_cascata_inteira() {
    let dir = DirTemp::novo("panico-490-tx");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear_cascata(porta);

    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    let r = ok(
        falar(
            &mut tx,
            r#""op":"atualizar","database":"loja","tabela":"mae","rowid":1,
                   "valores":{"id":1,"codigo":6}"#,
        ),
        "atualizar a mae na transacao",
    );
    assert_eq!(r.inteiro_ou("linhas", 0), 3, "{}", r.escrever());
    *s.panico_de_teste_na_op.lock().unwrap() = Some((
        "commit".into(),
        PanicoDeTeste::NoMotor(Ponto::AtualizarDepoisDoReg),
    ));
    let morto = falar(&mut tx, r#""op":"commit""#);
    assert!(
        morto.is_none(),
        "o commit armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    drop(tx);

    assert_eq!(
        cods_da_filha(porta),
        vec![6, 6],
        "a transacao CONFIRMADA (a marca estava no disco) nao levou a cascata inteira"
    );
    assert!(
        marcas(&dir).is_empty(),
        "a marca do commit completado ficou no disco: {:?}",
        marcas(&dir)
    );
}

/// A cascata inteira no disco, conferida pelo indice e nao so pelo `.reg`:
/// as duas filhas em 6, nenhuma achada pela chave velha, a mae achada pela
/// nova, e nenhuma marca sobrando.
fn cascata_inteira(porta: u16, dir: &std::path::Path, o_que: &str) {
    assert_eq!(
        cods_da_filha(porta),
        vec![6, 6],
        "{o_que}: a mae foi para 6 e ficou filha na chave velha -- orfa"
    );
    for (tabela, indice, chave, esperados) in [
        ("filha", "por_cod", 5, 0),
        ("filha", "por_cod", 6, 2),
        ("mae", "por_codigo", 6, 1),
    ] {
        let r = ok(
            pedir(
                porta,
                &format!(
                    r#""op":"buscar","database":"loja","tabela":"{tabela}",
                           "indice":"{indice}","chave":[{chave}]"#
                ),
            ),
            &format!("{o_que}: buscar {tabela} {chave}"),
        );
        assert_eq!(
            r.inteiro_ou("encontrados", -1),
            esperados,
            "{o_que}: o indice de {tabela} pela chave {chave}: {}",
            r.escrever()
        );
    }
    assert!(
        marcas(dir).is_empty(),
        "{o_que}: a marca da cascata ficou no disco: {:?}",
        marcas(dir)
    );
}

/// **Pedido 540, FORA de transacao: o panico no meio da cascata solta sai
/// com a cascata INTEIRA.** O `atualizar` solto da mae que tem filha grava
/// a marca antes, como uma transacao de uma instrucao, e o reparo da trava
/// a completa -- como faz com o COMMIT (451).
///
/// Antes do conserto a cascata solta nao tinha marca: o panico deixava a
/// mae na chave nova e as duas filhas na velha, e o 490 so conseguia fazer
/// a tabela RECUSAR ate um `reindexar`, que reconstruia o indice com a
/// orfa dentro -- e, com o 522, o proprio arranque o fazia calado.
///
/// # Prova real
///
/// Sem a marca (o `alterar_solto` voltando ao `t.atualizar` de antes), as
/// filhas ficam `[5, 5]` -- o vermelho medido.
#[test]
fn panico_no_meio_da_cascata_fora_da_transacao_sai_com_a_cascata_inteira() {
    let dir = DirTemp::novo("panico-540-fora");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear_cascata(porta);

    *s.panico_de_teste_na_op.lock().unwrap() = Some((
        "atualizar".into(),
        PanicoDeTeste::NoMotor(Ponto::AtualizarDepoisDoReg),
    ));
    let morto = pedir(
        porta,
        r#""op":"atualizar","database":"loja","tabela":"mae","rowid":1,
               "valores":{"id":1,"codigo":6}"#,
    );
    assert!(
        morto.is_none(),
        "o atualizar armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    cascata_inteira(porta, &dir, "depois do panico");
}

/// **Pedido 540, o irmao do upsert:** o `inserir` com `se_existir:
/// "atualizar"` que acha a mae e muda a chave dela cascateia pelo MESMO
/// caminho do `atualizar` solto -- e sem o conserto, pelo `t.atualizar` de
/// dentro do upsert, sem marca.
///
/// # Prova real
///
/// Com o upsert gravando por `t.atualizar` direto, as filhas ficam
/// `[5, 5]` -- o vermelho medido.
#[test]
fn panico_no_meio_da_cascata_do_upsert_solto_sai_com_a_cascata_inteira() {
    let dir = DirTemp::novo("panico-540-upsert");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear_cascata(porta);

    *s.panico_de_teste_na_op.lock().unwrap() = Some((
        "inserir".into(),
        PanicoDeTeste::NoMotor(Ponto::AtualizarDepoisDoReg),
    ));
    let morto = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"mae","se_existir":"atualizar",
               "valores":{"id":1,"codigo":6}"#,
    );
    assert!(
        morto.is_none(),
        "o upsert armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    cascata_inteira(porta, &dir, "depois do panico no upsert");
}

/// O lado que nao muda, e a janela: a cascata solta SEM queda grava
/// inteira, e a marca dela ESPERA o `fsync` das tabelas -- a da mae e a
/// das filhas, que agora passam pelos punhos da passada e entram na janela
/// --, e so sai no fecho. A alteracao sem filha nao grava marca nenhuma.
#[test]
fn a_cascata_solta_sem_queda_grava_inteira_e_a_marca_espera_o_fsync() {
    let dir = DirTemp::novo("cascata-540-sem-queda");
    let mut c = config_base(&dir);
    janela_parada(&mut c);
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear_cascata(porta);
    let r = ok(
        pedir(
            porta,
            r#""op":"atualizar","database":"loja","tabela":"mae","rowid":1,
                   "valores":{"id":1,"codigo":6}"#,
        ),
        "atualizar a mae",
    );
    assert!(r.campo("aviso").is_none(), "{}", r.escrever());
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a marca da cascata solta tinha de ficar PENDENTE, esperando o fsync"
    );
    s.descarregar_sujas();
    cascata_inteira(porta, &dir, "sem queda, depois do fecho");
    // A filha muda de mae sem chave nova na mae: sem cascata, sem marca.
    ok(
        pedir(
            porta,
            r#""op":"atualizar","database":"loja","tabela":"filha","rowid":1,
                   "valores":{"id":10,"cod":6}"#,
        ),
        "atualizar a filha",
    );
    assert!(marcas(&dir).is_empty(), "{:?}", marcas(&dir));
}

/// **A1: o panico no meio do fecho da janela nao apaga a marca de quem nao
/// foi ao disco.**
///
/// O `COMMIT` na `b` deixa a marca PENDENTE, esperando o `fsync` da `b`.
/// O fecho seguinte (o `bulkinsert(false)` da `a`, pela conexao) entra em
/// panico ao chegar na `b` -- e a `b` continua sem sincronizar, o erro de
/// E/S que prende a marca.
///
/// Com o defeito (o fecho DRENAVA as sujas antes do trabalho): o panico
/// levou a `b` junto com a lista, o reparo achou as sujas vazias e drenou
/// as marcas pendentes -- o bilhete de um commit cujo dado nunca passou
/// por `fsync` saiu do disco. Com o conserto a chave so sai depois do
/// `fsync`, e a marca fica ate ele acontecer.
#[test]
fn panico_no_fecho_da_janela_nao_apaga_a_marca_de_quem_nao_foi_ao_disco() {
    let dir = DirTemp::novo("panico-451-fecho");
    let mut c = config_base(&dir);
    janela_parada(&mut c);
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    duas_tabelas(porta);

    commit_em(porta, "b", 1);
    let pendente = marcas(&dir);
    assert_eq!(
        pendente.len(),
        1,
        "premissa: a marca do commit tinha de ficar PENDENTE, esperando o fsync"
    );

    *s.panico_no_fecho_de_teste.lock().unwrap() = Some(("loja/b".into(), String::new()));
    *s.fecho_falha_de_teste.lock().unwrap() = Some("loja/b".into());
    let mut carga = Ligacao::nova(porta);
    ok(
        falar(
            &mut carga,
            r#""op":"bulkinsert","database":"loja","tabela":"a","ligado":true"#,
        ),
        "bulkinsert true",
    );
    let morto = falar(
        &mut carga,
        r#""op":"bulkinsert","database":"loja","tabela":"a","ligado":false"#,
    );
    assert!(
        morto.is_none(),
        "o bulkinsert(false) tinha de cair no panico do fecho, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    assert!(
        s.panico_no_fecho_de_teste.lock().unwrap().is_none(),
        "o gancho do fecho nao disparou: o teste nao chegou ao meio do laco"
    );

    // O DANO primeiro: o bilhete do commit.
    assert_eq!(
        marcas(&dir),
        pendente,
        "a marca do commit SUMIU e a `b` nunca foi ao disco: o panico no meio \
             do fecho tirou a `b` das sujas, e o reparo drenou as marcas pendentes"
    );
    assert!(
        s.sujas.lock().unwrap().contains("loja/b"),
        "a `b` saiu das sujas sem ter sincronizado"
    );

    // Com a `b` voltando a sincronizar, o fecho seguinte leva a marca --
    // depois do `fsync`, e so entao.
    *s.fecho_falha_de_teste.lock().unwrap() = None;
    s.descarregar_sujas();
    assert!(
        marcas(&dir).is_empty(),
        "a marca nao saiu nem depois do fsync da `b`"
    );
    assert!(s.sujas.lock().unwrap().is_empty());
}

/// **Pedido 701 (d), refeito pelo 713.** Um `?` no meio do grupo da replica
/// -- aqui, a segunda tabela que nao abre mais -- com a primeira JA aplicada.
///
/// O 701 (d) queria a marca na lista da rodada, que a sincroniza e a SOLTA.
/// Com metade do grupo no disco isso e o F9: soltar a marca depois do
/// `fsync` perde o unico bilhete da metade que falta, e a venda fica pela
/// metade para sempre. Desde o 713 o grupo que para no meio se completa
/// pela marca na hora; quando nem isso fecha (a tabela continua fora do
/// lugar), a marca FICA no disco para o arranque e sai da lista da rodada.
///
/// A falta e montada de dentro da primeira inclusao do grupo (o gancho
/// roda nesta thread, com a trava na mao, e SEGUE): os arquivos da
/// segunda tabela saem do lugar depois da conferencia de posicao e antes
/// de o laco chegar nela.
///
/// Vermelho medido com o conserto do 713 reposto (a marca de volta na lista
/// e o `remove_file` da rodada): a marca sai do disco com metade do grupo.
#[test]
fn o_erro_no_meio_do_grupo_da_replica_deixa_a_marca_na_lista() {
    let dir = DirTemp::novo("grupo-699c-lista");
    let mut c = config_base(&dir);
    c.replicacao.imagem_da_linha = true;
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    let tabelas = ["itens", "pagamentos"];
    for db in ["origem", "loja"] {
        ok(
            pedir(
                porta,
                &format!(r#""op":"criar_database","database":"{db}""#),
            ),
            "criar_database",
        );
        for t in tabelas {
            ok(
                pedir(
                    porta,
                    &format!(
                        r#""op":"criar_tabela","database":"{db}","tabela":"{t}",
                               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}}]"#
                    ),
                ),
                "criar_tabela",
            );
        }
    }
    for t in tabelas {
        ok(
            pedir(
                porta,
                &format!(
                    r#""op":"inserir","database":"origem","tabela":"{t}","valores":{{"id":1}}"#
                ),
            ),
            "inserir na origem",
        );
    }
    let mut grupo = Vec::new();
    let mut filas = Vec::new();
    {
        let trava = s.travar_dados().unwrap();
        let origem = trava.abrir_database("origem").unwrap();
        for (i, t) in tabelas.iter().enumerate() {
            let (e, imagem) = origem
                .abrir_qualificada(t)
                .unwrap()
                .diario_com_imagem(0, 1)
                .unwrap()
                .remove(0);
            grupo.push((
                i,
                vec![crate::replica::EventoRecebido {
                    operacao: e.operacao,
                    rowid: e.rowid,
                    versao: e.versao,
                    imagem,
                    carimbo_ms: e.carimbo,
                    origem: 9,
                    posicao: 0,
                    tx: 0,
                }],
            ));
            filas.push(FilaDaReplica {
                no: crate::replica::NoSource {
                    nome: t.to_string(),
                    eventos: 1,
                    esquema: None,
                    proxima_sequencia: 0,
                },
                chave: format!("loja/{t}"),
                posicao: 0,
                conferir: false,
                conferencia: None,
                aplicados: 0,
                ordem: i,
            });
        }
    }
    let pasta = dir.join("loja");
    let fora = dir.join("fora");
    std::fs::create_dir_all(&fora).unwrap();
    phxsql_store::ndx::panico_de_teste::armar_gancho(Ponto::InserirDepoisDoContador, move || {
        for e in std::fs::read_dir(&pasta).unwrap().filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().starts_with("pagamentos") {
                std::fs::rename(e.path(), fora.join(e.file_name())).unwrap();
            }
        }
    });
    let mut marcas = Vec::new();
    let r = s.aplicar_grupo_da_replica("loja", &mut filas, grupo, "origem-de-teste", &mut marcas);
    phxsql_store::ndx::panico_de_teste::desarmar();
    assert!(
        r.is_err(),
        "a segunda tabela fora do lugar tinha de devolver o erro do meio do grupo"
    );
    let e = r.err().unwrap().to_string();
    assert!(e.contains("o grupo da replica parou no meio"), "{e}");
    assert!(
        marcas.is_empty(),
        "a marca do grupo pela metade ficou na lista que a rodada apaga depois do fsync"
    );
    let no_disco: Vec<_> = std::fs::read_dir(dir.join("loja"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("transacao_"))
        .collect();
    assert_eq!(
        no_disco.len(),
        1,
        "a marca do grupo pela metade saiu do disco: o arranque nao tem o que completar"
    );
}

/// **Pedido 700: o panico no meio do grupo do BIDIRECIONAL.** A marca do
/// grupo fica EM VOO como a da replica fiel, mas o reparo nao a completa:
/// o motor do reparo e o do rowid e da posicao, e a do bidirecional casa
/// pela chave. O reparo nao se afirma, a marca FICA, e o processo cai
/// para o arranque completa-la com a porta fechada.
///
/// O arranjo e o pior caso: a posicao do evento coincide com o tamanho do
/// diario daqui, e o motor do rowid aplicaria a alteracao sem recusar.
/// Vermelho medido sem o desvio no `reparo_da_trava`: o reparo «completa»,
/// devolve `Ok`, apaga a marca e o cliente 1 vira «novo» por cima.
#[test]
fn o_reparo_nao_completa_pelo_rowid_a_marca_do_bidi() {
    let dir = DirTemp::novo("panico-700-bidi");
    let mut c = config_base(&dir);
    c.replicacao.imagem_da_linha = true;
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear(porta);
    for nome in ["novo", "C1"] {
        ok(
            pedir(
                porta,
                &format!(
                    r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                           "valores":{{"id":1,"nome":"{nome}"}}"#
                ),
            ),
            "atualizar o cliente 1",
        );
    }
    let trava = s.travar_dados().unwrap();
    let mut t = trava
        .abrir_database("loja")
        .unwrap()
        .abrir_qualificada("clientes")
        .unwrap();
    let total = t.eventos().unwrap();
    let (velho, imagem) = t.diario_com_imagem(total - 2, 1).unwrap().remove(0);
    drop(t);
    assert_eq!(velho.operacao, Operacao::Alteracao);
    assert!(!imagem.is_empty(), "a alteracao tem de levar a imagem");
    let marca = crate::transacao::gravar_marca_do_bidi(
        &dir.join("loja"),
        777,
        crate::agora_ms(),
        &[crate::transacao::EventoDoGrupo {
            tabela: "clientes",
            operacao: Operacao::Alteracao,
            rowid: 1,
            carimbo_ms: velho.carimbo + 1,
            origem: 9,
            posicao: total,
            imagem: &imagem,
        }],
    )
    .unwrap();
    let em_voo = MarcaEmVoo {
        database: "loja".into(),
        caminho: marca.clone(),
        gravada: true,
    };
    let r = s.reparo_da_trava(&trava, Some(&em_voo));
    drop(trava);
    assert!(
        r.as_ref().is_err_and(|m| m.contains("bidirecional")),
        "o reparo completou pelo rowid a marca do bidirecional: {r:?}"
    );
    assert!(
        marca.exists(),
        "a marca do bidirecional saiu do disco no reparo"
    );
    assert_eq!(
        nome_do_cliente(porta, 1),
        "C1",
        "o reparo gravou pelo rowid"
    );
}

/// **M1: o reparo completa SO a marca em voo.**
///
/// Uma marca que nao e deste panico -- a do braco de erro do `COMMIT` que
/// ficou para o arranque, ou a de um `unlink` que falhou -- esta no disco,
/// e uma gravacao MAIS NOVA ja passou pela mesma linha. O panico vem numa
/// terceira tabela.
///
/// Com o defeito (o reparo varria todas as marcas): a velha foi completada
/// e reaplicou o `atualizar` sem condicao, desfazendo a gravacao mais
/// nova. Com o conserto o reparo nem a olha -- ela e do arranque.
#[test]
fn o_reparo_completa_so_a_marca_em_voo() {
    let dir = DirTemp::novo("panico-451-so-a-em-voo");
    let mut c = config_base(&dir);
    janela_parada(&mut c);
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    semear(porta);

    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    ok(
        falar(
            &mut tx,
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "valores":{"id":1,"nome":"velho"}"#,
        ),
        "atualizar na transacao",
    );
    ok(falar(&mut tx, r#""op":"commit""#), "commit");
    let pendente = marcas(&dir);
    assert_eq!(pendente.len(), 1, "premissa: a marca do commit, pendente");
    // A marca que NAO e deste panico: a mesma intencao, com outro nome.
    let alheia = dir.join("loja").join("transacao_999999.tx");
    std::fs::copy(dir.join("loja").join(&pendente[0]), &alheia).unwrap();

    ok(
        pedir(
            porta,
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "valores":{"id":1,"nome":"novo"}"#,
        ),
        "a gravacao mais nova",
    );
    assert_eq!(nome_do_cliente(porta, 1), "novo");

    armar(&s, "inserir");
    let morto = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"outra","valores":{"id":2}"#,
    );
    assert!(
        morto.is_none(),
        "o inserir armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );

    let nome = nome_do_cliente(porta, 1);
    assert_eq!(
        nome, "novo",
        "o reparo completou uma marca que NAO era do panico e desfez a \
             gravacao mais nova: o cliente 1 voltou para {nome:?}"
    );
    assert!(
        alheia.exists(),
        "o reparo apagou uma marca que nao era dele -- ela e do arranque"
    );
}

// --------------------------------------------- os processos filhos

/// A variavel que diz ao FILHO qual cenario montar. Ver
/// [`filho_do_panico_451`].
const CENARIO: &str = "PHXSQL_TESTE_451_CENARIO";

/// O que o filho do cenario `pausa_na_cascata_solta` diz no erro padrao
/// quando PAROU no meio da cascata -- e o pai so mata depois de ler isto.
const PAUSA_540: &str = "PHXSQL pausa de teste no meio da cascata solta (pedido 540)";

/// O que o filho do cenario `backup_pausado` diz quando a copia do backup
/// agendado PAROU com a ficha na mao (pedido 513).
const PAUSA_513: &str = "PHXSQL pausa de teste na copia do backup (pedido 513)";

/// Sobe o FILHO: este mesmo binario de testes, reexecutado so no
/// [`filho_do_panico_451`], com o cenario pedido.
///
/// # Por que um processo filho
///
/// O que estas provas medem e se o PROCESSO cai -- e um `abort` dentro do
/// binario de testes derrubaria a suite inteira. E o molde do
/// `comum::tracar_syscalls` do `phxsql-store`: nenhum gatilho novo mora no
/// binario de producao, nem no de depuracao -- os ganchos sao `cfg(test)`.
///
/// Pelo `sh`, com `ulimit -c 0`: o `abort` que a prova espera nao deixa
/// `core` na maquina de quem roda a suite com o limite aberto.
fn subir_filho(dir: &std::path::Path, cenario: &str) -> (std::process::Child, u16) {
    let eu = std::env::current_exe().expect("o proprio binario de testes");
    let nome = concat!(module_path!(), "::filho_do_panico_451");
    let nome = nome.split_once("::").map_or(nome, |(_, resto)| resto);
    let mut filho = std::process::Command::new("sh")
        .args(["-c", "ulimit -c 0; exec \"$0\" \"$@\""])
        .arg(eu)
        .args([
            nome,
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(FILHO, dir)
        .env(CENARIO, cenario)
        .env("RUST_BACKTRACE", "0")
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(dir.join("filho.err")).unwrap())
        .spawn()
        .expect("lancar o filho");
    let ate = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(p) = std::fs::read_to_string(dir.join("porta"))
            .ok()
            .and_then(|t| t.trim().parse().ok())
        {
            return (filho, p);
        }
        if Instant::now() > ate || filho.try_wait().unwrap().is_some() {
            let _ = filho.kill();
            let _ = filho.wait();
            panic!("o filho nao publicou a porta: {}", diagnostico(dir));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// O erro padrao do filho.
fn diagnostico(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("filho.err")).unwrap_or_default()
}

/// O destino de backup do cenario `fsync_backup` -- IRMAO de `dir`, nunca
/// FILHO dele. `dir` e' a raiz de dados (`config_base`), e `executar`
/// recusa um destino dentro da propria raiz (`backup.rs:391-395`) antes
/// de escrever byte nenhum: um destino aninhado faria o pedido de backup
/// falhar cedo demais para provar C1, e o teste passaria pelo motivo
/// errado.
fn destino_do_backup_c1(dir: &std::path::Path) -> std::path::PathBuf {
    dir.with_file_name(format!(
        "{}-backup-c1",
        dir.file_name().unwrap().to_string_lossy()
    ))
}

/// O destino do cenario `fsync_554`: uma pasta NOVA, irma de `dir` (a
/// raiz), para o backup ter de sincronizar a mae dela -- a pasta que
/// contem a raiz.
fn destino_do_backup_554(dir: &std::path::Path) -> std::path::PathBuf {
    dir.with_file_name(format!(
        "{}-backup-554",
        dir.file_name().unwrap().to_string_lossy()
    ))
}

/// Espera o filho terminar, ate o prazo. `None`: continua de pe.
fn fim_do_filho(
    filho: &mut std::process::Child,
    prazo: Duration,
) -> Option<std::process::ExitStatus> {
    let ate = Instant::now() + prazo;
    while Instant::now() < ate {
        if let Some(st) = filho.try_wait().unwrap() {
            return Some(st);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// O filho caiu pelo `abort` (SIGABRT), com a porta fechada.
fn caiu_pelo_abort(st: std::process::ExitStatus, dir: &std::path::Path, porta: u16) {
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(
        st.signal(),
        Some(6),
        "o filho caiu, mas nao pelo abort do piso: {st:?}\n{}",
        diagnostico(dir)
    );
    assert!(
        TcpStream::connect(("127.0.0.1", porta)).is_err(),
        "a porta do filho continua atendendo depois da queda"
    );
}

/// O que o processo de pe responde -- o dano, quando o defeito esta la.
fn o_que_ele_serve(porta: u16) -> Option<String> {
    pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":1000"#,
    )
    .map(|j| j.escrever())
}

/// Um `COMMIT` de 10, 11 e 12 em `clientes`, que o filho armado derruba.
fn commit_que_cai(porta: u16) {
    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    for id in [10, 11, 12] {
        ok(
            falar(
                &mut tx,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"clientes",
                       "valores":{{"id":{id},"nome":"T{id}"}}"#
                ),
            ),
            "inserir na transacao",
        );
    }
    let morto = falar(&mut tx, r#""op":"commit""#);
    assert!(
        morto.is_none(),
        "o commit armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
}

/// **(c) O piso, H5: o reparo que falha DERRUBA o processo.**
///
/// Com o defeito (a H1, ou um reparo que falha e segue): o processo fica
/// DE PE servindo a trava suja. Com o conserto: ele aborta (SIGABRT), e o
/// arranque seguinte completa a transacao que a marca confirmou.
#[cfg(unix)]
#[test]
fn reparo_que_falha_derruba_o_processo_em_vez_de_servir() {
    let dir = DirTemp::novo("panico-451-h5");
    let (mut filho, porta) = subir_filho(&dir, "reparo_falha");
    semear(porta);
    commit_que_cai(porta);
    let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(10)) else {
        let servindo = o_que_ele_serve(porta);
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "o reparo falhou e o processo seguiu DE PE, servindo de estado \
                 incerto: {servindo:?}"
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    assert!(
        diagnostico(&dir).contains("reparo da trava de dados FALHOU"),
        "o filho caiu sem dizer por que: {}",
        diagnostico(&dir)
    );
    // E o arranque repara: a marca confirmada completa, o indice em dia.
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a marca do commit tinha de estar no disco"
    );
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    assert!(marcas(&dir).is_empty(), "o arranque nao completou a marca");
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12]);
}

/// **Pedido 540, contra o SO: `SIGKILL` no meio da cascata SOLTA.**
///
/// O filho para o `atualizar` da mae na segunda gravacao de linha -- a
/// mae na chave nova, a primeira filha com o `.reg` gravado e o indice
/// para tras, a segunda filha intocada --, com a trava na mao, e morre
/// por `SIGKILL`: sem desenrolar, sem reparo, sem `Drop`. O arranque
/// seguinte tem de achar a cascata INTEIRA.
///
/// # Prova real
///
/// Sem a marca da cascata solta, o arranque reconstroi o indice marcado
/// (522) e a segunda filha fica na chave velha: `[6, 5]`, e nenhuma
/// marca no disco -- o vermelho medido. Com ela, a marca esta no disco
/// na hora da queda e o arranque a completa.
#[cfg(unix)]
#[test]
fn sigkill_no_meio_da_cascata_solta_o_arranque_a_completa() {
    use std::os::unix::process::ExitStatusExt;
    let dir = DirTemp::novo("sigkill-540");
    let (mut filho, porta) = subir_filho(&dir, "pausa_na_cascata_solta");
    semear_cascata(porta);
    let fio = std::thread::spawn(move || {
        pedir(
            porta,
            r#""op":"atualizar","database":"loja","tabela":"mae","rowid":1,
                   "valores":{"id":1,"codigo":6}"#,
        )
        .map(|j| j.escrever())
    });
    // O filho diz no erro padrao que parou -- ver `armar_pausa`.
    let parou =
        esperar_texto(Duration::from_secs(10), PAUSA_540, || diagnostico(&dir)).contains(PAUSA_540);
    let _ = filho.kill();
    let st = filho.wait().unwrap();
    let resposta = fio.join().unwrap();
    assert!(
        parou,
        "o filho nao parou no meio da cascata (respondeu {resposta:?}): {}",
        diagnostico(&dir)
    );
    assert_eq!(st.signal(), Some(9), "o filho nao caiu por SIGKILL: {st:?}");
    let marcas_na_queda = marcas(&dir).len();

    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    cascata_inteira(porta, &dir, "depois do SIGKILL e do arranque");
    assert_eq!(
        marcas_na_queda, 1,
        "a cascata solta caiu sem marca no disco para o arranque completar"
    );
}

/// **Pedido 509: o `fsync` recusado DERRUBA o processo, e a marca fica.**
///
/// O relogio fecha a janela a cada 150 ms. Depois de a semeadura ir ao
/// disco, o filho arma UM `fsync` recusado (EIO forjado) em toda a base, e
/// o `COMMIT` seguinte deixa a marca pendente esperando justamente o fecho
/// que vai encontrar a recusa -- no proprio `COMMIT` ou no relogio.
///
/// Com o defeito (o servidor que nao registra o gancho): o fecho recusa, o
/// seguinte recusa pelo diretorio envenenado, e o processo segue DE PE
/// gravando num disco que ja se sabe que mente -- medido: a `clientes`
/// recusa pelo byte 52 que ficou em 1, e o `inserir` na vizinha `outra`
/// responde com a recusa do 509, que o fecho da janela so da DEPOIS de a
/// linha ir ao `.reg` (o erro que grava, do 498). Sem o envenenamento
/// tambem, o fecho seguinte responde Ok e drena a marca: o 509 inteiro. Com
/// o conserto o filho cai pelo `abort` na recusa e a marca continua no
/// disco. Isto prova o FLUXO com recusa forjada, sem pagina perdida: se o
/// arranque no mesmo boot completa o que o disco perdeu, nao prova -- e o
/// papel C mediu que nao completa (o 509 segue aberto nessa metade).
#[cfg(unix)]
#[test]
fn fsync_recusado_derruba_o_processo_e_a_marca_fica() {
    let dir = DirTemp::novo("fsync-509");
    let (mut filho, porta) = subir_filho(&dir, "fsync_509");
    semear(porta);
    std::fs::write(dir.join("armar"), "").unwrap();
    let ate = Instant::now() + Duration::from_secs(20);
    while !dir.join("armado").exists() {
        assert!(
            Instant::now() < ate,
            "o filho nao armou a recusa: {}",
            diagnostico(&dir)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    for id in [10, 11, 12] {
        ok(
            falar(
                &mut tx,
                &format!(
                    r#""op":"inserir","database":"loja","tabela":"clientes",
                           "valores":{{"id":{id},"nome":"T{id}"}}"#
                ),
            ),
            "inserir na transacao",
        );
    }
    // O `COMMIT` pode responder (a janela adiou o fecho para o relogio)
    // ou cair (o fecho foi dele): nos dois a recusa vem depois da marca.
    let _ = falar(&mut tx, r#""op":"commit""#);
    drop(tx);

    let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(10)) else {
        let depois = pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"outra","valores":{"id":99}"#,
        )
        .map(|j| j.escrever());
        let sobrou = marcas(&dir);
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "o fsync foi recusado e o processo seguiu DE PE: o inserir numa \
                 tabela vizinha, depois da recusa, respondeu {depois:?}, e as \
                 marcas no disco sao {sobrou:?}\n{}",
            diagnostico(&dir)
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    assert!(
        diagnostico(&dir).contains("pedido 509"),
        "o filho caiu sem dizer por que: {}",
        diagnostico(&dir)
    );
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a marca do COMMIT tinha de ficar no disco: a queda nao drena nada"
    );
    // Pedido 509, saida (a): NO MESMO BOOT o arranque nao sobe -- o cache
    // do nucleo poderia estar devolvendo o que o disco perdeu.
    let sentinela = dir.join(super::SENTINELA_509);
    assert!(sentinela.exists(), "a queda nao deixou a sentinela");
    let e = Servidor::novo(config_base(&dir))
        .err()
        .expect("subiu no mesmo boot depois de um fsync recusado");
    assert!(e.to_string().contains("MESMO boot"), "{e}");
    assert_eq!(marcas(&dir).len(), 1, "a recusa de subir mexeu na marca");
    // Outro boot: a sentinela diz um boot que nao e este.
    let texto = std::fs::read_to_string(&sentinela).unwrap();
    let outro: String = texto
        .lines()
        .map(|l| {
            if l.starts_with("boot_id=") {
                "boot_id=um-boot-que-ja-passou".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&sentinela, outro).unwrap();
    // O arranque completa a transacao que a marca confirmou.
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(!sentinela.exists(), "a sentinela de outro boot nao saiu");
    let porta = porta_de_dados_de_verdade(&s);
    assert!(marcas(&dir).is_empty(), "o arranque nao completou a marca");
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12]);
}

/// Pedido 498, o resto: o `.log` que falha por DISCO CHEIO depois de a
/// linha estar no `.reg` derruba o processo pelo mesmo gancho do 509, e a
/// primeira abertura da tabela depois de subir completa o evento.
///
/// O filho arma UM ENOSPC forjado no `write` do evento (so depois da
/// semeadura), e o `inserir` do id 20 grava a linha e cai. Com o defeito
/// (sem o gancho) o filho segue DE PE e a linha fica sem diario -- o C2a
/// do 496, 294 linhas sem evento no tmpfs cheio. Sem a marca, ou sem a
/// cura na abertura, o processo cai mas o diario continua com 5 eventos.
///
/// E NAO e o 509: o disco que encheu nao mente, e o arranque no mesmo
/// boot sobe -- a sentinela do 509 nao pode nascer daqui.
#[cfg(unix)]
#[test]
fn diario_que_falha_depois_da_linha_derruba_e_a_abertura_completa() {
    let dir = DirTemp::novo("diario-498");
    let (mut filho, porta) = subir_filho(&dir, "diario_498");
    semear(porta);
    std::fs::write(dir.join("armar"), "").unwrap();
    let ate = Instant::now() + Duration::from_secs(20);
    while !dir.join("armado").exists() {
        assert!(
            Instant::now() < ate,
            "o filho nao armou a falha: {}",
            diagnostico(&dir)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let resposta = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":20,"nome":"T20"}"#,
    );
    let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(10)) else {
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "o diario falhou depois da linha e o processo seguiu DE PE, \
                 respondendo {:?}: linha sem diario com o servidor servindo\n{}",
            resposta.map(|j| j.escrever()),
            diagnostico(&dir)
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    assert!(
        diagnostico(&dir).contains("pedido 498"),
        "o filho caiu sem dizer por que: {}",
        diagnostico(&dir)
    );
    assert!(
        !dir.join(super::SENTINELA_509).exists(),
        "o disco cheio deixou a sentinela do 509, que impede subir no mesmo boot"
    );

    // Sobe no MESMO boot, e a primeira abertura completa.
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    assert!(
        ids(porta, "clientes").contains(&20),
        "a linha 20 nao esta no .reg -- a prova nao mediu o caso"
    );
    let mut t = Table::abrir(dir.join("loja"), "clientes").unwrap();
    let eventos = t.diario(0, 0).unwrap();
    let ultimo = eventos.last().expect("diario vazio");
    assert_eq!(
        eventos.len(),
        6,
        "a linha 20 ficou sem evento depois de subir: {eventos:?}"
    );
    assert_eq!(ultimo.operacao, phxsql_store::log::Operacao::Inclusao);
    let linha = t
        .ler(ultimo.rowid)
        .unwrap()
        .expect("o evento aponta o vazio");
    assert_eq!(
        linha[0],
        Value::Int(20),
        "o evento completado e de outra linha"
    );
}

/// Pedido 509 (a): sentinela que nao diz o boot nao se da por vencida --
/// o lado seguro e nao subir, e a recusa diz o arquivo.
#[test]
fn sentinela_sem_boot_nao_deixa_subir() {
    let dir = DirTemp::novo("sentinela-509-sem-boot");
    std::fs::create_dir_all(&dir.0).unwrap();
    std::fs::write(dir.join(super::SENTINELA_509), "caminho=/x\n").unwrap();
    let e = super::conferir_sentinela_509(&dir.0, |_, _| {}).unwrap_err();
    assert!(e.to_string().contains(super::SENTINELA_509), "{e}");
    assert!(dir.join(super::SENTINELA_509).exists());
}

/// **Pedido 524, condicao C1 do parecer do DBA: o `fsync` recusado no
/// DESTINO DE UM BACKUP nao pode derrubar o processo.**
///
/// O irmao exato do teste de cima -- mesma arma, mesmo `abort()` -- mas
/// aqui a recusa e' num disco que NAO e o do banco (o destino do backup,
/// nunca lido pela porta de dados). `backup::sincronizar_arquivo` usa
/// `sync_all_sem_abortar`: o `Err` sobe ao cliente e o gancho do 509
/// nunca roda. Com o defeito reposto (`sync_all` puro, com gancho, no
/// lugar de `sync_all_sem_abortar`), este teste FALHA: o filho cai pelo
/// mesmo `SIGABRT` do teste de cima, e `caiu_pelo_abort` (que aqui so'
/// seria usado para provar o contrario) mostraria a porta fechada onde
/// o teste espera resposta.
#[cfg(unix)]
#[test]
fn fsync_no_destino_do_backup_nao_derruba_o_servidor() {
    let dir = DirTemp::novo("fsync-backup-c1");
    let (mut filho, porta) = subir_filho(&dir, "fsync_backup");
    semear(porta);
    std::fs::write(dir.join("armar"), "").unwrap();
    let ate = Instant::now() + Duration::from_secs(20);
    while !dir.join("armado").exists() {
        assert!(
            Instant::now() < ate,
            "o filho nao armou a recusa: {}",
            diagnostico(&dir)
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let destino = destino_do_backup_c1(&dir).display().to_string();
    let resp = pedir(porta, &format!(r#""op":"backup","destino":"{destino}""#));

    // A porta continua respondendo -- o oposto de `caiu_pelo_abort`.
    let Some(resp) = resp else {
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "a conexao caiu sem resposta: o fsync do backup derrubou o \
                 processo em vez de virar erro ao cliente\n{}",
            diagnostico(&dir)
        );
    };
    assert!(
        filho.try_wait().unwrap().is_none(),
        "o filho caiu depois do pedido de backup: {}",
        diagnostico(&dir)
    );
    assert!(
        !resp.booleano_ou("ok", true),
        "o fsync recusado tinha de virar erro ao cliente, nao um backup \
             \"concluido\": {}",
        resp.escrever()
    );

    // O servidor continua DE PE, servindo o banco de verdade -- nao so
    // a mesma conexao: outro pedido, novo, do zero.
    assert!(
        o_que_ele_serve(porta).is_some(),
        "o servidor parou de responder depois da recusa no backup: {}",
        diagnostico(&dir)
    );

    let _ = filho.kill();
    let _ = filho.wait();
    // Irmao de `dir`, entao o `Drop` do `DirTemp` nao o alcanca.
    let _ = std::fs::remove_dir_all(destino_do_backup_c1(&dir));
}

/// **Pedido 554: a recusa do `fsync` no destino do backup nao pode
/// parar a escrita do banco.**
///
/// O irmao do teste de cima, um passo adiante: la o servidor fica de pe;
/// aqui o COMMIT seguinte tem de PASSAR. O destino e uma pasta nova ao
/// lado da raiz, e a arma recusa so o `fsync` da MAE dela -- a pasta que
/// CONTEM a raiz de dados. A marca dessa recusa ia para a lista do banco,
/// conferida por prefixo, e todo `fsync` da raiz recusava dali em diante:
/// um backup que falhou uma vez parava toda escrita ate reiniciar. Com o
/// defeito reposto (`recusar` no lugar de `recusar_fora` em
/// `sincronia::sync_all_interno`), este teste FALHA no `inserir`.
#[cfg(unix)]
#[test]
fn fsync_recusado_no_destino_do_backup_nao_para_o_commit() {
    let dir = DirTemp::novo("fsync-backup-554");
    let (mut filho, porta) = subir_filho(&dir, "fsync_554");
    semear(porta);
    std::fs::write(dir.join("armar"), "").unwrap();
    let ate = Instant::now() + Duration::from_secs(20);
    while !dir.join("armado").exists() {
        assert!(
            Instant::now() < ate,
            "o filho nao armou a recusa: {}",
            diagnostico(&dir)
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let destino = destino_do_backup_554(&dir).join("corrida");
    let resp = pedir(
        porta,
        &format!(r#""op":"backup","destino":"{}""#, destino.display()),
    );
    let resp = resp.unwrap_or_else(|| {
        let _ = filho.kill();
        let _ = filho.wait();
        panic!("a conexao caiu no backup: {}", diagnostico(&dir))
    });
    // O comportamento velho: a recusa no destino e erro do backup.
    assert!(
        !resp.booleano_ou("ok", true),
        "o fsync recusado tinha de virar erro do backup: {}",
        resp.escrever()
    );

    let commit = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "valores":{"id":6,"nome":"C6"}"#,
    );
    let _ = filho.kill();
    let _ = filho.wait();
    let _ = std::fs::remove_dir_all(destino_do_backup_554(&dir));
    let commit = commit.expect("a conexao caiu no inserir");
    assert!(
        commit.booleano_ou("ok", false),
        "a recusa no destino do backup parou a escrita do banco: {}",
        commit.escrever()
    );
}

/// **M3: a operacao IMPOSSIVEL depois do `completar` tambem derruba.**
///
/// O gatilho e o pedido 448 (a FK da transacao so e conferida depois da
/// marca): a filha com mae inexistente entra na lista, a passada morre na
/// PRIMEIRA escrita, e a completacao da marca em voo esbarra na FK. A
/// marca fica no disco (`NoArranque::Nao`) e o `AoSair` soltaria as travas
/// da transacao em seguida.
///
/// Com o defeito (o reparo que nao olha as impossiveis): o processo segue
/// de pe com a marca orfa no disco e as travas soltas. Com o conserto ele
/// cai, e o arranque a resolve com a porta fechada.
///
/// O 448 fechou, e a FK passou a recusar ANTES da marca: o cenario do
/// filho desliga a pre-conferencia (`pre_conferencia_desligada`) para o
/// gatilho continuar existindo. Sem isso o commit responde com erro em vez
/// de cair, e este teste diz isso em vez de passar por engano.
#[cfg(unix)]
#[test]
fn marca_em_voo_com_operacao_impossivel_derruba_o_processo() {
    let dir = DirTemp::novo("panico-451-impossivel");
    let (mut filho, porta) = subir_filho(&dir, "impossivel");
    ok(
        pedir(porta, r#""op":"criar_database","database":"loja""#),
        "criar_database",
    );
    ok(
        pedir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"nome","tipo":"Str(40)"}],
                   "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
        ),
        "criar_tabela clientes",
    );
    ok(
        pedir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"cliente_id","tipo":"Int8"}],
                   "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true},
                              {"nome":"por_cliente","colunas":["cliente_id"]}],
                   "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cliente_id"],
                                           "tabela_ref":"clientes","colunas_ref":["id"],
                                           "verificar":true,"ao_alterar":"cascata"}]"#,
        ),
        "criar_tabela pedidos",
    );
    ok(
        pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"clientes",
                   "valores":{"id":1,"nome":"Ana"}"#,
        ),
        "a mae",
    );
    let mut tx = Ligacao::nova(porta);
    ok(falar(&mut tx, r#""op":"begin","database":"loja""#), "begin");
    ok(
        falar(
            &mut tx,
            r#""op":"inserir","database":"loja","tabela":"pedidos",
                   "valores":{"id":1,"cliente_id":1}"#,
        ),
        "a filha com mae",
    );
    let orfa = falar(
        &mut tx,
        r#""op":"inserir","database":"loja","tabela":"pedidos",
               "valores":{"id":2,"cliente_id":999}"#,
    );
    assert!(
        orfa.as_ref().is_some_and(|j| j.booleano_ou("ok", false)),
        "o 448 fechou? a filha sem mae foi recusada ANTES da marca, e este \
             teste perdeu o gatilho da operacao impossivel: {:?}",
        orfa.map(|j| j.escrever())
    );
    let morto = falar(&mut tx, r#""op":"commit""#);
    assert!(
        morto.is_none(),
        "o commit armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    drop(tx);

    let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(10)) else {
        let sobrou = marcas(&dir);
        let servindo = pedir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"pedidos",
                   "valores":{"id":3,"cliente_id":1}"#,
        )
        .map(|j| j.escrever());
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "a marca em voo com operacao impossivel ficou no disco ({sobrou:?}) \
                 e o processo seguiu DE PE, com as travas da transacao soltas -- \
                 outra conexao gravou na tabela reservada: {servindo:?}"
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    assert!(
        diagnostico(&dir).contains("nao se completou"),
        "o filho caiu sem dizer que a marca em voo nao se completou: {}",
        diagnostico(&dir)
    );
    // O arranque a resolve: a primeira escrita fica (a ordem de digitacao
    // nao desfaz), a impossivel sai contada, a marca sai do disco.
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a marca em voo tinha de estar no disco"
    );
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    assert!(marcas(&dir).is_empty(), "o arranque nao resolveu a marca");
    let r = ok(
        pedir(
            porta,
            r#""op":"buscar","database":"loja","tabela":"pedidos","indice":"pk_id","chave":[1]"#,
        ),
        "buscar o pedido 1",
    );
    assert_eq!(r.inteiro_ou("encontrados", -1), 1, "{}", r.escrever());
}

/// **M4: a marca em voo JA GRAVADA que nao se rele derruba -- e FICA.**
///
/// O `COMMIT` grava e sincroniza a marca, a passada morre na primeira
/// escrita, e a releitura da marca no reparo falha com erro de E/S (o
/// `EMFILE` ou o `EIO` de verdade; aqui, injetado no `ler_marca`). Nao e
/// «commit que nunca comecou»: e a leitura que falhou.
///
/// Com o defeito (a falha contada como «nao confere»): a marca sai do
/// disco, e ou a trava volta a atender com a transacao pela metade, ou --
/// com o `completadas == 0` sozinho -- o processo cai e o arranque nao
/// acha bilhete nenhum. Com o conserto a marca fica, o processo cai, e o
/// arranque, com descritores novos, a le e completa: a transacao inteira.
#[cfg(unix)]
#[test]
fn marca_em_voo_que_nao_se_rele_derruba_o_processo_e_fica() {
    let dir = DirTemp::novo("panico-451-ilegivel");
    let (mut filho, porta) = subir_filho(&dir, "marca_ilegivel");
    semear(porta);
    commit_que_cai(porta);
    let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(10)) else {
        let servindo = o_que_ele_serve(porta);
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "a marca em voo JA GRAVADA nao se releu, o reparo a tratou como \
                 «nao confere», e a trava voltou a atender com a transacao \
                 confirmada pela metade: {servindo:?}"
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    // O DANO primeiro: o bilhete da transacao confirmada.
    assert_eq!(
        marcas(&dir).len(),
        1,
        "a marca confirmada SUMIU do disco: o reparo a apagou sem te-la lido, \
             e o arranque nao tem bilhete para completar a transacao"
    );
    assert!(
        diagnostico(&dir).contains("nao se releu"),
        "o filho caiu sem dizer que a marca nao se releu: {}",
        diagnostico(&dir)
    );
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    assert!(marcas(&dir).is_empty(), "o arranque nao completou a marca");
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12]);
}

/// **M3: o panico DENTRO do reparo e panico duplo, e derruba.**
///
/// O Rust aborta quando um panico escapa de um `Drop` no desenrolar -- e
/// o reparo mora num `Drop`. O que esta prova segura e que ninguem ponha
/// um `catch_unwind` em volta do reparo para «nao derrubar»: com ele, o
/// panico some, o reparo nao conta, e a trava fica fechada com o processo
/// de pe -- a H1 de volta, por dentro do conserto.
#[cfg(unix)]
#[test]
fn panico_dentro_do_reparo_derruba_o_processo() {
    let dir = DirTemp::novo("panico-451-duplo");
    let (mut filho, porta) = subir_filho(&dir, "panico_duplo");
    semear(porta);
    commit_que_cai(porta);
    let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(10)) else {
        let servindo = o_que_ele_serve(porta);
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "o panico DENTRO do reparo foi engolido e o processo seguiu DE PE: \
                 {servindo:?}"
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    assert!(
        diagnostico(&dir).contains("DENTRO do reparo"),
        "o filho caiu sem o panico do reparo no diagnostico: {}",
        diagnostico(&dir)
    );
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    assert!(marcas(&dir).is_empty(), "o arranque nao completou a marca");
    conferir_indice(porta, &[1, 2, 3, 4, 5, 10, 11, 12]);
}

/// **A2: o panico com a trava na mao numa thread de SERVICO derruba o
/// processo -- aqui, o relogio da janela.**
///
/// Com o defeito (o reparo cura a trava e deixa a thread morrer): o
/// processo fica de pe, e a janela para de fechar sozinha -- o ultimo
/// commit de uma rajada fica sem `fsync` e com a marca no disco para
/// sempre. Com o conserto o processo cai; o processo novo sobe com o
/// relogio, e a janela volta a fechar.
///
/// # Condicao observada, e nao prazo de parede (pedido 623)
///
/// A versao anterior mandava dois commits, dava 10 s ao filho para cair e,
/// de pe, 2 s ao relogio de 150 ms. Sob a suite inteira caiu sempre --
/// medido em 01/10/2026 com carga de `fsync` e CPU, 8 de 8 com o
/// diagnostico do filho VAZIO: o relogio nunca entrou em panico. A janela
/// fecha na propria gravacao quando ela chega 150 ms depois do ultimo
/// fecho (`Janela::hora_de_gravar`), e so fica pendente para o relogio a
/// gravacao que chega ANTES. Dois commits seguidos, com os nucleos
/// tomados, chegavam depois -- e a prova acusava o defeito sem o panico
/// ter acontecido.
///
/// Agora quatro conexoes gravam ate um de dois fatos aparecer: o filho
/// CAIU (o conserto), ou o diagnostico dele traz o relatorio do REPARO da
/// trava, que so sai quando a thread repara e segue (o defeito). Nenhum
/// dos dois depende de quanto o disco demora. O teto de 120 s so existe
/// para a prova nao pendurar, e vencido ela diz que nao provou nada.
#[cfg(unix)]
#[test]
fn panico_no_relogio_da_janela_derruba_o_processo_em_vez_de_parar_a_janela() {
    let dir = DirTemp::novo("panico-451-relogio");
    let (mut filho, porta) = subir_filho(&dir, "relogio");
    duas_tabelas(porta);
    // Sem conferir as respostas: com o conserto o processo cai NO MEIO
    // delas, assim que o relogio acha uma gravacao pendente.
    let parar = Arc::new(AtomicBool::new(false));
    let ids = Arc::new(AtomicU64::new(1));
    let gravadores: Vec<_> = (0..4)
        .map(|_| {
            let (parar, ids) = (Arc::clone(&parar), Arc::clone(&ids));
            std::thread::spawn(move || {
                while !parar.load(Ordering::SeqCst) {
                    let Some(mut tx) = Ligacao::tentar(porta) else {
                        return;
                    };
                    let id = ids.fetch_add(1, Ordering::SeqCst);
                    let _ = falar(&mut tx, r#""op":"begin","database":"loja""#);
                    let _ = falar(
                        &mut tx,
                        &format!(
                            r#""op":"inserir","database":"loja","tabela":"b","valores":{{"id":{id}}}"#
                        ),
                    );
                    let _ = falar(&mut tx, r#""op":"commit""#);
                }
            })
        })
        .collect();
    let ate = Instant::now() + Duration::from_secs(120);
    let fim = loop {
        if let Some(st) = filho.try_wait().unwrap() {
            break Some(st);
        }
        if diagnostico(&dir).contains("Reparo da trava de dados") {
            break None;
        }
        if Instant::now() >= ate {
            let _ = filho.kill();
            let _ = filho.wait();
            parar.store(true, Ordering::SeqCst);
            panic!(
                "em 120 s de commits o relogio-gravacao nao entrou em panico: a \
                     prova NAO provou nada, nem o defeito nem o conserto\n{}",
                diagnostico(&dir)
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    parar.store(true, Ordering::SeqCst);
    for g in gravadores {
        let _ = g.join();
    }
    let Some(st) = fim else {
        // O reparo rodou na thread de servico e o processo seguiu de pe.
        let sobrou = marcas(&dir);
        let _ = filho.kill();
        let _ = filho.wait();
        panic!(
            "o panico no relogio-gravacao nao derrubou o processo: a trava foi \
                 reparada e a thread morreu calada, e ninguem a sobe -- a janela para \
                 de fechar sozinha ({} marca(s) de commit no disco agora: {sobrou:?})\n{}",
            sobrou.len(),
            diagnostico(&dir)
        );
    };
    caiu_pelo_abort(st, &dir, porta);
    assert!(
        diagnostico(&dir).contains("SERVICO relogio-grav"),
        "o filho caiu sem nomear a thread de servico: {}",
        diagnostico(&dir)
    );
    // O processo novo, com o relogio: a janela fecha de novo. A condicao e
    // a marca sumir; o teto de 60 s so segura a prova de pendurar.
    let mut c = config_base(&dir);
    relogio_curto(&mut c);
    let s = Servidor::novo(c).unwrap();
    s.ligar_relogio_de_gravacao();
    let porta = porta_de_dados_de_verdade(&s);
    let proximo = i64::try_from(ids.load(Ordering::SeqCst)).unwrap();
    commit_em(porta, "b", proximo);
    commit_em(porta, "b", proximo + 1);
    let ate = Instant::now() + Duration::from_secs(60);
    while !marcas(&dir).is_empty() && Instant::now() < ate {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        marcas(&dir).is_empty(),
        "a janela nao fechou no processo novo: {:?}",
        marcas(&dir)
    );
}

/// **M2: o panico FORA da trava nao repara nem derruba.**
///
/// A conexao tem uma carga reservada e cai por um panico no `despachar`,
/// antes de qualquer trava. O `AoSair` dela solta a carga e, para isso,
/// descarrega as sujas -- tomando a trava no desenrolar. O filho tem o
/// reparo que falha armado: se o reparo rodar, o processo aborta.
///
/// Com o defeito (o portao so no `panicking()`): o reparo roda por um
/// panico que nunca tocou em dado, falha, e derruba o servidor de todos.
/// Com o conserto a conexao cai sozinha, a carga e solta, e o resto segue.
#[cfg(unix)]
#[test]
fn panico_fora_da_trava_nao_repara_nem_derruba() {
    let dir = DirTemp::novo("panico-451-fora");
    let (mut filho, porta) = subir_filho(&dir, "fora_da_trava");
    duas_tabelas(porta);
    let mut carga = Ligacao::nova(porta);
    ok(
        falar(
            &mut carga,
            r#""op":"bulkinsert","database":"loja","tabela":"a","ligado":true"#,
        ),
        "bulkinsert true",
    );
    ok(
        falar(
            &mut carga,
            r#""op":"inserir","database":"loja","tabela":"a","valores":{"id":1}"#,
        ),
        "inserir na carga",
    );
    let morto = falar(&mut carga, r#""op":"ping""#);
    assert!(
        morto.is_none(),
        "o ping armado tinha de cair no panico, e respondeu {:?}",
        morto.map(|j| j.escrever())
    );
    drop(carga);

    if let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(3)) {
        panic!(
            "um panico FORA da trava derrubou o processo: o `AoSair` tomou a \
                 trava no desenrolar e o reparo rodou (e, falhando, abortou) por um \
                 panico que nunca tocou em dado -- {st:?}\n{}",
            diagnostico(&dir)
        );
    }
    // A carga foi solta, e o servidor atende: outra conexao a reserva.
    let r = pedir(
        porta,
        r#""op":"bulkinsert","database":"loja","tabela":"a","ligado":true"#,
    );
    let _ = filho.kill();
    let _ = filho.wait();
    let r = r.expect("a reserva seguinte caiu sem resposta");
    assert!(
        r.booleano_ou("ok", false),
        "a carga da conexao que caiu nao foi solta: {}",
        r.escrever()
    );
    assert!(
        !diagnostico(&dir).contains("Reparo da trava"),
        "o reparo rodou por um panico de fora da trava: {}",
        diagnostico(&dir)
    );
}

/// O relogio da janela a cada 150 ms, e o lote fora de alcance: quem fecha
/// a janela quando ninguem grava e o relogio.
fn relogio_curto(c: &mut Config) {
    c.recursos.durabilidade = Durabilidade::PorLote;
    c.recursos.lote_operacoes = 1_000_000;
    c.recursos.lote_milissegundos = 150;
}

// ------------------------------------ pedido 502: job e backup agendados

/// O job que a prova do 502 agenda: um `inserir` na `loja.carga`, a cada
/// minuto -- vencido no arranque, porque nunca rodou nesta vida.
///
/// A `carga` nao tem indice de proposito: sem arvore, o `.ndx` nao fica
/// marcado para reindexar depois da queda, e o MESMO panico se repete no
/// arranque seguinte -- que e o laco que o pedido descreve. Com indice, o
/// segundo `inserir` seria recusado antes do ponto do panico, e o laco
/// nao apareceria na prova.
fn job_de_carga(dir: &std::path::Path) {
    std::fs::write(
        dir.join("jobs.json"),
        r#"{"jobs":[{"nome":"carga","ligado":true,"cada_minutos":1,"usuario":"",
                "pedido":{"op":"inserir","database":"loja","tabela":"carga",
                          "valores":{"n":1}}}]}"#,
    )
    .unwrap();
}

/// A `loja.clientes` criada POR DENTRO do filho: o relogio roda o job na
/// primeira volta, antes de o pai ter porta para semear. `let _`: no
/// segundo arranque ela ja existe.
fn loja_por_dentro(s: &Servidor) {
    let sessao = Sessao::default();
    let _ = s.executar(
        "criar_database",
        &Json::analisar(r#"{"database":"loja"}"#).unwrap(),
        &sessao,
    );
    let _ = s.executar(
        "criar_tabela",
        &Json::analisar(
            r#"{"database":"loja","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
        )
        .unwrap(),
        &sessao,
    );
    let _ = s.executar(
        "criar_tabela",
        &Json::analisar(
            r#"{"database":"loja","tabela":"carga","colunas":[{"nome":"n","tipo":"Int8"}]}"#,
        )
        .unwrap(),
        &sessao,
    );
}

/// O backup agendado «a cada 24 h»: vence no arranque, que e o que o 502
/// descreve.
fn backup_agendado(c: &mut Config, dir: &std::path::Path) {
    c.backup.agendado = true;
    c.backup.destino = destino_do_backup_agendado(dir);
    c.backup.hora = String::new();
    c.backup.cada_horas = 24;
}

/// O destino do backup agendado dos cenarios `backup` e `backup_pausado`
/// -- IRMAO de `dir`, pelo mesmo motivo do `destino_do_backup_c1`. Era
/// `dir/backups`, dentro da raiz, e a recusa do destino nunca aparecia
/// porque o gancho de teste disparava ANTES da copia; com a fase 1 do
/// 513 (passo 2) rodando antes do gancho, a recusa saia primeiro e a
/// prova caia pelo motivo errado. Quem usa apaga a pasta no fim.
fn destino_do_backup_agendado(dir: &std::path::Path) -> std::path::PathBuf {
    dir.with_file_name(format!(
        "{}-backups",
        dir.file_name().unwrap().to_string_lossy()
    ))
}

/// O `.log` das corridas dos jobs do filho.
fn corridas(dir: &std::path::Path) -> String {
    std::fs::read_to_string(crate::config::irmao_do_log(&dir.join("jobs.json"))).unwrap_or_default()
}

/// Espera o `texto` aparecer no que `ler` devolve, ate o prazo.
fn esperar_texto(prazo: Duration, texto: &str, ler: impl Fn() -> String) -> String {
    let ate = Instant::now() + prazo;
    loop {
        let agora = ler();
        if agora.contains(texto) || Instant::now() > ate {
            return agora;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// O segundo arranque no MESMO diretorio: a porta do primeiro sai antes,
/// senao o pai leria o numero velho.
fn subir_de_novo(dir: &std::path::Path, cenario: &str) -> (std::process::Child, u16) {
    let _ = std::fs::remove_file(dir.join("porta"));
    subir_filho(dir, cenario)
}

/// **502 (a): o job em panico com a trava de ESCRITA na mao vira corrida
/// que FALHOU, e o servidor fica de pe.**
///
/// Com o defeito (a corrida na propria thread `relogio-jobs`, familia
/// `servico`): o processo cai por `SIGABRT` na primeira volta do relogio.
/// Com o conserto: a filha repara a trava, o relogio anota a falha com o
/// texto do panico, e a porta continua atendendo.
#[cfg(unix)]
#[test]
fn job_em_panico_sob_a_trava_vira_corrida_que_falhou_e_o_servidor_fica() {
    let dir = DirTemp::novo("panico-502-job");
    let (mut filho, porta) = subir_filho(&dir, "job");
    if let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(3)) {
        panic!(
            "o job em panico DERRUBOU o servidor ({st:?}) -- e ele roda de novo \
                 a cada arranque: {}",
            diagnostico(&dir)
        );
    }
    let log = esperar_texto(Duration::from_secs(5), "PANICO", || corridas(&dir));
    let servindo = pedir(porta, r#""op":"ping""#);
    let _ = filho.kill();
    let _ = filho.wait();
    assert!(
        log.contains("\"ok\":false") && log.contains("PANICO"),
        "a corrida em panico nao virou corrida que FALHOU no historico: {log}"
    );
    ok(servindo, "o servidor de pe depois do panico do job");
}

/// **502 (b): a corrida que derrubou o processo NAO roda de novo no
/// arranque -- o laco de quedas acaba.**
///
/// O primeiro arranque cai de proposito: o reparo da trava falha (a H5,
/// que continua valendo, pedido 451), e o processo aborta NO MEIO da
/// corrida. O segundo arranque tem o mesmo job ligado e o mesmo panico
/// armado. Com o defeito (sem a lapide): o relogio roda o job logo na
/// partida, e o processo cai de novo -- o laco que so parava quando alguem
/// desligasse o job a mao. Com o conserto: a corrida aberta vira FALHOU no
/// historico, conta como a ultima, e o servidor fica de pe.
#[cfg(unix)]
#[test]
fn corrida_de_job_que_derrubou_o_processo_nao_roda_de_novo_no_arranque() {
    let dir = DirTemp::novo("panico-502-lapide-job");
    let (mut filho, porta) = subir_filho(&dir, "job_reparo_falha");
    let st = fim_do_filho(&mut filho, Duration::from_secs(10))
        .unwrap_or_else(|| panic!("o reparo que falha tinha de derrubar (H5)"));
    caiu_pelo_abort(st, &dir, porta);

    let (mut filho, porta) = subir_de_novo(&dir, "job_reparo_falha");
    if let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(3)) {
        panic!(
            "LACO DE QUEDAS: o job que derrubou o processo rodou DE NOVO no \
                 arranque e derrubou outra vez ({st:?}): {}",
            diagnostico(&dir)
        );
    }
    let servindo = pedir(porta, r#""op":"ping""#);
    let _ = filho.kill();
    let _ = filho.wait();
    ok(servindo, "o servidor de pe no segundo arranque");
    let log = corridas(&dir);
    assert_eq!(
        log.matches("\"em_curso\":true").count(),
        1,
        "o job rodou DE NOVO no arranque (cada abertura e uma corrida): {log}"
    );
    assert!(
        log.contains("nunca terminou"),
        "a corrida interrompida nao virou FALHOU no historico: {log}"
    );
}

/// **502 (c): o backup agendado em panico com a ficha da copia na mao
/// vira backup que FALHOU, e o servidor fica de pe.** Desde o 513 a
/// ficha e a COMPARTILHADA, e o panico tem de religar a escrita tambem:
/// o `ping` passa de qualquer jeito, entao a prova grava depois.
///
/// Com o defeito (o backup na propria thread `backup-agendado`): `SIGABRT`
/// na primeira volta. Com o conserto: a falha sai no erro padrao, com o
/// texto do panico, e a porta continua atendendo.
#[cfg(unix)]
#[test]
fn backup_em_panico_sob_a_trava_falha_e_o_servidor_fica() {
    let dir = DirTemp::novo("panico-502-backup");
    let (mut filho, porta) = subir_filho(&dir, "backup");
    if let Some(st) = fim_do_filho(&mut filho, Duration::from_secs(3)) {
        panic!(
            "o backup em panico DERRUBOU o servidor ({st:?}) -- e ele roda de \
                 novo a cada arranque: {}",
            diagnostico(&dir)
        );
    }
    let erro = esperar_texto(Duration::from_secs(5), "PANICO", || diagnostico(&dir));
    let servindo = pedir(porta, r#""op":"ping""#);
    // O portao do retrato (pedido 513) religado pelo `Drop` no
    // desenrolar: sem ele, esta gravacao esperaria para sempre.
    let gravou = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"carga","valores":{"n":1}"#,
    );
    let _ = filho.kill();
    let _ = filho.wait();
    let _ = std::fs::remove_dir_all(destino_do_backup_agendado(&dir));
    assert!(
        erro.contains("backup agendado FALHOU") && erro.contains("PANICO"),
        "o panico do backup nao virou falha dita: {erro}"
    );
    ok(servindo, "o servidor de pe depois do panico do backup");
    ok(gravou, "a escrita depois do panico do backup");
}

/// **502 (d): o backup que derrubou o processo NAO roda de novo no
/// arranque.** O irmao do (b), pela lapide no destino.
///
/// A queda era o `abort` do reparo que falha (H5), com o backup segurando
/// a ficha EXCLUSIVA. Desde o 513 a copia segura a COMPARTILHADA, que nao
/// grava nada e por isso nao tem reparo a fazer -- e o processo cai por
/// `SIGKILL` no meio da copia, que e a queda de verdade que a lapide
/// existe para lembrar. Com o defeito (sem lapide), o segundo arranque
/// para de novo na pausa, e o aviso dela aparece.
#[cfg(unix)]
#[test]
fn corrida_de_backup_que_derrubou_o_processo_nao_roda_de_novo_no_arranque() {
    let dir = DirTemp::novo("panico-502-lapide-backup");
    let (mut filho, _porta) = subir_filho(&dir, "backup_pausado");
    let parou = esperar_texto(Duration::from_secs(10), PAUSA_513, || diagnostico(&dir));
    let _ = filho.kill();
    let _ = filho.wait();
    assert!(
        parou.contains(PAUSA_513),
        "o backup agendado nunca chegou a copia: {parou}"
    );

    let (mut filho, porta) = subir_de_novo(&dir, "backup_pausado");
    let segundo = esperar_texto(Duration::from_secs(3), PAUSA_513, || diagnostico(&dir));
    let servindo = pedir(porta, r#""op":"ping""#);
    let _ = filho.kill();
    let _ = filho.wait();
    let _ = std::fs::remove_dir_all(destino_do_backup_agendado(&dir));
    assert!(
        !segundo.contains(PAUSA_513),
        "LACO DE QUEDAS: o backup que derrubou o processo rodou DE NOVO no \
             arranque: {segundo}"
    );
    ok(servindo, "o servidor de pe no segundo arranque");
    assert!(
        diagnostico(&dir).contains("nunca terminou"),
        "o arranque nao disse que a corrida anterior caiu: {}",
        diagnostico(&dir)
    );
}

/// O FILHO das provas de processo -- so roda reexecutado por elas, que
/// passam o diretorio pela variavel [`FILHO`] e o cenario pela
/// [`CENARIO`]. Sozinho, volta sem fazer nada.
///
/// Sobe o servidor com o cenario armado, publica a porta num arquivo e
/// espera. Se ainda estiver aqui depois do prazo, o panico nao derrubou o
/// processo -- e o pai decide se isso e o defeito ou o certo.
#[test]
#[ignore = "so roda reexecutado pelas provas de processo do pedido 451"]
fn filho_do_panico_451() {
    let Some(dir) = std::env::var_os(FILHO) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let cenario = std::env::var(CENARIO).unwrap_or_default();
    let mut c = config_base(&dir);
    if cenario == "relogio" || cenario == "fsync_509" {
        relogio_curto(&mut c);
    }
    // Pedido 502: o cadastro de jobs e lido no `Servidor::novo`, entao
    // nasce antes; o backup agendado e configuracao.
    if cenario.starts_with("job") {
        job_de_carga(&dir);
    }
    if cenario.starts_with("backup") {
        backup_agendado(&mut c, &dir);
    }
    let s = Servidor::novo(c).unwrap();
    match cenario.as_str() {
        "job" => {
            loja_por_dentro(&s);
            armar(&s, "inserir");
        }
        "job_reparo_falha" => {
            loja_por_dentro(&s);
            s.reparo_falha_de_teste.store(true, Ordering::SeqCst);
            armar(&s, "inserir");
        }
        "backup" => {
            loja_por_dentro(&s);
            *s.panico_de_teste_na_op.lock().unwrap() =
                Some(("backup_agendado".into(), PanicoDeTeste::Aqui));
        }
        // Pedido 513: a copia do backup agendado PARA com a ficha na mao,
        // e o pai mata o processo por `SIGKILL` no meio dela.
        "backup_pausado" => {
            loja_por_dentro(&s);
            *s.panico_de_teste_na_op.lock().unwrap() = Some((
                "backup_agendado".into(),
                PanicoDeTeste::Pausa(Duration::from_secs(50), PAUSA_513),
            ));
        }
        "reparo_falha" => {
            s.reparo_falha_de_teste.store(true, Ordering::SeqCst);
            armar(&s, "commit");
        }
        // Pedido 540: o `atualizar` solto da mae PARA na segunda gravacao
        // de linha -- a mae ja foi, a primeira filha no meio, a segunda
        // nao --, com a trava na mao, e o pai o mata por SIGKILL.
        "pausa_na_cascata_solta" => {
            *s.panico_de_teste_na_op.lock().unwrap() = Some((
                "atualizar".into(),
                PanicoDeTeste::PausaNoMotor(Ponto::AtualizarDepoisDoReg, 2, PAUSA_540),
            ));
        }
        "impossivel" => {
            // Desde o 448 a FK da lista e conferida ANTES da marca, e o
            // gatilho natural deste cenario (a filha com mae inexistente)
            // passou a ser recusado com zero gravado. O braco das
            // impossiveis continua existindo para o que a pre-conferencia
            // nao ve (a FK que vem do DEFAULT, pedido 514, e o disco que
            // muda entre o plano e a passada), entao o cenario desliga a
            // pre-conferencia -- o proprio defeito reposto do 448, so em
            // `cfg(test)` -- para a FK chegar a passada como antes.
            s.pre_conferencia_desligada.store(true, Ordering::SeqCst);
            armar(&s, "commit");
        }
        "marca_ilegivel" => {
            s.marca_ilegivel_de_teste.store(true, Ordering::SeqCst);
            armar(&s, "commit");
        }
        "panico_duplo" => {
            s.reparo_panica_de_teste.store(true, Ordering::SeqCst);
            armar(&s, "commit");
        }
        "relogio" => {
            s.ligar_relogio_de_gravacao();
            *s.panico_no_fecho_de_teste.lock().unwrap() =
                Some((String::new(), "relogio-grav".into()));
        }
        "fora_da_trava" => {
            s.reparo_falha_de_teste.store(true, Ordering::SeqCst);
            *s.panico_de_teste_na_op.lock().unwrap() =
                Some(("ping".into(), PanicoDeTeste::ForaDaTrava));
        }
        // Pedido 509: o pai cria `armar`, e o filho arma UM `fsync`
        // recusado em toda a base -- mas so depois de o relogio levar ao
        // disco o que a semeadura sujou, para a recusa cair no fecho que a
        // marca do COMMIT espera, e nao no da semeadura.
        "fsync_509" => {
            s.ligar_relogio_de_gravacao();
            let (s, dir) = (Arc::clone(&s), dir.clone());
            std::thread::spawn(move || {
                while !dir.join("armar").exists() {
                    std::thread::sleep(Duration::from_millis(10));
                }
                while s.janela.pendente() > 0
                    || !s.sujas.lock().map(|x| x.is_empty()).unwrap_or(false)
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                phxsql_store::sincronia::falha_de_teste::armar(
                    &dir,
                    phxsql_store::sincronia::falha_de_teste::Onde::Fsync,
                    1,
                );
                std::fs::write(dir.join("armado"), "").unwrap();
            });
        }
        // Pedido 498: o pai cria `armar`, e o filho arma UM ENOSPC no
        // `write` do proximo evento de diario da base -- o do `inserir`
        // que o pai manda depois, com a linha ja no `.reg`.
        "diario_498" => {
            let dir = dir.clone();
            std::thread::spawn(move || {
                while !dir.join("armar").exists() {
                    std::thread::sleep(Duration::from_millis(10));
                }
                phxsql_store::sincronia::falha_de_teste::armar(
                    &dir,
                    phxsql_store::sincronia::falha_de_teste::Onde::GravacaoDoDiario,
                    1,
                );
                std::fs::write(dir.join("armado"), "").unwrap();
            });
        }
        // Pedido 524, condicao C1: arma a recusa num destino de backup
        // (um disco QUE NAO E o do banco), nao na raiz de dados inteira
        // -- e' a distincao que este cenario prova, entao ele NAO pode
        // reusar o `fsync_509` (que arma a raiz toda).
        "fsync_backup" => {
            let dir = dir.clone();
            std::thread::spawn(move || {
                while !dir.join("armar").exists() {
                    std::thread::sleep(Duration::from_millis(10));
                }
                let destino = destino_do_backup_c1(&dir);
                phxsql_store::sincronia::falha_de_teste::armar(
                    &destino,
                    phxsql_store::sincronia::falha_de_teste::Onde::Fsync,
                    1,
                );
                std::fs::write(dir.join("armado"), "").unwrap();
            });
        }
        // Pedido 554: arma a recusa so no `fsync` da pasta que CONTEM a
        // raiz -- a mae do destino novo. A marca de uma pasta e gravada
        // pelo manifesto dentro dela, entao o prefixo e
        // `<mae>/backup.json`, e nao alcanca nenhum arquivo do banco.
        "fsync_554" => {
            let dir = dir.clone();
            std::thread::spawn(move || {
                while !dir.join("armar").exists() {
                    std::thread::sleep(Duration::from_millis(10));
                }
                let mae = destino_do_backup_554(&dir)
                    .parent()
                    .unwrap()
                    .join("backup.json");
                phxsql_store::sincronia::falha_de_teste::armar(
                    &mae,
                    phxsql_store::sincronia::falha_de_teste::Onde::Fsync,
                    1,
                );
                std::fs::write(dir.join("armado"), "").unwrap();
            });
        }
        outro => panic!("cenario desconhecido: {outro:?}"),
    }
    let porta = porta_de_dados_de_verdade(&s);
    std::fs::write(dir.join("porta"), porta.to_string()).unwrap();
    // As threads de servico do 502 sobem DEPOIS da porta publicada: a
    // queda que a prova espera (ou recusa) acontece na primeira volta
    // delas, e o pai precisa da porta para dizer o que o processo servia.
    if cenario.starts_with("job") {
        s.subir_jobs();
    }
    if cenario.starts_with("backup") {
        s.subir_backup_agendado();
    }
    std::thread::sleep(Duration::from_secs(60));
}
