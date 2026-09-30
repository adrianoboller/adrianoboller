//! PresentationML (.pptx): um master, dois layouts (titulo; titulo e
//! conteudo), um tema, e o master de anotacoes so quando ha anotacao.

use crate::opc::{self, NS_A, NS_P, NS_R, rel};
use crate::xml::{DECL, esc};
use crate::{Deck, Result, zip};
use std::path::Path;

const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml";
// 16:9 em EMU.
const SLIDE_W: u64 = 12_192_000;
const SLIDE_H: u64 = 6_858_000;

fn ns() -> String {
    format!("xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\"")
}

const GRP: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/>\
<a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";

/// Paragrafos de texto; `\n` dentro do item vira `a:br` (quebra no mesmo
/// marcador), porque cada item e um marcador so. Lista vazia ainda precisa de
/// um `a:p`, que o esquema exige em todo `txBody`.
fn paras(items: &[&str]) -> String {
    let mut s = String::new();
    for it in items {
        s.push_str("<a:p>");
        for (i, line) in it.split('\n').enumerate() {
            if i > 0 {
                s.push_str("<a:br><a:rPr lang=\"pt-BR\"/></a:br>");
            }
            let line = line.replace('\r', "");
            if !line.is_empty() {
                s.push_str(&format!(
                    "<a:r><a:rPr lang=\"pt-BR\" dirty=\"0\"/><a:t>{}</a:t></a:r>",
                    esc(&line)
                ));
            }
        }
        s.push_str("</a:p>");
    }
    if items.is_empty() {
        s.push_str("<a:p><a:endParaRPr lang=\"pt-BR\"/></a:p>");
    }
    s
}

/// Forma de placeholder. `xfrm` so no master/layout: no slide a posicao e
/// herdada, que e o que permite trocar o layout sem mexer nos slides.
fn ph(
    id: u32,
    name: &str,
    ph_attrs: &str,
    xfrm: Option<(u64, u64, u64, u64)>,
    body: &str,
) -> String {
    ph_estilo(id, name, ph_attrs, xfrm, "<a:lstStyle/>", body)
}

/// Como `ph`, com `lstStyle` proprio: o subtitulo da capa herda o estilo de
/// corpo do master (com marcador), e sem sobrepor aqui a capa sai com um
/// marcador na frente do subtitulo.
fn ph_estilo(
    id: u32,
    name: &str,
    ph_attrs: &str,
    xfrm: Option<(u64, u64, u64, u64)>,
    lst: &str,
    body: &str,
) -> String {
    let sppr = match xfrm {
        Some((x, y, cx, cy)) => format!(
            "<p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>"
        ),
        None => "<p:spPr/>".to_string(),
    };
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr>\
<p:nvPr><p:ph {ph_attrs}/></p:nvPr></p:nvSpPr>{sppr}<p:txBody><a:bodyPr/>{lst}{body}</p:txBody></p:sp>"
    )
}

const TITLE_BOX: (u64, u64, u64, u64) = (838_200, 365_125, 10_515_600, 1_325_563);
const BODY_BOX: (u64, u64, u64, u64) = (838_200, 1_825_625, 10_515_600, 4_351_338);
const CLRMAP: &str = "bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" \
accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"";

fn master_xml() -> String {
    let lvl = |sz: u32, bullet: bool| {
        let bu = if bullet {
            "<a:buFont typeface=\"Arial\"/><a:buChar char=\"\u{2022}\"/>"
        } else {
            "<a:buNone/>"
        };
        let ind = if bullet {
            " marL=\"228600\" indent=\"-228600\""
        } else {
            ""
        };
        format!(
            "<a:lvl1pPr{ind}>{bu}<a:defRPr sz=\"{sz}\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/>\
</a:solidFill><a:latin typeface=\"+mn-lt\"/></a:defRPr></a:lvl1pPr>"
        )
    };
    format!(
        "{DECL}<p:sldMaster {}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
<p:spTree>{GRP}{}{}</p:spTree></p:cSld><p:clrMap {CLRMAP}/>\
<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/><p:sldLayoutId id=\"2147483650\" r:id=\"rId2\"/>\
</p:sldLayoutIdLst><p:txStyles><p:titleStyle>{}</p:titleStyle><p:bodyStyle>{}</p:bodyStyle>\
<p:otherStyle>{}</p:otherStyle></p:txStyles></p:sldMaster>",
        ns(),
        ph(
            2,
            "Title Placeholder 1",
            "type=\"title\"",
            Some(TITLE_BOX),
            &paras(&["Titulo"])
        ),
        ph(
            3,
            "Text Placeholder 2",
            "type=\"body\" idx=\"1\"",
            Some(BODY_BOX),
            &paras(&["Texto"])
        ),
        lvl(4400, false),
        lvl(2800, true),
        lvl(1800, false),
    )
}

