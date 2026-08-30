# Preserve softdeleted on update; read memoria fixture
# 28/08 17:36

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let linha = json_para_linha(&valores_json, t.esquema())?;
        t.atualizar(rowid, &linha)?;
        self.gravar_de_verdade(&mut t, p)?;
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))
    }'''
novo='''        let mut linha = json_para_linha(&valores_json, t.esquema())?;

        // Quem alterou a linha nao mandou a coluna de sistema? Entao ela nao
        // muda. Sem isto, `json_para_linha` preencheria `false` e um
        // `atualizar` de rotina RESSUSCITARIA uma linha excluida -- sem erro
        // nenhum, e sem ninguem perceber ate a linha reaparecer na lista.
        if let Some(i) = t.esquema().coluna_softdeleted() {
            let nome = phxsql_core::schema::COLUNA_SOFTDELETED;
            let veio = matches!(&valores_json, Json::Objeto(_)) && valores_json.campo(nome).is_some()
                || matches!(&valores_json, Json::Lista(l) if l.len() > i);
            if !veio {
                if let Some(atual) = t.ler(rowid)? {
                    linha[i] = atual[i].clone();
                }
            }
        }

        t.atualizar(rowid, &linha)?;
        self.gravar_de_verdade(&mut t, p)?;
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
