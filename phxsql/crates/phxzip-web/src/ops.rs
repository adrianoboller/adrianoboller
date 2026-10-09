//! As rotas `/api/*` (contrato §2 a §6, fatias Z6 a Z8): o envelope `PZW1`,
//! o estado, os idiomas e as quatro operacoes sobre o motor `phxzip`.
//!
//! Nada aqui abre, le ou decide formato: abrir e o `Arquivo::abrir`, escrever
//! e o `Escritor`/`empacotar`, o tar e o `EscritorTar`, o nome e o
//! `conferir_nome`, o disco e o `disco::Destino`. O que mora aqui e so o
//! que e da WEB -- o envelope, os campos da cabeca, os cabecalhos de resposta
//! e o nome de erro do contrato.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::Path;

use phxsql_core::idiomas::IDIOMAS;
use phxsql_core::json::Json;
use phxzip::disco::{marcado_como_link, Destino};
use phxzip::tar::EscritorTar;
use phxzip::{Arquivo, Erro, Escritor, Limites, Metodo, Opcoes};

use crate::Resposta;

/// O teto da cabeca do envelope (`limites.cabeca`), conferido no `N` antes de
/// alocar.
pub const CABECA_MAX: usize = 1024 * 1024;

/// O maior trecho que se espia (`limites.espiar`). `ate` maior e rebaixado,
/// sem erro (contrato §4).
pub const ESPIAR_MAX: u64 = 256 * 1024;

/// A maior entrada que o servidor julga como JSON (`limites.avaliar_json`):
/// acima disto o veredito e `nao_avaliado`, e nao uma analise de 256 MiB.
pub const AVALIAR_JSON_MAX: u64 = 16 * 1024 * 1024;

/// Quantos `POST` descomprimem ao mesmo tempo (`limites.simultaneas`). A
/// memoria de pico e ≈ envio × simultaneas × 3 (pacote, bloco decodificado,
/// resposta).
pub const SIMULTANEAS: usize = 2;

/// Os niveis que o motor faz -- e nada alem (contrato §2.1).
pub const NIVEIS: [&str; 2] = ["armazenar", "lzma2"];

/// Os formatos que se gravam.
pub const FORMATOS: [&str; 2] = ["7z", "phz"];

const JSON: &str = "application/json; charset=utf-8";

// ================================================================ respostas

fn ok_json(j: Json) -> Resposta {
    Resposta {
        codigo: 200,
        tipo: JSON,
        extras: String::new(),
        corpo: Cow::Owned(j.escrever().into_bytes()),
    }
}

fn bytes(tipo: &'static str, extras: String, corpo: Vec<u8>) -> Resposta {
    Resposta {
        codigo: 200,
        tipo,
        extras,
        corpo: Cow::Owned(corpo),
    }
}

/// `PEDIDO_MALFORMADO`, com o campo que falhou (contrato §6).
pub fn malformado(campo: &str) -> Resposta {
    Resposta::erro(
        400,
        "PEDIDO_MALFORMADO",
        Some(Json::objeto(vec![("campo", Json::texto_de(campo))])),
    )
}

/// O erro do motor com o NOME dele (`Erro::nome`), sem traducao nem
/// renomeacao: a web e o terminal decidem pelo mesmo nome. O `detalhe` leva
/// os campos da variante.
pub fn do_motor(e: &Erro) -> Resposta {
    let t = Json::texto_de;
    let u = Json::de_u64;
    let detalhe = match e {
        Erro::VersaoNaoSuportada(v) => Some(vec![("versao", u(*v as u64))]),
        Erro::Estrutura(onde) | Erro::Corrompido(onde) | Erro::Tar(onde) => {
            Some(vec![("onde", t(onde.as_str()))])
        }
        // O `onde` do disco e do destino e texto do motor com o caminho da
        // pasta que o OPERADOR escolheu ao subir a porta: e a maquina dele, e
        // sem o caminho ele nao sabe o que consertar.
        Erro::DestinoInseguro(onde) | Erro::Disco(onde) => Some(vec![("onde", t(onde.as_str()))]),
        Erro::MetodoLegado(m) | Erro::MetodoDesconhecido(m) => {
            Some(vec![("metodo", t(m.as_str()))])
        }
        Erro::NomePerigoso(n) | Erro::EntradaEPasta(n) | Erro::NomeRepetido(n) => {
            Some(vec![("nome", t(n.as_str()))])
        }
        Erro::EntradaEspecial { nome, tipo } => {
            Some(vec![("nome", t(nome.as_str())), ("tipo", t(tipo.as_str()))])
        }
        Erro::GrandeDemais {
            oque,
            declarado,
            teto,
        } => Some(vec![
            ("oque", t(oque)),
            ("declarado", u(*declarado)),
            ("teto", u(*teto)),
        ]),
        Erro::NaoCabe { oque, valor } => Some(vec![("oque", t(oque)), ("valor", u(*valor))]),
        Erro::CiclosDemais { pedidos, teto } => Some(vec![
            ("pedidos", u(*pedidos as u64)),
            ("teto", u(*teto as u64)),
        ]),
        Erro::EntradaInexistente(i) => Some(vec![("indice", u(*i as u64))]),
        Erro::MaisDeUmaEntrada(n) => Some(vec![("quantas", u(*n))]),
        Erro::NaoE7z
        | Erro::SenhaErrada
        | Erro::SenhaErradaOuCorrompido
        | Erro::SenhaAusente
        | Erro::SemEntrada
        | Erro::SemCifra => None,
    };
    let codigo = match e {
        Erro::GrandeDemais { .. } => 413,
        Erro::Disco(_) => 500,
        _ => 422,
    };
    Resposta::erro(codigo, e.nome(), detalhe.map(Json::objeto))
}

