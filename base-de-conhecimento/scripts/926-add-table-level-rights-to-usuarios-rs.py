# Add table-level rights to usuarios.rs
# 29/08 00:25

import pathlib
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs")
s = p.read_text()

# ---- campo novo
alvo = '''    /// Poder por base. A chave `"*"` vale para as bases nao listadas.
    pub bases: Vec<(String, Permissoes)>,
}'''
novo = '''    /// Poder por base. A chave `"*"` vale para as bases nao listadas.
    pub bases: Vec<(String, Permissoes)>,
    /// Poder por TABELA, dentro de cada base.
    ///
    /// Chave de fora: a base (ou `"*"`). Chave de dentro: a tabela (ou `"*"`).
    /// Vem de `"tabelas"` dentro do objeto da base, no `config.json`:
    ///
    /// ```json
    /// "bases": {
    ///   "Z": {
    ///     "ler": true, "inserir": true,
    ///     "tabelas": {
    ///       "folha":    { },
    ///       "clientes": { "ler": true, "inserir": true, "alterar": true }
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Por que e um campo separado, e nao um campo dentro de `bases`
    ///
    /// Se a regra da tabela morasse dentro do objeto da base, listar uma base
    /// so para escrever uma regra de tabela nela passaria a NEGAR tudo o mais
    /// naquela base -- porque a base listada ganha da regra `"*"`, e o objeto
    /// listado so por causa das tabelas teria as dez permissoes em `false`.
    /// Separado, a precedencia de base fica exatamente como era.
    pub tabelas: Vec<(String, Vec<(String, Permissoes)>)>,
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ---- permissoes_em / pode_em
alvo = '''    /// Pode fazer a atividade nesta base?
    pub fn pode(&self, database: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes(database).pode(atividade)
    }'''
novo = '''    /// O poder deste usuario nesta TABELA desta base.
    ///
    /// # Por que existe
    ///
    /// Ate a 0.17.0 a permissao parava na base: quem lia a base lia todas as
    /// tabelas dela, e nao havia como dar `clientes` sem dar `folha`. A folha
    /// de pagamento e a tabela de clientes moram no mesmo banco porque o
    /// negocio e um so, e o direito de ler as duas nao e o mesmo direito.
    ///
    /// # A ordem de precedencia
    ///
    /// A mesma regra que ja valia entre base e `"*"`: **o especifico ganha do
    /// geral, e substitui**. Do mais especifico para o mais geral:
    ///
    /// 1. supervisor -- pode tudo, em toda tabela;
    /// 2. a regra desta tabela nesta base;
    /// 3. a regra `"*"` de tabela nesta base;
    /// 4. a regra desta tabela na base `"*"`;
    /// 5. a regra `"*"` de tabela na base `"*"`;
    /// 6. e so entao a regra da BASE (que por sua vez cai em `"*"` e no nivel).
    ///
    /// Substituir, e nao interceder, e o que permite os dois casos que a
    /// pratica pede: **tirar** uma tabela de quem le a base inteira, e **dar**
    /// uma tabela a quem nao le a base nenhuma.
    ///
    /// Tabela vazia -- operacao que nao fala de tabela, como `bancos` ou
    /// `criar_database` -- cai direto na regra da base.
    pub fn permissoes_em(&self, database: &str, tabela: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if !tabela.is_empty() {
            for base in [database, "*"] {
                if let Some((_, regras)) = self.tabelas.iter().find(|(b, _)| b == base) {
                    for alvo in [tabela, "*"] {
                        if let Some((_, p)) = regras.iter().find(|(t, _)| t == alvo) {
                            return *p;
                        }
                    }
                }
            }
        }
        self.permissoes(database)
    }

    /// Pode fazer a atividade nesta base?
    pub fn pode(&self, database: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes(database).pode(atividade)
    }

    /// Pode fazer a atividade nesta tabela desta base?
    ///
    /// Tabela vazia e o mesmo que perguntar so pela base.
    pub fn pode_em(&self, database: &str, tabela: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes_em(database, tabela).pode(atividade)
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# ---- parse
alvo = '''        let bases = match j.campo("bases") {
            Some(Json::Objeto(pares)) => pares
                .iter()
                .map(|(base, perm)| (base.clone(), Permissoes::de_json(perm)))
                .collect(),
            _ => Vec::new(),
        };'''
novo = '''        let bases = match j.campo("bases") {
            Some(Json::Objeto(pares)) => pares
                .iter()
                .map(|(base, perm)| (base.clone(), Permissoes::de_json(perm)))
                .collect(),
            _ => Vec::new(),
        };

        // `"tabelas"` sai de dentro do objeto da base. Base sem `"tabelas"`
        // nao entra aqui: uma lista vazia e uma lista ausente dariam na mesma
        // no lookup, e a ausente nao ocupa lugar.
        let mut tabelas: Vec<(String, Vec<(String, Permissoes)>)> = Vec::new();
        if let Some(Json::Objeto(pares)) = j.campo("bases") {
            for (base, perm) in pares {
                if let Some(Json::Objeto(porta)) = perm.campo("tabelas") {
                    if porta.is_empty() {
                        continue;
                    }
                    tabelas.push((
                        base.clone(),
                        porta
                            .iter()
                            .map(|(t, p)| (t.clone(), Permissoes::de_json(p)))
                            .collect(),
                    ));
                }
            }
        }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''            chave_publica,
            bases,
        })''','''            chave_publica,
            bases,
            tabelas,
        })''',1)

# ---- ficha
alvo = '''            (
                "bases",
                Json::Objeto(
                    self.bases
                        .iter()
                        .map(|(b, p)| (b.clone(), p.para_json()))
                        .collect(),
                ),
            ),
        ])'''
novo = '''            (
                "bases",
                Json::Objeto(
                    self.bases
                        .iter()
                        .map(|(b, p)| (b.clone(), p.para_json()))
                        .collect(),
                ),
            ),
            (
                "tabelas",
                Json::Objeto(
                    self.tabelas
                        .iter()
                        .map(|(b, regras)| {
                            (
                                b.clone(),
                                Json::Objeto(
                                    regras
                                        .iter()
                                        .map(|(t, p)| (t.clone(), p.para_json()))
                                        .collect(),
                                ),
                            )
                        })
                        .collect(),
                ),
            ),
        ])'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
