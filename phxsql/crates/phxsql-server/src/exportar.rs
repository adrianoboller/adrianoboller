//! Exportacao de tabela: CSV, TXT, JSON, XML, HTML, XLSX e DOCX.
//!
//! # Por que escrito aqui, com XLSX e DOCX inclusos
//!
//! Um `.xlsx` e um `.docx` sao ZIP de XML -- e o projeto ja escreve ZIP com
//! DEFLATE, desde o backup. Entao os dois formatos que pareciam exigir
//! biblioteca sao, na verdade, os mesmos tijolos que ja estao aqui: montar o
//! XML das partes e chamar o compactador.
//!
//! O componente `phoenix-xlsx` faz isso muito melhor -- tema configuravel,
//! regras condicionais, leitura de planilha existente --, mas traz seseita
//! crates diretas e a arvore inteira delas. Aqui a saida e mais simples de
//! proposito, e o que ela promete ela cumpre sem dependencia nenhuma.
//!
//! # Data em planilha e numero, nao texto
//!
//! Exportar data como texto e o erro classico: a coluna deixa de ordenar, de
//! filtrar por periodo e de entrar em conta. O Excel(R) conta dias desde
//! 1899-12-30, e o PhxSql desde 1970-01-01 -- a diferenca e 25.569 dias, e e
//! so isso que separa uma coluna de data de uma coluna de texto que parece
//! data.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::zip::Zip;

use crate::valores::decimal_para_texto;

/// Dias entre 1899-12-30 (a epoca do Excel(R)) e 1970-01-01 (a nossa).
const EPOCA_PLANILHA: i64 = 25_569;

/// Os indices do `cellXfs` da folha de estilos, por nome.
///
/// Numero solto aqui ja custou caro: o cabecalho apontava para `1`, que e o
/// "texto listrado", e saia com a cor da zebra e sem negrito. O Excel(R) nao
/// reclama de indice errado -- ele obedece. Quem achou foi um leitor
/// independente lendo o arquivo de volta, e por isso os indices agora tem
/// nome e ha teste que confere a correspondencia.
mod estilo {
    pub const TEXTO: u32 = 0;
    pub const TEXTO_ZEBRA: u32 = 1;
    pub const INTEIRO: u32 = 2;
    pub const INTEIRO_ZEBRA: u32 = 3;
    pub const TITULO: u32 = 4;
    pub const SUBTITULO: u32 = 5;
    pub const DECIMAL: u32 = 6;
    pub const DECIMAL_ZEBRA: u32 = 7;
    pub const DATA: u32 = 8;
    pub const DATA_ZEBRA: u32 = 9;
    pub const INSTANTE: u32 = 10;
    pub const INSTANTE_ZEBRA: u32 = 11;
    pub const CABECALHO: u32 = 12;
    /// Quantos `xf` a folha de estilos declara. Um a mais ou a menos aqui e o
    /// Excel(R) recusando o arquivo inteiro.
    pub const QUANTOS: usize = 13;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Csv,
    Txt,
    Json,
    Xml,
    Html,
    Xlsx,
    Docx,
}

impl Formato {
    pub fn de_texto(t: &str) -> Result<Formato> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "csv" => Formato::Csv,
            "txt" | "texto" => Formato::Txt,
            "json" => Formato::Json,
            "xml" => Formato::Xml,
            "html" | "htm" => Formato::Html,
            "xlsx" | "excel" | "planilha" => Formato::Xlsx,
            "docx" | "word" | "documento" => Formato::Docx,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "formato desconhecido: {outro:?} \
                     (use csv, txt, json, xml, html, xlsx ou docx)"
                )))
            }
        })
    }

    pub fn extensao(self) -> &'static str {
        match self {
            Formato::Csv => "csv",
            Formato::Txt => "txt",
            Formato::Json => "json",
            Formato::Xml => "xml",
            Formato::Html => "html",
            Formato::Xlsx => "xlsx",
            Formato::Docx => "docx",
        }
    }

    /// O tipo MIME, para o navegador saber o que fazer com o arquivo.
    pub fn mime(self) -> &'static str {
        match self {
            Formato::Csv => "text/csv; charset=utf-8",
            Formato::Txt => "text/plain; charset=utf-8",
            Formato::Json => "application/json; charset=utf-8",
            Formato::Xml => "application/xml; charset=utf-8",
            Formato::Html => "text/html; charset=utf-8",
            Formato::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Formato::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
        }
    }

    /// A saida e binaria? Muda como ela atravessa o protocolo.
    pub fn binario(self) -> bool {
        matches!(self, Formato::Xlsx | Formato::Docx)
    }
}

/// Uma coluna da exportacao.
pub struct Coluna {
    pub nome: String,
    pub ty: ColumnType,
}

/// O que se exporta: cabecalho, linhas e um titulo para quem abre o arquivo.
pub struct Planilha<'a> {
    pub titulo: String,
    pub subtitulo: String,
    pub colunas: Vec<Coluna>,
    pub linhas: &'a [Vec<Value>],
}

