# Add the heal tests
# 29/08 01:55

import pathlib
p = pathlib.Path("crates/phxsql-store/src/log.rs")
s = p.read_text()
alvo = '''    #[test]
    fn registra_as_tres_operacoes_em_ordem() {'''
novo = '''    /* --------------------------------------- o cabecalho preguicoso e a cura

       O cabecalho do `.log` deixou de ir a disco a cada evento, para o diario
       nao atrasar o `.reg`. O EVENTO continua indo na hora -- e a diferenca
       entre as duas coisas e o que estes testes protegem.

       Uma queda antes do `sincronizar` deixa o cabecalho atrasado. Sem a cura,
       a proxima gravacao escreveria POR CIMA dos eventos que ja estavam la:
       nao seria evento invisivel, seria evento destruido. */

    /// O caso da queda: grava, some sem sincronizar, reabre. Nada pode faltar.
    #[test]
    fn queda_sem_sincronizar_nao_perde_evento() {
        let d = dir_temp("cura-queda");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=500u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
            // De proposito SEM `sincronizar`: e o que uma queda do processo faz.
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 500, "a cura perdeu evento");
        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.len(), 500);
        assert_eq!(eventos[0].rowid, 1);
        assert_eq!(eventos[499].rowid, 500);
        assert_eq!(l.verificar().unwrap(), 500);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A cura tambem vale com imagem da linha, que e o modo da replicacao --
    /// ali o evento tem tamanho variavel, e a varredura precisa andar por
    /// `ocupa()` e nao por um passo fixo.
    #[test]
    fn a_cura_anda_por_evento_de_tamanho_variavel() {
        let d = dir_temp("cura-imagem");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=200u64 {
                // Imagens de tamanhos diferentes: passo fixo erraria na segunda.
                let imagem = vec![(i % 251) as u8; (i % 97) as usize];
                l.registrar_com_imagem(Operacao::Inclusao, i, 1, &imagem)
                    .unwrap();
            }
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 200);
        let com = l.ler_com_imagem(0, 0).unwrap();
        assert_eq!(com.len(), 200);
        assert_eq!(com[199].1.len(), 200 % 97);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O que a cura existe para impedir: gravar por cima. Depois de reabrir,
    /// o evento novo tem de entrar DEPOIS dos que ja estavam.
    #[test]
    fn depois_da_cura_o_novo_evento_nao_sobrescreve() {
        let d = dir_temp("cura-sobrescreve");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=50u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 51, 1).unwrap();
        l.sincronizar().unwrap();

        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.len(), 51, "o evento novo comeu os antigos");
        assert_eq!(eventos[49].rowid, 50);
        assert_eq!(eventos[50].rowid, 51);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Sincronizado, nao ha o que curar -- e reabrir tem de dar o mesmo.
    #[test]
    fn com_sincronizar_a_cura_nao_muda_nada() {
        let d = dir_temp("cura-nada");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=30u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
            l.sincronizar().unwrap();
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 30);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn registra_as_tres_operacoes_em_ordem() {'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
