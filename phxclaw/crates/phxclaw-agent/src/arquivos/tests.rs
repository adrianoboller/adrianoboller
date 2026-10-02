use super::*;
use phxclaw_test_support::pulado;
use std::time::{Duration, Instant};

/// Pasta da tarefa DENTRO de uma base propria: a base e o «fora» que os testes de
/// recusa conferem, e nenhum teste olha o /tmp compartilhado.
fn ambiente(nome: &str) -> (PathBuf, ToolContext) {
    let base =
        std::env::temp_dir().join(format!("phx-arq-{nome}-{}", phxclaw_types::new_uuid_v7()));
    let work = base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: work,
        timeout: Duration::from_secs(120),
    };
    (base, ctx)
}

fn sandbox_ok() -> bool {
    let ok = achar_bwrap().is_some();
    if !ok {
        pulado::pular("bwrap", "bwrap ausente; conversores nao provados");
    }
    ok
}

/// Marca a entrada `idx` do diretorio central como link Unix, como o `zip -y` grava.
fn marcar_link(z: &mut [u8], idx: usize) {
    let mut p = z
        .windows(4)
        .position(|w| w == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    for _ in 0..idx {
        let n = u16::from_le_bytes([z[p + 28], z[p + 29]]) as usize;
        p += 46 + n;
    }
    z[p + 5] = 3;
    z[p + 38..p + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
}

fn gravar_zip(ctx: &ToolContext, rel: &str, entradas: &[(&str, &[u8])]) -> Vec<u8> {
    let e: Vec<(String, Vec<u8>)> = entradas
        .iter()
        .map(|(n, d)| (n.to_string(), d.to_vec()))
        .collect();
    let z = zip::write_store(&e).unwrap();
    std::fs::write(ctx.workdir.join(rel), &z).unwrap();
    z
}

async fn rodar(t: &dyn Tool, ctx: &ToolContext, a: Value) -> Result<ToolOutput, ToolError> {
    t.run(a, ctx).await
}

// ---------------------------------------------------------------- zip

#[tokio::test]
async fn zip_com_entrada_que_sobe_de_pasta_e_recusado_e_nada_e_gravado() {
    let (base, ctx) = ambiente("trav");
    // A entrada boa vem ANTES da hostil: recusar so ao chegar nela deixaria meia extracao.
    // `../fora.txt` sai do destino; `../../fora.txt` sai da pasta da tarefa.
    for hostil in ["../fora.txt", "../../fora.txt"] {
        gravar_zip(&ctx, "h.zip", &[("bom.txt", b"ok"), (hostil, b"escapou")]);
        let r = rodar(
            &ZipTool,
            &ctx,
            json!({"action":"extract","path":"h.zip","dest":"saida"}),
        )
        .await;
        assert!(matches!(r, Err(ToolError::Denied(_))), "{hostil}: {r:?}");
        assert!(!base.join("fora.txt").exists(), "gravou fora da pasta");
        assert!(!ctx.workdir.join("fora.txt").exists(), "saiu do destino");
        assert!(!ctx.workdir.join("saida/bom.txt").exists(), "meia extracao");
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn zip_recusa_caminho_absoluto_letra_de_unidade_e_link() {
    let (base, ctx) = ambiente("abs");
    for nome in ["/tmp/x.txt", "C:/x.txt", "a\\..\\..\\x.txt", "a/./b"] {
        gravar_zip(&ctx, "a.zip", &[(nome, b"x")]);
        let r = rodar(&ZipTool, &ctx, json!({"action":"extract","path":"a.zip"})).await;
        assert!(matches!(r, Err(ToolError::Denied(_))), "{nome}: {r:?}");
    }
    let mut z = zip::write_store(&[("ln".to_string(), b"/etc/passwd".to_vec())]).unwrap();
    marcar_link(&mut z, 0);
    std::fs::write(ctx.workdir.join("l.zip"), &z).unwrap();
    let r = rodar(&ZipTool, &ctx, json!({"action":"extract","path":"l.zip"})).await;
    assert!(
        matches!(&r, Err(ToolError::Denied(m)) if m.contains("link")),
        "{r:?}"
    );
    assert!(!ctx.workdir.join("l").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn zip_recusa_destino_que_e_link_para_fora() {
    let (base, ctx) = ambiente("dest-link");
    std::fs::create_dir_all(base.join("alheio")).unwrap();
    std::os::unix::fs::symlink(base.join("alheio"), ctx.workdir.join("saida")).unwrap();
    gravar_zip(&ctx, "a.zip", &[("x.txt", b"x")]);
    let r = rodar(
        &ZipTool,
        &ctx,
        json!({"action":"extract","path":"a.zip","dest":"saida"}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    assert!(!base.join("alheio/x.txt").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn zip_extrai_cria_e_lista_ida_e_volta() {
    let (base, ctx) = ambiente("ida");
    gravar_zip(
        &ctx,
        "pac.zip",
        &[
            ("pasta/", b""),
            ("pasta/b.txt", "ação".as_bytes()),
            ("a.txt", b"um"),
        ],
    );
    let o = rodar(&ZipTool, &ctx, json!({"action":"extract","path":"pac.zip"}))
        .await
        .unwrap();
    assert!(o.content.contains("extraidos 2 arquivos"), "{}", o.content);
    assert_eq!(o.artifacts.len(), 2);
    assert_eq!(
        std::fs::read_to_string(ctx.workdir.join("pac/pasta/b.txt")).unwrap(),
        "ação"
    );
    // Link dentro da pasta zipada aponta para fora: e pulado, nao seguido.
    std::fs::write(base.join("segredo.txt"), "nao pode entrar").unwrap();
    std::os::unix::fs::symlink(base.join("segredo.txt"), ctx.workdir.join("pac/ln")).unwrap();
    let o = rodar(
        &ZipTool,
        &ctx,
        json!({"action":"create","path":"saida/novo.zip","files":["pac", "pac/a.txt"]}),
    )
    .await
    .unwrap();
    assert!(o.content.contains("2 arquivos"), "{}", o.content);
    assert!(o.content.contains("pac/ln"), "{}", o.content);
    let novo = std::fs::read(ctx.workdir.join("saida/novo.zip")).unwrap();
    let ar = zip::read(&novo).unwrap();
    let nomes: Vec<&str> = ar.names().collect();
    assert_eq!(nomes, ["pac/a.txt", "pac/pasta/b.txt"]);
    let l = rodar(&ZipListTool, &ctx, json!({"path":"saida/novo.zip"}))
        .await
        .unwrap();
    assert!(l.content.contains("2 entradas"), "{}", l.content);
    assert!(l.content.contains("pac/pasta/b.txt\t6\t6"), "{}", l.content);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn zip_recusa_pacote_com_entradas_demais_antes_de_gravar() {
    let (base, ctx) = ambiente("muitas");
    let nomes: Vec<String> = (0..=MAX_ENTRADAS).map(|i| format!("{i}.txt")).collect();
    let e: Vec<(&str, &[u8])> = nomes.iter().map(|n| (n.as_str(), &b""[..])).collect();
    gravar_zip(&ctx, "m.zip", &e);
    let r = rodar(&ZipTool, &ctx, json!({"action":"extract","path":"m.zip"})).await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    assert!(!ctx.workdir.join("m").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn zip_create_recusa_soma_acima_do_teto_sem_ler_o_arquivo() {
    let (base, ctx) = ambiente("grande");
    // Esparso: ocupa nada no disco, mas o metadado diz o tamanho inteiro.
    let f = std::fs::File::create(ctx.workdir.join("grande.bin")).unwrap();
    f.set_len(MAX_CRIAR_BYTES as u64 + 1).unwrap();
    let r = rodar(
        &ZipTool,
        &ctx,
        json!({"action":"create","path":"g.zip","files":["grande.bin"]}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    assert!(!ctx.workdir.join("g.zip").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn capacidades_do_zip() {
    assert_eq!(ZipListTool.capability(), "fs.read");
    assert_eq!(ZipTool.capability(), "fs.write");
    assert_eq!(DataFileTool.capability(), "fs.read");
    assert_eq!(DataFileFormatTool.capability(), "fs.write");
    assert_eq!(PdfTool.capability(), "fs.read");
    assert_eq!(PdfCreateTool.capability(), "fs.write");
}

// ---------------------------------------------------------------- json e xml

#[test]
fn json_bonito_mantem_ordem_e_numero_grande() {
    let src =
        r#"{"z":1,"a":[1, 2,{}, []],"n":123456789012345678901234567890,"s":"x, {\"y\": [1]}"}"#;
    assert_eq!(
        json_bonito(src),
        "{\n  \"z\": 1,\n  \"a\": [\n    1,\n    2,\n    {},\n    []\n  ],\n  \
\"n\": 123456789012345678901234567890,\n  \"s\": \"x, {\\\"y\\\": [1]}\"\n}"
    );
}

#[tokio::test]
async fn data_file_json_valida_com_linha_e_coluna_e_consulta_ponteiro() {
    let (base, ctx) = ambiente("json");
    std::fs::write(
        ctx.workdir.join("ruim.json"),
        "{\n  \"a\": 1,\n  \"b\": \n}",
    )
    .unwrap();
    let r = rodar(
        &DataFileTool,
        &ctx,
        json!({"path":"ruim.json","action":"validate"}),
    )
    .await;
    let Err(ToolError::Failed(m)) = r else {
        panic!("{r:?}")
    };
    assert!(m.contains("linha 4, coluna 1"), "{m}");
    std::fs::write(
        ctx.workdir.join("d.json"),
        "\u{FEFF}{\"itens\":[{\"nome\":\"a/b\"},{\"nome\":\"ç\"}],\"x~y\":2}",
    )
    .unwrap();
    let ok = |a: Value| rodar(&DataFileTool, &ctx, a);
    let v = ok(json!({"path":"d.json","action":"validate"}))
        .await
        .unwrap();
    assert!(v.content.contains("objeto com 2 chaves"), "{}", v.content);
    let q = ok(json!({"path":"d.json","action":"query","query":"/itens/1/nome"}))
        .await
        .unwrap();
    assert_eq!(q.content, "\"ç\"");
    let q = ok(json!({"path":"d.json","action":"query","query":"/x~0y"}))
        .await
        .unwrap();
    assert_eq!(q.content, "2");
    let r = ok(json!({"path":"d.json","action":"query","query":"/itens/5/nome"})).await;
    assert!(
        matches!(&r, Err(ToolError::Failed(m)) if m.contains("\"/itens\"") && m.contains("lista de 2 itens")),
        "{r:?}"
    );
    let r = ok(json!({"path":"d.json","action":"query","query":"itens"})).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
    let r = ok(json!({"path":"../d.json","action":"validate"})).await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn xml_valida_com_posicao() {
    let casos = [
        ("<a>\n  <b></c>\n</a>", "linha 2"),
        ("<a><b></b>", "sem fechamento"),
        ("<a/><b/>", "mais de um elemento raiz"),
        ("<a>&nada;</a>", "entidade nao declarada"),
        ("texto<a/>", "texto fora"),
        ("<?xml version=\"1.0\"?>", "sem elemento raiz"),
    ];
    for (src, esperado) in casos {
        let r = xml_validar(src);
        assert!(
            matches!(&r, Err(ToolError::Failed(m)) if m.contains(esperado) && m.contains("linha")),
            "{src}: {r:?}"
        );
    }
    xml_validar("<?xml version=\"1.0\"?>\n<a x='1'>&amp;&#231;<![CDATA[<z>]]><b/></a>\n").unwrap();
}

#[test]
fn xml_bonito_nao_mexe_no_texto() {
    assert_eq!(
        xml_bonito("<a>\n\n<b>x &amp; y</b>   <c/><d k=\"v\">t</d></a>").unwrap(),
        "<a>\n  <b>x &amp; y</b>\n  <c/>\n  <d k=\"v\">t</d>\n</a>"
    );
}

#[tokio::test]
async fn data_file_xml_consulta_e_formata_gravando() {
    let (base, ctx) = ambiente("xml");
    std::fs::write(
        ctx.workdir.join("c.xml"),
        "<cat xmlns:x='urn:x'><livro id='1'><x:titulo>Dom &amp; Casmurro</x:titulo></livro>\
<livro id='2'><x:titulo>Iracema</x:titulo><vazio/></livro></cat>",
    )
    .unwrap();
    let q = |c: &str| {
        rodar(
            &DataFileTool,
            &ctx,
            json!({"path":"c.xml","action":"query","query":c}),
        )
    };
    assert_eq!(
        q("cat/livro/titulo").await.unwrap().content,
        "[0] Dom & Casmurro\n[1] Iracema"
    );
    assert_eq!(q("/cat/*/@id").await.unwrap().content, "[0] 1\n[1] 2");
    assert_eq!(q("cat/livro/vazio").await.unwrap().content, "[0] ");
    assert!(matches!(q("cat/nada").await, Err(ToolError::Failed(_))));
    let o = rodar(
        &DataFileFormatTool,
        &ctx,
        json!({"path":"c.xml","output":"f/c.xml"}),
    )
    .await
    .unwrap();
    assert_eq!(o.artifacts[0].path, "f/c.xml");
    let f = std::fs::read_to_string(ctx.workdir.join("f/c.xml")).unwrap();
    assert!(
        f.starts_with(
            "<cat xmlns:x='urn:x'>\n  <livro id='1'>\n    <x:titulo>Dom &amp; Casmurro</x:titulo>"
        ),
        "{f}"
    );
    let r = rodar(
        &DataFileFormatTool,
        &ctx,
        json!({"path":"c.xml","output":"c.json"}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- pdf

/// PDF minimo escrito a mao, uma pagina por texto, com a tabela xref nos bytes certos:
/// prova a leitura sem depender do LibreOffice.
fn pdf_minimo(paginas: &[&str]) -> Vec<u8> {
    let n = paginas.len();
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {n} >>",
            (0..n)
                .map(|i| format!("{} 0 R", 4 + 2 * i))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ];
    for (i, t) in paginas.iter().enumerate() {
        let s = format!("BT /F1 24 Tf 72 720 Td ({t}) Tj ET");
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            5 + 2 * i
        ));
        objs.push(format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[tokio::test]
async fn pdf_info_texto_por_faixa_e_read_document() {
    if !sandbox_ok() || exige_programa("pdftotext", "poppler-utils").is_err() {
        pulado::pular("pdftotext", "poppler ausente");
        return;
    }
    let (base, ctx) = ambiente("pdf");
    std::fs::write(
        ctx.workdir.join("d.pdf"),
        pdf_minimo(&["Primeira pagina", "Segunda pagina", "Terceira pagina"]),
    )
    .unwrap();
    let i = rodar(&PdfTool, &ctx, json!({"path":"d.pdf","action":"info"}))
        .await
        .unwrap();
    assert!(i.content.starts_with("paginas: 3\n"), "{}", i.content);
    let t = rodar(
        &PdfTool,
        &ctx,
        json!({"path":"d.pdf","action":"text","first_page":2,"last_page":2}),
    )
    .await
    .unwrap();
    assert!(t.content.contains("Segunda pagina"), "{}", t.content);
    assert!(!t.content.contains("Primeira") && !t.content.contains("Terceira"));
    let todo = ler_documento(&ctx, "d.pdf").await.unwrap();
    assert!(todo.contains("Primeira pagina") && todo.contains("Terceira pagina"));
    let r = rodar(
        &PdfTool,
        &ctx,
        json!({"path":"d.pdf","action":"text","first_page":3,"last_page":1}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))));
    // PDF que nao e PDF: o erro do poppler chega ao modelo, nao um texto vazio.
    std::fs::write(ctx.workdir.join("falso.pdf"), "nada").unwrap();
    let r = rodar(&PdfTool, &ctx, json!({"path":"falso.pdf","action":"info"})).await;
    assert!(matches!(r, Err(ToolError::Failed(_))), "{r:?}");
    let r = rodar(&PdfTool, &ctx, json!({"path":"../d.pdf","action":"info"})).await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn programa_ausente_diz_o_que_instalar() {
    let r = exige_programa_em("pdftotext", "poppler-utils", &["/nao/existe"]);
    assert!(
        matches!(&r, Err(ToolError::Failed(m)) if m.contains("apt install poppler-utils")),
        "{r:?}"
    );
}

fn processo_vivo_com(marca: &str) -> bool {
    std::fs::read_dir("/proc").unwrap().flatten().any(|e| {
        let Ok(c) = std::fs::read(e.path().join("cmdline")) else {
            return false;
        };
        let estado = std::fs::read_to_string(e.path().join("stat")).unwrap_or_default();
        // Zumbi ja morreu; so falta alguem colher.
        let zumbi = estado
            .rsplit(')')
            .next()
            .is_some_and(|s| s.trim_start().starts_with('Z'));
        !zumbi && String::from_utf8_lossy(&c).contains(marca)
    })
}

#[tokio::test]
async fn estouro_do_prazo_mata_o_processo_do_conversor() {
    if !sandbox_ok() {
        return;
    }
    let (base, mut ctx) = ambiente("prazo");
    ctx.timeout = Duration::from_millis(400);
    // Neto em segundo plano: matar so o filho direto deixaria este vivo.
    let marca = format!("31.{}", std::process::id());
    let t0 = Instant::now();
    let r = no_sandbox(
        &ctx,
        format!("sleep {marca} & sleep {marca}"),
        SandboxExtras::default(),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Timeout(400))), "{r:?}");
    assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", t0.elapsed());
    let mut vivo = true;
    for _ in 0..50 {
        vivo = processo_vivo_com(&format!("sleep\0{marca}"));
        if !vivo {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!vivo, "sobrou processo do conversor depois do prazo");
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn pdf_create_converte_txt_e_html_e_recusa_fora_da_pasta() {
    let (base, ctx) = ambiente("soffice");
    let r = rodar(&PdfCreateTool, &ctx, json!({"source":"../x.docx"})).await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    let r = rodar(
        &PdfCreateTool,
        &ctx,
        json!({"source":"a.txt","output":"a.txt"}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
    if !sandbox_ok() || exige_programa("soffice", "libreoffice-writer").is_err() {
        pulado::pular("soffice", "LibreOffice ausente; conversao nao provada");
        let _ = std::fs::remove_dir_all(&base);
        return;
    }
    std::fs::write(
        ctx.workdir.join("a.txt"),
        "Relatorio de vendas\nsegunda linha\n",
    )
    .unwrap();
    std::fs::write(
        ctx.workdir.join("p.html"),
        "<html><body><h1>Titulo da pagina</h1><p>corpo</p></body></html>",
    )
    .unwrap();
    let o = rodar(
        &PdfCreateTool,
        &ctx,
        json!({"source":"a.txt","output":"out/a.pdf"}),
    )
    .await
    .unwrap();
    assert_eq!(o.artifacts[0].path, "out/a.pdf");
    let o2 = rodar(&PdfCreateTool, &ctx, json!({"source":"p.html"}))
        .await
        .unwrap();
    assert_eq!(o2.artifacts[0].path, "p.pdf");
    if exige_programa("pdftotext", "poppler-utils").is_ok() {
        let t = ler_documento(&ctx, "out/a.pdf").await.unwrap();
        assert!(t.contains("Relatorio de vendas"), "{t}");
        let t = ler_documento(&ctx, "p.pdf").await.unwrap();
        assert!(t.contains("Titulo da pagina"), "{t}");
    }
    // O perfil do LibreOffice fica no /tmp do sandbox, nao na pasta da tarefa.
    let sobras: Vec<_> = std::fs::read_dir(&ctx.workdir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let mut sobras = sobras;
    sobras.sort();
    assert_eq!(sobras, ["a.txt", "out", "p.html", "p.pdf"]);
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- read_document

#[tokio::test]
async fn read_document_le_pptx_html_e_texto_pela_ferramenta() {
    let (base, ctx) = ambiente("leitura");
    let deck = phxclaw_office::Deck {
        title: "Plano".into(),
        subtitle: None,
        slides: vec![phxclaw_office::Slide {
            title: "Metas".into(),
            bullets: vec!["vender".into()],
            notes: Some("lembrar".into()),
        }],
    };
    phxclaw_office::write_pptx(&deck, ctx.workdir.join("d.pptx")).unwrap();
    std::fs::write(
        ctx.workdir.join("p.html"),
        "<html><head><title>T</title><style>.x{}</style></head><body><p>Ola &amp; tchau</p></body></html>",
    )
    .unwrap();
    std::fs::write(ctx.workdir.join("g.md"), "x".repeat(MAX_CHARS + 10)).unwrap();
    let ler = crate::adaptadores::OfficeTool {
        kind: crate::adaptadores::OfficeKind::Read,
    };
    let t = rodar(&ler, &ctx, json!({"path":"d.pptx"})).await.unwrap();
    assert_eq!(
        t.content,
        "--- slide 1 ---\nPlano\n\n--- slide 2 ---\nMetas\nvender\n[notas] lembrar"
    );
    let h = rodar(&ler, &ctx, json!({"path":"p.html"})).await.unwrap();
    assert!(
        h.content.contains("(titulo: T) ---\nOla & tchau\n"),
        "{}",
        h.content
    );
    assert!(h.content.contains("--- HTML ---\n<html>"), "{}", h.content);
    let g = rodar(&ler, &ctx, json!({"path":"g.md"})).await.unwrap();
    assert!(
        g.content.contains("truncado"),
        "{}",
        &g.content[MAX_CHARS..]
    );
    let r = rodar(&ler, &ctx, json!({"path":"x.exe"})).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
    let r = rodar(&ler, &ctx, json!({"path":"../p.html"})).await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    let _ = std::fs::remove_dir_all(&base);
}
