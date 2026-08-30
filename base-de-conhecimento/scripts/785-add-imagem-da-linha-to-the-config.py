# Add imagem_da_linha to the config
# 28/08 20:12

import pathlib
p = pathlib.Path("crates/phxsql-server/src/config.rs")
s = p.read_text()

antigo = """    /// Origens de onde puxar (so na replica). Varias = multi-source.
    pub origens: Vec<Origem>,
}"""
novo = """    /// Origens de onde puxar (so na replica). Varias = multi-source.
    pub origens: Vec<Origem>,
    /// Gravar a imagem da linha no `.log`? So com ela da para REPLICAR.
    ///
    /// Sem ela o evento diz que o rowid 42 mudou e nao diz para que -- basta
    /// para auditoria, nao basta para uma replica aplicar. Custa: um registro
    /// de 200 bytes gasta ~244 bytes de diario por alteracao em vez de 44.
    ///
    /// Liga sozinha quando o papel e `source`, que e quando ela e obrigatoria:
    /// um source sem imagem no diario e um source que nao replica, e descobrir
    /// isso pela replica parada seria o pior jeito de descobrir.
    pub imagem_da_linha: bool,
}"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """            id_servidor: String::new(),
            replicas_autorizadas: Vec::new(),
            origens: Vec::new(),
        }
    }
}"""
novo = """            id_servidor: String::new(),
            replicas_autorizadas: Vec::new(),
            origens: Vec::new(),
            imagem_da_linha: false,
        }
    }
}"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """                    .unwrap_or_default(),
            },
        };"""
novo = """                    .unwrap_or_default(),
                imagem_da_linha: r.booleano_ou(
                    "imagem_da_linha",
                    // O padrao segue o papel: source liga, o resto nao.
                    Papel::de_texto(r.texto_ou("papel", "isolado"))? == Papel::Source,
                ),
            },
        };"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("config ok")
