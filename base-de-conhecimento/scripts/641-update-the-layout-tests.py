# Update the layout tests
# 28/08 18:27

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let s = esquema_clientes();
        // 6 colunas declaradas + a de sistema -> 7, ainda 1 byte de bitmap.
        assert_eq!(s.colunas().len(), 7);
        assert_eq!(s.bitmap_len(), 1);
        assert_eq!(s.offset_coluna(0).unwrap(), 1);
        assert_eq!(s.offset_coluna(1).unwrap(), 9);
        assert_eq!(s.offset_coluna(2).unwrap(), 69);
        // 1 + 8 + 60 + 14 + 16 + 16 + 16 + 1 do softdeleted
        assert_eq!(s.payload_len(), 132);
    }'''
novo='''        let s = esquema_clientes();
        // 6 declaradas + softdeleted + rownum = 8, e o bitmap ainda cabe em 1.
        assert_eq!(s.colunas().len(), 8);
        assert_eq!(s.bitmap_len(), 1);
        assert_eq!(s.offset_coluna(0).unwrap(), 1);
        assert_eq!(s.offset_coluna(1).unwrap(), 9);
        assert_eq!(s.offset_coluna(2).unwrap(), 69);
        // 1 + 8 + 60 + 14 + 16 + 16 + 16 + 1 do softdeleted + 8 do rownum
        assert_eq!(s.payload_len(), 140);
    }

    /// A ordem das duas colunas de sistema e parte do formato: `rownum` entra
    /// DEPOIS de `softdeleted`, e nao antes. Trocar a ordem deslocaria o
    /// offset da softdeleted em toda tabela ja gravada na v4.
    #[test]
    fn as_colunas_de_sistema_saem_nesta_ordem() {
        let s = esquema_clientes();
        let n = s.colunas().len();
        assert_eq!(s.coluna_softdeleted(), Some(n - 2));
        assert_eq!(s.coluna_rownum(), Some(n - 1));
        assert_eq!(s.colunas()[n - 1].ty, ColumnType::UInt8);
        assert!(!s.colunas()[n - 1].nullable);
    }

    #[test]
    fn rownum_com_outro_tipo_e_recusada() {
        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_ROWNUM, ColumnType::Int4).obrigatoria());
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("UInt8"), "{e}");
    }'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''        let i = com.coluna_softdeleted().unwrap();
        assert_eq!(i, com.colunas().len() - 1);'''
novo2='''        let i = com.coluna_softdeleted().unwrap();
        assert_eq!(i, com.colunas().len() - 2, "a softdeleted saiu do lugar");'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