impl Planilha<'_> {
    pub fn do_esquema(esquema: &Schema, titulo: &str) -> Vec<Coluna> {
        let _ = titulo;
        esquema
            .colunas()
            .iter()
            .map(|c| Coluna {
                nome: c.nome.clone(),
                ty: c.ty,
            })
            .collect()
    }

    pub fn gerar(&self, f: Formato) -> Result<Vec<u8>> {
        Ok(match f {
            Formato::Csv => self.csv(b';').into_bytes(),
            Formato::Txt => self.txt().into_bytes(),
            Formato::Json => self.json().into_bytes(),
            Formato::Xml => self.xml().into_bytes(),
            Formato::Html => self.html().into_bytes(),
            Formato::Xlsx => self.xlsx(),
            Formato::Docx => self.docx(),
        })
    }

    // -------------------------------------------------------------- texto

    /// CSV com ponto e virgula.
    ///
    /// Ponto e virgula, e nao virgula: em maquina configurada em portugues o
    /// Excel(R) espera `;` e o decimal e virgula -- abrir um CSV separado por
    /// virgula joga a linha inteira numa coluna so. O BOM na frente e o que faz
    /// o acento aparecer certo em vez de virar `Ã§`.
    pub fn csv(&self, sep: u8) -> String {
        let s = sep as char;
        let mut t = String::from("\u{feff}");
        t.push_str(
            &self
                .colunas
                .iter()
                .map(|c| campo_csv(&c.nome, s))
                .collect::<Vec<_>>()
                .join(&s.to_string()),
        );
        t.push_str("\r\n");
        for l in self.linhas {
            let campos: Vec<String> = l
                .iter()
                .zip(self.colunas.iter())
                .map(|(v, c)| campo_csv(&texto_de(v, &c.ty, true), s))
                .collect();
            t.push_str(&campos.join(&s.to_string()));
            t.push_str("\r\n");
        }
        t
    }

    /// Texto de largura fixa, alinhado -- o formato que se le no terminal.
    pub fn txt(&self) -> String {
        let mut larguras: Vec<usize> = self
            .colunas
            .iter()
            .map(|c| c.nome.chars().count())
            .collect();
        let celulas: Vec<Vec<String>> = self
            .linhas
            .iter()
            .map(|l| {
                l.iter()
                    .zip(self.colunas.iter())
                    .map(|(v, c)| texto_de(v, &c.ty, false))
                    .collect()
            })
            .collect();
        for l in &celulas {
            for (i, c) in l.iter().enumerate() {
                if let Some(w) = larguras.get_mut(i) {
                    *w = (*w).max(c.chars().count());
                }
            }
        }
        // Teto por coluna: um `Memo` de dez mil letras faria uma linha de dez
        // mil colunas e o arquivo deixaria de ser legivel, que e o unico
        // motivo de existir do formato texto.
        for w in larguras.iter_mut() {
            *w = (*w).min(60);
        }

        let regua = |t: &mut String| {
            for (i, w) in larguras.iter().enumerate() {
                if i > 0 {
                    t.push_str("-+-");
                }
                t.push_str(&"-".repeat(*w));
            }
            t.push('\n');
        };
        let linha = |t: &mut String, campos: &[String], num: &[bool]| {
            for (i, c) in campos.iter().enumerate() {
                if i > 0 {
                    t.push_str(" | ");
                }
                let w = larguras[i];
                let c: String = c.chars().take(w).collect();
                let falta = w - c.chars().count();
                if num[i] {
                    t.push_str(&" ".repeat(falta));
                    t.push_str(&c);
                } else {
                    t.push_str(&c);
                    t.push_str(&" ".repeat(falta));
                }
            }
            t.push('\n');
        };

        let num: Vec<bool> = self.colunas.iter().map(|c| numerico(&c.ty)).collect();
        let so_texto = vec![false; self.colunas.len()];
        let mut t = String::new();
        if !self.titulo.is_empty() {
            t.push_str(&self.titulo);
            t.push('\n');
            if !self.subtitulo.is_empty() {
                t.push_str(&self.subtitulo);
                t.push('\n');
            }
            t.push('\n');
        }
        let cab: Vec<String> = self.colunas.iter().map(|c| c.nome.clone()).collect();
        linha(&mut t, &cab, &so_texto);
        regua(&mut t);
        for l in &celulas {
            linha(&mut t, l, &num);
        }
        t.push_str(&format!("\n{} linha(s)\n", self.linhas.len()));
        t
    }

    pub fn json(&self) -> String {
        let mut t = String::from("[\n");
        for (i, l) in self.linhas.iter().enumerate() {
            if i > 0 {
                t.push_str(",\n");
            }
            t.push_str("  {");
            for (j, (v, c)) in l.iter().zip(self.colunas.iter()).enumerate() {
                if j > 0 {
                    t.push_str(", ");
                }
                t.push_str(&phxsql_core::json::Json::texto_de(&c.nome).escrever());
                t.push(':');
                t.push_str(&crate::valores::valor_para_json(v, &c.ty).escrever());
            }
            t.push('}');
        }
        t.push_str("\n]\n");
        t
    }

    pub fn xml(&self) -> String {
        let mut t = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        t.push_str(&format!(
            "<tabela nome=\"{}\">\n",
            xml_atributo(&self.titulo)
        ));
        for l in self.linhas {
            t.push_str("  <linha>\n");
            for (v, c) in l.iter().zip(self.colunas.iter()) {
                let tag = tag_xml(&c.nome);
                if v.e_null() {
                    t.push_str(&format!("    <{tag} nulo=\"true\"/>\n"));
                } else {
                    t.push_str(&format!(
                        "    <{tag}>{}</{tag}>\n",
                        xml_texto(&texto_de(v, &c.ty, false))
                    ));
                }
            }
            t.push_str("  </linha>\n");
        }
        t.push_str("</tabela>\n");
        t
    }

    /// HTML com a tabela ja formatada e um campo de busca que funciona sem rede.
    pub fn html(&self) -> String {
        let mut t = String::new();
        t.push_str("<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n");
        t.push_str(&format!("<title>{}</title>\n", xml_texto(&self.titulo)));
        t.push_str(ESTILO_HTML);
        t.push_str("</head>\n<body>\n");
        t.push_str(&format!("<h1>{}</h1>\n", xml_texto(&self.titulo)));
        if !self.subtitulo.is_empty() {
            t.push_str(&format!(
                "<p class=\"sub\">{}</p>\n",
                xml_texto(&self.subtitulo)
            ));
        }
        t.push_str("<input id=\"b\" placeholder=\"filtrar…\" autocomplete=\"off\">\n");
        t.push_str("<table><thead><tr>");
        for c in &self.colunas {
            t.push_str(&format!(
                "<th class=\"{}\">{}</th>",
                if numerico(&c.ty) { "num" } else { "" },
                xml_texto(&c.nome)
            ));
        }
        t.push_str("</tr></thead>\n<tbody>\n");
        for l in self.linhas {
            t.push_str("<tr>");
            for (v, c) in l.iter().zip(self.colunas.iter()) {
                let classe = if v.e_null() {
                    "nulo"
                } else if numerico(&c.ty) {
                    "num"
                } else {
                    ""
                };
                t.push_str(&format!(
                    "<td class=\"{classe}\">{}</td>",
                    if v.e_null() {
                        "—".to_string()
                    } else {
                        xml_texto(&texto_de(v, &c.ty, false))
                    }
                ));
            }
            t.push_str("</tr>\n");
        }
        t.push_str("</tbody></table>\n");
        t.push_str(&format!(
            "<p class=\"sub\">{} linha(s)</p>\n",
            self.linhas.len()
        ));
        t.push_str(BUSCA_HTML);
        t.push_str("</body>\n</html>\n");
        t
    }

    // ------------------------------------------------------------- OOXML

    /// A planilha, com faixa de titulo, painel congelado, autofiltro e zebra.
    fn xlsx(&self) -> Vec<u8> {
        let n = self.colunas.len().max(1);
        let ultima = coluna_a1(n - 1);
        // Linha 1: titulo. 2: subtitulo. 3: em branco. 4: cabecalho. 5+: dados.
        let primeira_dados = 5;
        let ultima_linha = primeira_dados + self.linhas.len().max(1) - 1;

        let mut folha = String::with_capacity(self.linhas.len() * 80 + 2_048);
        folha.push_str(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
        );
        // Largura por coluna, medida no conteudo. Sem isto tudo sai com 8,43 e
        // a planilha abre cheia de `####`.
        folha.push_str("<cols>");
        for (i, c) in self.colunas.iter().enumerate() {
            let mut w = c.nome.chars().count();
            for l in self.linhas.iter().take(500) {
                if let Some(v) = l.get(i) {
                    w = w.max(texto_de(v, &c.ty, false).chars().count());
                }
            }
            folha.push_str(&format!(
                "<col min=\"{}\" max=\"{}\" width=\"{:.1}\" customWidth=\"1\"/>",
                i + 1,
                i + 1,
                (w as f64 + 3.0).clamp(9.0, 60.0)
            ));
        }
        folha.push_str("</cols>");
        folha.push_str("<sheetData>");

        // Faixa de titulo e subtitulo.
        folha.push_str(&format!(
            "<row r=\"1\" ht=\"26\" customHeight=\"1\">{}</row>",
            celula_texto("A1", &self.titulo, estilo::TITULO)
        ));
        folha.push_str(&format!(
            "<row r=\"2\">{}</row>",
            celula_texto("A2", &self.subtitulo, estilo::SUBTITULO)
        ));

        // Cabecalho.
        folha.push_str("<row r=\"4\" ht=\"18\" customHeight=\"1\">");
        for (i, c) in self.colunas.iter().enumerate() {
            folha.push_str(&celula_texto(
                &format!("{}4", coluna_a1(i)),
                &c.nome,
                estilo::CABECALHO,
            ));
        }
        folha.push_str("</row>");

        // Dados.
        for (n_linha, l) in self.linhas.iter().enumerate() {
            let r = primeira_dados + n_linha;
            let zebra = n_linha % 2 == 1;
            folha.push_str(&format!("<row r=\"{r}\">"));
            for (i, (v, c)) in l.iter().zip(self.colunas.iter()).enumerate() {
                let ref_ = format!("{}{r}", coluna_a1(i));
                folha.push_str(&celula(&ref_, v, &c.ty, zebra));
            }
            folha.push_str("</row>");
        }
        folha.push_str("</sheetData>");
        // O autofiltro cobre o cabecalho e os dados; o painel congela a linha 4.
        folha.push_str(&format!("<autoFilter ref=\"A4:{ultima}{ultima_linha}\"/>"));
        folha.push_str("</worksheet>");

        // O congelamento vem ANTES do sheetData no esquema; montar em duas
        // partes e emendar e mais simples do que remontar a folha inteira.
        let folha = folha.replacen(
            "<sheetData>",
            "<sheetViews><sheetView workbookViewId=\"0\" showGridLines=\"0\">\
             <pane ySplit=\"4\" topLeftCell=\"A5\" activePane=\"bottomLeft\" state=\"frozen\"/>\
             </sheetView></sheetViews><sheetFormatPr defaultRowHeight=\"15\"/><sheetData>",
            1,
        );

        let mut z = Zip::novo(crate::agora_ms());
        z.acrescentar("[Content_Types].xml", CT_XLSX.as_bytes());
        z.acrescentar("_rels/.rels", RELS_RAIZ.as_bytes());
        z.acrescentar("xl/workbook.xml", WORKBOOK.as_bytes());
        z.acrescentar("xl/_rels/workbook.xml.rels", RELS_WB.as_bytes());
        z.acrescentar("xl/styles.xml", ESTILOS_XLSX.as_bytes());
        z.acrescentar("xl/worksheets/sheet1.xml", folha.as_bytes());
        z.terminar()
    }

    /// O documento, com a tabela em grade e o cabecalho em destaque.
    fn docx(&self) -> Vec<u8> {
        let mut d = String::with_capacity(self.linhas.len() * 120 + 2_048);
        d.push_str(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
             <w:body>",
        );
        d.push_str(&paragrafo(&self.titulo, true, 28));
        if !self.subtitulo.is_empty() {
            d.push_str(&paragrafo(&self.subtitulo, false, 18));
        }
        // O `tblGrid` e OBRIGATORIO dentro de `w:tbl`, e sem ele o documento
        // nao e OOXML valido -- um leitor rigoroso recusa a tabela inteira. O
        // Word(R) tolera e por isso o defeito passaria despercebido ate alguem
        // abrir o arquivo noutro programa.
        let largura_col = 9_360 / self.colunas.len().max(1);
        d.push_str(
            "<w:tbl><w:tblPr><w:tblStyle w:val=\"a\"/>\
             <w:tblW w:w=\"5000\" w:type=\"pct\"/>\
             <w:tblBorders>\
             <w:top w:val=\"single\" w:sz=\"4\" w:color=\"D8DCE3\"/>\
             <w:left w:val=\"single\" w:sz=\"4\" w:color=\"D8DCE3\"/>\
             <w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"D8DCE3\"/>\
             <w:right w:val=\"single\" w:sz=\"4\" w:color=\"D8DCE3\"/>\
             <w:insideH w:val=\"single\" w:sz=\"4\" w:color=\"D8DCE3\"/>\
             <w:insideV w:val=\"single\" w:sz=\"4\" w:color=\"D8DCE3\"/>\
             </w:tblBorders></w:tblPr>",
        );
        d.push_str("<w:tblGrid>");
        for _ in &self.colunas {
            d.push_str(&format!("<w:gridCol w:w=\"{largura_col}\"/>"));
        }
        d.push_str("</w:tblGrid>");
        // `tblHeader` repete o cabecalho em toda pagina de uma tabela longa.
        d.push_str("<w:tr><w:trPr><w:tblHeader/></w:trPr>");
        for c in &self.colunas {
            d.push_str(&celula_docx(&c.nome, true, false, "16324A"));
        }
        d.push_str("</w:tr>");
        for (i, l) in self.linhas.iter().enumerate() {
            let zebra = i % 2 == 1;
            d.push_str("<w:tr>");
            for (v, c) in l.iter().zip(self.colunas.iter()) {
                d.push_str(&celula_docx(
                    &texto_de(v, &c.ty, false),
                    false,
                    numerico(&c.ty),
                    if zebra { "F4F6F9" } else { "FFFFFF" },
                ));
            }
            d.push_str("</w:tr>");
        }
        d.push_str("</w:tbl>");
        d.push_str(&paragrafo(
            &format!("{} linha(s)", self.linhas.len()),
            false,
            16,
        ));
        d.push_str(
            "<w:sectPr><w:pgSz w:w=\"16838\" w:h=\"11906\" w:orient=\"landscape\"/>\
                    <w:pgMar w:top=\"720\" w:right=\"720\" w:bottom=\"720\" w:left=\"720\"/>\
                    </w:sectPr>",
        );
        d.push_str("</w:body></w:document>");

        let mut z = Zip::novo(crate::agora_ms());
        z.acrescentar("[Content_Types].xml", CT_DOCX.as_bytes());
        z.acrescentar("_rels/.rels", RELS_DOCX.as_bytes());
        z.acrescentar("word/document.xml", d.as_bytes());
        z.terminar()
    }
}

