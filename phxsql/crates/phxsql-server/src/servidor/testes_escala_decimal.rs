//! **O pedido 392: a escala de um `Decimal` nunca le o valor do outro.**
//!
//! # O defeito, e por que ele e o pior que um banco pode ter
//!
//! `Value::Decimal` guarda o inteiro ESCALADO -- 10,50 na escala 2 e `1050`,
//! e 10,5000 na escala 4 e `105000`. Quem da sentido a ele e a escala do
//! TIPO, e toda vez que um caminho junta valores de DUAS origens ele tem de
//! ler cada um pelo tipo de onde ele veio. Onde isso falhou, o numero saiu
//! **cem vezes** maior ou menor, sem erro, sem aviso e sem `truncado`: valor
//! monetario errado com cara de certo.
//!
//! # Os cinco caminhos, e o que cada um fazia (medido em 23/09/2026)
//!
//! | caminho | antes | agora |
//! |---|---|---|
//! | `unir` (`tabelas` e `partes`) | `10,5000` saia `1050,00` | promove a maior escala |
//! | `juntar` (op) | ja casava por valor | igual |
//! | `diferencas` chave | chave de `b` lida pela escala de `a`: `1050,00` | cada lado pela sua |
//! | `diferencas` linha | 7,25 e 0,0725 eram `iguais` | `diferentes` |
//! | `pivotar` junção | 7,25 casava 0,0725; 10,50 nao casava 10,5000 | casa por valor |
//! | `consultar` junção/`em` | zero linhas onde o SQL casa uma | casa |
//!
//! # A regua, e ela nao foi escolha desta casa
//!
//! Comparar `Decimal` e pelo VALOR, e os quatro motores convergem -- 4 de 4,
//! aceite automatico: PostgreSQL («numeric values are physically stored
//! without any extra leading or trailing zeroes»), MySQL e MariaDB (o
//! `decimal_cmp` de `strings/decimal.c` corta os zeros a direita antes de
//! comparar digito a digito) e SQLite («numeric values are always compared
//! numerically»). Para o TIPO da união, 3 de 3 maduros levam em conta todos
//! os bracos.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
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
    let s = Servidor::novo(c).unwrap();
    s.executar(
        "criar_database",
        &pedido(r#"{"database":"b"}"#),
        &Sessao::default(),
    )
    .unwrap();
    s
}

/// Pelo `despachar`: e por onde o pedido entra de verdade, e e ali que a
/// resposta vira JSON -- que e onde a escala errada aparecia.
fn pede(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// Uma tabela `id Int4` + `valor Decimal(12,escala)`, com indice na
/// `valor` para a comparação por CHAVE decimal ter por onde acontecer.
fn tabela(s: &Arc<Servidor>, nome: &str, escala: u8, chave_unica: bool) {
    pede(
        s,
        &format!(
            r#""op":"criar_tabela","database":"b","tabela":"{nome}",
                   "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                              {{"nome":"valor","tipo":"Decimal(12,{escala})"}}],
                   "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}},
                              {{"nome":"porValor","colunas":["valor"],"unico":{chave_unica}}}]"#
        ),
    )
    .unwrap_or_else(|e| panic!("criar {nome}: {e}"));
}

fn inserir(s: &Arc<Servidor>, nome: &str, id: i64, valor: &str) {
    pede(
        s,
        &format!(
            r#""op":"inserir","database":"b","tabela":"{nome}",
                   "linha":{{"id":{id},"valor":"{valor}"}}"#
        ),
    )
    .unwrap_or_else(|e| panic!("inserir em {nome}: {e}"));
}

