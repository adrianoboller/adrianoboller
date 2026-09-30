//! SpreadsheetML (.xlsx).

use crate::opc::{self, rel};
use crate::xml::{self, DECL, Event, esc};
use crate::{Cell, OfficeError, Result, Sheet, Workbook, zip};
use std::collections::HashMap;
use std::path::Path;

const NS_S: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const MAX_ROWS: usize = 1_048_576;
const MAX_COLS: usize = 16_384;

/// Letras da coluna a partir do indice 0 (0 -> A, 26 -> AA).
pub fn col_letters(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn col_index(letters: &str) -> Option<usize> {
    let mut n = 0usize;
    for c in letters.bytes() {
        if !c.is_ascii_uppercase() {
            return None;
        }
        n = n.checked_mul(26)?.checked_add((c - b'A' + 1) as usize)?;
    }
    n.checked_sub(1)
}

/// Regras do Excel para nome de aba. O Excel recusa abrir o arquivo inteiro
/// (e nao so a aba) quando uma delas e violada, por isso a recusa e aqui.
pub fn validate_sheet_name(name: &str) -> Result<()> {
    let bad = |m: &str| Err(OfficeError::Invalid(format!("nome de aba {name:?}: {m}")));
    if name.is_empty() {
        return bad("vazio");
    }
    if name.chars().count() > 31 {
        return bad("mais de 31 caracteres");
    }
    if let Some(c) = name.chars().find(|c| "[]:*?/\\".contains(*c)) {
        return bad(&format!("caractere proibido {c:?}"));
    }
    if xml::strip_forbidden(name) != name {
        return bad("caractere de controle");
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return bad("apostrofo no inicio ou no fim");
    }
    Ok(())
}

/// Numero no formato que o Excel le como `xsd:double`. Rust imprime todos os
/// digitos sem expoente; 1e300 viraria 301 caracteres, entao os extremos vao
/// em notacao cientifica. NaN e infinito nao existem numa celula do Excel.
fn fmt_number(n: f64) -> Result<String> {
    if !n.is_finite() {
        return Err(OfficeError::Invalid(format!("numero nao finito: {n}")));
    }
    let a = n.abs();
    if a != 0.0 && !(1e-5..1e15).contains(&a) {
        Ok(format!("{n:e}"))
    } else {
        Ok(format!("{n}"))
    }
}

#[derive(Default)]
struct Shared {
    index: HashMap<String, usize>,
    list: Vec<String>,
    refs: usize,
}

impl Shared {
    fn id(&mut self, s: &str) -> usize {
        self.refs += 1;
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        let i = self.list.len();
        self.list.push(s.to_string());
        self.index.insert(s.to_string(), i);
        i
    }
}

fn sheet_xml(sh: &Sheet, shared: &mut Shared) -> Result<String> {
    if sh.rows.len() > MAX_ROWS {
        return Err(OfficeError::Invalid(format!(
            "aba {:?}: linhas demais",
            sh.name
        )));
    }
    let mut data = String::new();
    for (r, row) in sh.rows.iter().enumerate() {
        if row.len() > MAX_COLS {
            return Err(OfficeError::Invalid(format!(
                "aba {:?}: colunas demais",
                sh.name
            )));
        }
        let style = if sh.bold_header && r == 0 {
            " s=\"1\""
        } else {
            ""
        };
        let mut cells = String::new();
        for (c, cell) in row.iter().enumerate() {
            let at = format!("{}{}", col_letters(c), r + 1);
            match cell {
                Cell::Empty => continue,
                Cell::Text(t) => cells.push_str(&format!(
                    "<c r=\"{at}\"{style} t=\"s\"><v>{}</v></c>",
                    shared.id(t)
                )),
                Cell::Number(n) => cells.push_str(&format!(
                    "<c r=\"{at}\"{style}><v>{}</v></c>",
                    fmt_number(*n)?
                )),
                Cell::Bool(b) => cells.push_str(&format!(
                    "<c r=\"{at}\"{style} t=\"b\"><v>{}</v></c>",
                    u8::from(*b)
                )),
                // Sem valor em cache: o calcPr fullCalcOnLoad manda o leitor
                // recalcular, em vez de mostrar um zero que ninguem calculou.
                Cell::Formula(f) => cells.push_str(&format!(
                    "<c r=\"{at}\"{style}><f>{}</f></c>",
                    esc(f.strip_prefix('=').unwrap_or(f))
                )),
            }
        }
        if !cells.is_empty() {
            data.push_str(&format!("<row r=\"{}\">{cells}</row>", r + 1));
        }
    }
    let data = if data.is_empty() {
        "<sheetData/>".to_string()
    } else {
        format!("<sheetData>{data}</sheetData>")
    };
    Ok(format!(
        "{DECL}<worksheet xmlns=\"{NS_S}\" xmlns:r=\"{}\">{data}</worksheet>",
        opc::NS_R
    ))
}

fn styles_xml() -> String {
    format!(
        "{DECL}<styleSheet xmlns=\"{NS_S}\"><fonts count=\"2\">\
<font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font>\
<font><b/><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font></fonts>\
<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill>\
<fill><patternFill patternType=\"gray125\"/></fill></fills>\
<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/></cellXfs>\
<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\
</styleSheet>"
    )
}

pub fn xlsx_bytes(wb: &Workbook) -> Result<Vec<u8>> {
    if wb.sheets.is_empty() {
        return Err(OfficeError::Invalid("pasta de trabalho sem abas".into()));
    }
    let mut seen = Vec::new();
    for sh in &wb.sheets {
        validate_sheet_name(&sh.name)?;
        // O Excel compara nomes de aba sem caixa.
        let low = sh.name.to_lowercase();
        if seen.contains(&low) {
            return Err(OfficeError::Invalid(format!("aba repetida: {:?}", sh.name)));
        }
        seen.push(low);
    }
    let sml = "application/vnd.openxmlformats-officedocument.spreadsheetml";
    let n = wb.sheets.len();
    let mut shared = Shared::default();
    let mut sheets = Vec::new();
    for sh in &wb.sheets {
        sheets.push(sheet_xml(sh, &mut shared)?);
    }
    let ws_ct = format!("{sml}.worksheet+xml");
    let mut over: Vec<(String, &str)> = Vec::new();
    let main_ct = format!("{sml}.sheet.main+xml");
    let st_ct = format!("{sml}.styles+xml");
    let ss_ct = format!("{sml}.sharedStrings+xml");
    over.push(("/xl/workbook.xml".into(), &main_ct));
    over.push(("/xl/styles.xml".into(), &st_ct));
    over.push(("/xl/sharedStrings.xml".into(), &ss_ct));
    for i in 1..=n {
        over.push((format!("/xl/worksheets/sheet{i}.xml"), &ws_ct));
    }
    let mut wb_xml = format!(
        "{DECL}<workbook xmlns=\"{NS_S}\" xmlns:r=\"{}\"><sheets>",
        opc::NS_R
    );
    let mut rels = Vec::new();
    for (i, sh) in wb.sheets.iter().enumerate() {
        wb_xml.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>",
            esc(&sh.name),
            i + 1,
            i + 1
        ));
        rels.push(rel(
            &format!("rId{}", i + 1),
            "worksheet",
            &format!("worksheets/sheet{}.xml", i + 1),
        ));
    }
    wb_xml.push_str("</sheets><calcPr calcId=\"191029\" fullCalcOnLoad=\"1\"/></workbook>");
    rels.push(rel(&format!("rId{}", n + 1), "styles", "styles.xml"));
    rels.push(rel(
        &format!("rId{}", n + 2),
        "sharedStrings",
        "sharedStrings.xml",
    ));

    let mut sst = format!(
        "{DECL}<sst xmlns=\"{NS_S}\" count=\"{}\" uniqueCount=\"{}\">",
        shared.refs,
        shared.list.len()
    );
    for s in &shared.list {
        sst.push_str(&format!(
            "<si><t xml:space=\"preserve\">{}</t></si>",
            esc(s)
        ));
    }
    sst.push_str("</sst>");

    let title = wb.sheets[0].name.clone();
    let mut entries = vec![
        (
            "[Content_Types].xml".to_string(),
            opc::content_types(&over).into_bytes(),
        ),
        (
            "_rels/.rels".into(),
            opc::root_rels("xl/workbook.xml").into_bytes(),
        ),
        ("docProps/core.xml".into(), opc::core(&title).into_bytes()),
        ("docProps/app.xml".into(), opc::app().into_bytes()),
        ("xl/workbook.xml".into(), wb_xml.into_bytes()),
        (
            "xl/_rels/workbook.xml.rels".into(),
            opc::relationships(&rels).into_bytes(),
        ),
        ("xl/styles.xml".into(), styles_xml().into_bytes()),
        ("xl/sharedStrings.xml".into(), sst.into_bytes()),
    ];
    for (i, s) in sheets.into_iter().enumerate() {
        entries.push((format!("xl/worksheets/sheet{}.xml", i + 1), s.into_bytes()));
    }
    zip::write_store(&entries)
}