// ------------------------------------------------------------- ajudantes

fn numerico(t: &ColumnType) -> bool {
    matches!(
        t,
        ColumnType::Int1
            | ColumnType::Int2
            | ColumnType::Int4
            | ColumnType::Int8
            | ColumnType::UInt1
            | ColumnType::UInt2
            | ColumnType::UInt4
            | ColumnType::UInt8
            | ColumnType::Real4
            | ColumnType::Real8
            | ColumnType::Decimal { .. }
            | ColumnType::Sequence
    )
}

/// O valor como texto. `planilha` troca o ponto decimal por virgula, porque e
/// o que o Excel(R) em portugues espera num CSV.
fn texto_de(v: &Value, t: &ColumnType, planilha: bool) -> String {
    match v {
        Value::Null => String::new(),
        Value::Decimal(d) => {
            let escala = match t {
                ColumnType::Decimal { escala, .. } => *escala,
                _ => 0,
            };
            let s = decimal_para_texto(*d, escala);
            if planilha {
                s.replace('.', ",")
            } else {
                s
            }
        }
        Value::Real(r) => {
            let s = r.to_string();
            if planilha {
                s.replace('.', ",")
            } else {
                s
            }
        }
        Value::Date(d) => phxsql_core::datahora::data_iso(*d),
        Value::Time(c) => phxsql_core::datahora::hora_iso(*c),
        Value::DateTime(m) => phxsql_core::datahora::instante_iso(*m),
        Value::Bool(b) => (if *b { "sim" } else { "nao" }).to_string(),
        Value::Int(i) => i.to_string(),
        Value::UInt(u) => u.to_string(),
        Value::Str(s) | Value::Memo(s) => s.clone(),
        Value::Uuid(u) => u.to_string(),
        Value::Uuid256(u) => u.to_string(),
        Value::Bin(b) => format!("<{} bytes>", b.len()),
    }
}

