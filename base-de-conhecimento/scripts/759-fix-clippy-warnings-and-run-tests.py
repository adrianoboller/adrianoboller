# Fix clippy warnings and run tests
# 28/08 19:50

import pathlib
p = pathlib.Path("crates/phxsql-server/src/valores.rs")
s = p.read_text()
antigo = """/// Le "12.34" com escala 2 e devolve 1234.

pub fn bytes_para_hex"""
novo = """/// Bytes crus em hexadecimal minusculo, para a tela e para o JSON.
pub fn bytes_para_hex"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """        // A conversao de tipo acontece aqui, com o esquema na mao, e uma linha
        // que nao converte entra na lista de recusadas em vez de derrubar a
        // carga inteira -- a menos que `parar_no_erro` mande parar.
        // A conversao acontece aqui, com o esquema na mao. Uma linha que nao
        // converte entra na lista de recusadas em vez de derrubar a carga
        // inteira -- a menos que `parar_no_erro` mande parar.
        let mut linhas: Vec<Vec<phxsql_core::value::Value>> = Vec::with_capacity(recebidas);
        let mut recusadas: Vec<(usize, String)> = Vec::new();
        for i in 0..recebidas {
            let convertida = match &colada {
                Some((c, _)) => phxsql_core::carga::linha_de_texto(c, i, t.esquema()),
                None => json_para_linha(&itens[i], t.esquema()),
            };"""
novo = """        // A conversao acontece aqui, com o esquema na mao. Uma linha que nao
        // converte entra na lista de recusadas em vez de derrubar a carga
        // inteira -- a menos que `parar_no_erro` mande parar.
        let mut linhas: Vec<Vec<phxsql_core::value::Value>> = Vec::with_capacity(recebidas);
        let mut recusadas: Vec<(usize, String)> = Vec::new();
        for i in 0..recebidas {
            let convertida = match (&colada, itens.get(i)) {
                (Some((c, _)), _) => phxsql_core::carga::linha_de_texto(c, i, t.esquema()),
                (None, Some(item)) => json_para_linha(item, t.esquema()),
                (None, None) => break,
            };"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
