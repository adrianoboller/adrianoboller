# Add the boundary table to RegFile
# 28/08 11:18

import pathlib
p = pathlib.Path('crates/phxsql-store/src/reg.rs')
s = p.read_text()

# ------------------------------------------------------- a struct da fronteira
v = '''    /// Leituras salvas pelo espelho nesta sessao.
    recuperados: u64,
}'''
n = '''    /// Leituras salvas pelo espelho nesta sessao.
    recuperados: u64,
    /// Onde cada volume comeca, quando a particao e por periodo.
    ///
    /// Indice do vetor = volume - 1. Vazio quando a particao e por quantidade,
    /// porque ali o volume sai de uma divisao e nao ha o que guardar.
    fronteiras: Vec<Fronteira>,
}

/// O comeco de um volume: o primeiro rowid que ele recebeu e o periodo em que
/// foi aberto.
///
/// As faixas sao contiguas e crescentes -- o volume N+1 comeca no rowid
/// seguinte ao ultimo do N --, porque a ordem de digitacao manda: linha nova
/// vai sempre para o volume corrente, mesmo que a data dela seja de um periodo
/// ja fechado. Por isso achar o volume de um rowid e uma busca binaria, e nao
/// um indice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fronteira {
    pub primeiro_rowid: RowId,
    pub chave_periodo: i64,
}'''
assert s.count(v) == 1
s = s.replace(v, n)

# --------------------------------------------------------- criar e abrir
v = '''            proxima_sequencia: 0,
            recuperados: 0,
        };
        r.volumes.criar(1)?;'''
n = '''            proxima_sequencia: 0,
            recuperados: 0,
            fronteiras: Vec::new(),
        };
        r.volumes.criar(1)?;'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        Ok(RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, esquema.paginacao()),
            esquema,
            slot_size,
            data_offset,
            slot_count,
            live_count,
            criado_em,
            proxima_sequencia,
            recuperados: 0,
        })
    }'''
n = '''        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, esquema.paginacao()),
            esquema,
            slot_size,
            data_offset,
            slot_count,
            live_count,
            criado_em,
            proxima_sequencia,
            recuperados: 0,
            fronteiras: Vec::new(),
        };
        r.reler_fronteiras()?;
        Ok(r)
    }

    /// Remonta a tabela de fronteiras lendo o cabecalho de cada volume.
    ///
    /// So faz sentido na particao por periodo. Le poucos bytes por volume, uma
    /// vez, na abertura -- e volume e coisa que se conta em dezenas, nao em
    /// milhares, porque cada um guarda `registros_por_arquivo` linhas.
    fn reler_fronteiras(&mut self) -> Result<()> {
        self.fronteiras.clear();
        if self.esquema.paginacao().modo.periodo().is_none() {
            return Ok(());
        }
        for volume in self.volumes.existentes() {
            let mut cab = [0u8; CAB_LEN];
            self.volumes.ler(volume, 0, &mut cab)?;
            let c = Campos(&cab);
            self.fronteiras.push(Fronteira {
                primeiro_rowid: c.u64(76),
                chave_periodo: c.u64(84) as i64,
            });
        }
        // Um volume que existe mas nunca foi escrito na v3 vem com zero. Zero
        // nao e rowid: seria endereco 1 para tudo. Melhor recusar alto do que
        // devolver a linha errada em silencio.
        if let Some(i) = self.fronteiras.iter().position(|f| f.primeiro_rowid == 0) {
            return Err(PhxError::Corrompido(format!(
                "volume {} de {} nao tem fronteira gravada; a tabela foi criada \\
                 antes da particao por periodo e precisa ser recriada",
                i + 1,
                self.volumes.nome()
            )));
        }
        Ok(())
    }

    /// As fronteiras de volume, para quem quiser mostra-las.
    pub fn fronteiras(&self) -> &[Fronteira] {
        &self.fronteiras
    }'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