pub fn write_xlsx(wb: &Workbook, path: impl AsRef<Path>) -> Result<()> {
    crate::write_file(path.as_ref(), &xlsx_bytes(wb)?)
}

pub fn read_xlsx_values(path: impl AsRef<Path>) -> Result<Workbook> {
    read_xlsx_values_bytes(&std::fs::read(path)?)
}

fn parse(ar: &zip::Archive, part: &str) -> Result<Option<Vec<Event>>> {
    match ar.get(part) {
        None => Ok(None),
        Some(b) => xml::events(&String::from_utf8_lossy(b))
            .map(Some)
            .map_err(|e| OfficeError::Corrupt(format!("{part}: {e}"))),
    }
}

/// Valores das celulas, aba por aba. Celula com formula e valor em cache
/// devolve o valor (e o que o agente quer ler); sem cache devolve a formula.
pub fn read_xlsx_values_bytes(bytes: &[u8]) -> Result<Workbook> {
    let ar = zip::read(bytes)?;
    let wb_part = "xl/workbook.xml";
    let wb_ev = parse(&ar, wb_part)?
        .ok_or_else(|| OfficeError::Corrupt("xl/workbook.xml ausente".into()))?;
    let mut targets = HashMap::new();
    let mut shared_part = "xl/sharedStrings.xml".to_string();
    for e in parse(&ar, "xl/_rels/workbook.xml.rels")?.unwrap_or_default() {
        if let Event::Start { name, attrs, .. } = e
            && xml::local(&name) == "Relationship"
        {
            let id = xml::attr(&attrs, "Id").unwrap_or_default().to_string();
            let t = opc::resolve("xl", xml::attr(&attrs, "Target").unwrap_or_default());
            if xml::attr(&attrs, "Type").is_some_and(|t| t.ends_with("/sharedStrings")) {
                shared_part = t.clone();
            }
            targets.insert(id, t);
        }
    }
    let shared = read_shared(parse(&ar, &shared_part)?.unwrap_or_default());
    let mut out = Workbook { sheets: Vec::new() };
    for e in wb_ev {
        if let Event::Start { name, attrs, .. } = e
            && xml::local(&name) == "sheet"
        {
            let sname = xml::attr(&attrs, "name").unwrap_or_default().to_string();
            let rid = attrs
                .iter()
                .find(|(k, _)| k.ends_with(":id"))
                .map(|(_, v)| v.as_str())
                .unwrap_or_default();
            let part = targets
                .get(rid)
                .ok_or_else(|| OfficeError::Corrupt(format!("aba {sname:?} sem relacao")))?;
            let ev = parse(&ar, part)?
                .ok_or_else(|| OfficeError::Corrupt(format!("parte {part} ausente")))?;
            out.sheets.push(Sheet {
                name: sname,
                rows: read_sheet(ev, &shared)?,
                bold_header: false,
            });
        }
    }
    Ok(out)
}

