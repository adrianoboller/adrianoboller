//! Sincronia entre uma tabela do PhxSql e a tabela prima no outro banco.
//!
//! # O que isto e, e o que NAO e
//!
//! E a convergencia de ESTADO entre duas tabelas com a mesma chave primaria:
//! linha que so existe de um lado e copiada para o outro, linha diferente nos
//! dois e resolvida por quem for o DONO. Nao e replicacao de eventos -- o
//! diario do outro banco nao e lido -- e por isso tres limites sao de desenho,
//! nao de preguica:
//!
//! - **exclusao nao viaja.** Uma linha apagada de um lado REAPARECE na proxima
//!   sincronia, vinda do outro. Propagar exclusao exigiria distinguir "apagada
//!   la" de "nova aqui", e sem diario dos dois lados isso e adivinhacao. O
//!   caminho certo para apagar e apagar NOS DOIS antes da proxima rodada.
//! - **a chave primaria e a identidade.** Trocar a chave de uma linha e, para
//!   a sincronia, apagar uma e criar outra.
//! - **o teto e `max_linhas` da ligacao.** Tabela maior que o teto recusa com
//!   erro claro em vez de sincronizar metade e fingir que acabou.
//!
//! O empurrao usa `INSERT ... ON DUPLICATE KEY UPDATE`, entao empurrar a mesma
//! linha duas vezes e inofensivo -- a rodada pode cair no meio e recomecar.

use std::collections::HashMap;

use phxsql_core::carga::valor_de_texto;
use phxsql_core::datahora::{data_iso, instante_iso};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

use super::conexao::Coluna;
use super::{entre_crases, literal};

/// Para onde o dado anda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sentido {
    /// So de la para ca.
    Puxar,
    /// So daqui para la.
    Empurrar,
    /// Os dois; conflito e decidido pelo `Dono`.
    Dois,
}

impl Sentido {
    pub fn de_texto(t: &str) -> Result<Sentido> {
        Ok(match t {
            "puxar" => Sentido::Puxar,
            "empurrar" => Sentido::Empurrar,
            "dois" | "" => Sentido::Dois,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "sentido {outro:?} desconhecido: use puxar, empurrar ou dois"
                )))
            }
        })
    }
    pub fn nome(self) -> &'static str {
        match self {
            Sentido::Puxar => "puxar",
            Sentido::Empurrar => "empurrar",
            Sentido::Dois => "dois",
        }
    }
}

/// Quem vence quando a MESMA linha esta diferente nos dois lados.
///
/// Marcar tudo para um lado por omissao desfaria em silencio o trabalho do
/// outro -- a mesma licao da janela de conflito. Aqui a resolucao e por LINHA
/// (a linha inteira do dono vence), porque sem diario nao ha como saber qual
/// COLUNA cada lado mexeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dono {
    Aqui,
    La,
}

impl Dono {
    pub fn de_texto(t: &str) -> Result<Dono> {
        Ok(match t {
            "aqui" | "" => Dono::Aqui,
            "la" => Dono::La,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "dono {outro:?} desconhecido: use aqui ou la"
                )))
            }
        })
    }
    pub fn nome(self) -> &'static str {
        match self {
            Dono::Aqui => "aqui",
            Dono::La => "la",
        }
    }
}

/// Uma tabela ligada: a prima de la, a nossa de ca, e a regra entre elas.
#[derive(Debug, Clone)]
pub struct Sincronia {
    pub remota: String,
    pub local_database: String,
    pub local_tabela: String,
    pub sentido: Sentido,
    pub dono: Dono,
    /// Coluna da chave primaria, igual nos dois lados.
    pub chave: String,
}

