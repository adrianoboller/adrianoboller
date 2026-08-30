# Capture the HTTP path too
# 28/08 23:00

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """            (None, true) => self.despachar(&pedido.corpo, &mut sessao, ip),
        };
        let remota = ja_remota.is_some() || !servidor_remoto.is_empty();
        let ms = inicio.elapsed().as_millis() as u64;"""
novo = """            (None, true) => {
                // O PROFILER olha aqui tambem. A porta da interface e HTTP e
                // nao JSON por linha, mas o pedido e o mesmo objeto e chega
                // pelo mesmo TCP -- deixar a web de fora faria o profiler
                // mentir por omissao justamente para quem esta olhando por
                // ela.
                let marca = {
                    let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
                    let nome_op = Json::analisar(&pedido.corpo)
                        .ok()
                        .map(|j| j.texto_ou("op", "?").to_string())
                        .unwrap_or_else(|| "?".into());
                    self.profiler.lock().ok().and_then(|mut pr| {
                        pr.chegou(
                            &pedido.corpo,
                            &nome_op,
                            sessao.login(),
                            &alvo.database,
                            &alvo.tabela,
                            ip,
                            agora,
                        )
                    })
                };
                let saida = self.despachar(&pedido.corpo, &mut sessao, ip);
                if let Some(serial) = marca {
                    if let Ok(mut pr) = self.profiler.lock() {
                        pr.terminou(
                            serial,
                            inicio.elapsed().as_millis() as u64,
                            saida.2.is_ok(),
                            &saida.2.as_ref().err().map(|e| e.to_string()).unwrap_or_default(),
                        );
                    }
                }
                saida
            }
        };
        let remota = ja_remota.is_some() || !servidor_remoto.is_embty_marcador();
        let ms = inicio.elapsed().as_millis() as u64;"""
assert antigo in s
s = s.replace(antigo, novo)
# desfaz o marcador de erro proposital
s = s.replace("!servidor_remoto.is_embty_marcador()", "!servidor_remoto.is_empty()")
p.write_text(s)
print("ok")