/// O `Content-Disposition` de um download. O nome e DADO que veio de fora (do
/// pacote ou da cabeca), e cabecalho HTTP e texto de protocolo: a forma
/// `filename=` leva so ASCII imprimivel sem aspas nem barra invertida, e o
/// nome inteiro vai em `filename*` com cada byte fora do conjunto seguro
/// escrito `%XX` (RFC 6266 / RFC 8187). Uma quebra de linha no nome nunca
/// chega a ser quebra de linha no cabecalho.
pub fn anexo(nome: &str) -> String {
    let ultimo = nome.rsplit(['/', '\\']).next().unwrap_or("");
    let ultimo = if ultimo.trim().is_empty() {
        "arquivo"
    } else {
        ultimo
    };
    let simples: String = ultimo
        .chars()
        .map(|c| {
            if (c.is_ascii_graphic() || c == ' ') && c != '"' && c != '\\' && c != '%' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut codificado = String::new();
    for b in ultimo.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            codificado.push(b as char);
        } else {
            codificado.push_str(&format!("%{b:02X}"));
        }
    }
    format!("Content-Disposition: attachment; filename=\"{simples}\"; filename*=UTF-8''{codificado}\r\n")
}

// ================================================================ o envelope

/// Um envelope aberto: a cabeca (objeto JSON) e a carga crua.
pub struct Envelope<'a> {
    pub cabeca: Json,
    pub carga: &'a [u8],
}

/// Abre o envelope do contrato (§3): `PZW1`, `N` u32 LE, `N` bytes de JSON
/// UTF-8 e o resto. O `N` passa pelo [`CABECA_MAX`] ANTES de qualquer fatia:
/// e numero que veio de fora.
pub fn abrir_envelope(corpo: &[u8]) -> Result<Envelope<'_>, Resposta> {
    if corpo.len() < 8 || &corpo[..4] != b"PZW1" {
        return Err(malformado("envelope"));
    }
    let n = u32::from_le_bytes([corpo[4], corpo[5], corpo[6], corpo[7]]) as usize;
    if n > CABECA_MAX {
        return Err(Resposta::erro(
            413,
            "GRANDE_DEMAIS",
            Some(Json::objeto(vec![
                ("oque", Json::texto_de("cabeca")),
                ("declarado", Json::de_u64(n as u64)),
                ("teto", Json::de_u64(CABECA_MAX as u64)),
            ])),
        ));
    }
    let Some(texto) = corpo.get(8..8 + n) else {
        return Err(malformado("envelope"));
    };
    let Ok(texto) = std::str::from_utf8(texto) else {
        return Err(malformado("cabeca"));
    };
    let cabeca = match Json::analisar(texto) {
        Ok(j @ Json::Objeto(_)) => j,
        _ => return Err(malformado("cabeca")),
    };
    Ok(Envelope {
        cabeca,
        carga: &corpo[8 + n..],
    })
}

/// A senha da cabeca: ausente, `null` ou vazia e «sem senha»; outro tipo e
/// pedido torto. Nunca volta em resposta nenhuma.
fn senha(cab: &Json) -> Result<Option<String>, Resposta> {
    match cab.campo("senha") {
        None | Some(Json::Nulo) => Ok(None),
        Some(Json::Texto(s)) if s.is_empty() => Ok(None),
        Some(Json::Texto(s)) => Ok(Some(s.clone())),
        Some(_) => Err(malformado("senha")),
    }
}

fn booleano(cab: &Json, nome: &str, padrao: bool) -> Result<bool, Resposta> {
    match cab.campo(nome) {
        None | Some(Json::Nulo) => Ok(padrao),
        Some(Json::Bool(b)) => Ok(*b),
        Some(_) => Err(malformado(nome)),
    }
}

