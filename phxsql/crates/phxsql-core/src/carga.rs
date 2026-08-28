//! Ler uma carga colada: JSON, CSV, TXT, HTML ou XML.
//!
//! E o caminho de volta do `exportar.rs`: o que sai por la entra por aqui. Os
//! cinco formatos de texto voltam; o XLSX e o DOCX nao -- ler ZIP de XML e
//! outro trabalho, e quem tem planilha salva como CSV em dois cliques.
//!
//! # O que estes leitores nao sao
//!
//! Nao sao analisadores completos de HTML nem de XML. Sao leitores do que o
//! `exportar.rs` escreve e do que uma planilha ou um banco de dados cospe: uma
//! TABELA. O HTML procura `<tr>` e `<td>`; o XML procura elementos repetidos
//! com campos dentro. Documento com estrutura aninhada nao entra -- e isso e
//! recusa explicita, e nao interpretacao criativa de um arquivo que o usuario
//! achou que ia funcionar.
//!
//! # A decisao que atravessa os cinco
//!
//! **A primeira linha e o cabecalho, e ele manda.** Os nomes das colunas vem
//! dela e sao casados com o esquema POR NOME, e nao por posicao. Casar por
//! posicao faria uma coluna a mais no meio do arquivo gravar tudo deslocado --
//! sem erro, porque os tipos costumam aceitar.
//!
//! Coluna do arquivo que nao existe na tabela e ERRO, com o nome dela. Coluna
//! da tabela que nao esta no arquivo fica nula (ou com o padrao, no caso das
//! colunas de sistema).

use crate::error::{PhxError, Result};
use crate::json::Json;

/// Uma carga lida: o cabecalho e as linhas, tudo em texto.
///
/// Texto de proposito: converter para o tipo da coluna e trabalho do
/// `valores.rs`, que ja sabe fazer isso e ja tem as mensagens de erro. Um
/// leitor de formato que tambem converte tipo erra nos dois.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Carga {
    pub colunas: Vec<String>,
    pub linhas: Vec<Vec<String>>,
}

impl Carga {
    /// Vira uma lista de objetos JSON, que e o que `json_para_linha` come.
    pub fn para_json(&self) -> Json {
        Json::Lista(
            self.linhas
                .iter()
                .map(|l| {
                    Json::Objeto(
                        self.colunas
                            .iter()
                            .zip(l.iter())
                            .map(|(c, v)| (c.clone(), Json::texto_de(v)))
                            .collect(),
                    )
                })
                .collect(),
        )
    }
}

/// Os formatos que entram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Json,
    Csv,
    Txt,
    Html,
    Xml,
}

impl Formato {
    pub fn de_texto(t: &str) -> Result<Formato> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "json" => Formato::Json,
            "csv" => Formato::Csv,
            "txt" | "texto" | "tsv" => Formato::Txt,
            "html" | "htm" => Formato::Html,
            "xml" => Formato::Xml,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "formato {outro:?} nao entra; use json, csv, txt, html ou xml"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Formato::Json => "json",
            Formato::Csv => "csv",
            Formato::Txt => "txt",
            Formato::Html => "html",
            Formato::Xml => "xml",
        }
    }
}

/// Advinha o formato pelo primeiro caractere que nao e espaco.
///
/// Serve para a tela nao obrigar a escolher quando e obvio. Erra para o lado
/// seguro: o que nao for reconhecido vira CSV, que e o formato que aceita mais
/// coisa -- e um CSV lido errado da erro de coluna, e nao dado torto.
pub fn adivinhar(texto: &str) -> Formato {
    let t = texto.trim_start();
    if t.starts_with('[') || t.starts_with('{') {
        return Formato::Json;
    }
    let baixo = t.to_ascii_lowercase();
    if baixo.starts_with("<?xml") {
        // Um `<?xml` pode preceder os dois. Quem decide e a presenca de `<tr`.
        return if baixo.contains("<tr") {
            Formato::Html
        } else {
            Formato::Xml
        };
    }
    if baixo.starts_with("<!doctype html") || baixo.starts_with("<html") || baixo.contains("<tr") {
        return Formato::Html;
    }
    if t.starts_with('<') {
        return Formato::Xml;
    }
    if t.lines().next().is_some_and(|l| l.contains('\t')) {
        return Formato::Txt;
    }
    Formato::Csv
}

/// Le a carga no formato pedido.
pub fn ler(texto: &str, formato: Formato) -> Result<Carga> {
    let c = match formato {
        Formato::Json => json(texto)?,
        Formato::Csv => separado(texto, None)?,
        Formato::Txt => separado(texto, Some('\t'))?,
        Formato::Html => html(texto)?,
        Formato::Xml => xml(texto)?,
    };
    conferir(&c)?;
    Ok(c)
}

