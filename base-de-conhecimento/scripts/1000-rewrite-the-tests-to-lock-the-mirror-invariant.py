# Rewrite the tests to lock the mirror invariant
# 29/08 02:10

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
i = s.index("/// O profiler desligado nao pode custar nada.")
novo = '''/// O profiler desligado nao pode custar nada.
///
/// Ate a 0.17.0 ele custava, e escondido: TODO pedido pagava dois
/// `Json::analisar` do corpo inteiro, tres `String` e um mutex ANTES de
/// `chegou` olhar `ligado` e devolver `None`. Num `inserir_lote` de cinco mil
/// linhas era analisar meio megabyte de JSON duas vezes, para nada -- medido em
/// 7% da carga pela rede (`bancada/carga/medir.py`).
///
/// O portao barato e um `AtomicBool`, e o que pode dar errado nele e
/// DIVERGIR do estado real: espelho presa em `true` faz o servidor pagar o
/// parse para sempre; presa em `false` faz o profiler nao ver nada, ligado.
/// E isso que estes testes travam.
///
/// A captura em si mora no laco da conexao, e nao no `despachar` -- entao ela
/// nao se exercita por aqui. Quem a exercita e a bancada, e o numero dela e o
/// que denuncia se o portao sumir.
#[cfg(test)]
mod testes_profiler_desligado {
    use super::*;
    use crate::usuarios::Cadastro;

    fn dir_temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-prof-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pedido(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            cadastro: Cadastro::default(),
            ..Config::default()
        };
        Servidor::novo(c).unwrap()
    }

    /// O espelho e o estado de verdade, lado a lado.
    fn conferir(s: &Arc<Servidor>, esperado: bool) {
        let real = s.profiler.lock().unwrap().ligado();
        let espelho = s.profiler_ligado.load(Ordering::Relaxed);
        assert_eq!(real, esperado, "o profiler de verdade");
        assert_eq!(
            espelho, real,
            "o espelho divergiu do profiler: espelho={espelho}, real={real}"
        );
    }

    /// Nasce desligado, senao o caminho quente pagaria desde o arranque por um
    /// profiler que ninguem ligou.
    #[test]
    fn nasce_desligado() {
        let dir = dir_temp("nasce");
        let s = servidor(&dir);
        conferir(&s, false);
    }

    /// Ligar e desligar, varias vezes: o espelho acompanha em toda volta.
    #[test]
    fn o_espelho_nunca_diverge() {
        let dir = dir_temp("espelho");
        let s = servidor(&dir);
        let sessao = Sessao::default();
        for volta in 1..=3 {
            s.executar("profiler_ligar", &pedido("{}"), &sessao).unwrap();
            conferir(&s, true);
            s.executar("profiler_desligar", &pedido("{}"), &sessao)
                .unwrap();
            conferir(&s, false);
            assert!(volta <= 3);
        }
    }

    /// Ligar duas vezes seguidas nao pode deixar o espelho para tras -- e
    /// desligar duas vezes tambem nao.
    #[test]
    fn ligar_ou_desligar_repetido_nao_confunde_o_espelho() {
        let dir = dir_temp("repetido");
        let s = servidor(&dir);
        let sessao = Sessao::default();

        s.executar("profiler_ligar", &pedido("{}"), &sessao).unwrap();
        s.executar("profiler_ligar", &pedido("{}"), &sessao).unwrap();
        conferir(&s, true);

        s.executar("profiler_desligar", &pedido("{}"), &sessao)
            .unwrap();
        s.executar("profiler_desligar", &pedido("{}"), &sessao)
            .unwrap();
        conferir(&s, false);
    }

    /// Ligar com filtro tambem liga o espelho: o filtro decide o que ENTRA no
    /// anel, e nao se a observacao existe.
    #[test]
    fn ligar_com_filtro_tambem_liga_o_espelho() {
        let dir = dir_temp("filtro");
        let s = servidor(&dir);
        s.executar(
            "profiler_ligar",
            &pedido(r#"{"database":"b","so_escrita":true}"#),
            &Sessao::default(),
        )
        .unwrap();
        conferir(&s, true);
    }
}
'''
s = s[:i] + novo
p.write_text(s)
print("ok")
