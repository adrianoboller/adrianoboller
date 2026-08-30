# Add and run a test for the moved ceiling
# 30/08 06:39

p='crates/phxsql-core/src/fio.rs'
s=open(p,encoding='utf-8').read()
ancora='''    fn o_texto_claro_nao_aparece_no_fio() {'''
assert s.count(ancora)==1
i=s.rindex('\n', 0, s.index(ancora))
# Sobe ate o comeco do comentario/atributo do teste, para nao cair no meio.
j=s.rindex('    #[test]', 0, s.index(ancora))

teste = '''    /// O teto de um registro vale nos DOIS canais, e essa e a razao de ele
    /// morar aqui.
    ///
    /// A protecao nasceu na replica, num `read_line` com `take`. A cifra
    /// depois trocou aquele `read_line` por este canal -- e juntar as duas
    /// frentes sem olhar teria devolvido a leitura ilimitada, com quem escolhe
    /// quanta memoria este lado reserva sendo o outro lado do fio. Repondo o
    /// defeito (tirar o `take` do `ler_ate`) este teste estoura a memoria em
    /// vez de recusar, que e o sintoma que ele existe para impedir.
    #[test]
    fn o_teto_do_registro_recusa_a_linha_grande_nos_dois_canais() {
        let teto = 64u64;
        let gorda = format!("{}\\n", "x".repeat(teto as usize + 10));

        let mut claro = Canal::Claro;
        let erro = claro
            .ler_ate(&mut gorda.as_bytes(), teto)
            .expect_err("linha acima do teto tem de ser recusada no canal claro");
        assert!(matches!(erro, PhxError::LimiteExcedido(_)), "{erro:?}");

        // E o que CABE continua passando -- teto que recusa tudo nao e teto,
        // e parede.
        let magra = "cabe\\n";
        assert_eq!(
            claro.ler_ate(&mut magra.as_bytes(), teto).unwrap(),
            Recebido::Linha(magra.to_string())
        );
    }

'''
s = s[:j] + teste + s[j:]
open(p,'w',encoding='utf-8').write(s)
print("teste do teto acrescentado")
