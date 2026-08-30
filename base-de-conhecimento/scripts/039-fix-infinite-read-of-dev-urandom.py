# Fix infinite read of /dev/urandom
# 27/08 19:01

p='crates/phxsql-core/src/senha.rs'
s=open(p).read()
s=s.replace('''fn sal_novo() -> [u8; SAL_LEN] {
    if let Ok(bytes) = std::fs::read("/dev/urandom") {
        // A leitura devolve o quanto o sistema entregou; 16 bytes bastam.
        if bytes.len() >= SAL_LEN {
            let mut sal = [0u8; SAL_LEN];
            sal.copy_from_slice(&bytes[..SAL_LEN]);
            return sal;
        }
    }
    sal_por_mistura()
}''','''fn sal_novo() -> [u8; SAL_LEN] {
    // ATENCAO: /dev/urandom e um dispositivo INFINITO. Ler o "arquivo inteiro"
    // nunca termina -- tem de ser exatamente SAL_LEN bytes.
    if let Some(sal) = sal_do_urandom() {
        return sal;
    }
    sal_por_mistura()
}

fn sal_do_urandom() -> Option<[u8; SAL_LEN]> {
    use std::io::Read;
    let mut arquivo = std::fs::File::open("/dev/urandom").ok()?;
    let mut sal = [0u8; SAL_LEN];
    arquivo.read_exact(&mut sal).ok()?;
    Some(sal)
}''')
s=s.replace('''    #[test]
    fn sal_nunca_repete_em_sequencia() {''','''    #[test]
    fn urandom_le_so_o_que_precisa_e_nao_trava() {
        // /dev/urandom e infinito: se a leitura nao for limitada, isto nunca
        // retorna. O teste existe para travar essa regressao.
        if let Some(a) = sal_do_urandom() {
            let b = sal_do_urandom().expect("segunda leitura tambem deve funcionar");
            assert_ne!(a, b, "duas leituras do urandom nao podem coincidir");
        }
    }

    #[test]
    fn sal_nunca_repete_em_sequencia() {''')
open(p,'w').write(s)