fn read_shared(ev: Vec<Event>) -> Vec<String> {
    let mut list = Vec::new();
    let mut cur = String::new();
    let (mut in_t, mut in_rph) = (false, false);
    for e in ev {
        match e {
            Event::Start { name, empty, .. } => match xml::local(&name) {
                "si" if empty => list.push(String::new()),
                "si" => cur.clear(),
                "t" if !empty && !in_rph => in_t = true,
                // Guia fonetico (japones) nao faz parte do valor.
                "rPh" if !empty => in_rph = true,
                _ => {}
            },
            Event::End(name) => match xml::local(&name) {
                "si" => list.push(std::mem::take(&mut cur)),
                "t" => in_t = false,
                "rPh" => in_rph = false,
                _ => {}
            },
            Event::Text(t) if in_t => cur.push_str(&t),
            Event::Text(_) => {}
        }
    }
    list
}

fn place(rows: &mut Vec<Vec<Cell>>, r: usize, c: usize, v: Cell) -> Result<()> {
    if r >= MAX_ROWS || c >= MAX_COLS {
        return Err(OfficeError::Corrupt(
            "referencia de celula fora da grade".into(),
        ));
    }
    if rows.len() <= r {
        rows.resize(r + 1, Vec::new());
    }
    let row = &mut rows[r];
    if row.len() <= c {
        row.resize(c + 1, Cell::Empty);
    }
    row[c] = v;
    Ok(())
}