fn layout_xml(title_slide: bool) -> String {
    let (ty, name, shapes) = if title_slide {
        (
            "title",
            "Title Slide",
            format!(
                "{}{}",
                ph(
                    2,
                    "Title 1",
                    "type=\"ctrTitle\"",
                    Some((1_524_000, 1_122_363, 9_144_000, 2_387_600)),
                    &paras(&[])
                ),
                ph_estilo(
                    3,
                    "Subtitle 2",
                    "type=\"subTitle\" idx=\"1\"",
                    Some((1_524_000, 3_602_038, 9_144_000, 1_655_762)),
                    "<a:lstStyle><a:lvl1pPr marL=\"0\" indent=\"0\" algn=\"ctr\"><a:buNone/>\
<a:defRPr sz=\"2400\"/></a:lvl1pPr></a:lstStyle>",
                    &paras(&[])
                )
            ),
        )
    } else {
        (
            "obj",
            "Title and Content",
            format!(
                "{}{}",
                ph(2, "Title 1", "type=\"title\"", None, &paras(&[])),
                ph(3, "Content Placeholder 2", "idx=\"1\"", None, &paras(&[]))
            ),
        )
    };
    format!(
        "{DECL}<p:sldLayout {} type=\"{ty}\" preserve=\"1\"><p:cSld name=\"{name}\"><p:spTree>{GRP}{shapes}\
</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>",
        ns()
    )
}

fn theme_xml() -> String {
    let clr = |n: &str, v: &str| format!("<a:{n}><a:srgbClr val=\"{v}\"/></a:{n}>");
    let fill = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>";
    let line = format!("<a:ln w=\"6350\">{fill}</a:ln>");
    let fonts = "<a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>";
    format!(
        "{DECL}<a:theme xmlns:a=\"{NS_A}\" name=\"PhxClaw\"><a:themeElements><a:clrScheme name=\"PhxClaw\">\
<a:dk1><a:sysClr val=\"windowText\" lastClr=\"000000\"/></a:dk1><a:lt1><a:sysClr val=\"window\" lastClr=\"FFFFFF\"/></a:lt1>\
{}{}{}{}{}{}{}{}{}{}</a:clrScheme><a:fontScheme name=\"PhxClaw\"><a:majorFont>{fonts}</a:majorFont>\
<a:minorFont>{fonts}</a:minorFont></a:fontScheme><a:fmtScheme name=\"PhxClaw\">\
<a:fillStyleLst>{fill}{fill}{fill}</a:fillStyleLst><a:lnStyleLst>{line}{line}{line}</a:lnStyleLst>\
<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle>\
<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>\
<a:bgFillStyleLst>{fill}{fill}{fill}</a:bgFillStyleLst></a:fmtScheme></a:themeElements>\
<a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        clr("dk2", "1F2937"),
        clr("lt2", "E5E7EB"),
        clr("accent1", "1D4ED8"),
        clr("accent2", "C63C0A"),
        clr("accent3", "15803D"),
        clr("accent4", "A16207"),
        clr("accent5", "7E22CE"),
        clr("accent6", "0E7490"),
        clr("hlink", "1D4ED8"),
        clr("folHlink", "7E22CE"),
    )
}

fn notes_master_xml() -> String {
    format!(
        "{DECL}<p:notesMaster {}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
<p:spTree>{GRP}{}</p:spTree></p:cSld><p:clrMap {CLRMAP}/></p:notesMaster>",
        ns(),
        ph(
            2,
            "Notes Placeholder 1",
            "type=\"body\" idx=\"1\"",
            Some((685_800, 4_400_550, 5_486_400, 3_600_450)),
            &paras(&[])
        )
    )
}

fn notes_xml(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    format!(
        "{DECL}<p:notes {}><p:cSld><p:spTree>{GRP}{}</p:spTree></p:cSld>\
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>",
        ns(),
        ph(
            2,
            "Notes Placeholder 1",
            "type=\"body\" idx=\"1\"",
            None,
            &paras(&lines)
        )
    )
}

