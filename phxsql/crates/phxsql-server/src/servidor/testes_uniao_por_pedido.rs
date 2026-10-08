//! **O braço da união é um PEDIDO, e não um nome de tabela** (pedido 393).
//!
//! O que estes testes travam, em ordem de importância:
//!
//! 1. o braço FILTRA dentro de si, e só o recorte empilha -- é o substrato
//!    que `SELECT nome FROM a WHERE uf='SC' UNION SELECT nome FROM b` não
//!    tinha, e é de onde saem os 64,2x remedidos no pedido 393;
//! 2. **quem manda `tabelas` continua recebendo o mesmo**, byte a byte: é o
//!    teste do comportamento VELHO, que vale mais que o do novo;
//! 3. o `distinta` continua desduplicando -- as colunas do motor (`rowid`, as
//!    de sistema) NÃO entram na chave, senão o `UNION` devolveria o mesmo que
//!    o `UNION ALL` anunciando `repetidas: 0`;
//! 4. a trava global não é tomada no caminho novo, e por isso ele funciona:
//!    `travar_dados` erra na reentrância, e um braço sob a trava receberia
//!    `trava_reentrante()`.
use super::*;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("un-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `g1` e `g2`, mesma forma, com um índice por `uf` para o braço `buscar`
/// ter o que usar -- é o índice que compra o número do pedido 393.
fn servidor(d: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: d.to_path_buf(),
        log_acessos: d.join("acessos.log"),
        blacklist: d.join("blacklist.json"),
        dblink: d.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    for tab in ["g1", "g2"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{tab}","colunas":[
                        {{"nome":"id","tipo":"Int4","obrigatoria":true}},
                        {{"nome":"nome","tipo":"Str(20)"}},
                        {{"nome":"uf","tipo":"Str(2)"}}],
                     "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                  "primario":true}},
                                {{"nome":"porUf","colunas":["uf"]}}]}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let grava = |tab: &str, linhas: &[(i64, &str, &str)]| {
        for (id, nome, uf) in linhas {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{tab}","linha":
                            {{"id":{id},"nome":"{nome}","uf":"{uf}"}}}}"#
                )),
                &Sessao::default(),
            )
            .unwrap();
        }
    };
    grava(
        "g1",
        &[(1, "ana", "SC"), (2, "bia", "SP"), (3, "caio", "RJ")],
    );
    grava("g2", &[(4, "duda", "SC"), (5, "elo", "SP")]);
    s
}

