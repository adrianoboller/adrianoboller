# Fix and rerun test
# 28/08 23:57

import pathlib
p = pathlib.Path("crates/phxsql-store/tests/conflito.rs")
s = p.read_text()
s = s.replace('''/// Slot que nunca foi usado nao tem versao. Sem isto, conferir a versao de um
/// rowid inventado devolveria zero e o zero passaria por "sem conferencia".
#[test]
fn slot_nunca_usado_nao_tem_versao() {
    let dir = DirTemp::novo("conflito-livre");
    let (mut t, _) = com_uma_linha(&dir);
    // O `.reg` nasce com uma pagina inteira de slots; o 2 existe e esta livre.
    assert_eq!(t.versao(2).unwrap(), None);
}''',
'''/// Rowid fora da tabela ERRA -- nao devolve "sem versao". A diferenca
/// importa: `None` quer dizer "esta linha nao existe mais", e um rowid
/// inventado nao pode se passar por linha excluida.
#[test]
fn rowid_fora_da_faixa_erra() {
    let dir = DirTemp::novo("conflito-faixa");
    let (mut t, r) = com_uma_linha(&dir);
    assert!(t.versao(r + 10_000).is_err());
}''', 1)
p.write_text(s)
