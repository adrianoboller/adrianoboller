//! As operacoes do DbLink, escritas uma vez e falando os dois dialetos.
//!
//! # Por que elas sairam do servidor
//!
//! Estavam dentro de `servidor.rs`, montando SQL de MySQL(R) no meio da
//! resposta JSON. Enquanto havia um motor so isso era simples; com dois, o
//! mesmo corpo teria de escolher o SQL em cada `format!` -- e a escolha
//! esquecida em UM deles vira a consulta que falha so no PostgreSQL(R), que
//! ninguem descobre lendo.
//!
//! Aqui elas ficam ao lado do `dialeto`, que e onde a diferenca entre os dois
//! mora. O servidor passa a ser o que ele e: o portao de permissao e o
//! despacho.
//!
//! # O que continua sendo do servidor
//!
//! A trava. Toda operacao de DbLink exige `administrar`, e `dblink_consultar`
//! exige que as DUAS travas cedam -- a da ligacao e a do proprio servidor.
//! Isso nao desce para ca: portao de permissao e UM so, e espalha-lo por
//! quarenta operacoes e como se perde a que alguem esquecer.
//!
//! # A ligacao que falta, e ela e mecanica
//!
//! O `servidor.rs` ainda chama `Definicao::conectar`, que e so do MySQL(R).
//! Enquanto ele nao passar a chamar `Definicao::abrir`, `Motor::conecta()`
//! acende o botao do PostgreSQL(R) na tela e a operacao sai com "esta ligacao
//! e PostgreSQL(R): use `conectar_pg`, ou `abrir`". O delta abaixo foi
//! aplicado, compilado (zero avisos) e conferido contra a suite inteira antes
//! de ser escrito aqui:
//!
//! ```text
//! // no topo:
//! -use crate::dblink::{mysql, Definicao, Motor};
//! +use crate::dblink::{Definicao, Motor};
//!
//! // `ligar` passa a devolver a conexao comum:
//! -fn ligar(&self, p: &Json) -> Result<(Definicao, mysql::Conexao)> { ... d.conectar()? ... }
//! +fn ligar(&self, p: &Json) -> Result<(Definicao, crate::dblink::Conexao)> { ... d.abrir()? ... }
//!
//! // e os cinco corpos viram uma linha cada:
//! op_dblink_testar    -> operacoes::testar(&d, c)
//! op_dblink_bancos    -> operacoes::bancos(&d, c)
//! op_dblink_tabelas   -> operacoes::tabelas(&d, c, p)
//! op_dblink_estrutura -> operacoes::estrutura(&d, c, p)
//! op_dblink_ler       -> operacoes::ler(&d, c, p)
//! ```
//!
//! O `op_dblink_consultar` e o unico que nao e uma linha: as duas travas ficam
//! nele, e a conferencia do `so_consulta` passa a vir **antes** de conectar --
//! recusar depois de abrir a conexao gasta uma ida a rede para dizer nao.

use std::time::Instant;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use super::conexao::{Conexao, Resultado};
use super::{nome_seguro, Definicao};

/// Um campo de uma linha do resultado, como texto.
fn campo(l: &[Option<String>], i: usize) -> String {
    l.get(i).cloned().flatten().unwrap_or_default()
}