/// As conferencias que valem para os cinco.
fn conferir(c: &Carga) -> Result<()> {
    if c.colunas.is_empty() {
        return Err(PhxError::Esquema(
            "a carga nao tem cabecalho: a primeira linha precisa trazer os nomes das colunas"
                .into(),
        ));
    }
    for (i, nome) in c.colunas.iter().enumerate() {
        if nome.trim().is_empty() {
            return Err(PhxError::Esquema(format!(
                "a coluna {} do cabecalho esta sem nome",
                i + 1
            )));
        }
        if c.colunas.iter().take(i).any(|o| o == nome) {
            return Err(PhxError::Esquema(format!(
                "a coluna {nome:?} aparece duas vezes no cabecalho"
            )));
        }
    }
    // Linha com mais campos que o cabecalho e erro, e nao sobra a ignorar: em
    // CSV isso quase sempre e uma virgula dentro de um campo sem aspas, e
    // aceitar calado gravaria o texto partido em duas colunas.
    if let Some((i, l)) = c
        .linhas
        .iter()
        .enumerate()
        .find(|(_, l)| l.len() > c.colunas.len())
    {
        return Err(PhxError::Esquema(format!(
            "a linha {} tem {} campos e o cabecalho tem {}: separador dentro de um campo sem aspas?",
            i + 1,
            l.len(),
            c.colunas.len()
        )));
    }
    Ok(())
}

// ------------------------------------------------------------------- JSON

fn json(texto: &str) -> Result<Carga> {
    let j = Json::analisar(texto)?;
    // Aceita a lista solta e o objeto com `linhas` dentro -- que e o que o
    // proprio `exportar` escreve, e obrigar a editar o arquivo antes de colar
    // seria implicancia.
    let itens = j
        .lista()
        .or_else(|| j.campo("linhas").and_then(Json::lista))
        .ok_or_else(|| {
            PhxError::Esquema(
                "o JSON precisa ser uma lista de objetos, ou um objeto com \"linhas\"".into(),
            )
        })?;

    let mut colunas: Vec<String> = Vec::new();
    let mut linhas = Vec::with_capacity(itens.len());
    for (i, item) in itens.iter().enumerate() {
        let Json::Objeto(pares) = item else {
            return Err(PhxError::Esquema(format!(
                "o item {} da lista nao e um objeto",
                i + 1
            )));
        };
        // O cabecalho sai do PRIMEIRO objeto, e os outros seguem. Objetos com
        // chaves diferentes entre si dariam uma tabela com buracos que ninguem
        // pediu; aqui o que faltar fica vazio e o que sobrar e erro.
        if i == 0 {
            colunas = pares.iter().map(|(k, _)| k.clone()).collect();
        }
        let mut linha = Vec::with_capacity(colunas.len());
        for c in &colunas {
            linha.push(match item.campo(c) {
                None | Some(Json::Nulo) => String::new(),
                Some(Json::Texto(t)) => t.clone(),
                Some(Json::Bool(b)) => (if *b { "true" } else { "false" }).to_string(),
                Some(outro) => outro.escrever(),
            });
        }
        if let Some((extra, _)) = pares.iter().find(|(k, _)| !colunas.contains(k)) {
            return Err(PhxError::Esquema(format!(
                "o item {} tem a chave {extra:?}, que nao esta no primeiro item",
                i + 1
            )));
        }
        linhas.push(linha);
    }
    Ok(Carga { colunas, linhas })
}

// --------------------------------------------------------------- CSV e TXT

/// Le separado por delimitador, com aspas no estilo do RFC 4180.
///
/// `sep` `None` faz o separador ser DESCOBERTO da primeira linha, entre `;`,
/// `,` e tabulacao -- nessa ordem. O ponto e virgula vem primeiro porque e o
/// que o Excel(R) em portugues escreve, e e o que o proprio `exportar` gera.
fn separado(texto: &str, sep: Option<char>) -> Result<Carga> {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let primeira = texto.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = sep.unwrap_or_else(|| {
        for c in [';', ',', '\t'] {
            if primeira.contains(c) {
                return c;
            }
        }
        ';'
    });

    let mut campos = campos_do_texto(texto, sep);
    if campos.is_empty() {
        return Ok(Carga::default());
    }
    let colunas: Vec<String> = campos
        .remove(0)
        .into_iter()
        .map(|c| c.trim().to_string())
        .collect();
    Ok(Carga {
        colunas,
        linhas: campos,
    })
}

