//! Traducao entre o JSON do protocolo e os valores do PhxSql.
//!
//! O tipo da coluna e quem manda: o mesmo `42` vira `Int`, `Date` ou
//! `Decimal` conforme a coluna que o recebe.
//!
//! Duas escolhas que valem explicacao:
//!
//! * **Decimal sai como texto**, nunca como numero JSON. `f64` nao representa
//!   1.10 exatamente, e um campo de dinheiro nao pode perder centavo no
//!   caminho de ida e volta.
//! * **Binario sai como hexadecimal**, que atravessa JSON sem escape e e
//!   conferivel a olho nu.

use phxsql_core::datahora::{data_iso, hora_iso};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

// As conversoes de TEXTO moram no nucleo, porque a linha de comando tambem
// carrega arquivo -- e duas implementacoes divergiriam. Aqui so o reexporte.
use phxsql_core::carga::data_de_texto;
pub use phxsql_core::carga::{hex_para_bytes, texto_para_decimal};
use phxsql_core::paginacao::{ModoParticao, Paginacao, Periodo, DIGITOS_PADRAO};
use phxsql_core::schema::Schema;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, IndiceDeTexto};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::uuid::{Uuid, Uuid256};
use phxsql_core::value::Value;

/// Formata um decimal escalado como texto: 1234 com escala 2 vira "12.34".
pub fn decimal_para_texto(valor: i128, escala: u8) -> String {
    phxsql_core::carga::decimal_para_texto(valor, escala)
}

/// Bytes crus em hexadecimal minusculo, para a tela e para o JSON.
pub fn bytes_para_hex(b: &[u8]) -> String {
    // Tabela em vez de `format!("{byte:02x}")`: o `format!` alocava uma String
    // POR BYTE, e a imagem de uma linha tem dezenas deles. Medido em
    // `--example onde-doi-na-replica`: era 3,48 us por evento da replicacao,
    // 14,6% de todo o caminho de CPU dos dois lados.
    const DIG: &[u8; 16] = b"0123456789abcdef";
    let mut s = Vec::with_capacity(b.len() * 2);
    for byte in b {
        s.push(DIG[(byte >> 4) as usize]);
        s.push(DIG[(byte & 0x0f) as usize]);
    }
    // Só saiu de `DIG`, que é ASCII: nao ha como nao ser UTF-8.
    String::from_utf8(s).unwrap_or_default()
}

/// Quantos bytes o tipo ocupa no slot -- o "tamanho" que a tela mostra.
///
/// Para `Str` e o numero de caracteres declarado, que e o que quem escreveu o
/// esquema tem na cabeca. Para `Bin` e `Memo` e zero no slot: o que mora ali e
/// um ponteiro, e o conteudo vive no arquivo externo.
pub fn largura_do_tipo(t: &ColumnType) -> u64 {
    match t {
        ColumnType::Bool | ColumnType::Int1 | ColumnType::UInt1 => 1,
        ColumnType::Int2 | ColumnType::UInt2 => 2,
        ColumnType::Int4 | ColumnType::UInt4 | ColumnType::Real4 => 4,
        ColumnType::Date | ColumnType::Time => 4,
        ColumnType::Int8 | ColumnType::UInt8 | ColumnType::Real8 => 8,
        ColumnType::DateTime | ColumnType::Sequence => 8,
        ColumnType::Uuid => 16,
        ColumnType::Uuid256 => 32,
        ColumnType::Decimal { .. } => 16,
        ColumnType::Str(n) => *n as u64,
        ColumnType::Bin | ColumnType::Memo => 0,
    }
}

/// Le um tipo de coluna escrito em texto.
///
/// Aceita as tres formas que aparecem na pratica, e a razao de aceitar as
/// tres e uma so: o que a operacao `esquema` DEVOLVE tem de poder voltar como
/// entrada. Sem isso, duplicar uma tabela exigiria traduzir o tipo na mao.
///
/// ```text
/// "Int8"                              simples
/// "Str(60)"                           com um parametro
/// "Decimal(15,2)"                     com dois
/// "Decimal { precisao: 15, escala: 2 }"   como o `esquema` escreve hoje
/// ```
pub fn tipo_de_texto(t: &str) -> Result<ColumnType> {
    let t = t.trim();

    // A forma que o `{:?}` do Rust produz para o Decimal.
    if t.starts_with("Decimal") && t.contains("precisao") {
        let numero = |chave: &str| -> Option<u32> {
            let i = t.find(chave)? + chave.len();
            t[i..]
                .trim_start_matches([':', ' '])
                .split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        };
        let (p, e) = (numero("precisao"), numero("escala"));
        return match (p, e) {
            (Some(p), Some(e)) => Ok(ColumnType::Decimal {
                precisao: p as u8,
                escala: e as u8,
            }),
            _ => Err(PhxError::Tipo(format!("Decimal mal escrito: {t:?}"))),
        };
    }

    let (nome, params) = match t.split_once('(') {
        Some((n, resto)) => (
            n.trim(),
            resto
                .trim_end_matches(')')
                .split(',')
                .filter_map(|x| x.trim().parse::<u32>().ok())
                .collect::<Vec<_>>(),
        ),
        None => (t, Vec::new()),
    };
    let p = |i: usize| params.get(i).copied();

    Ok(match nome {
        "Bool" => ColumnType::Bool,
        "Int1" => ColumnType::Int1,
        "Int2" => ColumnType::Int2,
        "Int4" => ColumnType::Int4,
        "Int8" => ColumnType::Int8,
        "UInt1" => ColumnType::UInt1,
        "UInt2" => ColumnType::UInt2,
        "UInt4" => ColumnType::UInt4,
        "UInt8" => ColumnType::UInt8,
        "Real4" => ColumnType::Real4,
        "Real8" => ColumnType::Real8,
        "Date" => ColumnType::Date,
        "Time" => ColumnType::Time,
        "DateTime" => ColumnType::DateTime,
        "Bin" => ColumnType::Bin,
        "Memo" => ColumnType::Memo,
        "Uuid" => ColumnType::Uuid,
        "Uuid256" => ColumnType::Uuid256,
        "Sequence" => ColumnType::Sequence,
        "Str" => ColumnType::Str(p(0).unwrap_or(60).min(65_535) as u16),
        "Decimal" => ColumnType::Decimal {
            precisao: p(0).unwrap_or(15).min(38) as u8,
            escala: p(1).unwrap_or(2).min(38) as u8,
        },
        outro => {
            return Err(PhxError::Tipo(format!(
                "tipo desconhecido: {outro:?}. Use Int8, Str(60), Decimal(15,2), \
                 Date, DateTime, Memo, Bin, Uuid, Uuid256, Sequence…"
            )))
        }
    })
}

/// Uma chave estrangeira, como o pedido a escreve.
///
/// ```json
/// {"nome":"fk_cliente","colunas":["cliente_id"],
///  "tabela_ref":"clientes","colunas_ref":["id"],
///  "ao_excluir":"restringir","ao_alterar":"cascata"}
/// ```
///
/// As colunas LOCAIS vao por nome e viram posicao aqui; as REFERENCIADAS ficam
/// por nome mesmo, porque elas moram na outra tabela e esta funcao nao a abre
/// -- resolver posicao ali exigiria ler a outra tabela na hora de criar esta,
/// e uma FK pode apontar para uma tabela que ainda vai nascer.
pub(crate) fn chave_estrangeira_de_json(
    f: &Json,
    i: usize,
    esquema: &Schema,
) -> Result<ForeignKey> {
    let nome = f.texto_ou("nome", "").trim().to_string();
    if nome.is_empty() {
        return Err(PhxError::Esquema(format!(
            "chave estrangeira {i} sem \"nome\""
        )));
    }
    let locais = f.textos("colunas");
    if locais.is_empty() {
        return Err(PhxError::Esquema(format!("{nome} sem \"colunas\"")));
    }
    let mut posicoes = Vec::with_capacity(locais.len());
    for c in &locais {
        let alvo = c.trim();
        let p = esquema
            .colunas()
            .iter()
            .position(|col| col.nome == alvo)
            .ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{nome} usa a coluna {alvo:?}, que nao existe nesta tabela"
                ))
            })?;
        posicoes.push(p);
    }

    let tabela_ref = f
        .texto_ou("tabela_ref", f.texto_ou("tabela", ""))
        .trim()
        .to_string();
    if tabela_ref.is_empty() {
        return Err(PhxError::Esquema(format!(
            "{nome} nao diz qual tabela referencia (\"tabela_ref\")"
        )));
    }
    // Sem `colunas_ref`, referencia colunas de MESMO NOME na outra tabela. E o
    // caso comum -- `cliente_id` apontando para `clientes.cliente_id` nao e o
    // comum, mas `id` apontando para `id` e --, e escrever a lista duas vezes
    // e onde alguem troca a ordem sem perceber.
    let colunas_ref = match f.textos("colunas_ref") {
        v if v.is_empty() => locais.clone(),
        v => v,
    };

    // `verificar` ausente e VERDADEIRO desde a decisao do dono: «padrao para
    // chave NOVA». A regra primordial diz «nunca se mata o pai que tem filhos»
    // sem condicao, e uma chave que precisa ser LEMBRADA de conferir nao honra
    // um «nunca».
    //
    // Isto NAO quebra banco que ja existe, e o motivo e de formato: o `PSCH`
    // v7 grava o byte por chave, entao o esquema em disco volta com o que foi
    // gravado nele -- chave declarada antes desta decisao continua sem
    // conferir ate alguem ligar. O que muda e o que nasce daqui em diante.
    //
    // Quem QUER declarar sem conferir continua podendo, mandando
    // `"verificar": false` -- e ai e escolha escrita, e nao esquecimento.
    Ok(ForeignKey::new(nome, posicoes, tabela_ref, colunas_ref)
        .ao_excluir(acao_ri_de_texto(
            f.texto_ou("ao_excluir", ""),
            Lado::AoExcluir,
        )?)
        .ao_alterar(acao_ri_de_texto(
            f.texto_ou("ao_alterar", ""),
            Lado::AoAlterar,
        )?)
        .conferindo(f.booleano_ou("verificar", true)))
}

