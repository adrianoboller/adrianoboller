# Make response building testable
# 27/08 19:37

p='crates/phxsql-server/src/http.rs'
s=open(p).read()
s=s.replace('''/// Monta e envia uma resposta.
pub fn responder(
    fluxo: &mut TcpStream,
    codigo: u16,
    tipo: &str,
    corpo: &str,
) -> std::io::Result<()> {
    let motivo = match codigo {''','''/// Monta o texto completo da resposta HTTP.
///
/// Separada do envio para poder ser conferida em teste -- os cabecalhos de
/// seguranca sao o tipo de coisa que some numa refatoracao sem ninguem notar.
pub fn montar_resposta(codigo: u16, tipo: &str, corpo: &str) -> String {
    let motivo = match codigo {''')
s=s.replace('''    // Cabecalhos de seguranca: a pagina nao carrega nada de fora, nao vai para
    // dentro de um quadro alheio e nao adivinha tipo de conteudo.
    let resposta = format!(''','''    // Cabecalhos de seguranca: a pagina nao carrega nada de fora, nao vai para
    // dentro de um quadro alheio e nao adivinha tipo de conteudo.
    format!(''')
s=s.replace('''         \\r\\n{corpo}",
        corpo.len()
    );
    fluxo.write_all(resposta.as_bytes())?;
    fluxo.flush()
}''','''         \\r\\n{corpo}",
        corpo.len()
    )
}

/// Envia a resposta.
pub fn responder(
    fluxo: &mut TcpStream,
    codigo: u16,
    tipo: &str,
    corpo: &str,
) -> std::io::Result<()> {
    fluxo.write_all(montar_resposta(codigo, tipo, corpo).as_bytes())?;
    fluxo.flush()
}''')
s=s.replace('''    #[test]
    fn a_resposta_traz_os_cabecalhos_de_seguranca() {
        // Monta a resposta pela mesma funcao, num soquete de mentira.
        let esperados = [
            "X-Content-Type-Options: nosniff",
            "X-Frame-Options: DENY",
            "Content-Security-Policy",
            "frame-ancestors 'none'",
            "Cache-Control: no-store",
        ];
        // A funcao escreve direto no fluxo; aqui conferimos o texto que ela
        // monta, reproduzido pela mesma formatacao.
        let corpo = "{}";
        let amostra = format!(
            "HTTP/1.1 200 OK\\r\\nContent-Type: application/json\\r\\nContent-Length: {}\\r\\n\\
             Cache-Control: no-store\\r\\nX-Content-Type-Options: nosniff\\r\\n\\
             X-Frame-Options: DENY\\r\\nReferrer-Policy: no-referrer\\r\\n\\
             Content-Security-Policy: default-src 'none'; frame-ancestors 'none'\\r\\n",
            corpo.len()
        );
        for e in esperados {
            assert!(
                amostra.contains(e) || e == "Content-Security-Policy",
                "faltou {e}"
            );
        }
    }''','''    #[test]
    fn a_resposta_traz_os_cabecalhos_de_seguranca() {
        let r = montar_resposta(200, "application/json", "{\\"ok\\":true}");
        for esperado in [
            "HTTP/1.1 200 OK",
            "Content-Length: 11",
            "Cache-Control: no-store",
            "X-Content-Type-Options: nosniff",
            "X-Frame-Options: DENY",
            "Referrer-Policy: no-referrer",
            "default-src 'none'",
            "frame-ancestors 'none'",
            "connect-src 'self'",
        ] {
            assert!(r.contains(esperado), "faltou o cabecalho: {esperado}");
        }
        assert!(r.ends_with("\\r\\n\\r\\n{\\"ok\\":true}"));
    }

    #[test]
    fn o_tamanho_declarado_bate_com_o_corpo() {
        for corpo in ["", "{}", "ação com acento", &"x".repeat(5_000)] {
            let r = montar_resposta(200, "text/plain", corpo);
            let declarado: usize = r
                .split("Content-Length: ")
                .nth(1)
                .unwrap()
                .split("\\r\\n")
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(declarado, corpo.len(), "corpo de {} bytes", corpo.len());
        }
    }

    #[test]
    fn codigos_de_erro_tem_motivo() {
        for (codigo, motivo) in [
            (400u16, "Bad Request"),
            (403, "Forbidden"),
            (404, "Not Found"),
            (405, "Method Not Allowed"),
            (413, "Payload Too Large"),
        ] {
            assert!(montar_resposta(codigo, "text/plain", "").starts_with(&format!(
                "HTTP/1.1 {codigo} {motivo}"
            )));
        }
    }''')
open(p,'w').write(s)
