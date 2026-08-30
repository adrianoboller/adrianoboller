# Add uuid lookup and honest attachment count
# 28/08 17:55

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    fn op_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 200).max(0) as u64;
        let com_anexos = p.booleano_ou("com_anexos", false);
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let descartadas = t.lixeira(pular, limite, com_anexos)?;'''
novo='''    fn op_lixeira(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pular = p.inteiro_ou("pular", 0).max(0) as u64;
        let limite = p.inteiro_ou("limite", 200).max(0) as u64;
        // Um `uuid` pede UMA linha, e ai os anexos vem sempre: quem pediu uma
        // linha especifica quer ela inteira. Sem uuid e listagem, e a listagem
        // nao carrega anexo por padrao -- um memo de megabytes vezes trezentas
        // linhas viraria uma resposta que ninguem consegue usar.
        let so_uma = p.texto_ou("uuid", "").trim().to_string();
        let com_anexos = p.booleano_ou("com_anexos", !so_uma.is_empty());
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let descartadas = if so_uma.is_empty() {
            t.lixeira(pular, limite, com_anexos)?
        } else {
            let alvo = phxsql_core::uuid::Uuid::de_texto(&so_uma)
                .map_err(|e| PhxError::Esquema(format!("uuid da linha descartada: {e}")))?;
            t.lixeira(0, 0, true)?
                .into_iter()
                .filter(|d| d.uuid.bytes() == alvo.bytes())
                .collect()
        };'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''                ("bytes", Json::de_u64(d.tamanho() as u64)),
                ("anexos", Json::de_u64(d.externos.len() as u64)),'''
novo2='''                ("bytes", Json::de_u64(d.tamanho() as u64)),
                // Do CABECALHO, e nao do vetor: numa listagem leve o vetor
                // esta vazio, e dizer "0 anexos" para uma linha que tem tres
                // faria quem investiga concluir que a foto nunca existiu.
                ("anexos", Json::de_u64(d.n_externos as u64)),'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            ("bytes", Json::de_u64(bytes)),
            ("colunas", crate::valores::colunas_para_json(&esquema)),
            ("descartadas", Json::Lista(linhas)),
        ]))'''
novo3='''        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("total", Json::de_u64(total)),
            ("bytes", Json::de_u64(bytes)),
            // A tela precisa saber se um campo externo vazio quer dizer "nao
            // tinha" ou "nao carreguei". Sao coisas diferentes.
            ("anexos_carregados", Json::Bool(com_anexos)),
            ("colunas", crate::valores::colunas_para_json(&esquema)),
            ("descartadas", Json::Lista(linhas)),
        ]))'''
assert velho3 in s
s=s.replace(velho3,novo3,1)
io.open(p,'w',encoding='utf-8').write(s)
