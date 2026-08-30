# Fix Nagle and mark rownum as a system column
# 28/08 18:39

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

velho='''        for conexao in ouvinte.incoming() {
            match conexao {
                Ok(fluxo) => {
                    let par = fluxo.peer_addr().ok();'''
novo='''        for conexao in ouvinte.incoming() {
            match conexao {
                Ok(fluxo) => {
                    // Sem isto, o Nagle segura a resposta ate 40 ms esperando
                    // mais bytes para encher um pacote -- e nunca vem mais,
                    // porque a resposta acabou. Medido: a pagina de uma tabela
                    // de 20.000 linhas levava 1 ms de servidor e 44 ms de
                    // relogio, e 43 deles eram esta linha faltando.
                    //
                    // O protocolo aqui e pedido-resposta curto, que e o caso
                    // exato em que o Nagle atrapalha em vez de ajudar.
                    let _ = fluxo.set_nodelay(true);
                    let par = fluxo.peer_addr().ok();'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''                let fluxo = match conexao {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let par = fluxo'''
novo2='''                let fluxo = match conexao {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                // Mesma razao da porta de dados: resposta curta, e o Nagle
                // segurando cada clique da tela por 40 ms.
                let _ = fluxo.set_nodelay(true);
                let par = fluxo'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
