# Expose the replication state in config
# 28/08 21:27

import pathlib
p = pathlib.Path("crates/phxsql-server/src/config.rs")
s = p.read_text()
antigo = """            (
                "replicacao_portas",
                Json::Objeto(
                    self.replicacao
                        .portas()
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), Json::texto_de(v)))
                        .collect(),
                ),
            ),"""
novo = """            (
                "replicacao_portas",
                Json::Objeto(
                    self.replicacao
                        .portas()
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), Json::texto_de(v)))
                        .collect(),
                ),
            ),
            // O que a tela da replicacao precisa saber para dizer a verdade:
            // sem a imagem no diario o servidor tem papel de source e nao
            // replica, e a tela ficaria dizendo que esta tudo pronto.
            (
                "imagem_da_linha",
                Json::Bool(self.replicacao.imagem_da_linha),
            ),
            (
                "origens",
                Json::Lista(
                    self.replicacao
                        .origens
                        .iter()
                        .map(|o| {
                            Json::objeto(vec![
                                ("nome", Json::texto_de(&o.nome)),
                                ("host", Json::texto_de(&o.host)),
                                ("porta", Json::de_u64(o.porta as u64)),
                                ("usuario", Json::texto_de(&o.usuario)),
                                ("reconectar_em", Json::de_u64(o.reconectar_em)),
                                (
                                    "databases",
                                    Json::Lista(
                                        o.databases.iter().map(Json::texto_de).collect(),
                                    ),
                                ),
                                // A senha NAO sai daqui, nem o hash: a tela nao
                                // precisa dela e a resposta do protocolo nunca
                                // carrega credencial.
                            ])
                        })
                        .collect(),
                ),
            ),"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
