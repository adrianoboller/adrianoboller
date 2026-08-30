# Add image encode/decode and apply
# 28/08 20:09

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

antigo = """    // ------------------------------------------------------------ leitura
"""
novo = """    // ------------------------------------------------------- replicacao

    /// A imagem da linha: os bytes que a replica precisa para reproduzi-la.
    ///
    /// ```text
    /// [tam_payload u32][payload]
    /// [qtd_externos u16][ (coluna u16, tamanho u32, conteudo) ... ]
    /// ```
    ///
    /// O payload vai CRU, do jeito que esta no `.reg` -- sem reencodar, sem
    /// passar por `Value`, sem perder precisao de decimal nem de data. E o
    /// conteudo dos externos vai junto porque os ponteiros do payload sao
    /// offsets do `.bin` e do `.memo` DAQUI: na outra maquina eles apontariam
    /// para qualquer coisa. E a mesma razao de o `.trash` guardar conteudo.
    pub fn imagem_da_linha(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        let externos = self.conteudo_externo(payload)?;
        let mut out = Vec::with_capacity(payload.len() + 64);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out.extend_from_slice(&(externos.len() as u16).to_le_bytes());
        for (coluna, bytes) in &externos {
            out.extend_from_slice(&coluna.to_le_bytes());
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Ok(out)
    }

    /// Desmonta a imagem. Inversa exata de [`Table::imagem_da_linha`].
    pub fn abrir_imagem(imagem: &[u8]) -> Result<(Vec<u8>, Vec<(u16, Vec<u8>)>)> {
        let curta = || PhxError::Corrompido("imagem de linha truncada".into());
        let ler_u32 = |i: usize| -> Result<u32> {
            imagem
                .get(i..i + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(curta)
        };
        let ler_u16 = |i: usize| -> Result<u16> {
            imagem
                .get(i..i + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .ok_or_else(curta)
        };

        let tam = ler_u32(0)? as usize;
        let payload = imagem.get(4..4 + tam).ok_or_else(curta)?.to_vec();
        let mut i = 4 + tam;
        let qtd = ler_u16(i)? as usize;
        i += 2;
        let mut externos = Vec::with_capacity(qtd);
        for _ in 0..qtd {
            let coluna = ler_u16(i)?;
            let n = ler_u32(i + 2)? as usize;
            i += 6;
            externos.push((coluna, imagem.get(i..i + n).ok_or_else(curta)?.to_vec()));
            i += n;
        }
        Ok((payload, externos))
    }

    /// Aplica um evento vindo do source. **So faz sentido numa replica.**
    ///
    /// # O que ela confere, e por que para em vez de seguir
    ///
    /// O `.reg` nunca reaproveita slot e o rowid e sempre `slot_count + 1`.
    /// Entao, se a replica aplicar TODOS os eventos NA ORDEM e mais ninguem
    /// escrever nela, os rowids saem identicos aos do source -- sem transmitir
    /// nem negociar nada. Isso da uma conferencia forte e de graca: se o rowid
    /// que ela gerou nao bate com o do evento, ela JA divergiu, e continuar so
    /// espalharia a divergencia. E o mesmo comportamento da thread SQL do
    /// MySQL(R) parando num erro.
    ///
    /// Devolve o rowid aplicado.
    pub fn aplicar_evento(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        imagem: &[u8],
    ) -> Result<RowId> {
        match operacao {
            Operacao::Exclusao => {
                // A exclusao nao leva imagem: o rowid basta. E ela e FISICA,
                // porque foi fisica no source -- a suave chega como alteracao,
                // que e o que ela e no `.reg`.
                self.excluir_de_vez(rowid, "replicacao")?;
                Ok(rowid)
            }
            Operacao::Inclusao | Operacao::Alteracao => {
                if imagem.is_empty() {
                    return Err(PhxError::Esquema(format!(
                        "evento de {} no rowid {rowid} veio sem imagem: o source \
                         gravou o diario com `imagem_da_linha` desligada",
                        operacao.nome()
                    )));
                }
                let (payload, externos) = Table::abrir_imagem(imagem)?;
                let valores = self.decodificar_com_externos(&payload, &externos)?;
                if operacao == Operacao::Inclusao {
                    let meu = self.inserir(&valores)?;
                    if meu != rowid {
                        return Err(PhxError::Corrompido(format!(
                            "replica divergiu em {}: o source diz rowid {rowid} e \
                             aqui saiu {meu}. A replicacao para aqui em vez de \
                             espalhar a divergencia",
                            self.nome
                        )));
                    }
                    Ok(meu)
                } else {
                    self.atualizar(rowid, &valores)?;
                    Ok(rowid)
                }
            }
        }
    }

    /// Decodifica um payload usando o conteudo externo da imagem no lugar do
    /// que os ponteiros dele apontariam.
    ///
    /// Os ponteiros do payload sao do OUTRO servidor. Ler por eles aqui daria
    /// bloco errado, ou erro, ou -- pior -- bloco de outra linha.
    fn decodificar_com_externos(
        &mut self,
        payload: &[u8],
        externos: &[(u16, Vec<u8>)],
    ) -> Result<Vec<Value>> {
        // Sem carregar externos: o que voltar nas colunas externas e ponteiro
        // alheio, e vai ser substituido logo abaixo.
        let mut valores = self.decodificar(payload, false)?;
        for i in 0..self.esquema.colunas().len() {
            let ty = self.esquema.colunas()[i].ty;
            if !ty.externo() {
                continue;
            }
            let nulo = payload[i / 8] & (1 << (i % 8)) != 0;
            valores[i] = match externos.iter().find(|(c, _)| *c as usize == i) {
                Some((_, bytes)) => match ty {
                    ColumnType::Bin => Value::Bin(bytes.clone()),
                    ColumnType::Memo => Value::Memo(
                        String::from_utf8(bytes.clone())
                            .map_err(|_| PhxError::Corrompido("memo nao e UTF-8".into()))?,
                    ),
                    _ => Value::Null,
                },
                // Nao veio na imagem: ou a coluna e nula, ou o source nao a
                // mandou. Nulo e a leitura segura -- inventar bytes seria pior.
                None if nulo => Value::Null,
                None => Value::Null,
            };
        }
        Ok(valores)
    }

    // ------------------------------------------------------------ leitura
"""
assert antigo in s
s = s.replace(antigo, novo, 1)
p.write_text(s)
print("ok")
