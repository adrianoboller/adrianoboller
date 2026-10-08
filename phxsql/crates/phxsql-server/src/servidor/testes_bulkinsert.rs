//! `BULKINSERT`: a tabela reservada para uma carga.
//!
//! O que estes testes travam, em ordem de importancia:
//!
//! 1. **a reserva de fato barra o outro**, e o recado diz quem reservou;
//! 2. **o dono continua trabalhando** -- reserva que barra o proprio dono seria
//!    so uma forma cara de derrubar o servico;
//! 3. **a queda da conexao solta** -- e a primeira rede contra reserva orfa;
//! 4. **o prazo solta** -- e a segunda, para o soquete pendurado vivo;
//! 5. **o erro e repetivel**: `EM_CARGA` diz `repetir: true`, e e o que separa
//!    «espere um pouco» de «voce nao pode».
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("bulk-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn com_tabela(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    s
}

/// Pelo `despachar`, que e onde mora o portao da carga.
fn pede(s: &Arc<Servidor>, ligacao: u64, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        ligacao,
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

const RESERVA: &str = r#""op":"bulkinsert","database":"b","tabela":"c","ligado":true"#;
const SOLTA: &str = r#""op":"bulkinsert","database":"b","tabela":"c","ligado":false"#;
const INSERE: &str = r#""op":"inserir","database":"b","tabela":"c","linha":{"id":1}"#;

/// O caso do enunciado: reservada, o outro nao entra -- e sabe por quem.
#[test]
fn reservada_barra_o_outro_e_diz_quem() {
    let dir = dir_temp("barra");
    let s = com_tabela(&dir);
    pede(&s, 1, RESERVA).unwrap();

    let e = pede(&s, 2, INSERE).unwrap_err();
    assert_eq!(e.nome(), "EM_CARGA");
    assert_eq!(e.codigo(), 4002);
    let texto = e.to_string();
    assert!(
        texto.contains("ligacao 1"),
        "nao disse quem reservou: {texto}"
    );
    assert!(texto.contains("b.c"), "nao disse qual tabela: {texto}");
}

/// A leitura tambem para. E de proposito: deixar ler durante a carga e o
/// que impediria adiar o indice mais tarde.
#[test]
fn a_leitura_do_outro_tambem_para() {
    let dir = dir_temp("leitura");
    let s = com_tabela(&dir);
    pede(&s, 1, RESERVA).unwrap();
    let e = pede(&s, 2, r#""op":"varrer","database":"b","tabela":"c""#).unwrap_err();
    assert_eq!(e.nome(), "EM_CARGA");
}

/// Uma tabela reservada nao barra a tabela do lado.
#[test]
fn a_reserva_e_de_uma_tabela_so() {
    let dir = dir_temp("outra");
    let s = com_tabela(&dir);
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"d",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    pede(&s, 1, RESERVA).unwrap();
    assert!(pede(&s, 2, r#""op":"varrer","database":"b","tabela":"d""#).is_ok());
}

/// Quem reservou continua trabalhando -- senao a reserva seria so uma
/// forma cara de derrubar o proprio servico.
#[test]
fn o_dono_continua_gravando() {
    let dir = dir_temp("dono");
    let s = com_tabela(&dir);
    pede(&s, 1, RESERVA).unwrap();
    for i in 1..=50 {
        pede(
            &s,
            1,
            &format!(r#""op":"inserir","database":"b","tabela":"c","linha":{{"id":{i}}}"#),
        )
        .unwrap();
    }
    let r = pede(&s, 1, SOLTA).unwrap();
    assert!(matches!(r.campo("liberada"), Some(Json::Bool(true))));
    assert!(matches!(r.campo("sincronizada"), Some(Json::Bool(true))));

    // Solta, o outro entra.
    let v = pede(&s, 2, r#""op":"varrer","database":"b","tabela":"c""#).unwrap();
    assert_eq!(v.inteiro_ou("devolvidas", -1), 50);
}

/// A PRIMEIRA rede: a conexao caiu, a tabela solta.
#[test]
fn a_queda_da_conexao_solta() {
    let dir = dir_temp("queda");
    let s = com_tabela(&dir);
    pede(&s, 1, RESERVA).unwrap();
    assert!(pede(&s, 2, INSERE).is_err());

    // E o que o `AoSair` do laco da conexao chama.
    s.soltar_cargas_da_ligacao(1);
    assert!(
        pede(&s, 2, INSERE).is_ok(),
        "a reserva sobreviveu a queda da conexao"
    );
}

/// A SEGUNDA rede: o prazo vence mesmo com o soquete pendurado vivo.
#[test]
fn o_prazo_solta() {
    let dir = dir_temp("prazo");
    let s = com_tabela(&dir);
    // Reserva com prazo ja vencido, direto no registro: e o estado em que
    // um cliente morto com o TCP vivo deixaria a tabela.
    let agora = crate::agora_ms();
    s.cargas
        .lock()
        .unwrap()
        .reservar("b", "c", "fulano", 1, "1.2.3.4", agora - 60_000, 1_000)
        .unwrap();
    assert!(
        pede(&s, 2, INSERE).is_ok(),
        "a reserva vencida continuou barrando"
    );
    assert_eq!(
        s.cargas.lock().unwrap().quantas(),
        0,
        "a reserva vencida ficou na lista"
    );
}

/// Reservar de novo o que ja e meu renova o prazo, em vez de recusar.
#[test]
fn reservar_de_novo_o_meu_renova() {
    let dir = dir_temp("renova");
    let s = com_tabela(&dir);
    pede(&s, 1, RESERVA).unwrap();
    let r = pede(&s, 1, RESERVA).unwrap();
    assert!(matches!(r.campo("reservada"), Some(Json::Bool(true))));
}

/// `EM_CARGA` e passageiro, e o protocolo tem de dizer isso: e o que
/// separa «espere um pouco» de «voce nao pode».
#[test]
fn em_carga_pede_nova_tentativa() {
    let dir = dir_temp("repetir");
    let s = com_tabela(&dir);
    pede(&s, 1, RESERVA).unwrap();
    let e = pede(&s, 2, INSERE).unwrap_err();
    assert!(e.adianta_repetir(), "EM_CARGA deveria pedir nova tentativa");
    assert!(
        !PhxError::Autorizacao(String::new()).adianta_repetir(),
        "e ACESSO_NEGADO nao"
    );
}

/// Reservar tabela que nao existe recusa na hora, em vez de esconder o
/// erro de digitacao ate o fim da carga.
#[test]
fn reservar_tabela_que_nao_existe_recusa() {
    let dir = dir_temp("inexistente");
    let s = com_tabela(&dir);
    assert!(pede(
        &s,
        1,
        r#""op":"bulkinsert","database":"b","tabela":"nao_existe","ligado":true"#
    )
    .is_err());
}

/// Pela web nao vale: HTTP nao tem conexao para a reserva morrer amarrada.
#[test]
fn pela_web_recusa_com_o_motivo() {
    let dir = dir_temp("web");
    let s = com_tabela(&dir);
    let e = pede(&s, 0, RESERVA).unwrap_err();
    assert!(
        e.to_string().contains("porta de dados"),
        "o recado nao explica: {e}"
    );
}

// ------------------------------- quem esconde a tabela do portao (322)
//
// O portao da carga lia so `"tabela"`. Cada teste abaixo pede a tabela
// reservada `c` por um campo que nao e esse, e todos caem com o portao
// lendo um campo so. A prova pelo soquete mora em
// `tests/carga-pelo-lado-b.rs`; aqui fica a familia inteira.

/// `com_tabela` e mais uma vizinha `d`, com o mesmo indice: as operacoes
/// de duas tabelas precisam de duas.
fn com_vizinha(dir: &std::path::Path) -> Arc<Servidor> {
    let s = com_tabela(dir);
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"d",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    s
}

const JUNTA_B: &str = r#""op":"juntar","database":"b",
        "a":{"tabela":"d","chave":"id"},"b":{"tabela":"c","chave":"id"}"#;

fn em_carga(s: &Arc<Servidor>, corpo: &str) {
    let e = pede(s, 2, corpo).expect_err(corpo);
    assert_eq!(e.nome(), "EM_CARGA", "{corpo} -> {e}");
    assert!(e.to_string().contains("b.c"), "nao disse qual: {e}");
}

#[test]
fn o_lado_b_do_juntar_nao_contorna_a_reserva() {
    let dir = dir_temp("lado-b");
    let s = com_vizinha(&dir);
    pede(&s, 1, RESERVA).unwrap();
    em_carga(&s, JUNTA_B);
}

#[test]
fn diferencas_unir_e_pivotar_nao_contornam_a_reserva() {
    let dir = dir_temp("familia");
    let s = com_vizinha(&dir);
    pede(&s, 1, RESERVA).unwrap();
    // Todas antes de reprovar: parar na primeira esconderia as outras.
    let passaram: Vec<String> = [
        r#""op":"diferencas","database":"b","a":"d","b":"c","indice":"porId""#,
        r#""op":"unir","database":"b","tabelas":["d","c"]"#,
        r#""op":"pivotar","database":"b","tabela":"d",
               "juntar":[{"tabela":"c","coluna":"id","prefixo":"f","chave":"id"}],
               "linhas":[{"campo":"f.id"}],"colunas":[{"campo":"id"}],
               "agregador":"contagem""#,
    ]
    .iter()
    .filter(|corpo| !matches!(pede(&s, 2, corpo), Err(ref e) if e.nome() == "EM_CARGA"))
    .map(|corpo| corpo.to_string())
    .collect();
    assert!(
        passaram.is_empty(),
        "alcancaram a tabela em carga: {passaram:?}"
    );
}

/// A copia que GRAVA na tabela reservada pelo `destino`.
#[test]
fn a_copia_para_o_destino_reservado_tambem_para() {
    let dir = dir_temp("destino");
    let s = com_vizinha(&dir);
    pede(&s, 1, RESERVA).unwrap();
    em_carga(
        &s,
        r#""op":"duplicar_tabela","database":"b","tabela":"d","destino":"c""#,
    );
}

/// O comportamento VELHO: sem reserva o `juntar` passa, e quem reservou
/// continua juntando a propria tabela. Sem este par, um portao que
/// recusasse toda junção passaria com louvor nos de cima.
#[test]
fn sem_reserva_e_para_o_dono_o_juntar_continua() {
    let dir = dir_temp("juntar-velho");
    let s = com_vizinha(&dir);
    pede(&s, 2, JUNTA_B).expect("sem reserva nenhuma o juntar passava");
    pede(&s, 1, RESERVA).unwrap();
    pede(&s, 1, JUNTA_B).expect("o dono da reserva deixou de juntar");
    pede(&s, 1, SOLTA).unwrap();
    pede(&s, 2, JUNTA_B).expect("solta a reserva, o outro voltou a juntar");
}
// ------------------------------------------------ o indice adiado (324)
//
// Decisao do dono de 30/09/2026: `"adiar_indice": true` no
// `bulkinsert(true)`, pedido. O que estes testes travam: o indice sai
// CERTO no fim (chave a chave), ninguem le a arvore suspensa -- nem a
// propria ligacao, nem o `verificar` --, a marca vai ao disco antes da
// primeira linha, as recusas da declaracao, a queda da conexao
// reconstruindo, e o comportamento VELHO de quem nao pede.

const RESERVA_ADIADA: &str =
    r#""op":"bulkinsert","database":"b","tabela":"p","ligado":true,"adiar_indice":true"#;
const RESERVA_P: &str = r#""op":"bulkinsert","database":"b","tabela":"p","ligado":true"#;
const SOLTA_P: &str = r#""op":"bulkinsert","database":"b","tabela":"p","ligado":false"#;

/// Tabela `p` sem indice unico: `porId` e `porCidade`, os dois comuns.
fn com_tabela_adiavel(dir: &std::path::Path) -> Arc<Servidor> {
    let s = com_tabela(dir);
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"p",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"cidade","tipo":"Str","tamanho":20}],
                    "indices":[{"nome":"porId","colunas":["id"]},
                               {"nome":"porCidade","colunas":["cidade"]}]}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    s
}

const CIDADES: [&str; 3] = ["Blumenau", "Joinville", "Lages"];

/// `n` linhas em lotes de 100, pela ligacao `ligacao`, em ordem
/// embaralhada (i * 7919 mod n) para a arvore nao sair ordenada de graca.
fn carregar(s: &Arc<Servidor>, ligacao: u64, n: u64) {
    let ids: Vec<u64> = (0..n).map(|i| (i * 7919) % n + 1).collect();
    for lote in ids.chunks(100) {
        let linhas: Vec<String> = lote
            .iter()
            .map(|i| {
                format!(
                    r#"{{"id":{i},"cidade":"{}"}}"#,
                    CIDADES[(*i as usize) % CIDADES.len()]
                )
            })
            .collect();
        pede(
            s,
            ligacao,
            &format!(
                r#""op":"inserir_lote","database":"b","tabela":"p","linhas":[{}]"#,
                linhas.join(",")
            ),
        )
        .unwrap();
    }
}

fn buscar_id(s: &Arc<Servidor>, ligacao: u64, id: u64) -> Result<Json> {
    pede(
        s,
        ligacao,
        &format!(r#""op":"buscar","database":"b","tabela":"p","indice":"porId","chave":[{id}]"#),
    )
}

/// Os bytes 52 e 53 do cabecalho do `.ndx`, lidos do ARQUIVO.
fn marcas_no_disco(dir: &std::path::Path) -> (u8, u8) {
    let bytes = std::fs::read(dir.join("b").join("p.ndx")).unwrap();
    (bytes[52], bytes[53])
}

/// **A prova principal.** A carga adiada termina com o indice CERTO, chave
/// a chave, e ninguem ve a arvore suspensa no meio.
///
/// Defeito reposto (o `t.reindexar()` tirado do `bulkinsert(false)`): a
/// reserva sai com o indice suspenso, e a primeira busca depois de soltar
/// recusa -- `buscar_id(..).unwrap()` cai.
/// Defeito reposto (o `suspender` sem gravar o cabecalho): o byte 53 le 0
/// no disco antes da primeira linha, e o `assert_eq!(marcas, (1, 1))` cai.
#[test]
fn a_carga_adiada_termina_com_o_indice_certo() {
    let dir = dir_temp("adiar-certo");
    let s = com_tabela_adiavel(&dir);
    let r = pede(&s, 1, RESERVA_ADIADA).unwrap();
    assert!(
        matches!(r.campo("indice_adiado"), Some(Json::Bool(true))),
        "{r:?}"
    );
    // R2: a marca no DISCO antes da primeira linha.
    assert_eq!(
        marcas_no_disco(&dir),
        (1, 1),
        "a marca de suspenso nao foi ao disco antes da carga"
    );

    const N: u64 = 600;
    carregar(&s, 1, N);

    // Ninguem le a arvore suspensa: nem a propria ligacao...
    let e = buscar_id(&s, 1, 1).unwrap_err();
    assert_eq!(e.nome(), "EM_CARGA", "{e}");
    assert!(e.to_string().contains("suspenso"), "{e}");
    assert!(
        !e.to_string().contains("reindex"),
        "mandou reparar um indice que a carga reconstroi: {e}"
    );
    // ...nem o `verificar`, que contaria «0 chaves para N registros»...
    let e = pede(&s, 1, r#""op":"verificar","database":"b","tabela":"p""#).unwrap_err();
    assert_eq!(e.nome(), "EM_CARGA", "{e}");
    // ...nem outra ligacao.
    assert_eq!(buscar_id(&s, 2, 1).unwrap_err().nome(), "EM_CARGA");

    let r = pede(&s, 1, SOLTA_P).unwrap();
    assert!(
        matches!(r.campo("indice_reconstruido"), Some(Json::Bool(true))),
        "{r:?}"
    );
    assert_eq!(
        marcas_no_disco(&dir),
        (0, 0),
        "o fecho nao baixou as marcas"
    );

    // Chave a chave, de OUTRA ligacao: exatamente uma linha por id.
    for id in 1..=N {
        let r = buscar_id(&s, 2, id).unwrap_or_else(|e| panic!("id {id}: {e}"));
        let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
        assert_eq!(linhas.len(), 1, "id {id}: {}", r.escrever());
        assert_eq!(linhas[0].inteiro_ou("id", -1), id as i64);
    }
    // O nao unico tambem, pela contagem de cada cidade.
    let mut total = 0;
    for c in CIDADES {
        let r = pede(
            &s,
            2,
            &format!(
                r#""op":"buscar","database":"b","tabela":"p","indice":"porCidade","chave":["{c}"],"max":100000"#
            ),
        )
        .unwrap();
        total += r.campo("linhas").and_then(Json::lista).unwrap().len();
    }
    assert_eq!(total as u64, N);
    pede(&s, 2, r#""op":"verificar","database":"b","tabela":"p""#).unwrap();
}

/// **O comportamento VELHO.** Sem `adiar_indice`, a reserva e a de sempre:
/// o indice acompanha linha a linha, o dono busca no meio da carga, nada
/// vai ao byte 53, e soltar nao reconstroi nada.
#[test]
fn sem_pedir_o_indice_adiado_nada_muda() {
    let dir = dir_temp("adiar-velho");
    let s = com_tabela_adiavel(&dir);
    let r = pede(&s, 1, RESERVA_P).unwrap();
    assert!(matches!(r.campo("indice_adiado"), Some(Json::Bool(false))));
    carregar(&s, 1, 200);
    let r = buscar_id(&s, 1, 37).unwrap();
    assert_eq!(r.campo("linhas").and_then(Json::lista).unwrap().len(), 1);
    assert_eq!(
        marcas_no_disco(&dir).1,
        0,
        "o byte 53 subiu sem ninguem pedir"
    );
    let r = pede(&s, 1, SOLTA_P).unwrap();
    assert!(r.campo("indice_reconstruido").is_none(), "{r:?}");
}

/// As recusas da DECLARACAO -- e a reserva que acabou de nascer sai junto,
/// senao a recusa deixaria a tabela presa para os outros.
#[test]
fn adiar_recusa_na_declaracao() {
    let dir = dir_temp("adiar-recusas");
    let s = com_tabela_adiavel(&dir);

    // R1: indice unico.
    let e = pede(
        &s,
        1,
        r#""op":"bulkinsert","database":"b","tabela":"c","ligado":true,"adiar_indice":true"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("UNICO"), "{e}");
    assert!(
        pede(&s, 2, INSERE).is_ok(),
        "a recusa deixou a reserva presa"
    );

    // Tabela com dados: o regime b', medido perdendo.
    pede(
        &s,
        1,
        r#""op":"inserir","database":"b","tabela":"p","linha":{"id":1}"#,
    )
    .unwrap();
    let e = pede(&s, 1, RESERVA_ADIADA).unwrap_err();
    assert!(e.to_string().contains("0,67"), "{e}");
    assert!(
        buscar_id(&s, 2, 1).is_ok(),
        "a recusa deixou a reserva presa"
    );

    // R3: mae de chave conferida.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"m",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"]}]}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"f",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"m_id","tipo":"Int4"}],
                    "indices":[{"nome":"porM","colunas":["m_id"]}],
                    "chaves_estrangeiras":[{"nome":"fk_m","colunas":["m_id"],
                                            "tabela_ref":"m","colunas_ref":["id"]}]}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    let e = pede(
        &s,
        1,
        r#""op":"bulkinsert","database":"b","tabela":"m","ligado":true,"adiar_indice":true"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("f declara"), "{e}");

    // Soltar nao aceita o pedido: quem decide reconstruir e a marca.
    let e = pede(
        &s,
        1,
        r#""op":"bulkinsert","database":"b","tabela":"p","ligado":false,"adiar_indice":true"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("so para reservar"), "{e}");
}

/// A conexao que cai no meio da carga adiada: o `bulkinsert(false)` nunca
/// vem, e e o fecho da janela da saida que reconstroi.
///
/// Defeito reposto (o braco `indice_suspenso()` tirado do
/// `descarregar_sujas_com`): a reserva sai, o indice fica suspenso, e a
/// busca do outro recusa -- `buscar_id(..).unwrap()` cai.
#[test]
fn a_queda_da_conexao_reconstroi_o_indice_adiado() {
    let dir = dir_temp("adiar-queda");
    let s = com_tabela_adiavel(&dir);
    pede(&s, 1, RESERVA_ADIADA).unwrap();
    carregar(&s, 1, 150);
    s.soltar_cargas_da_ligacao(1);
    for id in [1, 75, 150] {
        let r = buscar_id(&s, 2, id).unwrap_or_else(|e| panic!("id {id}: {e}"));
        assert_eq!(r.campo("linhas").and_then(Json::lista).unwrap().len(), 1);
    }
    assert_eq!(marcas_no_disco(&dir), (0, 0));
}

/// A reserva que VENCE no meio da carga adiada, sem ninguem soltar: a
/// arvore suspensa recusa -- nunca responde vazia -- ate o fecho seguinte
/// a reconstruir.
#[test]
fn a_reserva_vencida_nunca_responde_com_a_arvore_vazia() {
    let dir = dir_temp("adiar-vencida");
    let s = com_tabela_adiavel(&dir);
    pede(&s, 1, RESERVA_ADIADA).unwrap();
    carregar(&s, 1, 50);
    // Vence agora: o proprio `barra` limpa na primeira consulta de outro.
    {
        let mut c = s.cargas.lock().unwrap();
        let todas = c.todas();
        let r = &todas[0];
        c.reservar(
            &r.database,
            &r.tabela,
            "",
            r.ligacao,
            "",
            crate::agora_ms(),
            -1,
        )
        .unwrap();
    }
    match buscar_id(&s, 2, 7) {
        Err(e) => {
            eprintln!("324 vencida: recusou -- {e}");
            assert_eq!(e.nome(), "EM_CARGA", "{e}");
        }
        Ok(r) => assert_eq!(
            r.campo("linhas").and_then(Json::lista).unwrap().len(),
            1,
            "a arvore suspensa respondeu errado: {}",
            r.escrever()
        ),
    }
    s.descarregar_sujas();
    let r = buscar_id(&s, 2, 7).unwrap();
    assert_eq!(r.campo("linhas").and_then(Json::lista).unwrap().len(), 1);
}

/// R4 (pedido 322) NAO e pre-requisito, e isto e a prova: o lado B de uma
/// junção nao passa pelo Portao 4, mas a arvore suspensa recusa sozinha.
/// A pergunta que importa e se ha resposta ERRADA -- zero pares com a
/// tabela carregada --, e nao ha.
#[test]
fn o_lado_b_da_juncao_nao_le_a_arvore_suspensa() {
    let dir = dir_temp("adiar-juntar");
    let s = com_tabela_adiavel(&dir);
    pede(&s, 2, INSERE).unwrap();
    pede(&s, 1, RESERVA_ADIADA).unwrap();
    carregar(&s, 1, 10);
    let r = pede(
        &s,
        2,
        r#""op":"juntar","database":"b","a":{"tabela":"c","chave":"id"},
               "b":{"tabela":"p","chave":"id","indice":"porId"}"#,
    );
    match r {
        Err(e) => {
            eprintln!("324 juntar: recusou -- {e}");
            assert_eq!(e.nome(), "EM_CARGA", "{e}");
        }
        Ok(j) => {
            eprintln!("324 juntar: respondeu -- {}", j.escrever());
            let pares = j
                .campo("linhas")
                .and_then(Json::lista)
                .map_or(0, |l| l.len());
            assert_eq!(pares, 1, "junção leu a arvore suspensa: {}", j.escrever());
        }
    }
}
