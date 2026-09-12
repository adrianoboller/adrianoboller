//! Modo ledger encadeado privado -- SOBRE o motor Padrao, sem formato novo.
//!
//! Isto NAO e um `TipoDatabase` novo nem muda um byte do formato em disco: e
//! um MODO de usar uma [`Table`] comum cujo esquema tem as tres colunas que a
//! cadeia precisa -- `hash` (Uuid256), `anterior` (Uuid256) e `altura`
//! (Sequence) -- mais um indice unico `porAltura` sobre a altura. O exemplo
//! `identificadores.rs` cria exatamente esse esquema.
//!
//! # O hash cobre o CONTEUDO reserializado, nunca os bytes crus do `.reg`
//!
//! A pétrea da casa: «redige ANALISANDO, nunca recortando». O hash de um bloco
//! e o SHA-256 do seu conteudo canonico -- os VALORES das colunas de dado
//! reserializados por uma regra fixa e documentada, e nao os bytes do slot do
//! `.reg` (que carregam bitmap de nulos, tempero de cifra, ponteiros para o
//! `.bin`/`.memo` e o reaproveitamento de layout). Recortar os bytes crus
//! amarraria o hash a detalhes de armazenamento; reserializar o deixa
//! reproduzivel por qualquer ferramenta de fora do motor.
//!
//! ## Conteudo canonico -- o layout exato, para reproduzir de fora
//!
//! Percorre-se as colunas do esquema NA ORDEM delas. Ficam de FORA: a coluna
//! `hash` (um hash nao cobre a si mesmo), a coluna `assinatura` (se existir --
//! ela assina o hash, entao vem depois) e as colunas de sistema do motor
//! (`softdeleted`, `rownum`), que nao sao dado do bloco. Cada coluna restante
//! contribui um campo, e cada campo comeca com um byte de TIPO seguido do
//! valor em big-endian:
//!
//! ```text
//! Null       -> 0x00
//! Bool       -> 0x01  <1 byte 0|1>
//! Int        -> 0x02  <i64 BE, 8 bytes>
//! UInt       -> 0x03  <u64 BE, 8 bytes>       (a altura Sequence cai aqui)
//! Real       -> 0x04  <f64::to_bits BE, 8 bytes>
//! Decimal    -> 0x05  <i128 BE, 16 bytes>
//! Date       -> 0x06  <i32 BE, 4 bytes>
//! Time       -> 0x07  <i32 BE, 4 bytes>
//! DateTime   -> 0x08  <i64 BE, 8 bytes>
//! Str        -> 0x09  <u64 BE tamanho><bytes UTF-8>
//! Bin        -> 0x0A  <u64 BE tamanho><bytes>
//! Memo       -> 0x0B  <u64 BE tamanho><bytes UTF-8>
//! Uuid       -> 0x0C  <16 bytes>
//! Uuid256    -> 0x0D  <32 bytes>
//! ```
//!
//! O byte de tipo e o tamanho prefixado deixam a serie sem ambiguidade: dois
//! blocos so colidem se o conteudo for o mesmo campo a campo. O SHA-256 disso
//! e devolvido como [`Uuid256`], que existe justamente porque «um SHA-256 cabe
//! exato» (FORMATO.md §13).
//!
//! # Por que a altura entra no hash, e por que ela e' posta ANTES de gravar
//!
//! A altura e parte do conteudo do bloco -- adulterar a altura tem de mudar o
//! hash. Mas a coluna e' `Sequence`, que o motor numera na insercao. Se o hash
//! fosse calculado com a altura ainda nula e o motor a numerasse depois, o hash
//! gravado nao bateria o conteudo lido de volta. Por isso o ledger DERIVA a
//! altura da propria cadeia (topo + 1, genese = [`GENESE_ALTURA`]) e a grava a
//! mao: `Value::UInt` na coluna `Sequence` e usado como veio e empurra o
//! contador (ver `Table::numerar`). Assim o valor gravado e o valor hasheado
//! sao o mesmo.