impl Sincronia {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("remota", Json::texto_de(&self.remota)),
            ("local_database", Json::texto_de(&self.local_database)),
            ("local_tabela", Json::texto_de(&self.local_tabela)),
            ("sentido", Json::texto_de(self.sentido.nome())),
            ("dono", Json::texto_de(self.dono.nome())),
            ("chave", Json::texto_de(&self.chave)),
        ])
    }

    pub fn de_json(j: &Json) -> Result<Sincronia> {
        let remota = j.texto_ou("remota", "").trim().to_string();
        if remota.is_empty() {
            return Err(PhxError::Esquema("sincronia sem \"remota\"".into()));
        }
        Ok(Sincronia {
            local_database: j.texto_ou("local_database", "").trim().to_string(),
            local_tabela: {
                let t = j.texto_ou("local_tabela", "").trim().to_string();
                if t.is_empty() {
                    remota.clone()
                } else {
                    t
                }
            },
            remota,
            sentido: Sentido::de_texto(j.texto_ou("sentido", "dois"))?,
            dono: Dono::de_texto(j.texto_ou("dono", "aqui"))?,
            chave: j.texto_ou("chave", "").trim().to_string(),
        })
    }
}

/// O tipo local para uma coluna do outro banco.
///
/// # As duas contas que nao sao obvias
///
/// - `tamanho` de texto vem em BYTES do fio, e o utf8mb4 reserva 4 por
///   caractere: um `VARCHAR(60)` chega como 240. Dividir por 4 devolve o que a
///   pessoa declarou la.
/// - `tamanho` de `DECIMAL(p,s)` chega como p + 2 quando ha casa decimal (sinal
///   e ponto) e p + 1 quando nao ha (so o sinal).
fn tipo_local(c: &Coluna) -> Result<ColumnType> {
    Ok(match c.tipo.as_str() {
        "TINYINT" => {
            // TINYINT(1) e a convencao de booleano do MySQL(R); os outros sao
            // numeros pequenos de verdade.
            if c.tamanho == 1 {
                ColumnType::Bool
            } else {
                ColumnType::Int1
            }
        }
        "SMALLINT" => ColumnType::Int2,
        "MEDIUMINT" | "INT" | "YEAR" => ColumnType::Int4,
        "BIGINT" | "BIT" => ColumnType::Int8,
        "FLOAT" => ColumnType::Real4,
        "DOUBLE" => ColumnType::Real8,
        "DECIMAL" => {
            let precisao = c.tamanho.saturating_sub(if c.decimais > 0 { 2 } else { 1 });
            ColumnType::Decimal {
                precisao: precisao.clamp(1, 38) as u8,
                escala: c.decimais,
            }
        }
        "DATE" => ColumnType::Date,
        "TIME" => ColumnType::Time,
        "DATETIME" | "TIMESTAMP" => ColumnType::DateTime,
        "CHAR" | "VARCHAR" | "ENUM" | "SET" => {
            let chars = (c.tamanho / 4).max(1);
            ColumnType::Str(chars.min(u16::MAX as u32) as u16)
        }
        "TEXT" | "JSON" => ColumnType::Memo,
        "BLOB" | "BINARY" | "VARBINARY" | "GEOMETRY" => ColumnType::Bin,
        outro => {
            return Err(PhxError::Esquema(format!(
                "coluna {:?}: o tipo {outro} do outro banco nao tem par aqui",
                c.nome
            )))
        }
    })
}

/// Monta o esquema local espelhando as colunas remotas, e diz qual e a chave.
///
/// A chave primaria vira indice UNICO local -- e o que permite o upsert sem
/// varrer -- e tem de ser de UMA coluna: chave composta fica para quando
/// alguem precisar dela de verdade, com o pedido na mesa.
pub fn esquema_local_de(nome: &str, colunas: &[Coluna]) -> Result<(Schema, String)> {
    if colunas.is_empty() {
        return Err(PhxError::Esquema(format!(
            "a tabela remota {nome:?} nao tem colunas visiveis"
        )));
    }
    let primarias: Vec<usize> = (0..colunas.len())
        .filter(|i| colunas[*i].primaria)
        .collect();
    let pk = match primarias.as_slice() {
        [um] => *um,
        [] => {
            return Err(PhxError::Esquema(format!(
                "a tabela remota {nome:?} nao tem chave primaria: sem ela nao ha \
                 como casar as linhas dos dois lados"
            )))
        }
        _ => {
            return Err(PhxError::Esquema(format!(
                "a tabela remota {nome:?} tem chave primaria composta, que a \
                 sincronia ainda nao cobre"
            )))
        }
    };

    let mut cols = Vec::with_capacity(colunas.len());
    for c in colunas {
        let mut col = Column::new(&c.nome, tipo_local(c)?);
        if !c.nulavel {
            col = col.obrigatoria();
        }
        cols.push(col);
    }
    let indice = IndexDef::new("porChave", vec![IndexColumn::asc(pk)]).unico();
    let esquema = Schema::new(nome, cols, vec![indice])?;
    Ok((esquema, colunas[pk].nome.clone()))
}