/// Qual das duas acoes esta sendo lida -- e elas NAO aceitam as mesmas coisas.
#[derive(Clone, Copy, PartialEq)]
enum Lado {
    AoExcluir,
    AoAlterar,
}

/// A acao de integridade referencial escrita em texto.
///
/// Aceita o portugues, o SQL e a forma que o `esquema` DEVOLVE (`"Restringir"`)
/// -- pela mesma razao do `tipo_de_texto`: o que a operacao devolve tem de
/// poder voltar como entrada, senao recriar uma tabela exige traduzir na mao.
///
/// # A regra primordial da casa, e por que ela mora AQUI
///
/// Palavra do dono, e ela e do mesmo naipe da ordem de digitacao:
///
/// > *1 para muitos. Cascata ao alterar, Restringir ao excluir, sempre.*
/// > *Nunca pode matar o registro pai se tem filhos em outra tabela.*
/// > *O par Cascata/Cascata nao existe no PhxSql.*
///
/// Entao `ao_excluir` aceita **so** `Restringir`. Nao e teimosia de sintaxe: e
/// a unica forma de a regra ser verdadeira em todo banco criado daqui em
/// diante. Guarda que so vale quando alguem lembra de pedir nao e guarda.
///
/// E a recusa acontece na DECLARACAO, e nao na gravacao, de proposito: uma
/// tabela nasce uma vez e grava um milhao de vezes. Recusar cedo custa um erro
/// que se le enquanto se cria a tabela; recusar tarde custa um banco inteiro
/// modelado errado, descoberto no dia do primeiro `excluir`.
///
/// O par Cascata/Cascata some por CONSEQUENCIA disto, e nao por uma segunda
/// regra: sem cascata no excluir, nao ha par com cascata dos dois lados.
fn acao_ri_de_texto(t: &str, lado: Lado) -> Result<AcaoRi> {
    let acao = match t.trim().to_lowercase().replace([' ', '_'], "").as_str() {
        // Ausente vale o padrao de cada lado, e os padroes sao os da regra.
        "" => {
            return Ok(match lado {
                Lado::AoExcluir => AcaoRi::Restringir,
                Lado::AoAlterar => AcaoRi::Cascata,
            })
        }
        "restringir" | "restrict" => AcaoRi::Restringir,
        "cascata" | "cascade" => AcaoRi::Cascata,
        "anular" | "anularcampos" | "setnull" => AcaoRi::AnularCampos,
        "nada" | "naofazernada" | "noaction" => AcaoRi::NaoFazerNada,
        outro => {
            return Err(PhxError::Esquema(format!(
                "acao de integridade desconhecida: {outro:?} \
                 (use restringir ou cascata)"
            )))
        }
    };
    if lado == Lado::AoExcluir && acao != AcaoRi::Restringir {
        return Err(PhxError::Esquema(format!(
            "\"ao_excluir\": {t:?} nao existe no PhxSql -- ao excluir e sempre \
             \"restringir\". Nunca se mata o registro pai que tem filhos em \
             outra tabela, e por isso o par cascata/cascata tambem nao existe. \
             Para apagar o pai, apague as filhas antes"
        )));
    }
    Ok(acao)
}

/// Monta um `Schema` a partir do JSON de um pedido de criacao de tabela.
///
/// ```json
/// { "tabela": "clientes",
///   "colunas": [ {"nome":"id","tipo":"Int8","obrigatoria":true},
///                {"nome":"nome","tipo":"Str(60)"} ],
///   "indices": [ {"nome":"porId","colunas":["id"],"unico":true} ],
///   "registros_por_arquivo": 1000000 }
/// ```
///
/// As colunas dos indices vao por NOME, nao por posicao. Posicao e detalhe de
/// implementacao e muda quando alguem reordena o esquema; nome nao.
///
/// As chaves estrangeiras entram por `"chaves_estrangeiras"`, e a ausencia do
/// campo e uma lista vazia -- que e o que toda tabela ja criada tem.
/// Uma coluna, lida do JSON do pedido.
///
/// Num lugar so porque dois pedidos a montam -- o `criar_tabela` e o
/// `acrescentar_coluna` --, e uma segunda copia deste bloco seria a que
/// esqueceria o campo novo. `i` entra so na mensagem de erro.
pub fn coluna_de_json(c: &Json, i: usize) -> Result<Column> {
    let cn = c.texto_ou("nome", "").trim().to_string();
    if cn.is_empty() {
        return Err(PhxError::Esquema(format!("coluna {i} sem nome")));
    }
    let ty = tipo_de_texto(c.texto_ou("tipo", "Str(60)"))?;
    let mut col = Column::new(cn, ty)
        .com_caption(c.texto_ou("caption", ""))
        .com_descricao(c.texto_ou("descricao", ""))
        .com_mascara(c.texto_ou("mascara", ""))
        // Ausente = `Nao`, que e o que toda tabela ja criada tem. Cliente
        // escrito antes desta versao continua criando tabela igual.
        .com_dado_pessoal(DadoPessoal::de_texto(c.texto_ou("dado_pessoal", ""))?);
    // O `id` normalmente nasce aqui, sorteado. Aceitar um de fora existe
    // para UM caso: recriar uma tabela mantendo a identidade das colunas,
    // para que telas e relatorios que apontam para elas continuem valendo.
    let id = c.texto_ou("id", "").trim().to_string();
    if !id.is_empty() {
        col = col.com_id(
            Uuid::de_texto(&id).map_err(|e| PhxError::Esquema(format!("id da coluna {i}: {e}")))?,
        );
    }
    if c.booleano_ou("obrigatoria", false) {
        col = col.obrigatoria();
    }
    // As regras de escrita (v9 do PSCH). Cada uma e uma expressao em texto;
    // a recusa por sintaxe e por coluna inexistente acontece na DECLARACAO,
    // dentro de `Schema::new`, e nao na primeira gravacao.
    col = col
        .com_padrao(&expressao_de_json(c, "padrao"))?
        .com_check(&expressao_de_json(c, "check"))?
        .com_calculada(&expressao_de_json(c, "calculada"))?;
    Ok(col)
}

/// O texto de uma expressao de esquema vinda do pedido. Texto e a expressao
/// como esta; numero e booleano viram o literal correspondente -- quem manda
/// `"padrao": 7` quer o sete, e obriga-lo a escrever `"7"` seria formalismo
/// que so serve para recusar pedido certo. Ausente e nulo sao «nao tem».
fn expressao_de_json(c: &Json, campo: &str) -> String {
    match c.campo(campo) {
        None => String::new(),
        Some(Json::Texto(t)) => t.clone(),
        Some(Json::Bool(b)) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        Some(j) if j.e_nulo() => String::new(),
        // O numero sai pelo escritor de JSON, que imprime inteiro sem casas
        // e fracionario com as casas que tem -- o mesmo texto que chegou.
        Some(j) => j.escrever(),
    }
}

