# Add the regression test
# 28/08 14:29

p='crates/phxsql-server/src/sistema.rs'
s=open(p).read()
a='''        assert!((e.usado_percentual() - 25.0).abs() < 0.001);
        assert!((e.livre_percentual() - 75.0).abs() < 0.001);
    }
'''
b='''        assert!((e.usado_percentual() - 25.0).abs() < 0.001);
        assert!((e.livre_percentual() - 75.0).abs() < 0.001);
    }

    /// A conta e sobre `usado + livre`, e nao sobre o tamanho do disco.
    ///
    /// Os numeros sao os desta maquina, colados do `df -k`: 264 GB de disco,
    /// 21 GB usados, 18 GB alcancaveis. O `df` diz 55% -- e era 8% que este
    /// modulo mostrava antes, o que faria um alerta de "menos de 10% livre"
    /// ficar calado com o disco pela metade.
    #[test]
    fn a_reserva_do_sistema_de_arquivos_nao_conta_como_livre() {
        let e = EspacoEmDisco {
            caminho: "/".into(),
            dispositivo: "/dev/vda".into(),
            montagem: "/".into(),
            total_kb: 264_212_084,
            usado_kb: 20_986_728,
            livre_kb: 17_861_796,
        };
        // O `df` arredonda para cima e mostra 55; a conta exata da 54,02.
        assert!(
            (e.usado_percentual() - 54.02).abs() < 0.01,
            "{}",
            e.usado_percentual()
        );
        assert_eq!(e.utilizavel_kb(), 38_848_524);
        assert_eq!(e.reservado_kb(), 225_363_560);
    }
'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
