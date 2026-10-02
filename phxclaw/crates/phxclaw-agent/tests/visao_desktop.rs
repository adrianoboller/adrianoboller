//! A ferramenta `desktop` contra um X de verdade (Xvfb): a recusa sem DISPLAY, o mouse que
//! o SO devolve na mesma posicao, a captura que nao e de uma cor so (contada por um
//! decodificador de fora, o ffmpeg) e o teclado que chega a um xterm e vira arquivo.
//!
//! Um teste so, de proposito: ele mexe no DISPLAY do processo, e variavel de ambiente
//! trocada com outro teste rodando ao lado e corrida.

#[cfg(not(feature = "desktop"))]
use phxclaw_test_support::pulado;

#[cfg(not(feature = "desktop"))]
#[test]
fn desktop_desligado() {
    pulado::pular(
        "feature desktop",
        "desligada (cargo test -p phxclaw-agent --features desktop)",
    );
}

#[cfg(feature = "desktop")]
mod com_desktop {
    use phxclaw_agent::visao::DesktopTool;
    use phxclaw_agent_core::{Tool, ToolContext, ToolError};
    use phxclaw_test_support::pulado;
    use serde_json::json;
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    /// Mata o filho pelo PID exato ao sair, inclusive no panico.
    struct Filho(Child);
    impl Drop for Filho {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn ha(bin: &str) -> bool {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
            .unwrap_or(false)
    }

    /// Primeiro display livre a partir do :95, para nao pisar no Xvfb de outra frente.
    /// `-noreset`: cada acao abre e fecha a sua conexao X, e o Xvfb sem ele se reinicia
    /// quando o ultimo cliente sai -- medido: o mouse voltava ao centro (400,300) entre
    /// o `move` e o `position`. Desktop de verdade sempre tem cliente aberto.
    fn subir_xvfb() -> (Filho, String) {
        for n in 95..140 {
            if Path::new(&format!("/tmp/.X{n}-lock")).exists()
                || Path::new(&format!("/tmp/.X11-unix/X{n}")).exists()
            {
                continue;
            }
            let filho = Filho(
                Command::new("Xvfb")
                    .args([
                        format!(":{n}").as_str(),
                        "-screen",
                        "0",
                        "800x600x24",
                        "-nolisten",
                        "tcp",
                        "-noreset",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_secs(10) {
                if Path::new(&format!("/tmp/.X11-unix/X{n}")).exists() {
                    return (filho, format!(":{n}"));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        panic!("nenhum Xvfb subiu entre :95 e :139");
    }

    fn ctx() -> ToolContext {
        let d = std::env::temp_dir().join(format!("phx-desk-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        ToolContext {
            task_id: "t".into(),
            workdir: d,
            timeout: Duration::from_secs(20),
        }
    }

    /// Cores distintas do PNG pelos pixels que o ffmpeg decodifica: a prova nao pode ser a
    /// contagem que a propria ferramenta escreveu.
    fn cores_do_png(p: &Path) -> usize {
        let out = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-i"])
            .arg(p)
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "ffmpeg: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
            .as_chunks::<3>()
            .0
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    #[tokio::test]
    async fn desktop_no_xvfb() {
        let c = ctx();
        // SAFETY: e o unico teste deste binario e o runtime e de uma thread so; ninguem le
        // o ambiente ao mesmo tempo.
        unsafe {
            std::env::remove_var("DISPLAY");
            std::env::remove_var("WAYLAND_DISPLAY");
        }
        let e = DesktopTool
            .run(json!({"action":"position"}), &c)
            .await
            .unwrap_err();
        assert!(
            matches!(&e, ToolError::Denied(m) if m.contains("sem DISPLAY")),
            "{e}"
        );
        if !(ha("Xvfb") && ha("xterm") && ha("ffmpeg")) {
            pulado::pular("Xvfb", "falta Xvfb, xterm ou ffmpeg");
            return;
        }
        let (_xvfb, display) = subir_xvfb();
        // SAFETY: idem acima
        unsafe { std::env::set_var("DISPLAY", &display) };

        // acao desconhecida e argumento faltando
        let e = DesktopTool
            .run(json!({"action":"voar"}), &c)
            .await
            .unwrap_err();
        assert!(e.to_string().contains("acao desconhecida"), "{e}");
        let e = DesktopTool
            .run(json!({"action":"move","x":10}), &c)
            .await
            .unwrap_err();
        assert!(e.to_string().contains("'x' e 'y' juntos"), "{e}");

        // mouse: vai e o SO devolve a mesma posicao, por uma chamada separada
        DesktopTool
            .run(json!({"action":"move","x":321,"y":234}), &c)
            .await
            .unwrap();
        let r = DesktopTool
            .run(json!({"action":"position"}), &c)
            .await
            .unwrap();
        assert_eq!(r.content, "mouse em (321,234)");

        // teclado: um xterm que grava a linha lida; sem gerenciador de janelas o foco
        // segue o ponteiro, entao o clique dentro dele basta
        let gravado = c.workdir.join("lido.txt");
        let _xterm = Filho(
            Command::new("xterm")
                .args(["-geometry", "60x5+40+40", "-e", "sh", "-c"])
                // grava ao lado e renomeia: o teste olhando entre o `>` e o `printf` lia
                // vazio (medido: uma corrida deu "" em vez do codigo)
                .arg(format!(
                    "read l; printf %s \"$l\" > '{0}.tmp' && mv '{0}.tmp' '{0}'",
                    gravado.display()
                ))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        std::thread::sleep(Duration::from_secs(2));
        let codigo = format!(
            "phx{}",
            &phxclaw_types::new_uuid_v7().simple().to_string()[20..]
        );
        DesktopTool
            .run(json!({"action":"click","x":120,"y":70}), &c)
            .await
            .unwrap();
        DesktopTool
            .run(json!({"action":"type","text":codigo}), &c)
            .await
            .unwrap();
        // captura, com o xterm na tela
        let r = DesktopTool
            .run(
                json!({"action":"screenshot","path":"capturas/tela.png"}),
                &c,
            )
            .await
            .unwrap();
        assert!(r.content.contains("captura 800x600"), "{}", r.content);
        assert_eq!(r.artifacts[0].path, "capturas/tela.png");
        let cores = cores_do_png(&c.workdir.join("capturas/tela.png"));
        assert!(cores > 1, "captura de uma cor so: {cores}");

        // so agora o enter: o xterm fecha ao ler a linha
        DesktopTool
            .run(json!({"action":"key","key":"enter"}), &c)
            .await
            .unwrap();

        let t0 = Instant::now();
        while !gravado.is_file() && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(100));
        }
        let lido = std::fs::read_to_string(&gravado).unwrap_or_default();
        assert_eq!(lido, codigo, "o xterm nao recebeu o que foi digitado");

        let e = DesktopTool
            .run(json!({"action":"screenshot","path":"../fora.png"}), &c)
            .await
            .unwrap_err();
        assert!(matches!(e, ToolError::Denied(_)), "{e}");
    }
}