/// As posicoes das colunas de NEGOCIO do esquema local -- tudo menos as de
/// sistema (`softdeleted`, `rownum`), que o motor preenche sozinho.
///
/// A sincronia so fala destas: e o que deixa `inserir`/`atualizar` receberem a
/// linha sem as colunas do motor, como qualquer cliente.
pub fn posicoes_de_negocio(esquema: &Schema) -> Vec<usize> {
    esquema
        .colunas()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.nome != "softdeleted" && c.nome != "rownum")
        .map(|(i, _)| i)
        .collect()
}

/// Converte uma linha remota (texto do fio) para a linha de NEGOCIO local.
///
/// Toda coluna de negocio local precisa vir do outro lado -- faltar uma seria
/// inserir com buraco silencioso, entao e recusa com o conserto no texto.
pub fn linha_remota_para_negocio(
    esquema: &Schema,
    negocio: &[usize],
    mapa: &[(usize, usize)],
    remota: &[Option<String>],
) -> Result<Vec<Value>> {
    let mut linha = Vec::with_capacity(negocio.len());
    for pos in negocio {
        let de = mapa.iter().find(|(_, para)| para == pos).map(|(de, _)| *de);
        let Some(de) = de else {
            return Err(PhxError::Esquema(format!(
                "a coluna local {:?} nao existe na tabela remota: rode o \
                 assistente de novo para recriar a ligacao",
                esquema.colunas()[*pos].nome
            )));
        };
        let ty = &esquema.colunas()[*pos].ty;
        linha.push(match &remota[de] {
            None => Value::Null,
            Some(t) => valor_de_texto(t, ty)?,
        });
    }
    Ok(linha)
}

/// Casa as colunas remotas com as locais PELO NOME, nunca pela posicao.
///
/// Pela posicao, uma coluna acrescentada de um lado deslocaria todas as
/// seguintes e a sincronia gravaria cidade dentro de telefone -- com o CRC
/// batendo. E o mesmo motivo do retrato da replicacao.
pub fn mapa_de_colunas(esquema: &Schema, remotas: &[Coluna]) -> Result<Vec<(usize, usize)>> {
    let mut mapa = Vec::new();
    for (i, r) in remotas.iter().enumerate() {
        match esquema
            .colunas()
            .iter()
            .position(|c| c.nome.eq_ignore_ascii_case(&r.nome))
        {
            Some(p) => mapa.push((i, p)),
            None => {
                return Err(PhxError::Esquema(format!(
                    "a coluna remota {:?} nao existe na tabela local: rode o \
                     assistente de novo para recriar a ligacao",
                    r.nome
                )))
            }
        }
    }
    Ok(mapa)
}

