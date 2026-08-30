# Add the profiler operations
# 28/08 22:57

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

antigo = """            "posicao" => self.op_posicao(p, sessao),"""
novo = """            "profiler_ligar" => self.op_profiler_ligar(p),
            "profiler_desligar" => self.op_profiler_desligar(),
            "profiler" => self.op_profiler(p),
            "profiler_limpar" => self.op_profiler_limpar(),
            "posicao" => self.op_posicao(p, sessao),"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    // ----------------------------------------------------------- replicacao
"""
novo = """    // ------------------------------------------------------------- profiler

    /// `profiler_ligar`: comeca a observar o que chega pela porta.
    ///
    /// **So administrador**, e a razao esta no que ele mostra: o texto dos
    /// pedidos de todo mundo, com os dados que estao sendo gravados dentro.
    /// Quem pode ler uma tabela nao ganha por isto o direito de ver o que os
    /// outros escrevem nela.
    fn op_profiler_ligar(&self, p: &Json) -> Result<Json> {
        let filtro = crate::profiler::Filtro {
            database: p.texto_ou("database", "").trim().to_string(),
            usuario: p.texto_ou("usuario", "").trim().to_string(),
            op: p.texto_ou("operacao", "").trim().to_string(),
            so_escrita: p.booleano_ou("so_escrita", false),
        };
        let arquivo = p.texto_ou("arquivo", "").to_string();
        let teto = p.inteiro_ou("guardar", 500).max(0) as usize;
        let agora = crate::agora_ms();
        let mut prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        prof.ligar(filtro, &arquivo, teto, agora)?;
        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(true)),
            ("guardar", Json::de_u64(prof.teto() as u64)),
            (
                "arquivo",
                Json::texto_de(prof.caminho().display().to_string()),
            ),
            ("desde", Json::texto_de(phxsql_core::datahora::instante_iso(agora))),
        ]))
    }

    fn op_profiler_desligar(&self) -> Result<Json> {
        let mut prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        let n = prof.observados();
        prof.desligar(crate::agora_ms());
        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(false)),
            ("observados", Json::de_u64(n)),
        ]))
    }

    fn op_profiler_limpar(&self) -> Result<Json> {
        let mut prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        prof.limpar();
        Ok(Json::objeto(vec![("limpo", Json::Bool(true))]))
    }

    /// `profiler`: o que foi observado, do mais recente para o mais antigo.
    fn op_profiler(&self, p: &Json) -> Result<Json> {
        let max = p.inteiro_ou("max", 200).max(0) as usize;
        // `desde_serial` deixa a tela pedir so o que ainda nao viu, em vez de
        // rebaixar o anel inteiro a cada atualizacao.
        let desde = p.inteiro_ou("desde_serial", 0).max(0) as u64;
        let prof = self.profiler.lock().map_err(|_| trava_envenenada())?;
        let f = prof.filtro();

        let eventos: Vec<Json> = prof
            .eventos(max.clamp(1, 5_000))
            .into_iter()
            .filter(|e| e.serial > desde)
            .map(|e| {
                Json::objeto(vec![
                    ("serial", Json::de_u64(e.serial)),
                    (
                        "quando",
                        Json::texto_de(phxsql_core::datahora::instante_iso(e.quando_ms)),
                    ),
                    ("ip", Json::texto_de(e.ip)),
                    ("usuario", Json::texto_de(e.usuario)),
                    ("op", Json::texto_de(e.op)),
                    ("database", Json::texto_de(e.database)),
                    ("tabela", Json::texto_de(e.tabela)),
                    ("bytes", Json::de_u64(e.bytes as u64)),
                    // Ja vem redigido: os campos de senha viraram *** antes de
                    // encostar no anel.
                    ("pedido", Json::texto_de(e.pedido)),
                    (
                        "ms",
                        match e.duracao_ms {
                            Some(ms) => Json::de_u64(ms),
                            None => Json::Nulo,
                        },
                    ),
                    (
                        "ok",
                        match e.ok {
                            Some(v) => Json::Bool(v),
                            None => Json::Nulo,
                        },
                    ),
                    ("erro", Json::texto_de(e.erro)),
                ])
            })
            .collect();

        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(prof.ligado())),
            (
                "arquivo",
                Json::texto_de(prof.caminho().display().to_string()),
            ),
            ("observados", Json::de_u64(prof.observados())),
            ("esquecidos", Json::de_u64(prof.esquecidos())),
            ("guardar", Json::de_u64(prof.teto() as u64)),
            (
                "desde",
                Json::texto_de(if prof.ligado() {
                    phxsql_core::datahora::instante_iso(prof.ligado_em_ms())
                } else {
                    String::new()
                }),
            ),
            (
                "filtro",
                Json::objeto(vec![
                    ("database", Json::texto_de(&f.database)),
                    ("usuario", Json::texto_de(&f.usuario)),
                    ("operacao", Json::texto_de(&f.op)),
                    ("so_escrita", Json::Bool(f.so_escrita)),
                ]),
            ),
            ("eventos", Json::Lista(eventos)),
        ]))
    }

    // ----------------------------------------------------------- replicacao
"""
assert antigo in s
s = s.replace(antigo, novo, 1)
p.write_text(s)
print("ok")
