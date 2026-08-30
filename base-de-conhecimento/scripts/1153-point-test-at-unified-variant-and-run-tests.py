# Point test at unified variant and run tests
# 29/08 17:26

import pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs"); t = p.read_text()
t = t.replace('''    /// Os dois erros de papel sao recusa DEFINITIVA deste servidor: repetir o
    /// pedido aqui nao muda nada -- o conserto e falar com o primario.
    #[test]
    fn recusa_por_papel_nao_pede_nova_tentativa() {
        assert!(!PhxError::EscritaNaReplica(String::new()).adianta_repetir());''',
'''    /// Os dois erros de papel sao recusa DEFINITIVA deste servidor: repetir o
    /// pedido aqui nao muda nada -- o conserto e falar com o primario.
    ///
    /// `Redireciona` cobre os dois casos que nasceram separados: a escrita
    /// numa read replica e a escrita numa replica de cluster. Para quem chama,
    /// e o mesmo evento -- "va para o outro servidor" -- e um evento so tem
    /// um codigo.
    #[test]
    fn recusa_por_papel_nao_pede_nova_tentativa() {
        assert!(!PhxError::Redireciona(String::new()).adianta_repetir());''')
t = t.replace('        assert_eq!(PhxError::EscritaNaReplica(String::new()).classe(), "acesso");',
              '        assert_eq!(PhxError::Redireciona(String::new()).classe(), "acesso");')
p.write_text(t); print("teste apontado para Redireciona")
