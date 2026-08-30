# Write the carga module
# 29/08 02:53

import pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
s = p.read_text()
s = s.replace('''    /// Repetir so adianta no que pode ter mudado sozinho.
    #[test]
    fn so_o_erro_de_es_pede_nova_tentativa() {''','''    /// Repetir so adianta no que pode ter mudado sozinho. Sao dois, e o nome
    /// deste teste ja disse "so o de E/S" -- ate a tabela em carga existir.
    #[test]
    fn so_o_que_e_passageiro_pede_nova_tentativa() {''',1)
p.write_text(s)