use crate::table::Table;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::sha256;
use phxsql_core::schema::{e_coluna_de_sistema, Schema};
use phxsql_core::uuid::Uuid256;
use phxsql_core::value::Value;
use phxsql_core::RowId;

/// Coluna que guarda o hash do bloco. Fica de fora do proprio hash.
pub const COL_HASH: &str = "hash";
/// Coluna que liga este bloco ao anterior: guarda o `hash` do bloco de baixo.
pub const COL_ANTERIOR: &str = "anterior";
/// Coluna da altura do bloco (tipo `Sequence`).
pub const COL_ALTURA: &str = "altura";
/// Coluna da assinatura (E5, ainda nao implementada). Fica de fora do hash,
/// porque uma assinatura assina o hash -- entao ela vem DEPOIS dele.
pub const COL_ASSINATURA: &str = "assinatura";
/// Indice unico ascendente sobre `altura`: devolve os blocos na ordem da cadeia.
pub const IDX_POR_ALTURA: &str = "porAltura";
/// Altura do bloco genese. A `Sequence` do motor comeca em 1, e a cadeia segue.
pub const GENESE_ALTURA: u64 = 1;

// --------------------------------------------------------------- E1: o hash

/// Empacota um valor no formato canonico do conteudo (ver o doc do modulo).
fn empacotar_valor(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.push(0x00),
        Value::Bool(b) => {
            out.push(0x01);
            out.push(*b as u8);
        }
        Value::Int(n) => {
            out.push(0x02);
            out.extend_from_slice(&n.to_be_bytes());
        }
        Value::UInt(n) => {
            out.push(0x03);
            out.extend_from_slice(&n.to_be_bytes());
        }
        Value::Real(x) => {
            out.push(0x04);
            out.extend_from_slice(&x.to_bits().to_be_bytes());
        }
        Value::Decimal(n) => {
            out.push(0x05);
            out.extend_from_slice(&n.to_be_bytes());
        }
        Value::Date(n) => {
            out.push(0x06);
            out.extend_from_slice(&n.to_be_bytes());
        }
        Value::Time(n) => {
            out.push(0x07);
            out.extend_from_slice(&n.to_be_bytes());
        }
        Value::DateTime(n) => {
            out.push(0x08);
            out.extend_from_slice(&n.to_be_bytes());
        }
        Value::Str(s) => {
            out.push(0x09);
            empacotar_bytes(s.as_bytes(), out);
        }
        Value::Bin(b) => {
            out.push(0x0A);
            empacotar_bytes(b, out);
        }
        Value::Memo(s) => {
            out.push(0x0B);
            empacotar_bytes(s.as_bytes(), out);
        }
        Value::Uuid(u) => {
            out.push(0x0C);
            out.extend_from_slice(u.bytes());
        }
        Value::Uuid256(u) => {
            out.push(0x0D);
            out.extend_from_slice(u.bytes());
        }
    }
}

/// Tamanho em `u64` big-endian seguido dos bytes: campo variavel sem ambiguidade.
fn empacotar_bytes(b: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&(b.len() as u64).to_be_bytes());
    out.extend_from_slice(b);
}

/// O conteudo canonico de um bloco: os valores das colunas de dado, na ordem
/// do esquema, MENOS `hash`, `assinatura` e as colunas de sistema.
///
/// Serve para qualquer aridade de linha: uma linha montada so com as colunas
/// declaradas (a que vai ao `inserir`) e uma linha lida de volta (que traz as
/// colunas de sistema no fim) produzem o MESMO conteudo, porque tudo o que
/// ficaria alem das declaradas ou e' coluna de sistema (pulada pelo nome) ou
/// nao existe na linha curta (pulado por `i >= linha.len()`).
fn conteudo_canonico(esquema: &Schema, linha: &[Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, col) in esquema.colunas().iter().enumerate() {
        if i >= linha.len() {
            continue;
        }
        if col.nome == COL_HASH || col.nome == COL_ASSINATURA || e_coluna_de_sistema(&col.nome) {
            continue;
        }
        empacotar_valor(&linha[i], &mut out);
    }
    out
}

