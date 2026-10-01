//! WordprocessingML (.docx).

use crate::opc::{self, rel};
use crate::xml::{self, DECL, Event, esc};
use crate::{Block, Document, OfficeError, Result, zip};
use std::path::Path;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
// A4 com margens de 1 polegada, em vigesimos de ponto: a largura util define
// a largura das colunas da tabela, que o Word nao calcula sozinho sem grade.
const TEXT_WIDTH: usize = 11906 - 2 * 1440;

/// Um `w:r` por trecho; quebra de linha e tabulacao viram os elementos
/// proprios, porque `\n` cru dentro de `w:t` o Word mostra como espaco.
fn runs(text: &str, bold: bool) -> String {
    let rpr = if bold { "<w:rPr><w:b/></w:rPr>" } else { "" };
    let mut s = format!("<w:r>{rpr}");
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            s.push_str("<w:br/>");
        }
        for (j, part) in line.split('\t').enumerate() {
            if j > 0 {
                s.push_str("<w:tab/>");
            }
            if !part.is_empty() {
                s.push_str(&format!(
                    "<w:t xml:space=\"preserve\">{}</w:t>",
                    esc(&part.replace('\r', ""))
                ));
            }
        }
    }
    s.push_str("</w:r>");
    s
}

fn para(style: Option<&str>, extra_ppr: &str, text: &str, bold: bool) -> String {
    let mut ppr = String::new();
    if let Some(st) = style {
        ppr.push_str(&format!("<w:pStyle w:val=\"{st}\"/>"));
    }
    ppr.push_str(extra_ppr);
    let ppr = if ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{ppr}</w:pPr>")
    };
    format!("<w:p>{ppr}{}</w:p>", runs(text, bold))
}

fn table(header: &[String], rows: &[Vec<String>]) -> String {
    let ncols = rows
        .iter()
        .map(Vec::len)
        .chain(std::iter::once(header.len()))
        .max()
        .unwrap_or(0);
    if ncols == 0 {
        return String::new();
    }
    let w = TEXT_WIDTH / ncols;
    let border =
        |n: &str| format!("<w:{n} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>");
    let mut s = String::from("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders>");
    for n in ["top", "left", "bottom", "right", "insideH", "insideV"] {
        s.push_str(&border(n));
    }
    s.push_str("</w:tblBorders><w:tblLook w:val=\"04A0\"/></w:tblPr><w:tblGrid>");
    for _ in 0..ncols {
        s.push_str(&format!("<w:gridCol w:w=\"{w}\"/>"));
    }
    s.push_str("</w:tblGrid>");
    let mut row = |cells: &[String], head: bool| {
        s.push_str("<w:tr>");
        if head {
            // Cabecalho repete em cada pagina quando a tabela quebra.
            s.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        for i in 0..ncols {
            let t = cells.get(i).map(String::as_str).unwrap_or("");
            s.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>",
                para(None, "", t, head)
            ));
        }
        s.push_str("</w:tr>");
    };
    if !header.is_empty() {
        row(header, true);
    }
    for r in rows {
        row(r, false);
    }
    s.push_str("</w:tbl>");
    s
}

fn document_xml(doc: &Document) -> Result<String> {
    let mut body = String::new();
    if !doc.title.is_empty() {
        body.push_str(&para(Some("Title"), "", &doc.title, false));
    }
    let mut last_is_table = false;
    for b in &doc.blocks {
        last_is_table = false;
        match b {
            Block::Heading { level, text } => {
                if !(1..=3).contains(level) {
                    return Err(OfficeError::Invalid(format!(
                        "nivel de titulo {level} fora de 1..=3"
                    )));
                }
                body.push_str(&para(Some(&format!("Heading{level}")), "", text, false));
            }
            Block::Paragraph { text, bold } => body.push_str(&para(None, "", text, *bold)),
            Block::Bullets { items } => {
                for it in items {
                    body.push_str(&para(
                        Some("ListParagraph"),
                        "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr>",
                        it,
                        false,
                    ));
                }
            }
            Block::Table { header, rows } => {
                let t = table(header, rows);
                last_is_table = !t.is_empty();
                body.push_str(&t);
            }
        }
    }
    // O Word exige um paragrafo entre a ultima tabela e o sectPr; sem ele
    // declara o arquivo corrompido e oferece "recuperar".
    if last_is_table {
        body.push_str("<w:p/>");
    }
    Ok(format!(
        "{DECL}<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{}\"><w:body>{body}\
<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" \
w:bottom=\"1440\" w:left=\"1440\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr>\
</w:body></w:document>",
        opc::NS_R
    ))
}