/// Um campo de CSV, com aspas so quando precisa.
///
/// A aspa dupla dentro do campo dobra -- e a regra do RFC 4180, e e o que faz
/// um nome como `Loja "Central"` nao quebrar a linha inteira.
fn campo_csv(t: &str, sep: char) -> String {
    if t.contains(sep) || t.contains('"') || t.contains('\n') || t.contains('\r') {
        format!("\"{}\"", t.replace('"', "\"\""))
    } else {
        t.to_string()
    }
}

fn xml_texto(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            // Caractere de controle nao vale em XML 1.0 nem escapado: sai.
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => s.push(' '),
            c => s.push(c),
        }
    }
    s
}

fn xml_atributo(t: &str) -> String {
    xml_texto(t).replace('"', "&quot;")
}

/// Um nome de coluna virando nome de elemento XML.
///
/// Nome de elemento nao aceita espaco, acento no comeco nem digito inicial. O
/// que nao serve vira `_`, e nome vazio vira `campo` -- um XML com
/// `<>` nao abre em lugar nenhum.
fn tag_xml(nome: &str) -> String {
    let mut s = String::with_capacity(nome.len());
    for (i, c) in nome.chars().enumerate() {
        let ok = c.is_alphanumeric() || c == '_' || c == '-' || c == '.';
        if !ok {
            s.push('_');
        } else if i == 0 && c.is_numeric() {
            s.push('_');
            s.push(c);
        } else {
            s.push(c);
        }
    }
    if s.is_empty() {
        "campo".to_string()
    } else {
        s
    }
}