/// SHA-256 do conteudo canonico de um bloco, devolvido como [`Uuid256`].
///
/// Funcao pura: nao toca disco nem motor. Duas linhas com o mesmo conteudo dao
/// o mesmo hash em qualquer maquina, e mudar um byte de qualquer coluna de dado
/// muda o hash. A coluna `hash` (e a `assinatura`, se houver) sao ignoradas.
pub fn hash_do_bloco(esquema: &Schema, linha: &[Value]) -> Uuid256 {
    Uuid256(sha256(&conteudo_canonico(esquema, linha)))
}

// ------------------------------------------------------ E2: encadeamento

/// As posicoes das tres colunas que a cadeia exige. Erra dizendo qual falta.
fn posicoes(esquema: &Schema) -> Result<(usize, usize, usize)> {
    let acha = |nome: &str| {
        esquema.coluna_por_nome(nome).ok_or_else(|| {
            PhxError::Esquema(format!(
                "o modo ledger exige a coluna `{nome}`, e o esquema de `{}` nao a tem",
                esquema.nome()
            ))
        })
    };
    Ok((acha(COL_HASH)?, acha(COL_ANTERIOR)?, acha(COL_ALTURA)?))
}

fn ler_uuid256(v: &Value) -> Result<Uuid256> {
    match v {
        Value::Uuid256(u) => Ok(*u),
        // A genese grava a NULO nesta coluna; um nulo do banco vale o mesmo.
        Value::Null => Ok(Uuid256::NULO),
        outro => Err(PhxError::Tipo(format!(
            "esperava Uuid256 numa coluna da cadeia, veio {outro:?}"
        ))),
    }
}

fn ler_altura(v: &Value) -> Result<u64> {
    match v {
        Value::UInt(n) => Ok(*n),
        Value::Int(n) if *n >= 0 => Ok(*n as u64),
        outro => Err(PhxError::Tipo(format!(
            "esperava altura inteira, veio {outro:?}"
        ))),
    }
}

/// O topo da cadeia: `(rowid, hash, altura)` do bloco de maior altura, ou
/// `None` se a tabela esta vazia. Le pelo indice `porAltura`.
pub fn topo_da_cadeia(t: &mut Table) -> Result<Option<(RowId, Uuid256, u64)>> {
    let esquema = t.esquema().clone();
    let (i_hash, _i_anterior, i_altura) = posicoes(&esquema)?;
    let ordem = t.varrer_indice(IDX_POR_ALTURA)?;
    let Some(&rid) = ordem.last() else {
        return Ok(None);
    };
    let linha = t.ler(rid)?.ok_or_else(|| {
        PhxError::Corrompido(format!(
            "o indice `{IDX_POR_ALTURA}` aponta para o registro {rid}, que nao se le"
        ))
    })?;
    Ok(Some((
        rid,
        ler_uuid256(&linha[i_hash])?,
        ler_altura(&linha[i_altura])?,
    )))
}

/// Prepara um bloco novo para a cadeia: preenche `anterior` (= hash do topo, ou
/// NULO na genese), `altura` (= altura do topo + 1, ou [`GENESE_ALTURA`]) e
/// calcula `hash` sobre o conteudo canonico ja com esses campos postos. Devolve
/// a linha pronta para `Table::inserir`.
///
/// `valores` deve trazer as colunas declaradas da tabela; as tres colunas da
/// cadeia (`hash`, `anterior`, `altura`) sao SOBRESCRITAS -- o que quer que
/// esteja nelas na entrada e' ignorado.
///
/// Le o topo a cada chamada (uma varredura do `porAltura`). Quem constroi um
/// lote grande de blocos ganha em rastrear o topo por fora e usar
/// [`hash_do_bloco`] direto -- e' o que o `identificadores.rs` faz no laco.
pub fn preparar_bloco(t: &mut Table, mut valores: Vec<Value>) -> Result<Vec<Value>> {
    let esquema = t.esquema().clone();
    let (i_hash, i_anterior, i_altura) = posicoes(&esquema)?;

    for &i in &[i_hash, i_anterior, i_altura] {
        if i >= valores.len() {
            return Err(PhxError::Tipo(format!(
                "a linha tem {} valores, e a coluna da cadeia na posicao {i} nao cabe",
                valores.len()
            )));
        }
    }

    let (anterior, altura) = match topo_da_cadeia(t)? {
        Some((_rid, hash, alt)) => (hash, alt + 1),
        None => (Uuid256::NULO, GENESE_ALTURA),
    };

    valores[i_anterior] = Value::Uuid256(anterior);
    valores[i_altura] = Value::UInt(altura);
    // A coluna `hash` e' ignorada pelo calculo; poe-se NULO so' para nao levar
    // lixo de entrada, e sobrescreve-se com o hash logo abaixo.
    valores[i_hash] = Value::Null;
    let h = hash_do_bloco(&esquema, &valores);
    valores[i_hash] = Value::Uuid256(h);
    Ok(valores)
}

