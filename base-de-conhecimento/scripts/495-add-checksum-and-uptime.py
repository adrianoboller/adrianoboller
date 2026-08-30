# Add checksum and uptime
# 28/08 16:28

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# ---- uptime
a='''    /// Amostra anterior da maquina, para as taxas do painel.'''
b='''    /// Quando o servidor subiu, para o `ping` poder dizer ha quanto tempo.
    ///
    /// Um servidor que reiniciou sozinho de madrugada parece igual a um que
    /// nunca caiu -- ate alguem olhar o tempo no ar e ver duas horas.
    desde_ms: i64,
    /// Amostra anterior da maquina, para as taxas do painel.'''
assert a in s; s=s.replace(a,b,1)
a='''            monitor: Mutex::new(crate::sistema::Monitor::novo()),'''
b='''            desde_ms: crate::agora_ms(),
            monitor: Mutex::new(crate::sistema::Monitor::novo()),'''
assert a in s; s=s.replace(a,b,1)
a='''            "ping" => Ok(Json::objeto(vec![
                ("phxsql", Json::texto_de(VERSAO)),
                ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
                (
                    "conexoes",
                    Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                ),
            ])),'''
b='''            "ping" => Ok(Json::objeto(vec![
                ("phxsql", Json::texto_de(VERSAO)),
                ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
                (
                    "conexoes",
                    Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                ),
                (
                    "no_ar_s",
                    Json::de_u64(((crate::agora_ms() - self.desde_ms) / 1_000).max(0) as u64),
                ),
                (
                    "desde",
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
                ),
            ])),'''
assert a in s; s=s.replace(a,b,1)

# ---- checksum
a='''    // ------------------------------------------------------- estatisticas'''
b='''    /// A impressao digital de uma tabela, para comparar duas copias.
    ///
    /// # Para que serve
    ///
    /// Responder "estas duas tabelas sao a mesma?" sem transportar as duas.
    /// E o que falta para conferir uma replica contra a origem, e para provar
    /// que um backup restaurado ficou igual ao original -- hoje o
    /// `conferir-backup` compara ARQUIVO, e arquivo igual e mais forte do que
    /// preciso: dois `.reg` podem diferir no enchimento e ter o mesmo dado.
    ///
    /// # Como a conta e feita
    ///
    /// CRC-32 de cada linha viva, dobrado num acumulador que **depende da
    /// ordem**. Depender da ordem e de proposito: no PhxSql a ordem de
    /// digitacao E o dado, e duas tabelas com as mesmas linhas em ordem
    /// diferente nao sao a mesma tabela.
    ///
    /// Slot excluido nao entra. Se entrasse, restaurar um backup daria outro
    /// numero so porque os buracos caem em outro lugar.
    fn op_checksum(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let comeco = Instant::now();
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut soma: u64 = 0xcbf2_9ce4_8422_2325; // semente do FNV-1a de 64
        let mut linhas = 0u64;
        for (rowid, _) in t.varrer()? {
            let Some(linha) = t.ler(rowid)? else { continue };
            // A linha volta a forma canonica antes de entrar na conta: somar o
            // byte cru do slot faria o enchimento de um `Str` de largura fixa
            // pesar, e duas tabelas iguais com larguras diferentes dariam
            // numeros diferentes.
            let mut texto = String::with_capacity(64);
            for (v, c) in linha.iter().zip(esquema.colunas()) {
                texto.push('\\u{1}');
                if v.e_null() {
                    texto.push('\\u{0}');
                } else {
                    texto.push_str(&crate::valores::valor_para_json(v, &c.ty).escrever());
                }
            }
            let crc = phxsql_core::crc::crc32(texto.as_bytes()) as u64;
            // Multiplicar antes de somar e o que faz a ordem contar: trocar
            // duas linhas de lugar muda o resultado.
            soma = (soma ^ crc).wrapping_mul(0x1000_0000_01b3);
            linhas += 1;
        }

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("checksum", Json::texto_de(format!("{soma:016x}"))),
            ("linhas", Json::de_u64(linhas)),
            ("slots", Json::de_u64(t.registros())),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }

    // ------------------------------------------------------- estatisticas'''
assert a in s; s=s.replace(a,b,1)
a='''            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),'''
b='''            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),
            "checksum" | "soma_de_verificacao" => self.op_checksum(p, sessao),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
