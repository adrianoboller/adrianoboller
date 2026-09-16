//! A catraca do mapa das threads, dentro da SUITE -- e o que este arquivo muda
//! e' *quando* a regua roda, nunca *quanto* ela permite.
//!
//! # O defeito que motivou este arquivo
//!
//! Pendencia #252, metade (2), medida em 16/09/2026: as catracas dos dois
//! mapas de concorrencia so' rodavam no item 0 da bateria de ponta a ponta, e
//! a bateria e' um comando que alguem tem de lembrar de dar. A ultima corrida
//! versionada era de **29/08**; a seguinte, de **16/09**. Dezoito dias -- e
//! nos ultimos **oito** a catraca `alcancam-fsync` do mapa da trava esteve
//! furada (23 com teto 22, desde o merge `6245491` do PITR em 08/09 17:17),
//! com tres rodadas de integracao de `fmt`, `clippy` e suite VERDES sem que
//! nada acusasse. A catraca nao morava na suite.
//!
//! E' a mesma doenca do pedido 263, pago no mesmo dia por outro caminho:
//! **guarda que so' se confere quando alguem lembra e' guarda que nao se
//! confere.** La foram quatro dias e onze guardas quebradas; aqui, dezoito
//! dias.
//!
//! # Por que ESTA catraca entrou aqui, e a do mapa da trava nao
//!
//! Medido em 16/09/2026, tres corridas de cada, **com a carga da maquina
//! anotada ao lado** -- 4 nucleos, e outras frentes compilando. Tempo de
//! PAREDE medido sob carga e' teto superior, nao o custo da regua: ele diz
//! quanto a corrida demorou nesta maquina naquele minuto. O custo da regua e'
//! o tempo de CPU (user+sys), que nao cresce porque o vizinho compila.
//!
//! | Medidor | parede @ load ~4,9 | parede @ load ~9,4 | parede @ load ~14,8 | cpu @ ~9,4 / ~14,8 |
//! |---|---|---|---|---|
//! | `mapa-das-threads.py --catraca` | 1,122 / 0,885 / 0,811 s | 0,740 / 0,791 / 0,845 s | 0,726 / 0,680 / 0,708 s | 0,67 s / 0,67 s |
//! | `mapa-da-trava.py --catraca` | 3,113 / 3,056 / 3,187 s | 3,877 / 3,864 / 4,369 s | 4,469 / 3,457 / 3,077 s | 3,10-3,81 s / 2,94-3,03 s |
//!
//! Reproduza com `python3 bancada/concorrencia/custo-das-catracas.py`.
//!
//! O `1,122 s` da primeira corrida nao era carga: era **cache frio** -- a
//! rodada com o triplo de carga saiu mais rapida. A ~3,7x de sobrescrita (4
//! nucleos a load 14,8, com o provador de guardas e dois `cargo test` ao
//! lado) a PIOR corrida do mapa da trava foi 4,469 s, menos da metade dos dez
//! segundos que separariam a suite do `numeros-do-projeto.py`. **A escolha do
//! lugar e' a mesma nas tres cargas** -- e e' por isso que ela nao foi
//! decidida pelo numero, como se le abaixo.
//!
//! O estado atual: o das threads esta VERDE (`spawn-sem-teto` 0/0,
//! `catalogo-envelhecido` 0/0) e o da trava, VERMELHO (`alcancam-fsync` 23,
//! teto 22).
//!
//! Pelo CUSTO as duas caberiam aqui: um segundo e tres segundos somem dentro
//! de um `cargo test --workspace` que leva minutos. O que separa as duas nao
//! e' o numero, e' o ESTADO. A do mapa da trava esta vermelha por decisao do
//! dono -- a excecao da restauracao PITR e' a metade (1) da #252, parada com
//! ele --, e por a' num portao hoje deixaria a suite inteira vermelha para
//! todas as frentes ate' ele decidir. A saida mais barata dessa pressao seria
//! subir o teto de 22 para 23, que e' exatamente o que a petrea proibe: a
//! catraca so' desce. Entao ela ficou onde e' RELATO e nao portao -- item 0 da
//! bateria, aviso de hora em hora (`comunicacao.sh`, desde `560c63c`) e agora
//! tambem o `numeros-do-projeto.py`, que roda a cada rodada --, aparecendo
//! vermelha com a data em vez de sumir. Quando a #252 (1) for decidida, ela
//! entra aqui do mesmo jeito que esta.
//!
//! # O numero nao mora aqui, e isso e' de proposito
//!
//! Os tetos (`spawn-sem-teto` = 0, `catalogo-envelhecido` = 0) moram no
//! `CATRACAS` do proprio medidor. Uma copia deles neste arquivo seriam DUAS
//! contas, e catraca e inventario que divergem sao pior que catraca nenhuma --
//! e' a licao ja escrita no `catraca-fsync-por-fecho.rs` do `phxsql-store`.
//! Aqui ha uma medicao so', num lugar so': este teste **roda** o medidor e
//! cobra o veredito dele.

