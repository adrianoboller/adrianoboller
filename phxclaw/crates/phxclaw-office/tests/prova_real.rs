//! Prova real: os arquivos gerados abrem em leitores que nao sao nossos
//! (python-docx, openpyxl, python-pptx e, se houver, o LibreOffice).
//!
//! Sem Python ou sem as bibliotecas os testes pulam com o motivo impresso;
//! com `PHXCLAW_OFFICE_EXIGIR_PROVA=1` o pulo vira falha, para que um ambiente
//! de integracao nao passe verde sem ter provado nada.

use phxclaw_office::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn pasta(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("phxclaw-office-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/validar.py")
}

/// Devolve `false` (e explica) quando o ambiente nao tem como provar.
fn python_disponivel() -> bool {
    let ok = Command::new("python3")
        .args(["-c", "import docx, openpyxl, pptx"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        let msg = "PULADO: python3 com python-docx, openpyxl e python-pptx nao encontrado \
                   (pip install python-docx openpyxl python-pptx)";
        if std::env::var("PHXCLAW_OFFICE_EXIGIR_PROVA").as_deref() == Ok("1") {
            panic!("{msg}");
        }
        eprintln!("{msg}");
    }
    ok
}

fn validar(tipo: &str, arquivo: &Path, modelo_json: String, dir: &Path) {
    let m = dir.join(format!("modelo-{tipo}.json"));
    std::fs::write(&m, modelo_json).unwrap();
    let out = Command::new("python3")
        .arg(script())
        .arg(tipo)
        .arg(arquivo)
        .arg(&m)
        .output()
        .unwrap();
    let saida = String::from_utf8_lossy(&out.stdout);
    let erro = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "validador independente recusou o {tipo}:\n{saida}\n{erro}"
    );
}

