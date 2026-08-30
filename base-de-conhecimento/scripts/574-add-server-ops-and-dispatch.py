# Add server ops and dispatch
# 28/08 17:39

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let removeu = t.excluir(rowid)?;
        self.gravar_de_verdade(&mut t, p)?;
        if removeu {
            self.residente_mut(p, |m| m.anotar_exclusao(rowid));
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(removeu)),
        ]))
    }'''

novo='''    /// Exclui. **Suave por padrao**, fisica so quando pedida.
    ///
    /// # Por que o padrao e o suave
    ///
    /// O caminho reversivel e o padrao porque o irreversivel nao pode ser
    /// escolhido por omissao: um cliente antigo que manda `excluir` sem dizer
    /// nada esta pedindo "tira isto da minha lista", e e isso que ele recebe.
    /// Quem quer apagar de vez escreve `"fisico": true` e sabe o que esta
    /// fazendo. Numa tabela sem a coluna de sistema -- as anteriores a v4 do
    /// esquema -- so existe o caminho fisico, e ele e usado sem alarde.
    fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        let fisico = p.booleano_ou("fisico", false);
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let tem_marca = t.esquema().coluna_softdeleted().is_some();

        if fisico || !tem_marca {
            let removeu = t.excluir_de_vez(rowid, &motivo)?;
            self.gravar_de_verdade(&mut t, p)?;
            if removeu {
                self.residente_mut(p, |m| m.anotar_exclusao(rowid));
            }
            return Ok(Json::objeto(vec![
                ("rowid", Json::de_u64(rowid)),
                ("excluido", Json::Bool(removeu)),
                ("modo", Json::texto_de("fisico")),
                ("na_lixeira", Json::Bool(removeu)),
                ("reversivel", Json::Bool(false)),
            ]));
        }

        let marcou = t.excluir_suave(rowid, &motivo)?;
        self.gravar_de_verdade(&mut t, p)?;
        // A copia em RAM tem de esquecer a linha tambem: para quem consulta,
        // marcada e o mesmo que ausente.
        if marcou {
            self.residente_mut(p, |m| m.anotar_exclusao(rowid));
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(marcou)),
            ("modo", Json::texto_de("suave")),
            ("na_lixeira", Json::Bool(false)),
            ("reversivel", Json::Bool(true)),
        ]))
    }

    /// Desfaz uma exclusao suave.
    fn op_restaurar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let voltou = t.restaurar(rowid, &motivo)?;
        self.gravar_de_verdade(&mut t, p)?;
        if voltou {
            // A linha volta a existir para quem consulta em memoria.
            if let Some(linha) = t.ler(rowid)? {
                self.residente_mut(p, |m| m.anotar_insercao(rowid, &linha));
            }
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("restaurado", Json::Bool(voltou)),
        ]))
    }

    /// `lixeira`: as linhas que sairam do `.reg`. **So administrador.**
    ///
    /// Os anexos so vao junto com `"com_anexos": true`: listar mil linhas
    /// carregaria mil fotos para mostrar quem excluiu o que e quando.
    fn op_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 200).max(0) as u64;
        let com_anexos = p.booleano_ou("com_anexos", false);
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let descartadas = t.lixeira(pular, limite, com_anexos)?;
        let (total, bytes) = t.lixeira_tamanho()?;
        let esquema = t.esquema().clone();

        let mut linhas = Vec::with_capacity(descartadas.len());
        for d in &descartadas {
            // A linha pode nao decodificar: se o esquema mudou depois do
            // descarte, o payload guardado nao bate com ele. Isso nao pode
            // derrubar a listagem inteira -- a entrada aparece com o aviso, e
            // as outras continuam sendo mostradas.
            let (linha, aviso) = match t.linha_da_lixeira(d) {
                Ok(l) => (crate::valores::linha_para_json(&l, &esquema), String::new()),
                Err(e) => (Json::Nulo, e.to_string()),
            };
            linhas.push(Json::objeto(vec![
                ("uuid", Json::texto_de(&d.uuid.to_string())),
                ("rowid", Json::de_u64(d.rowid)),
                ("quando", Json::texto_de(&d.instante_iso())),
                ("usuario", Json::de_u64(d.usuario as u64)),
                ("usuario_nome", Json::texto_de(&self.nome_do_usuario(d.usuario))),
                ("bytes", Json::de_u64(d.tamanho() as u64)),
                ("anexos", Json::de_u64(d.externos.len() as u64)),
                ("linha", linha),
                ("aviso", Json::texto_de(&aviso)),
            ]));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            ("bytes", Json::de_u64(bytes)),
            ("colunas", crate::valores::colunas_para_json(&esquema)),
            ("descartadas", Json::Lista(linhas)),
        ]))
    }

    /// `motivos`: por que cada linha foi excluida. **So administrador.**
    fn op_motivos(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 500).max(0) as u64;
        let so_do_rowid = p.campo("rowid").is_some();
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let lista = if so_do_rowid {
            t.motivos_de(self.rowid(p)?)?
        } else {
            t.motivos(pular, limite)?
        };
        let total = t.total_de_motivos()?;
        let exige = t.esquema().motivo_obrigatorio();

        let registros = lista
            .iter()
            .map(|m| {
                Json::objeto(vec![
                    ("uuid", Json::texto_de(&m.uuid.to_string())),
                    ("rowid", Json::de_u64(m.rowid)),
                    ("quando", Json::texto_de(&m.instante_iso())),
                    ("carimbo", Json::de_i64(m.carimbo)),
                    ("tipo", Json::texto_de(m.tipo.nome())),
                    ("motivo", Json::texto_de(&m.motivo)),
                    ("identidade", Json::texto_de(&m.identidade)),
                    ("usuario", Json::de_u64(m.usuario as u64)),
                    ("usuario_nome", Json::texto_de(&self.nome_do_usuario(m.usuario))),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            ("motivo_obrigatorio", Json::Bool(exige)),
            ("motivos", Json::Lista(registros)),
        ]))
    }

    /// `esvaziar_lixeira`: daqui nao volta. **So administrador.**
    ///
    /// O expurgo e registrado no `.reason` ANTES de a lixeira ser apagada: o
    /// motivo tem de sobreviver ao dado, senao o rastro some junto com ele.
    fn op_esvaziar_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let motivo = p.texto_ou("motivo", "").trim().to_string();
        if motivo.is_empty() {
            return Err(PhxError::Esquema(
                "informe \\"motivo\\": esvaziar a lixeira nao tem volta, e sem o \\
                 registro do por que nao sobra rastro nenhum"
                    .into(),
            ));
        }
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let apagadas = t.esvaziar_lixeira(&motivo)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("apagadas", Json::de_u64(apagadas)),
        ]))
    }'''
assert velho in s
s=s.replace(velho,novo,1)

# despacho
velho2='''            "excluir" => self.op_excluir(p, sessao),'''
novo2='''            "excluir" => self.op_excluir(p, sessao),
            "restaurar" => self.op_restaurar(p, sessao),
            "lixeira" | "trash" => self.op_lixeira(p, sessao),
            "motivos" | "reasons" => self.op_motivos(p, sessao),
            "esvaziar_lixeira" => self.op_esvaziar_lixeira(p, sessao),'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