fn unir(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    s.executar(
        "unir",
        &pedido(&format!(r#"{{"database":"b",{corpo}}}"#)),
        &Sessao::default(),
    )
}

fn nomes(r: &Json) -> Vec<String> {
    r.campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|l| {
            l.lista()
                .and_then(|v| v.get(1))
                .and_then(Json::texto)
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

fn colunas(r: &Json) -> Vec<String> {
    r.campo("colunas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|c| c.texto_ou("nome", "").to_string())
        .collect()
}

/// **O ACHADO: o braço filtra, e só o recorte empilha.**
///
/// As duas tabelas têm 5 linhas ao todo e 2 são de SC. Pelo caminho
/// `tabelas` não há onde filtrar: a união devolve as 5 e quem perguntou
/// filtra fora. Pelo caminho `partes`, cada braço traz o que é dele -- e o
/// segundo braço traz pelo ÍNDICE, que é o que compra os 64,2x do pedido
/// 393 num volume de verdade.
///
/// **Prova real, com o defeito reposto:** troque o `partes` por
/// `"tabelas":["g1","g2"]` e a resposta vira 5 linhas -- as três de fora
/// de SC voltam.
#[test]
fn o_braco_pedido_filtra_dentro_e_so_o_recorte_empilha() {
    let d = dir("filtra");
    let s = servidor(&d);

    let r = unir(
        &s,
        r#""modo":"tudo","partes":[
                 {"op":"varrer","tabela":"g1",
                  "onde":[{"coluna":"uf","op":"=","valor":"SC"}]},
                 {"op":"buscar","tabela":"g2","indice":"porUf","chave":["SC"]}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("quantas", -1), 2, "{}", r.escrever());
    assert_eq!(nomes(&r), vec!["ana", "duda"], "{}", r.escrever());

    // E a resposta continua dizendo QUAIS tabelas leu -- agora derivado
    // dos braços, porque o pedido não traz mais uma lista de nomes.
    assert_eq!(
        r.campo("tabelas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|t| t.texto().unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        vec!["g1", "g2"],
        "{}",
        r.escrever()
    );

    // O mesmo pedido pelo caminho velho traz as cinco: é a diferença que
    // o número do pedido 393 mede.
    let velho = unir(&s, r#""modo":"tudo","tabelas":["g1","g2"]"#).unwrap();
    assert_eq!(velho.inteiro_ou("quantas", -1), 5, "{}", velho.escrever());
}

/// **O comportamento VELHO, que é o teste que mais importa numa guarda
/// nova.** Quem manda `tabelas` recebe o que sempre recebeu: as colunas
/// visíveis (sem as do motor), as cinco linhas e a contagem por parte.
#[test]
fn quem_manda_tabelas_continua_recebendo_o_mesmo() {
    let d = dir("velho");
    let s = servidor(&d);
    let r = unir(&s, r#""modo":"tudo","tabelas":["g1","g2"]"#).unwrap();

    assert_eq!(colunas(&r), vec!["id", "nome", "uf"], "{}", r.escrever());
    assert_eq!(r.inteiro_ou("quantas", -1), 5);
    assert_eq!(r.texto_ou("sql", ""), "UNION ALL");
    assert_eq!(
        r.campo("por_parte")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|n| n.inteiro().unwrap_or(-1))
            .collect::<Vec<_>>(),
        vec![3, 2],
        "{}",
        r.escrever()
    );
    // O eco da lista pedida continua igual.
    assert_eq!(
        r.campo("tabelas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|t| t.texto().unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        vec!["g1", "g2"]
    );
}

/// **O `distinta` do braço-pedido enxerga a linha repetida.**
///
/// O `varrer` põe o `rowid` na frente de cada linha e o esquema põe as de
/// sistema no fim. Todas são únicas por linha: se entrassem na chave, duas
/// linhas visivelmente iguais em tabelas diferentes nunca contariam como
/// repetidas -- o `UNION` devolveria o mesmo que o `UNION ALL` anunciando
/// `repetidas: 0`. É o defeito que o `rownum` causou até 22/09/2026, e o
/// braço-pedido é a porta nova por onde ele voltaria.
///
/// **Prova real, com o defeito reposto:** tire o `rowid` do
/// `coluna_do_motor` e este teste devolve 4 linhas e `repetidas: 0`.
#[test]
fn o_distinta_do_braco_pedido_ve_a_linha_repetida() {
    let d = dir("distinta");
    let s = servidor(&d);
    // A mesma linha visível nas duas tabelas, com rowid diferente: em g1
    // ela é a quarta linha, em g2 a terceira.
    for tab in ["g1", "g2"] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{tab}","linha":
                        {{"id":99,"nome":"igual","uf":"MG"}}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    }
    let r = unir(
        &s,
        r#""modo":"distinta","partes":[
                 {"op":"varrer","tabela":"g1",
                  "onde":[{"coluna":"uf","op":"=","valor":"MG"}]},
                 {"op":"varrer","tabela":"g2",
                  "onde":[{"coluna":"uf","op":"=","valor":"MG"}]}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("quantas", -1), 1, "{}", r.escrever());
    assert_eq!(r.inteiro_ou("repetidas", -1), 1, "{}", r.escrever());
    // E o cabeçalho é o das colunas que quem pergunta VÊ.
    assert_eq!(colunas(&r), vec!["id", "nome", "uf"], "{}", r.escrever());
}

/// A trava global NÃO é tomada no caminho novo -- e não é preferência: um
/// braço chamado sob a trava receberia `trava_reentrante()` da
/// `COM_A_TRAVA`. E ela também não fica presa: a operação seguinte, que a
/// toma, entra.
///
/// **Prova real, com o defeito reposto:** ponha
/// `let _t = self.travar_dados()?;` no topo do `unir_por_pedidos` e este
/// teste falha com «trava reentrante».
#[test]
fn o_caminho_novo_nao_segura_a_trava_global() {
    let d = dir("trava");
    let s = servidor(&d);
    let r = unir(
        &s,
        r#""modo":"tudo","partes":[{"op":"varrer","tabela":"g1"},
                                       {"op":"varrer","tabela":"g2"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("quantas", -1), 5, "{}", r.escrever());
    // A trava foi solta: quem a toma depois entra.
    s.executar(
        "varrer",
        &pedido(r#"{"database":"b","tabela":"g1","max":1}"#),
        &Sessao::default(),
    )
    .unwrap();
}

/// Os dois campos no mesmo pedido recusam NOMEANDO os dois, em vez de o
/// motor escolher calado -- campo que parece pedido e o motor ignora é a
/// família da configuração que ninguém lê.
#[test]
fn partes_e_tabelas_no_mesmo_pedido_recusa_nomeando() {
    let d = dir("ambos");
    let s = servidor(&d);
    let e = unir(
        &s,
        r#""tabelas":["g1","g2"],"partes":[{"op":"varrer","tabela":"g1"},
                                               {"op":"varrer","tabela":"g2"}]"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("partes") && e.contains("tabelas"), "{e}");
}

/// Um braço só não é união, e a recusa diz quantos vieram.
#[test]
fn um_braco_so_nao_e_uniao() {
    let d = dir("um");
    let s = servidor(&d);
    let e = unir(&s, r#""partes":[{"op":"varrer","tabela":"g1"}]"#)
        .unwrap_err()
        .to_string();
    assert!(e.contains("dois bracos"), "{e}");
}

/// **Operação que não devolve linhas nasce RECUSADA no braço**, pelo mesmo
/// portão do `consultar`: a lista é curta de propósito, e uma op de
/// escrita aqui dentro faria uma união gravar.
#[test]
fn braco_que_nao_devolve_linhas_recusa_nomeando_o_que_serve() {
    let d = dir("escrita");
    let s = servidor(&d);
    let e = unir(
        &s,
        r#""partes":[{"op":"inserir","tabela":"g1","linha":{"id":9}},
                         {"op":"varrer","tabela":"g2"}]"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("braco 1 da uniao"), "{e}");
    assert!(e.contains("varrer") && e.contains("buscar"), "{e}");
}

/// Braços de larguras diferentes recusam com a CONTA das colunas -- a
/// mesma conferência do caminho velho, agora sobre a projeção do braço.
#[test]
fn bracos_de_larguras_diferentes_recusam_com_a_conta() {
    let d = dir("largura");
    let s = servidor(&d);
    let e = unir(
        &s,
        r#""partes":[
                 {"op":"consultar","de":{"op":"varrer","tabela":"g1"},
                  "colunas":[{"coluna":"nome"}]},
                 {"op":"varrer","tabela":"g2"}]"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("coluna(s)"), "{e}");
}

/// O braço `consultar` com projeção empilha com o `varrer` de mesma
/// largura -- e é por aqui que o `SELECT` com colunas escolhidas vai
/// chegar quando o tradutor for escrito.
#[test]
fn o_braco_consultar_com_projecao_empilha() {
    let d = dir("projecao");
    let s = servidor(&d);
    let r = unir(
        &s,
        r#""modo":"tudo","partes":[
                 {"op":"consultar","de":{"op":"varrer","tabela":"g1"},
                  "colunas":[{"coluna":"nome"}],"expressao":"uf = 'SC'"},
                 {"op":"consultar","de":{"op":"varrer","tabela":"g2"},
                  "colunas":[{"coluna":"nome"}],"expressao":"uf = 'SC'"}]"#,
    )
    .unwrap();
    assert_eq!(colunas(&r), vec!["nome"], "{}", r.escrever());
    assert_eq!(r.inteiro_ou("quantas", -1), 2, "{}", r.escrever());
}