// ---------------------------------------------------- E3: verificacao

/// Qual das provas da cadeia falhou num bloco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prova {
    /// (i) O hash recalculado do conteudo nao bate o `hash` gravado: o conteudo
    /// do bloco foi adulterado depois de gravado.
    Conteudo {
        gravado: Uuid256,
        recalculado: Uuid256,
    },
    /// (ii) O `anterior` deste bloco nao aponta para o `hash` do bloco de baixo:
    /// a ligacao da cadeia foi quebrada.
    Ligacao {
        anterior_gravado: Uuid256,
        hash_esperado: Uuid256,
    },
    /// (iii) A altura pulou: a cadeia tem um buraco (ou um bloco fora de ordem).
    Altura { esperada: u64, veio: u64 },
}

/// O resultado de [`verificar_cadeia`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verificacao {
    /// Cadeia integra: `blocos` blocos, do genese ate `altura_maxima`.
    Integra { blocos: u64, altura_maxima: u64 },
    /// Falhou no bloco de altura `altura` (registro `rowid`), na prova `prova`.
    Falhou {
        altura: u64,
        rowid: RowId,
        prova: Prova,
    },
}

/// Varre a cadeia por `porAltura` e confere as tres provas em cada bloco:
///
/// - **(iii) altura contigua** -- a altura anda de 1 em 1 a partir do genese,
///   sem buraco;
/// - **(ii) ligacao** -- o `anterior` aponta para o `hash` do bloco de baixo
///   (o genese aponta para NULO);
/// - **(i) conteudo** -- o hash recalculado do conteudo bate o `hash` gravado.
///
/// Para na PRIMEIRA falha e devolve ONDE (altura + rowid) e QUAL prova caiu --
/// nao um booleano. Cadeia vazia e' integra com zero blocos.
///
/// A ordem das provas e' a estrutural primeiro (altura, ligacao) e o conteudo
/// por ultimo, que e' a prova cara -- so se recalcula o SHA-256 de um bloco
/// cuja posicao e ligacao ja passaram.
pub fn verificar_cadeia(t: &mut Table) -> Result<Verificacao> {
    let esquema = t.esquema().clone();
    let (i_hash, i_anterior, i_altura) = posicoes(&esquema)?;
    let ordem = t.varrer_indice(IDX_POR_ALTURA)?;

    let mut anterior_esperado = Uuid256::NULO;
    let mut altura_esperada = GENESE_ALTURA;
    let mut blocos = 0u64;
    let mut altura_maxima = 0u64;

    for rid in ordem {
        let linha = t.ler(rid)?.ok_or_else(|| {
            PhxError::Corrompido(format!(
                "o indice `{IDX_POR_ALTURA}` aponta para o registro {rid}, que nao se le"
            ))
        })?;
        let altura = ler_altura(&linha[i_altura])?;
        let hash_gravado = ler_uuid256(&linha[i_hash])?;
        let anterior_gravado = ler_uuid256(&linha[i_anterior])?;

        if altura != altura_esperada {
            return Ok(Verificacao::Falhou {
                altura,
                rowid: rid,
                prova: Prova::Altura {
                    esperada: altura_esperada,
                    veio: altura,
                },
            });
        }
        if anterior_gravado != anterior_esperado {
            return Ok(Verificacao::Falhou {
                altura,
                rowid: rid,
                prova: Prova::Ligacao {
                    anterior_gravado,
                    hash_esperado: anterior_esperado,
                },
            });
        }
        let recalculado = hash_do_bloco(&esquema, &linha);
        if recalculado != hash_gravado {
            return Ok(Verificacao::Falhou {
                altura,
                rowid: rid,
                prova: Prova::Conteudo {
                    gravado: hash_gravado,
                    recalculado,
                },
            });
        }

        anterior_esperado = hash_gravado;
        altura_esperada = altura + 1;
        blocos += 1;
        altura_maxima = altura;
    }

    Ok(Verificacao::Integra {
        blocos,
        altura_maxima,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
    use phxsql_core::types::ColumnType;
    use phxsql_core::uuid::Uuid;

    fn esquema_blocos() -> Schema {
        Schema::new(
            "blocos",
            vec![
                Column::new("id", ColumnType::Uuid).obrigatoria(),
                Column::new("hash", ColumnType::Uuid256).obrigatoria(),
                Column::new("anterior", ColumnType::Uuid256),
                Column::new("altura", ColumnType::Sequence),
                Column::new("autor", ColumnType::Str(60)),
                Column::new("carimbo", ColumnType::DateTime),
            ],
            vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
                IndexDef::new("porHash", vec![IndexColumn::asc(1)]).unico(),
                IndexDef::new("porAltura", vec![IndexColumn::asc(3)]).unico(),
            ],
        )
        .unwrap()
    }

    /// Insere uma cadeia de `quantos` blocos usando o caminho de producao
    /// (`preparar_bloco` + `inserir`).
    fn cadeia(t: &mut Table, quantos: usize) {
        for i in 0..quantos {
            let bruto = vec![
                Value::Uuid(Uuid::v7()),
                Value::Null, // hash: preenchido por preparar_bloco
                Value::Null, // anterior: idem
                Value::Null, // altura: idem
                Value::Str(format!("mineirador-{}", i % 3)),
                Value::DateTime(1_700_000_000_000 + i as i64),
            ];
            let pronto = preparar_bloco(t, bruto).unwrap();
            t.inserir(&pronto).unwrap();
        }
        t.sincronizar().unwrap();
    }

    // ---- E1: o hash cobre o conteudo, ignora a si mesmo, e pega 1 byte

    #[test]
    fn o_hash_cobre_conteudo_ignora_a_propria_coluna_e_pega_um_byte() {
        let esq = esquema_blocos();
        let base = vec![
            Value::Uuid(Uuid::v7()),
            Value::Uuid256(Uuid256::aleatorio()), // hash: deve ser IGNORADO
            Value::Uuid256(Uuid256::NULO),        // anterior
            Value::UInt(1),                       // altura
            Value::Str("mineirador".into()),
            Value::DateTime(1_700_000_000_000),
        ];
        let h = hash_do_bloco(&esq, &base);

        // Determinismo: mesmo conteudo, mesmo hash.
        assert_eq!(h, hash_do_bloco(&esq, &base));

        // A coluna `hash` NAO entra no hash: mudar so' ela nao muda nada.
        let mut so_hash = base.clone();
        so_hash[1] = Value::Uuid256(Uuid256::aleatorio());
        assert_eq!(
            h,
            hash_do_bloco(&esq, &so_hash),
            "o hash cobre o conteudo, nao a si mesmo"
        );

        // Trocar UM byte do conteudo (o autor) muda o hash.
        let mut outro_autor = base.clone();
        outro_autor[4] = Value::Str("mineiradox".into());
        assert_ne!(h, hash_do_bloco(&esq, &outro_autor));

        // ... e mudar o carimbo tambem.
        let mut outro_carimbo = base.clone();
        outro_carimbo[5] = Value::DateTime(1_700_000_000_001);
        assert_ne!(h, hash_do_bloco(&esq, &outro_carimbo));

        // ... e mudar a ligacao (anterior) tambem.
        let mut outro_ant = base.clone();
        outro_ant[2] = Value::Uuid256(Uuid256::aleatorio());
        assert_ne!(h, hash_do_bloco(&esq, &outro_ant));
    }

    /// Prova real da E1 nos dois sentidos: o hash GRAVADO tem de sair do
    /// conteudo, reproduzivel sem o motor. Com o defeito reposto
    /// (`Uuid256::aleatorio()` no lugar do hash), o gravado nao bate o
    /// recalculado e este teste falha; com o conserto, bate.
    #[test]
    fn o_hash_gravado_e_reproduzivel_fora_do_motor() {
        let d = DirTemp::novo("ledger-repro");
        let mut t = Table::criar(&d, esquema_blocos()).unwrap();
        cadeia(&mut t, 3);

        let esq = t.esquema().clone();
        let i_hash = esq.coluna_por_nome("hash").unwrap();
        for rid in t.varrer_indice("porAltura").unwrap() {
            let linha = t.ler(rid).unwrap().unwrap();
            let gravado = match &linha[i_hash] {
                Value::Uuid256(u) => *u,
                outro => panic!("hash gravado nao e Uuid256: {outro:?}"),
            };
            // Recalcula so' com os valores lidos de volta -- sem o motor.
            assert_eq!(
                gravado,
                hash_do_bloco(&esq, &linha),
                "o hash gravado tem de sair do conteudo, nao de aleatorio()"
            );
        }
    }

    // ---- E3: prova real nos dois sentidos, tres casos

    /// (c) Cadeia integra passa, altura contigua.
    #[test]
    fn cadeia_integra_passa() {
        let d = DirTemp::novo("ledger-integra");
        let mut t = Table::criar(&d, esquema_blocos()).unwrap();
        cadeia(&mut t, 5);

        match verificar_cadeia(&mut t).unwrap() {
            Verificacao::Integra {
                blocos,
                altura_maxima,
            } => {
                assert_eq!(blocos, 5, "cinco blocos gravados");
                assert_eq!(altura_maxima, 5, "altura contigua de 1 a 5");
            }
            outro => panic!("cadeia integra devia passar, veio {outro:?}"),
        }
    }

    /// (a) Corromper o conteudo de um bloco antigo -- por um caminho que o
    /// motor ACEITA (o CRC do `.reg` e recalculado no `atualizar`), deixando o
    /// `hash` gravado velho -- e' pego pela prova de CONTEUDO, na altura certa.
    ///
    /// Prova real nos dois sentidos: antes de adulterar, a cadeia passa
    /// (Integra); depois, a verificacao pega a altura 3 com `Prova::Conteudo`.
    /// Se `verificar_cadeia` deixasse de recalcular o hash, este teste falharia.
    #[test]
    fn conteudo_adulterado_e_pego_na_altura_certa() {
        let d = DirTemp::novo("ledger-conteudo");
        let mut t = Table::criar(&d, esquema_blocos()).unwrap();
        cadeia(&mut t, 5);

        assert!(
            matches!(
                verificar_cadeia(&mut t).unwrap(),
                Verificacao::Integra { .. }
            ),
            "a cadeia tinha de estar integra antes de adulterar"
        );

        // Bloco de altura 3 = posicao 2 na ordem do porAltura.
        let rid3 = t.varrer_indice("porAltura").unwrap()[2];
        let mut linha = t.ler(rid3).unwrap().unwrap();
        linha.truncate(6); // so' as colunas declaradas; o motor herda as de sistema
        linha[4] = Value::Str("intruso".into()); // muda o conteudo, NAO o hash
        t.atualizar(rid3, &linha).unwrap();

        match verificar_cadeia(&mut t).unwrap() {
            Verificacao::Falhou {
                altura,
                prova: Prova::Conteudo { .. },
                ..
            } => assert_eq!(altura, 3, "a prova de conteudo tinha de pegar a altura 3"),
            outro => panic!("esperava falha de conteudo na altura 3, veio {outro:?}"),
        }
    }

    /// (b) Corromper conteudo E hash juntos (o atacante reescreve o hash para
    /// bater o novo conteudo) faz o proprio bloco passar na prova de conteudo,
    /// mas QUEBRA A LIGACAO no bloco seguinte -- cujo `anterior` ainda aponta
    /// para o hash antigo. A prova de LIGACAO pega a altura 4.
    #[test]
    fn conteudo_e_hash_juntos_quebram_a_ligacao_no_seguinte() {
        let d = DirTemp::novo("ledger-ligacao");
        let mut t = Table::criar(&d, esquema_blocos()).unwrap();
        cadeia(&mut t, 5);

        let esq = t.esquema().clone();
        let rid3 = t.varrer_indice("porAltura").unwrap()[2];
        let mut linha = t.ler(rid3).unwrap().unwrap();
        linha.truncate(6);
        linha[4] = Value::Str("intruso".into()); // muda o conteudo
        let novo_hash = hash_do_bloco(&esq, &linha); // e recalcula o hash dele
        linha[1] = Value::Uuid256(novo_hash); // ... e grava o hash novo tambem
        t.atualizar(rid3, &linha).unwrap();

        match verificar_cadeia(&mut t).unwrap() {
            Verificacao::Falhou {
                altura,
                prova: Prova::Ligacao { .. },
                ..
            } => assert_eq!(
                altura, 4,
                "o bloco 3 fecha em si, mas o 4 ainda aponta para o hash antigo"
            ),
            outro => panic!("esperava falha de ligacao na altura 4, veio {outro:?}"),
        }
    }

    /// (iii) Um buraco na altura e' pego pela prova de ALTURA. Prova real nos
    /// dois sentidos: um bloco de altura 4 sobre uma cadeia que vai ate 2 (a 3
    /// nunca existiu), ligado corretamente ao topo, passa nas provas de ligacao
    /// e conteudo -- e' SO' a altura que denuncia o buraco. Se o teste de
    /// contiguidade sumisse de `verificar_cadeia`, este teste falharia.
    #[test]
    fn altura_com_buraco_e_pega_pela_prova_de_altura() {
        let d = DirTemp::novo("ledger-buraco");
        let mut t = Table::criar(&d, esquema_blocos()).unwrap();
        cadeia(&mut t, 2); // altura 1 e 2

        let esq = t.esquema().clone();
        let (_rid, hash2, alt2) = topo_da_cadeia(&mut t).unwrap().unwrap();
        assert_eq!(alt2, 2);

        // Bloco de altura 4 -- PULA a 3 --, mas ligado direito ao hash do topo.
        let mut bloco = vec![
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Uuid256(hash2),
            Value::UInt(4),
            Value::Str("furado".into()),
            Value::DateTime(1_700_000_000_500),
        ];
        bloco[1] = Value::Uuid256(hash_do_bloco(&esq, &bloco));
        t.inserir(&bloco).unwrap();
        t.sincronizar().unwrap();

        match verificar_cadeia(&mut t).unwrap() {
            Verificacao::Falhou {
                altura,
                prova: Prova::Altura { esperada, veio },
                ..
            } => assert_eq!((altura, esperada, veio), (4, 3, 4)),
            outro => panic!("esperava buraco de altura, veio {outro:?}"),
        }
    }

    /// Cadeia vazia e' integra com zero blocos -- resposta, nao erro.
    #[test]
    fn cadeia_vazia_e_integra() {
        let d = DirTemp::novo("ledger-vazia");
        let mut t = Table::criar(&d, esquema_blocos()).unwrap();
        assert_eq!(
            verificar_cadeia(&mut t).unwrap(),
            Verificacao::Integra {
                blocos: 0,
                altura_maxima: 0
            }
        );
    }

    /// Um esquema sem as colunas da cadeia recebe um erro que NOMEIA a coluna
    /// que falta, em vez de um curto por indice fora da faixa.
    #[test]
    fn esquema_sem_coluna_da_cadeia_erra_dizendo_qual() {
        let esq = Schema::new(
            "solta",
            vec![Column::new("id", ColumnType::Uuid).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let e = posicoes(&esq).unwrap_err();
        assert!(
            e.to_string().contains(COL_HASH),
            "o erro tinha de nomear a coluna que falta, veio {e}"
        );
    }
}
