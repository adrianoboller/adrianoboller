#!/usr/bin/env python3
"""O CATALOGO DOS DEFEITOS REPOSTOS -- os dados, e nada mais.

Cada entrada aqui e um defeito que esta casa ja pagou, escrito de um jeito que
a maquina consegue REPOR: o arquivo, o trecho que existe hoje, o trecho que
existia no dia do estrago, e QUAIS testes tem de cair quando ele volta.

    python3 bancada/guardas/provar-guardas.py

# Por que este arquivo e Python, e nao JSON

Porque quem le este catalogo tem de conseguir AUDITAR se o `troca` e mesmo o
defeito de origem -- e nao um sabotador qualquer que derruba o teste por outro
motivo. Codigo Rust dentro de JSON vira uma linha de `\\"` e `\\n` que ninguem
confere. Aqui ele aparece como esta no fonte. Nao ha logica neste arquivo: e
uma lista de dicionarios, lida pelo executor ao lado.

# Os campos

    id       nome curto, e o que aparece no relatorio
    titulo   o defeito em uma linha. E o UNICO campo daqui que sai
             impresso num documento (a tabela do `docs/TESTES.md`), e
             por isso e o unico com acento: texto de documento leva,
             identificador e comentario nao
    porque   de onde ele veio -- a licao que o CLAUDE.md ou o docs/ ja escreveu
    arquivo  caminho relativo a `phxsql/`
    trecho   o texto EXATO de hoje. Tem de aparecer UMA vez so no arquivo:
             duas ocorrencias sao recusadas, porque trocar a errada provaria
             outra coisa
    troca    o que entra no lugar -- o defeito reposto
    trocas   opcional, e so para o defeito que mexia em MAIS DE UM ponto (os
             dois da amarracao do slot cifrado): uma lista de
             `{arquivo, trecho, troca}`. Quem a usa nao usa os tres de cima
    pacote   o `-p` do cargo
    alvo     o seletor do binario de teste (["--lib"] ou ["--test", "nome"])
    caem     os testes que TEM de falhar. Se um deles passar, o achado e dele:
             e um teste que passa por engano, e a casa considera isso pior que
             teste que falta
    seguem   os testes que tem de CONTINUAR passando. Sem esta lista, uma troca
             que quebrasse o arquivo inteiro pareceria uma guarda provada
    espera   "falha" (o normal), "aborta" (ver `cadeia-sem-teto`: o defeito
             derruba o binario inteiro, e o tamanho do estrago E a prova) ou
             "nada muda" (ver `aad-fora-do-slot`: a entrada AFIRMA que tirar
             esta metade nao e sentida, porque a outra metade cobre sozinha --
             e o dia em que algum teste cair, a afirmacao morreu)
    nota_da_redundancia
             so com `espera: "nada muda"`: a frase que o relatorio imprime
             quando a afirmacao se confirma
    prazo    segundos ate o executor matar a rodada. Defeito que PENDURA em vez
             de falhar trava a bateria; o `sujas-com-a-trava` e exatamente esse
"""

DEFEITO_ALCANCAR_TABELA = """        // DEFEITO REPOSTO: a trava de dados tomada aqui e segurada ate o fim
        // da funcao -- e no meio do laco mora `replica::puxar`, que e uma ida
        // e volta de rede. Rede sa esconde; source mudo prende o servidor
        // inteiro ate o prazo de leitura de 30 s estourar.
        let trava = self.travar_dados()?;
        let db = trava.garantir_database(database)?;
        let mut tabela = match db.abrir_qualificada(&no.nome) {
            Ok(t) => t,
            Err(_) => match &no.esquema {
                Some(e) => {
                    let schema = no.nome.split_once('.').map(|(s, _)| s.to_string());
                    db.criar_tabela(schema.as_deref(), e.clone())?
                }
                None => return Ok(0),
            },
        };
        tabela.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);
        let mut posicao = tabela.eventos()?;
        if posicao >= no.eventos {
            return Ok(0);
        }
        let mut aplicados = 0u64;
        while posicao < no.eventos {
            let eventos = crate::replica::puxar(cliente, database, &no.nome, posicao)?;
            if eventos.is_empty() {
                break;
            }
            for e in &eventos {
                tabela.aplicar_evento(e.operacao, e.rowid, &e.imagem)?;
                aplicados += 1;
            }
            let nova = tabela.eventos()?;
            if nova <= posicao {
                break;
            }
            posicao = nova;
        }
        tabela.sincronizar()?;
        Ok(aplicados)
"""

HOJE_ALCANCAR_TABELA = """        let Some(mut posicao) = self.abrir_para_replicar(database, no)? else {
            return Ok(0);
        };
        if posicao >= no.eventos {
            return Ok(0);
        }
        let mut aplicados = 0u64;
        while posicao < no.eventos {
            // FORA da trava. Se a conexao cair aqui, o lote se perde e nada
            // foi gravado: a posicao local nao andou, e a proxima rodada pede
            // exatamente os mesmos eventos. Nao ha meio-lote possivel porque
            // o lote inteiro chega antes de a trava ser pedida.
            let eventos = crate::replica::puxar(cliente, database, &no.nome, posicao)?;
            if eventos.is_empty() {
                break;
            }
            let (n, nova) = self.aplicar_lote_da_replica(database, no, posicao, &eventos)?;
            aplicados += n;
            posicao = nova;
        }
        if aplicados > 0 {
            self.sincronizar_replicada(database, &no.nome)?;
        }
        Ok(aplicados)
"""

