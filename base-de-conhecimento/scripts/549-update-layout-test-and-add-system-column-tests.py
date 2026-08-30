# Update layout test and add system-column tests
# 28/08 17:24

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let s = esquema_clientes();
        // 6 colunas -> 1 byte de bitmap.
        assert_eq!(s.bitmap_len(), 1);
        assert_eq!(s.offset_coluna(0).unwrap(), 1);
        assert_eq!(s.offset_coluna(1).unwrap(), 9);
        assert_eq!(s.offset_coluna(2).unwrap(), 69);
        // 1 + 8 + 60 + 14 + 16 + 16 + 16
        assert_eq!(s.payload_len(), 131);
    }'''
novo='''        let s = esquema_clientes();
        // 6 colunas declaradas + a de sistema -> 7, ainda 1 byte de bitmap.
        assert_eq!(s.colunas().len(), 7);
        assert_eq!(s.bitmap_len(), 1);
        assert_eq!(s.offset_coluna(0).unwrap(), 1);
        assert_eq!(s.offset_coluna(1).unwrap(), 9);
        assert_eq!(s.offset_coluna(2).unwrap(), 69);
        // 1 + 8 + 60 + 14 + 16 + 16 + 16 + 1 do softdeleted
        assert_eq!(s.payload_len(), 132);
    }

    /// A coluna de sistema entra por ultimo, e so por ultimo: as colunas do
    /// usuario nao podem mudar de offset por causa dela.
    #[test]
    fn softdeleted_entra_no_fim_e_nao_desloca_ninguem() {
        let com = esquema_clientes();
        let sem = Schema::do_disco(
            "clientes",
            colunas_clientes(),
            vec![IndexDef::new("por_nome", vec![IndexColumn::asc(1)])],
        )
        .unwrap();

        let i = com.coluna_softdeleted().unwrap();
        assert_eq!(i, com.colunas().len() - 1);
        assert_eq!(com.colunas()[i].ty, ColumnType::Bool);
        assert!(!com.colunas()[i].nullable);
        assert!(sem.coluna_softdeleted().is_none());

        for j in 0..sem.colunas().len() {
            assert_eq!(
                com.offset_coluna(j).unwrap(),
                sem.offset_coluna(j).unwrap(),
                "a coluna {j} mudou de lugar"
            );
        }
    }

    /// Este e o teste que protege a tabela ja gravada: ler um esquema v3 do
    /// disco NAO pode inventar uma coluna. Se inventasse, cada linha passaria
    /// a ser lida com os offsets deslocados -- e o CRC do slot continuaria
    /// batendo, porque os bytes seriam os mesmos.
    #[test]
    fn esquema_v3_do_disco_nao_ganha_coluna() {
        let v4 = esquema_clientes();
        let bytes = v4.serializar();
        // Rebaixa a versao no cabecalho e corta o byte que a v4 acrescentou.
        let mut v3 = bytes.clone();
        v3[4..6].copy_from_slice(&3u16.to_le_bytes());
        v3.pop();
        // ... e tira a coluna de sistema da lista, como uma v3 de verdade.
        let lido = Schema::desserializar(&v3).unwrap();
        assert_eq!(lido.colunas().len(), v4.colunas().len());
        assert_eq!(lido.payload_len(), v4.payload_len());
        assert!(!lido.motivo_obrigatorio());
    }

    #[test]
    fn softdeleted_com_outro_tipo_e_recusada() {
        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_SOFTDELETED, ColumnType::Str(4)).obrigatoria());
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("Bool"), "{e}");

        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_SOFTDELETED, ColumnType::Bool));
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("nulo"), "{e}");
    }

    #[test]
    fn motivo_obrigatorio_atravessa_o_disco() {
        let s = esquema_clientes().com_motivo_obrigatorio(true);
        let volta = Schema::desserializar(&s.serializar()).unwrap();
        assert!(volta.motivo_obrigatorio());
        assert_eq!(s, volta);
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