/// Inteiro sem sinal exato: `5.5`, `-1` e `1e300` sao pedido torto, nao
/// numero arredondado.
fn natural(j: &Json) -> Option<u64> {
    let n = j.numero()?;
    if n.fract() != 0.0 || !(0.0..=(phxsql_core::json::INTEIRO_EXATO_MAX as f64)).contains(&n) {
        return None;
    }
    Some(n as u64)
}

// ================================================================ GET

/// O `GET /api/estado` (contrato §2.1). Nenhum numero se digita aqui: cada um
/// e a constante que o motor -- ou esta porta -- confere.
pub fn estado(envio: usize, pasta: bool) -> Resposta {
    let lim = Limites::default();
    let lista = |v: &[&str]| Json::Lista(v.iter().map(|s| Json::texto_de(*s)).collect());
    ok_json(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("produto", Json::texto_de("PhxZip")),
        ("versao", Json::texto_de(env!("CARGO_PKG_VERSION"))),
        (
            "limites",
            Json::objeto(vec![
                ("envio", Json::de_u64(envio as u64)),
                ("cabeca", Json::de_u64(CABECA_MAX as u64)),
                ("entrada", Json::de_u64(lim.entrada)),
                ("bloco", Json::de_u64(lim.bloco)),
                ("cabecalho", Json::de_u64(lim.cabecalho)),
                ("espiar", Json::de_u64(ESPIAR_MAX)),
                ("avaliar_json", Json::de_u64(AVALIAR_JSON_MAX)),
                ("simultaneas", Json::de_u64(SIMULTANEAS as u64)),
            ]),
        ),
        ("niveis", lista(&NIVEIS)),
        ("formatos", lista(&FORMATOS)),
        ("idiomas", lista(&IDIOMAS)),
        ("extrair_na_pasta", Json::Bool(pasta)),
    ]))
}

/// O `idioma=` da query, sem decodificar: os nomes de coluna sao ASCII, e o
/// que vier codificado nao casa com nenhum e cai no portugues.
pub fn idioma_da_consulta(consulta: &str) -> &str {
    consulta
        .split('&')
        .find_map(|par| par.strip_prefix("idioma="))
        .unwrap_or("")
}

pub fn idiomas(consulta: &str) -> Resposta {
    match crate::textos::tabela() {
        Ok(t) => ok_json(crate::textos::resposta(t, idioma_da_consulta(consulta))),
        Err(_) => Resposta::erro(500, "INTERNO", None),
    }
}

// ================================================================ POST

fn abrir<'a>(env: &Envelope<'a>) -> Result<Arquivo<'a>, Resposta> {
    let s = senha(&env.cabeca)?;
    Arquivo::abrir(env.carga, s.as_deref(), Limites::default()).map_err(|e| do_motor(&e))
}

/// `POST /api/listar` (contrato §3.2): so o cabecalho, nada se decodifica.
pub fn listar(env: &Envelope) -> Result<Resposta, Resposta> {
    let a = abrir(env)?;
    let entradas = a
        .entradas()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            Json::objeto(vec![
                ("indice", Json::de_u64(i as u64)),
                ("nome", Json::texto_de(e.nome.as_str())),
                ("pasta", Json::Bool(e.pasta)),
                ("tamanho", Json::de_u64(e.tamanho)),
                (
                    "modificado",
                    e.modificado
                        .map(|f| Json::de_i64(phxzip::filetime_para_unix(f)))
                        .unwrap_or(Json::Nulo),
                ),
                ("cifrada", Json::Bool(e.cifrada)),
                (
                    "crc",
                    e.crc
                        .map(|c| Json::texto_de(format!("{c:08x}")))
                        .unwrap_or(Json::Nulo),
                ),
                (
                    "bloco",
                    e.bloco()
                        .map(|b| Json::de_u64(b as u64))
                        .unwrap_or(Json::Nulo),
                ),
            ])
        })
        .collect();
    let blocos = a
        .blocos()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            Json::objeto(vec![
                ("indice", Json::de_u64(i as u64)),
                ("compactado", Json::de_u64(b.compactado)),
                ("tamanho", Json::de_u64(b.tamanho)),
                ("entradas", Json::de_u64(b.entradas as u64)),
                (
                    "metodos",
                    Json::Lista(b.metodos.iter().map(|m| Json::texto_de(*m)).collect()),
                ),
            ])
        })
        .collect();
    Ok(ok_json(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("tamanho_do_pacote", Json::de_u64(env.carga.len() as u64)),
        ("cabecalho_cifrado", Json::Bool(a.cabecalho_cifrado())),
        ("entradas", Json::Lista(entradas)),
        ("blocos", Json::Lista(blocos)),
    ])))
}