/// Os valores de uma coluna, na ordem em que a resposta os traz.
fn coluna_da_uniao(r: &Json, quantas: usize) -> Vec<String> {
    let linhas = r.campo("linhas").and_then(Json::lista).expect("sem linhas");
    assert_eq!(linhas.len(), quantas, "{}", r.escrever());
    linhas
        .iter()
        .map(|l| {
            l.lista()
                .and_then(|v| v.get(1))
                .and_then(Json::texto)
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

/// **O 392 pelo PROTOCOLO, nos dois sentidos e nos dois caminhos.**
///
/// A unidade do `juncao::unir` ja trava o empilhamento; este trava a
/// CORRENTE inteira -- `op_unir` -> `juncao::unir` -> `valor_para_json`
/// --, que e onde o numero chegava ao cliente cem vezes maior.
///
/// **Prova real:** tire o laco do `converter_para` do `juncao::unir` e a
/// primeira asserção volta a ver `1050.00`.
#[test]
fn a_uniao_nao_corrompe_o_decimal_do_outro_braco() {
    let d = DirTemp::novo("escala-uniao");
    let s = servidor(&d);
    tabela(&s, "d2", 2, false);
    tabela(&s, "d4", 4, false);
    inserir(&s, "d2", 1, "10.50");
    inserir(&s, "d4", 2, "10.5000");

    // O MESMO dinheiro nos dois bracos, em escalas diferentes: sai na
    // maior escala, e nenhum dos dois muda de valor.
    let r = pede(
        &s,
        r#""op":"unir","database":"b","modo":"tudo","tabelas":["d2","d4"]"#,
    )
    .unwrap();
    assert_eq!(
        coluna_da_uniao(&r, 2),
        vec!["10.5000".to_string(), "10.5000".to_string()],
        "{}",
        r.escrever()
    );

    // E na ordem contraria o resultado e o mesmo: a escala do resultado
    // nao pode depender de quem foi pedido primeiro.
    let r = pede(
        &s,
        r#""op":"unir","database":"b","modo":"tudo","tabelas":["d4","d2"]"#,
    )
    .unwrap();
    assert_eq!(
        coluna_da_uniao(&r, 2),
        vec!["10.5000".to_string(), "10.5000".to_string()],
        "{}",
        r.escrever()
    );

    // O braco-PEDIDO (o caminho `partes`) passa pela mesma função e
    // por isso pelo mesmo conserto -- mas e outro caminho de entrada, e
    // caminho irmao se prova, nao se supoe.
    let r = pede(
        &s,
        r#""op":"unir","database":"b","modo":"tudo",
               "partes":[{"op":"varrer","tabela":"d2"},
                         {"op":"varrer","tabela":"d4"}]"#,
    )
    .unwrap();
    assert_eq!(
        coluna_da_uniao(&r, 2),
        vec!["10.5000".to_string(), "10.5000".to_string()],
        "{}",
        r.escrever()
    );

    // E o `UNION` distinto ve as duas como a MESMA linha, que e o
    // corolario: valor igual, linha repetida.
    let r = pede(
        &s,
        r#""op":"unir","database":"b","modo":"distinta",
               "partes":[{"op":"agrupar","tabela":"d2","por":["valor"]},
                         {"op":"agrupar","tabela":"d4","por":["valor"]}]"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("repetidas").and_then(Json::numero),
        Some(1.0),
        "10,50 e 10,5000 nao contaram como a mesma linha: {}",
        r.escrever()
    );
}

/// **`diferencas` compara DINHEIRO, nao o inteiro guardado.**
///
/// Os dois sentidos numa medição so, e eles sao opostos:
///
/// - `id 1`: 10,50 (escala 2) e 10,5000 (escala 4) sao o MESMO dinheiro e
///   saiam como `diferentes`;
/// - `id 2`: 7,25 (escala 2) e 0,0725 (escala 4) guardam o MESMO inteiro
///   (725) e saiam como **`iguais`** -- a operação que existe para ser
///   acreditada dizendo que duas linhas batem quando uma vale cem vezes a
///   outra.
///
/// **Prova real:** troque o `mesma_celula` do `diferencas::comparar` por
/// `linha.get(*i) != outra.get(*i)` e `iguais` volta a 1 com o par
/// errado.
#[test]
fn diferencas_compara_o_valor_e_nao_o_inteiro_escalado() {
    let d = DirTemp::novo("escala-dif");
    let s = servidor(&d);
    tabela(&s, "a2", 2, true);
    tabela(&s, "b4", 4, true);
    inserir(&s, "a2", 1, "10.50");
    inserir(&s, "b4", 1, "10.5000");
    inserir(&s, "a2", 2, "7.25");
    inserir(&s, "b4", 2, "0.0725");

    let r = pede(
        &s,
        r#""op":"diferencas","database":"b","a":"a2","b":"b4","indice":"porId""#,
    )
    .unwrap();
    let texto = r.escrever();
    assert_eq!(
        r.campo("iguais").and_then(Json::numero),
        Some(1.0),
        "o mesmo dinheiro em escalas diferentes nao contou como igual: {texto}"
    );
    let dif = r
        .campo("diferentes")
        .and_then(Json::lista)
        .expect("sem diferentes");
    assert_eq!(dif.len(), 1, "{texto}");
    assert_eq!(
        dif[0]
            .campo("chave")
            .and_then(Json::lista)
            .and_then(|l| l.first())
            .and_then(Json::numero),
        Some(2.0),
        "a linha diferente e a do id 2 (7,25 contra 0,0725): {texto}"
    );
}

/// **E a CHAVE de cada lado sai pela escala DELE.**
///
/// E o 392 literal dentro do `diferencas`: a chave 10,5000 de uma
/// `Decimal(12,4)` era publicada com a escala 2 da outra tabela e virava
/// `1050.00` -- cem vezes, na lista que diz o que falta de cada lado.
///
/// **Prova real:** volte o `chave_json` a usar `tipos_a` nos dois lados e
/// a asserção do `so_em_b` ve `1050.00`.
#[test]
fn a_chave_de_cada_lado_sai_pela_escala_dele() {
    let d = DirTemp::novo("escala-chave");
    let s = servidor(&d);
    tabela(&s, "a2", 2, true);
    tabela(&s, "b4", 4, true);
    // Uma linha so de cada lado, com dinheiros DIFERENTES: cada chave tem
    // de sair como ela e.
    inserir(&s, "a2", 1, "7.25");
    inserir(&s, "b4", 1, "0.0725");

    let r = pede(
        &s,
        r#""op":"diferencas","database":"b","a":"a2","b":"b4","indice":"porValor""#,
    )
    .unwrap();
    let texto = r.escrever();
    let so = |campo: &str| -> String {
        r.campo(campo)
            .and_then(Json::lista)
            .and_then(|l| l.first())
            .and_then(Json::lista)
            .and_then(|l| l.first())
            .and_then(Json::texto)
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(so("so_em_a"), "7.25", "{texto}");
    assert_eq!(
        so("so_em_b"),
        "0.0725",
        "a chave de b saiu pela escala de a: {texto}"
    );
}

/// **O `pivotar` casa a tabela de consulta por VALOR**, e nos dois tipos
/// em que as duas chaves divergiam.
///
/// O mapa era montado com `rotulo(v, 0)` e procurado com `rotulo_cru` --
/// duas formas de escrever a mesma chave. No `Decimal` as duas escreviam
/// o inteiro guardado (7,25 casava com 0,0725, 10,50 nao casava com
/// 10,5000); na `Date` elas nem coincidiam (`2026-09-23` de um lado, o
/// numero de dias do outro), e a junção por data NUNCA casava.
///
/// **Prova real:** volte qualquer um dos dois lados a forma antiga e o
/// rotulo da linha vira `(vazio)`.
#[test]
fn o_pivot_casa_a_consulta_por_valor_e_nao_por_representacao() {
    let d = DirTemp::novo("escala-pivot");
    let s = servidor(&d);
    tabela(&s, "f2", 2, false);
    tabela(&s, "l4", 4, true);
    inserir(&s, "f2", 1, "10.50");
    inserir(&s, "f2", 2, "7.25");
    inserir(&s, "l4", 1, "10.5000");
    inserir(&s, "l4", 2, "0.0725");

    let r = pede(
        &s,
        r#""op":"pivotar","database":"b","tabela":"f2","agregador":"soma","valor":"valor",
               "linhas":[{"campo":"L.id"}],"colunas":[{"campo":"id"}],
               "juntar":[{"tabela":"l4","coluna":"valor","chave":"valor","prefixo":"L"}]"#,
    )
    .unwrap();
    let texto = r.escrever();
    let rotulos: Vec<String> = r
        .campo("rotulos_linha")
        .and_then(Json::lista)
        .expect("sem rotulos")
        .iter()
        .filter_map(Json::texto)
        .map(str::to_string)
        .collect();
    // O fato de 10,50 casa com a linha de consulta de 10,5000 (id 1); o
    // de 7,25 NAO casa com a de 0,0725 e fica sem rotulo.
    assert!(
        rotulos.contains(&"1".to_string()),
        "10,50 nao casou com 10,5000: {texto}"
    );
    assert!(
        !rotulos.contains(&"2".to_string()),
        "7,25 casou com 0,0725 -- o mesmo inteiro guardado, dinheiro diferente: {texto}"
    );

    // E a junção por DATA, que nunca casava.
    for t in ["fd", "ld"] {
        pede(
            &s,
            &format!(
                r#""op":"criar_tabela","database":"b","tabela":"{t}",
                       "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                                  {{"nome":"dia","tipo":"Date"}}],
                       "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                                    "primario":true}}]"#
            ),
        )
        .unwrap();
    }
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"fd","linha":{"id":1,"dia":"2026-09-23"}"#,
    )
    .unwrap();
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"ld","linha":{"id":7,"dia":"2026-09-23"}"#,
    )
    .unwrap();
    let r = pede(
        &s,
        r#""op":"pivotar","database":"b","tabela":"fd","agregador":"contagem",
               "linhas":[{"campo":"L.id"}],"colunas":[{"campo":"id"}],
               "juntar":[{"tabela":"ld","coluna":"dia","chave":"dia","prefixo":"L"}]"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("rotulos_linha")
            .and_then(Json::lista)
            .and_then(|l| l.first())
            .and_then(Json::texto),
        Some("7"),
        "a junção por data nao casou: {}",
        r.escrever()
    );
}

/// **A junção e o `IN` da composição casam decimais de escalas
/// diferentes.**
///
/// Na composição o `Decimal` viaja como TEXTO (`"10.50"`), para nao
/// passar por `f64` -- e o texto carrega a escala. `"10.50"` contra
/// `"10.5000"` sao chaves diferentes e o mesmo dinheiro: devolvia zero
/// linhas, que e o pior resultado possivel porque parece resposta.
///
/// **Prova real:** devolva `vec![false; pares.len()]` no lugar do
/// `pares_decimais` e as duas metades voltam a zero linhas.
#[test]
fn a_composicao_casa_decimais_de_escalas_diferentes() {
    let d = DirTemp::novo("escala-comp");
    let s = servidor(&d);
    tabela(&s, "d2", 2, false);
    tabela(&s, "d4", 4, false);
    inserir(&s, "d2", 1, "10.50");
    inserir(&s, "d4", 2, "10.5000");

    let r = pede(
        &s,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"d2"},"apelido":"a",
               "juntar":[{"tipo":"interno","de":{"op":"varrer","tabela":"d4"},"apelido":"c",
                          "em":[{"esquerda":"a.valor","direita":"c.valor"}]}]"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("devolvidas").and_then(Json::numero),
        Some(1.0),
        "a junção nao casou 10,50 com 10,5000: {}",
        r.escrever()
    );

    let r = pede(
        &s,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"d2"},
               "em":[{"coluna":"valor","de":{"op":"varrer","tabela":"d4"},"campo":"valor"}]"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("devolvidas").and_then(Json::numero),
        Some(1.0),
        "o IN nao casou 10,50 com 10,5000: {}",
        r.escrever()
    );

    // E o `existe`, que casa pelo mesmo espalhamento.
    let r = pede(
        &s,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"d2"},"apelido":"a",
               "existe":[{"de":{"op":"varrer","tabela":"d4"},"apelido":"c",
                          "em":[{"esquerda":"a.valor","direita":"c.valor"}]}]"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("devolvidas").and_then(Json::numero),
        Some(1.0),
        "o EXISTS nao casou 10,50 com 10,5000: {}",
        r.escrever()
    );
}

