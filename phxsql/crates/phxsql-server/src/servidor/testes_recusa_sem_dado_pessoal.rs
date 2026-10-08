//! Pedido 464: a recusa de conversao nao cita o valor de coluna marcada como
//! dado pessoal -- por NENHUM dos caminhos que convertem valor de coluna.
//!
//! O `citar` do 453 corta o valor LONGO; o curto (um CPF, um e-mail) cabia no
//! teto e saia inteiro na recusa, que volta ao cliente e vai ao
//! `acessos.log`. Cada teste daqui e um caminho que chega a um conversor
//! diferente, para que a guarda de cada porta caia sozinha: o protocolo
//! (`json_para_linha`, o filtro e o SQL, que viaja pelo mesmo), o
//! `atualizar` do upsert (`mesclar`), a carga colada (`valor_de_texto`) e a
//! faixa do tipo, que so se confere no slot (`escrever_inline`).
use super::*;

/// Curto de proposito: 14 bytes, bem abaixo do `TETO_DA_CITACAO` (48) --
/// e o caso que o teto do 453 nao alcanca.
const CPF: &str = "999.888.777-66";
/// Os digitos do CPF num `Int4`: converte para `Int` e so estoura a faixa
/// no slot.
const DOC: &str = "99988877766";

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `b.p` com `nasc` e `doc` marcadas, e `quando`/`qtd` sem marca, do
/// mesmo tipo das marcadas -- o irmao que prova o comportamento velho.
fn servidor(dir: &Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"p",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                               {"nome":"nasc","tipo":"Date"},
                               {"nome":"doc","tipo":"Int4"},
                               {"nome":"quando","tipo":"Date"},
                               {"nome":"qtd","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"p","colunas":{"nasc":"pessoal","doc":"sensivel"}}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"p","linha":{"id":1}}"#),
        &dono,
    )
    .unwrap();
    s
}

/// A resposta inteira, recusa ou nao: o `inserir_lote` sem
/// `parar_no_erro` devolve `ok` com a recusa DENTRO.
fn resposta(s: &Arc<Servidor>, op: &str, corpo: &str) -> String {
    match s.executar(op, &pedido(corpo), &Sessao::default()) {
        Ok(j) => j.escrever(),
        Err(e) => e.to_string(),
    }
}

/// A recusa nomeia a coluna e nao cita o valor.
fn redigida(quem: &str, r: &str, valor: &str, coluna: &str) {
    assert!(
        !r.contains(valor),
        "{quem}: a recusa citou o valor da coluna marcada: {r}"
    );
    assert!(
        r.contains(coluna),
        "{quem}: a recusa nao nomeia a coluna -- o teste passaria por \
             qualquer erro que nao fosse o da conversao: {r}"
    );
}

#[test]
fn a_recusa_do_protocolo_nao_cita_dado_pessoal() {
    let dir = DirTemp::novo("recusa-464-protocolo");
    let s = servidor(&dir);
    for (quem, op, corpo) in [
        (
            "inserir",
            "inserir",
            format!(r#"{{"database":"b","tabela":"p","linha":{{"id":2,"nasc":"{CPF}"}}}}"#),
        ),
        (
            "inserir por lista",
            "inserir",
            format!(r#"{{"database":"b","tabela":"p","linha":[2,"{CPF}",null,null,null]}}"#),
        ),
        (
            "atualizar",
            "atualizar",
            format!(
                r#"{{"database":"b","tabela":"p","rowid":1,"valores":{{"id":1,"nasc":"{CPF}"}}}}"#
            ),
        ),
        (
            "inserir_lote",
            "inserir_lote",
            format!(
                r#"{{"database":"b","tabela":"p","parar_no_erro":false,
                        "linhas":[{{"id":3,"nasc":"{CPF}"}}]}}"#
            ),
        ),
        (
            "filtro do varrer",
            "varrer",
            format!(
                r#"{{"database":"b","tabela":"p","onde":[{{"coluna":"nasc","valor":"{CPF}"}}]}}"#
            ),
        ),
        (
            "sql",
            "sql",
            format!(r#"{{"database":"b","texto":"INSERT INTO p (id, nasc) VALUES (4, '{CPF}')"}}"#),
        ),
    ] {
        let r = resposta(&s, op, &corpo);
        redigida(quem, &r, CPF, "nasc");
    }
}

#[test]
fn a_recusa_do_upsert_nao_cita_dado_pessoal() {
    let dir = DirTemp::novo("recusa-464-upsert");
    let s = servidor(&dir);
    let r = resposta(
        &s,
        "inserir",
        &format!(
            r#"{{"database":"b","tabela":"p","linha":{{"id":1}},
                    "se_existir":"atualizar","atualizar":{{"nasc":"{CPF}"}}}}"#
        ),
    );
    redigida("upsert", &r, CPF, "nasc");
}

#[test]
fn a_recusa_da_carga_colada_nao_cita_dado_pessoal() {
    let dir = DirTemp::novo("recusa-464-carga");
    let s = servidor(&dir);
    let r = resposta(
        &s,
        "inserir_lote",
        &format!(
            r#"{{"database":"b","tabela":"p","formato":"csv","parar_no_erro":false,
                    "texto":"id;nasc\n5;{CPF}\n"}}"#
        ),
    );
    redigida("carga colada", &r, CPF, "nasc");
}