GUARDAS = [
    # -----------------------------------------------------------------------
    # 1. O Profiler recortando o texto em vez de analisar
    # -----------------------------------------------------------------------
    {
        "id": "profiler-recorta",
        "titulo": "o Profiler recorta o texto do pedido em vez de analisar",
        "porque": (
            "regra do CLAUDE.md: funcionalidade que mostra texto cru redige "
            "ANALISANDO, nunca recortando. Recortar depende de o pedido estar "
            "escrito de um jeito; analisar e reserializar nao."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": """    let tamanho = linha.trim().len();
    match Json::analisar(linha) {
        Ok(j @ Json::Objeto(_)) => limpar(&j).escrever(),
        Ok(_) => format!("<pedido nao e objeto, {tamanho} bytes>"),
        Err(_) => format!("<pedido invalido, {tamanho} bytes>"),
    }
""",
        "troca": r'''    // DEFEITO REPOSTO: recorta o texto cru em vez de analisar e reserializar.
    // E a tentacao de sempre, porque parece mais barato: procura o pedaco
    // `"senha":"` e tapa ate a proxima aspa.
    let mut s = linha.to_string();
    let mut de = 0usize;
    while let Some(i) = s[de..].find("\"senha\":\"") {
        let ini = de + i + 9;
        match s[ini..].find('"') {
            Some(fim) => {
                s.replace_range(ini..ini + fim, "***");
                de = ini + 3;
            }
            None => break,
        }
    }
    s
''',
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        # Cinco, e nao sete -- e o corte foi MEDIDO, nao suposto.
        #
        # A primeira versao desta entrada listava sete, e o executor devolveu
        # NAO PEGOU em dois. Investigados um a um:
        #
        # - `aspas_escapadas_dentro_de_um_valor_nao_confundem` guarda o recorte
        #   errando para o OUTRO lado (tapando o que nao era segredo), e este
        #   recorte -- que exige o dois-pontos colado entre as aspas -- nao erra
        #   assim: dentro de um valor o texto chega escapado, e o par nunca fica
        #   colado. Quem o derruba e o `profiler-recorta-largo`, logo abaixo.
        # - `quebra_de_linha_no_pedido_nao_forja_linha_no_arquivo` nao passa
        #   pelo `redigir`: o pedido dele e `{}` e as quebras estao na `op`, no
        #   usuario e no banco. Quem o guarda e o `de_uma_linha`, e ele ganhou
        #   guarda propria (`evento-linha-sem-escape`).
        #
        # O comentario do fonte diz que os seis torcidos "todos falham se
        # alguem trocar a analise por um find e um corte". Medido: dependem de
        # QUAL corte. Nenhum dos dois testes esta errado -- errada estava a
        # conta de sete, que era minha.
        "caem": [
            "profiler::testes::pedido_invalido_nao_vira_texto",
            "profiler::testes::chave_escapada_em_unicode_tambem_e_senha",
            "profiler::testes::chave_com_espaco_no_nome_ainda_e_senha",
            "profiler::testes::topo_que_nao_e_objeto_nao_vira_texto",
            "profiler::testes::corpo_que_nao_e_json_vira_o_tamanho",
        ],
        "seguem": [
            "profiler::testes::o_resto_do_pedido_continua_visivel",
            # Verde de proposito: o recorte estreito nao tapa o que nao e
            # segredo. Deixa-lo aqui trava a diferenca entre os dois recortes
            # em vez de deixa-la so no comentario.
            "profiler::testes::aspas_escapadas_dentro_de_um_valor_nao_confundem",
        ],
    },
    # -----------------------------------------------------------------------
    # 1b. O MESMO defeito com a mao mais pesada -- e outros testes caem
    # -----------------------------------------------------------------------
    {
        "id": "profiler-recorta-largo",
        "titulo": "o Profiler recorta procurando a palavra `senha` solta",
        "porque": (
            "a outra ponta do mesmo erro. O recorte estreito deixa passar; "
            "este tapa DEMAIS e come dado -- o campo `obs` em que alguem "
            "escreveu a palavra senha vira `***`. E o motivo de o teste das "
            "aspas escapadas existir, e ele so cai aqui."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": """    let tamanho = linha.trim().len();
    match Json::analisar(linha) {
        Ok(j @ Json::Objeto(_)) => limpar(&j).escrever(),
        Ok(_) => format!("<pedido nao e objeto, {tamanho} bytes>"),
        Err(_) => format!("<pedido invalido, {tamanho} bytes>"),
    }
""",
        "troca": """    // DEFEITO REPOSTO: recorta procurando a PALAVRA `senha` e tapando o
    // proximo valor entre aspas. Nao distingue chave de conteudo, e por
    // isso erra para o OUTRO lado: come dado.
    let mut s = linha.to_string();
    let mut de = 0usize;
    while let Some(i) = s[de..].find("senha") {
        let base = de + i + 5;
        let Some(dp) = s[base..].find(':') else { break };
        let Some(a1) = s[base + dp..].find('"') else { break };
        let ini = base + dp + a1 + 1;
        match s[ini..].find('"') {
            Some(fim) => {
                s.replace_range(ini..ini + fim, "***");
                de = ini + 3;
            }
            None => break,
        }
    }
    s
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::aspas_escapadas_dentro_de_um_valor_nao_confundem",
            "profiler::testes::pedido_invalido_nao_vira_texto",
            "profiler::testes::topo_que_nao_e_objeto_nao_vira_texto",
            "profiler::testes::corpo_que_nao_e_json_vira_o_tamanho",
        ],
        "seguem": [
            "profiler::testes::o_resto_do_pedido_continua_visivel",
        ],
    },
    # -----------------------------------------------------------------------
    # 1c. A linha do arquivo que o SUSPEITO escreve
    # -----------------------------------------------------------------------
    {
        "id": "evento-linha-sem-escape",
        "titulo": "campo livre vai cru para o .txt e forja uma linha inteira",
        "porque": (
            "provado por soquete antes de virar teste: um pedido com uma "
            "quebra de linha no nome da `op` deixou no .txt uma segunda linha "
            "que se le como um evento de outro IP e de outro usuario. Quem "
            "investiga um incidente estaria lendo o que o suspeito escreveu."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        # Raw string, e nao a normal: o trecho tem `\n` DE VERDADE dentro do
        # fonte Rust, e uma string comum o transformaria numa quebra de linha.
        # O executor recusou a entrada na primeira tentativa justamente por
        # isso -- "o trecho nao esta mais no arquivo" -- e a recusa e a certa:
        # trecho que nao casa nao pode virar prova de nada.
        "trecho": r"""    for c in s.chars().take(teto) {
        match c {
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                saida.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => saida.push(c),
        }
    }
""",
        "troca": """    // DEFEITO REPOSTO: o campo livre vai CRU para o arquivo, com controle e
    // tudo -- e uma quebra de linha vira uma segunda linha de evento.
    for c in s.chars().take(teto) {
        saida.push(c);
    }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::quebra_de_linha_no_pedido_nao_forja_linha_no_arquivo",
        ],
        # O corte com aviso e outra coisa e continua valendo: se este cair
        # junto, a troca comeu mais do que o defeito comia.
        "seguem": [
            "profiler::testes::campo_gigante_e_cortado_na_linha",
        ],
    },
    # -----------------------------------------------------------------------
    # 2. O portao do Profiler ausente
    # -----------------------------------------------------------------------
    {
        "id": "profiler-sem-portao",
        "titulo": "o portão próprio do Profiler não existe; o leitor lê o pedido alheio",
        "porque": (
            "regra do CLAUDE.md: portao de permissao e UM so, e o campo que ele "
            "le e o furo. Nenhum pedido do profiler tem `database`, entao o "
            "portao geral pergunta pela base VAZIA e quem tem "
            '`bases:{\"*\":{administrar:true}}` responde sim sem ser admin.'
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        match &sessao.usuario {
            None => Ok(()),
            Some(u) if u.e_admin() => Ok(()),
            Some(u) => Err(PhxError::Autorizacao(format!(
                "{} nao e administrador deste servidor; o profiler mostra o \\
                 texto dos pedidos de todo mundo, inclusive das tabelas que \\
                 este login nao pode ler",
                u.login
            ))),
        }
""",
        "troca": """        // DEFEITO REPOSTO: sem portao proprio, o portao geral decide -- e ele
        // pergunta sobre a base vazia.
        let _ = sessao;
        Ok(())
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_portao_do_profiler::leitor_com_administrar_no_curinga_nao_liga_o_profiler",
        ],
        "seguem": [
            "servidor::testes_portao_do_profiler::administrador_continua_ligando",
            "servidor::testes_portao_do_profiler::sem_cadastro_nada_muda",
        ],
    },
    # -----------------------------------------------------------------------
    # 3, 4, 5. A familia do juntar/unir: quem varre a base sem campo `tabela`
    # -----------------------------------------------------------------------
    {
        "id": "pivotar-sem-portao",
        "titulo": "`pivotar` sem conferência própria: a junção vira a porta dos fundos",
        "porque": (
            "docs/TESTES.md 3.2 -- o pivot tem DOIS lugares com tabela, e o de "
            "dentro de `juntar` o portao geral nao alcanca. Os rotulos das "
            "linhas do cruzamento SAO os valores da tabela negada."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Some(u) = &sessao.usuario {
            let base = p.texto_ou("database", "");
            for j in p.campo("juntar").and_then(Json::lista).unwrap_or(&[]) {
                let alvo = j.texto_ou("tabela", "");
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(p.texto_ou("database", ""))?;
        let mut t = db.abrir_qualificada(p.texto_ou("tabela", ""))?;
        let esquema = t.esquema().clone();
""",
        "troca": """        // DEFEITO REPOSTO: `let _ = sessao;` era exatamente como a funcao
        // comecava antes da conferencia entrar.
        let _ = sessao;

        let dados = self.travar_dados()?;
        let db = dados.abrir_database(p.texto_ou("database", ""))?;
        let mut t = db.abrir_qualificada(p.texto_ou("tabela", ""))?;
        let esquema = t.esquema().clone();
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::pivotar_nao_e_a_porta_dos_fundos",
        ],
        "seguem": [
            "servidor::testes_direito_por_tabela::pivotar_na_tabela_permitida_continua_valendo",
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_nada_muda",
        ],
    },
    {
        "id": "sequencias-sem-portao",
        "titulo": "`sequencias` mostra o contador de toda tabela, inclusive a negada",
        "porque": (
            "docs/TESTES.md 3.3 -- a terceira porta para a lista que a arvore "
            "esconde. Nome, contador e quantas linhas ja bastam para saber que "
            "a folha existe e quanto ela cresceu no mes."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            // Ela varre a base inteira e NAO tem campo `tabela`, entao o
            // portao geral nao a alcanca: e o mesmo desenho do
            // `dados_pessoais`, que filtra tabela a tabela por dentro.
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
""",
        "troca": """            // DEFEITO REPOSTO: sem conferencia propria, a base inteira sai.
            let _ = sessao;
            let Ok(t) = db.abrir_qualificada(&nome) else {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::sequencias_esconde_a_tabela_negada",
        ],
        "seguem": [
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_posicao_e_sequencias_veem_tudo",
        ],
    },
    {
        "id": "posicao-sem-portao",
        "titulo": "`posicao` entrega eventos e o esquema cru de toda tabela",
        "porque": (
            "docs/TESTES.md 3.3 -- o `SHOW MASTER STATUS` daqui. A conferencia "
            "e de `replicar`, e nao de `ler`: e o direito que o portao aplicou "
            "a operacao."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&database, &nome, Atividade::Replicar) {
                    continue;
                }
            }
            let mut t = db.abrir_qualificada(&nome)?;
""",
        "troca": """            // DEFEITO REPOSTO: sem conferencia propria, a base inteira sai.
            let _ = sessao;
            let mut t = db.abrir_qualificada(&nome)?;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::posicao_esconde_a_tabela_negada",
        ],
        "seguem": [
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_posicao_e_sequencias_veem_tudo",
        ],
    },
    {
        "id": "duplicar-sem-destino",
        "titulo": "`duplicar_tabela` confere a origem e não o destino",
        "porque": (
            "docs/TESTES.md 3.4 -- o portao confere `criar` contra o campo "
            "`tabela`, que ali e a ORIGEM; a tabela que nasce tem o nome do "
            "campo `destino`."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Some(u) = &sessao.usuario {
            if !u.pode_em(database, destino, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {database}.{destino}"
                )));
            }
        }
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let copiados = db.duplicar_tabela(tabela, destino)?;
""",
        "troca": """        // DEFEITO REPOSTO: so a origem passou pelo portao geral.
        let _ = sessao;
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let copiados = db.duplicar_tabela(tabela, destino)?;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::duplicar_confere_o_direito_no_destino",
        ],
        "seguem": [
            "servidor::testes_direito_por_tabela::duplicar_com_direito_no_destino_continua_valendo",
        ],
    },
    # -----------------------------------------------------------------------
    # 5b. A guarda IMPOSTA em vez de pedida -- a regra que a casa mais repete
    # -----------------------------------------------------------------------
    {
        "id": "regra-de-tabela-imposta",
        "titulo": "sem regra de tabela, nega: a guarda nova entra imposta e nao pedida",
        "porque": (
            "regra do CLAUDE.md, e a que esta casa mais repete: guarda nova "
            "entra PEDIDA, nao imposta. Quando o direito por tabela entrou, o "
            "`permissoes_em` teve de cair na regra da BASE quando nao ha regra "
            "de tabela nenhuma -- senao todo `config.json` que ja existia "
            "passaria a negar tudo, e ninguem pediu isso. O teste que trava "
            "isso e o do comportamento VELHO."
        ),
        "arquivo": "crates/phxsql-server/src/usuarios.rs",
        "trecho": """                }
            }
        }
        self.permissoes(database)
    }
