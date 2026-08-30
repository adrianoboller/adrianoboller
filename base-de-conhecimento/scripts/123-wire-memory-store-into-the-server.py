# Wire memory store into the server
# 27/08 20:22

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# --- imports
s=s.replace('use phxsql_store::table::Table;',
            'use phxsql_store::memoria::{Consulta, Filtro, Operador, Ordem, TabelaMemoria};\nuse phxsql_store::table::Table;')
s=s.replace('use std::sync::atomic::{AtomicUsize, Ordering};',
            'use std::collections::HashMap;\nuse std::sync::atomic::{AtomicUsize, Ordering};')

# --- campo de residencia
s=s.replace('''    /// Sessoes do navegador. Vazio enquanto a interface web estiver desligada.
    sessoes: Mutex<http::Sessoes>,''','''    /// Sessoes do navegador. Vazio enquanto a interface web estiver desligada.
    sessoes: Mutex<http::Sessoes>,
    /// Tabelas residentes em RAM, por "database/tabela". Nada entra aqui
    /// sozinho: so o que alguem pediu para carregar.
    residentes: Mutex<HashMap<String, TabelaMemoria>>,''')
s=s.replace('            sessoes: Mutex::new(http::Sessoes::default()),',
            '            sessoes: Mutex::new(http::Sessoes::default()),\n            residentes: Mutex::new(HashMap::new()),')

# --- ops novas
s=s.replace('''            "diario" => self.op_diario(p, sessao),''','''            "diario" => self.op_diario(p, sessao),
            "memoria_carregar" => self.op_memoria_carregar(p, sessao),
            "memoria_liberar" => self.op_memoria_liberar(p),
            "memoria" => self.op_memoria(),
            // O nome que o Adriano pediu, e o nome em portugues do projeto.
            // Sao a mesma operacao: a interface usa um, o script usa o outro.
            "SelectMemory" | "selectmemory" | "selecionar_memoria" => {
                self.op_selecionar_memoria(p, sessao)
            }''')

# --- escrita mantem a residente de acordo
s=s.replace('''        let rowid = t.inserir(&linha)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("registros", Json::de_u64(t.registros())),
        ]))''','''        let rowid = t.inserir(&linha)?;
        t.sincronizar()?;
        // A copia em RAM acompanha DENTRO da mesma trava: nao existe instante
        // em que o disco e a memoria discordem.
        self.residente_mut(p, |m| m.anotar_insercao(rowid, &linha));
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("registros", Json::de_u64(t.registros())),
        ]))''')
s=s.replace('''        t.atualizar(rowid, &linha)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))''','''        t.atualizar(rowid, &linha)?;
        t.sincronizar()?;
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))''')
s=s.replace('''        let removeu = t.excluir(rowid)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![''','''        let removeu = t.excluir(rowid)?;
        t.sincronizar()?;
        if removeu {
            self.residente_mut(p, |m| m.anotar_exclusao(rowid));
        }
        Ok(Json::objeto(vec![''')

open(p,'w').write(s)
print('ok')