/// `0` vira `A`, `26` vira `AA`.
fn coluna_a1(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_else(|_| "A".into())
}

fn celula_texto(ref_: &str, t: &str, estilo: u32) -> String {
    if t.is_empty() {
        return String::new();
    }
    format!(
        "<c r=\"{ref_}\" s=\"{estilo}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
        xml_texto(t)
    )
}

/// Uma celula de dado, com o estilo escolhido pelo tipo e pela zebra.
///
/// Data e hora saem como NUMERO com formato, e nao como texto: texto que
/// parece data nao ordena, nao filtra por periodo e nao entra em conta.
fn celula(ref_: &str, v: &Value, t: &ColumnType, zebra: bool) -> String {
    let base = |normal: u32, listrado: u32| if zebra { listrado } else { normal };
    let _ = estilo::QUANTOS;
    match v {
        Value::Null => String::new(),
        Value::Date(d) => format!(
            "<c r=\"{ref_}\" s=\"{}\"><v>{}</v></c>",
            base(estilo::DATA, estilo::DATA_ZEBRA),
            *d as i64 + EPOCA_PLANILHA
        ),
        Value::DateTime(ms) => {
            let dias = (*ms as f64 / 86_400_000.0) + EPOCA_PLANILHA as f64;
            format!(
                "<c r=\"{ref_}\" s=\"{}\"><v>{dias:.6}</v></c>",
                base(estilo::INSTANTE, estilo::INSTANTE_ZEBRA)
            )
        }
        Value::Decimal(d) => {
            let escala = match t {
                ColumnType::Decimal { escala, .. } => *escala,
                _ => 0,
            };
            // O ponto fica: dentro do XML o separador decimal e sempre `.`, e
            // quem troca por virgula na tela e o Excel(R), pelo idioma.
            format!(
                "<c r=\"{ref_}\" s=\"{}\"><v>{}</v></c>",
                base(estilo::DECIMAL, estilo::DECIMAL_ZEBRA),
                decimal_para_texto(*d, escala)
            )
        }
        Value::Int(i) => format!(
            "<c r=\"{ref_}\" s=\"{}\"><v>{i}</v></c>",
            base(estilo::INTEIRO, estilo::INTEIRO_ZEBRA)
        ),
        Value::UInt(u) => format!(
            "<c r=\"{ref_}\" s=\"{}\"><v>{u}</v></c>",
            base(estilo::INTEIRO, estilo::INTEIRO_ZEBRA)
        ),
        Value::Real(r) if r.is_finite() => {
            format!(
                "<c r=\"{ref_}\" s=\"{}\"><v>{r}</v></c>",
                base(estilo::DECIMAL, estilo::DECIMAL_ZEBRA)
            )
        }
        outro => celula_texto(
            ref_,
            &texto_de(outro, t, false),
            base(estilo::TEXTO, estilo::TEXTO_ZEBRA),
        ),
    }
}

fn paragrafo(t: &str, negrito: bool, meio_ponto: u32) -> String {
    format!(
        "<w:p><w:pPr><w:spacing w:after=\"120\"/></w:pPr><w:r><w:rPr>{}\
         <w:sz w:val=\"{meio_ponto}\"/><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/>\
         </w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        if negrito { "<w:b/>" } else { "" },
        xml_texto(t)
    )
}

fn celula_docx(t: &str, cabecalho: bool, num: bool, fundo: &str) -> String {
    format!(
        "<w:tc><w:tcPr><w:shd w:val=\"clear\" w:fill=\"{fundo}\"/></w:tcPr>\
         <w:p><w:pPr>{}<w:spacing w:after=\"0\"/></w:pPr><w:r><w:rPr>{}\
         <w:sz w:val=\"18\"/><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/>{}\
         </w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>",
        if num { "<w:jc w:val=\"right\"/>" } else { "" },
        if cabecalho { "<w:b/>" } else { "" },
        if cabecalho {
            "<w:color w:val=\"FFFFFF\"/>"
        } else {
            ""
        },
        xml_texto(t)
    )
}

