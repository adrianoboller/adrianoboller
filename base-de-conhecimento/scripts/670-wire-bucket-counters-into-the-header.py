# Wire bucket counters into the header
# 28/08 18:46

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()

# abrir: campo novo
velho='''            proxima_sequencia,
            proximo_rownum,
            recuperados: 0,
            fronteiras: Vec::new(),
        };
        r.reler_fronteiras()?;
        Ok(r)'''
novo='''            proxima_sequencia,
            proximo_rownum,
            baldes: Vec::new(),
            recuperados: 0,
            fronteiras: Vec::new(),
        };
        r.reler_fronteiras()?;
        r.reler_baldes()?;
        Ok(r)'''
assert velho in s
s=s.replace(velho,novo,1)

# criar: baldes zerados
velho2='''        if r.esquema.paginacao().modo.periodo().is_some() {
            r.fronteiras.push(Fronteira {
                primeiro_rowid: 1,
                chave_periodo: SEM_PERIODO,
            });
        }
        r.volumes.criar(1)?;'''
novo2='''        if r.esquema.paginacao().modo.periodo().is_some() {
            r.fronteiras.push(Fronteira {
                primeiro_rowid: 1,
                chave_periodo: SEM_PERIODO,
            });
        }
        if r.esquema.paginacao().modo.por_letra() {
            // Os 37 baldes existem desde a criacao, todos vazios. O ARQUIVO de
            // cada um so nasce na primeira linha que cair nele: uma tabela de
            // clientes que nunca teve nome com Q nao precisa de um `_Q.reg`
            // vazio ocupando lugar.
            r.baldes = vec![0; BALDES.len()];
        }
        r.volumes.criar(1)?;'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# reler_baldes, logo depois de reler_fronteiras
velho3='''    /// Remonta a tabela de fronteiras lendo o cabecalho de cada volume.'''
novo3='''    /// Remonta os contadores dos baldes lendo o cabecalho de cada volume.
    ///
    /// Cada volume guarda quantos slots ja usou nos bytes 100..108 do proprio
    /// cabecalho. Fica no volume, e nao num arquivo separado, pela mesma razao
    /// da fronteira do periodo: um arquivo separado seria uma segunda verdade,
    /// e as duas divergem no primeiro caminho que esquecer de atualizar uma.
    fn reler_baldes(&mut self) -> Result<()> {
        if !self.esquema.paginacao().modo.por_letra() {
            self.baldes.clear();
            return Ok(());
        }
        self.baldes = vec![0; BALDES.len()];
        for volume in self.volumes.existentes() {
            let i = volume as usize;
            if i == 0 || i > self.baldes.len() {
                return Err(PhxError::Corrompido(format!(
                    "{} tem o volume {volume}, fora dos {} baldes",
                    self.volumes.nome(),
                    self.baldes.len()
                )));
            }
            let mut cab = [0u8; CAB_LEN];
            self.volumes.ler(volume, 0, &mut cab)?;
            self.baldes[i - 1] = Campos(&cab).u64(100);
        }
        Ok(())
    }

    /// Remonta a tabela de fronteiras lendo o cabecalho de cada volume.'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

# gravar_cabecalho grava o contador do balde
velho4='''        if let Some(f) = self.fronteiras.get(volume as usize - 1) {
            por_u64(&mut buf, 76, f.primeiro_rowid);
            por_u64(&mut buf, 84, f.chave_periodo as u64);
        }'''
novo4='''        if let Some(f) = self.fronteiras.get(volume as usize - 1) {
            por_u64(&mut buf, 76, f.primeiro_rowid);
            por_u64(&mut buf, 84, f.chave_periodo as u64);
        }
        // Na particao alfanumerica, quantos slots este balde ja usou. Por
        // volume, e nao no volume 1: o contador do `_S` tem de viajar junto
        // com o `_S`.
        if let Some(usados) = self.baldes.get(volume as usize - 1) {
            por_u64(&mut buf, 100, *usados);
        }'''
assert velho4 in s
s=s.replace(velho4,novo4,1)

s=s.replace("use phxsql_core::paginacao::Paginacao;","use phxsql_core::paginacao::{Paginacao, BALDES};",1)
io.open(p,'w',encoding='utf-8').write(s)