/// A raiz do repositorio, achada pelo proprio caminho deste teste.
///
/// `CARGO_MANIFEST_DIR` e' `<raiz>/crates/phxsql-server`; dois niveis acima
/// e' a raiz. Caminho relativo ao `cwd` mentiria: `cargo test` roda o binario
/// de teste com o `cwd` do pacote, e um `cargo` chamado da raiz ou de dentro
/// do crate dariam respostas diferentes.
fn raiz() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("dois niveis acima do crate e' a raiz do repositorio")
        .to_path_buf()
}

/// A catraca. **So desce.**
///
/// Ela reprova nos dois sentidos, e os dois estao no medidor: um sitio de
/// nascimento de thread sem entrada no catalogo (ou com entrada sem teto)
/// sobe o `spawn-sem-teto`; uma entrada do catalogo que nao casa com sitio
/// nenhum sobe o `catalogo-envelhecido`, porque catalogo velho e' pior que
/// catalogo nenhum -- ele parece completo.
#[test]
fn a_catraca_do_mapa_das_threads() {
    let medidor = raiz().join("bancada/concorrencia/mapa-das-threads.py");
    assert!(
        medidor.exists(),
        "o medidor desta catraca sumiu: {}\nSem ele a guarda nao guarda nada. \
         Reponha o arquivo, ou tire este teste no MESMO commit dizendo o motivo.",
        medidor.display()
    );

    // Sem `python3` a guarda NAO roda -- e guarda que nao roda tem de dizer
    // que nao rodou, nunca passar calada. Esta casa mede com Python (bateria,
    // dossie, catalogo de guardas): maquina sem `python3` e' bancada quebrada,
    // nao configuracao suportada.
    let saida = std::process::Command::new("python3")
        .arg(&medidor)
        .arg("--catraca")
        .current_dir(raiz())
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "nao consegui rodar `python3 {}`: {e}\nA catraca do mapa das \
                 threads NAO foi conferida -- e guarda que nao roda nao guarda. \
                 Instale o `python3` ou diga aqui por que esta maquina nao o tem.",
                medidor.display()
            )
        });

    let relato = String::from_utf8_lossy(&saida.stdout);
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        saida.status.success(),
        "a catraca do mapa das threads REPROVOU:\n{relato}{erro}\n\
         Toda thread de producao tem de estar no CATALOGO do medidor com o seu \
         teto e o lugar onde o teto mora -- e entrada que nao casa com sitio \
         nenhum e' entrada envelhecida, que tambem reprova. A catraca NAO sobe: \
         ponha o sitio no catalogo (ou tire a entrada morta) em vez de mexer no \
         teto.\nDetalhe: python3 bancada/concorrencia/mapa-das-threads.py"
    );
}
