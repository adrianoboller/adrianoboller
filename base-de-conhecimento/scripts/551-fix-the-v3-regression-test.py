# Fix the v3 regression test
# 28/08 17:25

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    #[test]
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
    }'''
novo='''    #[test]
    fn esquema_sem_a_coluna_de_sistema_volta_do_disco_sem_ela() {
        // Uma tabela gravada antes da v4 tem SO as colunas do usuario. O que
        // este teste prova e que a volta do disco nao inventa a setima.
        let antiga = Schema::do_disco("cadastroClientes", colunas_clientes(), vec![]).unwrap();
        assert!(antiga.coluna_softdeleted().is_none());

        let lido = Schema::desserializar(&antiga.serializar()).unwrap();
        assert!(
            lido.coluna_softdeleted().is_none(),
            "a leitura acrescentou a coluna de sistema numa tabela que nao a tem"
        );
        assert_eq!(lido.colunas().len(), 6);
        assert_eq!(lido.payload_len(), antiga.payload_len());
        assert_eq!(lido, antiga);
    }

    /// A v3 nao tem o byte do motivo obrigatorio no fim. Ler uma nao pode
    /// estourar nem trazer lixo -- tem de dar `false`.
    #[test]
    fn v3_no_disco_para_antes_do_byte_novo() {
        let mut bytes = esquema_clientes().serializar();
        bytes[4..6].copy_from_slice(&3u16.to_le_bytes());
        bytes.pop();
        let lido = Schema::desserializar(&bytes).unwrap();
        assert!(!lido.motivo_obrigatorio());
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
