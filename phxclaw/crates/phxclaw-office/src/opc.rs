//! Pecas comuns do pacote OPC: tipos de conteudo, relacoes e propriedades.

use crate::xml::{DECL, esc};

pub const REL_OFFICE_DOC: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

// Data fixa: com o relogio real, dois arquivos do mesmo modelo teriam hashes
// diferentes, e a prova por hash deixaria de valer. Casa com a data DOS do zip.
pub const FIXED_DATE: &str = "1980-01-01T00:00:00Z";

/// `overrides`: (nome da parte com `/` inicial, tipo de conteudo).
pub fn content_types(overrides: &[(String, &str)]) -> String {
    let mut s = String::from(DECL);
    s.push_str("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    s.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>");
    s.push_str("<Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    s.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>");
    s.push_str("<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>");
    for (part, ct) in overrides {
        s.push_str(&format!(
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            esc(part),
            ct
        ));
    }
    s.push_str("</Types>");
    s
}

/// `rels`: (Id, Tipo completo, Alvo).
pub fn relationships(rels: &[(String, String, String)]) -> String {
    let mut s = String::from(DECL);
    s.push_str(
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for (id, ty, target) in rels {
        s.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
            esc(id),
            esc(ty),
            esc(target)
        ));
    }
    s.push_str("</Relationships>");
    s
}

pub fn rel(id: &str, ty: &str, target: &str) -> (String, String, String) {
    let ty = if ty.starts_with("http") {
        ty.to_string()
    } else {
        format!("{REL}{ty}")
    };
    (id.to_string(), ty, target.to_string())
}

pub fn root_rels(main: &str) -> String {
    relationships(&[
        rel("rId1", REL_OFFICE_DOC, main),
        rel(
            "rId2",
            "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties",
            "docProps/core.xml",
        ),
        rel("rId3", "extended-properties", "docProps/app.xml"),
    ])
}

pub fn core(title: &str) -> String {
    format!(
        "{DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title><dc:creator>PhxClaw</dc:creator><cp:lastModifiedBy>PhxClaw</cp:lastModifiedBy>\
<dcterms:created xsi:type=\"dcterms:W3CDTF\">{FIXED_DATE}</dcterms:created>\
<dcterms:modified xsi:type=\"dcterms:W3CDTF\">{FIXED_DATE}</dcterms:modified></cp:coreProperties>",
        esc(title)
    )
}

pub fn app() -> String {
    format!(
        "{DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" \
xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">\
<Application>PhxClaw</Application></Properties>"
    )
}

/// Resolve o alvo de uma relacao relativo a pasta da parte de origem.
pub fn resolve(base_dir: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = base_dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relativo_e_absoluto() {
        assert_eq!(
            resolve("xl", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolve("xl", "/xl/sharedStrings.xml"),
            "xl/sharedStrings.xml"
        );
        assert_eq!(
            resolve("ppt/slides", "../slideLayouts/a.xml"),
            "ppt/slideLayouts/a.xml"
        );
    }
}