/// **O COMPORTAMENTO VELHO: `Decimal` continua nao casando com inteiro.**
///
/// A canonização entra so quando os DOIS lados sao `Decimal`. O
/// cabecalho do `consultar` registra, desde o pedido 237, que `"10.00"`
/// nao casa com o inteiro `10` -- «sao colunas de tipos diferentes» --, e
/// o conserto da escala nao pode mudar isso de carona: seria trocar um
/// contrato documentado sem ninguem ter pedido, e quem monta a junção
/// erraria de outro jeito.
///
/// Quem quiser o contrario tem o `juntar` de tabela (`juncao.rs`), que
/// casa por familia -- e essa divergencia entre os dois motores de
/// junção fica REGISTRADA aqui em vez de ser consertada de lado.
#[test]
fn decimal_continua_nao_casando_com_inteiro_na_composicao() {
    let d = DirTemp::novo("escala-velho");
    let s = servidor(&d);
    tabela(&s, "d2", 2, false);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"i8",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"valor","tipo":"Int8"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    inserir(&s, "d2", 1, "10.00");
    pede(
        &s,
        r#""op":"inserir","database":"b","tabela":"i8","linha":{"id":1,"valor":10}"#,
    )
    .unwrap();

    let r = pede(
        &s,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"d2"},"apelido":"a",
               "juntar":[{"tipo":"interno","de":{"op":"varrer","tabela":"i8"},"apelido":"c",
                          "em":[{"esquerda":"a.valor","direita":"c.valor"}]}]"#,
    )
    .unwrap();
    assert_eq!(
        r.campo("devolvidas").and_then(Json::numero),
        Some(0.0),
        "o conserto da escala mudou o contrato do Decimal contra inteiro: {}",
        r.escrever()
    );
}