/// `POST /api/testar` (contrato §3.3): decodifica tudo e confere todo CRC,
/// sem devolver conteudo.
pub fn testar(env: &Envelope) -> Result<Resposta, Resposta> {
    let mut a = abrir(env)?;
    let (mut arquivos, mut pastas, mut total) = (0u64, 0u64, 0u64);
    a.percorrer(|e, d| {
        if e.pasta {
            pastas += 1;
        } else {
            arquivos += 1;
            total += d.len() as u64;
        }
    })
    .map_err(|e| do_motor(&e))?;
    Ok(ok_json(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("entradas", Json::de_u64(arquivos)),
        ("pastas", Json::de_u64(pastas)),
        ("bytes", Json::de_u64(total)),
        ("blocos", Json::de_u64(a.blocos().len() as u64)),
    ])))
}

/// Um item da cabeca do compactar.
struct Item {
    nome: String,
    pasta: bool,
    tamanho: usize,
    modificado: Option<i64>,
}

fn itens(cab: &Json, carga: usize) -> Result<Vec<Item>, Resposta> {
    let Some(lista) = cab.campo("itens").and_then(Json::lista) else {
        return Err(malformado("itens"));
    };
    let mut v = Vec::with_capacity(lista.len());
    let mut soma: u64 = 0;
    for j in lista {
        let Some(nome) = j.campo("nome").and_then(Json::texto) else {
            return Err(malformado("nome"));
        };
        let pasta = booleano(j, "pasta", false)?;
        let tamanho = if pasta {
            0
        } else {
            match j.campo("tamanho").and_then(natural) {
                Some(t) => t,
                None => return Err(malformado("tamanho")),
            }
        };
        let modificado = match j.campo("modificado") {
            None | Some(Json::Nulo) => None,
            Some(m) => match m.inteiro() {
                Some(s) => Some(s),
                None => return Err(malformado("modificado")),
            },
        };
        soma = soma.saturating_add(tamanho);
        v.push(Item {
            nome: nome.to_string(),
            pasta,
            tamanho: tamanho as usize,
            modificado,
        });
    }
    // A soma dos tamanhos e o comprimento da carga: a fronteira entre um
    // conteudo e o proximo sai daqui, e uma soma que nao fecha deslocaria
    // todos os seguintes.
    if soma != carga as u64 {
        return Err(malformado("tamanho"));
    }
    Ok(v)
}

/// `POST /api/compactar` (contrato §3.1).
pub fn compactar(env: &Envelope) -> Result<Resposta, Resposta> {
    let cab = &env.cabeca;
    let formato = match cab.campo("formato") {
        None | Some(Json::Nulo) => "7z",
        Some(Json::Texto(f)) if FORMATOS.contains(&f.as_str()) => f.as_str(),
        Some(_) => return Err(malformado("formato")),
    };
    let nivel = match cab.campo("nivel") {
        None | Some(Json::Nulo) => "lzma2",
        Some(Json::Texto(n)) if NIVEIS.contains(&n.as_str()) => n.as_str(),
        Some(_) => return Err(malformado("nivel")),
    };
    let itens = itens(cab, env.carga.len())?;
    // Todo nome passa pelo motor ANTES de qualquer byte ser comprimido: o
    // zip-slip se recusa onde o nome nasce (contrato §7).
    for it in &itens {
        phxzip::conferir_nome(&it.nome).map_err(|e| do_motor(&e))?;
    }
    let senha = senha(cab)?;
    let nome = match cab.campo("nome").and_then(Json::texto) {
        Some(n) if !n.trim().is_empty() => n.to_string(),
        _ => format!("pacote.{formato}"),
    };
    let original = env.carga.len() as u64;

    let pacote = if formato == "phz" {
        // A regra do `.phz` e do motor (pedido 450): o que a web acrescenta e
        // so traduzir a lista de itens para a pergunta que ele faz.
        let Some(s) = senha.as_deref() else {
            return Err(do_motor(&Erro::SenhaAusente));
        };
        match itens.as_slice() {
            [] => return Err(do_motor(&Erro::SemEntrada)),
            [so] if so.pasta => return Err(do_motor(&Erro::EntradaEPasta(so.nome.clone()))),
            [so] => phxzip::empacotar(&so.nome, env.carga, s).map_err(|e| do_motor(&e))?,
            muitos => return Err(do_motor(&Erro::MaisDeUmaEntrada(muitos.len() as u64))),
        }
    } else {
        let cifrar_nomes = booleano(cab, "cifrar_nomes", true)?;
        let mut esc = Escritor::novo(Opcoes {
            metodo: if nivel == "armazenar" {
                Metodo::Copia
            } else {
                Metodo::Lzma2
            },
            cifrar_cabecalho: senha.is_some() && cifrar_nomes,
            senha,
            ciclos: phxzip::CICLOS_PADRAO,
        })
        .map_err(|e| do_motor(&e))?;
        let mut desde = 0usize;
        for it in &itens {
            let quando = it.modificado.map(phxzip::unix_para_filetime);
            if it.pasta {
                esc.pasta(&it.nome, quando)
            } else {
                let conteudo = &env.carga[desde..desde + it.tamanho];
                desde += it.tamanho;
                esc.arquivo(&it.nome, conteudo, quando)
            }
            .map_err(|e| do_motor(&e))?;
        }
        esc.terminar()
    };
    let extras = format!("{}X-PhxZip-Tamanho-Original: {original}\r\n", anexo(&nome));
    Ok(bytes("application/x-7z-compressed", extras, pacote))
}