pub fn esquema_de_json(j: &Json) -> Result<Schema> {
    let nome = j.texto_ou("tabela", "").trim().to_string();
    if nome.is_empty() {
        return Err(PhxError::Esquema("informe \"tabela\"".into()));
    }

    let cols_json = j
        .campo("colunas")
        .and_then(Json::lista)
        .ok_or_else(|| PhxError::Esquema("informe \"colunas\" como lista".into()))?;
    if cols_json.is_empty() {
        return Err(PhxError::Esquema(
            "a tabela precisa de ao menos uma coluna".into(),
        ));
    }

    let mut colunas = Vec::with_capacity(cols_json.len());
    for (i, c) in cols_json.iter().enumerate() {
        colunas.push(coluna_de_json(c, i)?);
    }

    let posicao = |nome: &str| -> Result<usize> {
        colunas
            .iter()
            .position(|c| c.nome == nome)
            .ok_or_else(|| PhxError::Esquema(format!("indice usa coluna inexistente: {nome:?}")))
    };

    let mut indices = Vec::new();
    if let Some(lista) = j.campo("indices").and_then(Json::lista) {
        for (i, idx) in lista.iter().enumerate() {
            let inome = idx.texto_ou("nome", "").trim().to_string();
            if inome.is_empty() {
                return Err(PhxError::Esquema(format!("indice {i} sem nome")));
            }
            let mut partes = Vec::new();
            let mut expressoes = Vec::new();
            for c in idx.textos("colunas") {
                // Uma coluna do indice ou e a forma de MARCAS (`cidade desc`,
                // `total nocase` -- nome da coluna e, opcional, desc/nocase),
                // ou e uma EXPRESSAO de uma coluna, cuja chave e o RESULTADO
                // coagido ao tipo da coluna: `lower(nome)` (funcao) e
                // `salario * 2` (operador aritmetico -- `+ - * /`, que o
                // contrato promete e o avaliador do core ja rende).
                //
                // A escolha e ESTRUTURAL, e nao "expressao quando aparece um
                // operador em qualquer lugar do texto": so os tokens DEPOIS do
                // primeiro sao varridos por operador. Por isso `minha-col desc`
                // continua marca -- o hifen esta no NOME da coluna, nao entre
                // dois termos --, enquanto `salario * 2` vira expressao. E um
                // token estranho SEM operador (`cidade descc`) segue caindo na
                // recusa que ensina desc/nocase, em vez de virar um erro de
                // sintaxe de expressao, que ensina menos. Antes do 243/A11 so a
                // forma com parentese virava expressao, e `salario * 2` recebia
                // "use desc ou nocase" -- recusa que mentia sobre o que o
                // contrato aceita.
                let e_expressao = c.contains('(') || {
                    let mut it = c.split_whitespace();
                    let primeiro = it.next().unwrap_or("");
                    let primeiro_e_coluna = colunas
                        .iter()
                        .any(|col| col.nome.eq_ignore_ascii_case(primeiro));
                    // Primeiro token que nem coluna e (`2 + x`, `salario*2`
                    // colado) e comeco de expressao, nao de marca; primeiro
                    // token que E coluna vira expressao so quando um token
                    // seguinte traz operador aritmetico (uma marca nunca tem).
                    !primeiro_e_coluna
                        || it.any(|m| m.chars().any(|ch| matches!(ch, '+' | '-' | '*' | '/')))
                };
                if e_expressao {
                    let e = phxsql_core::expressao::Expressao::analisar(&c)
                        .map_err(|erro| PhxError::Esquema(format!("indice {inome}: {erro}")))?;
                    let [unica] = e.colunas() else {
                        return Err(PhxError::Esquema(format!(
                            "indice {inome}: a expressao {c:?} tem de usar exatamente uma \
                             coluna, e usa {}",
                            e.colunas().len()
                        )));
                    };
                    partes.push(IndexColumn::asc(posicao(unica).or_else(|_| {
                        colunas
                            .iter()
                            .position(|col| col.nome.eq_ignore_ascii_case(unica))
                            .ok_or_else(|| {
                                PhxError::Esquema(format!(
                                    "indice usa coluna inexistente: {unica:?}"
                                ))
                            })
                    })?));
                    expressoes.push(Some(c.to_string()));
                    continue;
                }
                // A forma de MARCAS: "cidade desc" e "cidade nocase" no proprio
                // nome da coluna -- como se escreve um indice em uma linha.
                let mut it = c.split_whitespace();
                let cn = it.next().unwrap_or("").to_string();
                let mut ic = IndexColumn::asc(posicao(&cn)?);
                for marca in it {
                    match marca.to_ascii_lowercase().as_str() {
                        "desc" => ic.desc = true,
                        "nocase" => ic.nocase = true,
                        outro => {
                            return Err(PhxError::Esquema(format!(
                                "marca desconhecida no indice {inome}: {outro:?} \
                                 (use desc ou nocase)"
                            )))
                        }
                    }
                }
                partes.push(ic);
                expressoes.push(None);
            }
            if partes.is_empty() {
                return Err(PhxError::Esquema(format!("indice {inome} sem colunas")));
            }
            let mut d = IndexDef::new(inome, partes);
            for (k, e) in expressoes.iter().enumerate() {
                if let Some(e) = e {
                    d = d.com_expressao(k, e)?;
                }
            }
            // O indice PARCIAL: a linha so entra quando o `onde` da verdadeiro.
            d = d.com_onde(idx.texto_ou("onde", ""))?;
            if idx.booleano_ou("unico", false) {
                d = d.unico();
            }
            // Primaria implica unica; o `primaria()` cuida disso.
            if idx.booleano_ou("primario", false) || idx.booleano_ou("primaria", false) {
                d = d.primaria();
            }
            indices.push(d);
        }
    }

    // A tela de criar tabela marca "exigir motivo": a partir dai nenhuma
    // exclusao nesta tabela passa sem uma frase escrita.
    let esquema = Schema::new(nome, colunas, indices)?
        .com_motivo_obrigatorio(j.booleano_ou("motivo_obrigatorio", false));

    // As chaves estrangeiras.
    //
    // O formato as suporta e o `esquema` as reporta desde sempre -- mas
    // NENHUMA operacao do protocolo as criava: so dava para declarar uma pela
    // API Rust. Uma lista de pendencias dizia "chave estrangeira: pronto", e
    // era meia verdade.
    //
    // Ausente e uma lista vazia, que e o que toda tabela ja criada tem: cliente
    // escrito antes desta versao continua criando tabela exatamente igual.
    let esquema = match j
        .campo("chaves_estrangeiras")
        .or_else(|| j.campo("fks"))
        .and_then(Json::lista)
    {
        None => esquema,
        Some(lista) => {
            let mut fks = Vec::with_capacity(lista.len());
            for (i, f) in lista.iter().enumerate() {
                fks.push(chave_estrangeira_de_json(f, i, &esquema)?);
            }
            esquema.com_chaves_estrangeiras(fks)?
        }
    };

    // O INDICE DE TEXTO, e ele repete letra por letra a historia da chave
    // estrangeira logo acima: o formato o suporta desde o `PSCH v8`, o
    // `store` o usa, a bancada mede 31.399x com ele -- e NENHUMA operacao do
    // protocolo o criava. So dava para declarar pela API Rust, por um
    // `examples/` que a bancada chama.
    //
    // Pior que faltar: em 07/09/2026 medi que `criar_tabela` aceitava
    // `indices_texto`, `indices_de_texto`, `fts` e `texto` com `ok: true` e
    // ENGOLIA os quatro. A tabela nascia sem indice nenhum, e quem descobria
    // era o `procurar_texto` depois, dizendo que o indice nao existe. Campo
    // aceito e ignorado e pior que campo recusado: o recusado ninguem acha
    // que funcionou.
    //
    // Ausente e lista vazia, como nas FKs: cliente escrito antes desta versao
    // continua criando tabela exatamente igual.
    let esquema = match j
        .campo("indices_texto")
        .or_else(|| j.campo("indices_de_texto"))
        .and_then(Json::lista)
    {
        None => esquema,
        Some(lista) => {
            let mut textos = Vec::with_capacity(lista.len());
            for (i, it) in lista.iter().enumerate() {
                let nome = it.texto_ou("nome", "").trim().to_string();
                if nome.is_empty() {
                    return Err(PhxError::Esquema(format!("indice de texto {i} sem nome")));
                }
                let coluna = it.texto_ou("coluna", "").trim().to_string();
                if coluna.is_empty() {
                    return Err(PhxError::Esquema(format!(
                        "indice de texto {nome} sem \"coluna\""
                    )));
                }
                let pos = esquema
                    .colunas()
                    .iter()
                    .position(|c| c.nome == coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "indice de texto {nome} usa coluna inexistente: {coluna:?}"
                        ))
                    })?;
                textos.push(IndiceDeTexto {
                    nome,
                    coluna: pos,
                    // Nasce LIGADO, e o motivo esta medido no `docs/FTS.md`
                    // 5.1: a busca de hoje nao dobra acento, entao um indice
                    // sem dobra acharia MENOS que a varredura -- e indice que
                    // acha menos que a varredura e pior que nao ter indice.
                    dobrar: it.booleano_ou("dobrar", true),
                });
            }
            esquema.com_indices_de_texto(textos)?
        }
    };

    // A paginacao entra na criacao e nao muda depois: ela decide como o rowid
    // vira endereco. Trocar mais tarde seria reescrever a tabela inteira.
    let por_arquivo = j.inteiro_ou("registros_por_arquivo", 0);
    Ok(if por_arquivo > 0 {
        let digitos = j.inteiro_ou("digitos", DIGITOS_PADRAO as i64).clamp(1, 9) as u8;
        // Teto omitido nao quer dizer "sem teto": o sufixo tem largura fixa, e
        // com tres digitos o volume 1000 simplesmente nao tem nome. Entao o
        // padrao e o maior que cabe no sufixo, e nao zero -- que o validador
        // recusaria com uma mensagem que nao ajuda quem preencheu a tela.
        // A particao por periodo aponta a coluna por NOME, como os indices --
        // posicao e detalhe de implementacao.
        let modo = match j.texto_ou("particao", "").trim() {
            "" | "quantidade" | "faixa" => ModoParticao::PorQuantidade,
            "letra" | "alfanumerica" | "alfanumerico" => {
                let coluna = j.texto_ou("particao_coluna", "").trim().to_string();
                let i = esquema
                    .colunas()
                    .iter()
                    .position(|c| c.nome == coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a particao alfanumerica precisa de \"particao_coluna\" \
                             com o nome da coluna de referencia; recebi {coluna:?}"
                        ))
                    })?;
                ModoParticao::PorLetra { coluna: i as u16 }
            }
            nome_periodo => {
                let coluna = j.texto_ou("particao_coluna", "").trim().to_string();
                let i = esquema
                    .colunas()
                    .iter()
                    .position(|c| c.nome == coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a particao {nome_periodo} precisa de \"particao_coluna\" \
                             com o nome de uma coluna de data; recebi {coluna:?}"
                        ))
                    })?;
                ModoParticao::PorPeriodo {
                    coluna: i as u16,
                    periodo: Periodo::de_nome(nome_periodo)?,
                }
            }
        };

        // Na alfanumerica o numero de volumes NAO se escolhe: sao os 37
        // baldes, e o construtor cuida do sufixo. Deixar a tela mandar um teto
        // aqui so criaria um jeito de pedir uma tabela que o validador recusa.
        if let ModoParticao::PorLetra { coluna } = modo {
            return esquema.com_paginacao(Paginacao::por_letra(por_arquivo as u64, coluna)?);
        }

        let cabem = 10u32.pow(digitos as u32) - 1;
        let max = match j.inteiro_ou("max_arquivos", 0).max(0) as u32 {
            0 => cabem,
            outro => outro,
        };
        // A largura do sufixo entra ANTES do teto: `nova` confere o teto contra
        // os tres digitos do padrao, e um teto de 9999 seria recusado antes de
        // o quarto digito existir.
        esquema.com_paginacao(
            Paginacao::nova(por_arquivo as u64, 1)?
                .com_digitos(digitos)?
                .com_max_arquivos(max)?
                .com_modo(modo)?,
        )?
    } else {
        esquema
    })
}