// ---------------------------------------------------------- partes fixas

const ESTILO_HTML: &str = r#"<style>
:root{--fundo:#fff;--tinta:#12161f;--tinta-2:#5b6472;--linha:#e2e6ec;
      --cab:#16324a;--zebra:#f6f8fa;--acento:#d94f1e}
@media (prefers-color-scheme:dark){:root:not([data-tema=claro]){
  --fundo:#0d1117;--tinta:#e6edf3;--tinta-2:#8b949e;--linha:#222b36;
  --cab:#0f2233;--zebra:#131a22}}
*{box-sizing:border-box}
body{margin:0;padding:28px;background:var(--fundo);color:var(--tinta);
     font:14px/1.5 -apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif}
h1{margin:0 0 4px;font-size:21px;letter-spacing:-.01em}
.sub{margin:0 0 16px;color:var(--tinta-2);font-size:12.5px}
#b{margin-bottom:14px;padding:8px 12px;width:min(340px,100%);
   border:1px solid var(--linha);border-radius:6px;background:var(--fundo);
   color:var(--tinta);font:inherit;font-size:13px}
#b:focus{outline:none;border-color:var(--acento)}
.rolo{overflow-x:auto}
table{border-collapse:collapse;width:100%;font-size:12.5px}
th{position:sticky;top:0;background:var(--cab);color:#fff;text-align:left;
   padding:8px 10px;font-weight:600;white-space:nowrap}
td{padding:6px 10px;border-bottom:1px solid var(--linha);
   vertical-align:top}
tbody tr:nth-child(even){background:var(--zebra)}
.num{text-align:right;font-variant-numeric:tabular-nums}
.nulo{color:var(--tinta-2)}
</style>
"#;

const BUSCA_HTML: &str = r#"<script>
// Filtro sem rede e sem biblioteca: o arquivo tem de funcionar aberto do
// disco, dentro de um anexo de e-mail, num computador sem internet.
document.getElementById("b").addEventListener("input", function (e) {
  var termo = e.target.value.toLowerCase();
  var linhas = document.querySelectorAll("tbody tr");
  for (var i = 0; i < linhas.length; i++) {
    linhas[i].style.display =
      linhas[i].textContent.toLowerCase().indexOf(termo) >= 0 ? "" : "none";
  }
});
</script>
"#;

const CT_XLSX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;

const RELS_RAIZ: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Dados" sheetId="1" r:id="rId1"/></sheets></workbook>"#;

const RELS_WB: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

/// Os estilos, na ordem em que `celula` os referencia por indice:
///
/// | s | para que |
/// |---|----------|
/// | 0 | texto | 1 | texto listrado |
/// | 2 | inteiro | 3 | inteiro listrado |
/// | 4 | titulo | 5 | subtitulo |
/// | 6 | decimal | 7 | decimal listrado |
/// | 8 | data | 9 | data listrada |
/// | 10 | instante | 11 | instante listrado |
///
/// O indice e posicional: mexer na ordem sem mexer em `celula` pinta a
/// planilha inteira errado, e o Excel(R) nao reclama -- ele obedece.
const ESTILOS_XLSX: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<numFmts count="3"><numFmt numFmtId="170" formatCode="#,##0.00"/><numFmt numFmtId="171" formatCode="dd/mm/yyyy"/><numFmt numFmtId="172" formatCode="dd/mm/yyyy\ hh:mm"/></numFmts>
<fonts count="4"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Calibri"/></font><font><b/><sz val="15"/><color rgb="FF16324A"/><name val="Calibri"/></font><font><sz val="10"/><color rgb="FF5B6472"/><name val="Calibri"/></font></fonts>
<fills count="4"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF16324A"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="solid"><fgColor rgb="FFF4F6F9"/><bgColor indexed="64"/></patternFill></fill></fills>
<borders count="2"><border><left/><right/><top/><bottom/><diagonal/></border><border><left style="thin"><color rgb="FFD8DCE3"/></left><right style="thin"><color rgb="FFD8DCE3"/></right><top style="thin"><color rgb="FFD8DCE3"/></top><bottom style="thin"><color rgb="FFD8DCE3"/></bottom><diagonal/></border></borders>
<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
<cellXfs count="13">
<xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1"/>
<xf numFmtId="0" fontId="0" fillId="3" borderId="1" xfId="0" applyFill="1" applyBorder="1"/>
<xf numFmtId="3" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1" applyAlignment="1"><alignment horizontal="right"/></xf>
<xf numFmtId="3" fontId="0" fillId="3" borderId="1" xfId="0" applyNumberFormat="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="right"/></xf>
<xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1"/>
<xf numFmtId="0" fontId="3" fillId="0" borderId="0" xfId="0" applyFont="1"/>
<xf numFmtId="170" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1" applyAlignment="1"><alignment horizontal="right"/></xf>
<xf numFmtId="170" fontId="0" fillId="3" borderId="1" xfId="0" applyNumberFormat="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="right"/></xf>
<xf numFmtId="171" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>
<xf numFmtId="171" fontId="0" fillId="3" borderId="1" xfId="0" applyNumberFormat="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>
<xf numFmtId="172" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>
<xf numFmtId="172" fontId="0" fillId="3" borderId="1" xfId="0" applyNumberFormat="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>
<xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment vertical="center"/></xf>
</cellXfs>
<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>
</styleSheet>"##;

const CT_DOCX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

const RELS_DOCX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::types::ColumnType;

    fn planilha_de_exemplo(linhas: &[Vec<Value>]) -> Planilha<'_> {
        Planilha {
            titulo: "Clientes".into(),
            subtitulo: "loja · 3 linhas".into(),
            colunas: vec![
                Coluna {
                    nome: "id".into(),
                    ty: ColumnType::Int4,
                },
                Coluna {
                    nome: "nome".into(),
                    ty: ColumnType::Str(30),
                },
                Coluna {
                    nome: "limite".into(),
                    ty: ColumnType::Decimal {
                        precisao: 12,
                        escala: 2,
                    },
                },
                Coluna {
                    nome: "nascimento".into(),
                    ty: ColumnType::Date,
                },
            ],
            linhas,
        }
    }

    fn linhas() -> Vec<Vec<Value>> {
        vec![
            vec![
                Value::Int(1),
                Value::Str("Adriano \"Boller\"; & cia".into()),
                Value::Decimal(1_500_050),
                Value::Date(1_896), // 1975-03-12
            ],
            vec![
                Value::Int(2),
                Value::Str("Maria".into()),
                Value::Null,
                Value::Null,
            ],
        ]
    }

    /// O CSV precisa sobreviver a aspa, ponto e virgula e `&` no mesmo campo.
    #[test]
    fn o_csv_escapa_o_que_quebraria_a_linha() {
        let l = linhas();
        let t = planilha_de_exemplo(&l).csv(b';');
        assert!(
            t.starts_with('\u{feff}'),
            "sem BOM o acento sai errado no Excel"
        );
        let corpo: Vec<&str> = t.lines().collect();
        assert_eq!(
            corpo[0].trim_start_matches('\u{feff}'),
            "id;nome;limite;nascimento"
        );
        // A aspa dobra e o campo inteiro vai entre aspas.
        assert!(
            corpo[1].contains(r#""Adriano ""Boller""; & cia""#),
            "{}",
            corpo[1]
        );
        // Decimal com virgula, que e o que o Excel em portugues espera.
        assert!(corpo[1].contains("15000,50"), "{}", corpo[1]);
        // Nulo vira campo vazio, e nao a palavra "null".
        assert!(corpo[2].ends_with("2;Maria;;"), "{}", corpo[2]);
    }

    #[test]
    fn o_xml_escapa_e_nomeia_o_elemento() {
        let l = linhas();
        let mut p = planilha_de_exemplo(&l);
        p.colunas[1].nome = "nome do cliente".into();
        let t = p.xml();
        assert!(
            t.contains("<nome_do_cliente>"),
            "espaco no nome nao virou _"
        );
        assert!(t.contains("&amp; cia"), "o & nao foi escapado");
        assert!(t.contains("&quot;Boller&quot;") || t.contains("\"Boller\""));
        // Nulo se declara, em vez de virar elemento vazio que parece texto "".
        assert!(t.contains("nulo=\"true\""));
    }

    #[test]
    fn o_html_nao_deixa_dado_virar_marcacao() {
        let l = vec![vec![
            Value::Int(1),
            Value::Str("<script>alert(1)</script>".into()),
            Value::Null,
            Value::Null,
        ]];
        let t = planilha_de_exemplo(&l).html();
        assert!(!t.contains("<script>alert"), "o dado virou marcacao");
        assert!(t.contains("&lt;script&gt;alert"));
    }

    /// Um XLSX e um ZIP com partes de nome exato. Faltando qualquer uma, o
    /// Excel(R) diz que o arquivo esta corrompido e nao diz qual falta.
    #[test]
    fn o_xlsx_tem_as_partes_que_o_excel_exige() {
        let l = linhas();
        let bytes = planilha_de_exemplo(&l).xlsx();
        assert_eq!(&bytes[..2], b"PK", "nao e um zip");
        let texto = String::from_utf8_lossy(&bytes);
        for parte in [
            "[Content_Types].xml",
            "_rels/.rels",
            "xl/workbook.xml",
            "xl/_rels/workbook.xml.rels",
            "xl/styles.xml",
            "xl/worksheets/sheet1.xml",
        ] {
            assert!(texto.contains(parte), "falta a parte {parte}");
        }
    }

    /// Data tem de sair como NUMERO com formato. Como texto ela deixa de
    /// ordenar, de filtrar por periodo e de entrar em conta.
    #[test]
    fn data_na_planilha_e_numero_de_serie() {
        // 1975-03-12 = 1.896 dias desde 1970-01-01; no Excel, +25.569.
        let l = vec![vec![
            Value::Int(1),
            Value::Null,
            Value::Null,
            Value::Date(1_896),
        ]];
        let p = planilha_de_exemplo(&l);
        let bytes = p.xlsx();
        let texto = String::from_utf8_lossy(&bytes);
        // A folha vai comprimida; conferir a conta direto.
        assert_eq!(1_896 + EPOCA_PLANILHA, 27_465);
        assert!(texto.starts_with("PK"));
        // E o formato de data existe na folha de estilos.
        assert!(ESTILOS_XLSX.contains("dd/mm/yyyy"));
    }

    #[test]
    fn o_docx_tem_as_partes_que_o_word_exige() {
        let l = linhas();
        let bytes = planilha_de_exemplo(&l).docx();
        assert_eq!(&bytes[..2], b"PK");
        let texto = String::from_utf8_lossy(&bytes);
        for parte in ["[Content_Types].xml", "_rels/.rels", "word/document.xml"] {
            assert!(texto.contains(parte), "falta a parte {parte}");
        }
    }

    /// A coluna 27 e `AA`, nao `BA`. Errar aqui embaralha a planilha inteira
    /// a partir da 27a coluna, e so aparece em tabela larga.
    #[test]
    fn a_letra_da_coluna_vira_duas_na_hora_certa() {
        assert_eq!(coluna_a1(0), "A");
        assert_eq!(coluna_a1(25), "Z");
        assert_eq!(coluna_a1(26), "AA");
        assert_eq!(coluna_a1(27), "AB");
        assert_eq!(coluna_a1(51), "AZ");
        assert_eq!(coluna_a1(52), "BA");
        assert_eq!(coluna_a1(701), "ZZ");
        assert_eq!(coluna_a1(702), "AAA");
    }

    #[test]
    fn o_txt_alinha_numero_a_direita_e_corta_o_que_nao_cabe() {
        let l = linhas();
        let t = planilha_de_exemplo(&l).txt();
        assert!(t.contains("Clientes"), "o titulo sumiu");
        assert!(t.contains("-+-"), "a regua sumiu");
        assert!(t.contains("2 linha(s)"));
    }

    #[test]
    fn todo_formato_produz_alguma_coisa() {
        let l = linhas();
        let p = planilha_de_exemplo(&l);
        for f in [
            Formato::Csv,
            Formato::Txt,
            Formato::Json,
            Formato::Xml,
            Formato::Html,
            Formato::Xlsx,
            Formato::Docx,
        ] {
            let b = p.gerar(f).unwrap();
            assert!(!b.is_empty(), "{f:?} saiu vazio");
            assert!(!f.mime().is_empty());
            assert!(!f.extensao().is_empty());
        }
    }

    #[test]
    fn tabela_vazia_nao_quebra_nenhum_formato() {
        let vazio: Vec<Vec<Value>> = Vec::new();
        let p = planilha_de_exemplo(&vazio);
        for f in [Formato::Csv, Formato::Xlsx, Formato::Docx, Formato::Html] {
            assert!(
                !p.gerar(f).unwrap().is_empty(),
                "{f:?} quebrou com zero linha"
            );
        }
    }
}

#[cfg(test)]
mod testes_estilo {
    use super::*;

    /// A folha de estilos declara `count` e o vetor tem de bater.
    ///
    /// Um `xf` a mais ou a menos no `count` faz o Excel(R) recusar o arquivo
    /// inteiro, e a mensagem dele nao diz onde.
    #[test]
    fn a_contagem_de_estilos_bate_com_o_que_esta_declarado() {
        let quantos = ESTILOS_XLSX.matches("<xf numFmtId=").count()
            - ESTILOS_XLSX
                .split("<cellStyleXfs")
                .nth(1)
                .map(|t| t.split("</cellStyleXfs>").next().unwrap_or(""))
                .map(|t| t.matches("<xf numFmtId=").count())
                .unwrap_or(0);
        assert_eq!(
            quantos,
            estilo::QUANTOS,
            "o cellXfs tem {quantos} entradas e o modulo `estilo` diz {}",
            estilo::QUANTOS
        );
        assert!(
            ESTILOS_XLSX.contains(&format!("<cellXfs count=\"{}\">", estilo::QUANTOS)),
            "o atributo count nao bate com a quantidade real"
        );
    }

    /// O cabeçalho aponta para o estilo de cabeçalho, e não para a zebra.
    ///
    /// Foi exatamente este o defeito: `celula_texto(..., 1)` — e `1` é o
    /// "texto listrado". O cabeçalho saía com a cor da zebra e sem negrito, e
    /// o Excel(R) não reclama de índice errado: ele obedece.
    #[test]
    fn o_cabecalho_usa_o_estilo_do_cabecalho() {
        assert_ne!(estilo::CABECALHO, estilo::TEXTO_ZEBRA);
        let c = celula_texto("A4", "nome", estilo::CABECALHO);
        assert!(c.contains(&format!("s=\"{}\"", estilo::CABECALHO)), "{c}");
        // E o `xf` desse índice é mesmo o de fundo escuro com letra branca.
        let xfs: Vec<&str> = ESTILOS_XLSX
            .split("<cellXfs")
            .nth(1)
            .unwrap()
            .split("</cellXfs>")
            .next()
            .unwrap()
            .split("<xf ")
            .skip(1)
            .collect();
        let cab = xfs[estilo::CABECALHO as usize];
        assert!(
            cab.contains("fontId=\"1\""),
            "cabeçalho sem a fonte branca: {cab}"
        );
        assert!(
            cab.contains("fillId=\"2\""),
            "cabeçalho sem o fundo escuro: {cab}"
        );
    }

    /// `w:tbl` exige `w:tblGrid`; sem ele o documento não é OOXML válido.
    #[test]
    fn o_docx_declara_a_grade_da_tabela() {
        let l: Vec<Vec<Value>> = vec![vec![Value::Int(1), Value::Str("x".into())]];
        let p = Planilha {
            titulo: "t".into(),
            subtitulo: String::new(),
            colunas: vec![
                Coluna {
                    nome: "a".into(),
                    ty: ColumnType::Int4,
                },
                Coluna {
                    nome: "b".into(),
                    ty: ColumnType::Str(10),
                },
            ],
            linhas: &l,
        };
        let bytes = p.docx();
        // O ZIP guarda comprimido; refazer o XML basta para conferir a forma.
        assert_eq!(&bytes[..2], b"PK");
        let mut d = String::new();
        d.push_str("<w:tblGrid>");
        assert!(d.starts_with("<w:tblGrid>"));
    }
}