fn styles_xml() -> String {
    let heading = |n: u8, sz: u8| {
        format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Heading{n}\"><w:name w:val=\"heading {n}\"/>\
<w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/>\
<w:pPr><w:keepNext/><w:spacing w:before=\"240\" w:after=\"80\"/><w:outlineLvl w:val=\"{}\"/></w:pPr>\
<w:rPr><w:b/><w:sz w:val=\"{sz}\"/></w:rPr></w:style>",
            n - 1
        )
    };
    format!(
        "{DECL}<w:styles xmlns:w=\"{NS_W}\"><w:docDefaults><w:rPrDefault><w:rPr>\
<w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:eastAsia=\"Calibri\" w:cs=\"Calibri\"/>\
<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang w:val=\"pt-BR\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"264\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
</w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/>\
<w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:after=\"240\"/></w:pPr>\
<w:rPr><w:sz w:val=\"52\"/></w:rPr></w:style>{}{}{}\
<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/>\
<w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:ind w:left=\"720\"/></w:pPr></w:style>\
</w:styles>",
        heading(1, 32),
        heading(2, 28),
        heading(3, 24)
    )
}

fn numbering_xml() -> String {
    format!(
        "{DECL}<w:numbering xmlns:w=\"{NS_W}\"><w:abstractNum w:abstractNumId=\"0\">\
<w:multiLevelType w:val=\"hybridMultilevel\"/><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/>\
<w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"\u{2022}\"/><w:lvlJc w:val=\"left\"/>\
<w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr></w:lvl></w:abstractNum>\
<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num></w:numbering>"
    )
}

pub fn docx_bytes(doc: &Document) -> Result<Vec<u8>> {
    let wml = "application/vnd.openxmlformats-officedocument.wordprocessingml";
    let ct = opc::content_types(&[
        (
            "/word/document.xml".into(),
            &format!("{wml}.document.main+xml"),
        ),
        ("/word/styles.xml".into(), &format!("{wml}.styles+xml")),
        (
            "/word/numbering.xml".into(),
            &format!("{wml}.numbering+xml"),
        ),
    ]);
    let doc_rels = opc::relationships(&[
        rel("rId1", "styles", "styles.xml"),
        rel("rId2", "numbering", "numbering.xml"),
    ]);
    let entries = vec![
        ("[Content_Types].xml".to_string(), ct.into_bytes()),
        (
            "_rels/.rels".into(),
            opc::root_rels("word/document.xml").into_bytes(),
        ),
        (
            "docProps/core.xml".into(),
            opc::core(&doc.title).into_bytes(),
        ),
        ("docProps/app.xml".into(), opc::app().into_bytes()),
        ("word/document.xml".into(), document_xml(doc)?.into_bytes()),
        ("word/_rels/document.xml.rels".into(), doc_rels.into_bytes()),
        ("word/styles.xml".into(), styles_xml().into_bytes()),
        ("word/numbering.xml".into(), numbering_xml().into_bytes()),
    ];
    zip::write_store(&entries)
}

pub fn write_docx(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    crate::write_file(path.as_ref(), &docx_bytes(doc)?)
}

pub fn read_docx_text(path: impl AsRef<Path>) -> Result<String> {
    read_docx_text_bytes(&std::fs::read(path)?)
}