/// Valor do PhxSql em JSON, usando o tipo da coluna para dar sentido aos
/// inteiros de data, hora e decimal.
pub fn valor_para_json(v: &Value, ty: &ColumnType) -> Json {
    match (v, ty) {
        (Value::Null, _) => Json::Nulo,
        (Value::Bool(b), _) => Json::Bool(*b),
        (Value::Date(d), _) => Json::texto_de(data_iso(*d)),
        (Value::Time(c), _) => Json::texto_de(hora_iso(*c)),
        (Value::DateTime(ms), _) => Json::texto_de(phxsql_core::datahora::instante_iso(*ms)),
        (Value::Decimal(n), ColumnType::Decimal { escala, .. }) => {
            Json::texto_de(decimal_para_texto(*n, *escala))
        }
        (Value::Decimal(n), _) => Json::texto_de(n.to_string()),
        (Value::Int(n), _) => Json::de_i64(*n),
        // Um id (`Sequence`) acima de 2^53 nao cabe num `f64` sem perda: sair
        // como `Json::de_u64` volta trocado no fio (bloco 19). Acima do teto
        // ele sai como TEXTO, que e a unica forma honesta -- e a mesma que o
        // `json_para_valor` aceita de volta sem perda. Abaixo do teto continua
        // numero, exatamente como antes, para nao trocar o tipo do id comum
        // debaixo de quem le a grade. Vale so para a `Sequence`; o `UInt8`
        // partilha o teto e esta anotado em `docs/AUTONUMBER.md` como irmao.
        (Value::UInt(n), ColumnType::Sequence) if *n > phxsql_core::json::INTEIRO_EXATO_MAX => {
            Json::texto_de(n.to_string())
        }
        (Value::UInt(n), _) => Json::de_u64(*n),
        (Value::Real(n), _) => Json::Numero(*n),
        (Value::Str(s), _) | (Value::Memo(s), _) => Json::texto_de(s),
        (Value::Bin(b), _) => Json::texto_de(bytes_para_hex(b)),
        // Sempre na forma canonica minuscula: e o que o RFC manda escrever, e
        // manda o mesmo texto para a grade, para o log e para quem consome a
        // API. Um id que se escreve de dois jeitos vira dois ids no olho de
        // quem le.
        (Value::Uuid(u), _) => Json::texto_de(u.to_string()),
        (Value::Uuid256(u), _) => Json::texto_de(u.to_string()),
    }
}

/// JSON em valor do PhxSql, guiado pelo tipo da coluna.
pub fn json_para_valor(j: &Json, ty: &ColumnType) -> Result<Value> {
    if j.e_nulo() {
        return Ok(Value::Null);
    }
    let erro = |esperado: &str| PhxError::Tipo(format!("esperado {esperado}, recebido {j:?}"));

    // Inteiro escrito como TEXTO tambem serve.
    //
    // O `Decimal` desta mesma funcao ja EXIGE texto, para nao perder centavo
    // num `f64` -- e pela mesma razao o tradutor de SQL guarda todo literal
    // numerico como texto. Sem esta linha, `WHERE id = 2` chegaria como
    // `["2"]` e seria recusado por tipo, e o SELECT mais simples que existe
    // nao funcionaria contra uma coluna `Int4`. Vale tambem para o driver
    // ODBC e para o protocolo do PostgreSQL(R), onde TODO parametro chega
    // como texto.
    //
    // E so alargar: quem manda numero continua igual, e texto que nao e
    // numero continua recusado com o mesmo erro de tipo.
    let inteiro = || -> Option<i64> {
        match j {
            Json::Texto(t) => t.trim().parse::<i64>().ok(),
            outro => outro.inteiro(),
        }
    };

    Ok(match ty {
        ColumnType::Bool => match j {
            Json::Bool(b) => Value::Bool(*b),
            Json::Numero(n) => Value::Bool(*n != 0.0),
            _ => return Err(erro("booleano")),
        },
        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8 => {
            Value::Int(inteiro().ok_or_else(|| erro("inteiro"))?)
        }
        ColumnType::UInt1 | ColumnType::UInt2 | ColumnType::UInt4 | ColumnType::UInt8 => {
            let n = inteiro().ok_or_else(|| erro("inteiro sem sinal"))?;
            if n < 0 {
                return Err(PhxError::Tipo(format!(
                    "{n} e negativo numa coluna sem sinal"
                )));
            }
            Value::UInt(n as u64)
        }
        ColumnType::Real4 | ColumnType::Real8 => {
            // Real escrito como TEXTO tambem serve -- o mesmo alargamento do
            // inteiro acima, pelos mesmos clientes (ODBC e o protocolo do
            // PostgreSQL(R) mandam TODO parametro como texto) e agora pelos
            // gatilhos, que escrevem numero fracionario como texto porque a
            // conta deles nao passa por f64. Quem manda numero continua
            // exatamente como antes; texto que nao e numero continua
            // recusado com o mesmo erro.
            let n = match j {
                Json::Texto(t) => t.trim().parse::<f64>().ok(),
                outro => outro.numero(),
            };
            Value::Real(n.ok_or_else(|| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => {
            match j {
                Json::Texto(t) => Value::Decimal(texto_para_decimal(t, *escala)?),
                Json::Numero(_) => return Err(PhxError::Tipo(
                    "decimal precisa vir como texto (\"12.34\"), para nao perder centavo em f64"
                        .into(),
                )),
                _ => return Err(erro("decimal em texto")),
            }
        }
        // O id chega em texto, que e como ele viaja em JSON. Aceita-se
        // tambem a palavra "novo": e o pedido de "gere um para mim" sem que o
        // cliente precise saber como se monta um v7.
        ColumnType::Uuid => match j {
            Json::Texto(t) if t.eq_ignore_ascii_case("novo") || t.eq_ignore_ascii_case("v7") => {
                Value::Uuid(Uuid::v7())
            }
            Json::Texto(t) if t.eq_ignore_ascii_case("v4") => Value::Uuid(Uuid::v4()),
            Json::Texto(t) => Value::Uuid(Uuid::de_texto(t)?),
            _ => return Err(erro("UUID em texto")),
        },
        ColumnType::Uuid256 => match j {
            Json::Texto(t) if t.eq_ignore_ascii_case("novo") => {
                Value::Uuid256(Uuid256::aleatorio())
            }
            Json::Texto(t) => Value::Uuid256(Uuid256::de_texto(t)?),
            _ => return Err(erro("identificador de 256 bits em texto")),
        },
        // Nulo ja saiu no comeco da funcao: chegar aqui e o cliente tendo
        // escolhido o numero a mao, e a tabela empurra o contador para depois
        // dele.
        //
        // `inteiro()` (o fechamento acima, nao o metodo de `Json`) e o mesmo
        // alargamento do `UInt` -- e tem de ser: o tradutor de SQL guarda TODO
        // literal numerico como texto (`literal_para_json` em traduzir.rs), e
        // `Sequence` e o tipo da chave primaria de quase toda tabela nascida
        // pela tela. Usar `j.inteiro()` direto (que so aceita `Json::Numero`)
        // recusava `WHERE id = 2` com "esperado numero da sequencia, recebido
        // Texto(\"2\")" enquanto o irmao `Int8`/`UInt8` passava por aqui ao
        // lado sem problema -- pedido 223.
        ColumnType::Sequence => {
            // O teto REAL de uma `Sequence` pelo protocolo nao e o do formato
            // (2^64-1): e 2^53, o teto do `f64` do `Json` desta casa. Um numero
            // CRU acima dele ja chegou aqui trocado -- a perda aconteceu no
            // `Json::analisar`, antes desta funcao ver o valor. Recusar e o que
            // transforma corrupcao silenciosa (bloco 19) em erro lido. O mesmo
            // numero como TEXTO atravessa intacto (nao passa por f64), entao a
            // saida esta na propria mensagem. Ver `docs/AUTONUMBER.md`.
            if j.inteiro_impreciso() {
                return Err(PhxError::Tipo(format!(
                    "acima de {} o protocolo perde precisao num numero cru; \
                     envie o id como texto (\"{}\") ou use Uuid256 se precisar \
                     dessa faixa",
                    phxsql_core::json::INTEIRO_EXATO_MAX,
                    // O texto do f64 ja arredondado -- proposital: mostra o
                    // valor que SERIA gravado, para o cliente ver que nao era o
                    // que ele mandou.
                    j.inteiro().map(|v| v.to_string()).unwrap_or_default()
                )));
            }
            let n = inteiro().ok_or_else(|| erro("numero da sequencia"))?;
            if n < 0 {
                return Err(PhxError::Tipo(format!("{n} e negativo numa sequencia")));
            }
            Value::UInt(n as u64)
        }
        ColumnType::Date => match j {
            Json::Texto(t) => Value::Date(data_de_texto(t)?),
            Json::Numero(_) => Value::Date(j.inteiro().ok_or_else(|| erro("data"))? as i32),
            _ => return Err(erro("data")),
        },
        ColumnType::Time => Value::Time(j.inteiro().ok_or_else(|| erro("hora"))? as i32),
        ColumnType::DateTime => Value::DateTime(j.inteiro().ok_or_else(|| erro("data e hora"))?),
        ColumnType::Str(_) => Value::Str(j.texto().ok_or_else(|| erro("texto"))?.to_string()),
        ColumnType::Memo => Value::Memo(j.texto().ok_or_else(|| erro("texto"))?.to_string()),
        ColumnType::Bin => match j {
            Json::Texto(t) => Value::Bin(hex_para_bytes(t)?),
            Json::Lista(l) => Value::Bin(
                l.iter()
                    .map(|x| {
                        x.inteiro()
                            .filter(|n| (0..=255).contains(n))
                            .map(|n| n as u8)
                            .ok_or_else(|| PhxError::Tipo("byte fora de 0..255".into()))
                    })
                    .collect::<Result<Vec<u8>>>()?,
            ),
            _ => return Err(erro("binario em hexadecimal")),
        },
    })
}

/// Linha inteira em JSON, como objeto com o nome de cada coluna.
/// As colunas de um esquema, no minimo que uma grade precisa para desenhar.
///
/// Marca a coluna de sistema com `"sistema": true` para a tela poder trata-la
/// como o que ela e: nao se digita, nao se edita, e quem manda nela e o botao
/// de excluir. Sem essa marca, ela apareceria como mais um campo de formulario.
pub fn colunas_para_json(esquema: &Schema) -> Json {
    Json::Lista(
        esquema
            .colunas()
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    (
                        "sistema",
                        Json::Bool(phxsql_core::schema::e_coluna_de_sistema(&c.nome)),
                    ),
                ])
            })
            .collect(),
    )
}

pub fn linha_para_json(linha: &[Value], esquema: &Schema) -> Json {
    Json::Objeto(
        esquema
            .colunas()
            .iter()
            .zip(linha.iter())
            .map(|(c, v)| (c.nome.clone(), valor_para_json(v, &c.ty)))
            .collect(),
    )
}