/// Converte para PDF com o LibreOffice sem interface. Perfil proprio por
/// chamada: dois `soffice` no mesmo perfil disputam a trava e um deles sai
/// calado sem gerar nada.
fn para_pdf(arquivo: &Path, dir: &Path) -> Option<PathBuf> {
    let soffice = ["soffice", "libreoffice"].into_iter().find(|b| {
        Command::new(b)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });
    let Some(soffice) = soffice else {
        eprintln!("PULADO: LibreOffice nao encontrado; conversao para PDF nao provada");
        return None;
    };
    let perfil = dir.join(format!(
        "perfil-{}",
        arquivo.extension().unwrap().to_string_lossy()
    ));
    let out = Command::new(soffice)
        .arg(format!("-env:UserInstallation=file://{}", perfil.display()))
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(dir)
        .arg(arquivo)
        .output()
        .unwrap();
    let pdf = arquivo.with_extension("pdf");
    assert!(
        pdf.exists(),
        "LibreOffice nao gerou o PDF de {}:\n{}\n{}",
        arquivo.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(
        bytes.starts_with(b"%PDF") && bytes.len() > 1000,
        "PDF invalido"
    );
    Some(pdf)
}

/// Texto do PDF, quando o `pdftotext` existe: e o que prova que a formula foi
/// calculada pelo leitor, e nao so aceita.
fn texto_do_pdf(pdf: &Path) -> Option<String> {
    let out = Command::new("pdftotext").arg(pdf).arg("-").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

// Tudo o que costuma quebrar um OOXML: os cinco caracteres de escape,
// acentos, caractere de controle proibido e quebra de linha.
const ARMADILHA: &str = "Ação & <Relatório> \"aspas\" 'simples'\u{1}\u{b}";

fn documento() -> Document {
    Document {
        title: format!("Relatório técnico — {ARMADILHA}"),
        blocks: vec![
            Block::Heading {
                level: 1,
                text: "Introdução & objetivos".into(),
            },
            Block::Paragraph {
                text: format!("Parágrafo normal com {ARMADILHA}"),
                bold: false,
            },
            Block::Paragraph {
                text: "Conclusão em negrito: pão, maçã, coração.".into(),
                bold: true,
            },
            Block::Heading {
                level: 2,
                text: "Seção 2 <detalhes>".into(),
            },
            Block::Bullets {
                items: vec!["Primeiro item: ç".into(), "Segundo & último".into()],
            },
            Block::Heading {
                level: 3,
                text: "Tabela".into(),
            },
            Block::Table {
                header: vec!["Cidade".into(), "UF".into(), "População".into()],
                rows: vec![
                    vec!["São Paulo".into(), "SP".into(), "11.451.999".into()],
                    vec!["Blumenau".into(), "SC".into()],
                    vec!["Goiânia & região".into(), "GO".into(), "1.437.366".into()],
                ],
            },
        ],
    }
}

fn pasta_de_trabalho() -> Workbook {
    Workbook {
        sheets: vec![
            Sheet {
                name: "Vendas São Paulo".into(),
                bold_header: true,
                rows: vec![
                    vec![
                        "Produto".into(),
                        "Qtd".into(),
                        "Preço".into(),
                        "Ativo".into(),
                    ],
                    vec!["Maçã & pera".into(), 10i64.into(), 2.5.into(), true.into()],
                    vec![ARMADILHA.into(), 3i64.into(), 1234.56.into(), false.into()],
                    vec![
                        "Total".into(),
                        Cell::Formula("=SUM(B2:B3)".into()),
                        Cell::Formula("SUM(C2:C3)".into()),
                    ],
                ],
            },
            Sheet {
                name: "Notas (ç)".into(),
                bold_header: false,
                rows: vec![vec![Cell::Empty, "só na B1".into()], vec![(-0.125).into()]],
            },
        ],
    }
}

fn apresentacao() -> Deck {
    Deck {
        title: format!("Plano de ação — {ARMADILHA}"),
        subtitle: Some("São Paulo, 2026".into()),
        slides: vec![
            Slide {
                title: "Metas & prazos".into(),
                bullets: vec![
                    "Entregar o relatório <final>".into(),
                    "Revisão com \"diretoria\"".into(),
                    "Orçamento: R$ 1.000,00".into(),
                ],
                notes: Some("Falar devagar; citar a ação & o prazo".into()),
            },
            Slide {
                title: "Próximos passos".into(),
                bullets: vec![],
                notes: None,
            },
        ],
    }
}

#[test]
fn docx_abre_nos_leitores_independentes() {
    if !python_disponivel() {
        return;
    }
    let dir = pasta("docx");
    let arq = dir.join("relatorio.docx");
    let doc = documento();
    write_docx(&doc, &arq).unwrap();
    validar("docx", &arq, serde_json::to_string(&doc).unwrap(), &dir);
    if let Some(pdf) = para_pdf(&arq, &dir)
        && let Some(t) = texto_do_pdf(&pdf)
    {
        assert!(
            t.contains("Introdução & objetivos"),
            "PDF sem o titulo:\n{t}"
        );
        assert!(t.contains("Goiânia & região"), "PDF sem a tabela:\n{t}");
    }
    // So apaga no sucesso: na falha a pasta fica para inspecao.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn xlsx_abre_nos_leitores_independentes() {
    if !python_disponivel() {
        return;
    }
    let dir = pasta("xlsx");
    let arq = dir.join("vendas.xlsx");
    let wb = pasta_de_trabalho();
    write_xlsx(&wb, &arq).unwrap();
    validar("xlsx", &arq, serde_json::to_string(&wb).unwrap(), &dir);
    if let Some(pdf) = para_pdf(&arq, &dir)
        && let Some(t) = texto_do_pdf(&pdf)
    {
        // 10+3 e 2,5+1234,56: so aparecem se o LibreOffice calculou o SUM.
        assert!(t.contains("13"), "SUM de quantidades nao calculado:\n{t}");
        assert!(
            t.contains("1237.06") || t.contains("1237,06"),
            "SUM de precos nao calculado:\n{t}"
        );
    }
    // So apaga no sucesso: na falha a pasta fica para inspecao.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pptx_abre_nos_leitores_independentes() {
    if !python_disponivel() {
        return;
    }
    let dir = pasta("pptx");
    let arq = dir.join("plano.pptx");
    let deck = apresentacao();
    write_pptx(&deck, &arq).unwrap();
    validar("pptx", &arq, serde_json::to_string(&deck).unwrap(), &dir);
    if let Some(pdf) = para_pdf(&arq, &dir)
        && let Some(t) = texto_do_pdf(&pdf)
    {
        assert!(t.contains("Metas & prazos"), "PDF sem o slide:\n{t}");
    }
    // So apaga no sucesso: na falha a pasta fica para inspecao.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ida_e_volta_pelos_nossos_leitores() {
    let dir = pasta("ida-volta");
    let d = dir.join("a.docx");
    write_docx(&documento(), &d).unwrap();
    let t = read_docx_text(&d).unwrap();
    assert!(t.starts_with("Relatório técnico — Ação & <Relatório> \"aspas\" 'simples'\n"));
    assert!(t.contains("\nPrimeiro item: ç\nSegundo & último\n"));
    assert!(
        t.contains("\nBlumenau\nSC\n\n"),
        "celula vazia completada: {t:?}"
    );

    let x = dir.join("a.xlsx");
    write_xlsx(&pasta_de_trabalho(), &x).unwrap();
    let wb = read_xlsx_values(&x).unwrap();
    let orig = pasta_de_trabalho();
    assert_eq!(wb.sheets.len(), 2);
    assert_eq!(wb.sheets[0].rows[1], orig.sheets[0].rows[1]);
    assert_eq!(
        wb.sheets[0].rows[2][0],
        Cell::Text("Ação & <Relatório> \"aspas\" 'simples'".into())
    );
    assert_eq!(wb.sheets[0].rows[3][2], Cell::Formula("=SUM(C2:C3)".into()));
    assert_eq!(wb.sheets[1].rows, orig.sheets[1].rows);
    // So apaga no sucesso: na falha a pasta fica para inspecao.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn le_arquivos_comprimidos_de_outro_produtor() {
    if !python_disponivel() {
        return;
    }
    let dir = pasta("outro-produtor");
    let out = Command::new("python3")
        .arg(script())
        .arg("gerar")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let docx = std::fs::read(dir.join("py.docx")).unwrap();
    // Garante que a prova e mesmo do deflate: metodo 8 no primeiro cabecalho.
    assert_eq!(u16::from_le_bytes([docx[8], docx[9]]), 8);
    let t = read_docx_text_bytes(&docx).unwrap();
    assert_eq!(
        t,
        "Título gerado & lido\nParágrafo com ação\tapós tab\nSão Paulo\nSP"
    );
    let wb = read_xlsx_values(dir.join("py.xlsx")).unwrap();
    assert_eq!(wb.sheets[0].name, "Plan ção");
    assert_eq!(wb.sheets[0].rows[0], vec!["maçã".into(), 3.25.into()]);
    assert_eq!(
        wb.sheets[0].rows[2],
        vec![
            Cell::Empty,
            Cell::Empty,
            true.into(),
            Cell::Formula("=B1*2".into())
        ]
    );
    assert_eq!(wb.sheets[1].name, "Segunda");
    assert_eq!(wb.sheets[1].rows[1], vec![Cell::Empty, 7.0.into()]);
    // O python-pptx comprime, nomeia as partes do jeito dele e poe numero de
    // slide em campo (`a:fld`) nas anotacoes, que nao pode vazar como texto.
    let t = read_pptx_text(dir.join("py.pptx")).unwrap();
    assert_eq!(
        t,
        "--- slide 1 ---\nAgenda & ação\nprimeiro ponto\n[notas] lembrar do prazo\n\n--- slide 2 ---\nFim"
    );
    // So apaga no sucesso: na falha a pasta fica para inspecao.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mesma_entrada_mesmos_bytes() {
    assert_eq!(
        docx_bytes(&documento()).unwrap(),
        docx_bytes(&documento()).unwrap()
    );
    assert_eq!(
        xlsx_bytes(&pasta_de_trabalho()).unwrap(),
        xlsx_bytes(&pasta_de_trabalho()).unwrap()
    );
    assert_eq!(
        pptx_bytes(&apresentacao()).unwrap(),
        pptx_bytes(&apresentacao()).unwrap()
    );
}
