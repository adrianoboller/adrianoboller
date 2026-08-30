# Test the new builder method
# 28/08 10:51

import pathlib, re
p = pathlib.Path('crates/phxsql-core/src/paginacao.rs')
s = p.read_text()
# um teste para o novo metodo, junto dos que ja existem
m = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
assert s.count(m) == 1
teste = '''
    #[test]
    fn largura_do_sufixo_entra_antes_do_teto() {
        // O teto de 9999 so e valido depois que o quarto digito existe. Na
        // ordem contraria a validacao recusa -- e foi assim que a tela de
        // nova tabela quebrou na primeira tentativa.
        let p = Paginacao::nova(1000, 1)
            .unwrap()
            .com_digitos(4)
            .unwrap()
            .com_max_arquivos(9_999)
            .unwrap();
        assert_eq!(p.max_arquivos, 9_999);
        assert_eq!(p.digitos, 4);
        assert_eq!(p.capacidade(), 9_999_000);

        // E continua recusando o que nao cabe.
        assert!(Paginacao::nova(1000, 1)
            .unwrap()
            .com_max_arquivos(1_000)
            .is_err());
        assert!(Paginacao::nova(1000, 1)
            .unwrap()
            .com_max_arquivos(0)
            .is_err());
    }
'''
s = s.replace(m, m + teste, 1)
p.write_text(s)
print('ok')