/// Aceita a linha como objeto (por nome de coluna) ou como lista (na ordem do
/// esquema). Colunas ausentes no objeto entram como NULL.
pub fn json_para_linha(j: &Json, esquema: &Schema) -> Result<Vec<Value>> {
    let colunas = esquema.colunas();
    // A coluna de sistema pode ficar de fora do que chega pela rede: quem
    // manda a linha declarou as colunas dele e nao tem por que saber dela.
    // Falta ela na lista -> entra `false` no fim; falta no objeto -> idem.
    // Sem isso, `inserir` recusaria toda linha de todo cliente que existe
    // hoje, porque a coluna e obrigatoria e o ausente vira nulo.
    let sistema = esquema.coluna_softdeleted();
    let padrao_de = |i: usize| -> Value {
        if Some(i) == sistema {
            Value::Bool(false)
        } else {
            Value::Null
        }
    };
    match j {
        Json::Lista(itens) => {
            let curta = sistema.is_some_and(|i| itens.len() == i);
            if itens.len() != colunas.len() && !curta {
                return Err(PhxError::Tipo(format!(
                    "a lista tem {} valores, a tabela tem {} colunas",
                    itens.len(),
                    colunas.len()
                )));
            }
            colunas
                .iter()
                .enumerate()
                .map(|(i, c)| match itens.get(i) {
                    Some(v) => json_para_valor(v, &c.ty),
                    None => Ok(padrao_de(i)),
                })
                .collect()
        }
        Json::Objeto(pares) => {
            for (chave, _) in pares {
                if esquema.coluna_por_nome(chave).is_none() {
                    return Err(PhxError::Tipo(format!(
                        "coluna {chave:?} nao existe em {}",
                        esquema.nome()
                    )));
                }
            }
            colunas
                .iter()
                .enumerate()
                .map(|(i, c)| match j.campo(&c.nome) {
                    Some(v) => json_para_valor(v, &c.ty),
                    None => Ok(padrao_de(i)),
                })
                .collect()
        }
        _ => Err(PhxError::Tipo(
            "a linha precisa ser um objeto ou uma lista".into(),
        )),
    }
}

/// Chave de indice: lista de valores na ordem das colunas do indice.
pub fn json_para_chave(j: &Json, esquema: &Schema, indice: usize) -> Result<Vec<Value>> {
    let def = esquema
        .indices()
        .get(indice)
        .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} inexistente")))?;
    let itens = match j {
        Json::Lista(l) => l.clone(),
        outro => vec![outro.clone()],
    };
    if itens.len() != def.colunas.len() {
        return Err(PhxError::Tipo(format!(
            "o indice {} tem {} colunas, a chave veio com {}",
            def.nome,
            def.colunas.len(),
            itens.len()
        )));
    }
    itens
        .iter()
        .zip(def.colunas.iter())
        .map(|(v, ic)| json_para_valor(v, &esquema.colunas()[ic.coluna].ty))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_vai_e_volta_sem_perder_centavo() {
        for (valor, escala, texto) in [
            (150_000i128, 2u8, "1500.00"),
            (-1_234, 2, "-12.34"),
            (5, 2, "0.05"),
            (0, 2, "0.00"),
            (42, 0, "42"),
            (999_999_999_999_999, 4, "99999999999.9999"),
        ] {
            assert_eq!(decimal_para_texto(valor, escala), texto, "escala {escala}");
            assert_eq!(texto_para_decimal(texto, escala).unwrap(), valor);
        }
    }

    #[test]
    fn decimal_recusa_numero_json() {
        let ty = ColumnType::Decimal {
            precisao: 15,
            escala: 2,
        };
        let e = json_para_valor(&Json::Numero(12.34), &ty).unwrap_err();
        assert!(format!("{e}").contains("texto"), "erro foi {e}");
        assert_eq!(
            json_para_valor(&Json::texto_de("12.34"), &ty).unwrap(),
            Value::Decimal(1_234)
        );
    }

    #[test]
    fn decimal_recusa_casas_demais() {
        assert!(texto_para_decimal("1.234", 2).is_err());
        assert!(texto_para_decimal("abc", 2).is_err());
        assert!(texto_para_decimal("", 2).is_err());
        assert_eq!(texto_para_decimal("1.2", 2).unwrap(), 120);
        assert_eq!(texto_para_decimal("+7", 2).unwrap(), 700);
    }

    #[test]
    fn hexadecimal_vai_e_volta() {
        let b = vec![0u8, 1, 15, 16, 254, 255];
        let h = bytes_para_hex(&b);
        assert_eq!(h, "00010f10feff");
        assert_eq!(hex_para_bytes(&h).unwrap(), b);
        assert!(hex_para_bytes("abc").is_err());
        assert!(hex_para_bytes("zz").is_err());
    }

    #[test]
    fn data_aceita_iso_e_numero() {
        let ty = ColumnType::Date;
        assert_eq!(
            json_para_valor(&Json::texto_de("2024-10-04"), &ty).unwrap(),
            Value::Date(20_000)
        );
        assert_eq!(
            json_para_valor(&Json::Numero(20_000.0), &ty).unwrap(),
            Value::Date(20_000)
        );
        assert_eq!(
            valor_para_json(&Value::Date(20_000), &ty),
            Json::texto_de("2024-10-04")
        );
        assert!(json_para_valor(&Json::texto_de("04/10/2024"), &ty).is_err());
        assert!(json_para_valor(&Json::texto_de("2024-13-01"), &ty).is_err());
    }

    fn esquema() -> Schema {
        use phxsql_core::schema::{Column, IndexColumn, IndexDef};
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)),
                Column::new(
                    "limite",
                    ColumnType::Decimal {
                        precisao: 15,
                        escala: 2,
                    },
                ),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    #[test]
    fn linha_como_objeto_ou_lista() {
        let esq = esquema();
        let por_objeto = Json::analisar(r#"{"id":7,"nome":"Ana","limite":"10.50"}"#).unwrap();
        let por_lista = Json::analisar(r#"[7,"Ana","10.50"]"#).unwrap();
        let a = json_para_linha(&por_objeto, &esq).unwrap();
        let b = json_para_linha(&por_lista, &esq).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0], Value::Int(7));
        assert_eq!(a[2], Value::Decimal(1_050));

        // A volta preserva os nomes e o decimal em texto.
        let volta = linha_para_json(&a, &esq);
        assert_eq!(volta.texto_ou("nome", ""), "Ana");
        assert_eq!(volta.campo("limite").unwrap().texto().unwrap(), "10.50");
    }

    #[test]
    fn coluna_ausente_no_objeto_vira_null() {
        let esq = esquema();
        let j = Json::analisar(r#"{"id":1}"#).unwrap();
        let linha = json_para_linha(&j, &esq).unwrap();
        assert_eq!(linha[1], Value::Null);
        assert_eq!(linha[2], Value::Null);
    }

    #[test]
    fn coluna_inventada_e_recusada() {
        let esq = esquema();
        let j = Json::analisar(r#"{"id":1,"inexistente":2}"#).unwrap();
        assert!(json_para_linha(&j, &esq).is_err());
    }

    #[test]
    fn lista_com_tamanho_errado_e_recusada() {
        let esq = esquema();
        assert!(json_para_linha(&Json::analisar("[1,2]").unwrap(), &esq).is_err());
    }

    #[test]
    fn chave_de_indice_aceita_escalar_e_lista() {
        let esq = esquema();
        let a = json_para_chave(&Json::Numero(7.0), &esq, 0).unwrap();
        let b = json_para_chave(&Json::analisar("[7]").unwrap(), &esq, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0], Value::Int(7));
        assert!(json_para_chave(&Json::analisar("[7,8]").unwrap(), &esq, 0).is_err());
    }

    #[test]
    fn uuid_vai_e_volta_em_texto_canonico() {
        let ty = ColumnType::Uuid;
        let texto = "017f22e2-79b0-7cc3-98c4-dc0c0c07398f";
        let v = json_para_valor(&Json::texto_de(texto), &ty).unwrap();
        assert_eq!(valor_para_json(&v, &ty), Json::texto_de(texto));

        // MAIUSCULAS entram, mas saem na forma canonica: um id que se escreve
        // de dois jeitos vira dois ids no olho de quem le.
        let v = json_para_valor(&Json::texto_de(texto.to_uppercase()), &ty).unwrap();
        assert_eq!(valor_para_json(&v, &ty), Json::texto_de(texto));
    }

    #[test]
    fn a_palavra_novo_gera_um_v7() {
        let ty = ColumnType::Uuid;
        let a = json_para_valor(&Json::texto_de("novo"), &ty).unwrap();
        let b = json_para_valor(&Json::texto_de("novo"), &ty).unwrap();
        match (&a, &b) {
            (Value::Uuid(x), Value::Uuid(y)) => {
                assert_eq!(x.versao(), 7);
                assert!(y > x, "dois pedidos seguidos tem de crescer");
            }
            outro => panic!("esperado Uuid, veio {outro:?}"),
        }
        // "v4" pede a versao sem relogio.
        match json_para_valor(&Json::texto_de("v4"), &ty).unwrap() {
            Value::Uuid(u) => assert_eq!(u.versao(), 4),
            outro => panic!("esperado Uuid v4, veio {outro:?}"),
        }
    }

    #[test]
    fn uuid_torto_no_json_e_recusado() {
        let ty = ColumnType::Uuid;
        assert!(json_para_valor(&Json::texto_de("nao-e-uuid"), &ty).is_err());
        assert!(json_para_valor(&Json::de_i64(42), &ty).is_err());
    }

    #[test]
    fn hash_de_256_bits_aceita_o_prefixo_0x() {
        let ty = ColumnType::Uuid256;
        let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let a = json_para_valor(&Json::texto_de(hex), &ty).unwrap();
        let b = json_para_valor(&Json::texto_de(format!("0x{hex}")), &ty).unwrap();
        assert_eq!(a, b, "o 0x mudou o valor");
        assert_eq!(valor_para_json(&a, &ty), Json::texto_de(hex));
    }

    #[test]
    fn sequencia_recusa_negativo() {
        let ty = ColumnType::Sequence;
        assert_eq!(
            json_para_valor(&Json::de_i64(7), &ty).unwrap(),
            Value::UInt(7)
        );
        assert!(json_para_valor(&Json::de_i64(-1), &ty).is_err());
        // Nulo e o pedido de "numere voce": nao e erro.
        assert_eq!(json_para_valor(&Json::Nulo, &ty).unwrap(), Value::Null);
    }
}