/// Divide o texto inteiro em linhas de campos, respeitando aspas.
///
/// Feito num passo so, sobre o texto inteiro, e nao linha a linha: um campo
/// entre aspas pode CONTER quebra de linha, e dividir por linha antes de olhar
/// as aspas parte esse campo em dois -- que e o defeito classico de leitor de
/// CSV escrito as pressas.
fn campos_do_texto(texto: &str, sep: char) -> Vec<Vec<String>> {
    let mut linhas = Vec::new();
    let mut linha = Vec::new();
    let mut campo = String::new();
    let mut nas_aspas = false;
    let mut chars = texto.chars().peekable();

    while let Some(c) = chars.next() {
        if nas_aspas {
            if c == '"' {
                // Aspas dobradas dentro do campo sao uma aspa literal.
                if chars.peek() == Some(&'"') {
                    chars.next();
                    campo.push('"');
                } else {
                    nas_aspas = false;
                }
            } else {
                campo.push(c);
            }
            continue;
        }
        match c {
            '"' if campo.trim().is_empty() => {
                campo.clear();
                nas_aspas = true;
            }
            c if c == sep => linha.push(std::mem::take(&mut campo)),
            '\r' => {}
            '\n' => {
                linha.push(std::mem::take(&mut campo));
                // Linha em branco no meio do arquivo nao vira registro vazio.
                if !(linha.len() == 1 && linha[0].trim().is_empty()) {
                    linhas.push(std::mem::take(&mut linha));
                } else {
                    linha.clear();
                }
            }
            c => campo.push(c),
        }
    }
    if !campo.is_empty() || !linha.is_empty() {
        linha.push(campo);
        if !(linha.len() == 1 && linha[0].trim().is_empty()) {
            linhas.push(linha);
        }
    }
    linhas
}

// ------------------------------------------------------------------- HTML

/// Le a PRIMEIRA `<table>` do documento.
///
/// O cabecalho sai do `<thead>` se houver, senao da primeira `<tr>` -- que e
/// como uma tabela colada do navegador ou do Excel(R) costuma vir.
fn html(texto: &str) -> Result<Carga> {
    let linhas_brutas = celulas_por_linha(texto);
    if linhas_brutas.is_empty() {
        return Err(PhxError::Esquema(
            "nao achei nenhuma linha de tabela (<tr>) no HTML".into(),
        ));
    }
    let mut it = linhas_brutas.into_iter();
    let colunas = it.next().unwrap_or_default();
    Ok(Carga {
        colunas,
        linhas: it.collect(),
    })
}

/// As celulas de cada `<tr>`, ja sem marcacao e com as entidades resolvidas.
fn celulas_por_linha(texto: &str) -> Vec<Vec<String>> {
    let baixo = texto.to_ascii_lowercase();
    let mut saida = Vec::new();
    let mut pos = 0;
    while let Some(inicio) = baixo[pos..].find("<tr").map(|i| i + pos) {
        let fim = baixo[inicio..]
            .find("</tr")
            .map(|i| i + inicio)
            .unwrap_or(baixo.len());
        let mut celulas = Vec::new();
        let mut p = inicio;
        while let Some(ci) = baixo[p..fim]
            .find("<td")
            .or_else(|| baixo[p..fim].find("<th"))
            .map(|i| i + p)
        {
            // Pula o resto da tag de abertura: `<td class="x">`.
            let Some(abre) = baixo[ci..fim].find('>').map(|i| i + ci + 1) else {
                break;
            };
            let fecha = baixo[abre..fim]
                .find("</td")
                .or_else(|| baixo[abre..fim].find("</th"))
                .map(|i| i + abre)
                .unwrap_or(fim);
            celulas.push(sem_marcacao(&texto[abre..fecha]));
            p = fecha + 1;
            if p >= fim {
                break;
            }
        }
        if !celulas.is_empty() {
            saida.push(celulas);
        }
        pos = fim + 1;
        if pos >= baixo.len() {
            break;
        }
    }
    saida
}

/// Tira as tags de dentro de uma celula e resolve as entidades.
fn sem_marcacao(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dentro = false;
    for c in s.chars() {
        match c {
            '<' => dentro = true,
            '>' => dentro = false,
            c if !dentro => out.push(c),
            _ => {}
        }
    }
    entidades(out.trim())
}