/// **E nada passou a RECUSAR.** Guarda nova entra pedida, nao imposta --
/// e aqui nem guarda nova ha: o conserto corrige resposta, nunca recusa
/// pedido que antes respondia. As tres operações com tabelas de escalas
/// diferentes continuam respondendo `ok`.
#[test]
fn nenhuma_operacao_passou_a_recusar_por_escala_diferente() {
    let d = DirTemp::novo("escala-velho2");
    let s = servidor(&d);
    tabela(&s, "d2", 2, true);
    tabela(&s, "d4", 4, true);
    inserir(&s, "d2", 1, "1.11");
    inserir(&s, "d4", 1, "2.2222");
    for pedido in [
        r#""op":"unir","database":"b","modo":"tudo","tabelas":["d2","d4"]"#,
        r#""op":"juntar","database":"b","a":{"tabela":"d2","chave":"valor","prefixo":"A"},
               "b":{"tabela":"d4","chave":"valor","prefixo":"B"}"#,
        r#""op":"diferencas","database":"b","a":"d2","b":"d4","indice":"porId""#,
        r#""op":"pivotar","database":"b","tabela":"d2","agregador":"soma","valor":"valor",
               "linhas":[{"campo":"L.id"}],"colunas":[{"campo":"id"}],
               "juntar":[{"tabela":"d4","coluna":"valor","chave":"valor","prefixo":"L"}]"#,
    ] {
        pede(&s, pedido).unwrap_or_else(|e| panic!("passou a recusar: {e} -- em {pedido}"));
    }
}
