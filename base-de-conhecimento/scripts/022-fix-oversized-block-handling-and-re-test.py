# Fix oversized block handling and re-test
# 27/08 18:28

p='crates/phxsql-core/src/paginacao.rs'
s=open(p).read()
s=s.replace('''    /// Volume que deve receber o proximo bloco externo, dado quanto ja foi
    /// escrito no volume atual e o tamanho do bloco novo.
    ///
    /// Um bloco nunca e partido entre volumes: se nao couber no volume atual,
    /// vai inteiro para o proximo.
    pub fn volume_externo(&self, volume_atual: u32, usado: u64, novo: u64) -> (u32, bool) {
        if !self.ligada() || usado + novo <= self.bytes_por_arquivo {
            (volume_atual, false)
        } else {
            (volume_atual + 1, true)
        }
    }''','''    /// Volume que deve receber o proximo bloco externo, dado quanto ja foi
    /// escrito no volume atual e o tamanho do bloco novo.
    ///
    /// Um bloco nunca e partido entre volumes: se nao couber no volume atual,
    /// vai inteiro para o proximo.
    ///
    /// `volume_vazio` diz se o volume atual ainda nao tem nenhum bloco. Um
    /// bloco maior que `bytes_por_arquivo` fica sozinho no seu volume, em vez
    /// de ser recusado -- caso contrario uma foto de 2 MB seria impossivel de
    /// gravar num arquivo de 1 MB, e trocar de volume nao resolveria nada.
    pub fn volume_externo(
        &self,
        volume_atual: u32,
        usado: u64,
        novo: u64,
        volume_vazio: bool,
    ) -> (u32, bool) {
        if !self.ligada() || volume_vazio || usado + novo <= self.bytes_por_arquivo {
            (volume_atual, false)
        } else {
            (volume_atual + 1, true)
        }
    }''')
s=s.replace('''        // Cabe: fica no volume atual.
        assert_eq!(p.volume_externo(1, 900, 100), (1, false));
        // Nao cabe por um byte: vai inteiro para o proximo.
        assert_eq!(p.volume_externo(1, 900, 101), (2, true));
        // Sem paginacao nunca troca de volume.
        assert_eq!(
            Paginacao::DESLIGADA.volume_externo(1, u64::MAX / 2, 1_000),
            (1, false)
        );
    }''','''        // Cabe: fica no volume atual.
        assert_eq!(p.volume_externo(1, 900, 100, false), (1, false));
        // Nao cabe por um byte: vai inteiro para o proximo.
        assert_eq!(p.volume_externo(1, 900, 101, false), (2, true));
        // Sem paginacao nunca troca de volume.
        assert_eq!(
            Paginacao::DESLIGADA.volume_externo(1, u64::MAX / 2, 1_000, false),
            (1, false)
        );
    }

    #[test]
    fn bloco_maior_que_o_volume_fica_sozinho_em_vez_de_ser_recusado() {
        let p = Paginacao::nova(1_000, 999)
            .unwrap()
            .com_bytes_por_arquivo(1_000)
            .unwrap();
        // Volume ainda vazio e o bloco e maior que o volume inteiro:
        // grava assim mesmo, senao nunca caberia em lugar nenhum.
        assert_eq!(p.volume_externo(5, 64, 50_000, true), (5, false));
        // Com o volume ja ocupado, ele rola para o proximo.
        assert_eq!(p.volume_externo(5, 500, 50_000, false), (6, true));
    }''')
open(p,'w').write(s)

for p, cab in [('crates/phxsql-store/src/blob.rs','CAB_LEN'), ('crates/phxsql-store/src/log.rs','CAB_LEN')]:
    s=open(p).read()
    s=s.replace('''        let (volume, virou) = paginacao.volume_externo(self.volume_atual, atual.fim, precisa);''',
                '''        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, precisa, vazio);''')
    s=s.replace('''        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, EVENTO_LEN as u64);''',
                '''        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, EVENTO_LEN as u64, vazio);''')
    open(p,'w').write(s)
print("volume_externo corrigido")