fn entidades(s: &str) -> String {
    // A ordem importa: `&amp;` por ultimo, senao `&amp;lt;` vira `<`.
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

// -------------------------------------------------------------------- XML

/// Le elementos repetidos com campos simples dentro.
///
/// ```xml
/// <linhas>
///   <linha><id>1</id><nome>Adriano</nome></linha>
///   <linha><id>2</id><nome>Maria</nome></linha>
/// </linhas>
/// ```
///
/// O nome do elemento de linha nao importa: o que conta e ser o elemento que
/// se repete no segundo nivel. O cabecalho sai do PRIMEIRO, como no JSON.
fn xml(texto: &str) -> Result<Carga> {
    let sem_decl = match texto.find("?>") {
        Some(i) => &texto[i + 2..],
        None => texto,
    };
    // O elemento raiz, e dentro dele os filhos que se repetem.
    let Some(raiz_ini) = sem_decl.find('<') else {
        return Err(PhxError::Esquema("o XML esta vazio".into()));
    };
    let Some(raiz_fim) = sem_decl[raiz_ini..].find('>').map(|i| i + raiz_ini) else {
        return Err(PhxError::Esquema("o XML nao tem elemento raiz".into()));
    };
    let corpo = &sem_decl[raiz_fim + 1..];

    let mut colunas: Vec<String> = Vec::new();
    let mut linhas = Vec::new();
    let mut pos = 0;
    while let Some((nome, conteudo, fim)) = proximo_elemento(corpo, pos) {
        pos = fim;
        let mut campos: Vec<(String, String)> = Vec::new();
        let mut p = 0;
        while let Some((c_nome, c_texto, c_fim)) = proximo_elemento(&conteudo, p) {
            p = c_fim;
            campos.push((c_nome, entidades(c_texto.trim())));
        }
        if campos.is_empty() {
            continue;
        }
        if colunas.is_empty() {
            colunas = campos.iter().map(|(k, _)| k.clone()).collect();
        }
        let mut linha = Vec::with_capacity(colunas.len());
        for c in &colunas {
            linha.push(
                campos
                    .iter()
                    .find(|(k, _)| k == c)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default(),
            );
        }
        if let Some((extra, _)) = campos.iter().find(|(k, _)| !colunas.contains(k)) {
            return Err(PhxError::Esquema(format!(
                "o elemento <{nome}> tem <{extra}>, que nao esta no primeiro"
            )));
        }
        linhas.push(linha);
    }
    if colunas.is_empty() {
        return Err(PhxError::Esquema(
            "nao achei nenhum elemento com campos dentro no XML".into(),
        ));
    }
    Ok(Carga { colunas, linhas })
}

/// O proximo elemento a partir de `de`: `(nome, conteudo, onde acabou)`.
///
/// Ignora comentario e elemento vazio (`<x/>`), que aparecem em arquivo
/// exportado e nao sao dado.
fn proximo_elemento(s: &str, de: usize) -> Option<(String, String, usize)> {
    let mut pos = de;
    loop {
        let abre = s[pos..].find('<').map(|i| i + pos)?;
        if s[abre..].starts_with("<!--") {
            pos = s[abre..].find("-->").map(|i| i + abre + 3)?;
            continue;
        }
        if s[abre..].starts_with("</") {
            pos = s[abre..].find('>').map(|i| i + abre + 1)?;
            continue;
        }
        let fim_tag = s[abre..].find('>').map(|i| i + abre)?;
        let bruto = &s[abre + 1..fim_tag];
        if bruto.ends_with('/') {
            pos = fim_tag + 1;
            continue;
        }
        let nome: String = bruto
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        if nome.is_empty() {
            pos = fim_tag + 1;
            continue;
        }
        let fecha = format!("</{nome}>");
        let fim = s[fim_tag..].find(&fecha).map(|i| i + fim_tag)?;
        return Some((
            nome.clone(),
            s[fim_tag + 1..fim].to_string(),
            fim + fecha.len(),
        ));
    }
}

// ---------------------------------------------------- do texto para o valor
//
// Nos cinco formatos tudo e texto: o `1` de um CSV e a cadeia `"1"`, e nao o
// numero 1. Quem sabe que aquilo e um inteiro e o ESQUEMA, e a conversao tem
// de ser dirigida por ele.
//
// Esta parte mora no nucleo, e nao no servidor, porque a linha de comando
// tambem carrega arquivo -- e duas implementacoes da mesma conversao
// divergiriam no primeiro caso esquisito, que e justamente onde ela e usada.

use crate::datahora::dias_de_civil;
use crate::schema::Schema;
use crate::types::ColumnType;
use crate::uuid::{Uuid, Uuid256};
use crate::value::Value;

/// Decimal exato a partir do texto, ja escalado.
///
/// Texto e nao `f64` de proposito: `f64` nao representa 1,10 exatamente, e
/// dinheiro nao pode perder centavo no caminho.
pub fn texto_para_decimal(texto: &str, escala: u8) -> Result<i128> {
    let t = texto.trim();
    let (negativo, t) = match t.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let invalido = || PhxError::Tipo(format!("decimal invalido: {texto:?}"));
    let (inteiro, fracao) = match t.split_once('.') {
        Some((a, b)) => (a, b),
        None => (t, ""),
    };
    if inteiro.is_empty() && fracao.is_empty() {
        return Err(invalido());
    }
    if !inteiro.chars().all(|c| c.is_ascii_digit()) || !fracao.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalido());
    }
    // Mais casas do que a coluna tem seria perder centavo em silencio.
    if fracao.len() > escala as usize {
        return Err(PhxError::Tipo(format!(
            "{texto:?} tem {} casas decimais e a coluna tem {escala}",
            fracao.len()
        )));
    }
    let mut n: i128 = if inteiro.is_empty() {
        0
    } else {
        inteiro.parse().map_err(|_| invalido())?
    };
    for _ in 0..escala {
        n = n.checked_mul(10).ok_or_else(invalido)?;
    }
    if !fracao.is_empty() {
        let mut f: i128 = fracao.parse().map_err(|_| invalido())?;
        for _ in fracao.len()..escala as usize {
            f *= 10;
        }
        n += f;
    }
    Ok(if negativo { -n } else { n })
}