fn read_sheet(ev: Vec<Event>, shared: &[String]) -> Result<Vec<Vec<Cell>>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let (mut row_i, mut col_i) = (0usize, 0usize);
    let mut ty = String::new();
    let (mut v, mut f, mut is) = (None::<String>, None::<String>, None::<String>);
    let mut cur: Option<&'static str> = None;
    let mut in_is = false;
    let mut in_cell = false;
    for e in ev {
        match e {
            Event::Start { name, attrs, empty } => match xml::local(&name) {
                "row" => {
                    if let Some(r) = xml::attr(&attrs, "r").and_then(|r| r.parse::<usize>().ok()) {
                        row_i = r.saturating_sub(1);
                    }
                    col_i = 0;
                    if empty {
                        row_i += 1;
                    }
                }
                "c" => {
                    if let Some(r) = xml::attr(&attrs, "r") {
                        let letters: String =
                            r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                        col_i = col_index(&letters.to_ascii_uppercase())
                            .ok_or_else(|| OfficeError::Corrupt(format!("celula {r:?}")))?;
                    }
                    ty = xml::attr(&attrs, "t").unwrap_or("n").to_string();
                    v = None;
                    f = None;
                    is = None;
                    in_cell = !empty;
                    if empty {
                        col_i += 1;
                    }
                }
                "v" if in_cell && !empty => cur = Some("v"),
                "f" if in_cell && !empty => cur = Some("f"),
                "is" if in_cell => in_is = true,
                "t" if in_is && !empty => cur = Some("t"),
                _ => {}
            },
            Event::Text(t) => match cur {
                Some("v") => v.get_or_insert_default().push_str(&t),
                Some("f") => f.get_or_insert_default().push_str(&t),
                Some("t") => is.get_or_insert_default().push_str(&t),
                _ => {}
            },
            Event::End(name) => match xml::local(&name) {
                "v" | "f" | "t" => cur = None,
                "is" => in_is = false,
                "row" => row_i += 1,
                "c" if in_cell => {
                    in_cell = false;
                    let cell = match (ty.as_str(), v.take()) {
                        ("inlineStr", _) => Cell::Text(is.take().unwrap_or_default()),
                        ("s", Some(x)) => {
                            let i: usize = x.trim().parse().map_err(|_| {
                                OfficeError::Corrupt(format!("indice de texto {x:?}"))
                            })?;
                            Cell::Text(shared.get(i).cloned().ok_or_else(|| {
                                OfficeError::Corrupt(format!("texto compartilhado {i} ausente"))
                            })?)
                        }
                        ("b", Some(x)) => Cell::Bool(x.trim() == "1"),
                        ("n", Some(x)) => match x.trim().parse::<f64>() {
                            Ok(n) => Cell::Number(n),
                            Err(_) => Cell::Text(x),
                        },
                        (_, Some(x)) => Cell::Text(x),
                        (_, None) => match f.take() {
                            Some(fx) => Cell::Formula(format!("={fx}")),
                            None => Cell::Empty,
                        },
                    };
                    if cell != Cell::Empty {
                        place(&mut rows, row_i, col_i, cell)?;
                    }
                    col_i += 1;
                }
                _ => {}
            },
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exemplo() -> Workbook {
        Workbook {
            sheets: vec![
                Sheet {
                    name: "Vendas ção".into(),
                    bold_header: true,
                    rows: vec![
                        vec!["Item & <x>".into(), "Valor".into(), "Ok".into()],
                        vec!["maçã".into(), 1.5.into(), true.into()],
                        vec!["pão".into(), 2i64.into(), false.into()],
                        vec![Cell::Empty, Cell::Formula("=SUM(B2:B3)".into())],
                    ],
                },
                Sheet {
                    name: "Vazia".into(),
                    rows: vec![],
                    bold_header: false,
                },
            ],
        }
    }

    #[test]
    fn letras_de_coluna() {
        assert_eq!(col_letters(0), "A");
        assert_eq!(col_letters(25), "Z");
        assert_eq!(col_letters(26), "AA");
        assert_eq!(col_letters(701), "ZZ");
        assert_eq!(col_letters(702), "AAA");
        assert_eq!(col_letters(16383), "XFD");
        for i in [0, 25, 26, 701, 702, 16383] {
            assert_eq!(col_index(&col_letters(i)), Some(i));
        }
    }

    #[test]
    fn nomes_de_aba() {
        assert!(validate_sheet_name("Relatório 2026").is_ok());
        assert!(validate_sheet_name(&"x".repeat(31)).is_ok());
        assert!(validate_sheet_name(&"ç".repeat(31)).is_ok());
        assert!(validate_sheet_name(&"x".repeat(32)).is_err());
        assert!(validate_sheet_name("").is_err());
        assert!(validate_sheet_name("'a").is_err());
        assert!(validate_sheet_name("a\u{1}").is_err());
        for c in ["[", "]", ":", "*", "?", "/", "\\"] {
            assert!(validate_sheet_name(&format!("a{c}b")).is_err(), "{c}");
        }
    }

    #[test]
    fn aba_repetida_sem_caixa_e_recusada() {
        let mut wb = exemplo();
        wb.sheets[1].name = "VENDAS ÇÃO".into();
        wb.sheets[0].name = "vendas ção".into();
        assert!(xlsx_bytes(&wb).is_err());
    }

    #[test]
    fn numeros() {
        assert_eq!(fmt_number(1.5).unwrap(), "1.5");
        assert_eq!(fmt_number(2.0).unwrap(), "2");
        assert_eq!(fmt_number(1e300).unwrap(), "1e300");
        assert!(fmt_number(f64::NAN).is_err());
        assert!(fmt_number(f64::INFINITY).is_err());
    }

    #[test]
    fn ida_e_volta_dos_valores() {
        let b = xlsx_bytes(&exemplo()).unwrap();
        let wb = read_xlsx_values_bytes(&b).unwrap();
        assert_eq!(wb.sheets.len(), 2);
        let s = &wb.sheets[0];
        assert_eq!(s.name, "Vendas ção");
        assert_eq!(s.rows[0][0], Cell::Text("Item & <x>".into()));
        assert_eq!(s.rows[1], vec!["maçã".into(), 1.5.into(), true.into()]);
        assert_eq!(s.rows[2], vec!["pão".into(), 2.0.into(), false.into()]);
        assert_eq!(
            s.rows[3],
            vec![Cell::Empty, Cell::Formula("=SUM(B2:B3)".into())]
        );
        assert!(wb.sheets[1].rows.is_empty());
    }

    #[test]
    fn deterministico() {
        assert_eq!(
            xlsx_bytes(&exemplo()).unwrap(),
            xlsx_bytes(&exemplo()).unwrap()
        );
    }

    #[test]
    fn celula_em_json() {
        let c: Vec<Cell> = serde_json::from_str(
            r#"[{"text":"a"},{"number":3},{"bool":true},{"formula":"=A1"},"empty"]"#,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "a".into(),
                3.0.into(),
                true.into(),
                Cell::Formula("=A1".into()),
                Cell::Empty
            ]
        );
    }
}
