# Generalize the system-column filter
# 28/08 18:28

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("use phxsql_core::schema::{Schema, COLUNA_SOFTDELETED};",
            "use phxsql_core::schema::{e_coluna_de_sistema, Schema};",1)
s=s.replace('''fn tirar_a_coluna_de_sistema(r: &mut Resultado) {
    let sistema = COLUNA_SOFTDELETED;
    let manter: Vec<bool> = r
        .colunas
        .iter()
        .map(|c| !c.nome.rsplit('.').next().is_some_and(|n| n == sistema))
        .collect();''','''fn tirar_a_coluna_de_sistema(r: &mut Resultado) {
    let manter: Vec<bool> = r
        .colunas
        .iter()
        .map(|c| {
            !c.nome
                .rsplit('.')
                .next()
                .is_some_and(e_coluna_de_sistema)
        })
        .collect();''',1)
s=s.replace('''    // Mesma razao da juncao: a coluna de sistema seria falso em toda linha.
    if r.colunas.last().is_some_and(|c| c.nome == COLUNA_SOFTDELETED) {
        r.colunas.pop();
        for linha in &mut r.linhas {
            linha.pop();
        }
    }''','''    // Mesma razao da juncao. As de sistema estao no FIM, entao sair de tras
    // para a frente basta.
    while r.colunas.last().is_some_and(|c| e_coluna_de_sistema(&c.nome)) {
        r.colunas.pop();
        for linha in &mut r.linhas {
            linha.pop();
        }
    }''',1)
# a doc da funcao
s=s.replace('''/// Tira a coluna `softdeleted` de cada lado do resultado.
///
/// Uma junção de duas tabelas traria DUAS colunas dela -- `c.softdeleted` e
/// `p.softdeleted` --, e as duas seriam falso em toda linha: a junção só lê
/// linha ativa. Seria ruído em cada resultado, com o agravante de empurrar as
/// colunas úteis para fora da primeira tela da grade.''',
'''/// Tira as colunas do motor de cada lado do resultado.
///
/// Uma junção de duas tabelas traria DUAS de cada -- `c.softdeleted` e
/// `p.softdeleted`, `c.rownum` e `p.rownum`. As de exclusão seriam falso em
/// toda linha, porque a junção só lê linha ativa; e dois números de ordem, de
/// tabelas diferentes, não paginam coisa nenhuma. Seria ruído em cada
/// resultado, com o agravante de empurrar as colunas úteis para fora da
/// primeira tela da grade.''',1)
io.open(p,'w',encoding='utf-8').write(s)