/// Le uma data em `AAAA-MM-DD` e devolve dias desde a epoca.
pub fn data_de_texto(t: &str) -> Result<i32> {
    let partes: Vec<&str> = t.trim().split('-').collect();
    let invalida = || PhxError::Tipo(format!("data invalida: {t:?} (use AAAA-MM-DD)"));
    if partes.len() != 3 {
        return Err(invalida());
    }
    let ano: i32 = partes[0].parse().map_err(|_| invalida())?;
    let mes: u32 = partes[1].parse().map_err(|_| invalida())?;
    let dia: u32 = partes[2].parse().map_err(|_| invalida())?;
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
        return Err(invalida());
    }
    Ok(dias_de_civil(ano, mes, dia))
}

/// Bytes a partir de hexadecimal.
pub fn hex_para_bytes(hex: &str) -> Result<Vec<u8>> {
    let t = hex.trim();
    if t.len() % 2 != 0 {
        return Err(PhxError::Tipo(
            "hexadecimal precisa ter quantidade par de digitos".into(),
        ));
    }
    (0..t.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|_| PhxError::Tipo(format!("hexadecimal invalido: {hex:?}")))
        })
        .collect()
}

/// Normaliza numero escrito a brasileira para a forma que o analisador come.
///
/// # A regra, e o defeito que ela conserta
///
/// Trocar toda virgula por ponto parece obvio e esta errado: `2.000,00` viraria
/// `2.000.00`, que nao e numero nenhum. Aqui o **ultimo separador manda** --
/// ele e o decimal, e o outro e milhar e sai fora.
///
/// | entra | sai |
/// |---|---|
/// | `1500,50` | `1500.50` |
/// | `1.500,50` | `1500.50` |
/// | `1,500.50` | `1500.50` |
/// | `1500.50` | `1500.50` |
/// | `1.500` | `1.500` -- ambiguo, e fica como esta |
///
/// A ultima linha e a decisao dificil: `1.500` pode ser mil e quinhentos ou um
/// e meio, e nao ha como saber. Fica como veio, e o tipo da coluna decide.
/// Adivinhar mudaria o valor de quem digitou certo.
pub fn numero_pt(t: &str) -> String {
    let ponto = t.rfind('.');
    let virgula = t.rfind(',');
    match (ponto, virgula) {
        (Some(p), Some(v)) if v > p => t.replace('.', "").replace(',', "."),
        (Some(p), Some(v)) if p > v => t.replace(',', ""),
        (None, Some(_)) => t.replace(',', "."),
        _ => t.to_string(),
    }
}

