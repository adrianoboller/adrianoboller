# Fix the cascade bug
# 28/08 20:27

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """                None => return Ok(0),
            },
        };

        let mut posicao = tabela.eventos()?;"""
novo = """                None => return Ok(0),
            },
        };
        // O diario DESTA replica tambem carrega a imagem quando configurado.
        // Sem isto, uma replica intermediaria grava eventos sem linha dentro, e
        // a replica que puxa DELA nao tem o que aplicar -- a cascata
        // Master -> Slave01 -> Slave02 morre no segundo salto. Este caminho
        // abre a tabela direto, sem passar pelo `abrir_travada` que liga a
        // imagem para os pedidos que vem pela porta.
        tabela.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);

        let mut posicao = tabela.eventos()?;"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
