# Add the setters
# 28/08 20:08

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
s = s.replace("""            log,
            lixeira,
            motivos,
        };
        t.gravar_pag()?;""", """            log,
            lixeira,
            motivos,
            imagem_no_diario: false,
        };
        t.gravar_pag()?;""")
s = s.replace("""            log,
            lixeira,
            motivos,
        })
    }

    pub fn nome(&self) -> &str {""", """            log,
            lixeira,
            motivos,
            imagem_no_diario: false,
        })
    }

    /// Liga a imagem da linha no diario. Ver o modulo `log`.
    ///
    /// Desligado por padrao porque custa: um registro de 200 bytes gasta ~244
    /// bytes de diario por alteracao em vez de 36. Quem so quer auditoria nao
    /// paga isso; quem replica precisa dele, porque sem a imagem o evento diz
    /// que o rowid mudou e nao diz para que.
    pub fn com_imagem_no_diario(mut self, ligado: bool) -> Table {
        self.imagem_no_diario = ligado;
        self
    }

    /// O mesmo, sem consumir a tabela -- para quem ja a tem aberta.
    pub fn ligar_imagem_no_diario(&mut self, ligado: bool) {
        self.imagem_no_diario = ligado;
    }

    pub fn imagem_no_diario(&self) -> bool {
        self.imagem_no_diario
    }

    pub fn nome(&self) -> &str {""")
p.write_text(s)
print("ok")
