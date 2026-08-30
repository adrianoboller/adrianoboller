# Make the log header lazy, with a heal-on-open scan
# 29/08 01:54

import pathlib
p = pathlib.Path("crates/phxsql-store/src/log.rs")
s = p.read_text()

# ---------- 1. anexar: o cabecalho para de ir a disco por evento
alvo = '''        self.gravar_cab(Cabecalho {
            volume,
            fim: cab.fim + evento.ocupa(),
            qtd_eventos: cab.qtd_eventos + 1,
        })
    }'''
novo = '''        // O CABECALHO NAO VAI A DISCO AQUI, e essa e a diferenca que faz o
        // diario nao atrasar o `.reg`.
        //
        // O evento ja foi gravado -- ele e o que nao pode faltar. O cabecalho
        // e um CONTADOR: `fim`, onde o proximo entra, e `qtd_eventos`. Grava-lo
        // a cada evento era uma segunda chamada de escrita por linha inserida,
        // medida em 0,41 us, para levar a disco um numero que a leitura sabe
        // recalcular varrendo os proprios eventos.
        //
        // Ele passa a ir no `sincronizar`, junto com o resto. Se o processo
        // cair antes disso, o cabecalho fica ATRASADO em relacao aos eventos
        // que ja estao no arquivo -- e `abrir` cura isso varrendo para a
        // frente a partir do `fim` gravado, validando cada evento pelo CRC que
        // ele ja carrega. A varredura e limitada ao que entrou desde o ultimo
        // `sincronizar`, que e uma janela de centenas de eventos.
        //
        // O que NAO se faz aqui, de proposito: segurar o EVENTO em memoria.
        // Indice perdido se reconstroi do `.reg`; evento perdido nao se
        // reconstroi -- ele e a historia, e e a posicao de que a replicacao
        // depende.
        self.cabs.insert(
            volume,
            Cabecalho {
                volume,
                fim: cab.fim + evento.ocupa(),
                qtd_eventos: cab.qtd_eventos + 1,
            },
        );
        Ok(())
    }

    /// Varre para a frente a partir do `fim` gravado e conserta o cabecalho.
    ///
    /// Existe porque o cabecalho passou a ir a disco so no `sincronizar`: uma
    /// queda antes dele deixa eventos no arquivo que o cabecalho nao conta. Sem
    /// esta cura, a proxima gravacao ESCREVERIA POR CIMA deles.
    ///
    /// Cada evento carrega o proprio CRC, entao a varredura sabe onde parar: no
    /// primeiro que nao confere, ou no fim do arquivo. Regiao zerada nao passa
    /// -- o CRC-32 de 36 bytes zerados nao e zero.
    fn curar(&mut self, volume: u32) -> Result<u64> {
        let mut cab = self.cab(volume)?;
        let tamanho = self.volumes.tamanho(volume)?;
        let mut achados = 0u64;

        while cab.fim + EVENTO_CAB as u64 <= tamanho {
            let mut buf = [0u8; EVENTO_CAB];
            self.volumes.ler(volume, cab.fim, &mut buf)?;
            let evento = match Evento::ler(&buf) {
                Ok(e) => e,
                Err(_) => break,
            };
            if cab.fim + evento.ocupa() > tamanho {
                break;
            }
            if evento.tam_imagem > 0 {
                let mut imagem = vec![0u8; evento.tam_imagem as usize];
                self.volumes
                    .ler(volume, cab.fim + EVENTO_CAB as u64, &mut imagem)?;
                if evento.conferir(&buf, &imagem).is_err() {
                    break;
                }
            }
            cab = Cabecalho {
                volume,
                fim: cab.fim + evento.ocupa(),
                qtd_eventos: cab.qtd_eventos + 1,
            };
            achados += 1;
        }

        if achados > 0 {
            self.gravar_cab(cab)?;
        }
        Ok(achados)
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ---------- 2. sincronizar: o cabecalho vai agora, ANTES do fsync
alvo = '''    pub fn sincronizar(&mut self) -> Result<()> {
        self.volumes.sincronizar()
    }'''
novo = '''    /// Leva os cabecalhos a disco e sincroniza.
    ///
    /// A ordem importa: o cabecalho vai ANTES do `fsync`, senao ele ficaria
    /// para a proxima janela e a cura teria de varrer duas.
    pub fn sincronizar(&mut self) -> Result<()> {
        let pendentes: Vec<Cabecalho> = self.cabs.values().copied().collect();
        for cab in pendentes {
            self.gravar_cab(cab)?;
        }
        self.volumes.sincronizar()
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ---------- 3. abrir: cura o volume corrente
alvo = '''        l.cab(1)?;
        l.cab(volume_atual)?;
        Ok(l)
    }'''
novo = '''        l.cab(1)?;
        l.cab(volume_atual)?;
        // So o volume CORRENTE pode ter ficado atrasado: os anteriores foram
        // fechados quando a paginacao virou, e ali o cabecalho vai a disco na
        // hora.
        l.curar(volume_atual)?;
        Ok(l)
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