/// Texto corrido: um paragrafo por linha (celula de tabela tambem e
/// paragrafo). `w:tab`/`w:br` so contam dentro de `w:r`, porque `w:tab` fora
/// dele e definicao de parada de tabulacao, nao texto.
pub fn read_docx_text_bytes(bytes: &[u8]) -> Result<String> {
    let ar = zip::read(bytes)?;
    let main = opc::main_part(&ar, "word/document.xml")?;
    let src = ar
        .get(&main)
        .ok_or_else(|| OfficeError::Corrupt(format!("parte {main} ausente")))?;
    let src = String::from_utf8_lossy(src);
    let ev = xml::events(&src).map_err(OfficeError::Corrupt)?;
    let mut out = String::new();
    let (mut in_run, mut in_t, mut in_del) = (0usize, false, 0usize);
    for e in ev {
        match e {
            Event::Start { name, empty, .. } => match xml::local(&name) {
                "r" if !empty => in_run += 1,
                "t" if !empty && in_del == 0 => in_t = true,
                "del" if !empty => in_del += 1,
                "tab" if in_run > 0 => out.push('\t'),
                "br" | "cr" if in_run > 0 => out.push('\n'),
                "p" if empty => out.push('\n'),
                _ => {}
            },
            Event::End(name) => match xml::local(&name) {
                "r" => in_run = in_run.saturating_sub(1),
                "t" => in_t = false,
                "del" => in_del = in_del.saturating_sub(1),
                "p" => out.push('\n'),
                _ => {}
            },
            Event::Text(t) if in_t => out.push_str(&t),
            Event::Text(_) => {}
        }
    }
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exemplo() -> Document {
        Document {
            title: "Relatório & Ação".into(),
            blocks: vec![
                Block::Heading {
                    level: 1,
                    text: "Seção <1>".into(),
                },
                Block::Paragraph {
                    text: "linha\tcom tab\nsegunda \"linha\"\u{7}".into(),
                    bold: true,
                },
                Block::Bullets {
                    items: vec!["maçã".into(), "pão".into()],
                },
                Block::Table {
                    header: vec!["Cidade".into(), "UF".into()],
                    rows: vec![
                        vec!["São Paulo".into(), "SP".into()],
                        vec!["Blumenau".into()],
                    ],
                },
            ],
        }
    }

    #[test]
    fn ida_e_volta_do_texto() {
        let b = docx_bytes(&exemplo()).unwrap();
        let t = read_docx_text_bytes(&b).unwrap();
        assert_eq!(
            t,
            "Relatório & Ação\nSeção <1>\nlinha\tcom tab\nsegunda \"linha\"\nmaçã\npão\n\
Cidade\nUF\nSão Paulo\nSP\nBlumenau"
        );
    }

    #[test]
    fn deterministico() {
        assert_eq!(
            docx_bytes(&exemplo()).unwrap(),
            docx_bytes(&exemplo()).unwrap()
        );
    }

    #[test]
    fn nivel_de_titulo_fora_da_faixa_e_recusado() {
        let d = Document {
            title: String::new(),
            blocks: vec![Block::Heading {
                level: 4,
                text: "x".into(),
            }],
        };
        assert!(matches!(docx_bytes(&d), Err(OfficeError::Invalid(_))));
    }

    #[test]
    fn modelo_em_json() {
        let j = r#"{"title":"T","blocks":[{"type":"heading","level":2,"text":"H"},
            {"type":"paragraph","text":"p"},{"type":"bullets","items":["a"]},
            {"type":"table","header":["x"],"rows":[["1"]]}]}"#;
        let d: Document = serde_json::from_str(j).unwrap();
        assert_eq!(d.blocks.len(), 4);
        assert_eq!(
            d.blocks[1],
            Block::Paragraph {
                text: "p".into(),
                bold: false
            }
        );
    }
}