fn slide_xml(shapes: &str) -> String {
    format!(
        "{DECL}<p:sld {}><p:cSld><p:spTree>{GRP}{shapes}</p:spTree></p:cSld>\
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
        ns()
    )
}

pub fn pptx_bytes(deck: &Deck) -> Result<Vec<u8>> {
    // Slide 1 e o de titulo; os demais sao de conteudo, na ordem dada.
    let mut slides: Vec<(String, usize, Option<&str>)> = Vec::new();
    let sub = deck
        .subtitle
        .as_deref()
        .map(|s| vec![s])
        .unwrap_or_default();
    slides.push((
        slide_xml(&format!(
            "{}{}",
            ph(
                2,
                "Title 1",
                "type=\"ctrTitle\"",
                None,
                &paras(&[&deck.title])
            ),
            ph(
                3,
                "Subtitle 2",
                "type=\"subTitle\" idx=\"1\"",
                None,
                &paras(&sub)
            )
        )),
        1,
        None,
    ));
    for s in &deck.slides {
        let items: Vec<&str> = s.bullets.iter().map(String::as_str).collect();
        slides.push((
            slide_xml(&format!(
                "{}{}",
                ph(2, "Title 1", "type=\"title\"", None, &paras(&[&s.title])),
                ph(
                    3,
                    "Content Placeholder 2",
                    "idx=\"1\"",
                    None,
                    &paras(&items)
                )
            )),
            2,
            s.notes.as_deref(),
        ));
    }
    let has_notes = slides.iter().any(|s| s.2.is_some());

    let sld_ct = format!("{PML}.slide+xml");
    let notes_ct = format!("{PML}.notesSlide+xml");
    let main_ct = format!("{PML}.presentation.main+xml");
    let master_ct = format!("{PML}.slideMaster+xml");
    let layout_ct = format!("{PML}.slideLayout+xml");
    let nm_ct = format!("{PML}.notesMaster+xml");
    let props_ct = format!("{PML}.presProps+xml");
    let theme_ct = "application/vnd.openxmlformats-officedocument.theme+xml";
    let mut over: Vec<(String, &str)> = vec![
        ("/ppt/presentation.xml".into(), &main_ct),
        ("/ppt/presProps.xml".into(), &props_ct),
        ("/ppt/slideMasters/slideMaster1.xml".into(), &master_ct),
        ("/ppt/slideLayouts/slideLayout1.xml".into(), &layout_ct),
        ("/ppt/slideLayouts/slideLayout2.xml".into(), &layout_ct),
        ("/ppt/theme/theme1.xml".into(), theme_ct),
    ];
    if has_notes {
        over.push(("/ppt/notesMasters/notesMaster1.xml".into(), &nm_ct));
        over.push(("/ppt/theme/theme2.xml".into(), theme_ct));
    }
    let mut pres_rels = vec![
        rel("rId1", "slideMaster", "slideMasters/slideMaster1.xml"),
        rel("rId2", "theme", "theme/theme1.xml"),
        rel("rId3", "presProps", "presProps.xml"),
    ];
    if has_notes {
        pres_rels.push(rel("rId4", "notesMaster", "notesMasters/notesMaster1.xml"));
    }
    let mut sld_ids = String::new();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut notes_n = 0;
    for (i, (xml, layout, notes)) in slides.iter().enumerate() {
        let n = i + 1;
        over.push((format!("/ppt/slides/slide{n}.xml"), &sld_ct));
        pres_rels.push(rel(
            &format!("rId{}", 10 + n),
            "slide",
            &format!("slides/slide{n}.xml"),
        ));
        sld_ids.push_str(&format!(
            "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
            255 + n,
            10 + n
        ));
        let mut srels = vec![rel(
            "rId1",
            "slideLayout",
            &format!("../slideLayouts/slideLayout{layout}.xml"),
        )];
        entries.push((format!("ppt/slides/slide{n}.xml"), xml.clone().into_bytes()));
        if let Some(t) = notes {
            notes_n += 1;
            over.push((
                format!("/ppt/notesSlides/notesSlide{notes_n}.xml"),
                &notes_ct,
            ));
            srels.push(rel(
                "rId2",
                "notesSlide",
                &format!("../notesSlides/notesSlide{notes_n}.xml"),
            ));
            entries.push((
                format!("ppt/notesSlides/notesSlide{notes_n}.xml"),
                notes_xml(t).into_bytes(),
            ));
            entries.push((
                format!("ppt/notesSlides/_rels/notesSlide{notes_n}.xml.rels"),
                opc::relationships(&[
                    rel("rId1", "notesMaster", "../notesMasters/notesMaster1.xml"),
                    rel("rId2", "slide", &format!("../slides/slide{n}.xml")),
                ])
                .into_bytes(),
            ));
        }
        entries.push((
            format!("ppt/slides/_rels/slide{n}.xml.rels"),
            opc::relationships(&srels).into_bytes(),
        ));
    }
    let nm_list = if has_notes {
        "<p:notesMasterIdLst><p:notesMasterId r:id=\"rId4\"/></p:notesMasterIdLst>"
    } else {
        ""
    };
    let pres = format!(
        "{DECL}<p:presentation {} saveSubsetFonts=\"1\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/>\
</p:sldMasterIdLst>{nm_list}<p:sldIdLst>{sld_ids}</p:sldIdLst><p:sldSz cx=\"{SLIDE_W}\" cy=\"{SLIDE_H}\"/>\
<p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>",
        ns()
    );
    let layout_rels = opc::relationships(&[rel(
        "rId1",
        "slideMaster",
        "../slideMasters/slideMaster1.xml",
    )]);
    let mut all = vec![
        (
            "[Content_Types].xml".to_string(),
            opc::content_types(&over).into_bytes(),
        ),
        (
            "_rels/.rels".into(),
            opc::root_rels("ppt/presentation.xml").into_bytes(),
        ),
        (
            "docProps/core.xml".into(),
            opc::core(&deck.title).into_bytes(),
        ),
        ("docProps/app.xml".into(), opc::app().into_bytes()),
        ("ppt/presentation.xml".into(), pres.into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            opc::relationships(&pres_rels).into_bytes(),
        ),
        (
            "ppt/presProps.xml".into(),
            format!("{DECL}<p:presentationPr {}/>", ns()).into_bytes(),
        ),
        (
            "ppt/slideMasters/slideMaster1.xml".into(),
            master_xml().into_bytes(),
        ),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
            opc::relationships(&[
                rel("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                rel("rId2", "slideLayout", "../slideLayouts/slideLayout2.xml"),
                rel("rId3", "theme", "../theme/theme1.xml"),
            ])
            .into_bytes(),
        ),
        (
            "ppt/slideLayouts/slideLayout1.xml".into(),
            layout_xml(true).into_bytes(),
        ),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
            layout_rels.clone().into_bytes(),
        ),
        (
            "ppt/slideLayouts/slideLayout2.xml".into(),
            layout_xml(false).into_bytes(),
        ),
        (
            "ppt/slideLayouts/_rels/slideLayout2.xml.rels".into(),
            layout_rels.into_bytes(),
        ),
        ("ppt/theme/theme1.xml".into(), theme_xml().into_bytes()),
    ];
    if has_notes {
        all.push((
            "ppt/notesMasters/notesMaster1.xml".into(),
            notes_master_xml().into_bytes(),
        ));
        all.push((
            "ppt/notesMasters/_rels/notesMaster1.xml.rels".into(),
            opc::relationships(&[rel("rId1", "theme", "../theme/theme2.xml")]).into_bytes(),
        ));
        all.push(("ppt/theme/theme2.xml".into(), theme_xml().into_bytes()));
    }
    all.extend(entries);
    zip::write_store(&all)
}

pub fn write_pptx(deck: &Deck, path: impl AsRef<Path>) -> Result<()> {
    crate::write_file(path.as_ref(), &pptx_bytes(deck)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Slide;

    fn exemplo(notes: bool) -> Deck {
        Deck {
            title: "Plano & Metas".into(),
            subtitle: Some("São Paulo".into()),
            slides: vec![Slide {
                title: "Ações <já>".into(),
                bullets: vec!["um".into(), "dois".into()],
                notes: notes.then(|| "falar devagar".into()),
            }],
        }
    }

    #[test]
    fn deterministico_e_bem_formado() {
        for n in [false, true] {
            let a = pptx_bytes(&exemplo(n)).unwrap();
            assert_eq!(a, pptx_bytes(&exemplo(n)).unwrap());
            let ar = zip::read(&a).unwrap();
            for name in ar.names() {
                let s = String::from_utf8(ar.get(name).unwrap().to_vec()).unwrap();
                crate::xml::events(&s).unwrap();
            }
            assert_eq!(ar.get("ppt/notesMasters/notesMaster1.xml").is_some(), n);
        }
    }
}