/// Quais indices a cabeca do extrair pede: `todas`, ou a lista `indices`.
fn escolhidos(cab: &Json, total: usize) -> Result<Option<BTreeSet<usize>>, Resposta> {
    if booleano(cab, "todas", false)? {
        return Ok(None);
    }
    let Some(lista) = cab.campo("indices").and_then(Json::lista) else {
        return Err(malformado("indices"));
    };
    if lista.is_empty() {
        return Err(malformado("indices"));
    }
    let mut v = BTreeSet::new();
    for j in lista {
        let Some(i) = natural(j) else {
            return Err(malformado("indices"));
        };
        let i = usize::try_from(i).unwrap_or(usize::MAX);
        if i >= total {
            return Err(do_motor(&Erro::EntradaInexistente(i)));
        }
        v.insert(i);
    }
    Ok(Some(v))
}

/// `POST /api/extrair` (contrato §3.4). `pasta` e a pasta que o OPERADOR
/// deu ao subir a porta (`--pasta`); o navegador nunca escolhe caminho.
pub fn extrair(env: &Envelope, pasta: Option<&Path>) -> Result<Resposta, Resposta> {
    let cab = &env.cabeca;
    let mut a = abrir(env)?;
    let total = a.entradas().len();
    let escolha = escolhidos(cab, total)?;
    let quer = |i: usize| escolha.as_ref().is_none_or(|s| s.contains(&i));

    if booleano(cab, "na_pasta", false)? {
        let Some(raiz) = pasta else {
            return Err(Resposta::erro(422, "SEM_PASTA", None));
        };
        return na_pasta(&mut a, raiz, &quer);
    }

    // Um indice so, de arquivo: a entrada crua.
    if let Some(s) = &escolha {
        if s.len() == 1 {
            let i = *s.iter().next().unwrap_or(&0);
            let e = a.entradas()[i].clone();
            if !e.pasta {
                return uma(&mut a, i, &e, cab);
            }
        }
    }

    // Varias, ou uma pasta: o `.tar`, com as pastas e as datas. Uma pasta
    // pedida sozinha leva o que mora debaixo dela.
    let pastas_pedidas: Vec<String> = a
        .entradas()
        .iter()
        .enumerate()
        .filter(|(i, e)| e.pasta && escolha.as_ref().is_some_and(|s| s.contains(i)))
        .map(|(_, e)| format!("{}/", e.nome))
        .collect();
    let dentro =
        |i: usize, nome: &str| quer(i) || pastas_pedidas.iter().any(|p| nome.starts_with(p));
    let mut tar = EscritorTar::novo();
    let mut falha = None;
    let mut i = 0usize;
    a.percorrer(|e, d| {
        let este = i;
        i += 1;
        if falha.is_some() || !dentro(este, &e.nome) {
            return;
        }
        let quando = e.modificado.map(phxzip::filetime_para_unix).unwrap_or(0);
        let r = if e.pasta {
            tar.pasta(&e.nome, quando)
        } else {
            tar.arquivo(&e.nome, d, quando)
        };
        if let Err(x) = r {
            falha = Some(x);
        }
    })
    .map_err(|e| do_motor(&e))?;
    if let Some(e) = falha {
        return Err(do_motor(&e));
    }
    Ok(bytes(
        "application/x-tar",
        anexo("extraido.tar"),
        tar.terminar(),
    ))
}