/// Converte um texto para o valor da coluna, dirigido pelo tipo dela.
///
/// **Campo vazio vira NULO**, e nao zero nem cadeia vazia: numa planilha a
/// celula em branco quer dizer «nao informado», e gravar zero num campo de
/// valor mudaria o dado.
pub fn valor_de_texto(t: &str, ty: &ColumnType) -> Result<Value> {
    let t = t.trim();
    if t.is_empty() {
        return Ok(Value::Null);
    }
    let erro = |esperado: &str| PhxError::Tipo(format!("esperado {esperado}, recebido {t:?}"));

    Ok(match ty {
        ColumnType::Bool => match t.to_ascii_lowercase().as_str() {
            "1" | "true" | "sim" | "s" | "verdadeiro" | "v" | "yes" | "y" => Value::Bool(true),
            "0" | "false" | "nao" | "não" | "n" | "falso" | "no" => Value::Bool(false),
            _ => return Err(erro("sim/nao, true/false ou 1/0")),
        },
        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8 => {
            Value::Int(t.parse::<i64>().map_err(|_| erro("inteiro"))?)
        }
        ColumnType::UInt1
        | ColumnType::UInt2
        | ColumnType::UInt4
        | ColumnType::UInt8
        | ColumnType::Sequence => {
            Value::UInt(t.parse::<u64>().map_err(|_| erro("inteiro sem sinal"))?)
        }
        ColumnType::Real4 | ColumnType::Real8 => {
            Value::Real(numero_pt(t).parse::<f64>().map_err(|_| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => {
            Value::Decimal(texto_para_decimal(&numero_pt(t), *escala)?)
        }
        ColumnType::Date => Value::Date(data_de_texto(t)?),
        // Hora e instante chegam em numero -- centesimos desde a meia-noite e
        // milissegundos desde a epoca. Texto de relogio (`14:30`) nao entra
        // ainda, e o erro diz o que se espera em vez de gravar zero.
        ColumnType::Time => Value::Time(t.parse::<i32>().map_err(|_| erro("hora em centesimos"))?),
        ColumnType::DateTime => Value::DateTime(
            t.parse::<i64>()
                .map_err(|_| erro("instante em milissegundos"))?,
        ),
        ColumnType::Uuid if t.eq_ignore_ascii_case("novo") || t.eq_ignore_ascii_case("v7") => {
            Value::Uuid(Uuid::v7())
        }
        ColumnType::Uuid if t.eq_ignore_ascii_case("v4") => Value::Uuid(Uuid::v4()),
        ColumnType::Uuid => Value::Uuid(Uuid::de_texto(t)?),
        ColumnType::Uuid256 if t.eq_ignore_ascii_case("novo") => {
            Value::Uuid256(Uuid256::aleatorio())
        }
        ColumnType::Uuid256 => Value::Uuid256(Uuid256::de_texto(t)?),
        ColumnType::Str(_) => Value::Str(t.to_string()),
        ColumnType::Memo => Value::Memo(t.to_string()),
        ColumnType::Bin => Value::Bin(hex_para_bytes(t)?),
    })
}

/// Uma linha da carga virada em valores, casando as colunas POR NOME.
///
/// Por nome, e nao por posicao: uma coluna a mais no meio do arquivo gravaria
/// tudo deslocado -- sem erro, porque os tipos costumam aceitar.
///
/// Coluna do arquivo que a tabela nao tem e ERRO, com o nome dela. Coluna da
/// tabela que o arquivo nao traz fica nula -- ou com o padrao, no caso das
/// colunas de sistema.
pub fn linha_de_texto(carga: &Carga, i: usize, esquema: &Schema) -> Result<Vec<Value>> {
    let linha = carga
        .linhas
        .get(i)
        .ok_or_else(|| PhxError::Tipo(format!("a carga nao tem a linha {}", i + 1)))?;
    for c in &carga.colunas {
        if esquema.coluna_por_nome(c).is_none() {
            return Err(PhxError::Tipo(format!(
                "coluna {c:?} nao existe em {}",
                esquema.nome()
            )));
        }
    }
    esquema
        .colunas()
        .iter()
        .map(
            |col| match carga.colunas.iter().position(|c| *c == col.nome) {
                Some(j) => valor_de_texto(linha.get(j).map(String::as_str).unwrap_or(""), &col.ty),
                None if col.nome == crate::schema::COLUNA_SOFTDELETED => Ok(Value::Bool(false)),
                None if col.nome == crate::schema::COLUNA_ROWNUM => Ok(Value::UInt(0)),
                None => Ok(Value::Null),
            },
        )
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c(colunas: &[&str], linhas: &[&[&str]]) -> Carga {
        Carga {
            colunas: colunas.iter().map(|s| s.to_string()).collect(),
            linhas: linhas
                .iter()
                .map(|l| l.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn json_lista_de_objetos() {
        let t = r#"[{"id":1,"nome":"Adriano"},{"id":2,"nome":"Maria"}]"#;
        assert_eq!(
            ler(t, Formato::Json).unwrap(),
            c(&["id", "nome"], &[&["1", "Adriano"], &["2", "Maria"]])
        );
    }

    /// O que o proprio `exportar` escreve entra sem editar.
    #[test]
    fn json_do_exportar_entra_direto() {
        let t = r#"{"tabela":"clientes","linhas":[{"id":1,"nome":"Ana"}]}"#;
        assert_eq!(
            ler(t, Formato::Json).unwrap(),
            c(&["id", "nome"], &[&["1", "Ana"]])
        );
    }

    #[test]
    fn json_com_chave_a_mais_e_recusado() {
        let t = r#"[{"id":1},{"id":2,"sobra":"x"}]"#;
        let e = ler(t, Formato::Json).unwrap_err();
        assert!(format!("{e}").contains("sobra"), "{e}");
    }

    #[test]
    fn csv_com_ponto_e_virgula_e_bom() {
        let t = "\u{feff}id;nome\n1;Adriano\n2;Maria\n";
        assert_eq!(
            ler(t, Formato::Csv).unwrap(),
            c(&["id", "nome"], &[&["1", "Adriano"], &["2", "Maria"]])
        );
    }

    #[test]
    fn csv_descobre_a_virgula() {
        let t = "id,nome\n1,Adriano\n";
        assert_eq!(
            ler(t, Formato::Csv).unwrap(),
            c(&["id", "nome"], &[&["1", "Adriano"]])
        );
    }

    /// O defeito classico do leitor escrito as pressas: um campo entre aspas
    /// que contem quebra de linha, partido em dois registros.
    #[test]
    fn campo_entre_aspas_com_quebra_de_linha() {
        let t = "id;obs\n1;\"linha um\nlinha dois\"\n2;curto\n";
        let carga = ler(t, Formato::Csv).unwrap();
        assert_eq!(carga.linhas.len(), 2);
        assert_eq!(carga.linhas[0][1], "linha um\nlinha dois");
        assert_eq!(carga.linhas[1][1], "curto");
    }

    #[test]
    fn aspas_dobradas_viram_uma() {
        let t = "id;nome\n1;\"o \"\"grande\"\" Adriano\"\n";
        assert_eq!(
            ler(t, Formato::Csv).unwrap().linhas[0][1],
            "o \"grande\" Adriano"
        );
    }

    /// Separador dentro de campo sem aspas e o erro mais comum de carga, e
    /// aceitar calado gravaria o texto partido em duas colunas.
    #[test]
    fn campo_a_mais_e_recusado_com_o_numero_da_linha() {
        let t = "id;nome\n1;Silva;Souza\n";
        let e = ler(t, Formato::Csv).unwrap_err();
        let texto = format!("{e}");
        assert!(texto.contains("linha 1"), "{texto}");
        assert!(texto.contains("aspas"), "{texto}");
    }

    #[test]
    fn txt_por_tabulacao() {
        let t = "id\tnome\n1\tAdriano\n";
        assert_eq!(
            ler(t, Formato::Txt).unwrap(),
            c(&["id", "nome"], &[&["1", "Adriano"]])
        );
    }

    #[test]
    fn html_com_thead_e_tbody() {
        let t = r#"<table><thead><tr><th>id</th><th>nome</th></tr></thead>
                   <tbody><tr><td>1</td><td class="x">Adriano</td></tr>
                   <tr><td>2</td><td><b>Maria</b></td></tr></tbody></table>"#;
        assert_eq!(
            ler(t, Formato::Html).unwrap(),
            c(&["id", "nome"], &[&["1", "Adriano"], &["2", "Maria"]])
        );
    }

    #[test]
    fn html_resolve_entidade() {
        let t =
            "<table><tr><th>nome</th></tr><tr><td>Silva &amp; Souza &lt;Lda&gt;</td></tr></table>";
        assert_eq!(
            ler(t, Formato::Html).unwrap().linhas[0][0],
            "Silva & Souza <Lda>"
        );
    }

    #[test]
    fn xml_de_elementos_repetidos() {
        let t = r#"<?xml version="1.0"?><linhas>
            <linha><id>1</id><nome>Adriano</nome></linha>
            <linha><id>2</id><nome>Maria</nome></linha></linhas>"#;
        assert_eq!(
            ler(t, Formato::Xml).unwrap(),
            c(&["id", "nome"], &[&["1", "Adriano"], &["2", "Maria"]])
        );
    }

    #[test]
    fn xml_com_comentario_e_elemento_vazio() {
        let t = r#"<raiz><!-- nota --><linha><id>1</id><vazio/><nome>Ana</nome></linha></raiz>"#;
        assert_eq!(
            ler(t, Formato::Xml).unwrap(),
            c(&["id", "nome"], &[&["1", "Ana"]])
        );
    }

    #[test]
    fn cabecalho_repetido_e_recusado() {
        let e = ler("id;id\n1;2\n", Formato::Csv).unwrap_err();
        assert!(format!("{e}").contains("duas vezes"), "{e}");
    }

    #[test]
    fn cabecalho_sem_nome_e_recusado() {
        let e = ler("id;;nome\n1;2;3\n", Formato::Csv).unwrap_err();
        assert!(format!("{e}").contains("sem nome"), "{e}");
    }

    #[test]
    fn adivinha_o_formato() {
        assert_eq!(adivinhar("  [{\"a\":1}]"), Formato::Json);
        assert_eq!(adivinhar("{\"linhas\":[]}"), Formato::Json);
        assert_eq!(adivinhar("id;nome\n1;a"), Formato::Csv);
        assert_eq!(adivinhar("id\tnome\n1\ta"), Formato::Txt);
        assert_eq!(
            adivinhar("<table><tr><td>1</td></tr></table>"),
            Formato::Html
        );
        assert_eq!(
            adivinhar("<?xml version=\"1.0\"?><linhas><l/></linhas>"),
            Formato::Xml
        );
        // Um XML que por acaso tem <tr> dentro e HTML: e o caso do XHTML.
        assert_eq!(
            adivinhar("<?xml version=\"1.0\"?><html><tr></tr></html>"),
            Formato::Html
        );
    }

    #[test]
    fn a_carga_vira_json_de_objetos() {
        let carga = ler("id;nome\n1;Ana\n", Formato::Csv).unwrap();
        let j = carga.para_json();
        let itens = j.lista().unwrap();
        assert_eq!(itens.len(), 1);
        assert_eq!(itens[0].texto_ou("nome", ""), "Ana");
        assert_eq!(itens[0].texto_ou("id", ""), "1");
    }

    #[test]
    fn carga_vazia_da_erro_util() {
        let e = ler("", Formato::Csv).unwrap_err();
        assert!(format!("{e}").contains("cabecalho"), "{e}");
    }
}

#[cfg(test)]
mod testes_texto_para_valor {
    use super::*;
    use crate::schema::{Column, Schema};

    /// Trocar toda virgula por ponto parece obvio e esta errado: `2.000,00`
    /// viraria `2.000.00`, que nao e numero. Este foi o defeito, e este e o
    /// teste que o prende.
    #[test]
    fn o_ultimo_separador_e_o_decimal() {
        assert_eq!(numero_pt("1500,50"), "1500.50");
        assert_eq!(numero_pt("1.500,50"), "1500.50");
        assert_eq!(numero_pt("2.000,00"), "2000.00");
        assert_eq!(numero_pt("1.234.567,89"), "1234567.89");
        // A americana, que tambem chega de planilha.
        assert_eq!(numero_pt("1,500.50"), "1500.50");
        assert_eq!(numero_pt("1,234,567.89"), "1234567.89");
        assert_eq!(numero_pt("1500"), "1500");
        assert_eq!(numero_pt("1500.50"), "1500.50");
        assert_eq!(numero_pt("-42,5"), "-42.5");
    }

    /// `1.500` e ambiguo -- mil e quinhentos, ou um e meio? Fica como veio, e
    /// o tipo da coluna decide. Adivinhar mudaria o valor de quem digitou
    /// certo, e nao ha como saber quem foi.
    #[test]
    fn o_ambiguo_fica_como_veio() {
        assert_eq!(numero_pt("1.500"), "1.500");
    }

    #[test]
    fn decimal_a_brasileira_atravessa() {
        let ty = ColumnType::Decimal {
            precisao: 15,
            escala: 2,
        };
        assert_eq!(
            valor_de_texto("2.000,00", &ty).unwrap(),
            Value::Decimal(200_000)
        );
        assert_eq!(
            valor_de_texto("1500,50", &ty).unwrap(),
            Value::Decimal(150_050)
        );
        assert_eq!(
            valor_de_texto("-1.234,56", &ty).unwrap(),
            Value::Decimal(-123_456)
        );
    }

    /// Celula em branco de planilha quer dizer «nao informado», e nao zero.
    #[test]
    fn campo_vazio_vira_nulo_e_nao_zero() {
        let ty = ColumnType::Decimal {
            precisao: 15,
            escala: 2,
        };
        assert_eq!(valor_de_texto("", &ty).unwrap(), Value::Null);
        assert_eq!(valor_de_texto("   ", &ty).unwrap(), Value::Null);
        assert_eq!(valor_de_texto("", &ColumnType::Int8).unwrap(), Value::Null);
    }

    #[test]
    fn booleano_em_portugues() {
        for (t, esperado) in [
            ("sim", true),
            ("S", true),
            ("1", true),
            ("true", true),
            ("verdadeiro", true),
            ("nao", false),
            ("não", false),
            ("0", false),
            ("false", false),
            ("falso", false),
        ] {
            assert_eq!(
                valor_de_texto(t, &ColumnType::Bool).unwrap(),
                Value::Bool(esperado),
                "{t}"
            );
        }
        assert!(valor_de_texto("talvez", &ColumnType::Bool).is_err());
    }

    #[test]
    fn data_em_iso() {
        assert!(matches!(
            valor_de_texto("2026-08-28", &ColumnType::Date),
            Ok(Value::Date(_))
        ));
        let e = valor_de_texto("28/08/2026", &ColumnType::Date).unwrap_err();
        assert!(format!("{e}").contains("AAAA-MM-DD"), "{e}");
    }

    /// As colunas casam POR NOME. Uma coluna a mais no meio do arquivo gravaria
    /// tudo deslocado se o casamento fosse por posicao -- e sem erro, porque os
    /// tipos costumam aceitar.
    #[test]
    fn casa_por_nome_e_nao_por_posicao() {
        let e = Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(20)).obrigatoria(),
            ],
            vec![],
        )
        .unwrap();
        // O arquivo traz as colunas TROCADAS de ordem.
        let carga = ler("nome;id\nAdriano;7\n", Formato::Csv).unwrap();
        let l = linha_de_texto(&carga, 0, &e).unwrap();
        assert_eq!(l[0], Value::Int(7));
        assert_eq!(l[1], Value::Str("Adriano".into()));
        // E as duas de sistema entraram com o padrao.
        assert_eq!(l[2], Value::Bool(false));
        assert_eq!(l[3], Value::UInt(0));
    }

    #[test]
    fn coluna_que_a_tabela_nao_tem_e_erro_com_o_nome() {
        let e = Schema::new(
            "t",
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![],
        )
        .unwrap();
        let carga = ler("id;sobra\n1;x\n", Formato::Csv).unwrap();
        let err = linha_de_texto(&carga, 0, &e).unwrap_err();
        assert!(format!("{err}").contains("sobra"), "{err}");
    }

    #[test]
    fn coluna_que_falta_fica_nula() {
        let e = Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cidade", ColumnType::Str(20)),
            ],
            vec![],
        )
        .unwrap();
        let carga = ler("id\n1\n", Formato::Csv).unwrap();
        let l = linha_de_texto(&carga, 0, &e).unwrap();
        assert_eq!(l[0], Value::Int(1));
        assert_eq!(l[1], Value::Null);
    }
}
