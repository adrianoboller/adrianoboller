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
use phxsql_core::json::Json;
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

/// Um indice candidato a decidir «ja existe?» -- o que a escolha precisa
/// saber dele, e nada mais.
///
/// Existe para a regra da escolha ter UMA implementacao servindo dois donos:
/// o motor, que tem o `Schema`, e o direito por coluna, que so tem o
/// `esquema` em JSON e precisa achar a MESMA linha que o upsert vai
/// sobrescrever para repor nela o que o usuario nao pode alterar. Uma
/// segunda copia da regra la seria a que envelhece: o dia em que a escolha
/// aprendesse a olhar outro criterio, o direito por coluna reporia o valor
/// numa linha e o motor gravaria em outra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidato {
    pub nome: String,
    pub unico: bool,
    pub primario: bool,
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
    let candidatos: Vec<Candidato> = esquema
        .indices()
        .iter()
        .map(|i| Candidato {
            nome: i.nome.clone(),
            unico: i.unico,
            primario: i.primario,
        })
        .collect();
    escolher_indice(esquema.nome(), &candidatos, pedido)
}

/// A regra de [`indice_do_upsert`], sobre a lista de candidatos -- ver
/// [`Candidato`] para o motivo de ela estar separada do `Schema`.
pub fn escolher_indice(tabela: &str, candidatos: &[Candidato], pedido: &str) -> Result<String> {
    let pedido = pedido.trim();
    if !pedido.is_empty() {
        let def = candidatos
            .iter()
            .find(|i| i.nome.eq_ignore_ascii_case(pedido))
            .ok_or_else(|| {
                PhxError::NaoEncontrado(format!("o indice {pedido:?} nao existe em {tabela}"))
            })?;
        if !def.unico {
            return Err(PhxError::Esquema(format!(
                "o indice {pedido:?} nao e unico, entao ele nao sabe dizer se a \
                 linha ja existe: uma chave repetida ali e legitima"
            )));
        }
        return Ok(def.nome.clone());
    }
    if let Some(pk) = candidatos.iter().find(|i| i.primario) {
        return Ok(pk.nome.clone());
    }
    let unicos: Vec<&str> = candidatos
        .iter()
        .filter(|i| i.unico)
        .map(|i| i.nome.as_str())
        .collect();
    match unicos.len() {
        0 => Err(PhxError::Esquema(format!(
            "{tabela} nao tem chave primaria nem indice unico, entao nao ha como saber \
             se a linha ja existe. Declare um indice unico ou nao use \"se_existir\""
        ))),
        1 => Ok(unicos[0].to_string()),
        _ => Err(PhxError::Esquema(format!(
            "{tabela} tem mais de um indice unico e nenhuma chave primaria: diga em \
             \"indice\" qual deles decide se a linha ja existe. Os candidatos \
             sao {}",
            unicos
                .iter()
                .map(|n| format!("{n:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// O campo `"atualizar"` do pedido: o que entra POR CIMA da linha que ja
/// existe, conferido contra o `se_existir`.
///
/// E o `ON CONFLICT (c) DO UPDATE SET ...` e o `ON DUPLICATE KEY UPDATE ...`
/// do SQL -- o tradutor poe o SET aqui --, e vale igual pelo protocolo. Com
/// ele, a linha existente recebe SO estas colunas; a `linha`/`valores` do
/// pedido so entra quando a chave nao existe. Sem ele, o contrato de sempre:
/// a linha do pedido inteira por cima.
///
/// Ele so faz sentido com `se_existir: "atualizar"`, e fora disso RECUSA em
/// vez de ignorar: o motor passou uma rodada inteira ignorando o campo
/// calado -- o `INSERT ... ON CONFLICT DO UPDATE SET nome = 'B'` gravava o
/// `VALUES` por cima da linha, com NULL nas colunas que o `VALUES` nao
/// trazia --, e nenhum erro apareceu porque nenhum foi emitido.
pub fn atualizar_do_pedido(p: &Json, modo: Option<SeExistir>) -> Result<Option<&Json>> {
    let Some(a) = p.campo("atualizar") else {
        return Ok(None);
    };
    if modo != Some(SeExistir::Atualizar) {
        return Err(PhxError::Esquema(
            "o campo \"atualizar\" so vale com se_existir: \"atualizar\" -- ele diz o \
             que entra por cima da linha que ja existe, e sem esse modo nenhuma \
             linha e sobrescrita"
                .into(),
        ));
    }
    if !matches!(a, Json::Objeto(_)) {
        return Err(PhxError::Esquema(
            "\"atualizar\" precisa ser um objeto {coluna: valor}: sao as colunas que \
             entram por cima da linha que ja existe"
                .into(),
        ));
    }
    Ok(Some(a))
}

/// A linha LIDA com o `atualizar` por cima, coluna a coluna e pelo tipo do
/// esquema -- o `VALUES` fica de fora, como no SQL.
///
/// Por VALOR e nao por um vaivem JSON: converter a linha inteira para JSON
/// e de volta so para trocar duas colunas pagaria a conversao de toda coluna
/// da tabela, e `json_para_valor` ja sabe converter uma. Coluna que nao
/// existe recusa nomeando, como o `json_para_linha` faz na insercao.
pub fn mesclar(velha: &[Value], set: &Json, esquema: &Schema) -> Result<Vec<Value>> {
    let Json::Objeto(pares) = set else {
        return Err(PhxError::Esquema(
            "\"atualizar\" precisa ser um objeto {coluna: valor}".into(),
        ));
    };
    let mut nova = velha.to_vec();
    for (k, v) in pares {
        let i = esquema.coluna_por_nome(k).ok_or_else(|| {
            PhxError::Tipo(format!("coluna {k:?} nao existe em {}", esquema.nome()))
        })?;
        let ty = &esquema.colunas()[i].ty;
        nova[i] = crate::valores::json_para_valor(v, ty)?;
    }
    Ok(nova)
}

/// O que aconteceu com uma linha.
#[derive(Debug, Clone, PartialEq)]
pub struct Feito {
    pub rowid: RowId,
    pub atualizada: bool,
    pub ignorada: bool,
    /// A linha como FICOU gravada, quando ela nao e a que veio: o upsert com
    /// `atualizar` grava a lida mesclada, e quem mantem a copia em memoria
    /// precisa dessa e nao da do pedido. `None` quer dizer «a que veio».
    pub gravada: Option<Vec<Value>>,
}

impl Feito {
    pub fn inserida(rowid: RowId) -> Feito {
        Feito {
            rowid,
            atualizada: false,
            ignorada: false,
            gravada: None,
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
///
/// # O `atualizar`, quando vem
///
/// Com `Some(set)`, a linha que ja existe recebe SO o `set` por cima da que
/// esta gravada -- e a `linha` do pedido nao entra nela. E o que o SQL
/// promete no `ON CONFLICT DO UPDATE SET`: o `VALUES` e para a linha nova, o
/// `SET` e para a que existe. Sem `set`, a `linha` inteira por cima, que e o
/// contrato de sempre do `se_existir: "atualizar"`.
pub fn aplicar(
    t: &mut Table,
    indice: &str,
    linha: &[Value],
    modo: SeExistir,
    atualizar: Option<&Json>,
) -> Result<Feito> {
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
                gravada: None,
            }),
            SeExistir::Atualizar => {
                let gravada = match atualizar {
                    None => None,
                    Some(set) => {
                        let velha = t.ler(rowid)?.ok_or_else(|| {
                            PhxError::Corrompido(format!(
                                "o indice {indice} apontou para o rowid {rowid}, que nao \
                                 se le"
                            ))
                        })?;
                        Some(mesclar(&velha, set, t.esquema())?)
                    }
                };
                t.atualizar(rowid, gravada.as_deref().unwrap_or(linha))?;
                Ok(Feito {
                    rowid,
                    atualizada: true,
                    ignorada: false,
                    gravada,
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
