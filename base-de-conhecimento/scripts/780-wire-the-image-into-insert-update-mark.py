# Wire the image into insert/update/mark
# 28/08 20:09

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

antigo = """        if nasce_marcada {
            self.reg.mudar_marcadas(1)?;
        }
        self.log.registrar(Operacao::Inclusao, rowid, 1)?;
        Ok(rowid)
    }"""
novo = """        if nasce_marcada {
            self.reg.mudar_marcadas(1)?;
        }
        self.anotar(Operacao::Inclusao, rowid, 1, &payload)?;
        Ok(rowid)
    }

    /// Grava o evento no diario, com a imagem da linha se estiver ligada.
    ///
    /// A imagem custa uma leitura de cada anexo da linha -- e o preco de a
    /// replica receber o conteudo em vez de um ponteiro que so vale aqui. Por
    /// isso ela esta atras do interruptor, e por isso este caminho existe em
    /// vez de a chamada ao `log` estar espalhada.
    fn anotar(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        versao: u64,
        payload: &[u8],
    ) -> Result<()> {
        if !self.imagem_no_diario {
            self.log.registrar(operacao, rowid, versao)?;
            return Ok(());
        }
        let imagem = self.imagem_da_linha(payload)?;
        self.log
            .registrar_com_imagem(operacao, rowid, versao, &imagem)?;
        Ok(())
    }"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """        self.liberar_externos(&ponteiros_antigos)?;
        self.log.registrar(Operacao::Alteracao, rowid, versao)?;
        Ok(())
    }"""
novo = """        self.liberar_externos(&ponteiros_antigos)?;
        self.anotar(Operacao::Alteracao, rowid, versao, &payload)?;
        Ok(())
    }"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """        self.log.registrar(Operacao::Alteracao, rowid, versao)?;
        Ok(true)
    }"""
novo = """        // A marca vai para a replica como ALTERACAO, que e o que ela e no
        // `.reg`: o byte da coluna de sistema mudou e nada mais.
        self.anotar(Operacao::Alteracao, rowid, versao, &payload)?;
        Ok(true)
    }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