#[cfg(test)]
mod testes_esquema {
    use super::*;

    fn json(t: &str) -> Json {
        Json::analisar(t).expect("json de teste invalido")
    }

    const COLS: &str = r#""colunas":[{"nome":"id","tipo":"Int8"},
                                     {"nome":"txt","tipo":"Str(80)"}]"#;

    /// O indice de texto DECLARADO pelo protocolo nasce no esquema.
    ///
    /// Antes de 07/09/2026 o `criar_tabela` aceitava `indices_texto` com
    /// `ok: true` e ENGOLIA o campo: a tabela nascia sem indice nenhum, e
    /// quem descobria era o `procurar_texto` depois. Campo aceito e ignorado
    /// e pior que campo recusado -- o recusado ninguem acha que funcionou.
    #[test]
    fn indice_de_texto_declarado_pelo_protocolo_nasce_no_esquema() {
        let e = esquema_de_json(&json(&format!(
            r#"{{"tabela":"t",{COLS},
                 "indices_texto":[{{"nome":"ft","coluna":"txt"}}]}}"#
        )))
        .expect("devia criar");
        assert_eq!(e.indices_de_texto().len(), 1);
        assert_eq!(e.indices_de_texto()[0].nome, "ft");
        // A coluna vira POSICAO: `txt` e a segunda.
        assert_eq!(e.indices_de_texto()[0].coluna, 1);
    }

    /// A dobra de acento NASCE LIGADA, e e o inverso de «guarda nova entra
    /// pedida» de proposito: `docs/FTS.md` 5.1 mediu que a busca sem dobra
    /// acha MENOS que a varredura (0 de 200 para «fenix» contra «fenix»), e
    /// indice que acha menos que a varredura e pior que nao ter indice.
    #[test]
    fn a_dobra_de_acento_nasce_ligada_e_quem_nao_quer_desliga() {
        let liga = esquema_de_json(&json(&format!(
            r#"{{"tabela":"t",{COLS},"indices_texto":[{{"nome":"ft","coluna":"txt"}}]}}"#
        )))
        .expect("devia criar");
        assert!(liga.indices_de_texto()[0].dobrar, "devia nascer ligada");

        let desliga = esquema_de_json(&json(&format!(
            r#"{{"tabela":"t",{COLS},
                 "indices_texto":[{{"nome":"ft","coluna":"txt","dobrar":false}}]}}"#
        )))
        .expect("devia criar");
        assert!(
            !desliga.indices_de_texto()[0].dobrar,
            "quem desliga, desliga"
        );
    }

    /// O TESTE DO COMPORTAMENTO VELHO, e e o que mais importa aqui: quem nao
    /// manda `indices_texto` cria tabela exatamente como criava antes. Guarda
    /// nova que muda o que ja existe nao e protecao, e estrago.
    #[test]
    fn tabela_sem_indice_de_texto_continua_nascendo_igual() {
        let e =
            esquema_de_json(&json(&format!(r#"{{"tabela":"t",{COLS}}}"#))).expect("devia criar");
        assert!(e.indices_de_texto().is_empty());
    }

    /// Coluna inexistente RECUSA na declaracao, e nao na gravacao -- a mesma
    /// decisao da chave estrangeira: a tabela nasce uma vez e grava um milhao
    /// de vezes, entao recusar cedo custa um erro lido enquanto se cria.
    #[test]
    fn indice_de_texto_sobre_coluna_que_nao_existe_recusa_na_declaracao() {
        let e = esquema_de_json(&json(&format!(
            r#"{{"tabela":"t",{COLS},
                 "indices_texto":[{{"nome":"ft","coluna":"nao_existe"}}]}}"#
        )));
        let erro = e.expect_err("devia recusar").to_string();
        assert!(
            erro.contains("nao_existe"),
            "a recusa nomeia a coluna: {erro}"
        );
    }

    /// Sem nome, e sem coluna: as duas recusas nomeiam o que falta em vez de
    /// deixarem o indice nascer torto.
    #[test]
    fn indice_de_texto_sem_nome_ou_sem_coluna_recusa() {
        for (corpo, pedaco) in [
            (r#"{"coluna":"txt"}"#, "sem nome"),
            (r#"{"nome":"ft"}"#, "coluna"),
        ] {
            let e = esquema_de_json(&json(&format!(
                r#"{{"tabela":"t",{COLS},"indices_texto":[{corpo}]}}"#
            )));
            let erro = e.expect_err("devia recusar").to_string();
            assert!(erro.contains(pedaco), "esperava {pedaco:?} em {erro:?}");
        }
    }

    /// A propriedade que importa: o que a operacao `esquema` DEVOLVE tem de
    /// voltar como entrada. Sem isso, duplicar uma tabela pela tela exigiria
    /// traduzir cada tipo na mao -- e um `Decimal` traduzido errado perde
    /// centavo em silencio.
    #[test]
    fn tipo_volta_da_forma_que_o_esquema_escreve() {
        for ty in [
            ColumnType::Bool,
            ColumnType::Int1,
            ColumnType::Int8,
            ColumnType::UInt4,
            ColumnType::Real8,
            ColumnType::Date,
            ColumnType::Time,
            ColumnType::DateTime,
            ColumnType::Bin,
            ColumnType::Memo,
            ColumnType::Uuid,
            ColumnType::Uuid256,
            ColumnType::Sequence,
            ColumnType::Str(60),
            ColumnType::Str(1),
            ColumnType::Decimal {
                precisao: 15,
                escala: 2,
            },
            ColumnType::Decimal {
                precisao: 38,
                escala: 0,
            },
        ] {
            let escrito = format!("{ty:?}");
            assert_eq!(
                tipo_de_texto(&escrito).unwrap(),
                ty,
                "nao voltou de {escrito:?}"
            );
        }
    }

    #[test]
    fn tipo_tambem_aceita_a_forma_curta_de_quem_digita() {
        assert_eq!(tipo_de_texto("Str(80)").unwrap(), ColumnType::Str(80));
        assert_eq!(
            tipo_de_texto("Decimal(15,2)").unwrap(),
            ColumnType::Decimal {
                precisao: 15,
                escala: 2
            }
        );
        assert_eq!(
            tipo_de_texto(" Int8 ").unwrap(),
            ColumnType::Int8,
            "espaco em volta nao pode derrubar"
        );
        assert!(tipo_de_texto("Varchar").is_err());
    }

    #[test]
    fn esquema_le_colunas_e_indices_por_nome() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"pedidos",
                "colunas":[{"nome":"id","tipo":"Sequence","obrigatoria":true},
                           {"nome":"cidade","tipo":"Str(40)"},
                           {"nome":"total","tipo":"Decimal(15,2)"}],
                "indices":[{"nome":"porId","colunas":["id"],"unico":true},
                           {"nome":"porCidade","colunas":["cidade nocase","total desc"]}]}"#,
        ))
        .unwrap();

        assert_eq!(e.nome(), "pedidos");
        // Tres declaradas mais as DUAS de sistema, que entram sozinhas no fim.
        assert_eq!(e.colunas().len(), 5);
        assert_eq!(e.coluna_softdeleted(), Some(3));
        assert_eq!(e.coluna_rownum(), Some(4));
        assert!(!e.colunas()[0].nullable, "obrigatoria virou nullable");
        assert!(e.colunas()[1].nullable);

        let i = &e.indices()[1];
        assert_eq!(i.nome, "porCidade");
        // A posicao sai do NOME: cidade e a coluna 1, total a 2.
        assert_eq!(i.colunas[0].coluna, 1);
        assert!(i.colunas[0].nocase && !i.colunas[0].desc);
        assert_eq!(i.colunas[1].coluna, 2);
        assert!(i.colunas[1].desc && !i.colunas[1].nocase);
        assert!(e.indices()[0].unico);
    }

    #[test]
    fn teto_de_volumes_omitido_vira_o_que_cabe_no_sufixo() {
        // Zero nao e "sem teto": o sufixo tem largura fixa, e com tres digitos
        // o volume 1000 nao teria nome de arquivo.
        let e = esquema_de_json(&json(
            r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Int8"}],
                "registros_por_arquivo":1000}"#,
        ))
        .unwrap();
        let p = e.paginacao();
        assert_eq!(p.digitos, 3);
        assert_eq!(p.max_arquivos, 999);
        assert_eq!(p.capacidade(), 999_000);

        let e = esquema_de_json(&json(
            r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Int8"}],
                "registros_por_arquivo":1000,"digitos":4}"#,
        ))
        .unwrap();
        assert_eq!(e.paginacao().max_arquivos, 9_999);
    }

    #[test]
    fn sem_registros_por_arquivo_a_tabela_e_arquivo_unico() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Int8"}]}"#,
        ))
        .unwrap();
        assert!(!e.paginacao().ligada());
    }

    #[test]
    fn o_que_falta_no_pedido_vira_erro_com_nome() {
        let casos = [
            (r#"{"colunas":[{"nome":"a","tipo":"Int8"}]}"#, "tabela"),
            (r#"{"tabela":"t"}"#, "colunas"),
            (r#"{"tabela":"t","colunas":[]}"#, "coluna"),
            (r#"{"tabela":"t","colunas":[{"tipo":"Int8"}]}"#, "nome"),
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Int8"}],
                    "indices":[{"nome":"i","colunas":["naoexiste"]}]}"#,
                "inexistente",
            ),
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Int8"}],
                    "indices":[{"nome":"i","colunas":["a crescente"]}]}"#,
                "desc ou nocase",
            ),
        ];
        for (pedido, pedaco) in casos {
            let erro = esquema_de_json(&json(pedido)).unwrap_err().to_string();
            assert!(
                erro.contains(pedaco),
                "erro {erro:?} nao diz o que falta ({pedaco:?})"
            );
        }
    }

    /// **A PROVA REAL do 243 (A11): o indice por expressao com OPERADOR nasce
    /// como expressao, e nao cai na recusa de marca.** `salario * 2` usa uma
    /// coluna e vira a chave do indice; antes desta rodada so a forma com
    /// parentese virava expressao, e o operador batia em "use desc ou nocase",
    /// contrariando o `+ - * /` que o contrato promete.
    ///
    /// Sabotagem: voltar o teste da linha para `if c.contains('(')` manda
    /// `salario * 2` ao laco de marcas, que bate no token "*" e recusa com
    /// "use desc ou nocase" -- e a primeira assercao (`expect`) reprova.
    #[test]
    fn indice_por_expressao_com_operador_nasce_como_expressao() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"folha",
                "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                           {"nome":"salario","tipo":"Int8"}],
                "indices":[{"nome":"porDobro","colunas":["salario * 2"]}]}"#,
        ))
        .expect("o indice por expressao com operador foi recusado");

        let i = &e.indices()[0];
        assert_eq!(i.nome, "porDobro");
        // A IndexColumn aponta para a UNICA coluna que a expressao usa
        // (salario, posicao 1), e a chave e o RESULTADO -- guardado no texto
        // da expressao, nao como marca crua.
        assert_eq!(i.colunas[0].coluna, 1);
        assert!(
            i.expressoes[0].is_some(),
            "o operador nao virou expressao -- caiu como marca crua"
        );

        // As funcoes ja aceitas continuam funcionando: `lower(nome)`.
        let e = esquema_de_json(&json(
            r#"{"tabela":"c",
                "colunas":[{"nome":"nome","tipo":"Str(20)"}],
                "indices":[{"nome":"porNome","colunas":["lower(nome)"]}]}"#,
        ))
        .expect("lower(nome) parou de funcionar");
        assert!(e.indices()[0].expressoes[0].is_some());

        // E o hifen no NOME da coluna nao vira subtracao: `salario-bruto desc`
        // e marca (desc), porque so os tokens DEPOIS do primeiro sao varridos
        // por operador. Sem isto, uma coluna com hifen perderia o indice.
        let e = esquema_de_json(&json(
            r#"{"tabela":"h",
                "colunas":[{"nome":"salario-bruto","tipo":"Int8"}],
                "indices":[{"nome":"porHifen","colunas":["salario-bruto desc"]}]}"#,
        ))
        .expect("a coluna com hifen parou de indexar por marca");
        let i = &e.indices()[0];
        assert!(i.colunas[0].desc, "o desc da coluna com hifen se perdeu");
        assert!(
            i.expressoes[0].is_none(),
            "a marca com hifen no nome virou expressao"
        );
    }
}