""",
        "troca": """                }
            }
            // DEFEITO REPOSTO: sem regra de tabela, NEGA -- em vez de cair na
            // regra da base. E a guarda nova imposta a quem nao pediu.
            return Permissoes::default();
        }
        self.permissoes(database)
    }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_nada_muda",
        ],
        # O estrago LARGO e o ponto desta entrada, e nao um efeito colateral:
        # uma guarda imposta tira o direito de todo mundo que ja funcionava.
        # Medido: com o defeito reposto caem 14 dos 540 testes do `--lib`, e
        # entre eles estao os «continua valendo» do duplicar e do pivotar. Quem
        # fica aqui e o que TEM de sobreviver mesmo assim, porque nao depende de
        # regra de tabela nenhuma: o supervisor passa por cima de todas.
        "seguem": [
            "servidor::testes_direito_por_tabela::supervisor_passa_por_cima",
        ],
    },
    # -----------------------------------------------------------------------
    # 3. O abraco mortal: `descarregar_sujas()` com a trava de dados na mao
    # -----------------------------------------------------------------------
    {
        "id": "sujas-com-a-trava",
        "titulo": "`descarregar_sujas()` chamado com a trava de dados já na mão",
        "porque": (
            "o `Mutex` do Rust nao e reentrante: a thread para para sempre "
            "segurando o servidor inteiro. So aparece com DUAS tabelas -- com "
            "uma so o conjunto de sujas fica vazio e a funcao volta antes de "
            "pedir a trava. O teste tem PRAZO porque um abraco mortal sem "
            "prazo pendura o `cargo test` inteiro, e um teste que pendura nao "
            "acusa nada. "
            "ATUALIZADO na rodada da guarda de reentrancia: o defeito NAO "
            "pendura mais -- a segunda tomada volta com erro, o `else "
            "{ return }` do `descarregar_sujas` engole, e a janela de "
            "durabilidade simplesmente nao fecha. O teste ganhou a asercao da "
            "consequencia (o conjunto de sujas tem de esvaziar) e passou a "
            "reprovar em 0,12 s em vez de 30. A licao vale para toda guarda "
            "cujo unico sintoma era o travamento: quem troca um travamento "
            "por um erro engolido ENFRAQUECE esses testes, e tem de olhar a "
            "consequencia no lugar."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        self.descarregar_sujas_com(dados);
        Ok(())
    }

    /// Sincroniza tudo que foi escrito e ainda nao foi para o disco.
""",
        "troca": """        // DEFEITO REPOSTO: a versao SEM a trava na mao, pedindo a trava de novo.
        self.descarregar_sujas();
        Ok(())
    }

    /// Sincroniza tudo que foi escrito e ainda nao foi para o disco.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_janela_e_cadeia::duas_tabelas_na_mesma_janela_nao_travam_o_servidor",
        ],
        "seguem": [
            "servidor::testes_janela_e_cadeia::uma_tabela_so_grava_como_sempre",
        ],
        # O `com_prazo` do proprio teste espera 30 s; o executor precisa de
        # folga sobre isso, senao MATA a rodada antes de o teste reprovar --
        # e uma rodada morta pelo executor nao prova guarda nenhuma.
        "prazo": 420,
    },
    # -----------------------------------------------------------------------
    # 4. A cadeia de gatilhos sem teto -- o unico que ABORTA o processo
    # -----------------------------------------------------------------------
    {
        "id": "cadeia-sem-teto",
        "titulo": "a cadeia de gatilhos sem fundo: o binário aborta com stack overflow",
        "porque": (
            "um `AFTER INSERT ON t` que grava em `t` chama a si mesmo. Nao e "
            "laco lento: e recursao de pilha, e o Rust ABORTA O PROCESSO. Como "
            "o corpo mora no `gatilhos.json`, ele derrubava de novo a cada "
            "tentativa."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        let nivel = PROFUNDIDADE_DA_CADEIA.with(|c| c.get());
        if nivel >= CADEIA_MAXIMA {
""",
        "troca": """        // DEFEITO REPOSTO: o teto nao existe -- `u32::MAX` e o mesmo que nao
        // ter fundo, e a pilha estoura antes de chegar la.
        let nivel = PROFUNDIDADE_DA_CADEIA.with(|c| c.get());
        if nivel >= u32::MAX {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        # ESPERA "aborta": reposto o defeito, o teste NAO falha -- ele derruba
        # o binario de teste inteiro com "stack overflow". Exigir `FAILED` aqui
        # daria "nao pegou" num defeito que a guarda pega do jeito mais
        # barulhento possivel. O tamanho do estrago E a prova.
        "espera": "aborta",
        "caem": [
            "servidor::testes_janela_e_cadeia::a_cadeia_de_gatilhos_para_no_teto_e_avisa",
        ],
        "seguem": [],
        "prazo": 420,
    },
    # -----------------------------------------------------------------------
    # 5. `excluir_tabela` com a lista curta de extensoes
    # -----------------------------------------------------------------------
    {
        "id": "excluir-tabela-lista-curta",
        "titulo": "`excluir_tabela` apaga SEIS extensões e a tabela já tem NOVE",
        "porque": (
            "a mesma armadilha da peca nova no fim de uma lista que o `rownum` "
            "armou na tela: `.trash`, `.reason` e `.pag` entraram depois e "
            "ninguem voltou aqui. Recriar a tabela com o mesmo nome passava a "
            "ser impossivel, e a tabela nova herdaria a lixeira alheia."
        ),
        "arquivo": "crates/phxsql-store/src/catalogo.rs",
        "trecho": """        let mut apagados = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
""",
        "troca": """        // DEFEITO REPOSTO: a lista de COPIAR no lugar da lista de APAGAR.
        let mut apagados = Vec::new();
        for ext in Self::EXTENSOES {
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "catalogo::testes_gestao::excluir_tabela_deixa_o_nome_livre_para_a_proxima",
        ],
        "seguem": [
            "catalogo::testes_gestao::excluir_tabela_leva_os_arquivos_dela_e_so_os_dela",
            "catalogo::testes_gestao::excluir_tabela_que_nao_existe_e_erro",
        ],
    },
    # -----------------------------------------------------------------------
    # 6. A conferencia de SHA-256 do backup desligada
    # -----------------------------------------------------------------------
    {
        "id": "backup-sem-sha256",
        "titulo": "restaurar aceita o backup adulterado: só o tamanho é conferido",
        "porque": (
            "trocar bytes MANTENDO o tamanho passa pela conferencia de bytes. "
            "So o SHA-256 pega, e sem ele o backup adulterado vira database "
            "novo sem ninguem reclamar."
        ),
        "arquivo": "crates/phxsql-store/src/restaurar.rs",
        "trecho": """            let confere = para_hex(&sha256(&dados));
            if &confere != sha {
                return Err(PhxError::Corrompido(format!(
                    "{caminho}: o SHA-256 nao bate com o {MANIFESTO} -- \\
                     este backup nao esta integro e NADA foi restaurado"
                )));
            }
""",
        "troca": """            // DEFEITO REPOSTO: o SHA-256 e calculado e jogado fora -- so o
            // tamanho, que e a conferencia mais fraca, continua valendo.
            let _ = (para_hex(&sha256(&dados)), sha);
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "restaurar::tests::manifesto_que_nao_confere_e_recusado_e_nada_e_escrito",
        ],
        "seguem": [
            "restaurar::tests::arquivo_que_nao_esta_no_manifesto_recusa",
            "restaurar::tests::backup_antigo_sem_escopo_no_manifesto_ainda_restaura",
        ],
    },
    # -----------------------------------------------------------------------
    # 7. O AAD fora do slot cifrado
    # -----------------------------------------------------------------------
    # -----------------------------------------------------------------------
    # 8. A amarracao do corpo cifrado ao ENDERECO -- e as duas fechaduras
    #
    # A entrada nasceu como uma so, "tirar o AAD", porque e o que a ficha do
    # teste manda ("Provado com o defeito reposto: tirando o `aad` do
    # `montar_slot` e do `abrir_slot`, este teste passa a ler a linha
    # trocada"). Medido, isso e FALSO: o teste continuou verde.
    #
    # O motivo, achado seguindo o codigo depois da medicao: o endereco esta
    # amarrado DUAS vezes. O `aad_do_slot` leva (volume, rowid, versao), e o
    # `nonce_de_pedaco(rowid, volume, versao, tempero)` leva os mesmos tres.
    # Sao duas fechaduras na mesma porta, e cada uma segura sozinha.
    #
    # Entao viraram tres entradas, e as tres sao medidas: as duas primeiras
    # AFIRMAM a redundancia (tirar so uma nao muda nada) e a terceira prova a
    # guarda de verdade (tirar as duas derruba o teste). No dia em que alguem
    # trocar o nonce por um sorteado e guardado no slot, a primeira entrada
    # deixa de ser redundante e o relatorio avisa.
    # -----------------------------------------------------------------------
    {
        "id": "aad-fora-do-slot",
        "titulo": "só o dado associado sai: o nonce sozinho ainda amarra o endereço",
        "porque": (
            "a ficha do teste dizia que tirar o AAD o derrubava. Medido, nao "
            "derruba -- e a diferenca entre diagnostico plausivel e "
            "diagnostico medido."
        ),
        "espera": "nada muda",
        "nota_da_redundancia": (
            "confirmado: tirar so o AAD nao e sentido por teste nenhum, "
            "porque o `nonce_de_pedaco` carrega (rowid, volume, versao)"
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-store/src/reg.rs",
                "trecho": """            let selado = self
                .material
                .selar(&nonce, &aad_do_slot(volume, rowid, versao), &claro);
""",
                "troca": """            // DEFEITO REPOSTO (1/2): a etiqueta cobre o conteudo, nao o endereco.
            let selado = self.material.selar(&nonce, b"", &claro);
""",
            },
            {
                "arquivo": "crates/phxsql-store/src/reg.rs",
                "trecho": """        let claro = self.material.abrir(
            &nonce,
            &aad_do_slot(volume, rowid, versao),
            &guardado,
""",
                "troca": """        // DEFEITO REPOSTO (2/2): a conferencia tambem deixa de olhar o endereco.
        let claro = self.material.abrir(
            &nonce,
            b"",
            &guardado,
""",
            },
        ],
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-dados"],
        "caem": [],
        "seguem": [
            "trocar_o_corpo_de_uma_linha_pela_outra_nao_passa",
            "cifrada_a_tabela_funciona_igual",
        ],
    },
    {
        "id": "nonce-sem-endereco",
        "titulo": "só o endereço sai do nonce: o AAD sozinho ainda amarra",
        "porque": (
            "a outra metade da mesma medicao. Tirar (rowid, volume) do "
            "`nonce_de_pedaco` tambem nao derruba nada, porque o AAD cobre."
        ),
        "espera": "nada muda",
        "nota_da_redundancia": (
            "confirmado: tirar so o endereco do nonce tambem passa despercebido"
        ),
        "arquivo": "crates/phxsql-store/src/cofre.rs",
        "trecho": """    let mut n = [0u8; XNONCE_LEN];
    n[0..8].copy_from_slice(&onde.to_le_bytes());
    n[8..12].copy_from_slice(&quem.to_le_bytes());
""",
        "troca": """    // DEFEITO REPOSTO: o nonce deixa de carregar o endereco -- so a versao e
    // o tempero ficam, e sao eles que impedem o nonce repetido.
    let mut n = [0u8; XNONCE_LEN];
    let _ = (onde, quem);
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-dados"],
        "caem": [],
        "seguem": [
            "trocar_o_corpo_de_uma_linha_pela_outra_nao_passa",
            # Este e o que importa aqui: o `tempero` continua no nonce, entao
            # duas gravacoes da mesma linha continuam sem repetir texto cifrado.
            "regravar_a_mesma_linha_nunca_repete_o_texto_cifrado",
        ],
    },
    {
        "id": "endereco-fora-da-amarracao",
        "titulo": "as DUAS fechaduras somem: dá para embaralhar as linhas cifradas",
        "porque": (
            "sem amarrar o corpo ao endereco, quem tem o arquivo e nao tem a "
            "chave copia os bytes do slot 5 por cima do 9, conserta o CRC-32 "
            "-- que e publico -- e a linha 9 devolve o conteudo da 5 sem erro "
            "nenhum. Cifra sem essa amarracao protege o conteudo e nao protege "
            "a tabela."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-store/src/reg.rs",
                "trecho": """            let selado = self
                .material
                .selar(&nonce, &aad_do_slot(volume, rowid, versao), &claro);
""",
                "troca": """            // DEFEITO REPOSTO (1/3): a etiqueta deixa de cobrir o endereco.
            let selado = self.material.selar(&nonce, b"", &claro);
""",
            },
            {
                "arquivo": "crates/phxsql-store/src/reg.rs",
                "trecho": """        let claro = self.material.abrir(
            &nonce,
            &aad_do_slot(volume, rowid, versao),
            &guardado,
""",
                "troca": """        // DEFEITO REPOSTO (2/3): a conferencia tambem deixa de olhar o endereco.
        let claro = self.material.abrir(
            &nonce,
            b"",
            &guardado,
""",
            },
            {
                "arquivo": "crates/phxsql-store/src/cofre.rs",
                "trecho": """    let mut n = [0u8; XNONCE_LEN];
    n[0..8].copy_from_slice(&onde.to_le_bytes());
    n[8..12].copy_from_slice(&quem.to_le_bytes());
""",
                "troca": """    // DEFEITO REPOSTO (3/3): e o nonce tambem.
    let mut n = [0u8; XNONCE_LEN];
    let _ = (onde, quem);
""",
            },
        ],
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-dados"],
        "caem": [
            "trocar_o_corpo_de_uma_linha_pela_outra_nao_passa",
        ],
        # A cifra continua indo e voltando: se estes cairem junto, a troca
        # quebrou mais do que devia e a guarda nao esta provada.
        "seguem": [
            "cifrada_a_tabela_funciona_igual",
            "o_indice_sobre_a_coluna_marcada_continua_em_claro",
            "regravar_a_mesma_linha_nunca_repete_o_texto_cifrado",
        ],
    },
    {
        "id": "cache-de-chaves-nao-limpo",
        "titulo": "trocar a senha da cifra não limpa o cache: a senha errada abre",
        "porque": (
            "o cache e por (sal, iteracoes), NAO por senha. Deixando-o de pe, "
            "`derivar` acha a entrada do sal e devolve a chave da senha "
            "ANTIGA. Um servidor que aceita a senha errada por ter aberto o "
            "arquivo antes e pior que um que a recusa."
        ),
        "arquivo": "crates/phxsql-store/src/cofre.rs",
        "trecho": """    if let Ok(mut d) = DERIVADAS.lock() {
        *d = None;
    }
    Ok(())
}

/// Desliga a cifra. Volumes ja cifrados deixam de abrir ate ela voltar.
""",
        "troca": """    // DEFEITO REPOSTO: o cache fica de pe, e com ele a chave da senha antiga.
    Ok(())
}

/// Desliga a cifra. Volumes ja cifrados deixam de abrir ate ela voltar.
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-dados"],
        "caem": [
            "senha_errada_e_falta_de_senha_param_na_abertura",
        ],
        "seguem": [
            "cifrada_a_tabela_funciona_igual",
            "tabela_escrita_antes_da_cifra_continua_abrindo",
        ],
    },
    # -----------------------------------------------------------------------
    # 9. A catraca dos textos fora da fabrica
    # -----------------------------------------------------------------------
    {
        "id": "catraca-dos-textos",
        "titulo": "mais um texto de tela cravado, fora da fábrica de idiomas",
        "porque": (
            "regra petrea do dono: texto de tela entra pela fabrica de "
            "idiomas. A catraca existe para reprovar quem acrescentar mais um "
            "-- e o defeito reposto e literalmente isso: UM rotulo a mais, "
            "escrito em portugues dentro do HTML."
        ),
        "arquivo": "crates/phxsql-server/ui/index.html",
        "trecho": """<title data-txt="tela.titulo_pagina">PhxSql — Centro de Controle</title>
""",
        "troca": """<title data-txt="tela.titulo_pagina">PhxSql — Centro de Controle</title>
<p hidden>Relatorio mensal de vendas</p>
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "conferidor::testes::a_catraca_dos_textos_fora_da_fabrica",
        ],
        "seguem": [
            "conferidor::testes::reprova_o_rotulo_cravado_e_aprova_o_da_fabrica",
            "conferidor::testes::dado_interpolado_nunca_conta_como_rotulo",
        ],
    },
    # -----------------------------------------------------------------------
    # 10. A trava de dados tomada fora do ponto unico
    # -----------------------------------------------------------------------
    {
        "id": "trava-fora-do-ponto-unico",
        "titulo": "uma tomada da trava de dados fora do `travar_dados()`",
        "porque": (
            "o comentario do `travar_dados` afirmava ser «o unico lugar que a "
            "toma» e ficou errado por rodadas: havia 13 fora dele, e o "
            "`espera_ms_s` da telemetria media so uma parte da fila. "
            "Comentario nao conta; teste conta -- e a conta sai do PROPRIO "
            "fonte, pelo mesmo `include_str!` do conferidor de textos, para "
            "nao haver como contar um arquivo e compilar outro."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        let dados = self.travar_dados()?;
        Ok(idiomas::estado(&dados, idioma))
""",
        "troca": """        // DEFEITO REPOSTO: a decima-quarta tomada, fora do ponto unico.
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        Ok(idiomas::estado(&dados, idioma))
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_janela_e_cadeia::so_um_lugar_toma_a_trava",
        ],
        # A tomada direta funciona igual -- ela so nao e cronometrada. Por
        # isso a escrita comum segue de pe: o que este defeito estraga e a
        # MEDIDA, e e por isso que ele passou tanto tempo sem ninguem ver.
        "seguem": [
            "servidor::testes_janela_e_cadeia::uma_tabela_so_grava_como_sempre",
            "servidor::testes_janela_e_cadeia::sem_reentrancia_nada_muda",
        ],
    },
    # -----------------------------------------------------------------------
    # 11. A guarda de reentrancia da trava
    # -----------------------------------------------------------------------
    {
        "id": "trava-sem-guarda-de-reentrancia",
        "titulo": "a trava pedida duas vezes pela mesma thread pendura o servidor",
        "porque": (
            "`std::sync::Mutex` nao e reentrante, e o abraco mortal ja "
            "aconteceu tres vezes neste projeto -- a ultima em configuracao "
            "padrao, com escrita comum em duas tabelas. Sem a guarda o "
            "servidor nao falha: ele PARA, sem log e sem pilha, segurando "
            "todas as outras conexoes. O teste tem prazo porque um defeito "
            "que pendura penduraria o `cargo test` inteiro."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if COM_A_TRAVA.with(std::cell::Cell::get) {
            return Err(trava_reentrante());
        }
""",
        "troca": """        // DEFEITO REPOSTO: sem a pergunta, a segunda tomada da mesma thread
        // espera por si mesma e nao volta nunca.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_janela_e_cadeia::a_trava_pedida_duas_vezes_pela_mesma_thread_vira_erro",
        ],
        # O `sem_reentrancia_nada_muda` e o teste do comportamento VELHO: quem
        # nunca aninha nao ve diferenca, com ou sem guarda. Ele TEM de seguir
        # de pe -- se cair, a guarda esta cobrando de quem nao a usa.
        "seguem": [
            "servidor::testes_janela_e_cadeia::sem_reentrancia_nada_muda",
            "servidor::testes_janela_e_cadeia::uma_tabela_so_grava_como_sempre",
        ],
        # Mesmo motivo do `sujas-com-a-trava`: o `com_prazo` do teste espera
        # 30 s, e o executor precisa de folga sobre isso.
        "prazo": 420,
    },
    # 12. A exclusao na janela virando o PADRAO
    # -----------------------------------------------------------------------
    {
        "id": "exclusao-na-janela-por-padrao",
        "titulo": "a exclusão entra na janela por padrão, sem ninguém pedir",
        "porque": (
            "regra do CLAUDE.md: guarda nova entra pedida, nao imposta -- e "
            "retirar guarda sem pedido e o mesmo estrago pelo outro lado. Hoje "
            "um `excluir` que responde OK ja esta no disco; ligar a janela por "
            "padrao mudaria o significado da resposta para todo cliente ja "
            "escrito. E exatamente como o Sprint 1 do SPRINTS-CASSANDRA.md "
            "estava redigido antes de a §2.1 do SPRINTS.md o reescrever."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """            // Ver o campo: desligado e o comportamento de sempre.
            exclusao_na_janela: false,
""",
        "troca": """            // DEFEITO REPOSTO: a janela ligada por padrao, que e como o
            // Sprint 1 chegou escrito -- «por_lote (o padrao): a exclusao
            // entra na janela que ja existe».
            exclusao_na_janela: true,
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "exclusao-na-janela-pelo-config"],
        "caem": [
            "config_sem_o_campo_continua_esperando_o_disco",
        ],
        "seguem": [
            "pedido_no_config_o_valor_chega_ao_motor",
            "o_campo_esta_na_lista_que_a_tela_monta",
        ],
    },
    # 13. O campo que ninguem le
    # -----------------------------------------------------------------------
    {
        "id": "exclusao-na-janela-sem-leitor",
        "titulo": "`exclusao_na_janela` no config.json, no MANUAL e na tela — e ninguém o lê",
        "porque": (
            "a armadilha do `recursos.cache_paginas`, que passou tres versoes "
            "prometendo um cache que nao existia. Campo de configuracao sem "
            "leitor e pior que campo ausente: o ausente ninguem ajusta "
            "esperando efeito."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """        phxsql_store::lixeira::definir_na_janela(self.exclusao_na_janela);
""",
        "troca": """        // DEFEITO REPOSTO: o campo existe, e nada o le.
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "exclusao-na-janela-pelo-config"],
        "caem": [
            "pedido_no_config_o_valor_chega_ao_motor",
        ],
        "seguem": [
            "config_sem_o_campo_continua_esperando_o_disco",
            "o_campo_esta_na_lista_que_a_tela_monta",
        ],
    },
    # 14. O `.reg` fechando antes do `.trash`
    # -----------------------------------------------------------------------
    {
        "id": "reg-fecha-antes-do-trash",
        "titulo": "a janela sincroniza o `.reg` antes do `.trash`",
        "porque": (
            "com a exclusao na janela os dois passam a fechar em "
            "`Table::sincronizar`, e fechar o `.reg` primeiro e escolher, de "
            "proposito, a unica ordem em que uma queda no meio do fechamento "
            "deixa a linha liberada sem a copia de recuperacao. Ver "
            "docs/DESEMPENHO.md §4.12 -- o quarto caso."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        self.lixeira.sincronizar()?;
        self.bin.sincronizar()?;
        self.memo.sincronizar()?;
        self.log.sincronizar()?;
        self.motivos.sincronizar()?;
        self.trilha.sincronizar()?;
        self.ndx.sincronizar()?;
        self.reg.sincronizar()?;
""",
        "troca": """        // DEFEITO REPOSTO: a ordem antiga, com o `.reg` na frente.
        self.reg.sincronizar()?;
        self.ndx.sincronizar()?;
        self.bin.sincronizar()?;
        self.memo.sincronizar()?;
        self.log.sincronizar()?;
        self.lixeira.sincronizar()?;
        self.motivos.sincronizar()?;
        self.trilha.sincronizar()?;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "exclusao"],
        "caem": [
            "o_trash_fecha_antes_do_reg",
        ],
        "seguem": [
            "sem_pedir_a_janela_cada_exclusao_espera_o_disco",
            "a_lixeira_esta_no_disco_antes_de_o_slot_sair",
        ],
    },
    # 15. O rodizio do Profiler ignorando o zero
    # -----------------------------------------------------------------------
    {
        "id": "rodizio-do-profiler-ignora-o-zero",
        "titulo": "`profiler.arquivo_mib: 0` deixa de querer dizer «sem rodízio»",
        "porque": (
            "o rodizio do .txt nasceu LIGADO, e a saida de quem quer o "
            "comportamento de antes e escrever zero. Se o zero deixar de ser "
            "lido, essa saida some sem ninguem perceber -- e o campo passa a "
            "dizer uma coisa e fazer outra, que e a armadilha da configuracao "
            "que mente."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": """        if self.teto_do_arquivo == 0 || self.arquivo.is_none() {
""",
        "troca": """        // DEFEITO REPOSTO: o zero deixa de desligar o rodizio.
        if self.arquivo.is_none() {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::teto_zero_nao_rodizia",
        ],
        "seguem": [
            "profiler::testes::o_rodizio_poe_teto_no_disco",
            "profiler::testes::o_sem_sufixo_e_sempre_o_mais_novo",
        ],
    },
    # 16. O cabecalho do rodizio aceitando linha forjada
    # -----------------------------------------------------------------------
    {
        "id": "cabecalho-do-profiler-forjado",
        "titulo": "o cabeçalho do arquivo do Profiler aceita linha forjada",
        "porque": (
            "o furo ORIGINAL era do cabecalho de `ligar`, e so apareceu ao "
            "escrever o rodizio: ele interpola a descricao do filtro, e o "
            "filtro vem do pedido. Um `\"operacao\"` com quebra de linha "
            "dentro poe no .txt uma segunda linha que se le como evento de "
            "outro IP -- exatamente o defeito que o EVENTO ja fechava."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": """            de_uma_linha(&descrever(&self.filtro), TETO_DO_CABECALHO)
""",
        "troca": """            // DEFEITO REPOSTO: o filtro entra no cabecalho como veio.
            descrever(&self.filtro)
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::o_cabecalho_do_rodizio_nao_aceita_linha_forjada",
        ],
        "seguem": [
            "profiler::testes::quebra_de_linha_no_pedido_nao_forja_linha_no_arquivo",
            "profiler::testes::o_rodizio_poe_teto_no_disco",
        ],
    },
    # 17. O profiler sem descritor voltando calado
    # -----------------------------------------------------------------------
    {
        "id": "profiler-sem-descritor-calado",
        "titulo": "sem descritor, com arquivo pedido, a linha some sem ser contada",
        "porque": (
            "e o defeito do disco cheio voltando pela porta do rodizio: um "
            "rodizio que nao consegue reabrir o arquivo deixa o profiler sem "
            "descritor, e a tela seguiria dizendo «gravando em ...» com nada "
            "sendo gravado -- medido antes: 400 pedidos, 223 linhas."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": """                if !self.caminho.as_os_str().is_empty() {
                    self.falhas_de_escrita += 1;
                }
                return;
""",
        "troca": """                // DEFEITO REPOSTO: volta calada, com arquivo pedido ou sem.
                return;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::sem_descritor_com_arquivo_pedido_a_perda_e_contada",
        ],
        "seguem": [
            "profiler::testes::sem_arquivo_pedido_nao_ha_falha_a_contar",
            "profiler::testes::linha_que_o_disco_recusa_e_contada",
        ],
    },
    # 18. A trava de dados presa atras de uma leitura de rede
    # -----------------------------------------------------------------------
    {
        "id": "trava-atras-da-rede",
        "titulo": "o laço da réplica segura a trava de dados enquanto lê do soquete",
        "porque": (
            "achado da bancada de conteiner e reproduzido no loopback: com um "
            "corte SILENCIOSO, `ping` na replica respondia em 4 ms e `varrer` "
            "em 30.079 ms -- o servidor no ar e sem atender dado nenhum. No "
            "bidirecional os dois lados se trancavam um ao outro sem corte "
            "nenhum, 14x mais lento e com EAGAIN de 30 s no diario de cada. "
            "A regra que sai daqui: nenhuma leitura de rede acontece com a "
            "trava de dados na mao."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": HOJE_ALCANCAR_TABELA,
        "troca": DEFEITO_ALCANCAR_TABELA,
        "pacote": "phxsql-server",
        "alvo": ["--test", "trava-atras-da-rede"],
        "caem": [
            "source_mudo_nao_prende_a_trava_de_dados",
        ],
        # O teste do comportamento VELHO. Sem ele, um conserto que quebrasse a
        # replicacao inteira passaria com louvor no de cima: laco que nao
        # replica nada tambem nao segura trava nenhuma.
        "seguem": [
            "com_a_rede_sa_a_replica_conversa_e_o_servidor_atende",
        ],
        # Medido: 9,4 s com o defeito reposto (8 s de sonda pendurada + o
        # arranque), contra 1,3 s com a arvore limpa. O prazo tem de ser MAIOR
        # que a soma dos dois, senao o executor mata a rodada antes de o teste
        # conseguir reprovar -- a mesma licao do `sujas-com-a-trava`.
        "prazo": 120,
    },
    # 19. A cifra do fio: o segredo todo-zeros aceito como chave de sessao
    # -----------------------------------------------------------------------
    {
        "id": "ordem-pequena-aceita",
        "titulo": "o segredo X25519 todo-zeros aceito como chave de sessão",
        "porque": (
            "regra da casa: criptografia se confere contra vetor oficial, e o "
            "que a RFC 7748 secao 6.1 chama de opcional aqui NAO e: as chaves "
            "de sessao saem deste segredo, e um ponto de ordem pequena faz os "
            "dois lados fecharem o aperto sem ninguem ter provado nada."
        ),
        "arquivo": "crates/phxsql-core/src/x25519.rs",
        "trecho": """    let k = multiplicar(privada, publica);
    if iguais_em_tempo_constante(&k, &[0u8; CHAVE_LEN]) {
        return Err(PhxError::Autorizacao(
            "chave publica de ordem pequena: o segredo compartilhado sairia \\
             todo-zeros, e um aperto assim fecha sem ninguem provar nada"
                .into(),
        ));
    }
    Ok(k)
""",
        "troca": """    // DEFEITO REPOSTO: a recusa do ponto de ordem pequena vira comentario, que
    // e exatamente o que a RFC permite para o Diffie-Hellman puro -- e que
    // aqui entrega o tunel a quem escolher a efemera.
    Ok(multiplicar(privada, publica))
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "x25519::testes::ponto_de_ordem_pequena_e_recusado",
            "fio::testes::efemera_de_ordem_pequena_derruba_o_aperto",
        ],
        "seguem": [
            "x25519::testes::vetor_1_da_secao_5_2",
            "x25519::testes::diffie_hellman_da_secao_6_1",
            "fio::testes::aperto_fecha_e_os_dois_lados_derivam_o_mesmo",
        ],
    },
    # 20. A cifra do fio: o contador do nonce que nao anda
    # -----------------------------------------------------------------------
    {
        "id": "contador-do-fio-parado",
        "titulo": "o contador de registros do fio parado — nonce repetido",
        "porque": (
            "secao 3 do docs/CIFRA-DO-FIO.md: repetir o par (chave, nonce) e o "
            "unico jeito de quebrar isto sem quebrar a matematica. O contador "
            "por direcao e o que impede -- e sem ele o registro repetido volta "
            "a abrir, que e replay puro."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """        let nonce = nonce_do_contador(self.n);
        self.n += 1;
        Ok(nonce)
""",
        "troca": """        // DEFEITO REPOSTO: o contador nao anda. Cada registro sai e entra com o
        // nonce zero, entao o par (chave, nonce) se repete a cada linha.
        Ok(nonce_do_contador(self.n))
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        # Tres, e nao quatro -- e o corte foi MEDIDO, nao suposto.
        #
        # A primeira versao desta entrada listava tambem o `canal_leva_e_traz`,
        # e o executor devolveu NAO PEGOU. Investigado: com o contador parado,
        # os DOIS lados usam nonce zero em todo registro, entao uma conversa
        # que vai e volta uma vez continua fechando -- ela nao repete registro
        # nenhum, que e o unico jeito de sentir a falta do contador. O teste
        # nao esta errado; errada estava a minha conta de quatro.
        "caem": [
            "fio::testes::registro_repetido_nao_abre",
            "fio::testes::registro_fora_de_ordem_nao_abre",
            "fio::testes::contador_no_teto_recusa_em_vez_de_repetir",
        ],
        "seguem": [
            "fio::testes::aperto_fecha_e_os_dois_lados_derivam_o_mesmo",
            "fio::testes::registro_mexido_nao_abre",
            "fio::testes::canal_leva_e_traz",
        ],
    },
    # 21. A cifra do fio: o EOF sem despedida virando fim limpo
    # -----------------------------------------------------------------------
    {
        "id": "fio-cortado-vira-fim",
        "titulo": "o fio cortado no meio devolvido como fim de conversa",
        "porque": (
            "secao 4 do docs/CIFRA-DO-FIO.md: a camada de registro tem de "
            "distinguir «fim de conversa» de «fio cortado no meio». Fio "
            "cortado e erro, nunca sucesso silencioso -- senao falta dado e "
            "ninguem ve faltar."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """                if lidos == 0 {
                    return if t.fim_recebido() {
                        Ok(Recebido::Fim)
                    } else {
                        Err(PhxError::Corrompido(
                            "o fio cifrado foi cortado: a conexao acabou sem a \\
                             despedida, entao pode faltar dado que ninguem viu \\
                             faltar"
                                .into(),
                        ))
                    };
                }
""",
        "troca": """                // DEFEITO REPOSTO: EOF e fim, como em claro. E a tentacao de
                // sempre -- parece que "a conexao acabou" e uma coisa so.
                if lidos == 0 {
                    return Ok(Recebido::Fim);
                }
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::fim_e_corte_sao_vereditos_diferentes",
        ],
        "seguem": [
            "fio::testes::canal_leva_e_traz",
            "fio::testes::em_claro_o_eof_continua_sendo_fim",
        ],
    },
    # 22. A cifra do fio: o `exigir` imposto em vez de pedido
    # -----------------------------------------------------------------------
    {
        "id": "cifra-do-fio-imposta",
        "titulo": "a cifra do fio EXIGIDA por padrão, quebrando todo cliente velho",
        "porque": (
            "regra petrea: guarda nova entra pedida, nao imposta. Protecao que "
            "quebra todo cliente antigo nao e protecao, e estrago -- e o teste "
            "que trava isso e o do comportamento VELHO, nao o do novo."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """        CifraFio {
            ligada: true,
            exigir: false,
""",
        "troca": """        CifraFio {
            ligada: true,
            // DEFEITO REPOSTO: exigir o tunel por padrao. Parece o padrao
            // seguro, e e o padrao que derruba toda instalacao existente no
            // primeiro pedido depois da atualizacao.
            exigir: true,
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cifra-do-fio"],
        "caem": [
            "cliente_sem_cifra_continua_como_antes",
        ],
        "seguem": [
            "com_o_aperto_o_mesmo_trabalho_acontece_cifrado",
            "exigir_recusa_texto_claro_e_deixa_o_tunel_passar",
        ],
    },
    # 23. A cifra do fio: a transcricao que nao cobre o aperto inteiro
    # -----------------------------------------------------------------------
    {
        "id": "transcricao-sem-o-cifrado",
        "titulo": "o hash da transcrição sem o texto cifrado da mensagem 2",
        "porque": (
            "secao 4 do docs/CIFRA-DO-FIO.md: o hash da transcricao tem de "
            "cobrir o aperto INTEIRO -- e o que faz a etiqueta final so fechar "
            "se as duas mensagens chegaram byte a byte como sairam."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """        let claro = cifra::abrir(&self.k, &self.nonce(), &self.h, &cifrado[..corte], &tag)?;
        self.n += 1;
        // O hash come o CIFRADO, e nao o claro: e o que o outro lado viu.
        self.misturar_hash(cifrado);
        Ok(claro)
""",
        "troca": """        let claro = cifra::abrir(&self.k, &self.nonce(), &self.h, &cifrado[..corte], &tag)?;
        self.n += 1;
        // DEFEITO REPOSTO: a transcricao para de acompanhar o que chegou. Os
        // dois lados divergem no `h` e a etiqueta seguinte nao fecha.
        Ok(claro)
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::aperto_fecha_e_os_dois_lados_derivam_o_mesmo",
            "fio::testes::pino_certo_passa_e_pino_errado_derruba",
        ],
        "seguem": [
            "fio::testes::mensagem_2_mexida_nao_autentica",
            "fio::testes::mensagem_do_tamanho_errado_e_recusada",
        ],
    },
    # 24. O teto do registro do fio, que a integracao quase perdeu
    # -----------------------------------------------------------------------
    {
        "id": "fio-sem-teto-de-registro",
        "titulo": "a leitura do fio volta a ser ilimitada",
        "porque": (
            "o teto nasceu na replica, num `read_line` com `take`; a frente da "
            "cifra trocou aquele `read_line` pelo `Canal`, que lia sem teto. "
            "Juntar as duas sem olhar devolveria a leitura ilimitada, com quem "
            "escolhe a memoria deste lado sendo o outro lado do fio. O teto "
            "desceu para o `Canal` porque la ele vale para o caminho cifrado e "
            "para o claro. A assercao e sobre QUANTO foi lido: conferir so o "
            "veredito passava com o defeito reposto, porque a conferencia vem "
            "depois da leitura -- e ai a memoria ja foi gasta."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """        let lidos = {
            let mut limitado = <&mut L as std::io::Read>::take(leitor, teto + 1);
            limitado.read_line(&mut linha)?
        };
""",
        "troca": """        // DEFEITO REPOSTO: a leitura volta a ser ilimitada.
        let lidos = leitor.read_line(&mut linha)?;
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois",
        ],
    },
    # -----------------------------------------------------------------------
    # 25. O `acrescentar_coluna` -- as quatro guardas do sprint 25
    # -----------------------------------------------------------------------
    {
        "id": "alter-compacta-o-buraco",
        "titulo": "a reescrita da coluna nova pula os slots excluídos e renumera o rowid",
        "porque": (
            "regra da casa: a ordem de digitacao e sagrada, e o `.reg` nunca "
            "reaproveita slot excluido. Pular o buraco na reescrita e a "
            "otimizacao obvia -- e ela renumera tudo depois do primeiro "
            "buraco, quebrando a ordem e todo o `.ndx` de uma vez, sem erro "
            "nenhum no caminho."
        ),
        "arquivo": "crates/phxsql-store/src/reg.rs",
        "trecho": """        de.read_exact(&mut slot)?;
        let novo = transformar(volume, primeiro_rowid + i, &slot, &nome)?;
""",
        "troca": """        de.read_exact(&mut slot)?;
        // DEFEITO REPOSTO: compacta o buraco na passagem.
        if slot[0] != STATUS_ATIVO {
            continue;
        }
        let novo = transformar(volume, primeiro_rowid + i, &slot, &nome)?;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "acrescentar-coluna"],
        "caem": [
            "a_coluna_entra_e_o_rowid_de_cada_linha_continua_o_mesmo",
        ],
        "seguem": [
            "o_indice_nao_e_tocado_e_continua_achando_a_linha",
            "sem_padrao_a_linha_antiga_recebe_nulo",
        ],
    },
    {
        "id": "alter-sem-remapear-posicao",
        "titulo": "a coluna nova desloca as de sistema e ninguém remapeia quem guarda posição",
        "porque": (
            "a armadilha nomeada do sprint 25, e a familia do `rownum`: "
            "coluna de sistema nova quebra quem filtra pela primeira. Tres "
            "coisas guardam POSICAO e nao nome -- indice, chave estrangeira e "
            "coluna de particao -- e a coluna nova empurra todas a partir "
            "dela."
        ),
        "arquivo": "crates/phxsql-core/src/schema.rs",
        "trecho": """        let desloca = |i: usize| if i >= posicao { i + 1 } else { i };
""",
        "troca": """        // DEFEITO REPOSTO: a posicao nao anda.
        let desloca = |i: usize| i;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "acrescentar-coluna"],
        "caem": [
            "indice_sobre_coluna_de_sistema_e_remapeado",
            "a_chave_estrangeira_e_remapeada",
        ],
        "seguem": [
            "a_coluna_entra_e_o_rowid_de_cada_linha_continua_o_mesmo",
            "com_softdeleted_no_meio_a_coluna_do_usuario_nao_se_move",
        ],
    },
    {
        "id": "alter-espelho-para-tras",
        "titulo": "o espelho `.bkp` fica com a largura velha depois de acrescentar coluna",
        "porque": (
            "o espelho e a segunda chance do `.reg`, e uma segunda chance com "
            "a largura errada e pior que nenhuma: a leitura que recorresse a "
            "ele leria o slot errado."
        ),
        "arquivo": "crates/phxsql-store/src/reg.rs",
        "trecho": """        for (v, _, caminho, espelho) in &primeiros {
            trocar_pelo_novo(caminho)?;
""",
        "troca": """        // DEFEITO REPOSTO: o espelho nao acompanha a troca.
        for (v, _, caminho, espelho) in &primeiros {
            let espelho: &Option<PathBuf> = &None;
            trocar_pelo_novo(caminho)?;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "acrescentar-coluna"],
        "caem": [
            "o_espelho_acompanha_e_continua_salvando",
        ],
        "seguem": [
            "a_coluna_entra_e_o_rowid_de_cada_linha_continua_o_mesmo",
        ],
    },
    {
        "id": "alter-queda-no-meio",
        "titulo": "o conjunto de volumes misturado abre e lê o volume 3 com a largura do 1",
        "porque": (
            "sem a recuperacao e a guarda, uma queda entre as trocas deixa a "
            "tabela numa roleta: cada linha do volume que ficou para tras sai "
            "deslocada da anterior, e nao ha CRC que reclame, porque os bytes "
            "lidos sao bytes de outra linha."
        ),
        "arquivo": "crates/phxsql-store/src/reg.rs",
        "trecho": """        r.terminar_troca_interrompida()?;
        r.conferir_volumes_uniformes()?;
""",
        "troca": """        // DEFEITO REPOSTO: abrir nao termina a troca nem confere os volumes.
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "acrescentar-coluna"],
        "caem": [
            "a_queda_entre_as_trocas_e_terminada_na_abertura",
            "a_queda_sem_o_novo_recusa_em_vez_de_ler_deslocado",
        ],
        "seguem": [
            "paginada_reescreve_cada_volume_e_preserva_a_ordem",
        ],
    },
]
