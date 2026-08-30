# Add challenge-response authentication
# 27/08 19:23

p='crates/phxsql-core/src/senha.rs'
s=open(p).read()
s=s.replace('''/// Sal novo de 16 bytes.''','''/// Bytes aleatorios, para sal e para nonce.
///
/// Tenta `/dev/urandom`; onde ele nao existe, cai na mistura descrita em
/// [`sal_novo`].
pub fn bytes_aleatorios(quantos: usize) -> Vec<u8> {
    let mut saida = Vec::with_capacity(quantos);
    while saida.len() < quantos {
        match sal_do_urandom() {
            Some(b) => saida.extend_from_slice(&b),
            None => saida.extend_from_slice(&sal_por_mistura()),
        }
    }
    saida.truncate(quantos);
    saida
}

/// O material derivado que esta guardado dentro de um hash de senha.
///
/// E o que o desafio-resposta usa como chave: o servidor ja tem, e o cliente
/// chega nele a partir da senha, do sal e das iteracoes.
pub fn derivado_do_hash(guardado: &str) -> Result<Vec<u8>> {
    destrinchar(guardado).map(|(_, _, hash)| hash)
}

/// Sal e iteracoes de um hash guardado, para mandar ao cliente no desafio.
pub fn sal_e_iteracoes(guardado: &str) -> Result<(Vec<u8>, u32)> {
    destrinchar(guardado).map(|(it, sal, _)| (sal, it))
}

/// Sal novo de 16 bytes.''')
s=s.replace('''    #[test]
    fn sal_nunca_repete_em_sequencia() {''','''    #[test]
    fn bytes_aleatorios_no_tamanho_pedido() {
        for n in [0usize, 1, 15, 16, 17, 64, 100] {
            assert_eq!(bytes_aleatorios(n).len(), n);
        }
        assert_ne!(bytes_aleatorios(32), bytes_aleatorios(32));
    }

    #[test]
    fn extrai_o_derivado_e_o_sal_do_hash() {
        let h = cifrar_com("segredo", RAPIDO);
        let dk = derivado_do_hash(&h).unwrap();
        assert_eq!(dk.len(), 32);
        let (sal, it) = sal_e_iteracoes(&h).unwrap();
        assert_eq!(sal.len(), 16);
        assert_eq!(it, RAPIDO);
        // Refazer a conta a partir da senha da o mesmo derivado.
        let mut refeito = vec![0u8; 32];
        crate::hash::pbkdf2_sha256(b"segredo", &sal, it, &mut refeito);
        assert_eq!(refeito, dk);
        assert!(derivado_do_hash("nao-e-hash").is_err());
    }

    #[test]
    fn sal_nunca_repete_em_sequencia() {''')
open(p,'w').write(s)