fn numero(l: &[Option<String>], i: usize) -> u64 {
    // O `reltuples` do PostgreSQL(R) vem como `-1` quando a tabela nunca foi
    // analisada, e como decimal em versoes antigas. As duas coisas viram zero
    // em vez de virar erro: e uma ESTIMATIVA ao lado do nome numa tela.
    campo(l, i)
        .split('.')
        .next()
        .and_then(|n| n.parse::<i64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(0) as u64
}

/// Qual base usar: a que o pedido escolheu, ou a do login.
fn base_escolhida(d: &Definicao, p: &Json) -> String {
    match p.texto_ou("database", "").trim() {
        "" => d.database.clone(),
        outro => outro.to_string(),
    }
}

/// `dblink_testar`: conecta, da `ping` e diz com quem esta falando.
pub fn testar(d: &Definicao, mut c: Conexao) -> Result<Json> {
    let comeco = Instant::now();
    c.ping()?;
    let versao = c.versao();
    let id = c.conexao_id();
    // A pergunta que o operador realmente quer: com quem o outro banco acha
    // que esta falando, e em que base caiu. As tres colunas saem na mesma
    // ordem nos dois motores porque o dialeto as montou assim.
    let quem = c.consultar(d.motor.sql_quem_sou(), 1).ok();
    c.encerrar();
    let dado = |i: usize| {
        quem.as_ref()
            .and_then(|r| r.celula(0, i))
            .unwrap_or_default()
    };
    Ok(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("dblink", Json::texto_de(&d.nome)),
        ("motor", Json::texto_de(d.motor.nome())),
        // A versao do `ParameterStatus`/do saudar e curta ("17.2"); a do
        // `version()` e a linha inteira. Vale a curta, que e a que cabe.
        (
            "versao",
            Json::texto_de(if versao.is_empty() { dado(2) } else { versao }),
        ),
        ("conexao_id", Json::de_u64(id as u64)),
        ("usuario_efetivo", Json::texto_de(dado(0))),
        ("database", Json::texto_de(dado(1))),
        ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
    ]))
}

/// `dblink_bancos`: as bases do outro servidor.
pub fn bancos(d: &Definicao, mut c: Conexao) -> Result<Json> {
    let r = c.consultar(d.motor.sql_bancos(), 1_000);
    c.encerrar();
    let r = r?;
    Ok(Json::objeto(vec![(
        "bancos",
        Json::Lista(
            r.linhas
                .iter()
                .filter_map(|l| l.first().cloned().flatten())
                .map(Json::texto_de)
                .collect(),
        ),
    )]))
}