/// A entrada crua, inteira ou o trecho do espiar, com o veredito do JSON.
fn uma(a: &mut Arquivo, i: usize, e: &phxzip::Entrada, cab: &Json) -> Result<Resposta, Resposta> {
    let mut dado = a.extrair(i).map_err(|e| do_motor(&e))?;
    let mut extras = anexo(&e.nome);
    extras.push_str(&format!("X-PhxZip-Tamanho: {}\r\n", e.tamanho));
    if booleano(cab, "avaliar_json", false)? {
        // O juiz e o analisador da casa, sobre o conteudo INTEIRO -- nao o
        // trecho que a tela vai mostrar.
        let veredito = if e.tamanho > AVALIAR_JSON_MAX {
            "nao_avaliado".to_string()
        } else {
            match std::str::from_utf8(&dado) {
                Err(_) => "nao_e_texto".to_string(),
                Ok(t) => match Json::analisar_com_posicao(t) {
                    Ok(_) => "valido".to_string(),
                    Err(f) => format!("invalido\r\nX-PhxZip-Json-Posicao: {}", f.posicao),
                },
            }
        };
        extras.push_str(&format!("X-PhxZip-Json: {veredito}\r\n"));
    }
    match cab.campo("ate") {
        None | Some(Json::Nulo) => {}
        Some(j) => {
            let Some(ate) = natural(j) else {
                return Err(malformado("ate"));
            };
            let ate = ate.min(ESPIAR_MAX) as usize;
            if dado.len() > ate {
                dado.truncate(ate);
                extras.push_str("X-PhxZip-Cortado: 1\r\n");
            }
        }
    }
    Ok(bytes("application/octet-stream", extras, dado))
}

