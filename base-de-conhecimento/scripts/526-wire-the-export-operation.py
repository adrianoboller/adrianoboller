# Wire the export operation
# 28/08 16:59

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''    // ------------------------------------------------------- estatisticas'''
b='''    /// Exporta uma tabela, ou o resultado de uma varredura, em sete formatos.
    ///
    /// Binario (XLSX, DOCX) volta em base64, porque o protocolo e JSON por
    /// linha e byte cru nao atravessa. Texto volta como texto, para caber num
    /// `curl` sem ninguem ter de decodificar nada.
    fn op_exportar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let f = Formato::de_texto(p.texto_ou("formato", "csv"))?;
        let comeco = Instant::now();
        // Teto proprio, e maior que o da varredura: exportar e justamente o
        // caso em que se quer a tabela inteira, e nao a primeira pagina.
        let teto = p.inteiro_ou("max", 100_000).clamp(1, 1_000_000) as usize;

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut linhas: Vec<Vec<Value>> = Vec::new();
        let mut truncado = false;
        for (rowid, _) in t.varrer()? {
            if linhas.len() >= teto {
                truncado = true;
                break;
            }
            if let Some(l) = t.ler(rowid)? {
                linhas.push(l);
            }
        }

        let nome = p.texto_ou("tabela", "tabela");
        let base = p.texto_ou("database", "");
        let planilha = crate::exportar::Planilha {
            titulo: nome.to_string(),
            subtitulo: format!(
                "{base} · {} linha(s) · exportado em {}",
                linhas.len(),
                phxsql_core::datahora::instante_iso(crate::agora_ms())
            ),
            colunas: crate::exportar::Planilha::do_esquema(&esquema, nome),
            linhas: &linhas,
        };
        let bytes = planilha.gerar(f)?;

        // O nome do arquivo sai daqui e nao da tela: quem chama por `curl` tem
        // o mesmo nome que quem clica, e o nome carrega a data.
        let arquivo = format!(
            "{}_{}.{}",
            nome.replace('.', "_"),
            phxsql_core::datahora::instante_iso(crate::agora_ms())
                .replace([' ', ':', ','], "-"),
            f.extensao()
        );

        let mut campos = vec![
            ("formato", Json::texto_de(p.texto_ou("formato", "csv"))),
            ("arquivo", Json::texto_de(&arquivo)),
            ("mime", Json::texto_de(f.mime())),
            ("bytes", Json::de_u64(bytes.len() as u64)),
            ("linhas", Json::de_u64(linhas.len() as u64)),
            ("truncado", Json::Bool(truncado)),
            ("binario", Json::Bool(f.binario())),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ];
        if f.binario() {
            campos.push(("base64", Json::texto_de(phxsql_core::base64::codificar(&bytes))));
        } else {
            campos.push((
                "conteudo",
                Json::texto_de(String::from_utf8_lossy(&bytes).to_string()),
            ));
        }
        Ok(Json::objeto(campos))
    }

    // ------------------------------------------------------- estatisticas'''
assert a in s; s=s.replace(a,b,1)
a='''            "checksum" | "soma_de_verificacao" => self.op_checksum(p, sessao),'''
b='''            "checksum" | "soma_de_verificacao" => self.op_checksum(p, sessao),
            "exportar" | "export" => self.op_exportar(p, sessao),'''
assert a in s; s=s.replace(a,b,1)
a='''use crate::juncao::{Lado, Tipo as TipoJuncao, Uniao};'''
b='''use crate::exportar::Formato;
use crate::juncao::{Lado, Tipo as TipoJuncao, Uniao};'''
assert a in s; s=s.replace(a,b,1)
a='''            "checksum",
            "sessoes",'''
b='''            "checksum",
            "exportar",
            "sessoes",'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
