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
    # 6. O abraco mortal: `descarregar_sujas()` com a trava de dados na mao
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
            "acusa nada."
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
    # 7. A cadeia de gatilhos sem teto -- o unico que ABORTA o processo
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
    # 8. `excluir_tabela` com a lista curta de extensoes
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
    # 9. A conferencia de SHA-256 do backup desligada
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
    # 10. O AAD fora do slot cifrado
    # -----------------------------------------------------------------------
    # -----------------------------------------------------------------------
    # 10. A amarracao do corpo cifrado ao ENDERECO -- e as duas fechaduras
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
    # 12. A catraca dos textos fora da fabrica
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
    # 13. A exclusao na janela virando o PADRAO
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
    # -----------------------------------------------------------------------
    # 14. O campo que ninguem le
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
    # -----------------------------------------------------------------------
    # 15. O `.reg` fechando antes do `.trash`
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
]