#[test]
fn a_recusa_da_faixa_no_slot_nao_cita_dado_pessoal() {
    let dir = DirTemp::novo("recusa-464-faixa");
    let s = servidor(&dir);
    let r = resposta(
        &s,
        "inserir",
        &format!(r#"{{"database":"b","tabela":"p","linha":{{"id":6,"doc":"{DOC}"}}}}"#),
    );
    redigida("faixa do Int4", &r, DOC, "doc");
}

/// **Pedido 558: a conta que PARTE de coluna marcada nao cita o valor,
/// mesmo caindo numa coluna sem marca.**
///
/// A porta do 464 olha a coluna de DESTINO, e aqui o destino e sem marca:
/// `faixa` (`Int1 = renda / 1000`), `dia` (`Date = cod`) e `neg`
/// (`Int8 = -ativo`) nao sao dado pessoal, e o valor que a conta carrega
/// e. Quem redige e o motor da expressao, que e quem conhece o valor -- a
/// `descricao` do numero e do booleano, e a recusa do conversor de data
/// dentro do `coagir`.
///
/// **Defeito reposto** (a `descricao` voltando a escrever `o numero {n}`
/// e `o booleano {b}`, e o `coagir` com o `valor_de_texto(t, ty)?` cru):
/// cai cada caminho, com `o numero 500`, o CPF e `o booleano true`.
#[test]
fn a_conta_que_parte_de_coluna_marcada_nao_cita_o_valor() {
    let dir = DirTemp::novo("recusa-558-conta");
    let s = servidor(&dir);
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"q",
                    "colunas":[{"nome":"id","tipo":"Int8"},
                               {"nome":"renda","tipo":"Int8"},
                               {"nome":"cod","tipo":"Str(20)"},
                               {"nome":"ativo","tipo":"Bool"},
                               {"nome":"faixa","tipo":"Int1","calculada":"renda / 1000"},
                               {"nome":"dia","tipo":"Date","calculada":"cod"},
                               {"nome":"neg","tipo":"Int8","calculada":"-ativo"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "marcar_lgpd",
        &pedido(
            r#"{"database":"b","tabela":"q",
                    "colunas":{"renda":"sensivel","cod":"pessoal","ativo":"pessoal"}}"#,
        ),
        &dono,
    )
    .unwrap();
    let mut vazou = Vec::new();
    for (quem, linha, valor) in [
        (
            "numero derivado",
            r#"{"id":1,"renda":500000,"cod":null,"ativo":null}"#,
            "500",
        ),
        (
            "texto que vira data",
            r#"{"id":2,"renda":null,"cod":"999.888.777-66","ativo":null}"#,
            CPF,
        ),
        (
            "booleano na conta",
            r#"{"id":3,"renda":null,"cod":null,"ativo":true}"#,
            "true",
        ),
    ] {
        let r = resposta(
            &s,
            "inserir",
            &format!(r#"{{"database":"b","tabela":"q","linha":{linha}}}"#),
        );
        // So a recusa e conferida aqui, e nao a frase: com o defeito
        // quem recusa o numero e o slot, que nao nomeia a conta.
        assert!(
            !r.starts_with('{'),
            "{quem}: o cenario nao recusou pela conta: {r}"
        );
        if r.contains(valor) {
            vazou.push(format!("{quem}: {r}"));
        }
    }
    assert!(
        vazou.is_empty(),
        "a recusa da conta citou o valor que partiu de coluna marcada:\n  {}",
        vazou.join("\n  ")
    );
}

/// **O comportamento velho.** Coluna sem marca continua citando o valor
/// curto -- e o que mostra a quem digitou o proprio erro. Uma guarda que
/// redigisse tudo passaria nos quatro testes de cima e cegaria o
/// diagnostico de toda tabela sem dado pessoal.
#[test]
fn a_coluna_sem_marca_continua_citando_o_valor() {
    let dir = DirTemp::novo("recusa-464-velho");
    let s = servidor(&dir);
    for (quem, op, corpo, valor) in [
        (
            "inserir",
            "inserir",
            format!(r#"{{"database":"b","tabela":"p","linha":{{"id":7,"quando":"{CPF}"}}}}"#),
            CPF,
        ),
        (
            "carga colada",
            "inserir_lote",
            format!(
                r#"{{"database":"b","tabela":"p","formato":"csv","parar_no_erro":false,
                        "texto":"id;quando\n8;{CPF}\n"}}"#
            ),
            CPF,
        ),
        (
            "faixa do Int4",
            "inserir",
            format!(r#"{{"database":"b","tabela":"p","linha":{{"id":9,"qtd":"{DOC}"}}}}"#),
            DOC,
        ),
    ] {
        let r = resposta(&s, op, &corpo);
        assert!(
            r.contains(valor),
            "{quem}: a coluna sem marca parou de citar: {r}"
        );
    }
}
