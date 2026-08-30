# Implementar a marca no percorrer
# 29/08 03:45

import io
p='crates/phxsql-store/src/log.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    fn percorrer(
        &mut self,
        pular: u64,
        limite: u64,
        com_imagem: bool,
    ) -> Result<Vec<(Evento, Vec<u8>)>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            if vistos + cab.qtd_eventos <= pular {
                vistos += cab.qtd_eventos;
                continue;
            }
            let mut offset = CAB_LEN as u64;
            while offset + EVENTO_CAB as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_CAB];
                self.volumes.ler(volume, offset, &mut buf)?;
                let evento = Evento::ler(&buf)?;
                if vistos >= pular {
                    let mut imagem = Vec::new();
                    if evento.tam_imagem > 0 {
                        imagem = vec![0u8; evento.tam_imagem as usize];
                        self.volumes
                            .ler(volume, offset + EVENTO_CAB as u64, &mut imagem)?;
                        evento.conferir(&buf, &imagem)?;
                        if !com_imagem {
                            imagem.clear();
                        }
                    }
                    saida.push((evento, imagem));
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += evento.ocupa();
            }
        }
        Ok(saida)
    }'''
novo = '''    fn percorrer(
        &mut self,
        pular: u64,
        limite: u64,
        com_imagem: bool,
    ) -> Result<Vec<(Evento, Vec<u8>)>> {
        let mut saida = Vec::new();

        // De onde comecar. A marca so serve para uma posicao que esteja DEPOIS
        // dela: caminhar para tras nao da, o evento nao tem largura fixa.
        let (mut vistos, comeco) = match self.marca {
            Some(m) if m.evento <= pular => (m.evento, Some(m)),
            _ => (0, None),
        };

        for volume in self.volumes.existentes() {
            if let Some(m) = comeco {
                if volume < m.volume {
                    continue; // ja contado dentro do `vistos` da marca
                }
            }
            let cab = self.cab(volume)?;
            // O volume inteiro se pula de graca pelo `qtd_eventos` do
            // cabecalho -- mas so quando `vistos` esta no comeco dele, e nao
            // no meio, que e onde a marca pode ter parado.
            let no_comeco_do_volume = comeco.is_none_or(|m| m.volume != volume);
            if no_comeco_do_volume && vistos + cab.qtd_eventos <= pular {
                vistos += cab.qtd_eventos;
                continue;
            }
            let mut offset = match comeco {
                Some(m) if m.volume == volume => m.offset,
                _ => CAB_LEN as u64,
            };
            while offset + EVENTO_CAB as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_CAB];
                self.volumes.ler(volume, offset, &mut buf)?;
                let evento = Evento::ler(&buf)?;
                if vistos >= pular {
                    let mut imagem = Vec::new();
                    if evento.tam_imagem > 0 {
                        imagem = vec![0u8; evento.tam_imagem as usize];
                        self.volumes
                            .ler(volume, offset + EVENTO_CAB as u64, &mut imagem)?;
                        evento.conferir(&buf, &imagem)?;
                        if !com_imagem {
                            imagem.clear();
                        }
                    }
                    saida.push((evento, imagem));
                    if limite > 0 && saida.len() as u64 >= limite {
                        // A marca aponta para o PROXIMO, que e o que o leitor
                        // sequencial vai pedir na chamada seguinte.
                        self.marca = Some(MarcaDoDiario {
                            evento: vistos + 1,
                            volume,
                            offset: offset + evento.ocupa(),
                        });
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += evento.ocupa();
            }
        }
        Ok(saida)
    }

    /// Onde a ultima varredura parou. Ver [`MarcaDoDiario`].
    ///
    /// O servidor abre e fecha a tabela a cada pedido, entao a marca morreria
    /// entre um `replicar` e o seguinte -- que sao justamente os dois pedidos
    /// em que ela vale. Exportar e reimportar deixa quem sabe que os pedidos
    /// sao seguidos guardar a dica, do mesmo jeito que a paginacao ja faz com
    /// o cursor.
    pub fn marca(&self) -> Option<MarcaDoDiario> {
        self.marca
    }

    /// Aceita uma dica de onde comecar. Ver [`MarcaDoDiario`].
    ///
    /// Nao ha o que validar aqui, e de proposito: uma marca errada faz a
    /// varredura comecar no lugar errado e o evento lido nao passar no CRC, ou
    /// o offset cair depois do `fim` e a leitura devolver vazio. Nenhum dos
    /// dois entrega dado errado -- e por isso ela e uma dica.
    pub fn definir_marca(&mut self, marca: Option<MarcaDoDiario>) {
        self.marca = marca;
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# curar trunca o rabo: a marca pode apontar para dentro do que sumiu.
import re
m=re.search(r'(    fn curar\(&mut self, volume: u32\) -> Result<u64> \{\n)', s)
assert m, 'curar'
s = s[:m.end(1)] + '        // O reparo pode cortar o rabo do arquivo: uma marca apontando para\n        // dentro do que sumiu passaria a apontar para nada.\n        self.marca = None;\n' + s[m.end(1):]
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