/// O texto SQL de um `Value`, para o INSERT do empurrao.
pub fn valor_para_sql(v: &Value, ty: &ColumnType) -> Result<String> {
    Ok(match v {
        Value::Null => "NULL".into(),
        Value::Bool(b) => if *b { "1" } else { "0" }.into(),
        Value::Int(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::Real(x) => {
            if x.is_finite() {
                x.to_string()
            } else {
                "NULL".into()
            }
        }
        Value::Decimal(escalado) => {
            let escala = match ty {
                ColumnType::Decimal { escala, .. } => *escala as u32,
                _ => 0,
            };
            let base = 10i128.pow(escala);
            let inteiro = escalado / base;
            let fracao = (escalado % base).abs();
            if escala == 0 {
                inteiro.to_string()
            } else {
                format!(
                    "{}{}.{:0largura$}",
                    if *escalado < 0 && inteiro == 0 {
                        "-"
                    } else {
                        ""
                    },
                    inteiro,
                    fracao,
                    largura = escala as usize
                )
            }
        }
        Value::Date(dias) => format!("'{}'", data_iso(*dias)),
        Value::Time(cs) => {
            let s = cs / 100;
            format!("'{:02}:{:02}:{:02}'", s / 3600, (s / 60) % 60, s % 60)
        }
        Value::DateTime(ms) => format!("'{}'", instante_iso(*ms).replace(',', ".")),
        Value::Str(t) | Value::Memo(t) => literal(t)?,
        // Os identificadores viajam como o texto canonico deles: o outro lado
        // nao tem tipo UUID garantido, e qualquer VARCHAR os recebe.
        Value::Uuid(u) => literal(&u.to_string())?,
        Value::Uuid256(u) => literal(&u.to_string())?,
        Value::Bin(b) => {
            let mut s = String::with_capacity(2 + b.len() * 2);
            s.push_str("0x");
            for byte in b {
                s.push_str(&format!("{byte:02x}"));
            }
            if b.is_empty() {
                "''".into()
            } else {
                s
            }
        }
    })
}

/// O que uma rodada decidiu fazer, antes de tocar em qualquer lado.
#[derive(Debug, Default)]
pub struct Plano {
    /// Linhas (ja no formato local) a gravar AQUI.
    pub para_ca: Vec<Vec<Value>>,
    /// Linhas locais a gravar LA.
    pub para_la: Vec<Vec<Value>>,
    pub iguais: u64,
    pub conflitos: u64,
}

/// Decide, sem gravar nada: e a parte da sincronia que se testa sem rede.
///
/// As duas entradas ja estao no formato LOCAL (a remota convertida por
/// `linha_remota_para_local`), com a chave em texto canonico como indice.
pub fn plano(
    sentido: Sentido,
    dono: Dono,
    remotas: &HashMap<String, Vec<Value>>,
    locais: &HashMap<String, Vec<Value>>,
) -> Plano {
    let mut p = Plano::default();
    for (chave, r) in remotas {
        match locais.get(chave) {
            None => {
                if sentido != Sentido::Empurrar {
                    p.para_ca.push(r.clone());
                }
            }
            Some(l) if l == r => p.iguais += 1,
            Some(l) => {
                p.conflitos += 1;
                match sentido {
                    Sentido::Puxar => p.para_ca.push(r.clone()),
                    Sentido::Empurrar => p.para_la.push(l.clone()),
                    Sentido::Dois => match dono {
                        Dono::Aqui => p.para_la.push(l.clone()),
                        Dono::La => p.para_ca.push(r.clone()),
                    },
                }
            }
        }
    }
    for (chave, l) in locais {
        if !remotas.contains_key(chave) && sentido != Sentido::Puxar {
            p.para_la.push(l.clone());
        }
    }
    p
}

/// A chave de uma linha, em texto canonico, para casar os dois lados.
pub fn chave_canonica(v: &Value) -> String {
    match v {
        Value::Str(t) => t.clone(),
        Value::Int(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        outro => format!("{outro:?}"),
    }
}

/// Grava no lado de ca as linhas que o plano mandou: upsert pelo indice unico.
pub fn aplicar_para_ca(
    t: &mut Table,
    indice_da_chave: &str,
    pos_chave: usize,
    linhas: &[Vec<Value>],
) -> Result<(u64, u64)> {
    let (mut inseridas, mut alteradas) = (0u64, 0u64);
    for l in linhas {
        let achadas = t.buscar(indice_da_chave, &[l[pos_chave].clone()])?;
        match achadas.first() {
            Some(rowid) => {
                t.atualizar(*rowid, l)?;
                alteradas += 1;
            }
            None => {
                t.inserir(l)?;
                inseridas += 1;
            }
        }
    }
    Ok((inseridas, alteradas))
}

/// Monta os INSERT ... ON DUPLICATE KEY UPDATE do empurrao, em lotes.
///
/// O ON DUPLICATE e o que torna a rodada REENTRAVEL: cair no meio e recomecar
/// grava a mesma linha de novo e nada dobra.
pub fn sql_do_empurrao(
    tabela_remota: &str,
    colunas: &[(String, ColumnType)],
    linhas: &[Vec<Value>],
    por_lote: usize,
) -> Result<Vec<String>> {
    if linhas.is_empty() {
        return Ok(Vec::new());
    }
    let nomes: Vec<String> = colunas.iter().map(|(n, _)| entre_crases(n)).collect();
    let atualiza: Vec<String> = nomes.iter().map(|n| format!("{n}=VALUES({n})")).collect();

    let mut sqls = Vec::new();
    for lote in linhas.chunks(por_lote.max(1)) {
        let mut valores = Vec::with_capacity(lote.len());
        for l in lote {
            let mut celulas = Vec::with_capacity(colunas.len());
            for (i, (_, ty)) in colunas.iter().enumerate() {
                celulas.push(valor_para_sql(&l[i], ty)?);
            }
            valores.push(format!("({})", celulas.join(",")));
        }
        sqls.push(format!(
            "INSERT INTO {} ({}) VALUES {} ON DUPLICATE KEY UPDATE {}",
            entre_crases(tabela_remota),
            nomes.join(","),
            valores.join(","),
            atualiza.join(",")
        ));
    }
    Ok(sqls)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn col(nome: &str, tipo: &str, tamanho: u32, decimais: u8, primaria: bool) -> Coluna {
        Coluna {
            nome: nome.into(),
            tipo: tipo.into(),
            tamanho,
            decimais,
            nulavel: !primaria,
            primaria,
            ..Coluna::default()
        }
    }

    fn linha(id: i64, nome: &str) -> Vec<Value> {
        vec![Value::Int(id), Value::Str(nome.into())]
    }

    fn lados(
        remotas: &[(i64, &str)],
        locais: &[(i64, &str)],
    ) -> (HashMap<String, Vec<Value>>, HashMap<String, Vec<Value>>) {
        let monta = |lado: &[(i64, &str)]| {
            lado.iter()
                .map(|(id, n)| (id.to_string(), linha(*id, n)))
                .collect()
        };
        (monta(remotas), monta(locais))
    }

    // ------------------------------------------------------------- o plano
    //
    // A parte que decide sem rede. Cada caso escreve o resultado esperado
    // ANTES de olhar o codigo: linha so de la, linha so de ca, a mesma linha
    // diferente nos dois -- nos tres sentidos e com os dois donos.

    #[test]
    fn plano_dois_sentidos_cada_falta_atravessa_para_o_outro_lado() {
        let (r, l) = lados(&[(1, "a"), (2, "so_la")], &[(1, "a"), (3, "so_ca")]);
        let p = plano(Sentido::Dois, Dono::Aqui, &r, &l);
        assert_eq!(p.para_ca, vec![linha(2, "so_la")]);
        assert_eq!(p.para_la, vec![linha(3, "so_ca")]);
        assert_eq!((p.iguais, p.conflitos), (1, 0));
    }

    #[test]
    fn plano_conflito_vai_para_o_lado_do_dono_e_nunca_para_os_dois() {
        // A linha 1 esta diferente nos dois lados. Se o conflito fosse
        // copiado para os DOIS, a rodada desfaria o trabalho de alguem --
        // e o teste falha se qualquer conserto reintroduzir isso.
        let (r, l) = lados(&[(1, "de_la")], &[(1, "de_ca")]);

        let p = plano(Sentido::Dois, Dono::Aqui, &r, &l);
        assert_eq!(p.para_la, vec![linha(1, "de_ca")]);
        assert!(p.para_ca.is_empty(), "dono aqui, e mesmo assim puxou");
        assert_eq!(p.conflitos, 1);

        let p = plano(Sentido::Dois, Dono::La, &r, &l);
        assert_eq!(p.para_ca, vec![linha(1, "de_la")]);
        assert!(p.para_la.is_empty(), "dono la, e mesmo assim empurrou");
        assert_eq!(p.conflitos, 1);
    }

    #[test]
    fn plano_puxar_nunca_escreve_la() {
        let (r, l) = lados(
            &[(1, "de_la"), (2, "nova_la")],
            &[(1, "de_ca"), (3, "so_ca")],
        );
        let p = plano(Sentido::Puxar, Dono::Aqui, &r, &l);
        // Puxando, o conflito vem de la MESMO com dono aqui: puxar e
        // declarar que la e a origem. O dono so arbitra no sentido "dois".
        assert_eq!(p.para_ca.len(), 2);
        assert!(p.para_la.is_empty(), "sentido puxar gerou escrita remota");
    }

    #[test]
    fn plano_empurrar_nunca_escreve_ca() {
        let (r, l) = lados(
            &[(1, "de_la"), (2, "nova_la")],
            &[(1, "de_ca"), (3, "so_ca")],
        );
        let p = plano(Sentido::Empurrar, Dono::La, &r, &l);
        assert_eq!(p.para_la.len(), 2); // o conflito e a que so existe aqui
        assert!(p.para_ca.is_empty(), "sentido empurrar gravou local");
    }

    #[test]
    fn plano_linha_igual_nao_viaja() {
        let (r, l) = lados(&[(1, "a"), (2, "b")], &[(1, "a"), (2, "b")]);
        let p = plano(Sentido::Dois, Dono::Aqui, &r, &l);
        assert!(p.para_ca.is_empty() && p.para_la.is_empty());
        assert_eq!((p.iguais, p.conflitos), (2, 0));
    }

    // ------------------------------------------------- o espelho dos tipos

    #[test]
    fn tipo_local_desfaz_a_conta_de_bytes_do_utf8mb4() {
        // Um VARCHAR(60) chega como 240 bytes; a tabela local tem de nascer
        // com 60 caracteres, nao 240 -- senao toda ficha "cabe" quatro vezes.
        assert_eq!(
            tipo_local(&col("nome", "VARCHAR", 240, 0, false)).unwrap(),
            ColumnType::Str(60)
        );
    }

    #[test]
    fn tipo_local_tinyint_um_e_booleano_os_outros_sao_numeros() {
        assert_eq!(
            tipo_local(&col("ativo", "TINYINT", 1, 0, false)).unwrap(),
            ColumnType::Bool
        );
        assert_eq!(
            tipo_local(&col("idade", "TINYINT", 4, 0, false)).unwrap(),
            ColumnType::Int1
        );
    }

    #[test]
    fn tipo_local_decimal_desconta_sinal_e_ponto_do_tamanho_do_fio() {
        // DECIMAL(10,2) viaja como tamanho 12 (sinal e ponto);
        // DECIMAL(5,0) como 6 (so o sinal).
        assert_eq!(
            tipo_local(&col("limite", "DECIMAL", 12, 2, false)).unwrap(),
            ColumnType::Decimal {
                precisao: 10,
                escala: 2
            }
        );
        assert_eq!(
            tipo_local(&col("qtde", "DECIMAL", 6, 0, false)).unwrap(),
            ColumnType::Decimal {
                precisao: 5,
                escala: 0
            }
        );
    }

    #[test]
    fn tipo_local_recusa_o_que_nao_tem_par() {
        assert!(tipo_local(&col("area", "POLYGON", 0, 0, false)).is_err());
    }

    // ------------------------------------------------- o esquema espelhado

    #[test]
    fn esquema_exige_chave_primaria_de_uma_coluna() {
        let com_pk = [
            col("id", "INT", 11, 0, true),
            col("nome", "VARCHAR", 240, 0, false),
        ];
        let (esq, chave) = esquema_local_de("clientes", &com_pk).unwrap();
        assert_eq!(chave, "id");
        assert!(esq.indices().iter().any(|i| i.nome == "porChave"));

        let sem_pk = [col("nome", "VARCHAR", 240, 0, false)];
        assert!(esquema_local_de("clientes", &sem_pk).is_err());

        let composta = [col("a", "INT", 11, 0, true), col("b", "INT", 11, 0, true)];
        assert!(esquema_local_de("clientes", &composta).is_err());
    }

    // ------------------------------------------------------ o mapa por nome

    #[test]
    fn mapa_casa_por_nome_mesmo_com_as_colunas_remotas_em_outra_ordem() {
        // Pela posicao, "cidade" cairia dentro de "nome" -- com o CRC batendo.
        let remotas = [
            col("id", "INT", 11, 0, true),
            col("nome", "VARCHAR", 240, 0, false),
        ];
        let (esq, _) = esquema_local_de("clientes", &remotas).unwrap();

        let em_outra_ordem = [remotas[1].clone(), remotas[0].clone()];
        let mapa = mapa_de_colunas(&esq, &em_outra_ordem).unwrap();
        let pos_nome = esq.colunas().iter().position(|c| c.nome == "nome").unwrap();
        let pos_id = esq.colunas().iter().position(|c| c.nome == "id").unwrap();
        assert_eq!(mapa, vec![(0, pos_nome), (1, pos_id)]);
    }

    #[test]
    fn mapa_recusa_coluna_remota_que_nao_existe_aqui() {
        let remotas = [col("id", "INT", 11, 0, true)];
        let (esq, _) = esquema_local_de("clientes", &remotas).unwrap();
        let com_nova = [remotas[0].clone(), col("telefone", "VARCHAR", 80, 0, false)];
        assert!(mapa_de_colunas(&esq, &com_nova).is_err());
    }

    // -------------------------------------------------- o texto do empurrao

    #[test]
    fn decimal_negativo_menor_que_um_leva_o_sinal_emprestado() {
        // -0,50 escalado e -50: o inteiro da divisao e 0, que nao carrega
        // sinal -- sem o emprestimo, sairia "0.50" e o outro banco gravaria
        // credito onde era divida.
        let ty = ColumnType::Decimal {
            precisao: 10,
            escala: 2,
        };
        assert_eq!(valor_para_sql(&Value::Decimal(-50), &ty).unwrap(), "-0.50");
        assert_eq!(
            valor_para_sql(&Value::Decimal(-1250), &ty).unwrap(),
            "-12.50"
        );
        assert_eq!(valor_para_sql(&Value::Decimal(1250), &ty).unwrap(), "12.50");
        assert_eq!(valor_para_sql(&Value::Decimal(305), &ty).unwrap(), "3.05");
    }

    #[test]
    fn texto_com_aspa_recusa_em_vez_de_escapar() {
        // A mesma decisao do modulo: escapar depende do modo do outro
        // servidor (NO_BACKSLASH_ESCAPES), entao texto que emendaria SQL e
        // recusado com erro claro -- nunca emendado. O limite esta no
        // DBLINK.md; se um dia cair, e por decisao, nao por acidente.
        let ty = ColumnType::Str(60);
        assert!(valor_para_sql(&Value::Str("Sant'Ana".into()), &ty).is_err());
        assert_eq!(
            valor_para_sql(&Value::Str("Blumenau".into()), &ty).unwrap(),
            "'Blumenau'"
        );
    }

    #[test]
    fn empurrao_em_lotes_e_sempre_reentravel() {
        let colunas = vec![
            ("id".to_string(), ColumnType::Int4),
            ("nome".to_string(), ColumnType::Str(60)),
        ];
        let linhas = vec![linha(1, "a"), linha(2, "b"), linha(3, "c")];
        let sqls = sql_do_empurrao("clientes", &colunas, &linhas, 2).unwrap();
        assert_eq!(sqls.len(), 2, "3 linhas em lotes de 2 sao 2 comandos");
        for sql in &sqls {
            // E o ON DUPLICATE que deixa a rodada cair no meio e recomecar.
            assert!(sql.contains("ON DUPLICATE KEY UPDATE"), "{sql}");
            assert!(
                sql.starts_with("INSERT INTO `clientes` (`id`,`nome`)"),
                "{sql}"
            );
        }
        assert!(sqls[0].contains("(1,'a'),(2,'b')"));
        assert!(sqls[1].contains("(3,'c')"));
    }
}