/// `dblink_tabelas`: as tabelas de uma base, com tamanho e comentario.
pub fn tabelas(d: &Definicao, mut c: Conexao, p: &Json) -> Result<Json> {
    let base = base_escolhida(d, p);
    let sql = d.motor.sql_tabelas(&base)?;
    let r = c.consultar(&sql, 5_000);
    c.encerrar();
    let r = r?;
    Ok(Json::objeto(vec![
        ("dblink", Json::texto_de(&d.nome)),
        ("database", Json::texto_de(&base)),
        (
            "tabelas",
            Json::Lista(
                r.linhas
                    .iter()
                    .map(|l| {
                        Json::objeto(vec![
                            ("nome", Json::texto_de(campo(l, 0))),
                            ("tipo", Json::texto_de(campo(l, 1))),
                            ("motor", Json::texto_de(campo(l, 2))),
                            // ESTIMATIVA nos dois -- `TABLE_ROWS` do InnoDB e
                            // `reltuples` do PostgreSQL(R). Dizer que e
                            // contagem seria mentir num numero que a tela
                            // mostra ao lado do nome.
                            ("registros_estimados", Json::de_u64(numero(l, 3))),
                            ("bytes", Json::de_u64(numero(l, 4))),
                            ("comentario", Json::texto_de(campo(l, 5))),
                            ("schema", Json::texto_de(campo(l, 6))),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]))
}

/// `dblink_estrutura`: colunas e indices de uma tabela.
pub fn estrutura(d: &Definicao, mut c: Conexao, p: &Json) -> Result<Json> {
    let tabela = nome_seguro(p.texto_ou("tabela", ""))?;
    let base = base_escolhida(d, p);
    let colunas = c.consultar(&d.motor.sql_colunas(&base, &tabela)?, 2_000);
    let indices = c.consultar(&d.motor.sql_indices(&base, &tabela)?, 2_000);
    c.encerrar();
    Ok(Json::objeto(vec![
        ("dblink", Json::texto_de(&d.nome)),
        ("tabela", Json::texto_de(&tabela)),
        ("colunas", colunas?.para_json()),
        ("indices", indices?.para_json()),
    ]))
}

/// `dblink_ler`: o conteudo de uma tabela, paginado.
pub fn ler(d: &Definicao, mut c: Conexao, p: &Json) -> Result<Json> {
    let tabela = nome_seguro(p.texto_ou("tabela", ""))?;
    let base = base_escolhida(d, p);
    let alvo = d.motor.alvo(&base, &tabela)?;
    let limite = p
        .inteiro_ou("limite", d.max_linhas as i64)
        .clamp(1, d.max_linhas as i64);
    let salto = p.inteiro_ou("salto", 0).max(0);
    // A ordem entra pelo NOME da coluna, validado, e nunca pelo texto cru:
    // "ORDER BY " + o que vier da tela seria SQL de fora entrando inteiro.
    let ordem = match p.texto_ou("ordem", "").trim() {
        "" => String::new(),
        coluna => format!(
            " ORDER BY {} {}",
            d.motor.citar(&nome_seguro(coluna)?),
            if p.booleano_ou("descendente", false) {
                "DESC"
            } else {
                "ASC"
            }
        ),
    };
    // Pede uma linha a mais do que o teto: se ela vier, ha mais pagina.
    let sql = format!(
        "SELECT * FROM {alvo}{ordem}{}",
        d.motor.limite_offset(limite + 1, salto)
    );
    let r = c.consultar(&sql, limite as u64 + 1);
    c.encerrar();
    let mut r = r?;
    let tem_mais = r.linhas.len() as i64 > limite;
    r.linhas.truncate(limite as usize);
    let mut saida = r.para_json();
    if let Json::Objeto(campos) = &mut saida {
        campos.push(("dblink".into(), Json::texto_de(&d.nome)));
        campos.push(("tabela".into(), Json::texto_de(&tabela)));
        campos.push(("salto".into(), Json::de_u64(salto as u64)));
        campos.push(("tem_mais".into(), Json::Bool(tem_mais)));
    }
    Ok(saida)
}

/// `dblink_consultar`: uma instrucao escrita a mao.
///
/// As duas travas ficam com quem chama -- ver o cabecalho deste modulo.
pub fn consultar(d: &Definicao, mut c: Conexao, sql: &str, limite: u64) -> Result<Json> {
    if sql.trim().is_empty() {
        return Err(PhxError::Esquema("dblink_consultar sem \"sql\"".into()));
    }
    let comeco = Instant::now();
    let r = c.consultar(sql, limite);
    c.encerrar();
    let r = r?;
    let mut saida = r.para_json();
    if let Json::Objeto(campos) = &mut saida {
        campos.push(("dblink".into(), Json::texto_de(&d.nome)));
        campos.push((
            "ms".into(),
            Json::de_u64(comeco.elapsed().as_millis() as u64),
        ));
    }
    Ok(saida)
}

/// O resultado no formato da grade. Existe para quem ja tem o `Resultado`.
pub fn resultado_para_json(r: &Resultado) -> Json {
    r.para_json()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_reltuples_negativo_do_postgres_vira_zero() {
        // `-1` e o que o PostgreSQL(R) 14+ devolve para tabela que nunca foi
        // analisada. Mostrar "-1 registros" ao lado do nome seria pior que
        // mostrar zero.
        let linha = vec![Some("-1".to_string())];
        assert_eq!(numero(&linha, 0), 0);
        let linha = vec![Some("1234.0".to_string())];
        assert_eq!(numero(&linha, 0), 1234);
        let linha = vec![None];
        assert_eq!(numero(&linha, 0), 0);
    }

    #[test]
    fn campo_que_falta_vira_texto_vazio_e_nao_panico() {
        let linha = vec![Some("a".to_string())];
        assert_eq!(campo(&linha, 0), "a");
        assert_eq!(campo(&linha, 9), "");
    }
}
