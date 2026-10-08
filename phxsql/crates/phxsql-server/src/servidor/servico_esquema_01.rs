//! Sequencias e DDL.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// A declaracao da chave com a trava SOLTA (pedido 422): a mae aberta antes
/// do congelamento -- quando a chave varre e a mae existe --, e a posse das
/// tabelas congeladas. Soltar a posse descongela, inclusive no `?` de um erro
/// no meio.
struct DeclaracaoSolta {
    mae: Option<Table>,
    _posse: (
        phxsql_store::congelamento::Congelada,
        Option<phxsql_store::congelamento::Congelada>,
    ),
}

impl Servidor {
    /// `sequences`: o contador de cada tabela do banco, num lugar so.
    ///
    /// # Onde o numero mora de verdade
    ///
    /// Cada tabela guarda o proprio contador no cabecalho do `.reg` dela, e
    /// **continua assim**. Esta operacao junta os contadores para mostrar; nao
    /// e um arquivo `sequences` com uma segunda copia.
    ///
    /// A razao e a mesma que impede gravar "e chave primaria" na coluna: uma
    /// segunda copia e uma segunda verdade, e as duas divergem no primeiro
    /// caminho que esquecer de atualizar uma delas. Alem disso um arquivo
    /// separado custaria uma leitura e uma gravacao a mais por insercao --
    /// justamente na operacao que ja e a mais cara.
    pub(super) fn op_sequencias(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            // A terceira porta para a MESMA lista que a arvore esconde. O
            // `tabelas`, o `sistabelas` e o `siscolunas` ja filtravam; esta
            // nao, e ela devolve nome da tabela, contador e quantas linhas --
            // que e o suficiente para saber que a folha existe, quantas
            // pessoas ela tem e quanto ela cresceu no mes.
            //
            // Ela varre a base inteira e NAO tem campo `tabela`, entao o
            // portao geral nao a alcanca: e o mesmo desenho do
            // `dados_pessoais`, que filtra tabela a tabela por dentro.
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };
            let e = t.esquema();
            let col = e.coluna_sequencia();
            linhas.push(Json::objeto(vec![
                ("tabela", Json::texto_de(&nome)),
                (
                    "coluna",
                    match col {
                        None => Json::Nulo,
                        Some(i) => Json::texto_de(&e.colunas()[i].nome),
                    },
                ),
                // Zero quer dizer "nunca usada": o primeiro numero sai 1.
                ("proxima", Json::de_u64(t.sequencia_atual())),
                ("registros", Json::de_u64(t.registros())),
                ("tem_sequencia", Json::Bool(col.is_some())),
            ]));
        }
        // As NOMEADAS (pedido 229) vem na mesma resposta, em lista propria:
        // nao sao tabela e nao tem coluna, e misturar as duas listas faria o
        // `total` de cima mentir sobre quantas tabelas existem.
        let mut nomeadas = Vec::new();
        for nome in db.todas_as_sequencias_nomeadas()? {
            let (schema, curto) = phxsql_store::catalogo::separar_qualificado(&nome);
            let Ok((s, _posse)) = db.abrir_sequencia(schema.as_deref(), &curto) else {
                continue;
            };
            let mut ficha = Self::ficha_da_sequencia(s.estado());
            ficha.insert(0, ("sequencia", Json::texto_de(&nome)));
            nomeadas.push(Json::objeto(ficha));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("sequencias", Json::Lista(linhas)),
            ("nomeadas", Json::Lista(nomeadas)),
        ]))
    }

    // ------------------------------------------------- sequencia nomeada
    //
    // O `.seq` do pedido 229 (`phxsql_store::sequencia`): um contador com
    // nome, fora de qualquer tabela, durado em disco a cada numero. Entrou
    // pela matriz dos quatro motores (`docs/AUTONUMBER.md` §C.5): PostgreSQL
    // (4) e MariaDB (3) tem `CREATE SEQUENCE`, MySQL (2) e SQLite (1) nao --
    // 7 x 3. So pelo protocolo: a gramatica SQL daqui nao tem `CREATE` de
    // objeto nenhum (nem `CREATE TABLE`), e o `NEXT VALUE FOR` dentro de
    // expressao e recusado na §B.2.4.

    /// `database`, `schema` e nome da sequencia nomeada no pedido. O campo e
    /// `sequencia`, qualificavel como a tabela (`filial.nf`).
    fn sequencia_do_pedido(p: &Json) -> Result<(String, Option<String>, String)> {
        let database = p.texto_ou("database", "").trim().to_string();
        let qualificado = p.texto_ou("sequencia", "").trim().to_string();
        if database.is_empty() || qualificado.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"sequencia\" (o nome, ou schema.nome)".into(),
            ));
        }
        let (schema, nome) = phxsql_store::catalogo::separar_qualificado(&qualificado);
        Ok((database, schema, nome))
    }

    /// O estado de uma sequencia nomeada como a resposta mostra. Os numeros
    /// saem pelo `valor_para_json` de uma `Int8`: acima de 2^53 viram texto,
    /// pela mesma regra de toda coluna inteira.
    fn ficha_da_sequencia(e: &phxsql_store::sequencia::Estado) -> Vec<(&'static str, Json)> {
        let n = |v: i64| crate::valores::valor_para_json(&Value::Int(v), &ColumnType::Int8);
        vec![
            ("proximo", n(e.proximo)),
            ("inicio", n(e.inicio)),
            ("passo", n(e.passo)),
            ("minimo", n(e.minimo)),
            ("maximo", n(e.maximo)),
            ("ciclo", Json::Bool(e.ciclo)),
            ("esgotada", Json::Bool(e.esgotada)),
            ("entregues", Json::de_u64(e.entregues)),
        ]
    }

    /// Cria uma sequencia nomeada. O `fsync` do arquivo e da pasta ficam
    /// fora da trava, como no `criar_schema` (pedido 589).
    pub(super) fn op_criar_sequencia(&self, p: &Json) -> Result<Json> {
        let (database, schema, nome) = Self::sequencia_do_pedido(p)?;
        let definicao = phxsql_store::sequencia::Definicao {
            inicio: crate::valores::inteiro_opcional(p, "inicio")?,
            passo: crate::valores::inteiro_opcional(p, "passo")?,
            minimo: crate::valores::inteiro_opcional(p, "minimo")?,
            maximo: crate::valores::inteiro_opcional(p, "maximo")?,
            ciclo: p.booleano_ou("ciclo", false),
        };
        let dados = self.travar_dados()?;
        let (s, pendente) = dados
            .abrir_database(&database)?
            .criar_sequencia_adiando_o_fsync(schema.as_deref(), &nome, definicao)?;
        let estado = *s.estado();
        drop(s);
        drop(dados);
        pendente.levar_ao_disco()?;
        let mut ficha = Self::ficha_da_sequencia(&estado);
        ficha.insert(
            0,
            (
                "sequencia",
                Json::texto_de(p.texto_ou("sequencia", "").trim()),
            ),
        );
        ficha.insert(0, ("database", Json::texto_de(&database)));
        Ok(Json::objeto(ficha))
    }

    /// O proximo numero de uma sequencia nomeada.
    ///
    /// # As duas travas, e a ordem delas
    ///
    /// Primeiro a [`Self::trava_das_sequencias`], depois a global -- e a
    /// global SOLTA antes do disco. A global so acha e abre o arquivo (nomes
    /// conferidos num lugar so, trava de instancia tomada); o `proximo`, que
    /// grava e espera o `fdatasync`, roda com a global ja devolvida. Abrir
    /// DENTRO da trava das sequencias e o que impede dois pedidos de lerem o
    /// mesmo estado e devolverem o mesmo numero. Quem cria ou apaga toma so a
    /// global, e por isso a ordem nunca se inverte.
    pub(super) fn op_proximo_da_sequencia(&self, p: &Json) -> Result<Json> {
        let (database, schema, nome) = Self::sequencia_do_pedido(p)?;
        let _fila = self
            .trava_das_sequencias
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (mut s, _posse) = {
            let dados = self.travar_dados()?;
            dados
                .abrir_database(&database)?
                .abrir_sequencia(schema.as_deref(), &nome)?
        };
        let valor = s.proximo()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&database)),
            (
                "sequencia",
                Json::texto_de(p.texto_ou("sequencia", "").trim()),
            ),
            (
                "valor",
                crate::valores::valor_para_json(&Value::Int(valor), &ColumnType::Int8),
            ),
        ]))
    }

    /// O estado de uma sequencia nomeada, sem mexer nela.
    pub(super) fn op_sequencia(&self, p: &Json) -> Result<Json> {
        let (database, schema, nome) = Self::sequencia_do_pedido(p)?;
        let dados = self.travar_dados()?;
        let (s, _posse) = dados
            .abrir_database(&database)?
            .abrir_sequencia(schema.as_deref(), &nome)?;
        let mut ficha = Self::ficha_da_sequencia(s.estado());
        ficha.insert(
            0,
            (
                "sequencia",
                Json::texto_de(p.texto_ou("sequencia", "").trim()),
            ),
        );
        ficha.insert(0, ("database", Json::texto_de(&database)));
        Ok(Json::objeto(ficha))
    }

    /// Apaga uma sequencia nomeada. Pede o nome repetido em `confirmar`, como
    /// o `excluir_tabela`: nao ha desfazer, e o proximo numero some junto.
    pub(super) fn op_excluir_sequencia(&self, p: &Json) -> Result<Json> {
        let (database, schema, nome) = Self::sequencia_do_pedido(p)?;
        let qualificado = p.texto_ou("sequencia", "").trim().to_string();
        if p.texto_ou("confirmar", "") != qualificado {
            return Err(PhxError::Esquema(format!(
                "para excluir, repita o nome da sequencia no campo \"confirmar\": \
                 esperado {qualificado:?}"
            )));
        }
        let dados = self.travar_dados()?;
        let pendente = dados
            .abrir_database(&database)?
            .excluir_sequencia_adiando_o_fsync(schema.as_deref(), &nome)?;
        drop(dados);
        pendente.levar_ao_disco()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&database)),
            ("sequencia", Json::texto_de(&qualificado)),
            ("excluida", Json::Bool(true)),
        ]))
    }

    /// Ajusta o contador de uma tabela -- zerar, ou pular uma faixa.
    ///
    /// Exige `administrar`: baixar o contador abaixo de um numero ja gravado
    /// faz a proxima insercao repetir, e o erro aparece longe de quem causou.
    /// Por isso a resposta diz o que era e o que passou a ser.
    pub(super) fn op_ajustar_sequencia(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // `"pelo_maior": true` -- o realinhamento do pedido 290 (parecer do
        // DBA, NAO 290-b): o alvo sai do DADO, entao nao ha «proxima» a
        // conferir. O braco dele mora DEPOIS da trava de baixo, e nao com uma
        // trava propria: uma secao a mais alcancando `fsync` sob a trava sobe
        // a catraca `alcancam-fsync-2` (`bancada/concorrencia/mapa-da-trava.py`).
        let pelo_maior = p.booleano_ou("pelo_maior", false);
        // A sequencia NOMEADA (pedido 229) entra pela mesma porta, com o
        // campo `sequencia` no lugar de `tabela`: e a mesma ordem de
        // administrador (o `setval` do PostgreSQL, o `ALTER SEQUENCE ...
        // RESTART` do MariaDB), e duas operacoes para a mesma decisao seriam
        // a decisao escrita duas vezes.
        if !p.texto_ou("sequencia", "").trim().is_empty() {
            return self.ajustar_sequencia_nomeada(p);
        }
        // Um "proxima" cru acima de 2^53 ja chegou arredondado -- ajustar o
        // contador para um valor que nao foi o pedido e o mesmo estrago do
        // bloco 19, so que no contador em vez de na linha. Recusa cedo, com a
        // saida (texto) na mensagem. Ver `docs/AUTONUMBER.md`, bloco 16 e 19.
        if !pelo_maior && p.campo("proxima").is_some_and(Json::inteiro_impreciso) {
            return Err(PhxError::Tipo(format!(
                "acima de {} o protocolo perde precisao num numero cru; \
                 envie \"proxima\" como texto se precisar dessa faixa",
                phxsql_core::json::INTEIRO_EXATO_MAX
            )));
        }
        // Aceita "proxima" como texto tambem, pelo mesmo motivo do id: e a unica
        // forma que atravessa acima do teto sem perda.
        let proxima = match p.campo("proxima") {
            _ if pelo_maior => 0,
            Some(Json::Texto(t)) => t.trim().parse::<i64>().unwrap_or(-1),
            _ => p.inteiro_ou("proxima", -1),
        };
        if proxima < 0 {
            return Err(PhxError::Esquema(
                "informe \"proxima\" com o numero que a sequencia deve dar em seguida \
                 (0 = zerar, e o primeiro sai 1)"
                    .into(),
            ));
        }
        let _trava = self.travar_dados()?;
        if pelo_maior {
            // A tabela gravada pelo contador defeituoso nao abre -- a
            // conferencia da faixa recusa --, e o `abrir_travada` de baixo
            // bateria nela: sem esta porta ela ficava sem saida.
            let database = p.texto_ou("database", "");
            let tabela = p.texto_ou("tabela", "");
            if database.is_empty() || tabela.is_empty() {
                return Err(PhxError::Esquema(
                    "informe \"database\" e \"tabela\"".into(),
                ));
            }
            let (antes, maior, depois) = _trava
                .abrir_database(database)?
                .realinhar_sequencia(tabela)?;
            return Ok(Json::objeto(vec![
                ("database", Json::texto_de(database)),
                ("tabela", Json::texto_de(tabela)),
                ("antes", Json::de_u64(antes)),
                ("maior", Json::de_u64(maior)),
                ("proxima", Json::de_u64(depois)),
            ]));
        }
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        if t.esquema().coluna_sequencia().is_none() {
            return Err(PhxError::Esquema(format!(
                "a tabela {} nao tem coluna Sequence",
                p.texto_ou("tabela", "")
            )));
        }
        let antes = t.sequencia_atual();
        t.ajustar_sequencia(proxima as u64)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("antes", Json::de_u64(antes)),
            ("proxima", Json::de_u64(proxima as u64)),
            (
                "aviso",
                Json::texto_de(if (proxima as u64) < antes {
                    "o contador andou para TRAS: se ja houver numero gravado nessa \
                     faixa, a proxima insercao repete e um indice unico recusa"
                } else {
                    ""
                }),
            ),
        ]))
    }

    /// O braco nomeado do [`Self::op_ajustar_sequencia`]: `proxima` dentro
    /// da faixa, inclusive para tras, e o `esgotada` cai. O `fdatasync` do
    /// ajuste roda fora da trava global, como o `proximo`.
    fn ajustar_sequencia_nomeada(&self, p: &Json) -> Result<Json> {
        let (database, schema, nome) = Self::sequencia_do_pedido(p)?;
        let Some(proxima) = crate::valores::inteiro_opcional(p, "proxima")? else {
            return Err(PhxError::Esquema(
                "informe \"proxima\" com o numero que a sequencia deve dar em seguida".into(),
            ));
        };
        let _fila = self
            .trava_das_sequencias
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (mut s, _posse) = {
            let dados = self.travar_dados()?;
            dados
                .abrir_database(&database)?
                .abrir_sequencia(schema.as_deref(), &nome)?
        };
        let antes = s.estado().proximo;
        s.ajustar(proxima)?;
        let mut ficha = Self::ficha_da_sequencia(s.estado());
        ficha.insert(0, ("antes", Json::de_i64(antes)));
        ficha.insert(
            0,
            (
                "sequencia",
                Json::texto_de(p.texto_ou("sequencia", "").trim()),
            ),
        );
        ficha.insert(0, ("database", Json::texto_de(&database)));
        if proxima < antes {
            ficha.push((
                "aviso",
                Json::texto_de(
                    "o contador andou para TRAS: numeros ja entregues nessa faixa vao sair \
                     de novo",
                ),
            ));
        }
        Ok(Json::objeto(ficha))
    }

    /// Cria um schema -- uma pasta dentro do database.
    ///
    /// Estava prometido em dois lugares (a tabela de permissoes e a lista de
    /// operacoes de escrita) e nao existia no despacho: pedir `criar_schema`
    /// pela rede respondia "operacao desconhecida". A biblioteca ja sabia
    /// fazer; faltava a porta.
    pub(super) fn op_criar_schema(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let schema = p.texto_ou("schema", "").trim();
        if schema.is_empty() {
            return Err(PhxError::Esquema("informe \"schema\"".into()));
        }
        let dados = self.travar_dados()?;
        let (_, pendente) = dados
            .abrir_database(database)?
            .criar_schema_adiando_o_fsync(schema)?;
        // Pedido 589: o `fsync` do database, onde mora a entrada da pasta
        // nova, fora da trava e antes da resposta.
        drop(dados);
        pendente.levar_ao_disco()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("schema", Json::texto_de(schema)),
        ]))
    }

    /// Cria uma tabela. Fecha o buraco que estava aberto desde a revisao: a
    /// paginacao existia, o `criar_tabela` existia na biblioteca, e nao havia
    /// caminho pela rede -- so escrevendo Rust.
    pub(super) fn op_criar_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let esquema = crate::valores::esquema_de_json(p)?;
        // Pedido 175: a chave conferida sem indice na filha tirava da MAE o
        // `excluir` inteiro, e o exemplo do nosso proprio `MANUAL.txt` caia
        // nisso. O indice nasce aqui, pela mesma regra da `declarar_fk`, e a
        // resposta o diz -- estrutura que nasce sem ser dita mente.
        let criados = esquema.indices_que_as_chaves_pedem();
        let indices_criados: Vec<String> = criados.iter().map(|i| i.nome.clone()).collect();
        let mut esquema = esquema.com_indices(criados)?;

        // `filial.clientes` e o schema `filial` mais a tabela `clientes`, e
        // nao uma tabela chamada "filial.clientes".
        //
        // Toda leitura ja separava assim -- `abrir_qualificada` faz isso desde
        // sempre. So a CRIACAO nao fazia, e o resultado eram cinco arquivos
        // chamados `filial.clientes.reg` na raiz do banco, que nenhuma outra
        // operacao conseguia abrir: a tabela nascia inalcancavel, e o servidor
        // respondia "criada".
        let (do_nome, nome) = phxsql_store::catalogo::separar_qualificado(esquema.nome());
        let dito = p.texto_ou("schema", "").trim().to_string();
        let schema = match (do_nome.as_deref(), dito.as_str()) {
            (Some(a), b) if !b.is_empty() && a != b => {
                return Err(PhxError::Esquema(format!(
                    "o nome diz schema {a:?} e o campo \"schema\" diz {b:?}:                      escolha um dos dois"
                )))
            }
            (Some(a), _) => Some(a.to_string()),
            (None, "") => None,
            (None, b) => Some(b.to_string()),
        };
        if do_nome.is_some() {
            esquema.renomear(&nome);
        }

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        if db.existe_tabela(schema.as_deref(), &nome)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {} ja existe em {database}",
                phxsql_store::catalogo::qualificar(schema.as_deref(), &nome)
            )));
        }
        let (t, pendente) = db.criar_tabela_adiando_o_fsync(schema.as_deref(), esquema)?;
        // Pedido 589: os arquivos da tabela, a pasta dela e -- se o schema
        // nasceu aqui -- o database vao ao disco FORA da trava, e antes da
        // resposta. A tabela fecha antes da trava soltar, como sempre fechou:
        // o `Drop` dela mexe no `.ndx`, e isso e trabalho de quem segura.
        let esquema_criado = t.esquema().clone();
        drop(t);
        drop(dados);
        // A janela do pedido 605: a tabela existe, a trava saiu, o `fsync`
        // ainda nao -- e e aqui que o teste poe o terceiro.
        #[cfg(test)]
        {
            let gancho = self
                .na_janela_da_criacao
                .lock()
                .ok()
                .and_then(|mut g| g.take());
            if let Some(g) = gancho {
                g();
            }
        }
        pendente.levar_ao_disco()?;
        let qualificado = phxsql_store::catalogo::qualificar(schema.as_deref(), &nome);
        // A regra de coluna que ja ESPERAVA por esta tabela -- pedido 235. O
        // cadastro a aceitou com aviso porque a tabela nao existia; se ela
        // nasce sem a coluna citada, a regra e inerte, e este e o unico
        // momento em que alguem esta olhando. Nao recusa: a tabela e a
        // modelagem certa e o cadastro e o que esta errado -- e o cadastro se
        // conserta pelo `usuario_alterar`, que agora recusa a mesma coluna.
        let avisos = self.regras_de_coluna_inertes(database, &qualificado, &esquema_criado);
        let mut pares = vec![
            ("database", Json::texto_de(database)),
            (
                "schema",
                match &schema {
                    Some(s) => Json::texto_de(s),
                    None => Json::Nulo,
                },
            ),
            ("tabela", Json::texto_de(qualificado)),
            (
                "colunas",
                Json::de_u64(esquema_criado.colunas().len() as u64),
            ),
            (
                "indices",
                Json::de_u64(esquema_criado.indices().len() as u64),
            ),
            (
                "paginada",
                Json::Bool(esquema_criado.paginacao().registros_por_arquivo > 0),
            ),
        ];
        // So quando ha, como os avisos: a resposta de sempre nao muda.
        if !indices_criados.is_empty() {
            pares.push((
                "indices_criados",
                Json::Lista(indices_criados.iter().map(Json::texto_de).collect()),
            ));
        }
        // So quando ha: a resposta de sempre nao ganha campo vazio.
        if !avisos.is_empty() {
            for aviso in &avisos {
                eprintln!("AVISO: {aviso}");
            }
            pares.push((
                "avisos",
                Json::Lista(avisos.iter().map(Json::texto_de).collect()),
            ));
        }
        Ok(Json::objeto(pares))
    }

    /// As regras de coluna do cadastro vivo que citam, em `base.tabela`,
    /// coluna que este esquema nao tem -- uma mensagem por regra inerte.
    ///
    /// Casa base e tabela por nome EXATO, e nao pela precedencia de
    /// `regras_de_coluna`: o curinga `"*"` nao nomeia esta tabela, e uma
    /// regra `"*"` citando `salario` e legitima em toda tabela que o tenha.
    /// Custa uma passada pelas listas de coluna do cadastro, que sao vazias
    /// para quem nunca escreveu `"colunas"`.
    fn regras_de_coluna_inertes(&self, base: &str, tabela: &str, esquema: &Schema) -> Vec<String> {
        let cadastro = self.cadastro();
        let mut avisos = Vec::new();
        for u in cadastro.root.iter().chain(cadastro.usuarios.iter()) {
            let Some((_, tabelas)) = u.colunas.iter().find(|(b, _)| b == base) else {
                continue;
            };
            let Some((_, regras)) = tabelas.iter().find(|(t, _)| t == tabela) else {
                continue;
            };
            for (coluna, _) in regras {
                let existe = esquema
                    .colunas()
                    .iter()
                    .any(|c| crate::direito_coluna::mesmo_nome(&c.nome, coluna));
                if !existe {
                    avisos.push(format!(
                        "usuario {}: a regra de coluna em {base}.{tabela} cita {coluna:?}, e a \
                         tabela que acabou de nascer nao tem essa coluna -- a regra esta \
                         INERTE. As colunas sao: {}",
                        u.login,
                        esquema
                            .colunas()
                            .iter()
                            .map(|c| c.nome.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
        avisos
    }

    /// Declara uma chave estrangeira numa tabela QUE JA EXISTE.
    ///
    /// E o que o editor do diagrama ER chama quando alguem puxa uma coluna
    /// ate a coluna de outra tabela. Ate aqui a chave so entrava junto com o
    /// `criar_tabela` -- e o diagrama liga tabelas que ja nasceram.
    ///
    /// A chave nasce CONFERIDA (decisao do dono), e desde o pedido 175 o
    /// indice que ela pede na FILHA nasce junto quando falta -- e a resposta
    /// o nomeia em `indices_criados`. A operacao pede o poder de CRIAR, como
    /// o `criar_tabela` que sempre pode declara-la, e nao mais.
    pub(super) fn op_declarar_fk(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // `tabela_ref` e obrigatoria AQUI, embora o leitor da chave aceite
        // `tabela` como apelido dela: neste pedido o campo `tabela` e a
        // tabela que RECEBE a declaracao, e o apelido a transformaria em uma
        // referencia a si mesma sem ninguem ter pedido.
        if p.texto_ou("tabela_ref", "").trim().is_empty() {
            return Err(PhxError::Esquema(
                "informe \"tabela_ref\" (a tabela referenciada); \
                 \"tabela\" e a que recebe a chave"
                    .into(),
            ));
        }
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let nova = crate::valores::chave_estrangeira_de_json(p, 0, t.esquema())?;
        let fk_nova = nova.clone();
        let mut fks = t.esquema().chaves_estrangeiras().to_vec();
        if fks.iter().any(|f| f.nome == nova.nome) {
            return Err(PhxError::Duplicado(format!(
                "a chave {} ja esta declarada em {}; exclua-a antes de redeclarar",
                nova.nome,
                t.nome()
            )));
        }
        let nome = nova.nome.clone();
        // Guarda o mesmo valor que o `esquema` vai devolver depois -- e o
        // ponto do conserto do pedido 227: havia um literal `false` aqui,
        // sobrevivente de antes de "chave declarada nasce conferida", e a
        // tela recebia duas respostas diferentes sobre a MESMA chave (esta
        // resposta dizia nao-imposta, o `esquema` dizia `verificar:true`).
        let imposta = nova.verificar;
        fks.push(nova);
        // Pedido 422: a varredura roda com a trava SOLTA, e as tabelas
        // congeladas -- ver `preparar_a_declaracao_solta`. O esquema se grava
        // num ponto so, na trava retomada: e a mesma forma para a chave que
        // varre e para a que nao varre.
        let mut solta = self.preparar_a_declaracao_solta(&dados, p, &t, &fk_nova)?;
        drop(dados);
        let recibo = t.conferir_chaves_que_nascem(&fks, solta.mae.as_mut())?;
        solta.mae = None;
        // E a reescrita do `.reg`, quando o bloco de esquema nao cabe antes do
        // slot 1: a FASE A tambem aqui fora, e so os `rename` la dentro.
        let troca = t.preparar_chaves_estrangeiras(&fks)?;
        // Em bloco, e nao solto: o mapa da trava le `#[cfg(test)]` ate a
        // chave seguinte, e solto ele engolia a secao da trava retomada.
        #[cfg(test)]
        {
            self.rodar_gancho_da_janela();
        }
        let dados = self.travar_dados()?;
        let reescreveu = t.redeclarar_depois_de_conferir(fks, recibo, troca)?;
        // Pedido 175: o indice da FILHA nasce com a chave, pela mesma regra
        // do `criar_tabela` -- so para a chave que esta nascendo, e so se ela
        // confere. Ainda congelada e sob a trava: o `.ndx` se refaz aqui, e
        // ninguem pode gravar na filha entre o esquema novo e a arvore nova.
        let novo_indice = t.esquema().indice_que_a_chave_pede(&fk_nova);
        let indices_criados = t.acrescentar_indices(novo_indice.into_iter().collect())?;
        if !indices_criados.is_empty() {
            self.gravar_de_verdade(&dados, &mut t, p)?;
        }
        // O congelamento sai antes da trava: quem estava esperando por ela
        // encontra as tabelas ja abertas.
        drop(solta);
        drop(dados);
        let mut pares = vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("nome", Json::texto_de(nome)),
            (
                "chaves_estrangeiras",
                Json::de_u64(t.esquema().chaves_estrangeiras().len() as u64),
            ),
            // A chave e DECLARADA, e "imposta" aqui e o MESMO `verificar` que
            // o esquema grava e devolve -- nunca um literal a parte.
            ("imposta", Json::Bool(imposta)),
            ("arquivos_reescritos", Json::Bool(reescreveu)),
        ];
        // So quando ha, como no `criar_tabela`: a resposta de sempre nao muda
        // para quem declara a chave com o indice ja la.
        if !indices_criados.is_empty() {
            pares.push((
                "indices_criados",
                Json::Lista(indices_criados.iter().map(Json::texto_de).collect()),
            ));
        }
        Ok(Json::objeto(pares))
    }

    /// **Pedido 422: a varredura da chave que nasce conferida roda FORA da
    /// trava global**, com a filha e a mae congeladas.
    ///
    /// A varredura (`conferir_chave`) le a filha inteira e busca cada linha na
    /// mae: O(linhas da filha), e com a trava global na mao era o servidor
    /// inteiro parado por ela. O padrao e o da `acrescentar_coluna`: travar,
    /// congelar, soltar, trabalhar, retravar, gravar. O que muda e o ALCANCE
    /// do congelamento -- as DUAS tabelas, e esse e o «segundo escopo» que o
    /// parecer pedia: com a filha congelada nenhuma linha nova escapa da
    /// varredura; com a mae congelada nenhum pai sai de baixo de uma filha ja
    /// conferida. Congelar so a filha compilaria, passaria na suite e
    /// deixaria o `excluir` da mae abrir uma orfa dentro de uma chave
    /// declarada conferida -- que e exatamente o «nunca» da regra primordial.
    ///
    /// A mae e aberta ANTES do congelamento: depois dele, o `Table::abrir`
    /// dela seria recusado pelo proprio congelamento que protege a varredura.
    ///
    /// # Uma forma so, varra ou nao
    ///
    /// Toda declaracao congela, solta, confere, retoma e grava -- inclusive a
    /// que nao varre (chave sem `verificar`, ja conferida, ou mae que ainda
    /// nao existe), que so congela a filha e confere nada. Dois caminhos, um
    /// sob a trava original e outro sob a retomada, seriam dois pontos
    /// segurando a trava ate o `fsync` do cabecalho; um so e o que acontece
    /// de fato, e e o que o mapa da trava consegue ver.
    ///
    /// # Transacao viva na vizinhanca: quem cede e a declaracao
    ///
    /// Congelar faria o `COMMIT` de uma transacao que alcanca a filha ou a
    /// mae bater no congelamento (pedido 426). A declaracao recusa na hora com
    /// `EM_TRANSACAO` e nada gravado, como a `acrescentar_coluna`: os quatro
    /// motores convergem em que quem paga e o DDL, nunca a transacao
    /// (`docs/propostas/commit-contra-ddl-4-motores.md`, D5).
    ///
    /// A regravacao do esquema -- inclusive a condicional, quando o bloco nao
    /// cabe -- continua sob a trava retomada.
    /// So nos testes: roda o [`Servidor::na_janela_sem_trava`], uma vez.
    #[cfg(test)]
    pub(super) fn rodar_gancho_da_janela(&self) {
        let gancho = self
            .na_janela_sem_trava
            .lock()
            .ok()
            .and_then(|mut g| g.take());
        if let Some(g) = gancho {
            g();
        }
    }

    fn preparar_a_declaracao_solta(
        &self,
        dados: &Instancia,
        p: &Json,
        t: &Table,
        nova: &phxsql_core::schema::ForeignKey,
    ) -> Result<DeclaracaoSolta> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if let Some(recado) = self.transacao_na_vizinhanca(dados, database, tabela, t)? {
            return Err(PhxError::EmTransacao(recado));
        }
        // A mae so importa para a chave que varre, e so se ela ja existe: a
        // que falta e ordem legitima de modelagem, e a conferencia diz isso.
        let mae = if t.pede_varredura(nova) {
            let nome_mae = phxsql_store::table::nome_simples(&nova.tabela_ref);
            match Table::abrir(t.diretorio(), nome_mae) {
                Ok(m) => Some(m),
                Err(PhxError::NaoEncontrado(_)) => None,
                Err(e) => return Err(e),
            }
        } else {
            None
        };
        if let Some(m) = &mae {
            if let Some(recado) = self.transacao_na_vizinhanca(dados, database, tabela, m)? {
                return Err(PhxError::EmTransacao(recado));
            }
        }
        let motivo = format!("declarando a chave {}", nova.nome);
        let filha = phxsql_store::congelamento::congelar(t.diretorio(), t.nome(), motivo.clone())?;
        // Autorreferencia: a mae E a filha, e congelar duas vezes a mesma
        // tabela e o que o registro recusa -- uma posse basta. «A mesma» e
        // pela CHAVE do registro, e nao por uma segunda regra de nome escrita
        // aqui: o `eq_ignore_ascii_case` de antes e a chave em minusculas
        // divergiam fora do ASCII (`Ação`/`ação`), e a declaracao recusava a
        // si mesma ao congelar duas vezes a mesma chave (pedido 428).
        let mesma = |m: &phxsql_store::Table| {
            phxsql_store::congelamento::chave(m.diretorio(), m.nome())
                == phxsql_store::congelamento::chave(t.diretorio(), t.nome())
        };
        let da_mae = match &mae {
            Some(m) if !mesma(m) => Some(phxsql_store::congelamento::congelar(
                m.diretorio(),
                m.nome(),
                motivo,
            )?),
            _ => None,
        };
        Ok(DeclaracaoSolta {
            mae,
            _posse: (filha, da_mae),
        })
    }

    /// Acrescenta uma coluna a uma tabela que ja tem dado.
    ///
    /// # O que ele reescreve, e o que nao toca
    ///
    /// O `.reg` inteiro -- e o `.bkp`, quando ha espelho --, porque o slot fica
    /// mais largo e o endereco de cada linha sai de
    /// `data_offset + (rowid-1) * slot_size`. O `.ndx` NAO e tocado: ele aponta
    /// para rowid, e o rowid nao muda. O `.log`, o `.trash`, o `.reason` e o
    /// `.lgpd` tambem nao: eles guardam o passado, e o passado nao ganha coluna.
    ///
    /// Medido em `--example custo-do-alter`: 0,55 us por linha, 10 milhoes de
    /// linhas em 5,5 s. A resposta traz `slots` e `ms` para quem chamou poder
    /// dizer isso na tela em vez de estimar.
    pub(super) fn op_acrescentar_coluna(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // A coluna chega como um objeto `coluna`, ou solta nos campos do
        // proprio pedido -- que e como a tela mais curta a manda. So o objeto
        // `coluna` proprio e puro; o pedido solto carrega `op`/`token`/
        // `database`/`tabela`/`default`, entao a conferencia de chave
        // desconhecida (O1) SO vale para o objeto proprio, senao ela acusaria
        // os campos de protocolo e quebraria a tela curta que ja existe.
        let (corpo, estrito) = match p.campo("coluna") {
            Some(sub) => (sub, true),
            None => (p, false),
        };
        let coluna = crate::valores::coluna_de_json(corpo, 0, estrito)?;

        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;

        // O valor que a linha VELHA recebe.
        //
        // Ate o pedido 475 este campo relia o JSON cru de "padrao" pelo
        // mesmo caminho de um `inserir` (`json_para_valor`) -- e por isso
        // "abc" numa coluna Int8 e recusado aqui, e nao gravado em dez mil
        // linhas. Mas `coluna.padrao`, alguns passos acima
        // (`coluna_de_json`), JA leu o MESMO campo "padrao" como EXPRESSAO
        // (MANUAL.txt:398): um texto vira DEFAULT so entre aspas simples
        // (`'ativo'`), e reler o JSON cru aqui devolvia o texto com as
        // aspas dentro -- a linha velha ganhava literalmente `'ativo'`, nao
        // `ativo`. A linha velha tem de receber o MESMO valor que o padrao
        // promete as linhas novas, entao avalia-se a expressao ja validada,
        // e nao o JSON de novo. Sem resolvedor de coluna: um valor unico
        // para todo o backfill nao pode variar linha a linha, entao um
        // padrao que citasse outra coluna erraria aqui do mesmo jeito que
        // errava antes (guardando texto cru sem sentido).
        //
        // `"default"` continua servindo de escape: quem manda SO ele (sem
        // "padrao") pede um valor CRU, do jeito de sempre -- e e para isso
        // que ele existe, ja que `coluna_de_json` nunca o le.
        let padrao = if let Some(expr) = &coluna.padrao {
            let v = expr
                .avaliar(&|_| None)
                .map_err(|e| PhxError::Esquema(format!("padrao de {}: {e}", coluna.nome)))?;
            Some(phxsql_core::expressao::coagir(&v, &coluna.ty)?)
        } else {
            match p.campo("default") {
                None | Some(Json::Nulo) => None,
                Some(j) => Some(crate::valores::json_para_valor_da_coluna(j, &coluna)?),
            }
        };

        // Pedido 426: a transacao viva que alcanca esta tabela segura a
        // reescrita -- a pergunta e feita AQUI, com a trava global na mao, e
        // nao so no portao de fora. Ver `transacao_na_vizinhanca`.
        if let Some(recado) = self.transacao_na_vizinhanca(
            &dados,
            p.texto_ou("database", ""),
            p.texto_ou("tabela", ""),
            &t,
        )? {
            return Err(PhxError::EmTransacao(recado));
        }

        let inicio = std::time::Instant::now();
        // O CONGELAMENTO entra com a trava na mao, e e isso que faz o retrato
        // da FASE A ser tirado num ponto QUIETO: so se abre tabela gravavel
        // com a ficha exclusiva, entao com a trava na mao nao ha escritor em
        // voo nesta tabela. Dela em diante, quem tentar gravar ouve a recusa
        // nomeada em vez de entrar num arquivo que vai ser renomeado por cima.
        let congelada = phxsql_store::congelamento::congelar(
            t.diretorio(),
            t.nome(),
            format!("acrescentando a coluna {}", coluna.nome),
        )?;
        // A trava global SAI aqui. O que ela protegia -- o volume vivo contra
        // escrita concorrente durante a FASE A -- passou para o congelamento,
        // que custa um `load` atomico a quem nao esta migrando nada.
        drop(dados);

        // FASE A, FORA da trava: e a parte cara (0,69-1,03 us por slot,
        // medido em 23/09/2026), e o servidor atende todo o resto enquanto
        // ela corre.
        //
        // A recusa do CHECK que linha velha viola (245, O2a) e do motor -- a
        // decisao e a contagem moram no `Table` --, e daqui sai so a FRASE,
        // pela fabrica, como toda mensagem que este servidor devolve.
        let pendente = t.acrescentar_coluna_fase_a_recusando(coluna.clone(), padrao, |v| {
            PhxError::Esquema(self.msg(
                "erro.check_novo_violado",
                &[
                    ("coluna", &v.coluna),
                    ("check", &v.check),
                    ("violam", &v.violam.to_string()),
                    ("linhas", &v.linhas.to_string()),
                ],
            ))
        })?;

        let dados = self.travar_dados()?;
        // A REVALIDACAO, e ela nao e cerimonia: se um caminho novo escapar do
        // congelamento amanha, a troca ABORTA em vez de renomear por cima de
        // escrita confirmada. Ver `TrocaPendente::conferir_retrato`.
        if let Err(e) = pendente.conferir_retrato() {
            pendente.descartar();
            return Err(e);
        }
        let slots = t.acrescentar_coluna_fase_b(pendente)?;
        // Depois da FASE B, como sempre foi: acrescentar coluna nao muda o
        // numero de registros, mas ler o contador antes da troca seria uma
        // segunda verdade esperando divergir.
        let registros = t.registros();
        drop(dados);
        drop(congelada);
        // Pedido 647: o inode velho morre AQUI, fora da trava global e do
        // congelamento -- ninguem espera por ele.
        t.soltar_volumes_velhos();
        let ms = inicio.elapsed().as_secs_f64() * 1e3;

        let pares = vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("coluna", Json::texto_de(coluna.nome.clone())),
            (
                "posicao",
                Json::de_u64(t.esquema().coluna_por_nome(&coluna.nome).unwrap_or(0) as u64),
            ),
            ("colunas", Json::de_u64(t.esquema().colunas().len() as u64)),
            ("slots_reescritos", Json::de_u64(slots)),
            ("registros", Json::de_u64(registros)),
            ("ms", Json::Numero(ms)),
            // Quem chamou precisa poder dizer a verdade na tela: o indice nao
            // foi refeito porque nao precisou.
            ("indices_refeitos", Json::Bool(false)),
        ];
        // Sem `avisos`: o CHECK e conferido contra a linha velha (245, O2a)
        // e a calculada e preenchida nela (245, O2b) -- nao sobrou o que a
        // linha velha deixe de ganhar, e aviso que diz o contrario mente.
        Ok(Json::objeto(pares))
    }

    /// A **porta** do `PSCH` v10: leva ao formato atual uma tabela nascida
    /// antes dele, e diz o preco ANTES de cobrar.
    ///
    /// # Por que uma operacao do protocolo, e nao um passo de arranque
    ///
    /// Porque migrar reescreve o `.reg` INTEIRO uma vez por coluna que falta,
    /// e isso e uma janela de parada -- coisa que se marca. Um passo de
    /// arranque so teria duas formas, e as duas sao piores: migrar sozinho
    /// (o servidor que sobe de madrugada e volta horas depois sem ninguem ter
    /// pedido) ou imprimir um aviso no meio do log, que ninguem le. Migrar e
    /// administracao, e administracao tem hora e tem dono.
    ///
    /// # Tres caminhos, e os dois primeiros nao tocam em disco
    ///
    /// - **sem `tabela`**: varre a base e diz QUAIS faltam e quanto cada uma
    ///   custa. Sem isso a porta existiria e ninguem saberia em que bater.
    /// - **com `tabela`, sem `confirmar`**: a vista previa desta tabela.
    /// - **com `confirmar` igual ao nome da tabela**: migra. O nome repetido e
    ///   o mesmo pedagio do `excluir_tabela`, e pelo mesmo motivo -- o que vai
    ///   acontecer nao tem desfazer barato.
    ///
    /// # O portao, e o campo que ele le
    ///
    /// `Atividade::Administrar` pelo portao geral, sobre o campo `tabela`. A
    /// VARREDURA e uma das que escondem tabela dele -- ela nao tem o campo --,
    /// entao ela paga conferencia propria tabela a tabela, como o
    /// `dados_pessoais` e o `sequencias` ao lado. Sem ela, quem so administra
    /// uma tabela leria o nome e o tamanho de todas as outras.
    pub(super) fn op_migrar_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").to_string();
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if tabela.is_empty() {
            return self.migrar_esquema_varredura(&database, sessao);
        }

        let confirmar = p.texto_ou("confirmar", "").trim().to_string();
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        // O plano vem ANTES de qualquer decisao: e ele que recusa a
        // tabela-cadeia, e recusar depois de confirmar seria mandar o
        // administrador marcar uma parada que nunca ia acontecer.
        let plano = t.plano_do_psch_v10()?;
        let faltando: Vec<Json> = plano.colunas().iter().map(|c| Json::texto_de(*c)).collect();
        let passadas = plano.passadas() as u64;
        // O que se paga e slot, e nao registro: o `.reg` nunca reaproveita
        // slot excluido, entao a reescrita passa por todos eles -- e passa uma
        // vez por coluna que falta.
        let a_reescrever = plano.slots() * passadas;
        let mut pares = vec![
            ("database", Json::texto_de(&database)),
            ("tabela", Json::texto_de(&tabela)),
            ("precisa", Json::de_bool(plano.pendente())),
            ("colunas_faltando", Json::Lista(faltando)),
            ("passadas", Json::de_u64(passadas)),
            ("registros", Json::de_u64(plano.registros())),
            ("slots", Json::de_u64(plano.slots())),
            ("slots_a_reescrever", Json::de_u64(a_reescrever)),
        ];

        if !plano.pendente() {
            // Idempotente pela porta, como e' por dentro: quem chama de novo
            // recebe «nao ha o que fazer», e nao um erro que um script tenta
            // tratar para sempre.
            pares.push(("migrado", Json::de_bool(false)));
            pares.push((
                "nota",
                Json::texto_de(format!(
                    "a tabela {tabela} ja esta no formato de esquema atual"
                )),
            ));
            return Ok(Json::objeto(pares));
        }

        if confirmar.is_empty() {
            // A VISTA PREVIA, e ela e o motivo de a operacao existir em dois
            // tempos: o numero de linhas que vao ser reescritas tem de passar
            // pelos olhos de quem marca a parada antes de a parada comecar.
            pares.push(("migrado", Json::de_bool(false)));
            pares.push((
                "aviso",
                Json::texto_de(format!(
                    "migrar reescreve o arquivo de dados de {tabela} INTEIRO \
                     {passadas} vez(es) -- {a_reescrever} slot(s) ao todo -- e a \
                     tabela fica travada durante a reescrita. Nada foi feito \
                     agora: isto e o custo, nao a migracao"
                )),
            ));
            pares.push((
                "para_migrar",
                Json::texto_de(format!(
                    "{{\"op\":\"migrar_esquema\",\"database\":\"{database}\",\
                     \"tabela\":\"{tabela}\",\"confirmar\":\"{tabela}\"}}"
                )),
            ));
            return Ok(Json::objeto(pares));
        }
        if confirmar != tabela {
            return Err(PhxError::Esquema(format!(
                "para migrar, repita o nome da tabela no campo \"confirmar\": \
                 esperado {tabela:?}"
            )));
        }

        // Pedido 426, o IRMAO do `op_acrescentar_coluna`: mesmas funcoes na
        // mesma ordem (`travar_dados` -> `abrir_travada` -> `congelar` -> solta
        // a trava), entao a mesma pergunta, no mesmo ponto.
        if let Some(recado) = self.transacao_na_vizinhanca(&dados, &database, &tabela, &t)? {
            return Err(PhxError::EmTransacao(recado));
        }

        let inicio = std::time::Instant::now();
        // O CONGELAMENTO entra com a trava na mao -- ver o comentario gemeo em
        // `op_acrescentar_coluna`, e o cabecalho de
        // `phxsql_store::congelamento` para o porque de ele morar no armazem
        // e nao aqui.
        //
        // O registro `sujas` NAO precisa ser descarregado antes: o proprio
        // congelamento resolve o achado do papel C (§7 do parecer). O fecho da
        // janela de escrita (`descarregar_sujas_com`) reabre cada tabela suja
        // para sincronizar, e a reabertura desta passa a ser RECUSADA -- entao
        // ela fica na lista, as marcas pendentes ficam penduradas, e a proxima
        // passada as alcanca depois da troca. E o lado seguro que aquela
        // funcao ja escolhia para a tabela que nao abre.
        let congelada = phxsql_store::congelamento::congelar(
            t.diretorio(),
            t.nome(),
            format!("migracao para o PSCH v10 ({passadas} passada(s))"),
        )?;
        drop(dados);

        // Uma passada por coluna que falta: a FASE A de cada uma corre FORA da
        // trava, e so a FASE B (os `rename`) a retoma. Entre duas passadas o
        // disco esta num estado valido -- com a primeira coluna e sem a
        // segunda --, que e o que o `plano_do_psch_v10` da volta seguinte
        // reconhece sozinho.
        let mut slots = 0u64;
        loop {
            let Some(pendente) = t.migrar_v10_fase_a()? else {
                break;
            };
            let dados = self.travar_dados()?;
            if let Err(e) = pendente.conferir_retrato() {
                pendente.descartar();
                return Err(e);
            }
            slots = t.migrar_v10_fase_b(pendente)?;
            drop(dados);
            // Pedido 647, o IRMAO das outras FASES B: o inode velho morre fora
            // da trava -- e a CADA passada, senao o disco seguraria uma copia
            // morta da tabela por coluna migrada ate o fim do laco.
            t.soltar_volumes_velhos();
        }
        drop(congelada);
        let ms = inicio.elapsed().as_secs_f64() * 1e3;
        pares.push(("migrado", Json::de_bool(true)));
        pares.push(("slots_reescritos", Json::de_u64(slots)));
        pares.push(("ms", Json::Numero(ms)));
        // As linhas que ja existiam ficam com ZERO nas colunas novas, e quem
        // migrou precisa poder dizer isso na tela: zero quer dizer «nasceu
        // antes de a coluna existir», e nao «nasceu na epoca zero».
        pares.push((
            "nota",
            Json::texto_de(format!(
                "as {} linha(s) que ja existiam ficaram com ZERO nas colunas \
                 novas -- zero quer dizer «esta linha nasceu antes de a coluna \
                 existir». So as linhas gravadas daqui em diante nascem \
                 carimbadas",
                plano.registros()
            )),
        ));
        Ok(Json::objeto(pares))
    }

    /// A varredura da base: quais tabelas ainda nao estao no v10, e quanto
    /// custa cada uma. **Nao migra nada.**
    ///
    /// Uma tabela que nao abre, ou que RECUSA a migracao (a tabela-cadeia com
    /// historia assinada), entra na lista dizendo isso -- e nao derruba o
    /// relatorio inteiro. Levantamento que para no primeiro caso estranho nao
    /// levanta nada, e e o mesmo desenho do `dados_pessoais`.
    fn migrar_esquema_varredura(&self, database: &str, sessao: &Sessao) -> Result<Json> {
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        let mut pendentes = 0u64;
        let mut no_formato_atual = 0u64;
        let mut total_a_reescrever = 0u64;

        for nome in db.todas_as_tabelas()? {
            // A conferencia PROPRIA: este pedido nao tem campo `tabela`, entao
            // o portao geral so conferiu a base.
            if !self.pode_administrar_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                linhas.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("conferida", Json::de_bool(false)),
                ]));
                continue;
            };
            match t.plano_do_psch_v10() {
                Err(e) => linhas.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("precisa", Json::de_bool(true)),
                    ("recusa", Json::texto_de(e.to_string())),
                ])),
                Ok(plano) if !plano.pendente() => {
                    no_formato_atual += 1;
                }
                Ok(plano) => {
                    pendentes += 1;
                    let a_reescrever = plano.slots() * plano.passadas() as u64;
                    total_a_reescrever += a_reescrever;
                    linhas.push(Json::objeto(vec![
                        ("tabela", Json::texto_de(&nome)),
                        ("precisa", Json::de_bool(true)),
                        (
                            "colunas_faltando",
                            Json::Lista(
                                plano.colunas().iter().map(|c| Json::texto_de(*c)).collect(),
                            ),
                        ),
                        ("passadas", Json::de_u64(plano.passadas() as u64)),
                        ("registros", Json::de_u64(plano.registros())),
                        ("slots", Json::de_u64(plano.slots())),
                        ("slots_a_reescrever", Json::de_u64(a_reescrever)),
                    ]));
                }
            }
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("pendentes", Json::de_u64(pendentes)),
            ("no_formato_atual", Json::de_u64(no_formato_atual)),
            ("slots_a_reescrever", Json::de_u64(total_a_reescrever)),
            ("tabelas", Json::Lista(linhas)),
        ]))
    }

    /// Quem esta na sessao pode ADMINISTRAR esta tabela desta base?
    ///
    /// O irmao do `pode_ver_tabela`, com a atividade que a migracao pede. Sem
    /// sessao -- servidor sem cadastro -- e sim, pelo mesmo motivo dele: o
    /// portao de usuario nao existe naquele modo.
    fn pode_administrar_tabela(&self, sessao: &Sessao, database: &str, tabela: &str) -> bool {
        match &sessao.usuario {
            None => true,
            Some(u) => u.pode_em(database, tabela, Atividade::Administrar),
        }
    }

    /// Desfaz a declaracao de uma chave estrangeira, pelo nome.
    ///
    /// Encolher o bloco de esquema cabe sempre no lugar, entao isto nunca
    /// reescreve arquivo -- e nao toca em dado nenhum, porque a chave nunca
    /// foi imposta.
    pub(super) fn op_excluir_fk(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        if nome.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"nome\" (o nome da chave declarada)".into(),
            ));
        }
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let mut fks = t.esquema().chaves_estrangeiras().to_vec();
        let antes = fks.len();
        fks.retain(|f| f.nome != nome);
        if fks.len() == antes {
            return Err(PhxError::NaoEncontrado(format!(
                "{} nao tem uma chave chamada {nome:?}; as declaradas sao [{}]",
                t.nome(),
                t.esquema()
                    .chaves_estrangeiras()
                    .iter()
                    .map(|f| f.nome.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        t.redeclarar_chaves_estrangeiras(fks)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("nome", Json::texto_de(nome)),
            (
                "chaves_estrangeiras",
                Json::de_u64(t.esquema().chaves_estrangeiras().len() as u64),
            ),
        ]))
    }

    /// Redeclara os indices de TEXTO de uma tabela que ja existe (pedido 364).
    ///
    /// Ate aqui o `criar_tabela` era o unico caminho que aceitava
    /// `indices_texto`, e quem perdeu a declaracao no defeito do pedido 353
    /// ficava com um `.fts` orfao e so recuperava recriando a tabela. A
    /// lista chega no MESMO campo e pelo MESMO leitor do `criar_tabela`
    /// (`valores::indices_de_texto_de_json`), e o `.fts` se refaz pelo MESMO
    /// laco do `reindexar` -- nenhuma das duas metades e copia.
    ///
    /// Substitui a lista inteira; a lista vazia tira a declaracao e apaga o
    /// arquivo. Campo ausente e recusado, e nao lido como vazio: apagar um
    /// indice porque o cliente errou o nome do campo seria o defeito de
    /// «campo aceito e ignorado» com o sinal trocado.
    pub(super) fn op_redeclarar_indices_texto(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let lista = p
            .campo("indices_texto")
            .or_else(|| p.campo("indices_de_texto"))
            .and_then(Json::lista)
            .ok_or_else(|| {
                PhxError::Esquema(
                    "informe \"indices_texto\" como lista -- a lista inteira que a \
                     tabela passa a ter; vazia tira o indice de texto"
                        .into(),
                )
            })?;
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        // A mesma pergunta do `acrescentar_coluna`: transacao viva que
        // alcanca a tabela segura a troca do esquema, e quem cede e o DDL.
        if let Some(recado) = self.transacao_na_vizinhanca(
            &dados,
            p.texto_ou("database", ""),
            p.texto_ou("tabela", ""),
            &t,
        )? {
            return Err(PhxError::EmTransacao(recado));
        }
        let textos = crate::valores::indices_de_texto_de_json(lista, t.esquema())?;
        // O padrao do `acrescentar_coluna`: congela com a trava na mao, solta,
        // faz o O(linhas) fora -- o `.fts` montado ao lado e os `*.novo` do
        // `.reg` --, e retoma so para os `rename`. Uma secao nova com `fsync`
        // sob a trava global a catraca `alcancam-fsync-2` nao deixa entrar.
        let congelada = phxsql_store::congelamento::congelar(
            t.diretorio(),
            t.nome(),
            "redeclarando o indice de texto".to_string(),
        )?;
        drop(dados);
        let troca = t.preparar_indices_de_texto(textos)?;
        let dados = self.travar_dados()?;
        let linhas = t.redeclarar_indices_de_texto_fase_b(troca)?;
        drop(dados);
        drop(congelada);
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            (
                "indices_texto",
                Json::Lista(
                    t.esquema()
                        .indices_de_texto()
                        .iter()
                        .map(|it| Json::texto_de(&it.nome))
                        .collect(),
                ),
            ),
            ("linhas_indexadas", Json::de_u64(linhas)),
        ]))
    }

    /// Apaga os cinco arquivos de uma tabela.
    ///
    /// Exige o nome repetido no campo `confirmar`. Nao e burocracia: excluir
    /// uma tabela apaga o `.reg`, o `.ndx`, o `.bin`, o `.memo` e o `.log` de
    /// uma vez, e nao ha desfazer. Um `rowid` errado perde uma linha; um nome
    /// errado aqui perde tudo.
    pub(super) fn op_excluir_tabela(&self, p: &Json) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if p.texto_ou("confirmar", "") != tabela {
            return Err(PhxError::Esquema(format!(
                "para excluir, repita o nome da tabela no campo \"confirmar\": \
                 esperado {tabela:?}"
            )));
        }
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let (apagados, pendente) = db.excluir_tabela_adiando_o_fsync(tabela);
        let apagados = match apagados {
            Ok(a) => a,
            Err(e) => {
                // Pedido 595: o erro no meio deixa nomes que JA sairam, e eles
                // devem o `fsync` da pasta como no caminho feliz -- fora da
                // trava tambem. A recusa do disco, se vier, fala mais alto.
                drop(db);
                drop(dados);
                pendente.levar_ao_disco()?;
                return Err(e);
            }
        };
        self.renomear_nas_sujas(database, tabela, None);
        self.esquecer_diario(database, tabela);
        // Os gatilhos da tabela saem junto, como no MySQL(R): um orfao
        // dispararia contra uma homonima futura que nao tem nada com ele. Sob
        // a trava sai so da MEMORIA; o `gatilhos.json` vai ao disco depois.
        let mut gatilhos_apagados = 0usize;
        let mut gatilhos_por_gravar = None;
        let rotinas = if self.ha_gatilhos.load(Ordering::Relaxed) {
            self.rotinas.tomar("rotinas").map(|mut r| {
                (gatilhos_apagados, gatilhos_por_gravar) =
                    r.excluir_gatilhos_da_tabela(database, tabela);
                self.ha_gatilhos.store(r.ha_gatilhos(), Ordering::Relaxed);
            })
        } else {
            Ok(())
        };
        drop(db);
        drop(dados);
        // Pedido 595, e nesta ORDEM: primeiro o `gatilhos.json` sem os
        // gatilhos dela, duravel; depois o `fsync` da pasta onde moravam os
        // nomes que sairam (591). Ao contrario, uma queda entre os dois
        // deixava a tabela fora do disco e o gatilho DENTRO -- e ele
        // dispararia sobre a homonima que alguem criasse depois, que e
        // escrever no dado de outro. Nesta ordem a queda entre os dois pode,
        // no maximo, devolver a tabela sem os gatilhos de uma exclusao que o
        // cliente nunca ouviu terminar, e que ele repete.
        let gravou = rotinas.and_then(|()| self.gravar_rotinas(gatilhos_por_gravar));
        // O `fsync` da pasta vem mesmo se o cadastro falhou: os nomes ja
        // sairam do disco, e devolve-los numa queda e o defeito do 591.
        pendente.levar_ao_disco()?;
        gravou?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("tabela", Json::texto_de(tabela)),
            (
                "arquivos_apagados",
                Json::Lista(apagados.iter().map(Json::texto_de).collect()),
            ),
            ("gatilhos_apagados", Json::de_u64(gatilhos_apagados as u64)),
        ]))
    }

    /// Copia uma tabela inteira para outro nome, no mesmo database.
    ///
    /// Copia byte a byte os cinco arquivos, entao a copia nasce com a MESMA
    /// ordem de digitacao e os MESMOS rowids do original -- que e o que se
    /// espera de uma duplicata, e o que uma reinsercao linha a linha nao
    /// daria.
    pub(super) fn op_duplicar_tabela(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        let destino = p.texto_ou("destino", "");
        // O portao geral confere `criar` contra o campo `tabela`, que aqui e
        // a ORIGEM -- e a tabela que NASCE tem outro nome, no campo `destino`.
        // Sem esta linha, quem pode criar UMA tabela nominalmente cria
        // qualquer outra, bastando duplicar a que ele pode para o nome que
        // quiser. E exatamente a conferencia que o `copiar_tabela` ao lado ja
        // faz no destino dele; a diferenca era so que aqui o destino mora no
        // mesmo database e por isso parecia coberto.
        if let Some(u) = &sessao.usuario {
            if !u.pode_em(database, destino, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {database}.{destino}"
                )));
            }
        }
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let copia = db.duplicar_tabela_adiando_o_fsync(tabela, destino)?;
        // O mesmo do `copiar_tabela` (pedido 586): o `fsync` fora da trava,
        // antes da resposta.
        drop(dados);
        let copiados = copia.levar_ao_disco()?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("origem", Json::texto_de(tabela)),
            ("destino", Json::texto_de(destino)),
            ("arquivos", Json::de_u64(copiados as u64)),
        ]))
    }

    /// `RENAME TABLE` -- move a tabela para outro nome, no mesmo database.
    ///
    /// Exige direito nos DOIS nomes, e e a mesma armadilha que o
    /// `duplicar_tabela` ao lado ja tinha pago: o portao geral confere o campo
    /// `"tabela"`, que aqui e a ORIGEM. Sem a conferencia do destino, quem
    /// pode mexer numa tabela nominalmente escreveria em qualquer nome, bastando
    /// renomear a que ele pode para o nome que quiser.
    ///
    /// E pede `Criar` no destino e `Excluir` na origem porque e o que a
    /// operacao FAZ: um nome nasce e o outro morre. Pedir so `Criar` deixaria
    /// quem nao pode apagar uma tabela apaga-la, dando-lhe outro nome.
    pub(super) fn op_renomear_tabela(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        let destino = p.texto_ou("destino", "");
        if let Some(u) = &sessao.usuario {
            if !u.pode_em(database, destino, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {database}.{destino}"
                )));
            }
            if !u.pode_em(database, tabela, Atividade::Excluir) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de excluir em {database}.{tabela}"
                )));
            }
        }
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let movidos = db.renomear_tabela(tabela, destino)?;
        self.renomear_nas_sujas(database, tabela, Some(destino));
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("origem", Json::texto_de(tabela)),
            ("destino", Json::texto_de(destino)),
            ("arquivos", Json::de_u64(movidos as u64)),
        ]))
    }
}
