# Centralise the object extraction
# 28/08 16:26

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''                ok: resultado.is_ok(),
                duracao_ms: duracao,
                erro: resultado.as_ref().err().map(|e| e.to_string()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });

            if writeln!(saida, "{}", resposta.escrever()).is_err() {'''
b='''                ok: resultado.is_ok(),
                duracao_ms: duracao,
                erro: resultado.as_ref().err().map(|e| e.to_string()),
                // O objeto do pedido, para o log poder somar por tabela.
                ..objeto_do_pedido(&linha, &resultado)
            });

            if writeln!(saida, "{}", resposta.escrever()).is_err() {'''
assert a in s; s=s.replace(a,b,1)

a='''            database: pedido.texto_ou("database", "").to_string(),
            tabela: pedido.texto_ou("tabela", "").to_string(),
            codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        });'''
b='''            ..objeto_do_pedido(&pedido.corpo, &resultado)
        });'''
assert a in s; s=s.replace(a,b,1)

a='''fn resposta_erro(op: &str, e: &PhxError, ms: u64) -> Json {'''
b='''/// Os campos do log que saem do PEDIDO, e nao do resultado.
///
/// Devolve um `Acesso` so para preencher com `..`: os outros campos do
/// registro vem de quem chama, e repetir a leitura do corpo em dois lugares e
/// como os dois caminhos (porta de dados e web) divergiriam com o tempo.
fn objeto_do_pedido(corpo: &str, resultado: &Result<Json>) -> Acesso {
    let j = Json::analisar(corpo).unwrap_or(Json::Nulo);
    Acesso {
        database: j.texto_ou("database", "").to_string(),
        tabela: j.texto_ou("tabela", "").to_string(),
        codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        ..Acesso::default()
    }
}

fn resposta_erro(op: &str, e: &PhxError, ms: u64) -> Json {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
