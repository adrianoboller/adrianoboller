//! O upsert por indice unico: um caminho so, para os dois donos.
//!
//! # Por que ele saiu do DbLink
//!
//! `dblink/sincronia.rs::aplicar_para_ca` ja fazia exatamente isto -- buscar
//! no indice unico e decidir entre `atualizar` e `inserir` -- desde que a
//! sincronia existe. Quando o `inserir` do protocolo ganhou `se_existir`, a
//! escolha era copiar cinco linhas ou extrair uma funcao.
//!
//! Copiar teria compilado e teria passado nos testes. E teria criado o defeito
//! que este projeto ja pagou tres vezes: **conserto que entra num caminho e
//! nao no irmao**. O dia em que alguem ensinasse a busca a olhar o indice
//! parcial, ou a tratar a chave nula de outro jeito, a copia esquecida
//! passaria a responder diferente -- e a divergencia apareceria como «a
//! sincronia duplicou a linha que o upsert atualizou», que ninguem acha por
//! leitura.
//!
//! # O que ele NAO decide
//!
//! Permissao, gatilho, transacao e trilha. Isto aqui e o passo de MOTOR:
//! buscar, e gravar de um jeito ou de outro. Quem chama e quem sabe se ha
//! gatilho para disparar e se ha transacao para empilhar -- e e por isso que
//! o `op_inserir` continua sendo o dono do caminho de escrita, e nao esta
//! funcao.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::value::Value;
use phxsql_core::RowId;
use phxsql_store::table::Table;

/// O que fazer quando a chave unica ja existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeExistir {
    /// Nao grava nada e devolve o rowid de quem ja estava la.
    Ignorar,
    /// Grava a linha por cima da que existe.
    Atualizar,
}

