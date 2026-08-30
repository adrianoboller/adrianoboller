# Rewrite the test to assert on bytes consumed
# 30/08 06:39

p='crates/phxsql-core/src/fio.rs'
s=open(p,encoding='utf-8').read()
i=s.index('    /// O teto de um registro vale nos DOIS canais')
j=s.index('\n    }\n', s.index('fn o_teto_do_registro_recusa')) + len('\n    }\n')
novo = '''    /// O teto de um registro vale nos DOIS canais, e essa e a razao de ele
    /// morar aqui.
    ///
    /// A protecao nasceu na replica, num `read_line` com `take`. A cifra
    /// depois trocou aquele `read_line` por este canal -- e juntar as duas
    /// frentes sem olhar teria devolvido a leitura ilimitada, com quem escolhe
    /// quanta memoria este lado reserva sendo o outro lado do fio.
    ///
    /// A assercao e sobre **quanto foi lido**, e nao sobre o veredito, e a
    /// primeira versao deste teste ensinou por que: conferir so o erro passava
    /// com o defeito reposto, porque a conferencia `lidos > teto` vem DEPOIS
    /// da leitura e acusa mesmo sem o `take` -- so que ai a memoria ja foi
    /// gasta, que e justamente o dano. Teste que passa por engano e pior que
    /// teste que falta.
    #[test]
    fn o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois() {
        let teto = 64u64;
        let gorda = format!("{}\\n", "x".repeat(10_000));
        let bytes = gorda.as_bytes();

        let mut claro = Canal::Claro;
        let mut fonte: &[u8] = bytes;
        let erro = claro
            .ler_ate(&mut fonte, teto)
            .expect_err("linha acima do teto tem de ser recusada");
        assert!(matches!(erro, PhxError::LimiteExcedido(_)), "{erro:?}");

        // O que importa: sobrou quase tudo por ler. Sem o `take`, `fonte`
        // estaria vazia -- os 10 KiB teriam entrado na memoria antes da recusa.
        let consumido = bytes.len() - fonte.len();
        assert!(
            consumido as u64 <= teto + 1,
            "leu {consumido} bytes com teto de {teto}: a recusa veio depois de \\
             gastar a memoria, que e o defeito que este teto existe para impedir"
        );

        // E o que CABE continua passando -- teto que recusa tudo nao e teto,
        // e parede.
        let magra = "cabe\\n";
        assert_eq!(
            claro.ler_ate(&mut magra.as_bytes(), teto).unwrap(),
            Recebido::Linha(magra.to_string())
        );
    }
'''
open(p,'w',encoding='utf-8').write(s[:i]+novo+s[j:])
print("teste refeito: mede bytes consumidos, nao o veredito")
