use super::*;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

/// **O teste do laco morto.** Um diario com dois eventos -- um escrito
/// aqui, outro que chegou de «beta» -- e o `replicar` de quem se diz
/// «beta»: so o local pode voltar.
///
/// # O defeito que ele repoe
///
/// Tirar o filtro do `op_replicar` (mandar todos os eventos, ignorando o
/// `para`) faz este teste falhar na primeira asercao -- e e exatamente o
/// laco infinito do bidirecional: beta recebe de volta o que beta
/// escreveu, aplica, gera evento, e os dois servidores giram para sempre.
/// Provado tambem pela bancada, estagio (c): sem o filtro os eventos
/// crescem sozinhos em vez de parar em 2.
#[test]
fn evento_nao_volta_para_quem_o_escreveu() {
    let dir = DirTemp::novo("supressao");
    std::fs::create_dir_all(dir.join("loja")).unwrap();

    let hash_beta = bidirecional::hash_id("beta");
    {
        // A tabela nasce fora do servidor so para o diario ficar com os
        // dois eventos que interessam, sem precisar de um segundo
        // servidor no ar.
        let esquema = Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria()],
        )
        .unwrap();
        let mut t = Table::criar(dir.join("loja"), esquema)
            .unwrap()
            .com_imagem_no_diario(true);
        t.inserir(&[Value::Int(1), Value::Str("nascida aqui".into())])
            .unwrap();
        t.forcar_proximo_evento(1_700_000_000_000, hash_beta);
        t.inserir(&[Value::Int(2), Value::Str("veio de beta".into())])
            .unwrap();
        t.sincronizar().unwrap();
    }

    let txt = r#"{"token":"t","replicacao":{"papel":"multi","id_servidor":"alfa",
            "origens":[{"nome":"beta","host":"127.0.0.1","porta":5000,"token":"t"}]}}"#;
    let mut c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
    c.base = dir.to_path_buf();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();

    let pedir = |para: &str| {
        let p = Json::analisar(&format!(
            r#"{{"database":"loja","tabela":"clientes","desde":0,"max":100,"para":"{para}"}}"#
        ))
        .unwrap();
        s.op_replicar(&p, &sessao).unwrap()
    };

    // Beta pergunta: leva so o que NAO nasceu nele.
    let r = pedir("beta");
    let eventos = r.campo("eventos").and_then(Json::lista).unwrap().to_vec();
    assert_eq!(
        eventos.len(),
        1,
        "o evento de beta voltou para beta: e o laco infinito"
    );
    assert_eq!(eventos[0].inteiro_ou("rowid", 0), 1);

    // E a POSICAO anda por cima do suprimido: suprimir e nao mandar de
    // volta, nao fingir que o evento nao existe. Se `ate` parasse em 1,
    // beta pediria de novo a partir de 1 para sempre.
    assert_eq!(
        r.inteiro_ou("ate", 0),
        2,
        "a posicao nao andou pelo suprimido"
    );
    assert!(r.booleano_ou("fim", false));

    // Um terceiro servidor leva os DOIS: a supressao e por destino, e nao
    // uma censura no diario.
    let r = pedir("gama");
    assert_eq!(
        r.campo("eventos").and_then(Json::lista).unwrap().len(),
        2,
        "gama nao escreveu nenhum dos dois e tem de receber os dois"
    );

    // E sem o campo `para` -- toda replica de hoje -- nada e suprimido.
    // E o comportamento VELHO, e e o que mais importa: uma replica
    // classica nao pode receber menos do que recebia.
    let p =
        Json::analisar(r#"{"database":"loja","tabela":"clientes","desde":0,"max":100}"#).unwrap();
    let r = s.op_replicar(&p, &sessao).unwrap();
    assert_eq!(
        r.campo("eventos").and_then(Json::lista).unwrap().len(),
        2,
        "replica sem `para` passou a receber menos: quebrou o cliente antigo"
    );

    // A origem sai traduzida: a escrita local vira o hash DESTE servidor,
    // para o outro lado saber de quem veio sem tabela de traducao.
    let eventos = r.campo("eventos").and_then(Json::lista).unwrap();
    assert_eq!(
        eventos[0].inteiro_ou("origem", 0),
        bidirecional::hash_id("alfa") as i64
    );
    assert_eq!(eventos[1].inteiro_ou("origem", 0), hash_beta as i64);

    std::fs::remove_dir_all(&dir).unwrap();
}