impl SeExistir {
    /// `None` quando o pedido nao traz o campo -- e ai o `inserir` e o de
    /// sempre, que RECUSA a chave repetida. Guarda nova entra pedida.
    pub fn de_texto(t: &str) -> Result<Option<SeExistir>> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "" => None,
            "ignorar" | "ignore" | "nada" => Some(SeExistir::Ignorar),
            "atualizar" | "update" | "substituir" => Some(SeExistir::Atualizar),
            outro => {
                return Err(PhxError::Esquema(format!(
                    "se_existir {outro:?} nao existe (use \"ignorar\" ou \"atualizar\")"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            SeExistir::Ignorar => "ignorar",
            SeExistir::Atualizar => "atualizar",
        }
    }
}

/// Os valores da chave desta linha, para um indice.
///
/// Mora aqui e nao no `servidor.rs` porque os dois donos do upsert precisam
/// dela, e o DbLink nao enxerga o modulo do servidor.
pub fn valores_do_indice(esquema: &Schema, indice: &str, linha: &[Value]) -> Vec<Value> {
    let Some(def) = esquema.indices().iter().find(|i| i.nome == indice) else {
        return Vec::new();
    };
    def.colunas
        .iter()
        .filter_map(|c| linha.get(c.coluna).cloned())
        .collect()
}

/// Qual indice unico decide "ja existe?".
///
/// # A ordem, e por que ambiguo RECUSA
///
/// 1. o indice que o pedido nomeou -- e ele tem de ser unico;
/// 2. a chave PRIMARIA, que e a resposta obvia quando ela existe;
/// 3. o unico indice unico da tabela, quando so ha um.
///
/// Dois indices unicos e nenhum primario e AMBIGUO, e a recusa nomeia os
/// candidatos. Escolher o primeiro seria decidir por conta qual chave define
/// "a mesma linha" -- e num cadastro com `cpf` unico e `email` unico as duas
/// respostas sao diferentes e as duas parecem certas.
pub fn indice_do_upsert(esquema: &Schema, pedido: &str) -> Result<String> {
    let pedido = pedido.trim();
    if !pedido.is_empty() {
        let def = esquema
            .indices()
            .iter()
            .find(|i| i.nome.eq_ignore_ascii_case(pedido))
            .ok_or_else(|| {
                PhxError::NaoEncontrado(format!(
                    "o indice {pedido:?} nao existe em {}",
                    esquema.nome()
                ))
            })?;
        if !def.unico {
            return Err(PhxError::Esquema(format!(
                "o indice {pedido:?} nao e unico, entao ele nao sabe dizer se a \
                 linha ja existe: uma chave repetida ali e legitima"
            )));
        }
        return Ok(def.nome.clone());
    }
    if let Some(pk) = esquema.chave_primaria() {
        return Ok(pk.nome.clone());
    }
    let unicos: Vec<&str> = esquema
        .indices()
        .iter()
        .filter(|i| i.unico)
        .map(|i| i.nome.as_str())
        .collect();
    match unicos.len() {
        0 => Err(PhxError::Esquema(format!(
            "{} nao tem chave primaria nem indice unico, entao nao ha como saber \
             se a linha ja existe. Declare um indice unico ou nao use \"se_existir\"",
            esquema.nome()
        ))),
        1 => Ok(unicos[0].to_string()),
        _ => Err(PhxError::Esquema(format!(
            "{} tem mais de um indice unico e nenhuma chave primaria: diga em \
             \"indice\" qual deles decide se a linha ja existe. Os candidatos \
             sao {}",
            esquema.nome(),
            unicos
                .iter()
                .map(|n| format!("{n:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// O que aconteceu com uma linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Feito {
    pub rowid: RowId,
    pub atualizada: bool,
    pub ignorada: bool,
}

impl Feito {
    pub fn inserida(rowid: RowId) -> Feito {
        Feito {
            rowid,
            atualizada: false,
            ignorada: false,
        }
    }
}

/// Grava uma linha decidindo entre inserir e atualizar pelo indice unico.
///
/// # A chave NULA nao encontra nada, e isso e o certo
///
/// `buscar` com uma chave que tem nulo nao acha linha nenhuma, entao a linha
/// entra como nova -- que e o comportamento do SQL: `NULL` nao e igual a
/// `NULL`, nem para efeito de chave unica. Uma tabela cuja chave e uma
/// `Sequence` cai sempre aqui na primeira gravacao, e tem de cair: o valor
/// ainda nao existe quando a linha e montada.
pub fn aplicar(t: &mut Table, indice: &str, linha: &[Value], modo: SeExistir) -> Result<Feito> {
    let chave = valores_do_indice(t.esquema(), indice, linha);
    if chave.is_empty() || chave.iter().any(Value::e_null) {
        return Ok(Feito::inserida(t.inserir(linha)?));
    }
    match t.buscar(indice, &chave)?.first().copied() {
        Some(rowid) => match modo {
            SeExistir::Ignorar => Ok(Feito {
                rowid,
                atualizada: false,
                ignorada: true,
            }),
            SeExistir::Atualizar => {
                t.atualizar(rowid, linha)?;
                Ok(Feito {
                    rowid,
                    atualizada: true,
                    ignorada: false,
                })
            }
        },
        None => Ok(Feito::inserida(t.inserir(linha)?)),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn esquema(indices: Vec<IndexDef>) -> Schema {
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cpf", ColumnType::Str(14)),
                Column::new("email", ColumnType::Str(40)),
            ],
            indices,
        )
        .unwrap()
    }

    /// A chave primaria e a resposta obvia, e ela vence os outros unicos.
    #[test]
    fn a_primaria_decide_quando_existe() {
        let e = esquema(vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico(),
        ]);
        assert_eq!(indice_do_upsert(&e, "").unwrap(), "porId");
    }

    /// Um unico indice unico e sem primaria: nao ha o que escolher.
    #[test]
    fn o_unico_unico_serve_sozinho() {
        let e = esquema(vec![
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico()
        ]);
        assert_eq!(indice_do_upsert(&e, "").unwrap(), "porCpf");
    }

    /// **Dois unicos e nenhuma primaria RECUSA, nomeando os candidatos.**
    ///
    /// Num cadastro com `cpf` unico e `email` unico, "a mesma linha" tem duas
    /// respostas diferentes e as duas parecem certas -- escolher a primeira
    /// seria decidir por conta qual delas o dono quis.
    #[test]
    fn dois_unicos_sem_primaria_recusam_nomeando() {
        let e = esquema(vec![
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico(),
            IndexDef::new("porEmail", vec![IndexColumn::asc(2)]).unico(),
        ]);
        let erro = indice_do_upsert(&e, "").expect_err("escolheu sozinho");
        let t = erro.to_string();
        assert!(t.contains("porCpf") && t.contains("porEmail"), "{t}");
        // E dizer qual resolve.
        assert_eq!(indice_do_upsert(&e, "porEmail").unwrap(), "porEmail");
    }

    /// Indice que nao e unico nao sabe dizer se a linha ja existe.
    #[test]
    fn indice_nao_unico_recusa() {
        let e = esquema(vec![IndexDef::new("porNome", vec![IndexColumn::asc(1)])]);
        let erro = indice_do_upsert(&e, "porNome").expect_err("aceitou indice repetivel");
        assert!(erro.to_string().contains("nao e unico"), "{erro}");
        // E sem indice unico nenhum, a recusa ensina o que fazer.
        let erro = indice_do_upsert(&e, "").expect_err("aceitou tabela sem chave");
        assert!(erro.to_string().contains("indice unico"), "{erro}");
    }
}
