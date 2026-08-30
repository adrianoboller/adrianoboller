# Test the mirror and repair
# 27/08 21:45

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
teste = '''
    #[test]
    fn o_espelho_salva_um_registro_estragado() {
        let d = temp("espelho");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        assert!(r.tem_espelho());

        let mut ids = Vec::new();
        for i in 0..20u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            p[1] = i.wrapping_mul(3);
            ids.push(r.inserir(&p).unwrap());
        }
        r.sincronizar().unwrap();
        // O .bkp existe e tem o mesmo tamanho do .reg.
        let reg = d.join("cadastroClientes.reg");
        let bkp = d.join("cadastroClientes.bkp");
        assert!(bkp.is_file(), "o espelho nao foi criado");
        assert_eq!(
            std::fs::metadata(&reg).unwrap().len(),
            std::fs::metadata(&bkp).unwrap().len()
        );

        let antes = r.ler(7).unwrap().unwrap();
        drop(r);

        // Estraga um byte do payload do registro 7 SO no principal.
        let mut r2 = RegFile::abrir(&d, "cadastroClientes").unwrap();
        let (volume, offset) = r2.localizar(7);
        let mut slot = vec![0u8; r2.slot_size];
        r2.volumes.ler(volume, offset, &mut slot).unwrap();
        slot[SLOT_CAB] ^= 0xff;
        r2.volumes.escrever(volume, offset, &slot).unwrap();
        r2.sincronizar().unwrap();
        drop(r2);

        // Sem espelho: a leitura acusa corrupcao, como tem de acusar.
        let mut sem = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert!(sem.ler(7).is_err(), "sem espelho, tem de recusar");
        drop(sem);

        // Com espelho: a leitura volta certa, e o contador registra.
        let mut com = RegFile::abrir(&d, "cadastroClientes").unwrap();
        com.espelhar().unwrap();
        assert_eq!(com.recuperados(), 0);
        assert_eq!(com.ler(7).unwrap().unwrap(), antes, "o espelho nao salvou");
        assert_eq!(com.recuperados(), 1, "a recuperacao tem de aparecer");
        // Os vizinhos continuam saindo do principal, sem contar recuperacao.
        assert!(com.ler(6).unwrap().is_some());
        assert_eq!(com.recuperados(), 1);
    }

    #[test]
    fn reparar_conserta_os_dois_lados() {
        let d = temp("reparar");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        for i in 0..12u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            r.inserir(&p).unwrap();
        }
        r.sincronizar().unwrap();
        drop(r);

        // Estraga o registro 3 no principal e o 9 no espelho.
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        r.espelhar().unwrap();
        for (rowid, no_espelho) in [(3u64, false), (9u64, true)] {
            let (v, off) = r.localizar(rowid);
            let mut slot = vec![0u8; r.slot_size];
            if no_espelho {
                r.volumes.ler_do_espelho(v, off, &mut slot).unwrap();
                slot[SLOT_CAB] ^= 0x5a;
                r.volumes.escrever_no_espelho(v, off, &slot).unwrap();
            } else {
                r.volumes.ler(v, off, &mut slot).unwrap();
                slot[SLOT_CAB] ^= 0x5a;
                // escrever() duplica no espelho; aqui queremos so o principal.
                let f = r.volumes.arquivo(v, true).unwrap();
                use std::io::{Seek, SeekFrom, Write};
                f.seek(SeekFrom::Start(off)).unwrap();
                f.write_all(&slot).unwrap();
            }
        }
        r.sincronizar().unwrap();

        let (conferidos, reparados, perdidos) = r.reparar().unwrap();
        assert_eq!(conferidos, 12);
        assert_eq!(reparados, 2, "um de cada lado");
        assert_eq!(perdidos, 0);

        // Depois do reparo, tudo le sem precisar da segunda chance.
        let mut depois = RegFile::abrir(&d, "cadastroClientes").unwrap();
        depois.espelhar().unwrap();
        for rowid in 1..=12 {
            assert!(depois.ler(rowid).unwrap().is_some(), "rowid {rowid}");
        }
        assert_eq!(depois.recuperados(), 0, "nada precisou do espelho");
    }

    #[test]
    fn os_dois_lados_perdidos_nao_viram_dado_inventado() {
        let d = temp("perdidos");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        for i in 0..5u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            r.inserir(&p).unwrap();
        }
        r.sincronizar().unwrap();
        // Estraga o 2 nos DOIS lados: nao ha o que salvar.
        let (v, off) = r.localizar(2);
        let mut slot = vec![0u8; r.slot_size];
        r.volumes.ler(v, off, &mut slot).unwrap();
        slot[SLOT_CAB] ^= 0xaa;
        r.volumes.escrever(v, off, &slot).unwrap(); // vai para os dois
        r.sincronizar().unwrap();

        assert!(r.ler(2).is_err(), "sem copia boa, tem de acusar e nao inventar");
        let (_, reparados, perdidos) = r.reparar().unwrap();
        assert_eq!(reparados, 0);
        assert_eq!(perdidos, 1);
    }

    #[test]
    fn sem_espelho_reparar_recusa_em_vez_de_fingir() {
        let d = temp("semespelho");
        let mut r = RegFile::criar(&d, "cadastroClientes", esquema()).unwrap();
        assert!(!r.tem_espelho());
        assert!(r.reparar().is_err());
    }
'''
i=s.rindex('\n}\n')
open(p,'w').write(s[:i]+teste+s[i:])
