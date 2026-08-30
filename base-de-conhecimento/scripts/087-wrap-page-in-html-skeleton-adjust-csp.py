# Wrap page in HTML skeleton, adjust CSP
# 27/08 19:47

p='crates/phxsql-server/src/http.rs'
s=open(p).read()

s = s.replace('''/// A interface, embutida no binario em tempo de compilacao.
pub const PAGINA: &str = include_str!("../ui/index.html");''','''/// A interface, embutida no binario em tempo de compilacao.
///
/// E um FRAGMENTO de proposito -- comeca no `<title>`, sem `<!doctype>` nem
/// `<html>`. O mesmo arquivo e publicado como artefato na web, onde o
/// esqueleto vem de fora; aqui ele e montado por [`montar_pagina`]. Um arquivo
/// so, servido nos dois lugares, sem risco de divergirem.
pub const PAGINA: &str = include_str!("../ui/index.html");

/// Envolve o fragmento no esqueleto que o navegador espera.
///
/// Sem `<!doctype html>` o navegador entra em modo de compatibilidade e o
/// layout muda -- e o tipo de defeito que so aparece na maquina do usuario.
/// O `<title>`, o `<meta>` e os `<link>` do fragmento sao subidos para o
/// cabecalho pelo proprio analisador de HTML, exatamente como acontece quando
/// a pagina e publicada como artefato.
pub fn montar_pagina() -> String {
    format!(
        "<!doctype html>\\n<html lang=\\"pt-BR\\">\\n<head>\\n<meta charset=\\"utf-8\\">\\n</head>\\n<body>\\n{PAGINA}\\n</body>\\n</html>\\n"
    )
}''')

# CSP: HTML precisa da fonte da marca; o resto continua no regime fechado.
velho = '''    // Cabecalhos de seguranca: a pagina nao carrega nada de fora, nao vai para
    // dentro de um quadro alheio e nao adivinha tipo de conteudo.
    format!(
        "HTTP/1.1 {codigo} {motivo}\\r\\n\\
         Content-Type: {tipo}\\r\\n\\
         Content-Length: {}\\r\\n\\
         Cache-Control: no-store\\r\\n\\
         X-Content-Type-Options: nosniff\\r\\n\\
         X-Frame-Options: DENY\\r\\n\\
         Referrer-Policy: no-referrer\\r\\n\\
         Content-Security-Policy: default-src 'none'; \\
         style-src 'unsafe-inline'; script-src 'unsafe-inline'; \\
         img-src data:; connect-src 'self'; form-action 'none'; \\
         frame-ancestors 'none'; base-uri 'none'\\r\\n\\
         Connection: close\\r\\n\\
         \\r\\n{corpo}",
        corpo.len()
    )'''
novo = '''    // Cabecalhos de seguranca: a pagina nao vai para dentro de um quadro
    // alheio, nao adivinha tipo de conteudo e so conversa com esta origem.
    //
    // A unica coisa que ela busca fora e a fonte da marca, e so no HTML --
    // por isso a folga do `style-src`/`font-src` nao existe nas respostas de
    // dados. Servidor sem internet: a fonte nao carrega, a pilha de reserva
    // assume e a pagina continua inteira.
    let externo = tipo.starts_with("text/html");
    let estilo = if externo {
        "style-src 'unsafe-inline' https://fonts.googleapis.com; \\
         font-src https://fonts.gstatic.com; "
    } else {
        "style-src 'unsafe-inline'; "
    };
    format!(
        "HTTP/1.1 {codigo} {motivo}\\r\\n\\
         Content-Type: {tipo}\\r\\n\\
         Content-Length: {}\\r\\n\\
         Cache-Control: no-store\\r\\n\\
         X-Content-Type-Options: nosniff\\r\\n\\
         X-Frame-Options: DENY\\r\\n\\
         Referrer-Policy: no-referrer\\r\\n\\
         Content-Security-Policy: default-src 'none'; {estilo}\\
         script-src 'unsafe-inline'; \\
         img-src data:; connect-src 'self'; form-action 'none'; \\
         frame-ancestors 'none'; base-uri 'none'\\r\\n\\
         Connection: close\\r\\n\\
         \\r\\n{corpo}",
        corpo.len()
    )'''
assert s.count(velho)==1, "CSP nao casou"
s = s.replace(velho, novo)
open(p,'w').write(s)
print("http.rs ok")