/// Extrai na pasta do operador, SO pelo `disco::Destino` do motor -- o mesmo
/// do PhxZipCmd e do tar: nome conferido de novo, colisao arquivo/pasta antes
/// do primeiro byte, nada segue link, nada se sobrescreve, e o DONO de cada
/// pasta do caminho conferido a cada pedido (a revisao SEC da Z9: pasta onde
/// outro usuario escreve e recusada, porque ele trocaria uma pasta por um
/// link no meio da extracao).
fn na_pasta(
    a: &mut Arquivo,
    raiz: &Path,
    quer: &dyn Fn(usize) -> bool,
) -> Result<Resposta, Resposta> {
    Destino::conferir_lote(
        a.entradas()
            .iter()
            .enumerate()
            .filter(|(i, _)| quer(*i))
            .map(|(_, e)| (e.nome.as_str(), e.pasta)),
    )
    .map_err(|e| do_motor(&e))?;
    let destino = Destino::novo(raiz).map_err(|e| do_motor(&e))?;
    let (mut gravadas, mut links) = (0u64, 0u64);
    let mut falha = None;
    let mut i = 0usize;
    let r = a.percorrer(|e, d| {
        let este = i;
        i += 1;
        if falha.is_some() || !quer(este) {
            return;
        }
        match destino.entrada(e, d) {
            Ok(()) => {
                gravadas += 1;
                if marcado_como_link(e.atributos) {
                    links += 1;
                }
            }
            Err(x) => falha = Some(x),
        }
    });
    if let Some(e) = falha.or(r.err()) {
        // O que ja foi gravado fica (nada se apaga calado), e a resposta diz
        // quantas: meia extracao que se anuncia como nada seria mentira.
        let mut resp = do_motor(&e);
        if let Ok(Json::Objeto(mut pares)) =
            Json::analisar(std::str::from_utf8(&resp.corpo).unwrap_or(""))
        {
            let mut det = match pares.iter().position(|(k, _)| k == "detalhe") {
                Some(p) => pares.remove(p).1,
                None => Json::Objeto(Vec::new()),
            };
            det.definir("gravadas", Json::de_u64(gravadas));
            pares.push(("detalhe".into(), det));
            resp.corpo = Cow::Owned(Json::Objeto(pares).escrever().into_bytes());
        }
        return Err(resp);
    }
    Ok(ok_json(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("gravadas", Json::de_u64(gravadas)),
        ("links_gravados_como_arquivo", Json::de_u64(links)),
    ])))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn env(cabeca: &str, carga: &[u8]) -> Vec<u8> {
        let mut v = b"PZW1".to_vec();
        v.extend_from_slice(&(cabeca.len() as u32).to_le_bytes());
        v.extend_from_slice(cabeca.as_bytes());
        v.extend_from_slice(carga);
        v
    }

    fn erro_de(r: &Resposta) -> String {
        let j = Json::analisar(std::str::from_utf8(&r.corpo).unwrap()).unwrap();
        j.campo("erro").and_then(Json::texto).unwrap().to_string()
    }

    #[test]
    fn envelope_torto_e_malformado_e_cabeca_grande_e_413() {
        for ruim in [&b""[..], b"PZW", b"PZW2\0\0\0\0", b"PZW1\x05\0\0\0{}"] {
            let r = abrir_envelope(ruim).err().unwrap();
            assert_eq!((r.codigo, erro_de(&r)), (400, "PEDIDO_MALFORMADO".into()));
        }
        let r = abrir_envelope(&env("[1]", b"")).err().unwrap();
        assert_eq!(r.codigo, 400);
        let mut grande = b"PZW1".to_vec();
        grande.extend_from_slice(&((CABECA_MAX as u32) + 1).to_le_bytes());
        let r = abrir_envelope(&grande).err().unwrap();
        assert_eq!((r.codigo, erro_de(&r)), (413, "GRANDE_DEMAIS".into()));
        let e = env(r#"{"a":1}"#, b"\xff\x00");
        let ok = abrir_envelope(&e).ok().unwrap();
        assert_eq!(ok.carga, b"\xff\x00");
    }

    #[test]
    fn o_anexo_nao_deixa_o_nome_quebrar_o_cabecalho() {
        let a = anexo("pasta/a\"b\r\nX-Mal: 1.txt");
        assert_eq!(a.matches("\r\n").count(), 1, "{a:?}");
        assert!(a.ends_with("\r\n"));
        assert!(a.contains("filename=\"a_b__X-Mal: 1.txt\""), "{a}");
        assert!(
            a.contains("filename*=UTF-8''a%22b%0D%0AX-Mal%3A%201.txt"),
            "{a}"
        );
        assert!(anexo("ação.7z").contains("filename*=UTF-8''a%C3%A7%C3%A3o.7z"));
    }

    /// Ida e volta pelo motor, sem rede: compactar, listar, testar, extrair.
    #[test]
    fn compactar_listar_testar_extrair() {
        let cab = r#"{"formato":"7z","nivel":"lzma2","senha":"s","nome":"r.7z","itens":[
            {"nome":"r","pasta":true,"modificado":null},
            {"nome":"r/a.json","tamanho":7,"modificado":1790000000},
            {"nome":"r/b.txt","tamanho":3}]}"#;
        let corpo = env(cab, b"{\"a\":1}abc");
        let r = compactar(&abrir_envelope(&corpo).ok().unwrap())
            .ok()
            .unwrap();
        assert_eq!(r.tipo, "application/x-7z-compressed");
        assert!(r.extras.contains("X-PhxZip-Tamanho-Original: 10\r\n"));
        let pacote = r.corpo.to_vec();

        let sem = env("{}", &pacote);
        let r = listar(&abrir_envelope(&sem).ok().unwrap()).err().unwrap();
        assert_eq!(erro_de(&r), "SENHA_AUSENTE");

        let com = env(r#"{"senha":"s"}"#, &pacote);
        let l = listar(&abrir_envelope(&com).ok().unwrap()).ok().unwrap();
        let j = Json::analisar(std::str::from_utf8(&l.corpo).unwrap()).unwrap();
        assert_eq!(j.campo("cabecalho_cifrado"), Some(&Json::Bool(true)));
        let ents = j.campo("entradas").and_then(Json::lista).unwrap();
        assert_eq!(ents.len(), 3);
        assert_eq!(
            ents[1].campo("nome").and_then(Json::texto),
            Some("r/a.json")
        );
        assert_eq!(
            ents[1].campo("modificado").and_then(Json::inteiro),
            Some(1_790_000_000)
        );
        assert!(!j.campo("blocos").and_then(Json::lista).unwrap().is_empty());

        let t = testar(&abrir_envelope(&com).ok().unwrap()).ok().unwrap();
        let j = Json::analisar(std::str::from_utf8(&t.corpo).unwrap()).unwrap();
        assert_eq!(j.campo("entradas").and_then(Json::inteiro), Some(2));
        assert_eq!(j.campo("pastas").and_then(Json::inteiro), Some(1));
        assert_eq!(j.campo("bytes").and_then(Json::inteiro), Some(10));

        let um = env(
            r#"{"senha":"s","indices":[1],"ate":3,"avaliar_json":true}"#,
            &pacote,
        );
        let r = extrair(&abrir_envelope(&um).ok().unwrap(), None)
            .ok()
            .unwrap();
        assert_eq!(&r.corpo[..], b"{\"a");
        assert!(r.extras.contains("X-PhxZip-Cortado: 1\r\n"));
        assert!(r.extras.contains("X-PhxZip-Json: valido\r\n"));
        assert!(r.extras.contains("X-PhxZip-Tamanho: 7\r\n"));

        let todas = env(r#"{"senha":"s","todas":true}"#, &pacote);
        let r = extrair(&abrir_envelope(&todas).ok().unwrap(), None)
            .ok()
            .unwrap();
        assert_eq!(r.tipo, "application/x-tar");
        let lidas = phxzip::tar::ler(&r.corpo).unwrap();
        let nomes: Vec<&str> = lidas.iter().map(|e| e.nome.as_str()).collect();
        assert_eq!(nomes, ["r", "r/a.json", "r/b.txt"]);
        assert_eq!(lidas[2].conteudo, b"abc");
        assert_eq!(lidas[1].modificado, 1_790_000_000);
    }

    #[test]
    fn json_invalido_diz_a_posicao() {
        let cab = r#"{"itens":[{"nome":"c.json","tamanho":9}]}"#;
        let p = compactar(&abrir_envelope(&env(cab, b"{\"a\":1,}x")).ok().unwrap())
            .ok()
            .unwrap()
            .corpo
            .to_vec();
        let r = extrair(
            &abrir_envelope(&env(r#"{"indices":[0],"avaliar_json":true}"#, &p))
                .ok()
                .unwrap(),
            None,
        )
        .ok()
        .unwrap();
        assert!(
            r.extras
                .contains("X-PhxZip-Json: invalido\r\nX-PhxZip-Json-Posicao: "),
            "{}",
            r.extras
        );
    }

    /// RED zip-slip pela web, na ida: o nome com `..` e recusado antes de
    /// qualquer byte ser comprimido.
    #[test]
    fn nome_perigoso_no_compactar_e_recusado() {
        for ruim in ["../fora.txt", "/etc/x", "a/../../b", "C:\\x"] {
            let cab = format!(
                r#"{{"itens":[{{"nome":{},"tamanho":1}}]}}"#,
                Json::texto_de(ruim).escrever()
            );
            let r = compactar(&abrir_envelope(&env(&cab, b"x")).ok().unwrap())
                .err()
                .unwrap();
            assert_eq!(
                (r.codigo, erro_de(&r)),
                (422, "NOME_PERIGOSO".into()),
                "{ruim}"
            );
        }
    }

    #[test]
    fn soma_dos_tamanhos_tem_de_fechar_com_a_carga() {
        let cab = r#"{"itens":[{"nome":"a","tamanho":2}]}"#;
        let r = compactar(&abrir_envelope(&env(cab, b"abc")).ok().unwrap())
            .err()
            .unwrap();
        assert_eq!(r.codigo, 400);
        assert!(std::str::from_utf8(&r.corpo)
            .unwrap()
            .contains("\"campo\":\"tamanho\""));
    }

    #[test]
    fn as_regras_do_phz_sao_as_do_motor() {
        let caso = |cab: &str, carga: &[u8]| {
            erro_de(
                &compactar(&abrir_envelope(&env(cab, carga)).ok().unwrap())
                    .err()
                    .unwrap(),
            )
        };
        assert_eq!(
            caso(
                r#"{"formato":"phz","itens":[{"nome":"a","tamanho":1}]}"#,
                b"x"
            ),
            "SENHA_AUSENTE"
        );
        assert_eq!(
            caso(r#"{"formato":"phz","senha":"s","itens":[]}"#, b""),
            "SEM_ENTRADA"
        );
        assert_eq!(
            caso(
                r#"{"formato":"phz","senha":"s","itens":[{"nome":"a","tamanho":1},{"nome":"b","tamanho":0}]}"#,
                b"x"
            ),
            "MAIS_DE_UMA_ENTRADA"
        );
        assert_eq!(
            caso(
                r#"{"formato":"phz","senha":"s","itens":[{"nome":"a","pasta":true}]}"#,
                b""
            ),
            "ENTRADA_E_PASTA"
        );
        let ok = compactar(
            &abrir_envelope(&env(
                r#"{"formato":"phz","senha":"s","itens":[{"nome":"c.json","tamanho":2}]}"#,
                b"{}",
            ))
            .ok()
            .unwrap(),
        )
        .ok()
        .unwrap();
        assert_eq!(
            phxzip::desempacotar(&ok.corpo, "s").unwrap(),
            ("c.json".into(), b"{}".to_vec())
        );
    }

    /// A senha nunca volta: nem no erro de senha errada.
    #[test]
    fn a_senha_nao_volta_em_resposta_nenhuma() {
        let cab = r#"{"senha":"segredo-unico-42","itens":[{"nome":"a","tamanho":1}]}"#;
        let p = compactar(&abrir_envelope(&env(cab, b"x")).ok().unwrap())
            .ok()
            .unwrap();
        assert!(!p.extras.contains("segredo-unico-42"));
        let r = listar(
            &abrir_envelope(&env(r#"{"senha":"outra-senha-77"}"#, &p.corpo))
                .ok()
                .unwrap(),
        )
        .err()
        .unwrap();
        let texto = String::from_utf8_lossy(&r.corpo);
        assert!(
            !texto.contains("outra-senha-77") && !texto.contains("segredo"),
            "{texto}"
        );
    }

    #[test]
    fn idioma_da_consulta() {
        assert_eq!(super::idioma_da_consulta("idioma=Alemao"), "Alemao");
        assert_eq!(super::idioma_da_consulta("x=1&idioma=Ingles"), "Ingles");
        assert_eq!(super::idioma_da_consulta(""), "");
    }
}