#[cfg(test)]
mod testes_metadados {
    use super::*;

    fn json(t: &str) -> Json {
        Json::analisar(t).expect("json de teste invalido")
    }

    #[test]
    fn os_metadados_do_campo_chegam_no_esquema() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"lancamentos",
                "colunas":[{"nome":"emissao","tipo":"Date","obrigatoria":true,
                            "caption":"Emissão","descricao":"Data do lançamento",
                            "mascara":"@D6"}]}"#,
        ))
        .unwrap();
        let c = &e.colunas()[0];
        assert_eq!(c.nome, "emissao");
        assert_eq!(c.caption, "Emissão");
        assert_eq!(c.descricao, "Data do lançamento");
        assert_eq!(c.mascara, "@D6");
        // Cada coluna nasce com um id proprio, sorteado.
        assert_ne!(c.id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    /// **O teste do cliente velho.** Um `criar_tabela` escrito antes desta
    /// versao nao manda `dado_pessoal` -- e tem de criar a tabela igual, com
    /// nenhuma coluna classificada. Marca que entra sozinha e marca errada.
    #[test]
    fn sem_dado_pessoal_no_pedido_nada_muda() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"clientes",
                "colunas":[{"nome":"nome","tipo":"Str(60)"},
                           {"nome":"cpf","tipo":"Str(11)"},
                           {"nome":"tipo_sanguineo","tipo":"Str(3)"}]}"#,
        ))
        .unwrap();
        assert!(
            !e.tem_dado_pessoal(),
            "o motor classificou coluna que ninguem pediu"
        );
        assert!(e.colunas_pessoais().is_empty());
    }

    #[test]
    fn o_grau_de_dado_pessoal_chega_no_esquema() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"pacientes",
                "colunas":[{"nome":"id","tipo":"Int8"},
                           {"nome":"nome","tipo":"Str(60)","dado_pessoal":"pessoal"},
                           {"nome":"laudo","tipo":"Memo","dado_pessoal":"sensivel"}]}"#,
        ))
        .unwrap();
        assert_eq!(e.colunas()[0].dado_pessoal, DadoPessoal::Nao);
        assert_eq!(e.colunas()[1].dado_pessoal, DadoPessoal::Pessoal);
        assert_eq!(e.colunas()[2].dado_pessoal, DadoPessoal::Sensivel);
        assert_eq!(e.colunas_pessoais().len(), 2);
    }

    #[test]
    fn grau_escrito_errado_recusa_em_vez_de_ignorar() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"t","colunas":[{"nome":"x","tipo":"Int8",
                                         "dado_pessoal":"talvez"}]}"#,
        ))
        .unwrap_err();
        // Aceitar calado deixaria a coluna sem marca e o operador achando que
        // marcou -- que e pior do que recusar.
        assert!(format!("{e}").contains("talvez"), "{e}");
    }

    #[test]
    fn coluna_sem_caption_usa_o_nome_como_rotulo() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"t","colunas":[{"nome":"cidade","tipo":"Str(40)"}]}"#,
        ))
        .unwrap();
        assert_eq!(e.colunas()[0].rotulo(), "cidade");
    }

    #[test]
    fn a_chave_primaria_marca_as_colunas_dela() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"pedidos",
                "colunas":[{"nome":"filial","tipo":"Int4","obrigatoria":true},
                           {"nome":"numero","tipo":"Int8","obrigatoria":true},
                           {"nome":"cliente","tipo":"Str(40)"}],
                "indices":[{"nome":"porFilialNumero","colunas":["filial","numero"],
                            "primario":true}]}"#,
        ))
        .unwrap();

        // Primaria implica unica, mesmo sem "unico" no pedido.
        let pk = e.chave_primaria().expect("devia ter chave primaria");
        assert!(pk.unico, "primaria tem de ser unica");
        assert!(pk.composta());

        for i in 0..2 {
            let p = e.papel_da_coluna(i);
            assert!(p.primaria, "coluna {i} devia estar na chave primaria");
            assert!(p.primaria_composta, "a chave tem duas colunas");
        }
        assert!(!e.papel_da_coluna(2).primaria, "cliente esta fora da chave");
    }

    #[test]
    fn coluna_de_chave_primaria_nao_pode_aceitar_nulo() {
        // Uma identidade nula nao identifica: e erro de esquema, nao gosto.
        let erro = esquema_de_json(&json(
            r#"{"tabela":"t","colunas":[{"nome":"id","tipo":"Int8"}],
                "indices":[{"nome":"pk","colunas":["id"],"primario":true}]}"#,
        ))
        .unwrap_err()
        .to_string();
        assert!(
            erro.contains("chave primaria") && erro.contains("nulo"),
            "{erro}"
        );
    }

    #[test]
    fn duas_chaves_primarias_e_erro() {
        let erro = esquema_de_json(&json(
            r#"{"tabela":"t",
                "colunas":[{"nome":"a","tipo":"Int8","obrigatoria":true},
                           {"nome":"b","tipo":"Int8","obrigatoria":true}],
                "indices":[{"nome":"k1","colunas":["a"],"primario":true},
                           {"nome":"k2","colunas":["b"],"primario":true}]}"#,
        ))
        .unwrap_err()
        .to_string();
        assert!(erro.contains("chaves primarias"), "{erro}");
    }

    #[test]
    fn particao_por_periodo_le_a_coluna_pelo_nome() {
        let e = esquema_de_json(&json(
            r#"{"tabela":"lancamentos",
                "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                           {"nome":"emissao","tipo":"Date","obrigatoria":true}],
                "registros_por_arquivo":1000,
                "particao":"bimestral","particao_coluna":"emissao"}"#,
        ))
        .unwrap();
        let m = e.paginacao().modo;
        assert_eq!(m.periodo().map(|p| p.nome()), Some("bimestral"));
        assert_eq!(m.coluna(), Some(1), "emissao e a coluna 1");
    }

    #[test]
    fn particao_por_periodo_recusa_o_que_nao_pode_dar_certo() {
        let casos = [
            // Sem a coluna.
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Date","obrigatoria":true}],
                    "registros_por_arquivo":10,"particao":"mensal"}"#,
                "particao_coluna",
            ),
            // Coluna que nao existe.
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Date","obrigatoria":true}],
                    "registros_por_arquivo":10,"particao":"mensal","particao_coluna":"zzz"}"#,
                "particao_coluna",
            ),
            // Coluna que nao e data.
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Int8","obrigatoria":true}],
                    "registros_por_arquivo":10,"particao":"mensal","particao_coluna":"a"}"#,
                "coluna de data",
            ),
            // Data que aceita nulo: sem data nao ha periodo.
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Date"}],
                    "registros_por_arquivo":10,"particao":"mensal","particao_coluna":"a"}"#,
                "nulo",
            ),
            // Periodo que nao existe.
            (
                r#"{"tabela":"t","colunas":[{"nome":"a","tipo":"Date","obrigatoria":true}],
                    "registros_por_arquivo":10,"particao":"quinzenal","particao_coluna":"a"}"#,
                "periodo desconhecido",
            ),
        ];
        for (pedido, pedaco) in casos {
            let erro = esquema_de_json(&json(pedido)).unwrap_err().to_string();
            assert!(
                erro.contains(pedaco),
                "erro {erro:?} nao menciona {pedaco:?}"
            );
        }
    }
}

