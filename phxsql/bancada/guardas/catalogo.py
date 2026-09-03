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
                "trecho": """            let selado = material.selar(&nonce, &aad_do_slot(volume, rowid, versao), &claro);
""",
                "troca": """            // DEFEITO REPOSTO (1/2): a etiqueta cobre o conteudo, nao o endereco.
            let selado = material.selar(&nonce, b"", &claro);
""",
            },
            {
                "arquivo": "crates/phxsql-store/src/reg.rs",
                "trecho": """    let claro = material.abrir(&nonce, &aad_do_slot(volume, rowid, versao), &guardado, nome)?;
""",
                "troca": """    // DEFEITO REPOSTO (2/2): a conferencia tambem deixa de olhar o endereco.
    let claro = material.abrir(&nonce, b"", &guardado, nome)?;
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
                "trecho": """            let selado = material.selar(&nonce, &aad_do_slot(volume, rowid, versao), &claro);
""",
                "troca": """            // DEFEITO REPOSTO (1/3): a etiqueta deixa de cobrir o endereco.
            let selado = material.selar(&nonce, b"", &claro);
""",
            },
            {
                "arquivo": "crates/phxsql-store/src/reg.rs",
                "trecho": """    let claro = material.abrir(&nonce, &aad_do_slot(volume, rowid, versao), &guardado, nome)?;
""",
                "troca": """    // DEFEITO REPOSTO (2/3): a conferencia tambem deixa de olhar o endereco.
    let claro = material.abrir(&nonce, b"", &guardado, nome)?;
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
    # -----------------------------------------------------------------------
    # 38 a 43. A fronteira de C do PhxSql embutido (crates/phxsql-ffi)
    #
    # Uma ABI e o unico lugar da casa em que um defeito nao vira teste
    # vermelho: vira o aplicativo do cliente fechando sozinho, sem log. Por
    # isso as seis entradas abaixo -- e a primeira delas e a unica de toda esta
    # lista, junto com a `cadeia-sem-teto`, que espera ABORTO em vez de falha:
    # o tamanho do estrago E a prova.
    # -----------------------------------------------------------------------
    {
        "id": "ffi-panico-atravessa",
        "titulo": "o pânico atravessa a fronteira de C em vez de virar código de erro",
        "porque": (
            "docs/EMBUTIDO.md secao 3.1: um panic desenrolando a pilha para "
            "dentro de um quadro de C e comportamento indefinido, e num "
            "aplicativo de celular ele nao aparece como erro tratavel -- "
            "aparece como o app fechando sozinho."
        ),
        "arquivo": "crates/phxsql-ffi/src/punho.rs",
        "trecho": """    match catch_unwind(AssertUnwindSafe(|| f(&mut punho.dentro))) {
        Ok(codigo) => codigo,
        Err(carga) => {
""",
        "troca": """    // DEFEITO REPOSTO: sem o catch_unwind o panico sai por uma funcao
    // `extern "C"`, e o processo aborta em vez de devolver PHX_ERRO_PANICO.
    // O binario de teste inteiro cai junto -- e esse e o tamanho do estrago.
    #[allow(clippy::unnecessary_wraps)]
    fn sem_rede<R>(r: R) -> std::result::Result<R, Box<dyn std::any::Any + Send>> {
        Ok(r)
    }
    match sem_rede(f(&mut punho.dentro)) {
        Ok(codigo) => codigo,
        Err(carga) => {
""",
        "pacote": "phxsql-ffi",
        "alvo": ["--lib"],
        "espera": "aborta",
        "caem": [
            "testes::panico_nao_atravessa_a_fronteira",
            "testes::panico_envenena_o_punho_e_so_o_fechar_passa",
        ],
        "seguem": [],
        "prazo": 300,
    },
    {
        "id": "ffi-panico-nao-envenena",
        "titulo": "o punho continua sendo usado depois de um pânico capturado",
        "porque": (
            "capturar o panico salva o processo e NAO conserta o objeto: um "
            "panico no meio de um inserir pode ter deixado o .reg com o "
            "cabecalho gravado e o payload nao. E a mesma licao do "
            "`aplicar_evento`, que PARA quando a replica divergiu."
        ),
        "arquivo": "crates/phxsql-ffi/src/punho.rs",
        "trecho": """            punho.envenenado = true;
            anotar(
                PHX_ERRO_PANICO,
""",
        "troca": """            // DEFEITO REPOSTO: o punho volta ao trabalho como se nada
            // tivesse acontecido, sobre um objeto que pode estar pela metade.
            anotar(
                PHX_ERRO_PANICO,
""",
        "pacote": "phxsql-ffi",
        "alvo": ["--lib"],
        "caem": [
            "testes::panico_envenena_o_punho_e_so_o_fechar_passa",
        ],
        "seguem": [
            "testes::panico_nao_atravessa_a_fronteira",
            "testes::ciclo_basico_grava_le_e_varre",
        ],
    },
    {
        "id": "ffi-texto-ate-o-byte-zero",
        "titulo": "a fronteira trunca o dado do cliente no primeiro byte zero",
        "porque": (
            "docs/EMBUTIDO.md secao 3.5: dado de cliente TEM byte zero -- um "
            "Bin e binario por definicao, um Memo colado de arquivo pode ter "
            "\\0 no meio. Um NUL-terminado grava metade e nao avisa, que e a "
            "pior classe de defeito: a que nao da erro."
        ),
        "arquivo": "crates/phxsql-ffi/src/texto.rs",
        "trecho": """    Some(std::slice::from_raw_parts(p, tam))
}
""",
        "troca": """    // DEFEITO REPOSTO: para no primeiro byte zero, como faria um strlen.
    // O `tam` que o chamador deu vira teto em vez de verdade.
    let cru = std::slice::from_raw_parts(p, tam);
    let ate = cru.iter().position(|b| *b == 0).unwrap_or(tam);
    Some(&cru[..ate])
}
""",
        "pacote": "phxsql-ffi",
        "alvo": ["--lib"],
        # A replicacao cai JUNTO, e isso foi medido, nao suposto: a primeira
        # versao desta entrada a listava em `seguem` e o executor devolveu
        # ESTRAGOU. Faz sentido -- a imagem de um evento e payload cru do
        # `.reg`, cheio de bytes zero, e ela entra pelo mesmo `bytes()`. O
        # truncamento no byte zero nao quebra so o memo do usuario: quebra a
        # sincronia inteira.
        "caem": [
            "testes::byte_zero_no_dado_do_cliente_sobrevive",
            "testes::replicacao_de_ponta_a_ponta_pela_abi",
        ],
        "seguem": [
            "testes::ciclo_basico_grava_le_e_varre",
            "testes::cursor_atravessa_a_fronteira_do_lote",
        ],
    },
    {
        "id": "ffi-erro-global",
        "titulo": "a mensagem de erro é global e uma thread lê o erro da outra",
        "porque": (
            "docs/EMBUTIDO.md secao 3.2: a vaga do ultimo erro e por thread "
            "pelo mesmo motivo do `errno`. Global, duas threads escrevendo "
            "fazem uma ler a mensagem da outra -- e o diagnostico passa a "
            "apontar para o lugar errado justamente quando ha concorrencia."
        ),
        "arquivo": "crates/phxsql-ffi/src/erro.rs",
        "trecho": """thread_local! {
    /// A mensagem do ultimo erro DESTA thread.
    static ULTIMO: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Guarda a mensagem e devolve o codigo, para o chamador escrever
/// `return anotar(...)` numa linha so.
pub fn anotar(codigo: i32, mensagem: impl Into<String>) -> i32 {
    let m = mensagem.into();
    ULTIMO.with(|u| *u.borrow_mut() = m);
    codigo
}

/// Traduz um erro do motor no codigo publico dele, guardando o texto.
pub fn do_motor(e: &PhxError) -> i32 {
    anotar(e.codigo() as i32, e.to_string())
}

/// O que `phx_ultimo_erro` entrega. Vazio quando nada falhou nesta thread.
pub fn ultimo() -> String {
    ULTIMO.with(|u| u.borrow().clone())
}

/// Limpa a vaga. Toda entrada da ABI comeca por aqui, para que uma mensagem
/// velha nunca seja lida como se fosse do erro de agora.
pub fn limpar() {
    ULTIMO.with(|u| u.borrow_mut().clear());
}
""",
        "troca": """// DEFEITO REPOSTO: uma vaga so para o processo inteiro, em vez de uma por
// thread. Duas threads escrevendo nela fazem uma ler a mensagem da outra.
static ULTIMO: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn anotar(codigo: i32, mensagem: impl Into<String>) -> i32 {
    *ULTIMO.lock().unwrap() = mensagem.into();
    codigo
}

pub fn do_motor(e: &PhxError) -> i32 {
    anotar(e.codigo() as i32, e.to_string())
}

pub fn ultimo() -> String {
    ULTIMO.lock().unwrap().clone()
}

pub fn limpar() {
    ULTIMO.lock().unwrap().clear();
}
""",
        "pacote": "phxsql-ffi",
        "alvo": ["--lib"],
        "caem": [
            "testes::ultimo_erro_e_por_thread",
        ],
        "seguem": [
            "testes::ciclo_basico_grava_le_e_varre",
        ],
    },
    {
        "id": "ffi-rowid-fora-e-erro",
        "titulo": "«não há essa linha» volta de duas formas diferentes conforme o motivo",
        "porque": (
            "achado pelo programa em C na PRIMEIRA rodada dele, e nao lendo o "
            "codigo: dentro do motor um slot livre devolve Ok(None) e um rowid "
            "alem do fim devolve NaoEncontrado. A diferenca e real la dentro e "
            "invisivel para quem chama -- sem a dobra o aplicativo mostra "
            "caixa vermelha para metade dos «nao achei»."
        ),
        "arquivo": "crates/phxsql-ffi/src/lib.rs",
        "trecho": """        resultado_do_rowid(x.t.ler(rowid), |l| match l {""",
        "troca": """        // DEFEITO REPOSTO: o NaoEncontrado do motor atravessa como erro 3001.
        resultado(x.t.ler(rowid), |l| match l {""",
        "pacote": "phxsql-ffi",
        "alvo": ["--lib"],
        "caem": [
            "testes::rowid_que_nao_existe_e_sempre_nao_ha_seja_qual_for_o_motivo",
        ],
        "seguem": [
            "testes::ciclo_basico_grava_le_e_varre",
        ],
    },
    {
        "id": "ffi-cursor-para-no-lote",
        "titulo": "o cursor entrega só o primeiro lote e diz que a tabela acabou",
        "porque": (
            "o cursor de digitacao anda em lotes pelo keyset do .reg para nao "
            "materializar um milhao de rowids na memoria de um celular. Parar "
            "na fronteira do lote entrega a tabela pela metade -- e sem erro "
            "nenhum, que e o que faz ninguem perceber."
        ),
        "arquivo": "crates/phxsql-ffi/src/lib.rs",
        # O defeito e mirado: nao "o cursor nao anda", que quebraria tudo e
        # provaria nada (a primeira versao desta entrada devolveu ESTRAGOU,
        # derrubando ate o ciclo basico de duas linhas). E "o cursor busca UMA
        # vez" -- numa tabela pequena ninguem nota, e a de 519 linhas para em
        # 256.
        "trecho": """                    if (ids.len() as u64) < LOTE_CURSOR {
                        cur.esgotado = true;
                    }
""",
        "troca": """                    // DEFEITO REPOSTO: um lote so, e acabou. Numa tabela
                    // menor que o lote isto nao muda nada -- e por isso passa.
                    cur.esgotado = true;
""",
        "pacote": "phxsql-ffi",
        "alvo": ["--lib"],
        "caem": [
            "testes::cursor_atravessa_a_fronteira_do_lote",
        ],
        "seguem": [
            "testes::ciclo_basico_grava_le_e_varre",
            "testes::cursor_de_indice_sai_na_ordem_do_indice",
        ],
    },
    # -----------------------------------------------------------------------
    # 26. O texto colado nas seis colunas de idioma
    # -----------------------------------------------------------------------
    {
        "id": "texto-colado-nos-seis",
        "titulo": "a mesma frase colada nas seis colunas de idioma",
        "porque": (
            "a catraca de cima conta o que ainda NAO passa pela fabrica; esta "
            "conta o contrario -- o que passa pela fabrica e mesmo assim nao "
            "esta traduzido. Colar o portugues nas seis colunas faz o numero "
            "da cobertura subir e a tela continuar em portugues, que e o pior "
            "dos dois mundos: a conta que dirige a proxima leva passa a mentir."
        ),
        "arquivo": "crates/phxsql-server/src/idiomas.rs",
        "trecho": (
            '    texto!("tela.tl_cartao_vazio", "nenhuma atividade aqui '
            '\u2014 quando houver, clique numa bolha para ver o descritivo '
            'completo", "aucune activit\u00e9 ici'
        ),
        "troca": (
            "    // DEFEITO REPOSTO: o portugues colado nas seis colunas.\n"
            '    texto!("tela.tl_cartao_vazio", "nenhuma atividade aqui '
            '\u2014 quando houver, clique numa bolha para ver o descritivo '
            'completo", "nenhuma atividade aqui \u2014 quando houver, clique '
            'numa bolha para ver o descritivo completo", "nenhuma atividade '
            'aqui \u2014 quando houver, clique numa bolha para ver o '
            'descritivo completo", "nenhuma atividade aqui \u2014 quando '
            'houver, clique numa bolha para ver o descritivo completo", '
            '"nenhuma atividade aqui \u2014 quando houver, clique numa bolha '
            'para ver o descritivo completo", "nenhuma atividade aqui '
            '\u2014 quando houver, clique numa bolha para ver o descritivo '
            'completo"), // "aucune activit\u00e9 ici'
        ),
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "conferidor::testes::nenhuma_chave_com_os_seis_idiomas_colados",
            # A guarda da frase longa pega o mesmo estrago pelo outro lado --
            # seis idiomas sao mais que tres --, e isso e desenho, nao
            # duplicacao: uma pega o colar INTEIRO, a outra o PARCIAL.
            "conferidor::testes::nenhuma_frase_longa_repetida_em_tres_idiomas",
        ],
        "seguem": [
            # O criterio nao e "igual ao portugues": as 33 chaves com espanhol
            # identico ao portugues (`Database`, `Profiler`, `Menu principal`)
            # continuam passando, e e por isso que a guarda pode existir.
            "conferidor::testes::a_catraca_dos_textos_fora_da_fabrica",
            "idiomas::testes::a_fabrica_e_bem_formada",
            "idiomas::testes::todo_texto_da_fabrica_e_pedido_por_alguem",
        ],
    },
    # -----------------------------------------------------------------------
    # 27. A frase longa repetida em tres idiomas
    # -----------------------------------------------------------------------
    {
        "id": "frase-longa-repetida",
        "titulo": "uma frase longa repetida em três das seis colunas de idioma",
        "porque": (
            "e o colar PARCIAL, que a guarda dos seis nao pega: quem traduz "
            "tres colunas e cola o portugues nas outras tres passa por ela. "
            "Duas linguas coincidirem numa palavra e comum; tres coincidirem "
            "numa frase de mais de 25 caracteres de MIOLO (o texto sem os "
            "marcadores), nao."
        ),
        "arquivo": "crates/phxsql-server/src/idiomas.rs",
        "trecho": (
            '"in chiusura\u2026 l\'operazione si interrompe al prossimo '
            'punto sicuro.", "wird beendet\u2026 die Operation bricht am '
            'n\u00e4chsten sicheren Punkt ab.", "finalizando\u2026 la '
            'operaci\u00f3n aborta en el pr\u00f3ximo punto seguro."'
        ),
        # DEFEITO REPOSTO: o italiano e o espanhol recebem o portugues. Sao
        # DUAS colunas, e nao uma, porque com uma so seriam dois idiomas
        # iguais -- e dois nao e o defeito: duas linguas irmas coincidirem e
        # comum. A guarda comeca a valer no TERCEIRO.
        "troca": (
            '"encerrando\u2026 a opera\u00e7\u00e3o aborta no pr\u00f3ximo '
            'ponto seguro.", "wird beendet\u2026 die Operation bricht am '
            'n\u00e4chsten sicheren Punkt ab.", "encerrando\u2026 a '
            'opera\u00e7\u00e3o aborta no pr\u00f3ximo ponto seguro."'
        ),
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "conferidor::testes::nenhuma_frase_longa_repetida_em_tres_idiomas",
        ],
        "seguem": [
            # Com DUAS colunas iguais a guarda dos seis nao dispara -- e e essa
            # a divisao de trabalho entre as duas.
            "conferidor::testes::nenhuma_chave_com_os_seis_idiomas_colados",
            # E o molde de marcadores continua passando: o miolo de
            # `tela.tl_quem_atividade` e «peso», a mesma palavra em portugues,
            # italiano e espanhol, e ele nao pode ser acusado.
            "conferidor::testes::o_miolo_tira_os_marcadores",
        ],
    },
    {
        "id": "rest-operacao-sem-documento",
        "titulo": "operação nova no despachar que a especificação OpenAPI não documenta",
        "porque": (
            "a regra desta casa: quando um gerador depende de uma lista, a "
            "lista tem de sair do codigo. Uma especificacao que nao cobre "
            "tudo mente por omissao -- e mente com aparencia de documento "
            "oficial, que e pior que nao ter documento."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            "verificar" => self.op_verificar(p, sessao),
""",
        "troca": """            // DEFEITO REPOSTO: operacao nova no despachar, e nada a documenta.
            "verificar" | "operacao_nova_sem_documento" => self.op_verificar(p, sessao),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "rest::testes::toda_operacao_do_despachar_esta_na_especificacao",
            "catalogo::testes::o_catalogo_e_o_despachar_sao_a_mesma_lista",
        ],
        "seguem": [
            "rest::testes::toda_rota_da_especificacao_existe_no_despachar",
        ],
    },
    {
        "id": "rest-rota-fantasma",
        "titulo": "a especificação promete uma rota que o servidor não atende",
        "porque": (
            "e o outro lado do laco, e o pior dos dois: quem le acredita e "
            "integra, e descobre em producao. E a mesma armadilha da chave "
            "morta dos idiomas -- o tradutor traduz e nada muda na tela."
        ),
        "arquivo": "crates/phxsql-server/src/rest.rs",
        "trecho": """    let caminhos: Vec<(String, Json)> = OPERACOES
        .iter()
        .map(|o| (caminho_da_operacao(o), operacao_openapi(o)))
        .collect();
""",
        "troca": """    // DEFEITO REPOSTO: uma rota escrita a mao, que o servidor nao atende.
    let mut caminhos: Vec<(String, Json)> = OPERACOES
        .iter()
        .map(|o| (caminho_da_operacao(o), operacao_openapi(o)))
        .collect();
    caminhos.push(("/exportar_para_o_sap".to_string(), Json::objeto(Vec::new())));
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "rest::testes::toda_rota_da_especificacao_existe_no_despachar",
        ],
        "seguem": [
            "rest::testes::toda_operacao_do_despachar_esta_na_especificacao",
            "rest::testes::a_especificacao_e_json_valido_e_tem_as_pecas_obrigatorias",
        ],
    },
    {
        "id": "rest-nasce-ligado",
        "titulo": "o webservice REST passa a escutar numa atualização, sem ninguém pedir",
        "porque": (
            "guarda nova entra PEDIDA, nao imposta -- e porta nova tambem. Um "
            "servidor que ja roda hoje nao pode expor superficie de ataque so "
            "porque alguem trocou o binario. O teste que mais importa e o do "
            "comportamento velho."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """        Rest {
            ligado: false,
""",
        "troca": """        // DEFEITO REPOSTO: a secao ausente nasce ligada.
        Rest {
            ligado: true,
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "config::tests::config_sem_a_secao_rest_nao_escuta",
        ],
        "seguem": [
            "config::tests::o_rest_liga_sem_o_explorador",
            "config::tests::o_token_do_rest_nao_sai_nem_entra_pela_tela",
        ],
    },
    {
        "id": "rest-corpo-manda-no-caminho",
        "titulo": "o corpo do pedido REST troca a operação do caminho, em silêncio",
        "porque": (
            "o caminho e o que o operador ve no log do proxy e nas regras do "
            "firewall. Deixar o corpo mandar faz um `POST /v1/ping` ser um "
            "`excluir` no servidor e continuar um `ping` em tudo o que "
            "observa de fora."
        ),
        "arquivo": "crates/phxsql-server/src/rest.rs",
        "trecho": """    match pedido.campo("op").and_then(Json::texto) {
        Some(outro) if outro != op => {
            return Err(PhxError::Esquema(format!(
                "o caminho pede a operacao {op:?} e o corpo traz \\"op\\":{outro:?}; \\
                 no REST quem manda e o caminho -- tire o campo do corpo"
            )));
        }
        _ => {}
    }
""",
        "troca": """    // DEFEITO REPOSTO: o corpo discordante e ignorado em vez de recusado.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "rest::testes::corpo_com_outra_operacao_e_recusado",
        ],
        "seguem": [
            "rest::testes::corpo_vazio_vale_como_objeto_vazio",
            "rest::testes::corpo_que_nao_e_objeto_e_recusado",
        ],
    },
    {
        "id": "rest-filtro-so-o-campo-tabela",
        "titulo": "o filtro de tabelas do REST olha só o campo `tabela` — e a junção é a porta dos fundos",
        "porque": (
            "e literalmente o furo que a casa ja pagou quatro vezes: o portao "
            "passou a olhar um campo novo e havia operacao sem esse campo. "
            "`juntar` guarda em `a.tabela`/`b.tabela` e `unir` numa lista, "
            "entao bastaria pedir a tabela escondida como o lado B."
        ),
        "arquivo": "crates/phxsql-server/src/rest.rs",
        "trecho": """    for nome in tabelas_citadas(pedido) {
""",
        "troca": """    // DEFEITO REPOSTO: so o campo `tabela` do primeiro nivel.
    for nome in pedido
        .campo("tabela")
        .and_then(Json::texto)
        .map(str::to_string)
        .into_iter()
    {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "rest::testes::a_lista_pega_a_tabela_escondida_na_juncao_e_na_uniao",
        ],
        "seguem": [
            "rest::testes::tabela_fora_da_lista_nao_aparece",
            "rest::testes::sem_lista_de_tabelas_nada_muda",
        ],
    },
    {
        "id": "rest-fecha-sem-escoar",
        "titulo": "a recusa por lista negra é engolida por um RST, e quem foi barrado vê «connection reset»",
        "porque": (
            "achado da bancada do REST no primeiro dia. Fechar um soquete com "
            "bytes por ler faz o sistema mandar RST, e o RST descarta a "
            "resposta em voo: a recusa com o motivo e o prazo nunca chegava em "
            "quem mais precisava dela. Vale para as tres portas HTTP, e valia "
            "desde que a interface web existe."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            let _ = http::erro_json(fluxo, 403, &self.recado_de_bloqueio(&b));
            http::escoar(fluxo);
""",
        "troca": """            // DEFEITO REPOSTO: fecha sem escoar, e o RST engole a recusa.
            let _ = http::erro_json(fluxo, 403, &self.recado_de_bloqueio(&b));
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [],
        "espera": "nada muda",
        "nota_da_redundancia": (
            "confirmado: nenhum teste de unidade sente isto, e nao poderia -- "
            "o RST e do sistema operacional, e so aparece com um soquete de "
            "verdade. Quem pega e o passo 13 de `bancada/rest/provar.py`, e "
            "esta entrada existe para dizer, com o numero da rodada, que a "
            "cobertura mora la e nao aqui"
        ),
        "seguem": [
            "rest::testes::o_status_http_sai_da_faixa_do_codigo",
        ],
    },
    # -----------------------------------------------------------------------
    # 28. A transacao gravando direto no disco em vez de empilhar
    # -----------------------------------------------------------------------
    {
        "id": "transacao-nao-empilha",
        "titulo": "a transação escreve direto no disco em vez de empilhar",
        "porque": (
            "a regra que decide o desenho inteiro: nada vai a disco antes do "
            "COMMIT. Gravar ao empilhar faria o ROLLBACK ter de desfazer -- e "
            "desfazer um insert exigiria devolver o slot, que o `.reg` nunca "
            "reaproveita. O buraco seria permanente, e a replicacao teria de "
            "receber a transacao revertida para queimar o mesmo slot do outro "
            "lado."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if OPS_EMPILHAVEIS.contains(&op) {
            self.por_prazo_na_operacao(sessao);
            return Some(self.empilhar(op, p, sessao));
        }
""",
        "troca": """        // DEFEITO REPOSTO: a escrita passa direto, como se nao houvesse
        // transacao nenhuma. O ROLLBACK deixa de desfazer.
        if OPS_EMPILHAVEIS.contains(&op) {
            return None;
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::o_rollback_de_um_insert_nao_queima_slot",
            "servidor::testes_transacoes::o_commit_aplica_na_ordem_e_com_o_rowid_prometido",
            "servidor::testes_transacoes::o_savepoint_trunca_a_lista_e_a_transacao_segue",
        ],
        "seguem": [
            # O comportamento VELHO nao pode mudar nem com o defeito reposto:
            # ele nao passa por este portao.
            "servidor::testes_transacoes::sem_transacao_nada_muda",
        ],
    },
    # -----------------------------------------------------------------------
    # 29. O COMMIT confirmando uma transacao em ABORT_ONLY
    # -----------------------------------------------------------------------
    {
        "id": "commit-confirma-abortada",
        "titulo": "o COMMIT confirma uma transação que já estava em ABORT_ONLY",
        "porque": (
            "e a melhor ideia do capitulo que o dono mandou, e o motivo dela: "
            "depois de um erro de TRANSACAO o conjunto de escrita esta em "
            "duvida, e confirmar trabalho meio invalido e pior do que recusar. "
            "O `XACT_STATE()` do SQL Server e o estado abortado do "
            "PostgreSQL(R) existem exatamente para isto."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        # O ramo inteiro, para a troca fechar as chaves e os parenteses: uma
        # troca que so tira o `return Err(` deixa dois fechamentos sobrando, e
        # a guarda vira QUEBRADA -- que nao prova nada.
        "trecho": """                crate::transacao::Estado::AbortOnly => {
                    return Err(PhxError::TransacaoAbortada(format!(
                        "{}; a transacao nao pode ser confirmada -- mande ROLLBACK",
                        if tx.motivo_do_aborto.is_empty() {
                            "houve erro de TRANSACAO".to_string()
                        } else {
                            tx.motivo_do_aborto.clone()
                        }
                    )))
                }
""",
        "troca": """                // DEFEITO REPOSTO: `ABORT_ONLY` confirma como se nada
                // tivesse acontecido -- e o trabalho meio invalido vai para o
                // disco.
                crate::transacao::Estado::AbortOnly => {}
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::o_teto_leva_a_abort_only_e_o_commit_recusa",
        ],
        "seguem": [
            "servidor::testes_transacoes::erro_de_instrucao_nao_derruba_a_transacao",
        ],
    },
    # -----------------------------------------------------------------------
    # 30. A marca de commit apagada ANTES do fsync da tabela
    # -----------------------------------------------------------------------
    {
        "id": "marca-antes-do-fsync",
        "titulo": "a marca `.tx` é apagada antes de a tabela sincronizar",
        "porque": (
            "e a ordem que faz o group commit ser seguro, e ela nao se "
            "inverte: a marca e o bilhete que traz o dado de volta se a "
            "energia cair antes do `fsync`. Apaga-la antes abre a janela em "
            "que o dado nao esta no disco e nao ha bilhete nenhum -- a mesma "
            "janela sem conserto da lixeira, pelo outro lado."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                if self.tabelas_ainda_sujas(&database, &escritas) {
                    if let Ok(mut m) = self.marcas_pendentes.lock() {
                        m.push(marca.clone());
                    }
                } else {
                    let _ = std::fs::remove_file(&marca);
                }
""",
        "troca": """                // DEFEITO REPOSTO: a marca sai sempre, sem esperar o fsync.
                let _ = std::fs::remove_file(&marca);
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_marca_espera_o_fsync_e_so_entao_e_apagada",
        ],
        "seguem": [
            # Com `por_operacao` a janela fecha na propria gravacao, entao a
            # marca sairia no commit de qualquer jeito -- e este teste
            # continua verde, o que prova que o defeito e da OUTRA metade.
            "servidor::testes_transacoes::sem_janela_a_marca_sai_no_commit",
            "servidor::testes_transacoes::a_recuperacao_completa_o_commit_e_nao_duplica",
        ],
    },
    # -----------------------------------------------------------------------
    # 31. O `INSERT` sem travar o fim da tabela
    # -----------------------------------------------------------------------
    {
        "id": "insert-sem-travar-o-fim",
        "titulo": "duas transações que anexam preveem o mesmo rowid",
        "porque": (
            "o rowid E o endereco, e o proximo e `slots() + 1` -- um so. Sem "
            "travar o fim, duas transacoes que anexam ao mesmo tempo preveem "
            "o MESMO slot, e a segunda descobre isso na passada de commit, com "
            "metade do trabalho gravado."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        let rowid_pedido = match acao {
            Acao::Inserir => crate::travas::FIM_DA_TABELA,
            _ => self.rowid(p)?,
        };
""",
        "troca": """        let rowid_pedido = match acao {
            // DEFEITO REPOSTO: o INSERT trava um lugar SEU em vez do fim da
            // tabela. Dois que anexam ao mesmo tempo preveem o mesmo slot e
            // deixam de se esbarrar, porque cada um travou outra coisa.
            Acao::Inserir => sessao.ligacao,
            _ => self.rowid(p)?,
        };
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::duas_transacoes_que_anexam_disputam_o_fim_da_tabela",
        ],
        "seguem": [
            # A trava de LINHA continua inteira: o defeito e so no anexar.
            "servidor::testes_transacoes::dois_caixas_em_linhas_diferentes_nao_se_esbarram",
        ],
    },
    # -----------------------------------------------------------------------
    # 32. A recuperacao sem reconstruir o indice que a queda deixou para tras
    # -----------------------------------------------------------------------
    {
        "id": "recuperar-sem-reindexar",
        "titulo": "a recuperação não reconstrói o `.ndx` que a queda deixou para trás",
        "porque": (
            "achado pela prova por SOQUETE, e por nenhum teste unitario: um "
            "SIGKILL no meio da passada levanta a marca de «o indice ficou "
            "para tras», e enquanto ela estiver la TODA operacao de indice "
            "recusa. A recuperacao reabria a tabela, tentava inserir e recebia "
            "«reconstrua com reparar indice» -- o commit ficava pela metade e "
            "a tabela inutilizavel, sem ninguem ser avisado."
        ),
        "arquivo": "crates/phxsql-server/src/transacao.rs",
        "trecho": """                        if t.indice_precisa_reconstruir() {
                            match t.reindexar() {
                                Ok(_) => r.indices_reconstruidos += 1,
""",
        "troca": """                        // DEFEITO REPOSTO: a recuperacao nao reconstroi o
                        // indice, e o commit fica pela metade.
                        if false {
                            match t.reindexar() {
                                Ok(_) => r.indices_reconstruidos += 1,
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        # Nenhum teste unitario cai, e a entrada AFIRMA isso: quem pega este
        # defeito e o `bancada/transacoes/provar.py`, que mata o processo de
        # verdade. Deixa-lo aqui trava a afirmacao -- no dia em que um teste
        # de unidade passar a pegar, o executor avisa que a afirmacao morreu.
        "espera": "nada muda",
        "nota_da_redundancia": (
            "confirmado: nenhum teste de unidade pega este defeito. O indice "
            "so fica para tras quando o PROCESSO morre no meio da passada, e "
            "isso so acontece de verdade em `bancada/transacoes/provar.py` -- "
            "que e por isso que a prova por soquete existe."
        ),
        "caem": [],
        "seguem": [
            "servidor::testes_transacoes::a_recuperacao_completa_o_commit_e_nao_duplica",
            "servidor::testes_transacoes::marca_que_nao_confere_e_commit_que_nunca_comecou",
        ],
    },
    # -----------------------------------------------------------------------
    # 33. A escrita COMUM anexando por baixo do fim que a transacao segura
    # -----------------------------------------------------------------------
    {
        "id": "comum-anexa-no-fim-travado",
        "titulo": "a escrita comum que anexa não olha o fim travado",
        "porque": (
            "a revisao achou a corrida pelo outro lado -- a transacao pedia a "
            "trava do fim tarde demais --, e o teste defende a garantia pelos "
            "dois: quem anexa SEM transacao nenhuma tambem tem de ver o fim "
            "travado. O estrago nao e o erro no COMMIT, que e visivel: e a "
            "RECUPERACAO encontrar o slot ocupado pela linha do outro, trata-lo "
            "como «ja aplicado» e descartar a nossa em silencio."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                // Anexar disputa o FIM da tabela: o proximo slot e um so.
                "inserir" | "inserir_lote" | "importar" | "carga" | "duplicar_tabela"
                | "copiar_tabela" => {
                    travas.conflito_de_linha(&chave, meu, crate::travas::FIM_DA_TABELA)
                }
""",
        "troca": """                // DEFEITO REPOSTO: quem anexa sem transacao nenhuma passa por
                // baixo do fim travado, e o slot que a transacao prometeu vira
                // de outra linha.
                "inserir" | "inserir_lote" | "importar" | "carga" | "duplicar_tabela"
                | "copiar_tabela" => None,
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::escrita_comum_nao_anexa_enquanto_a_transacao_segura_o_fim",
        ],
        "seguem": [
            # A trava de LINHA da escrita comum continua inteira: o defeito e
            # so no anexar, e um `atualizar` no rowid travado ainda recusa.
            "servidor::testes_transacoes::sem_transacao_nada_muda",
            "servidor::testes_transacoes::dois_caixas_em_linhas_diferentes_nao_se_esbarram",
        ],
    },
    # -----------------------------------------------------------------------
    # 34. Zero dependencias externas -- achado do QA-PDCA: a petrea mais
    #     repetida do CLAUDE.md nao tinha guarda nenhuma
    # -----------------------------------------------------------------------
    {
        "id": "dependencia-de-fora-fica-invisivel",
        "titulo": "o filtro de dependência externa vira mudo (mede e nunca acusa)",
        "porque": (
            "`cargo build --offline` recusava uma dependencia de fora por "
            "ACIDENTE (crate ausente do cache local), nao por regra -- numa "
            "maquina com a crate ja em cache, ou com rede, passaria calado. "
            "Isto e a regra escrita: um conjunto de nomes contra o "
            "`Cargo.lock`, que nao depende de cache nem de conectividade."
        ),
        "arquivo": "crates/phxsql-server/src/conferidor_dependencias.rs",
        "trecho": """    pacotes
        .iter()
        .filter(|(n, _)| !permitidos.contains(n))
        .cloned()
        .collect()
""",
        "troca": """    // DEFEITO REPOSTO: a guarda desligada -- mede, e nunca acusa nada.
    let _ = permitidos;
    Vec::new()
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "conferidor_dependencias::testes::deteta_pacote_de_fora_do_workspace",
        ],
        "seguem": [
            # O Cargo.lock de VERDADE nao tem dependencia externa nenhuma
            # hoje -- entao "sempre vazio" ainda bate com "vazio de verdade"
            # nestes dois, e e por isso que a prova real deste defeito mora
            # no fixture de cima, e nao no Cargo.lock real.
            "conferidor_dependencias::testes::workspace_zero_dependencia_externa",
            "conferidor_dependencias::testes::sem_pacote_de_fora_nao_acusa_nada",
            "conferidor_dependencias::testes::os_nomes_do_workspace_batem_com_os_diretorios_de_crates",
        ],
    },
    # -----------------------------------------------------------------------
    # 35. A metade "indice na filha" da chave conferida -- a metade "indice
    #     na mae" ja tinha guarda (servidor.rs, achado #11 acima); esta nao
    # -----------------------------------------------------------------------
    {
        "id": "sem-indice-na-filha-ignora-em-vez-de-recusar",
        "titulo": "sem índice na filha, a exclusão da mãe ignora em vez de recusar",
        "porque": (
            "a regra petrea diz \"sem um deles o motor recusa dizendo qual "
            "falta\" -- e os dez testes historicos de chave-estrangeira.rs "
            "usavam todos a mesma `filha()`, que SEMPRE cria o indice da "
            "coluna da chave. Nenhum exercitava a recusa do outro lado: "
            "sem indice, a exclusao varreria a tabela de filhas inteira a "
            "cada exclusao de mae -- o custo escondido que a regra existe "
            "para impedir."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """                let Some(indice) = indice_que_cobre(filha.esquema(), &colunas) else {
                    return Err(PhxError::Integridade(format!(
                        "{eu}: nao da para conferir as filhas de {irma} pela chave \\
                         {:?}, que nao tem indice comecando por ({}) -- crie o \\
                         indice na filha ou desligue `verificar` na chave",
                        fk.nome,
                        colunas.join(", ")
                    )));
                };
""",
        "troca": """                let Some(indice) = indice_que_cobre(filha.esquema(), &colunas) else {
                    // DEFEITO REPOSTO: sem indice, ignora em vez de recusar.
                    continue;
                };
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "chave-estrangeira"],
        "caem": [
            "sem_indice_na_filha_a_recusa_diz_qual_indice_falta",
        ],
        "seguem": [
            "a_mae_aberta_e_ja_gravada_e_vista",
            "a_mae_nao_gravada_recusa_dizendo_por_que",
            "sem_conferir_a_mae_aberta_nao_muda_nada",
            "a_mae_com_filha_nao_pode_ser_apagada",
            "a_mae_sem_filha_sai_normalmente",
            "filha_de_outra_linha_nao_tranca_esta",
            "sem_conferir_a_mae_com_filha_sai_como_sempre",
            "sem_conferir_a_mae_sai_mesmo_sem_indice_na_filha",
        ],
    },
    # -----------------------------------------------------------------------
    # 36. `recursos.cache_paginas` -- o campo que deu nome a armadilha nunca
    #     ganhou o teste ponta-a-ponta que ele proprio inspirou nos irmaos
    # -----------------------------------------------------------------------
    {
        "id": "cache-paginas-nao-chega-ao-motor",
        "titulo": "`cache_paginas` do config.json deixa de chegar ao motor",
        "porque": (
            "o campo que deu nome a \"configuracao que nao e lida mente\" -- "
            "tres versoes no config.json, no MANUAL e na tela sem leitor -- "
            "nunca ganhou o teste ponta-a-ponta que os campos irmaos "
            "(`exclusao_na_janela`, `diario_volume_mib`) ganharam DEPOIS "
            "dele, citando-o como motivo. O unico teste que tocava o campo "
            "conferia o `Config` em memoria, nao o motor."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        phxsql_store::ndx::definir_cache_paginas(config.recursos.cache_paginas);
""",
        "troca": """        // DEFEITO REPOSTO: a armadilha do cache_paginas, de volta.
        // phxsql_store::ndx::definir_cache_paginas(config.recursos.cache_paginas);
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cache-paginas-pelo-config"],
        "caem": [
            "o_campo_do_config_chega_ao_motor_no_arranque",
            "um_segundo_arranque_com_outro_valor_muda_o_teto_de_novo",
        ],
        "seguem": [
            # O teste do comportamento velho continua de pe: sem o campo no
            # config.json, o teto ja nasce no padrao mesmo sem a linha --
            # e o que prova que este `caem` nao e um portao que recusa tudo.
            "config_sem_o_campo_sobe_com_o_teto_padrao",
        ],
    },
    # -----------------------------------------------------------------------
    # A replica APLICA, ela nao JULGA -- os quatro defeitos que a sonda mediu
    # -----------------------------------------------------------------------
    {
        "id": "replica-julga-fk",
        "titulo": "a replica volta a conferir chave estrangeira no evento que aplica",
        "porque": (
            "a replicacao anda por TABELA, cada uma com a sua posicao, e nao "
            "existe ordem global entre tabelas. Conferindo, a replica recusava "
            "a filha que a ORIGEM ja tinha aceitado: medido em "
            "`--example sonda-replica-fk`, `pedidos` ficava com 0 dos 2 "
            "eventos nas ordens \"mae primeiro\" e \"filha primeiro\". A guarda "
            "causava a perda de dado que existe para impedir."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        self.conferir_aridade(valores)?;
        if !self.fks_conferidas.is_empty() && self.julga_integridade() {
            self.conferir_fks(valores)?;
        }
        // Numerar ANTES das chaves""",
        "troca": """        // DEFEITO REPOSTO: a replica volta a julgar o que a origem ja julgou.
        self.conferir_aridade(valores)?;
        if !self.fks_conferidas.is_empty() {
            self.conferir_fks(valores)?;
        }
        // Numerar ANTES das chaves""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "replicacao-integridade"],
        "caem": [
            "a_replica_converge_nas_tres_ordens_de_tabela",
            # Este cai junto por consequencia, e nao por acaso: ele aplica um
            # evento com a mae AUSENTE de proposito, para so entao provar que a
            # marca nao vaza. Com o portao de volta o evento nem entra.
            "a_marca_de_replica_nao_vaza_para_a_escrita_local",
        ],
        "seguem": [
            # A imagem do evento da cascata e outra garantia, e nao se mexe:
            # sem este `seguem` a troca poderia estar quebrando o arquivo todo.
            "o_evento_da_cascata_carrega_a_imagem_da_linha",
        ],
    },
    {
        "id": "cascata-sem-imagem-no-diario",
        "titulo": "a filha que a cascata abre volta a nascer sem imagem no diario",
        "porque": (
            "a cascata do `ao_alterar` grava na filha por um handle proprio, "
            "aberto pelo motor. Nascendo com o padrao, o evento de alteracao "
            "da filha ia para o diario SEM a imagem da linha, e a replica o "
            "recusava com \"veio sem imagem\" nas TRES ordens. Quem replica liga "
            "a imagem na tabela que abre; a que o motor abre por baixo tem de "
            "sair igual, senao a garantia vale so para quem passou pela mao de "
            "quem ligou."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """                filha.ligar_imagem_no_diario(self.imagem_no_diario);
                filha.ligar_imagem_na_exclusao(self.imagem_na_exclusao);
""",
        "troca": """                // DEFEITO REPOSTO: a filha da cascata nao herda mais a imagem.
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "replicacao-integridade"],
        "caem": [
            "o_evento_da_cascata_carrega_a_imagem_da_linha",
            "a_replica_converge_nas_tres_ordens_de_tabela",
        ],
        "seguem": [
            "a_marca_de_replica_nao_vaza_para_a_escrita_local",
        ],
    },
    {
        "id": "replica-refaz-a-cascata",
        "titulo": "a replica volta a refazer a cascata que o source ja mandou",
        "porque": (
            "o source cascateia e replica o evento que a cascata dele gerou. "
            "Refazendo aqui, a replica grava a filha duas vezes e deixa no "
            "diario dela um evento que o source nunca mandou -- divergencia "
            "medida, e ela some de qualquer prova que compare so o VALOR da "
            "linha em vez do diario."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        let mut cascata = if self.julga_integridade() {
            self.planejar_ao_alterar(&valores_antigos, valores)?
        } else {
            Vec::new()
        };""",
        "troca": """        // DEFEITO REPOSTO: a replica planeja e roda a cascata de novo.
        let mut cascata = self.planejar_ao_alterar(&valores_antigos, valores)?;""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "replicacao-integridade"],
        "caem": [
            # So o teste que conta os EVENTOS pega: o valor da linha fica certo
            # dos dois jeitos, e e por isso que o defeito sobreviveu ate a
            # sonda contar o diario.
            "a_replica_converge_nas_tres_ordens_de_tabela",
        ],
        "seguem": [
            "o_evento_da_cascata_carrega_a_imagem_da_linha",
            "a_marca_de_replica_nao_vaza_para_a_escrita_local",
        ],
    },
    {
        "id": "marca-de-replica-fica-acesa",
        "titulo": "a marca de replica nao se apaga na volta do `aplicar_evento`",
        "porque": (
            "a marca e de UM evento, e nao do handle. Sem o par liga/desliga, "
            "o handle que aplicou um evento continuaria \"sendo replica\" e "
            "pararia de conferir integridade na escrita LOCAL seguinte -- um "
            "portao que se apaga sozinho e pior que portao nenhum, porque "
            "ninguem procura por ele."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        let r = self.aplicar_evento_interno(operacao, rowid, imagem);
        self.como_replica = false;
        r""",
        "troca": """        // DEFEITO REPOSTO: a marca fica acesa depois do evento.
        self.aplicar_evento_interno(operacao, rowid, imagem)""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "replicacao-integridade"],
        "caem": [
            "a_marca_de_replica_nao_vaza_para_a_escrita_local",
        ],
        "seguem": [
            "a_replica_converge_nas_tres_ordens_de_tabela",
            "o_evento_da_cascata_carrega_a_imagem_da_linha",
        ],
    },
    {
        "id": "fk-nao-pergunta-se-a-mae-esta-viva",
        "titulo": "a conferencia da chave volta a perguntar so se a mae EXISTE",
        "porque": (
            "a mae excluida de forma SUAVE continua no `.reg` com a chave dela "
            "no indice. Perguntando so «existe?», a filha nascia apontando "
            "para um cliente que a tela nao mostra mais -- a orfa por "
            "construcao. E o outro lado do tempo da petrea do `excluir_suave`, "
            "que ja confere as filhas pela mesma frase: a casa fechava a porta "
            "e deixava a janela."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """            if mae.esquema.coluna_softdeleted().is_some() {
                let mut viva = false;""",
        "troca": """            // DEFEITO REPOSTO: existir volta a valer por estar viva.
            if false {
                let mut viva = false;""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "chave-estrangeira"],
        "caem": [
            "a_filha_nao_nasce_apontando_para_mae_excluida_suave",
            "o_atualizar_tambem_nao_aponta_a_filha_para_mae_morta",
            # Este entrou como `seguem` e o executor devolveu ESTRAGOU: ele
            # confere a recusa ANTES de restaurar a mae, entao ele tambem pega
            # o defeito. Medido, e nao lido -- e o terceiro caso desta casa em
            # que "este teste pega aquele defeito" so vale depois de rodar.
            "mae_restaurada_volta_a_aceitar_filha",
        ],
        "seguem": [
            # O controle e o comportamento VELHO: sem eles, um portao que
            # recusasse toda gravacao passaria pelos `caem` acima.
            "a_mae_viva_continua_aceitando_filha",
            "sem_conferir_a_mae_morta_nao_tranca_nada",
        ],
    },
]
