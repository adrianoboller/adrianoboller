# Add the carga methods to the server
# 29/08 02:54

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# --------------------------------------------------- os metodos da carga
alvo = '''    fn gravar_de_verdade(&self, t: &mut Table, p: &Json) -> Result<()> {'''
novo = '''    // ------------------------------------------------------------- carga
    //
    // `BULKINSERT`: a tabela reservada para quem esta carregando, e so para
    // ele. Ver `crate::carga` para o desenho e para as duas redes de protecao
    // contra reserva orfa.

    /// Solta o que esta ligacao reservou, e sincroniza o que ficou por gravar.
    ///
    /// Roda na saida da conexao, por qualquer caminho. O `sincronizar` vai
    /// junto porque durante a reserva a janela de durabilidade fica aberta de
    /// proposito -- soltar sem fechar deixaria a carga inteira dependendo de o
    /// sistema operacional lembrar dela.
    fn soltar_cargas_da_ligacao(&self, ligacao: u64) {
        let soltas = match self.cargas.lock() {
            Ok(mut c) => c.soltar_da_ligacao(ligacao),
            Err(_) => return,
        };
        if soltas.is_empty() {
            return;
        }
        if let Ok(mut sujas) = self.sujas.lock() {
            for r in &soltas {
                sujas.insert(format!("{}/{}", r.database, r.tabela));
            }
        }
        self.descarregar_sujas();
    }

    /// A tabela deste pedido esta reservada por OUTRA ligacao?
    ///
    /// Uma reserva vencida e limpa aqui, que e onde alguem repara nela: um
    /// relogio de fundo so para isso seria uma linha de execucao acordando
    /// para, quase sempre, nao fazer nada.
    fn barrado_por_carga(&self, database: &str, tabela: &str, ligacao: u64) -> Option<String> {
        if database.is_empty() || tabela.is_empty() {
            return None;
        }
        self.cargas
            .lock()
            .ok()?
            .barra(database, tabela, ligacao, crate::agora_ms())
    }

    /// Esta tabela esta reservada por ESTA ligacao?
    ///
    /// Enquanto estiver, a janela de durabilidade fica aberta: a carga inteira
    /// vira um `fsync` so, no fim.
    fn em_carga_por_mim(&self, p: &Json, sessao: &Sessao) -> bool {
        let (db, tab) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
        if db.is_empty() || tab.is_empty() {
            return false;
        }
        let k = crate::carga::chave(db, tab);
        match self.cargas.lock() {
            Ok(c) => c
                .todas()
                .iter()
                .any(|r| r.ligacao == sessao.ligacao && crate::carga::chave(&r.database, &r.tabela) == k),
            Err(_) => false,
        }
    }

    /// `bulkinsert`: reserva a tabela para uma carga, ou solta.
    ///
    /// So pela porta de dados. Ver o porque em `crate::carga`.
    fn op_bulkinsert(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").trim().to_string();
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \\"database\\" e \\"tabela\\"".into(),
            ));
        }
        // Aceita `{"ligado":true}` e tambem `{"bulkinsert":true}`, que e como
        // o comando se le quando a camada SQL existir: BULKINSERT(true).
        let ligar = p
            .campo("ligado")
            .or_else(|| p.campo("bulkinsert"))
            .or_else(|| p.campo("valor"))
            .and_then(Json::booleano)
            .ok_or_else(|| {
                PhxError::Esquema("informe \\"ligado\\": true para reservar, false para soltar".into())
            })?;

        if sessao.ligacao == 0 {
            return Err(PhxError::Esquema(
                "BULKINSERT so vale pela porta de dados: HTTP nao tem conexao \\
                 para a reserva morrer amarrada. Pela tela, use \\"inserir_lote\\", \\
                 que ja e uma operacao so"
                    .into(),
            ));
        }

        // A tabela tem de existir -- reservar o que nao existe esconderia um
        // erro de digitacao ate o fim da carga.
        {
            let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
            dados.abrir_database(&database)?.abrir_qualificada(&tabela)?;
        }

        let agora = crate::agora_ms();
        let mut cargas = self.cargas.lock().map_err(|_| trava_envenenada())?;

        if ligar {
            let prazo = self.config.recursos.carga_prazo_min as i64 * 60_000;
            let r = cargas.reservar(
                &database,
                &tabela,
                sessao.login(),
                sessao.ligacao,
                "",
                agora,
                prazo,
            )?;
            drop(cargas);
            return Ok(Json::objeto(vec![
                ("bulkinsert", Json::Bool(true)),
                ("database", Json::texto_de(&database)),
                ("tabela", Json::texto_de(&tabela)),
                ("reservada", Json::Bool(true)),
                (
                    "expira_em_s",
                    Json::de_u64(((r.expira_ms - agora).max(0) / 1000) as u64),
                ),
                ("prazo_min", Json::de_u64(self.config.recursos.carga_prazo_min)),
            ]));
        }

        // Soltar: o dono solta o seu; o administrador solta o de qualquer um.
        let forcar = sessao
            .usuario
            .as_ref()
            .map(|u| u.e_admin())
            .unwrap_or(true);
        let r = cargas.soltar(&database, &tabela, sessao.ligacao, forcar, agora)?;
        drop(cargas);

        // O fsync que a carga inteira adiou acontece agora.
        {
            let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
            let mut t = self.abrir_travada(&_trava, p, sessao)?;
            t.sincronizar()?;
        }
        if let Ok(mut sujas) = self.sujas.lock() {
            sujas.remove(&format!("{database}/{tabela}"));
        }

        Ok(Json::objeto(vec![
            ("bulkinsert", Json::Bool(false)),
            ("database", Json::texto_de(&database)),
            ("tabela", Json::texto_de(&tabela)),
            ("liberada", Json::Bool(true)),
            (
                "durou_s",
                Json::de_u64(((agora - r.desde_ms).max(0) / 1000) as u64),
            ),
            ("sincronizada", Json::Bool(true)),
        ]))
    }

    /// `cargas`: quais tabelas estao reservadas agora, e por quem.
    fn op_cargas(&self) -> Result<Json> {
        let agora = crate::agora_ms();
        let c = self.cargas.lock().map_err(|_| trava_envenenada())?;
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(c.quantas() as u64)),
            (
                "cargas",
                Json::Lista(c.todas().iter().map(|r| r.para_json(agora)).collect()),
            ),
        ]))
    }

    fn gravar_de_verdade(&self, t: &mut Table, p: &Json) -> Result<()> {'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