/// Inteiro escrito como texto.
///
/// O tradutor de SQL guarda todo literal numerico como texto -- pelo mesmo
/// motivo que o `Decimal` daqui EXIGE texto: `f64` nao representa `1500.00`
/// exatamente. Sem aceitar o texto, `WHERE id = 2` nao funcionaria contra uma
/// coluna `Int4`, e o SELECT mais simples que existe morreria por tipo.
#[cfg(test)]
mod testes_inteiro_em_texto {
    use super::*;

    /// **O teste do comportamento VELHO, que e o que mais importa.** Alargar
    /// nao pode mudar nada de quem ja mandava numero.
    #[test]
    fn numero_continua_valendo_exatamente_como_antes() {
        assert_eq!(
            json_para_valor(&Json::de_i64(42), &ColumnType::Int4).unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            json_para_valor(&Json::de_u64(42), &ColumnType::UInt4).unwrap(),
            Value::UInt(42)
        );
        assert_eq!(
            json_para_valor(&Json::Nulo, &ColumnType::Int4).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn texto_com_numero_dentro_tambem_serve() {
        assert_eq!(
            json_para_valor(&Json::texto_de("42"), &ColumnType::Int4).unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            json_para_valor(&Json::texto_de(" -7 "), &ColumnType::Int8).unwrap(),
            Value::Int(-7)
        );
        assert_eq!(
            json_para_valor(&Json::texto_de("42"), &ColumnType::UInt2).unwrap(),
            Value::UInt(42)
        );
    }

    /// E o que NAO e numero continua recusado, com o mesmo erro de tipo:
    /// alargar nao pode virar engolir.
    #[test]
    fn texto_que_nao_e_numero_continua_recusado() {
        let e = json_para_valor(&Json::texto_de("abc"), &ColumnType::Int4).unwrap_err();
        assert_eq!(e.nome(), "TIPO_INVALIDO", "{e}");
        assert!(e.to_string().contains("inteiro"), "{e}");

        // Negativo em coluna sem sinal continua recusado dizendo por que.
        let e = json_para_valor(&Json::texto_de("-1"), &ColumnType::UInt4).unwrap_err();
        assert!(e.to_string().contains("negativo"), "{e}");

        // E o decimal continua exigindo texto e recusando numero -- esta
        // regra nao foi tocada.
        let e = json_para_valor(
            &Json::Numero(12.34),
            &ColumnType::Decimal {
                precisao: 10,
                escala: 2,
            },
        )
        .unwrap_err();
        assert!(e.to_string().contains("centavo"), "{e}");
    }

    /// O Real alarga como o inteiro alargou: texto numerico serve, numero
    /// continua igual, texto torto continua recusado. Quem precisa disso e o
    /// gatilho (a conta dele nao passa por f64, entao fracionario sai como
    /// texto) e todo cliente que so fala texto — ODBC, fio do PostgreSQL(R).
    #[test]
    fn real_aceita_texto_como_o_inteiro_ja_aceita() {
        assert_eq!(
            json_para_valor(&Json::texto_de("1.5"), &ColumnType::Real8).unwrap(),
            Value::Real(1.5)
        );
        assert_eq!(
            json_para_valor(&Json::Numero(1.5), &ColumnType::Real8).unwrap(),
            Value::Real(1.5),
            "quem manda numero continua exatamente como antes"
        );
        let e = json_para_valor(&Json::texto_de("abc"), &ColumnType::Real4).unwrap_err();
        assert_eq!(e.nome(), "TIPO_INVALIDO", "{e}");
    }

    /// Pedido 223: `Sequence` tem o MESMO alargamento do `Int`/`UInt` acima, e
    /// nao um proprio -- e por isso que os quatro casos abaixo espelham os de
    /// `Int4`/`UInt4` linha por linha. `Sequence` e o tipo da chave primaria
    /// de quase toda tabela nascida pela tela, e o tradutor de SQL manda TODO
    /// literal numerico como texto (`literal_para_json`): sem este alargamento,
    /// `WHERE id = 2` recusava com "esperado numero da sequencia, recebido
    /// Texto(\"2\")" enquanto a MESMA consulta contra uma coluna `Int8`/`UInt8`
    /// passava ao lado, sem erro.
    #[test]
    fn sequence_aceita_texto_como_o_uint_ja_aceita() {
        // O caso que reproduz o pedido 223 ao pe da letra: "2" como o
        // tradutor de SQL manda, contra uma coluna Sequence.
        assert_eq!(
            json_para_valor(&Json::texto_de("2"), &ColumnType::Sequence).unwrap(),
            Value::UInt(2)
        );
        // Numero direto continua valendo -- e o caminho que ja funcionava.
        assert_eq!(
            json_para_valor(&Json::de_u64(7), &ColumnType::Sequence).unwrap(),
            Value::UInt(7)
        );
        // Nulo continua nulo -- a tabela empurra o contador para depois dele.
        assert_eq!(
            json_para_valor(&Json::Nulo, &ColumnType::Sequence).unwrap(),
            Value::Null
        );
        // Negativo em texto continua recusado, dizendo por que -- alargar nao
        // pode virar engolir.
        let e = json_para_valor(&Json::texto_de("-1"), &ColumnType::Sequence).unwrap_err();
        assert!(e.to_string().contains("negativo"), "{e}");
        // Controle negativo: texto que NAO e numero continua recusado com o
        // mesmo erro de tipo -- e o `Str` da tabela irma nunca deveria aceitar
        // numero sem aspas, sequencia ou nao.
        let e = json_para_valor(&Json::texto_de("abc"), &ColumnType::Sequence).unwrap_err();
        assert_eq!(e.nome(), "TIPO_INVALIDO", "{e}");
        assert!(e.to_string().contains("numero da sequencia"), "{e}");
    }

    /// Defeito (b): id acima de 2^53 mandado como NUMERO CRU perdia precisao
    /// em silencio. Prova real nos dois sentidos.
    ///
    /// Reponha o defeito tirando o `if j.inteiro_impreciso()` do ramo
    /// `Sequence`: o valor volta a ser aceito como `Value::UInt(9007199254740992)`
    /// -- o `2^53+1` mandado vira `2^53` gravado -- e este teste cai na
    /// primeira asserção. Ver `docs/AUTONUMBER.md`, bloco 19.
    #[test]
    fn sequencia_recusa_numero_cru_acima_do_teto_do_f64() {
        // Como o cliente manda pelo fio: `Json::analisar` de um numero cru, que
        // e onde o f64 ja arredonda `2^53+1` para `2^53`.
        let cru = Json::analisar("9007199254740993").unwrap();
        let e = json_para_valor(&cru, &ColumnType::Sequence).unwrap_err();
        assert_eq!(e.nome(), "TIPO_INVALIDO", "{e}");
        assert!(e.to_string().contains("perde precisao"), "{e}");
        // A mensagem aponta a saida: mandar como texto.
        assert!(e.to_string().contains("texto"), "{e}");

        // 2^53 exato tambem cai, e de proposito: um f64 que le 2^53 tanto pode
        // ser o proprio quanto um `2^53+1` arredondado, e aqui sao a mesma
        // coisa -- indistinguiveis.
        let no_teto = Json::analisar("9007199254740992").unwrap();
        assert!(json_para_valor(&no_teto, &ColumnType::Sequence).is_err());

        // Logo abaixo do teto continua passando, exatamente como antes.
        let abaixo = Json::analisar("9007199254740991").unwrap();
        assert_eq!(
            json_para_valor(&abaixo, &ColumnType::Sequence).unwrap(),
            Value::UInt(9_007_199_254_740_991)
        );
    }

    /// A saida que a recusa aponta funciona: o MESMO numero grande como TEXTO
    /// atravessa sem perda -- porque texto nao passa por f64. E o `Uuid256`
    /// continua sendo a alternativa para faixas ainda maiores.
    #[test]
    fn sequencia_grande_como_texto_atravessa_intacta() {
        assert_eq!(
            json_para_valor(&Json::texto_de("9007199254740993"), &ColumnType::Sequence).unwrap(),
            Value::UInt(9_007_199_254_740_993),
            "o 2^53+1 exato so sobrevive como texto"
        );
        // Bem acima do teto, tambem exato, ainda como texto.
        assert_eq!(
            json_para_valor(&Json::texto_de("3000000000000000"), &ColumnType::Sequence).unwrap(),
            Value::UInt(3_000_000_000_000_000)
        );
    }

    /// E o lado de VOLTA: um id acima do teto gravado no `.reg` (ele chega la
    /// por replicacao ou por `ajustar_sequencia`, que nao passam por este
    /// crivo) sai do servidor como TEXTO, nao como numero mentiroso.
    ///
    /// Reponha o defeito tirando o ramo `(Value::UInt(n), ColumnType::Sequence)`
    /// de `valor_para_json`: ele volta a `Json::de_u64`, o f64 arredonda, e a
    /// asserção do texto exato cai.
    #[test]
    fn sequencia_grande_sai_como_texto_no_json() {
        assert_eq!(
            valor_para_json(&Value::UInt(9_007_199_254_740_993), &ColumnType::Sequence),
            Json::texto_de("9007199254740993"),
        );
        // Abaixo do teto continua numero -- o id comum nao troca de tipo.
        assert_eq!(
            valor_para_json(&Value::UInt(42), &ColumnType::Sequence),
            Json::de_u64(42),
        );
        // 2^53 exato ainda cabe no f64, entao sai numero (o corte e `> 2^53`
        // na saida, porque aqui o valor JA e exato e nao ha ambiguidade).
        assert_eq!(
            valor_para_json(&Value::UInt(9_007_199_254_740_992), &ColumnType::Sequence),
            Json::de_u64(9_007_199_254_740_992),
        );
        // E um `UInt8` comum acima do teto continua numero: o irmao fica, e a
        // decisao de alcanca-lo esta anotada em docs/AUTONUMBER.md.
        assert!(matches!(
            valor_para_json(&Value::UInt(9_007_199_254_740_993), &ColumnType::UInt8),
            Json::Numero(_)
        ));
    }
}
