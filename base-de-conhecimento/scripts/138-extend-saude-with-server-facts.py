# Extend /saude with server facts
# 27/08 20:35

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
velho='''                let _ = http::responder_json(
                    &mut fluxo,
                    200,
                    &Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("phxsql", Json::texto_de(VERSAO)),
                    ]),
                );'''
novo='''                // Diz o que a pagina precisa para montar o formulario: a porta
                // que este servidor REALMENTE escuta (nao a de fabrica), os
                // destinos que ela pode alcancar e se ha chave a informar.
                // Nada aqui e segredo, e nada aqui depende de token.
                let _ = http::responder_json(
                    &mut fluxo,
                    200,
                    &Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("phxsql", Json::texto_de(VERSAO)),
                        (
                            "porta_dados",
                            Json::de_u64(
                                self.config.endereco().map(|e| e.port()).unwrap_or(0) as u64
                            ),
                        ),
                        (
                            "destinos",
                            Json::Lista(
                                self.config.web.destinos.iter().map(Json::texto_de).collect(),
                            ),
                        ),
                        (
                            "exige_chave",
                            Json::Bool(self.config.cadastro.alguem_exige_chave()),
                        ),
                    ]),
                );'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
