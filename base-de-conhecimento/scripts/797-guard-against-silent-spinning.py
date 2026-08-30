# Guard against silent spinning
# 28/08 20:17

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """            Operacao::Exclusao => {
                // A exclusao nao leva imagem: o rowid basta. E ela e FISICA,
                // porque foi fisica no source -- a suave chega como alteracao,
                // que e o que ela e no `.reg`.
                self.excluir_de_vez(rowid, "replicacao")?;
                Ok(rowid)
            }"""
novo = """            Operacao::Exclusao => {
                // A exclusao nao leva imagem: o rowid basta. E ela e FISICA,
                // porque foi fisica no source -- a suave chega como alteracao,
                // que e o que ela e no `.reg`.
                //
                // Nao ter o que excluir e divergencia, e nao um caso benigno:
                // numa replica fiel a linha existe, porque a inclusao dela
                // passou por aqui antes. E se nao para, o evento nao gera
                // evento local, a posicao nao anda, e a replicacao gira em
                // falso puxando o mesmo evento para sempre.
                if !self.excluir_de_vez(rowid, "replicacao")? {
                    return Err(PhxError::Corrompido(format!(
                        "replica divergiu em {}: o source excluiu o rowid {rowid} e \\
                         aqui ele nao existe",
                        self.nome
                    )));
                }
                Ok(rowid)
            }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """            for e in &eventos {
                tabela.aplicar_evento(e.operacao, e.rowid, &e.imagem)?;
                aplicados += 1;
            }
            // A posicao LOCAL, e nao `posicao + eventos.len()`: aplicar gera
            // eventos no diario daqui, e e por ele que a proxima rodada se
            // orienta. Contar do lado do source deixaria os dois numeros
            // andarem separados no primeiro evento que nao gerasse outro.
            posicao = tabela.eventos()?;
        }"""
novo = """            for e in &eventos {
                tabela.aplicar_evento(e.operacao, e.rowid, &e.imagem)?;
                aplicados += 1;
            }
            // A posicao LOCAL, e nao `posicao + eventos.len()`: aplicar gera
            // eventos no diario daqui, e e por ele que a proxima rodada se
            // orienta. Contar do lado do source deixaria os dois numeros
            // andarem separados no primeiro evento que nao gerasse outro.
            let nova = tabela.eventos()?;
            if nova <= posicao {
                // Aplicou e a posicao nao andou: o proximo pedido traria os
                // mesmos eventos, e o laco giraria em falso para sempre.
                return Err(PhxError::Corrompido(format!(
                    "replicacao de {database}.{}: {} evento(s) aplicado(s) e a \\
                     posicao continua em {posicao}",
                    no.nome,
                    eventos.len()
                )));
            }
            posicao = nova;
        }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
