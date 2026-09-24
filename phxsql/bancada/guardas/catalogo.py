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

TRECHO_PAGINA_ORDENADA = """        let i = self.idx_por_nome(indice)?;
        // `limite` zero quer dizer «tudo», e ai nao ha onde parar: o pedaco e o
        // indice inteiro e o laco roda uma volta so.
        let mut pedaco = if limite == 0 {
            0
        } else {
            (pular as usize).saturating_add(limite as usize)
        };
        // `Todas` sem sobreposicao nao esconde nada: nao ha o que ler para
        // decidir, e o recorte e direto na lista de rowids.
        let so_recorta = visao == Visao::Todas && self.sobreposta.is_none();
        let mut apos: Option<Vec<u8>> = None;
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        loop {
            let (entradas, ultima, acabou) = self.ndx.varrer_apos(i, apos.as_deref(), pedaco)?;
            apos = ultima;
            // Os nascidos na transacao entram no FIM: eles nao tem lugar na
            // ordem da chave, e por isso so aparecem quando o indice acabou.
            // O porque esta em `varrer_indice`.
            let nascidos = if acabou { self.nascidos() } else { Vec::new() };
            for r in entradas.into_iter().chain(nascidos) {
                if !so_recorta && !self.visivel(r, None, visao)? {
                    continue;
                }
                if vistos >= pular {
                    saida.push(r);
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
            }
            if acabou {
                return Ok(saida);
            }
            pedaco = pedaco.saturating_mul(2);
        }
    }"""

TROCA_PAGINA_ORDENADA = """        // DEFEITO REPOSTO (pedido 188): a varredura do indice INTEIRO antes
        // de qualquer recorte. O `break` do limite para a leitura das LINHAS
        // do `.reg`, e nunca a varredura do `.ndx` -- por isso 50 linhas
        // custavam o mesmo que 1.000.
        let todos = self.varrer_indice(indice)?;
        if visao == Visao::Todas && self.sobreposta.is_none() {
            return Ok(todos
                .into_iter()
                .skip(pular as usize)
                .take(if limite == 0 { usize::MAX } else { limite as usize })
                .collect());
        }
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for r in todos {
            if !self.visivel(r, None, visao)? {
                continue;
            }
            if vistos >= pular {
                saida.push(r);
                if limite > 0 && saida.len() as u64 >= limite {
                    break;
                }
            }
            vistos += 1;
        }
        Ok(saida)
    }"""

TRECHO_CURSOR = """        if apos.is_some() {
            pos += 1;
        }"""

TROCA_CURSOR = """        // DEFEITO REPOSTO (pedido 188): o cursor sem o `+1` devolve de novo a
        // entrada que ja foi entregue na volta anterior.
        if false {
            pos += 1;
        }"""

DEFEITO_ALCANCAR_TABELA = """        let mut aplicados = 0u64;
        while posicao < no.eventos {
            // DEFEITO REPOSTO: a trava de dados e tomada ANTES da leitura de
            // rede e segurada durante ela -- e no meio do laco mora
            // `replica::puxar`, que e uma ida e volta de rede. Rede sa
            // esconde; source mudo prende o servidor inteiro ate o prazo de
            // leitura de 30 s estourar.
            let presa_atras_da_rede = self.travar_dados()?;
            let desde = posicao.saturating_sub(1);
            let eventos = crate::replica::puxar(cliente, database, &no.nome, desde)?;
            drop(presa_atras_da_rede);
            if eventos.is_empty() {
                break;
            }
"""

# REANCORADO em 17/09/2026 (onda 3, papel G): o commit `49a3af7` (papel B,
# item 5, continuidade da replica pedida pelo papel C) reescreveu
# `alcancar_tabela` -- ela ganhou a conferencia de continuidade
# (`abrir_para_replicar` devolve so a posicao, e o `puxar` agora comeca em
# `posicao - 1` para comparar o evento de conferencia). O ponto onde a trava
# podia voltar a prender a leitura de rede continua sendo o mesmo: o `puxar`
# dentro do laco. A entrada foi reancorada ali, e nao na funcao inteira --
# o resto da funcao (a checagem de continuidade, o ramo `posicao >=
# no.eventos`) nao toca o defeito que esta guarda prova.
HOJE_ALCANCAR_TABELA = """        let mut aplicados = 0u64;
        while posicao < no.eventos {
            // FORA da trava. Se a conexao cair aqui, o lote se perde e nada
            // foi gravado: a posicao local nao andou, e a proxima rodada pede
            // exatamente os mesmos eventos. Nao ha meio-lote possivel porque
            // o lote inteiro chega antes de a trava ser pedida.
            //
            // A partir de `posicao - 1`: o primeiro evento e a conferencia.
            let desde = posicao.saturating_sub(1);
            let eventos = crate::replica::puxar(cliente, database, &no.nome, desde)?;
            if eventos.is_empty() {
                break;
            }
"""

# O ponto de reposicao ANDOU em 18/09/2026 (pedido 356): o `if self.sigiloso`
# virou `self.sigilo.no_lugar_do_pedido()`, porque o arquivo passou a ter DOIS
# motivos para calar -- a lista declarada e o `.reg` cifrado -- e um `bool` nao
# diz qual dos dois. A entrada foi conferida com o provador no mesmo passo: o
# defeito reposto continua sendo «a linha escreve `self.pedido`».
TRECHO_PERFIL_SEM_TEXTO = """            // O ARQUIVO nao leva o texto de tabela sigilosa -- ver o cabecalho
            // do modulo. A coluna de bytes ao lado ja diz o tamanho, entao o
            // que se perde e o conteudo e nao a medida: continua dando para
            // achar o pedido gigante que derrubou o servidor.
            self.sigilo
                .no_lugar_do_pedido()
                .unwrap_or(self.pedido.as_str()),"""

TROCA_PERFIL_SEM_TEXTO = """            self.pedido.as_str(),"""

TRECHO_PERFIL_PELO_DISCO = """            RegNoDisco::Cifrado | RegNoDisco::Ilegivel => true,
            RegNoDisco::EmClaro | RegNoDisco::SemVolume => false,"""
# O defeito de origem em uma linha: o disco responde e ninguem age. E a forma
# mais fiel do 356 -- a pergunta ao disco nem existia.
TROCA_PERFIL_PELO_DISCO = """            RegNoDisco::Cifrado | RegNoDisco::Ilegivel => false,
            RegNoDisco::EmClaro | RegNoDisco::SemVolume => false,"""

TRECHO_PERFIL_ERRO_SEM_TEXTO = """                (false, true) => {
                    format!("  <- <erro nao gravado, {} bytes>", self.erro.len())
                }"""
TROCA_PERFIL_ERRO_SEM_TEXTO = """                (false, true) => format!("  <- {}", self.erro),"""

TRECHO_RAIZ_NO_LIGAR = """        prof.definir_raiz_dos_dados(&self.config.base);"""
# Nao basta apagar a linha: `self.config.base` continua usado no resto do
# arquivo, mas o `let _` mantem a troca visivel para quem audita o catalogo.
TROCA_RAIZ_NO_LIGAR = """        let _ = &self.config.base;"""

TRECHO_COLHER_DESCE = """                colher_tabelas(v, meu_banco, saida);"""

TROCA_COLHER_DESCE = """                let _ = v;"""

TRECHO_FASE_FIXA = """            .map(|a| a.fase_cancelavel("somando a tabela"));"""

TROCA_FASE_FIXA = """            .map(|a| a.fase_cancelavel(&format!("somando {} linhas", linhas)));"""


TRECHO_CACHE_ESVAZIA = """    // O cache de chaves derivadas e por (sal, iteracoes) -- NAO por senha.
    // Deixando-o de pe, trocar a senha nao trocaria a chave de nenhum arquivo
    // ja aberto neste processo: `derivar` acharia a entrada do sal e
    // devolveria a chave da senha ANTIGA. Um servidor que aceitasse a senha
    // errada por ter aberto o arquivo antes seria pior que um que a recusa.
    if let Ok(mut d) = DERIVADAS.lock() {
        *d = None;
    }
    Ok(())
}"""

TROCA_CACHE_ESVAZIA = """    // (o defeito reposto: o cache sobrevive a troca de senha)
    Ok(())
}"""


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
        Ok(j @ Json::Objeto(_)) => {
            let mut alvos = Vec::new();
            colher_tabelas(&j, database, &mut alvos);
            (limpar(&j).escrever(), alvos)
        }
        // Pedido que nao e objeto nao vira texto -- vira o tamanho. E sem
        // arvore nao ha tabela a colher: a lista sai vazia, e o evento cai no
        // caminho de sempre. Nao ha o que esconder num pedido que nao virou
        // texto nenhum.
        Ok(_) => (
            format!("<pedido nao e objeto, {tamanho} bytes>"),
            Vec::new(),
        ),
        Err(_) => (format!("<pedido invalido, {tamanho} bytes>"), Vec::new()),
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
    // A colheita de tabelas FICA: o defeito reposto aqui e o recorte do
    // TEXTO. Repor os dois de uma vez esconderia qual derrubou o teste.
    let mut alvos = Vec::new();
    if let Ok(j) = Json::analisar(linha) {
        colher_tabelas(&j, database, &mut alvos);
    }
    (s, alvos)
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
        Ok(j @ Json::Objeto(_)) => {
            let mut alvos = Vec::new();
            colher_tabelas(&j, database, &mut alvos);
            (limpar(&j).escrever(), alvos)
        }
        // Pedido que nao e objeto nao vira texto -- vira o tamanho. E sem
        // arvore nao ha tabela a colher: a lista sai vazia, e o evento cai no
        // caminho de sempre. Nao ha o que esconder num pedido que nao virou
        // texto nenhum.
        Ok(_) => (
            format!("<pedido nao e objeto, {tamanho} bytes>"),
            Vec::new(),
        ),
        Err(_) => (format!("<pedido invalido, {tamanho} bytes>"), Vec::new()),
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
    // A colheita de tabelas FICA: o defeito reposto aqui e o recorte do
    // TEXTO. Repor os dois de uma vez esconderia qual derrubou o teste.
    let mut alvos = Vec::new();
    if let Ok(j) = Json::analisar(linha) {
        colher_tabelas(&j, database, &mut alvos);
    }
    (s, alvos)
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
    # E DAS TRES, quem segura e o ROWID -- medido em 03/09/2026 com
    # `medir-redundancia.py`, e nao lido. O teste que decide copia o slot 5
    # INTEIRO por cima do slot 9 (cabecalho junto, entao a versao e o tempero
    # viajam com a copia), e os dois moram no mesmo volume: dos tres valores,
    # DOIS sao iguais dos dois lados. Tirar `volume` ou `versao` de qualquer
    # uma das fechaduras nao muda nada; tirar o `rowid` de UMA delas, com a
    # outra ja tirada, derruba o teste. Quem ler «(volume, rowid, versao)» e
    # remover o rowid confiando na frase remove justamente o unico que trabalha.
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
            "porque o `nonce_de_pedaco` carrega o ROWID. Medido em 03/09/2026, "
            "e nao deduzido: tirando o AAD e SO o rowid do nonce -- volume e "
            "contador ficando --, o teste CAI. Volume e versao nao entram nesta "
            "conta porque o teste copia o slot INTEIRO, e os dois slots moram no "
            "mesmo volume com a mesma versao"
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
            "confirmado: tirar so o endereco do nonce tambem passa despercebido, "
            "porque o AAD carrega o ROWID. Medido em 03/09/2026: tirando o "
            "endereco do nonce e SO o rowid do AAD -- volume e versao ficando --, "
            "o teste CAI"
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
    {
        "id": "coluna-externa-sozinha-em-claro",
        "titulo": "tabela cujas únicas colunas marcadas são externas nasce em claro",
        "porque": (
            "pedido 210, entregue vermelho em 05/09/2026 e consertado em "
            "09/09/2026. A condicao que LIGA o material do `.reg` era derivada "
            "das faixas INLINE (`faixas.is_empty()`): coluna `Memo`/`Bin` "
            "marcada nao gera faixa, entao a tabela nascia `EM_CLARO` com o "
            "cofre ligado e o `selar_externo` -- escrito e certo -- nunca era "
            "ligado. O conserto le `Schema::tem_dado_pessoal`, que enxerga as "
            "externas. E a forma dos pedidos 172, 173 e 176: o conserto entrou "
            "no caminho que o motivou e o irmao ficou."
        ),
        "arquivo": "crates/phxsql-store/src/reg.rs",
        "trecho": """        let material = if esquema.tem_dado_pessoal() {
            cofre::Material::novo()?
        } else {
            cofre::Material::EM_CLARO
        };
""",
        "troca": """        // DEFEITO REPOSTO: o material sai das faixas inline, e coluna externa
        // marcada nao gera faixa.
        let material = if faixas.is_empty() {
            cofre::Material::EM_CLARO
        } else {
            cofre::Material::novo()?
        };
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-dados"],
        "caem": [
            "coluna_externa_marcada_sozinha_nao_pode_ir_em_claro",
            # As duas provam o mesmo lado por outro caminho: com o material em
            # claro, marcar depois passa a ser aceito (a recusa so existe em
            # tabela cifrada) e a coluna inline acrescentada nao ganha etiqueta.
            "marcar_coluna_depois_numa_tabela_cifrada_e_recusado_e_o_grau_pode_mudar",
            "acrescentar_coluna_inline_marcada_a_tabela_cifrada_so_de_externas_sela_a_nova",
        ],
        # O comportamento velho tem de continuar de pe com o defeito reposto:
        # tabela sem marca em claro, tabela com inline marcada cifrada. Se um
        # deles cair, a troca quebrou mais que o defeito de origem.
        "seguem": [
            "tabela_sem_coluna_marcada_continua_em_claro_com_o_cofre_ligado",
            "coluna_inline_marcada_continua_com_a_etiqueta_de_16_bytes",
            "o_dado_da_coluna_marcada_some_do_disco",
            "cifrada_a_tabela_funciona_igual",
            "sem_cofre_nada_muda_no_disco",
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
        # Desde 05/09 a trava e um `RwLock<Raiz>`, e o defeito reposto tem de
        # COMPILAR para poder provar alguma coisa: `.lock()` nao existe mais, e
        # a ficha exclusiva sai do guard de escrita. Guarda cujo defeito nao
        # compila nao e guarda -- e por isso o executor a chama de QUEBRADA em
        # vez de PROVADA, e foi ele que pegou esta.
        "troca": """        // DEFEITO REPOSTO: a decima-quarta tomada, fora do ponto unico.
        let mut raiz = self.dados.write().map_err(|_| trava_envenenada())?;
        let dados = raiz.exclusiva();
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
        # O trecho carrega o COMENTARIO de cima, e nao e enfeite: desde 05/09
        # ha DUAS portas (`travar_dados` e `travar_dados_para_ler`) e as duas
        # comecam com esta mesma pergunta. Sem o comentario o trecho aparece
        # duas vezes e o executor recusa a entrada -- corretamente, porque
        # trocar a errada provaria outra coisa. A porta de LEITURA tem a guarda
        # irma logo abaixo.
        "trecho": """        // ANTES de qualquer trabalho, e antes de parar na fila: se esta thread
        // ja tem a trava, esperar por ela e esperar por si mesma.
        if COM_A_TRAVA.with(std::cell::Cell::get) {
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
        # O PONTO DE REPOSICAO ANDOU DE ARQUIVO -- pedido 263, 16/09/2026.
        # Em `d59967a` (08/09, pedido 228) a formula «teto zero nunca gira»
        # saiu do `profiler::girar_se_encheu` para `rodizio::deve_girar`,
        # para `acessos.log` e `diretivas.log` chamarem a MESMA em vez de
        # cada um reescrever a sua. O zero continua sendo lido -- num lugar
        # so --, e e la que tira-lo reproduz o defeito de origem. A entrada
        # segue a logica, e nao o nome do arquivo: guardar o `profiler.rs`
        # so por ser onde a licao nasceu deixaria a guarda apontando para um
        # arquivo que nao decide mais nada.
        #
        # O alcance ficou MAIOR do que o defeito de origem, e isso esta
        # declarado: tirar o zero de `deve_girar` derruba o teste do
        # Profiler (o defeito de origem) E o teste da propria formula. Os
        # dois estao em `caem`; os do `acesso.rs`/`diretivas.rs`, que
        # tambem sentem, ficam de fora porque nao sao o defeito que esta
        # entrada descreve.
        "arquivo": "crates/phxsql-server/src/rodizio.rs",
        "trecho": """    teto_do_arquivo != 0 && bytes_no_arquivo != 0 && bytes_no_arquivo + proxima > teto_do_arquivo""",
        "troca": """    // DEFEITO REPOSTO: o zero deixa de desligar o rodizio.
    bytes_no_arquivo != 0 && bytes_no_arquivo + proxima > teto_do_arquivo""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::teto_zero_nao_rodizia",
            "rodizio::testes::teto_zero_nunca_manda_girar",
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
        "id": "cifra-do-fio-rebaixada",
        "titulo": "a cifra do fio de volta a OPCIONAL por padrão",
        "porque": (
            "ordem do dono, 18/09/2026: *a comunicacao deve obrigatoriamente "
            "ser cifrada*. `cifra_fio.exigir` nasce `true`, e `\"exigir\": "
            "false` e o escape ESCRITO -- o mesmo padrao do `\"verificar\": "
            "false` da chave que nasce conferida."
            "\n\nESTA GUARDA TROCOU DE LADO em 18/09/2026, e o historico fica "
            "escrito porque ele ensina: ate o pedido 370 ela se chamava "
            "`cifra-do-fio-imposta` e repunha o defeito CONTRARIO -- exigir a "
            "cifra por padrao --, em nome da petrea *guarda nova entra pedida, "
            "nao imposta*. Quem revogou a petrea para ESTE interruptor foi o "
            "dono, e o teste que ela vigiava "
            "(`cliente_sem_cifra_continua_como_antes`) mudou de lado junto, "
            "virando `o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo`. "
            "O que a petrea continua protegendo esta no escape escrito, e ele "
            "tem guarda propria: o `o_escape_escrito_deixa_o_cliente_em_claro_entrar` "
            "esta em `seguem`, e nao em `caem` -- ele e o que fica IGUAL dos "
            "dois lados da virada."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """            exigir: true,
            exigir_amarra: false,
""",
        "troca": """            // DEFEITO REPOSTO: a cifra volta a ser opcional por padrao.
            // Parece inofensivo -- ninguem perde acesso --, e e o padrao que
            // deixa senha, token e dado viajarem em claro em toda instalacao
            // que nao souber que precisa escrever o campo.
            exigir: false,
            exigir_amarra: false,
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cifra-do-fio"],
        "caem": [
            "o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo",
        ],
        "seguem": [
            "o_escape_escrito_deixa_o_cliente_em_claro_entrar",
            "com_o_aperto_o_mesmo_trabalho_acontece_cifrado",
            "exigir_recusa_texto_claro_e_deixa_o_tunel_passar",
        ],
    },
    # 22-bis. As portas HTTP que ignoravam o interruptor da cifra
    # -----------------------------------------------------------------------
    {
        "id": "portas-http-sem-o-portao-da-cifra",
        "titulo": "as portas HTTP atendendo em claro com a cifra exigida",
        "porque": (
            "pedido 370, medido contra servidor de pe em 18/09/2026: com "
            "`cifra_fio.exigir: true` a porta nativa recusava e, no MESMO "
            "servidor e no MESMO instante, `POST /api {\"op\":\"login\"}` "
            "devolvia 200 com a sessao aberta e a senha em claro; `/v1/login`, "
            "`POST /mcp` e o explorador da especificacao idem. Interruptor de "
            "seguranca se mede pelo que ele RECUSA, nunca pelo que publica."
            "\n\nO portao e UM so -- `portao_de_rede_http` --, e por ele "
            "entram as tres portas HTTP e o endpoint `/mcp`, que viaja na porta "
            "do REST. Repor o defeito e apagar essa conferencia."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if self.config.cifra_fio.exigir && !self.proxy_desta_porta_http(op).0 {
            self.recusar_http_em_claro(fluxo, ip, porta, op, agora);
            return false;
        }
""",
        "troca": """        // DEFEITO REPOSTO: a porta HTTP volta a ignorar o interruptor da
        // cifra. A porta de dados continua recusando ao lado, e e por isso
        // que o furo passou um dia inteiro sem aparecer em teste nenhum.
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cifra-das-portas-http"],
        "caem": [
            "com_a_cifra_exigida_as_portas_http_recusam_e_dizem_o_que_fazer",
            "sem_a_secao_cifra_fio_a_porta_http_ja_nasce_recusando",
        ],
        "seguem": [
            "com_o_escape_escrito_as_portas_http_continuam_como_antes",
            "o_proxy_declarado_deixa_as_portas_http_atenderem_com_a_cifra_exigida",
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
    # 23b. A cifra do fio: a amarracao da credencial ao canal, ignorada
    # -----------------------------------------------------------------------
    {
        "id": "amarra-ao-canal-ignorada",
        "titulo": "o login amarrado ao canal conferido SEM a transcricao",
        "porque": (
            "secao 10 do docs/CIFRA-DO-FIO.md: sem amarrar a prova a "
            "transcricao do tunel, um homem-no-meio que terminou o tunel do "
            "cliente reencaminha a prova e ela confere. A prova real e o caso "
            "do atacante, que a LEITURA do codigo nao pega: com o defeito "
            "reposto o cliente honesto para de entrar, e e por ele que o teste "
            "cai."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        let canal_ref = canal_amarrado.as_ref().map(|t| &t[..]);
""",
        "troca": """        // DEFEITO REPOSTO: o login ignora a transcricao do tunel. A prova
        // deixa de dizer QUAL canal a carregou, e um proxy que terminou o
        // tunel do cliente reencaminha a prova sem que nada acuse.
        let canal_ref: Option<&[u8]> = None;
        let _ = canal_amarrado;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_cadastro_de_usuarios::login_amarrado_ao_canal_confere_contra_a_transcricao_da_sessao",
        ],
        "seguem": [
            # Login sem amarracao (senha pura) fica verde: prova que o defeito
            # e local a amarracao, e nao um estrago em todo login.
            "servidor::testes_cadastro_de_usuarios::cria_grava_no_arquivo_e_o_login_novo_ja_entra",
        ],
    },
    # 23c. A cifra do fio: a amarracao EXIGIDA, ignorada
    # -----------------------------------------------------------------------
    {
        "id": "amarra-exigida-ignorada",
        "titulo": "o servidor exige a amarracao ao canal, mas o login nao a cobra",
        "porque": (
            "secao 10 do docs/CIFRA-DO-FIO.md: a amarracao ao canal e PEDIDA, "
            "e um atacante ativo que terminou o tunel do cliente corta o campo "
            "`amarrar_canal` antes de reencaminhar -- a mesma aritmetica do "
            "rebaixamento do `exigir`. Contra ele so vale o servidor EXIGIR a "
            "amarracao quando ha tunel. Com o defeito reposto o op_login ignora "
            "`cifra_fio.exigir_amarra` e quem nao amarra volta a entrar: o caso "
            "(a) do teste cai na `unwrap_err` da recusa nomeada."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if self.config.cifra_fio.exigir_amarra && sessao.transcricao_do_fio.is_some() && !amarrar {
            return Err(PhxError::Autorizacao(self.msg("erro.amarra_exigida", &[])));
        }
""",
        "troca": """        // DEFEITO REPOSTO: o op_login NAO exige a amarracao. Um cliente que
        // nao pede `amarrar_canal` entra mesmo com `exigir_amarra` ligado --
        // exatamente o que o atacante que cortou o campo antes de reencaminhar
        // consegue quando a exigencia nao morde.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_cadastro_de_usuarios::login_exige_amarra_quando_ha_tunel",
        ],
        "seguem": [
            # A amarracao PEDIDA (exigir desligado) fica verde: prova que o
            # defeito e local a EXIGENCIA, e nao um estrago na amarracao em si.
            "servidor::testes_cadastro_de_usuarios::login_amarrado_ao_canal_confere_contra_a_transcricao_da_sessao",
        ],
    },
    # 23d. A cifra do fio: o Remoto da interface nao liga o tunel
    # -----------------------------------------------------------------------
    {
        "id": "remoto-em-claro-para-quem-exige",
        "titulo": "o abrir_remoto manda o login em claro mesmo com cifra: true",
        "porque": (
            "secao 10 do docs/CIFRA-DO-FIO.md: `web.servidores` passou a "
            "aceitar objetos com `cifra`/`chave_do_fio`, e o `Remoto` liga o "
            "tunel ANTES do login -- e a prova e o token do login que o tunel "
            "esconde. Com o defeito reposto o aperto nao acontece, o login vai "
            "em CLARO, e contra um destino que EXIGE a cifra a conexao e "
            "recusada nomeando a cifra do fio. E por essa recusa (o caso (b)/(c) "
            "do teste, com o destino em `exigir: true`) que o teste cai -- a "
            "LEITURA do codigo nao pega, porque em claro contra um destino que "
            "NAO exige tudo continua funcionando."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Some(sv) = self.config.web.servidor(destino) {
            if sv.cifra {
                let pino = sv.pino_do_fio().map_err(|e| (op.clone(), e))?;
                remoto.cifrar(pino).map_err(|e| (op.clone(), e))?;
            }
        }
""",
        "troca": """        // DEFEITO REPOSTO: o abrir_remoto NAO liga o tunel. O login (o
        // `linha` abaixo) vai em CLARO mesmo quando web.servidores diz
        // `cifra: true`. Contra um destino que EXIGE a cifra isso vira recusa
        // nomeada em vez de vazamento -- e e por ela que o teste cai.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_remoto_cifrado::abrir_remoto_liga_o_tunel_quando_a_config_pede_cifra",
        ],
        "seguem": [
            # O aperto em si (Remoto::cifrar) fica verde: prova que o defeito e
            # local a DECISAO do abrir_remoto, e nao um estrago no tunel.
            "servidor::testes_remoto_cifrado::o_remoto_liga_o_tunel_e_carrega_um_pedido_real",
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
            "depois da leitura -- e ai a memoria ja foi gasta. ESTA entrada prova "
            "a MAQUINA (o `take`) com o teto passado NA MAO (`ler_ate(leitor, "
            "64)`); ela NUNCA tocou a CONSTANTE `TETO_DO_REGISTRO` que o "
            "`Canal::ler` usa de verdade -- essa prova e das guardas "
            "`teto-do-fio-sem-a-constante` e `-no-soquete` (pedido 303)."
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
        # NAO estenda o alvo desta entrada ao teste de soquete
        # (`--test teto-da-resposta`). Medido em 17/09/2026 pelo papel G: com
        # ESTE mesmo defeito reposto (o `take` fora), um dos dois testes de
        # soquete fica VERDE -- a conferencia `lidos > teto` ainda acontece, so
        # que DEPOIS de ler os 129 MiB inteiros, entao o veredito sai certo com
        # a memoria ja gasta. E a propria licao escrita no `porque` acima, agora
        # do outro lado: o teste de soquete afirma o VEREDITO
        # (`matches!(erro, PhxError::LimiteExcedido(_))`), nao QUANTO foi lido.
        # O outro cai, mas por PRAZO (15,73 s de execucao), e nao pela garantia
        # que esta entrada nomeia -- vira NAO PEGOU falso nos dois sentidos.
        # Alvo de soquete tem entrada propria, e com OUTRO defeito reposto: a
        # `teto-do-fio-sem-a-constante-no-soquete`.
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois",
        ],
    },
    # 24c. O teto do fio: a CONSTANTE trocada por um teto quase infinito
    # -----------------------------------------------------------------------
    {
        "id": "teto-do-fio-sem-a-constante",
        "titulo": "o `Canal::ler` de producao troca `TETO_DO_REGISTRO` por um teto quase infinito",
        "porque": (
            "pedido 303, commit be7e361 -- a entrada `fio-sem-teto-de-registro` "
            "e o teste que ja existia desde 30/08 provavam a MAQUINA (o `take` "
            "antes da leitura), passando o teto NA MAO; nenhum dos dois tocava a "
            "CONSTANTE que o `Canal::ler` realmente usa. Medido em 17/09/2026: "
            "trocando a chamada de producao por `ler_ate(leitor, u64::MAX - 1)` "
            "-- o fio inteiro sem teto --, ZERO testes do repositorio acusavam "
            "antes desta rodada. Prova de mecanismo nao e prova de configuracao."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        self.ler_ate(leitor, TETO_DO_REGISTRO)
    }
""",
        "troca": """    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        // DEFEITO REPOSTO: a chamada de producao troca a constante por um teto
        // praticamente infinito -- o fio inteiro fica sem teto.
        self.ler_ate(leitor, u64::MAX - 1)
    }
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::a_leitura_padrao_para_no_teto_do_registro_e_nao_no_que_o_outro_lado_mandar",
        ],
        # O teste que passa o teto NA MAO continua VERDE: ele nunca chama
        # `Canal::ler`, so o `ler_ate` direto. E ele que prova que o defeito e
        # local a CONSTANTE, e nao um estrago no mecanismo do `take`.
        "seguem": [
            "fio::testes::o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois",
        ],
    },
    # 24d. A mesma troca da constante, vista PELO SOQUETE
    # -----------------------------------------------------------------------
    {
        "id": "teto-do-fio-sem-a-constante-no-soquete",
        "titulo": "a mesma troca da constante por um teto quase infinito, vista pela rede",
        "porque": (
            "pedido 303, commit be7e361 -- irma da `teto-do-fio-sem-a-constante`, "
            "pelo SOQUETE: o que atravessa a rede se prova contra a rede, que e a "
            "licao do BULKINSERT. O `tests/teto-da-resposta.rs` exercita os dois "
            "lados que RECEBEM pelo `Canal::ler` -- a replica (o `TETO_DA_RESPOSTA` "
            "de `docs/REPLICACAO.md` §18) e o laco de conexao do servidor. Entra "
            "SEPARADA da `fio-sem-teto-de-registro`, e com outro defeito reposto: "
            "ver o aviso naquela entrada."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        self.ler_ate(leitor, TETO_DO_REGISTRO)
    }
""",
        "troca": """    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        // DEFEITO REPOSTO: a chamada de producao troca a constante por um teto
        // praticamente infinito -- o fio inteiro fica sem teto.
        self.ler_ate(leitor, u64::MAX - 1)
    }
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "teto-da-resposta"],
        "caem": [
            "a_resposta_acima_do_teto_e_recusada_por_limite_e_a_rodada_seguinte_abre_outra",
            "o_pedido_acima_do_teto_recebe_a_recusa_e_entra_no_log_com_o_tamanho",
        ],
        "seguem": [
            "a_resposta_grande_que_cabe_no_teto_atravessa_como_sempre",
            "o_pedido_de_sempre_continua_sendo_atendido",
        ],
        # Medido em 17/09/2026: 0,55 s de execucao dos quatro testes com o
        # defeito reposto (17,25 s reais, com a compilacao do zero), contra os
        # 300 s do prazo padrao -- ja caberia sem prazo proprio. Ganha um mesmo
        # assim, pelo motivo da `laco-preso-no-unico-secundario`: teste de
        # soquete tem variancia que unitario nao tem, e os dois testes desta
        # bateria carregam um PRAZO interno de 15 s por leitura ou escrita, entao
        # uma maquina carregada pode bater nesse teto mais de uma vez antes de
        # qualquer assercao rodar. 120 s da ~7x de folga sobre o medido sem
        # esconder uma pendura de verdade.
        "prazo": 120,
    },
    # 24b. A cifra do CLUSTER: o pulso da eleicao saindo em claro
    # -----------------------------------------------------------------------
    {
        "id": "pulso-do-cluster-em-claro",
        "titulo": "o pulso da eleição saindo em claro com a cifra do cluster ligada",
        "porque": (
            "secao 10 do docs/CIFRA-DO-FIO.md e regra da casa: cifrar so METADE "
            "do trafego do cluster e pior que nao cifrar nenhuma, porque parece "
            "protegido. O pulso e a replicacao do cluster passam os dois pela "
            "`replica::Cliente`; a replicacao ja cifrava por `origem.cifra`, e o "
            "pulso e o caminho que ficava em claro. A prova e o caso do soquete "
            "que a leitura nao pega: com o defeito reposto o pulso bate no "
            "`cifra_fio.exigir` do outro no e cai, e o no some do `cluster_estado` "
            "-- so o tunel real faz os dois nos se enxergarem."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if c.cifra {
            cliente.cifrar(no.pino_do_fio()?)?;
        }
        if !c.usuario.is_empty() {
""",
        "troca": """        // DEFEITO REPOSTO: o pulso sai em claro mesmo com cluster.cifra
        // ligada. Cifrar so a replicacao e deixar o pulso em claro e a metade
        // que engana -- o `exigir` do outro no o recusa e o cluster nao forma.
        if !c.usuario.is_empty() {
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cluster-cifrado"],
        "caem": [
            "pulso_do_cluster_cifrado_atravessa_no_que_exige_tunel",
        ],
    },
    # 24c. A cifra do CLUSTER: a replicacao entre os nos saindo em claro
    # -----------------------------------------------------------------------
    {
        "id": "replicacao-do-cluster-em-claro",
        "titulo": "a replicação entre os nós do cluster saindo em claro",
        "porque": (
            "a outra metade do `pulso-do-cluster-em-claro`: cifrar o pulso e "
            "esquecer a replicacao deixaria a mesma metade protegida e metade "
            "nao. A `origem_do_master` e pura de proposito, para o teste pegar "
            "que a linha `cifra: c.cifra` sumiu -- coisa que a leitura do laco "
            "vivo nao pega."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        // A replicacao do cluster viaja pela MESMA cifra do pulso -- ligar
        // `cluster.cifra` protege o trafego INTEIRO do cluster, nunca so metade.
        cifra: c.cifra,
""",
        "troca": """        // DEFEITO REPOSTO: a replicacao do cluster sai em claro, enquanto o
        // pulso vai cifrado -- metade protegida, que e pior que nenhuma.
        cifra: false,
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_config_gravar::origem_do_cluster_carrega_a_cifra_e_o_pino",
        ],
        "seguem": [
            # A cifra do PULSO (outra linha) fica verde: prova que o defeito e
            # local a replicacao, e nao um estrago em toda a cifra do cluster.
            "config::tests::cifra_do_cluster_ligada_le_o_pino_por_no",
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
        "trecho": """        for (caminho, espelho) in &pendente.trocas {
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
        # Trecho movido em 24/09/2026 pelo pedido 426: o que vem depois da
        # marca passou a morar em `depois_da_marca`. O defeito e o mesmo.
        "trecho": """                if self.tabelas_ainda_sujas(database, &escritas) {
                    if let Ok(mut m) = self.marcas_pendentes.lock() {
                        m.push(marca.to_path_buf());
                    }
                } else {
                    let _ = std::fs::remove_file(marca);
                }
""",
        "troca": """                // DEFEITO REPOSTO: a marca sai sempre, sem esperar o fsync.
                let _ = std::fs::remove_file(marca);
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
        # ATUALIZADO em 16/09/2026 (pedido 263). Mesmo motivo da entrada
        # `recuperacao-nao-reconstroi-a-filha`, que guarda o interruptor
        # vinte e poucas linhas abaixo deste bloco no MESMO `completar()`:
        # `2fe8658` (12/09) desaninhou o laco `for op in &marca.operacoes` e
        # o bloco inteiro subiu quatro espacos. A logica nao mudou.
        "arquivo": "crates/phxsql-server/src/transacao.rs",
        "trecho": """                    if t.indice_precisa_reconstruir() {
                        match t.reindexar() {
                            Ok(_) => r.indices_reconstruidos += 1,
""",
        "troca": """                    // DEFEITO REPOSTO: a recuperacao nao reconstroi o
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
        # O trecho de hoje: `fks_conferidas` (a lista de indices em cache) saiu
        # do `inserir` no mesmo dia em que a APOSENTADA `portao-de-fk-com-
        # -esquema-velho` explica -- o portao pergunta ao esquema na hora
        # (`fks_que_conferem`). O comentario apos o `}` cresceu junto, entao a
        # amarra e so ate onde ele continua unico neste arquivo (`inserir`; o
        # `atualizar` tem o mesmo `if` mas nao este comentario).
        #
        # ATUALIZADO em 16/09/2026 (pedido 263). `2fe8658` (12/09, o P0 da FK
        # dentro da transacao) renomeou o corpo do `inserir` para
        # `inserir_com_maes_opt` e trocou `conferir_fks(valores)` por
        # `conferir_fks_com(valores, maes)`. O PORTAO nao mudou de lugar nem
        # de forma -- continua sendo o `&& self.julga_integridade()` da mesma
        # linha --, so a chamada de dentro dele. A amarra continua sendo o
        # comentario do `// Numerar ANTES`, que segue unico no arquivo (o
        # `atualizar_com_maes_opt` tem o mesmo `if`, e nao este comentario).
        "trecho": """        self.conferir_aridade(valores)?;
        if fks_que_conferem(&self.esquema).next().is_some() && self.julga_integridade() {
            self.conferir_fks_com(valores, maes)?;
        }
        // Numerar ANTES das chaves, pela mesma razao da sequencia: se a coluna""",
        "troca": """        // DEFEITO REPOSTO: a replica volta a julgar o que a origem ja julgou.
        self.conferir_aridade(valores)?;
        if fks_que_conferem(&self.esquema).next().is_some() {
            self.conferir_fks_com(valores, maes)?;
        }
        // Numerar ANTES das chaves, pela mesma razao da sequencia: se a coluna""",
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
        # ATUALIZADO em 16/09/2026 (pedido 263). `7d29f5f` (15/09, o ACID-C)
        # acrescentou o `cascatear &&` ao MESMO `if`: dentro da transacao a
        # corrente ja foi achatada na lista de escritas, e refazer a cascata
        # aqui gravaria a filha duas vezes. Sao DOIS motivos independentes
        # para nao cascatear, e esta entrada guarda so o dela -- por isso a
        # troca tira o `self.julga_integridade()` e DEIXA o `cascatear`.
        # Tirar os dois reporia dois defeitos de uma vez, e o veredito nao
        # diria qual dos dois o teste pegou.
        "trecho": """        let mut cascata = if cascatear && self.julga_integridade() {
            self.planejar_ao_alterar(&valores_antigos, valores)?
        } else {
            Vec::new()
        };""",
        "troca": """        // DEFEITO REPOSTO: a replica planeja e roda a cascata de novo.
        let mut cascata = if cascatear {
            self.planejar_ao_alterar(&valores_antigos, valores)?
        } else {
            Vec::new()
        };""",
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
        # ATUALIZADO em 16/09/2026 (pedido 263). `2fe8658` (12/09, o P0 da FK
        # dentro da transacao) tirou o corpo do laco de dentro do
        # `conferir_fks` e o pos num ajudante proprio,
        # `Table::conferir_uma_fk(mae, fk, chave)`, para os DOIS caminhos da
        # mae -- a aberta do disco e a emprestada pela transacao -- passarem
        # pelo mesmo codigo. A logica nao mudou uma letra; o que mudou foi o
        # NIVEL de indentacao (doze espacos viraram oito), e foi so isso que
        # tirou o trecho do lugar. O ponto de reposicao e o mesmo `if`.
        "trecho": """        if mae.esquema.coluna_softdeleted().is_some() {
            let mut viva = false;""",
        "troca": """        // DEFEITO REPOSTO: existir volta a valer por estar viva.
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
    {
        "id": "drop-table-mata-o-pai",
        "titulo": "o `excluir_tabela` volta a apagar a mae com filha apontando",
        "porque": (
            "a regra primordial vale no nivel da TABELA, e nao so no da linha. "
            "Medido por sonda: este caminho apagava os 8 arquivos da mae e a "
            "filha ficava com a linha intacta apontando para o vazio -- e o "
            "`renomear_tabela` ja recusava o MESMO cenario, entao o motor "
            "sabia fazer a pergunta e nao a fazia aqui."
        ),
        "arquivo": "crates/phxsql-store/src/catalogo.rs",
        "trecho": """        if let Some(filha) = self.quem_aponta_para(&dir, nome)? {
            return Err(PhxError::Integridade(format!(
                "a tabela {qualificado} nao pode ser apagada: {filha} declara \\
                 uma chave estrangeira para ela. Nunca se apaga o pai que tem \\
                 filhos -- apague {filha} primeiro, ou tire a chave dela"
            )));
        }
""",
        "troca": """        // DEFEITO REPOSTO: o drop volta a matar o pai que tem filhos.
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "catalogo::testes_copia_entre_bancos::excluir_recusa_a_mae_que_tem_filha_apontando",
        ],
        "seguem": [
            # Os dois controles: sem eles, um portao que recusasse TODO apagar
            # passaria pelo `caem` acima sem proteger nada.
            "catalogo::testes_copia_entre_bancos::apagada_a_filha_a_mae_sai_normalmente",
            "catalogo::testes_copia_entre_bancos::excluir_tabela_sem_chave_nenhuma_nao_muda_nada",
            "catalogo::testes_gestao::excluir_tabela_leva_os_arquivos_dela_e_so_os_dela",
        ],
    },
    {
        "id": "before-sem-prazo-de-parede",
        "titulo": "o corpo do gatilho BEFORE volta a rodar sem prazo, com a trava global na mão",
        "porque": (
            "o `PASSOS_MAX` do avaliador era citado como se fosse teto da "
            "TRAVA, e nao e: teto de passos nao e teto de trabalho. Um milhao "
            "de passos de aritmetica custa 27,2 ms e um milhao copiando meio "
            "megabyte custa 28.590 ms -- medido, com a trava GLOBAL de dados "
            "na mao. O prazo de parede e o unico dos tres tetos que limita a "
            "trava."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            let mut ctx = Contexto::de_gatilho(nova_json.take(), gravavel, velha_json.clone())
                .com_prazo(PRAZO_DO_GATILHO_ANTES);
""",
        "troca": """            // DEFEITO REPOSTO: o BEFORE volta a rodar sem prazo de parede.
            let mut ctx =
                Contexto::de_gatilho(nova_json.take(), gravavel, velha_json.clone());
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            # SO a catraca estatica pega. Medido: com o prazo removido, o teste
            # de ponta a ponta `gatilho_before_sem_fundo_nao_derruba_o_servidor`
            # continua PASSANDO em 0,97 s, porque o teto de TEXTO segura o
            # mesmo corpo -- e ele so promete «nao derruba», que continua
            # verdade. Prova de que a catraca nao e redundante: ela e a unica.
            "servidor::testes_janela_e_cadeia::o_before_roda_com_prazo_e_o_after_nao",
        ],
        "seguem": [
            # Os controles: tirar o prazo NAO pode quebrar gatilho honesto --
            # se quebrasse, o `caem` acima passaria por uma razao errada.
            "servidor::testes_gatilhos::before_insert_normaliza_o_campo",
            "servidor::testes_gatilhos::sinal_cancela_a_escrita_e_a_linha_nao_entra",
            "servidor::testes_gatilhos::lote_passa_pelo_before_por_linha",
            # E este e o achado desta entrada, guardado como controle: o teste
            # de ponta a ponta segue de pe com o defeito reposto.
            "servidor::testes_janela_e_cadeia::gatilho_before_sem_fundo_nao_derruba_o_servidor",
        ],
    },
    # O IRMAO DESTA ENTRADA NAO ESTA AQUI, E ISSO E DECISAO.
    #
    # O outro teto do mesmo commit e o `TEXTO_MAX`, e repo-lo faz o binario
    # alocar ate o alocador falhar -- 8 a 16 GiB nesta maquina, com o risco de
    # o kernel matar o processo de OUTRA frente para arranjar memoria. Guarda
    # que derruba o trabalho do vizinho e a mesma falha do zelador que apaga o
    # `target` de quem esta compilando.
    #
    # Ela fica MANUAL, com a receita escrita no comentario do teste
    # `o_texto_que_dobra_para_no_teto_em_vez_de_derrubar_o_processo` e no
    # `docs/CONCORRENCIA.md`: `ulimit -v 2000000` limita o processo a 2 GiB, e
    # ai o defeito reposto aborta em 13,9 s dizendo «memory allocation of
    # 536870912 bytes failed» sem levar ninguem junto. Conferida assim em
    # 03/09.
    {
        "id": "declara-conferida-sobre-orfa",
        "titulo": "a chave volta a nascer conferida sobre tabela que ja tem orfa",
        "porque": (
            "sonda `--example sonda-fk-buracos`, item 4: dava para declarar "
            "`verificar: true` numa tabela que ja tinha orfa, e a orfa "
            "continuava la. A tabela nascia com uma promessa falsa -- um "
            "`verificar` que nunca valeu para as linhas ja gravadas --, e "
            "promessa falsa e pior que a ausencia dela: quem le o esquema para "
            "de perguntar. Mesma familia de \"configuracao que nao e lida "
            "mente\"."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """            if !fk.verificar || self.ja_era_conferida(fk) {
                continue;
            }""",
        "troca": """            // DEFEITO REPOSTO: a declaracao volta a nao olhar o dado gravado.
            if true {
                continue;
            }""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "verificador-de-consistencia"],
        "caem": [
            "redeclarar_recusa_chave_conferida_sobre_orfa",
        ],
        "seguem": [
            # Os controles: o comportamento VELHO (declarar sem conferir), o
            # dado limpo, e a chave que ja conferia. Sem eles, um portao que
            # recusasse toda declaracao passaria pelo `caem` acima.
            "declarar_sem_conferir_continua_passando_com_orfa",
            "com_dado_limpo_a_chave_nasce_conferida",
            "redeclarar_chave_ja_conferida_nao_varre_de_novo",
        ],
    },
    {
        "id": "verificador-nao-pergunta-se-a-mae-esta-viva",
        "titulo": "o verificador volta a aceitar mae excluida como mae",
        "porque": (
            "e a mesma pergunta que o `conferir_fks` faz na gravacao, e ela "
            "tem de ser a mesma nos dois lugares: um verificador que diz "
            "\"limpo\" sobre uma base que o motor recusaria gravar de novo "
            "mente com a autoridade de uma ferramenta de diagnostico."
        ),
        "arquivo": "crates/phxsql-store/src/integridade.rs",
        "trecho": """                Ok(match mae.ler(r)? {
                    Some(l) => !mae.esta_excluida(&l),
                    None => false,
                })""",
        "troca": """                // DEFEITO REPOSTO: existir volta a valer por estar viva.
                let _ = r;
                Ok(true)""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "verificador-de-consistencia"],
        "caem": [
            "a_mae_excluida_suave_e_uma_falha_com_nome_proprio",
        ],
        "seguem": [
            "base_limpa_nao_acusa_nada",
            "a_orfa_aparece_e_o_verificador_nao_a_conserta",
        ],
    },
    {
        "id": "restaurar-nao-pergunta-pela-mae",
        "titulo": "restaurar volta a ressuscitar a filha sem olhar a mae",
        "porque": (
            "restaurar e a terceira porta pela qual uma linha volta a existir "
            "para quem le -- as outras duas conferem. A orfa por construcao "
            "sobreviveu versoes porque a porta que faltava era a que ninguem "
            "olhava: porta que nao faz a pergunta que as irmas fazem e a "
            "proxima a virar buraco."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        # O mesmo envelhecimento da guarda irma acima: `fks_conferidas` (lista
        # em cache) virou `fks_que_conferem(&self.esquema)` (calculado na hora).
        "trecho": """        if fks_que_conferem(&self.esquema).next().is_some() && self.julga_integridade() {
            // A linha so se le quando ha chave a conferir: sem elas, restaurar
            // continua custando o que sempre custou.
            if let Some(linha) = self.ler(rowid)? {""",
        "troca": """        // DEFEITO REPOSTO: restaurar volta a nao perguntar pela mae.
        if false {
            if let Some(linha) = self.ler(rowid)? {""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "chave-estrangeira"],
        "caem": [
            "a_filha_marcada_nao_volta_sem_mae_viva",
        ],
        "seguem": [
            # Os dois controles: com mae viva, e o comportamento velho.
            "com_mae_viva_restaurar_continua_igual",
            "sem_conferir_restaurar_nao_pergunta_nada",
        ],
    },
    {
        "id": "bidirecional-julga-fk",
        "titulo": "o bidirecional volta a conferir a chave do evento que aplica",
        "porque": (
            "o bidirecional casa por CHAVE, nao por rowid -- o rowid e o "
            "rownum sao locais --, entao ele nao passa pelo `aplicar_evento` e "
            "chamava o `inserir` de sempre. Caia no MESMO buraco da replica, e "
            "com consequencia pior: o erro subia pelo `?` do laco, `desde` "
            "nunca andava, e o mesmo lote voltava na rodada seguinte para "
            "sempre. Nao e uma linha perdida, e o par de servidores PARADO."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """    pub fn inserir_replicado(&mut self, valores: &[Value]) -> Result<RowId> {
        self.como_replica = true;
        let r = self.inserir(valores);
        self.como_replica = false;
        r
    }""",
        "troca": """    pub fn inserir_replicado(&mut self, valores: &[Value]) -> Result<RowId> {
        // DEFEITO REPOSTO: o bidirecional volta a julgar o que a origem julgou.
        self.inserir(valores)
    }""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "bidirecional-no-store"],
        "caem": [
            "o_bidirecional_aceita_a_filha_que_chega_antes_da_mae",
        ],
        "seguem": [
            # A exclusao e a outra metade, e por outro caminho: sem este
            # `seguem` a troca poderia estar quebrando o arquivo inteiro.
            "o_bidirecional_apaga_a_mae_cuja_filha_ainda_nao_saiu",
            "o_evento_forcado_guarda_carimbo_e_origem_do_nascimento",
        ],
    },
    {
        "id": "bidirecional-julga-as-filhas",
        "titulo": "o bidirecional volta a recusar apagar a mae que tem filha",
        "porque": (
            "na origem a filha ja saiu ANTES da mae -- foi o `conferir_filhas` "
            "dela que obrigou --, e os dois eventos chegam aqui em qualquer "
            "ordem, porque a replicacao anda por tabela. Recusar o da mae "
            "travaria o par de servidores por uma ordem que se resolve sozinha "
            "no lote seguinte."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """    pub fn excluir_de_vez_replicado(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        self.como_replica = true;
        let r = self.excluir_de_vez(rowid, motivo);
        self.como_replica = false;
        r
    }""",
        "troca": """    pub fn excluir_de_vez_replicado(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        // DEFEITO REPOSTO: o bidirecional volta a conferir as filhas.
        self.excluir_de_vez(rowid, motivo)
    }""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "bidirecional-no-store"],
        "caem": [
            "o_bidirecional_apaga_a_mae_cuja_filha_ainda_nao_saiu",
        ],
        "seguem": [
            "o_bidirecional_aceita_a_filha_que_chega_antes_da_mae",
        ],
    },
    {
        "id": "recascata-sem-conferir-a-arvore",
        "titulo": "a recuperação gravava a primeira filha e só então descobria que a neta da segunda restringe",
        "porque": (
            "o pedido 168 pos `recascatear` na recuperacao; o 169 pos "
            "`conferir_a_arvore` no `atualizar`. `recascatear` nasceu antes da "
            "conferencia e ficou sem ela -- conserto novo entra no caminho que "
            "o motivou, e o caminho irmao fica. So aparece com DUAS filhas no "
            "nivel 1: com uma so, a recusa da neta chega antes de qualquer "
            "escrita e nada denuncia o buraco."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        Self::conferir_a_arvore(&mut passos, 1)?;
        self.aplicar_ao_alterar(passos)""",
        "troca": """        // DEFEITO REPOSTO: aplica sem conferir a arvore, que e como
        // `recascatear` nasceu -- grava a primeira filha e so entao recusa.
        self.aplicar_ao_alterar(passos)""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cascata-ao-alterar"],
        "caem": [
            "a_recascata_recusa_antes_de_gravar_a_primeira_filha",
        ],
        "seguem": [
            "a_cascata_alcanca_a_neta",
            "a_filha_acompanha_a_chave_que_a_mae_mudou",
        ],
    },
    {
        "id": "auto-referencia-em-silencio",
        "titulo": "a auto-referência sai da cascata em silêncio e orfana a subordinada",
        "porque": (
            "ate 03/09/2026 `planejar_ao_alterar` fazia `if irma == eu "
            "{ continue }` seco: alterar a chave de uma tabela que aponta para "
            "si passava, a subordinada ficava na chave velha e o `atualizar` "
            "devolvia Ok. Os dois motores de referencia recusam -- «it acts "
            "like RESTRICT» --, e orfa que ninguem ve e pior que orfa que da "
            "erro. `docs/INTEGRIDADE.md` SS7.4."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """                        || !Self::chave_referenciada_mudou(&self.esquema, fk, antes, depois)""",
        "troca": """                        // DEFEITO REPOSTO: sem a conferencia da chave a recusa
                        // nunca dispara, e a auto-referencia volta a sair do
                        // plano em silencio.
                        || true""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cascata-ao-alterar"],
        "caem": [
            "a_auto_referencia_recusa_em_vez_de_orfanar_calada",
        ],
        "seguem": [
            "mudar_coluna_que_nao_e_a_referenciada_continua_passando",
            "a_cascata_alcanca_a_neta",
        ],
    },
    {
        "id": "recado-manda-reparar-arquivo-sao",
        "titulo": "a mãe invisível manda reparar o índice — de um arquivo intacto",
        "porque": (
            "mae escrita e nao sincronizada faz a conferencia de chave recusar, "
            "e a recusa esta certa. Errado era o TEXTO: o erro cru vinha "
            "embrulhado com o imperativo «reconstrua com `reparar indice`», "
            "mandando reparar arquivo sao -- a primeira metade do recado "
            "contradizendo a segunda. Durou porque o comentario acima da linha "
            "JA dizia que o erro cru era ruim, com o `({e})` logo abaixo: "
            "envolver nao e substituir."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        # ATUALIZADO em 16/09/2026 (pedido 263). Mesma mudanca que moveu o
        # `fk-nao-pergunta-se-a-mae-esta-viva`: `2fe8658` (12/09) tirou este
        # bloco de dentro do laco do `conferir_fks` e o pos no ajudante
        # `Table::conferir_uma_fk`, que os dois caminhos da mae (a do disco e
        # a emprestada pela transacao) chamam. A unica diferenca no texto e a
        # indentacao, de doze espacos para oito -- a pergunta a mae, que e o
        # que separa arquivo sao de corrupcao de verdade, esta intacta.
        "trecho": """        let pendente = mae
            .indice_precisa_reconstruir()
            .then(|| caminho(mae.diretorio(), mae.nome(), EXT_NDX));""",
        "troca": """        // DEFEITO REPOSTO: sem o portao, tudo cai no caminho do erro
        // cru e o recado volta a mandar reparar arquivo intacto.
        let pendente: Option<std::path::PathBuf> = None;""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "chave-estrangeira"],
        "caem": [
            "a_mae_invisivel_nao_manda_reparar_indice_sao",
            "a_mae_nao_gravada_recusa_dizendo_por_que",
        ],
        # O controle e a mae JA gravada: ela nao passa pelo portao novo, e
        # tem de continuar sendo vista com o defeito reposto -- senao a troca
        # quebrou o arquivo inteiro em vez de provar a guarda.
        "seguem": [
            "a_mae_aberta_e_ja_gravada_e_vista",
            "sem_conferir_a_mae_aberta_nao_muda_nada",
        ],
    },
    {
        "id": "procura-das-filhas-manda-reparar-arquivo-sao",
        "titulo": "a procura pelas filhas manda reparar o índice — de um arquivo intacto",
        "porque": (
            "o IRMAO do `recado-manda-reparar-arquivo-sao`: aquele e o lado "
            "«existe esta mae?», este e o lado «quem aponta para esta mae?». "
            "Os dois recusavam com o mesmo erro cru embrulhado, sob comentarios "
            "que os dois afirmavam que o erro cru era ruim. Terceira vez no "
            "mesmo dia em que um conserto entra num caminho e o irmao fica."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """                let filha_com_indice_pendente = filha
                    .indice_precisa_reconstruir()
                    .then(|| caminho(filha.diretorio(), filha.nome(), EXT_NDX));""",
        "troca": """                // DEFEITO REPOSTO: sem o portao, o recado volta a mandar
                // reparar um indice intacto.
                let filha_com_indice_pendente: Option<std::path::PathBuf> = None;""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cascata-ao-alterar"],
        "caem": [
            "a_procura_das_filhas_nao_manda_reparar_indice_sao",
        ],
        # Controles: a cascata normal e a que nao paga nada tem de seguir --
        # senao a troca quebrou o arquivo em vez de provar a guarda.
        "seguem": [
            "a_filha_acompanha_a_chave_que_a_mae_mudou",
            "a_cascata_alcanca_a_neta",
        ],
    },
    {
        "id": "recuperacao-nao-reconstroi-a-filha",
        "titulo": "a recuperação não reconstrói o índice da filha, e a cascata fica pela metade",
        "porque": (
            "o `completar()` reconstruia o `.ndx` sujo de toda tabela NOMEADA "
            "NA MARCA, e a filha da cascata nunca vira `Escrita` -- a maquina "
            "existia, rodava, e nao alcancava justamente a tabela que a cascata "
            "ia consertar. Medido pela matriz de durabilidade: 9 de 21 corridas "
            "caiam nesse caso. Terceira instancia da lei «conserto entra no "
            "caminho que o motivou e o irmao fica»."
        ),
        # ATUALIZADO em 16/09/2026 (pedido 263). `2fe8658` (12/09) tirou o
        # laco `for op in &marca.operacoes` de dentro de um bloco a mais em
        # `completar()` -- a tabela agora SAI do mapa enquanto reaplica, para
        # a conferencia de FK poder emprestar as maes que a mesma marca ja
        # reaplicou. A linha do interruptor e a mesma; o que mudou foi um
        # nivel de indentacao (vinte e quatro espacos viraram vinte).
        "arquivo": "crates/phxsql-server/src/transacao.rs",
        "trecho": """                    t.ligar_reconstrucao_do_indice_da_filha(true);""",
        "troca": """                    // DEFEITO REPOSTO: a recuperacao volta a recusar
                    // cascatear para a filha com indice sujo.
                    t.ligar_reconstrucao_do_indice_da_filha(false);""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cascata-na-recuperacao"],
        "caem": [
            "a_recuperacao_reconstroi_o_indice_da_filha_e_completa_a_cascata",
        ],
        # Controles: a recuperacao SEM indice sujo tem de seguir completando --
        # senao a troca desligou a cascata inteira em vez de provar a guarda.
        "seguem": [
            "a_recuperacao_refaz_a_cascata_que_a_queda_deixou_pela_metade",
            "com_a_mae_no_valor_velho_a_recuperacao_cascateia",
        ],
    },
    # ---------------------------------------------------------------------
    # APOSENTADA em 03/09/2026: `portao-de-fk-com-esquema-velho`
    #
    # Ela guardava o panico do `index out of bounds` quando
    # `redeclarar_chaves_estrangeiras` trocava o esquema sem refazer
    # `fks_conferidas` -- uma lista de INDICES para dentro das chaves.
    #
    # A entrada saiu porque o DEFEITO deixou de poder existir, e nao porque
    # alguem afrouxou: a lista foi APAGADA no mesmo dia. O portao pergunta ao
    # esquema na hora, entao nao ha indice para envelhecer nem para reordenar.
    # Guarda cujo defeito virou impossivel nao se mantem por educacao -- ela
    # passaria a provar que um trecho existe, e nao que uma garantia vale.
    #
    # A decisao foi por NUMERO e nao por gosto: a lista comprava 0,28-0,86 ns
    # e calcular na hora custa 0,92-1,37 ns (`docs/PESQUISA-ESTADO-DERIVADO.md`).
    # O irmao `colunas_marcadas` FICA, com a guarda dele, porque compra
    # 4,6-26,6 ns -- trinta vezes mais.
    #
    # O teste continua: `trocar_as_chaves_nao_deixa_o_portao_apontando_para_o
    # _esquema_velho`, em `tests/chave-estrangeira.rs`, deixou de poder cair
    # por panico e passou a afirmar o comportamento -- redeclarar para lista
    # vazia aceita a linha que ninguem confere mais.
    # ---------------------------------------------------------------------
    # ------------------------------------------------ a ficha compartilhada
    {
        "id": "pista-de-leitura-engole-a-trilha",
        "titulo": "a pista de leitura aceita tabela com dado pessoal, e a trilha fica sem o registro",
        "porque": "abrir uma tabela para LER escreve em seis lugares, e este e o "
                  "unico que acontece em TODA varredura de tabela marcada. A ficha "
                  "compartilhada nao sabe escrever -- e por isso ela tem de RECUSAR a "
                  "tabela, em vez de atende-la calada. Trilha que perde registro em "
                  "silencio e pior que trilha nenhuma: ela PARECE completa, e quem "
                  "audita seis meses depois conclui que ninguem leu aquela ficha.",
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": "        if (self.espelho() && !t.tem_espelho()) || t.tem_dado_pessoal() {",
        "troca": "        // DEFEITO REPOSTO: a pista de leitura aceita a tabela com\n"
                 "        // coluna marcada, e o registro de acesso nunca e gravado.\n"
                 "        if self.espelho() && !t.tem_espelho() {",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_da_ficha_compartilhada::"
            "a_trilha_de_dado_pessoal_sobrevive_a_pista_de_leitura"
        ],
        "seguem": [
            "servidor::testes_da_ficha_compartilhada::sem_a_ficha_compartilhada_nada_muda",
            "servidor::testes_da_ficha_compartilhada::o_espelho_continua_nascendo_no_varrer",
            "servidor::testes_da_ficha_compartilhada::"
            "quatro_leitores_ao_mesmo_tempo_leem_a_mesma_pagina",
        ],
    },
    {
        "id": "pista-de-leitura-nao-espelha",
        "titulo": "a pista de leitura aceita tabela sem `.bkp` e o espelho deixa de nascer",
        "porque": "com `recursos.espelho` ligado, ABRIR uma tabela sem espelho o CRIA -- "
                  "e criar arquivo e escrever. E a segunda das duas escritas que moram "
                  "fora do construtor, e a que some sem ninguem perceber: a tabela "
                  "continua respondendo, so fica sem a copia que o `reparar` usa.",
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": "        if (self.espelho() && !t.tem_espelho()) || t.tem_dado_pessoal() {",
        "troca": "        // DEFEITO REPOSTO: a pista de leitura aceita a tabela que\n"
                 "        // ainda precisa ser espelhada, e o `.bkp` nunca nasce.\n"
                 "        if t.tem_dado_pessoal() {",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_da_ficha_compartilhada::o_espelho_continua_nascendo_no_varrer"
        ],
        "seguem": [
            "servidor::testes_da_ficha_compartilhada::sem_a_ficha_compartilhada_nada_muda",
            "servidor::testes_da_ficha_compartilhada::"
            "a_trilha_de_dado_pessoal_sobrevive_a_pista_de_leitura",
        ],
    },
    {
        "id": "leitura-sem-recuo-para-a-exclusiva",
        "titulo": "a tabela que pede a ficha exclusiva vira erro em vez de recuo",
        "porque": "guarda nova entra PEDIDA, nao imposta: quem chama nao pediu pista "
                  "nenhuma e nao pode receber erro por causa dela. O recuo e o que faz "
                  "o comportamento visto de fora nao mudar para tabela nenhuma -- e sem "
                  "ele, toda tabela nascida antes do `.trash` para de responder ao "
                  "`varrer`.",
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": "        let Aberta::Pronta(mut t) = raiz.abrir_para_ler(database, tabela)? else {\n"
                  "            return Ok(None);\n"
                  "        };",
        "troca": "        // DEFEITO REPOSTO: sem recuo, a tabela que precisaria\n"
                 "        // escrever para abrir passa a responder erro.\n"
                 "        let Aberta::Pronta(mut t) = raiz.abrir_para_ler(database, tabela)? else {\n"
                 "            return Err(PhxError::Corrompido(\"sem recuo\".into()));\n"
                 "        };",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_da_ficha_compartilhada::sem_a_ficha_compartilhada_nada_muda"
        ],
        "seguem": [
            "servidor::testes_da_ficha_compartilhada::"
            "quatro_leitores_ao_mesmo_tempo_leem_a_mesma_pagina",
            "servidor::testes_janela_e_cadeia::so_as_duas_operacoes_medidas_usam_a_ficha_compartilhada",
        ],
    },
    {
        "id": "abrir-para-ler-cria-a-lixeira",
        "titulo": "abrir para LER cria o `.trash` que falta, sob a ficha compartilhada",
        "porque": "e o achado que quase custou a entrega: a garantia por tipo escondia "
                  "os METODOS de escrita, e quatro das seis escritas de uma varredura "
                  "moram dentro do CONSTRUTOR. Dois leitores simultaneos criariam o "
                  "mesmo arquivo: o `create_new` do segundo falha, ou ele le o "
                  "cabecalho que o primeiro ainda nao terminou de gravar.",
        "arquivo": "crates/phxsql-store/src/lixeira.rs",
        "trecho": "        let volumes = Volumes::novo(&diretorio, nome, EXT_TRASH, paginacao);\n"
                  "        if volumes.existentes().is_empty() {\n"
                  "            return Ok(None);\n"
                  "        }\n"
                  "        LixeiraFile::abrir(diretorio, nome, paginacao).map(Some)",
        "troca": "        // DEFEITO REPOSTO: a abertura somente-leitura volta a CRIAR o\n"
                 "        // `.trash` quando ele falta, que e escrever sob a ficha\n"
                 "        // compartilhada.\n"
                 "        LixeiraFile::abrir(diretorio, nome, paginacao).map(Some)",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "leitura::testes::"
            "a_tabela_que_precisaria_escrever_para_abrir_manda_para_a_exclusiva"
        ],
        "seguem": ["leitura::testes::as_duas_fichas_leem_a_mesma_pagina"],
    },
    {
        "id": "leitura-sem-guarda-de-reentrancia",
        "titulo": "a ficha compartilhada pedida com a exclusiva na mão pendura o servidor",
        "porque": "e a guarda IRMA da `trava-sem-guarda-de-reentrancia`, e ela nasceu "
                  "junto com a segunda porta. O `RwLock` piora o abraco: com um "
                  "escritor na fila, a segunda leitura da mesma thread trava as tres "
                  "pontas -- ela, o escritor, e todo leitor que chegar depois. A "
                  "`COM_A_TRAVA` e UMA para as duas portas de proposito, e e isso que "
                  "esta entrada prova.",
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """    fn travar_dados_para_ler(&self) -> Result<TravaDeLeitura<'_>> {
        if COM_A_TRAVA.with(std::cell::Cell::get) {
            return Err(trava_reentrante());
        }
""",
        "troca": """    fn travar_dados_para_ler(&self) -> Result<TravaDeLeitura<'_>> {
        // DEFEITO REPOSTO: a porta de leitura sem a pergunta. A mesma thread
        // que ja tem a exclusiva pede a compartilhada e espera por si mesma.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "prazo": 420,
        "caem": [
            "servidor::testes_janela_e_cadeia::as_duas_fichas_na_mesma_thread_viram_erro"
        ],
        "seguem": [
            "servidor::testes_janela_e_cadeia::"
            "a_trava_pedida_duas_vezes_pela_mesma_thread_vira_erro",
            "servidor::testes_janela_e_cadeia::sem_reentrancia_nada_muda",
        ],
    },
    {
        "id": "familia-pela-grafia-crua",
        "titulo": "a grafia do caminho divide a família do registro de `fsync`, e o volume sujo fica para trás",
        "porque": "o comentario do proprio `ESCRITAS_PENDENTES` afirmava que duas "
                  "grafias dariam duas familias e que a degradacao era benigna -- "
                  "«nunca para menos que o comportamento antigo». A sonda mediu com "
                  "`strace` que o comportamento antigo NAO alcanca o volume do meio, "
                  "entao a familia partida perde dado, e so numa queda de energia. "
                  "*A lista do que falta tambem e palpite ate alguem medir*, e uma "
                  "afirmacao de «isto e benigno» e a mesma familia de palpite.",
        "arquivo": "crates/phxsql-store/src/volume.rs",
        "trecho": """fn familia(diretorio: &Path, nome: &str, ext: &str) -> PathBuf {
    let arquivo = format!("{nome}.{ext}");
    match absoluto_lexico(diretorio) {
        Some(a) => a.join(arquivo),
        None => diretorio.join(arquivo),
    }
}""",
        "troca": """fn familia(diretorio: &Path, nome: &str, ext: &str) -> PathBuf {
    // DEFEITO REPOSTO: a chave da familia pelo caminho CRU. Quem abre por
    // `dados/loja` e quem fecha a janela por `/srv/dados/loja` entram em duas
    // familias, e a marca de quem escreveu nao chega a quem sincroniza.
    let arquivo = format!("{nome}.{ext}");
    let _ = absoluto_lexico(diretorio);
    diretorio.join(arquivo)
}""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "grafia-do-diretorio-nao-divide-a-familia"],
        "prazo": 420,
        "caem": ["o_fecho_por_outra_grafia_alcanca_o_volume_sujo"],
        "seguem": ["a_tabela_resolve_o_diretorio_uma_vez_ao_abrir"],
    },
    {
        "id": "pag-gravado-com-truncagem",
        "titulo": "o `.pag` escrito com `fs::write` aparece pela metade para quem lê de fora",
        "porque": "arquivo derivado quer ATOMICIDADE, nao durabilidade: um `.pag` "
                  "perdido se regrava sozinho, um `.pag` pela metade mente para a "
                  "unica plateia que ele tem. E a janela nao precisa de queda -- o "
                  "`O_TRUNC` a abre a cada `sincronizar()`, por 33,2 us medidos.",
        "arquivo": "crates/phxsql-store/src/pag.rs",
        "trecho": """    if let Err(e) = std::fs::write(&temporario, texto) {
        let _ = std::fs::remove_file(&temporario);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&temporario, &caminho) {
        let _ = std::fs::remove_file(&temporario);
        return Err(e.into());
    }""",
        "troca": """    // DEFEITO REPOSTO: `fs::write` direto no alvo. Ele abre com `O_TRUNC`,
    // entao o arquivo fica vazio ou partido enquanto a escrita nao termina.
    let _ = &temporario;
    std::fs::write(&caminho, texto)?;""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "pag-se-troca-inteiro"],
        "prazo": 420,
        "caem": ["quem_le_de_fora_nunca_pega_o_pag_pela_metade"],
        "seguem": [],
    },
    {
        "id": "pagina-anterior-de-um-em-um",
        "titulo": "a página anterior anda de um em um pelo vazio entre baldes — e ali o `ler` cru RECUSA em vez de dizer «vazio»",
        "porque": "*Conserto entra no caminho que o motivou, e o caminho IRMAO fica.* "
                  "O `pagina_depois_de` nasceu sabendo saltar o vazio entre baldes "
                  "(pelo `proximo_ativo`); o `pagina_antes_de` ficou com o laco de "
                  "um em um e o `ler` cru. Na alfanumerica o slot alem do `usados` "
                  "do balde NAO EXISTE, e `conferir_faixa` devolve `NaoEncontrado` "
                  "-- entao o efeito nao era lentidao, era recusa. Pela porta de "
                  "dados o `varrer` monta o `ha_antes` com essa funcao, e toda "
                  "pagina que comecasse no primeiro slot de um balde voltava "
                  "«rowid N nao existe» no lugar das linhas. Os 16 testes de "
                  "`alfanumerica.rs` provavam a ida; nenhum provava a volta.",
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        let mut ate = cursor - 1;
        while let Some((id, payload)) = self.reg.anterior_ativo(ate)? {
            if self.visivel(id, Some(&payload), visao)? {
                saida.push(id);
                if limite > 0 && saida.len() as u64 >= limite {
                    break;
                }
            }
            if id == 1 {
                break;
            }
            ate = id - 1;
        }""",
        "troca": """        // DEFEITO REPOSTO: o laco de tras para a frente de um em um, com o
        // `ler` cru. Na alfanumerica o slot alem do `usados` do balde nao
        // existe, e o `ler` responde `NaoEncontrado` em vez de `None`.
        let mut rowid = cursor - 1;
        while rowid >= 1 {
            if let Some(payload) = self.reg.ler(rowid)? {
                if self.visivel(rowid, Some(&payload), visao)? {
                    saida.push(rowid);
                    if limite > 0 && saida.len() as u64 >= limite {
                        break;
                    }
                }
            }
            if rowid == 1 {
                break;
            }
            rowid -= 1;
        }""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "alfanumerica"],
        "prazo": 420,
        "caem": ["a_pagina_anterior_atravessa_o_vazio_entre_baldes"],
        "seguem": ["a_varredura_salta_os_vazios_entre_baldes",
                   "a_ordem_de_digitacao_esta_no_rownum",
                   "achar_pelo_numero_de_ordem_continua_certo_com_baldes"],
    },
    # -----------------------------------------------------------------------
    # O fecho de janela que sincroniza as K tabelas AO MESMO TEMPO (pedido 180)
    # -----------------------------------------------------------------------
    {
        "id": "fecho-em-paralelo-engole-o-erro",
        "titulo": "o `fsync` que falha dentro do fio, e o `join` que engole o erro",
        "porque": (
            "o comboio do fecho e 93-96% `fsync`, e o conserto foi sincronizar "
            "as K tabelas sujas ao mesmo tempo em vez de em laco. Isso poe um "
            "`join` entre o `fsync` e a decisao de apagar as marcas de commit, "
            "e um erro engolido ali vira marca apagada sem o dado no disco -- "
            "a marca e a UNICA coisa que traz o commit de volta. O caminho e "
            "diferente do erro de ABERTURA (esse tem guarda propria): so o "
            "`join` cobre a falha de quem ja subiu. A matriz de queda esta na "
            "secao 12.6 do `docs/CONCORRENCIA.md`."
        ),
        # ATUALIZADO em 16/09/2026 (pedido 263). `2d33c5c` (16/09, a saude do
        # disco) fez o `quebrados` carregar o ERRO junto do indice
        # (`Vec<(usize, Option<PhxError>)>`), para o `fecho_recusado` poder
        # avisar por e-mail e SMS. O `join` continua sendo o unico lugar em
        # que a falha de quem ja subiu chega, e o defeito reposto continua
        # sendo o mesmo: as duas pernas de erro viram `None` e o indice nunca
        # volta para o `faltaram`. O `None::<usize>.map(...)` sobrevive de
        # proposito -- ele engole sem que o compilador reclame de braco
        # inalcancavel, e mantem visivel que o `i` existia e foi jogado fora.
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                    .filter_map(|(i, f)| match f.join() {
                        Ok(Ok(())) => None,
                        Ok(Err(e)) => Some((i, Some(e))),
                        Err(_) => Some((i, None)),
                    })""",
        "troca": """                    // DEFEITO REPOSTO: o erro do fio some no `join`, e o
                    // fecho segue como se todas tivessem sincronizado.
                    .filter_map(|(i, f)| match f.join() {
                        Ok(Ok(())) => None,
                        _ => None::<usize>.map(|_: usize| (i, None::<PhxError>)),
                    })""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_janela_e_cadeia::fsync_que_falha_no_fio_tambem_segura_as_marcas",
        ],
        "seguem": [
            "servidor::testes_janela_e_cadeia::com_todas_sincronizadas_as_marcas_saem",
            "servidor::testes_janela_e_cadeia::uma_tabela_so_grava_como_sempre",
        ],
        "prazo": 420,
    },
    {
        "id": "fecho-em-paralelo-fio-que-nao-sobe",
        "titulo": "uma tabela do fecho fica sem fio, e ninguém percebe",
        "porque": (
            "e o defeito que NENHUMA das outras asercoes pega, e por isso ele "
            "esta aqui: com uma tabela fora do arranjo, as marcas saem, o "
            "conjunto de sujas esvazia, o dado aparece na tela -- e ele so "
            "existe no cache do nucleo. Quem acusa e a asercao que mede o "
            "FATO, `volume::familias_devendo_em`, porque uma familia so sai do "
            "registro de escritas pendentes depois do `fsync`. Os contadores "
            "que medem a INTENCAO (`sincronizacoes()`, `selo()`) sobem ANTES "
            "do laco e passariam -- foi assim que o pedido 186 escapou de um "
            "teste."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                let fios: Vec<_> = abertas
                    .iter_mut()
                    .map(|t| escopo.spawn(move || t.sincronizar()))
                    .collect();""",
        "troca": """                // DEFEITO REPOSTO: a primeira tabela do pedaco nao
                // ganha fio, e ninguem a sincroniza.
                let fios: Vec<_> = abertas
                    .iter_mut()
                    .skip(1)
                    .map(|t| escopo.spawn(move || t.sincronizar()))
                    .collect();""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_janela_e_cadeia::com_todas_sincronizadas_as_marcas_saem",
        ],
        "seguem": [
            "servidor::testes_janela_e_cadeia::uma_tabela_so_grava_como_sempre",
            "servidor::testes_janela_e_cadeia::tabela_que_nao_sincroniza_segura_as_marcas",
        ],
        "prazo": 420,
    },
    {
        "id": "pagina-ordenada-varre-o-indice-inteiro",
        "titulo": "a grade ordenada percorre o índice inteiro para devolver 50 linhas",
        "porque": (
            "pedido 188. O conserto de 02/09/2026 fez a pagina PARAR -- e "
            "parou so o `.reg`. O `varrer_indice` continuou percorrendo o "
            "`.ndx` inteiro antes de qualquer recorte, e o comentario que se "
            "declarava resolvido era o motivo de ninguem olhar de novo. "
            "Medido pelo fio numa tabela de 1.000.000 com pagina de 50: "
            "54,81 ms e 8.335 paginas do indice, contra 0,56 ms e 3 paginas "
            "depois; na tela, num navegador de verdade, a espera ia de 48 ms "
            "para 98 ms. A guarda mede o EFEITO (paginas do `.ndx` tocadas) e "
            "nao o veredito, porque o resultado NAO mudava: as mesmas 50 "
            "linhas, na mesma ordem."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": TRECHO_PAGINA_ORDENADA,
        "troca": TROCA_PAGINA_ORDENADA,
        "pacote": "phxsql-store",
        "alvo": ["--test", "paginacao"],
        "caem": [
            "a_pagina_ordenada_nao_percorre_o_indice_inteiro",
            "a_pagina_ordenada_custa_o_mesmo_em_tabela_dez_vezes_maior",
        ],
        "seguem": [
            "a_pagina_por_indice_que_para_devolve_o_mesmo_que_a_que_lia_tudo",
            "a_pagina_respeita_a_visao",
            "as_paginas_reconstroem_a_varredura_inteira",
        ],
        "prazo": 300,
    },
    {
        "id": "cursor-do-pedaco-sem-o-mais-um",
        "titulo": "o cursor da varredura em pedaços devolve de novo a linha da borda",
        "porque": (
            "e o defeito que a varredura em pedacos do pedido 188 criou: "
            "`descer` para NA entrada de `apos`, que ja foi entregue, entao "
            "sem o `+1` a borda de cada pedaco volta em dobro. Ele so aparece "
            "quando o laco da a SEGUNDA volta, e ela so acontece com linha "
            "invisivel no caminho -- a primeira versao da prova varria tabela "
            "sem exclusao nenhuma e PASSAVA com o defeito reposto."
        ),
        "arquivo": "crates/phxsql-store/src/ndx.rs",
        "trecho": TRECHO_CURSOR,
        "troca": TROCA_CURSOR,
        "pacote": "phxsql-store",
        "alvo": ["--test", "paginacao"],
        "caem": [
            "a_varredura_em_pedacos_costura_a_mesma_ordem",
            "a_pagina_por_indice_que_para_devolve_o_mesmo_que_a_que_lia_tudo",
        ],
        "seguem": [
            "a_pagina_ordenada_nao_percorre_o_indice_inteiro",
            "as_paginas_reconstroem_a_varredura_inteira",
        ],
        "prazo": 300,
    },
    {
        "id": "perfil-grava-o-texto-da-tabela-declarada",
        "titulo": "o perfil.txt grava em claro o pedido de uma tabela declarada em cifra.tabelas",
        "porque": (
            "e o furo que a frente CIFRA-POR-TABELA achou medindo: o Profiler "
            "guardava `redigir(linha_crua)` -- o pedido inteiro, redigido so "
            "por NOME de campo -- e escrevia isso num arquivo de texto ao lado "
            "do .reg cifrado. `SEGREDOS` tem `senha`, `token`, `chave`... e "
            "NAO tem `linha`. Quem leva o disco leva o perfil.txt, e isso anula "
            "o proposito escrito no proprio cofre: «protege o ARQUIVO COPIADO»."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": TRECHO_PERFIL_SEM_TEXTO,
        "troca": TROCA_PERFIL_SEM_TEXTO,
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes_tabela_sigilosa::o_anel_ve_o_texto_e_o_arquivo_nao",
            "profiler::testes_tabela_sigilosa::a_declaracao_e_por_banco_e_nao_pelo_nome_curto",
            "profiler::testes_tabela_sigilosa::tabela_escondida_em_juncao_uniao_e_pivot_tambem_cega_o_arquivo",
            "profiler::testes_tabela_sigilosa::declarar_com_o_profiler_ligado_vale_do_pedido_seguinte_em_diante",
        ],
        "seguem": [
            "profiler::testes_tabela_sigilosa::sem_lista_o_arquivo_continua_com_o_texto",
            "profiler::testes::a_senha_nunca_aparece",
        ],
        "prazo": 300,
    },
    {
        "id": "perfil-decide-so-pela-lista-e-nao-pelo-reg-cifrado",
        "titulo": "o perfil.txt decide pela lista do config e a cifra acontece pela marca de coluna",
        "porque": (
            "e a petrea do portao unico em outra roupa, achada pela varredura "
            "SEC de 18/09/2026 (pedido 356): o portao lia `cifra.tabelas` -- a "
            "INTENCAO -- e quem cifra e a marca de coluna, gravada no cabecalho "
            "do .reg na criacao. Como a lista nasce VAZIA em todo config.json, "
            "o caso comum era o furo: cofre ligado, coluna marcada, ninguem "
            "declarou nada, e o valor marcado ia em claro para o arquivo que "
            "viaja com o disco. A §13.3 do SEGURANCA.md ja dizia «o disco "
            "manda, a lista pede» desde 05/09 -- e o decisor nao seguia a "
            "propria secao."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": TRECHO_PERFIL_PELO_DISCO,
        "troca": TROCA_PERFIL_PELO_DISCO,
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes_reg_cifrado::reg_cifrado_sem_lista_nenhuma_cega_o_arquivo",
            "profiler::testes_reg_cifrado::reg_ilegivel_nao_arrisca_o_texto",
        ],
        # Os dois primeiros sao o comportamento VELHO com o disco sendo
        # consultado: sem eles, um conserto que cegasse TODA tabela passaria
        # pela guarda como se tivesse protegido alguma coisa.
        "seguem": [
            "profiler::testes_reg_cifrado::reg_em_claro_continua_com_o_texto",
            "profiler::testes_reg_cifrado::tabela_sem_volume_continua_com_o_texto",
            "profiler::testes_tabela_sigilosa::sem_lista_o_arquivo_continua_com_o_texto",
        ],
        "prazo": 300,
    },
    {
        "id": "perfil-grava-o-erro-que-cita-o-valor",
        "titulo": "o perfil.txt tapa o pedido e grava o erro, que cita o valor da coluna marcada",
        "porque": (
            "achado procurando quem NAO tinha o campo novo, em 18/09/2026, e "
            "ele estava na coluna ao lado da que acabara de ser tapada: o "
            "texto do erro CITA o valor que chegou -- «123456 nao cabe em "
            "inteiro de 16 bits» --, e ali o valor era de uma coluna marcada. "
            "Pior, a §13.9 do SEGURANCA.md afirmava com veredito que o campo "
            "`erro` nao carrega valor de linha: a conferencia tinha olhado "
            "DUAS familias de mensagem (indice unico e FK, que citam nome) e "
            "concluido sobre todas. Agora o erro de evento sigiloso vira o "
            "TAMANHO -- analisando, nunca recortando."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": TRECHO_PERFIL_ERRO_SEM_TEXTO,
        "troca": TROCA_PERFIL_ERRO_SEM_TEXTO,
        "pacote": "phxsql-server",
        "alvo": ["--test", "profiler-da-tabela-cifrada"],
        "caem": [
            "o_erro_que_cita_o_valor_tambem_fica_fora_do_arquivo",
        ],
        # O outro teste do mesmo binario tem de seguir de pe: ele prova o
        # PEDIDO tapado, e a troca aqui so mexe na coluna do erro.
        "seguem": [
            "tabela_cifrada_pela_marca_de_coluna_tambem_cega_o_arquivo",
        ],
        "prazo": 300,
    },
    {
        "id": "profiler-ligado-sem-a-raiz-dos-dados",
        "titulo": "o Profiler liga sem a raiz de dados e volta a decidir por um campo só",
        "porque": (
            "e o irmao exato do `definir_sigilosas` que ninguem chamasse: o "
            "conserto do pedido 356 mora no profiler.rs e depende de haver "
            "disco a quem perguntar. Sem esta linha no `profiler_ligar`, os "
            "seis testes de `testes_reg_cifrado` continuam passando -- eles "
            "mesmos definem a raiz -- e o perfil.txt de um servidor de verdade "
            "volta a gravar em claro, calado."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": TRECHO_RAIZ_NO_LIGAR,
        "troca": TROCA_RAIZ_NO_LIGAR,
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_profiler_desligado::a_raiz_dos_dados_chega_ao_profiler_no_ligar",
        ],
        "seguem": [
            "servidor::testes_profiler_desligado::a_lista_de_tabelas_declaradas_chega_ao_profiler_no_ligar",
            "profiler::testes_tabela_sigilosa::o_anel_ve_o_texto_e_o_arquivo_nao",
        ],
        "prazo": 300,
    },
    {
        "id": "perfil-so-olha-a-tabela-do-primeiro-nivel",
        "titulo": "o Profiler so olha a tabela do primeiro nível e a junção vira a porta dos fundos",
        "porque": (
            "e a mesma porta dos fundos que o portao de permissao ja teve, e o "
            "CLAUDE.md a nomeia: `juntar` guarda as tabelas em `a.tabela` e "
            "`b.tabela`, `unir` numa LISTA, e `pivotar` poe a tabela de fatos "
            "no campo que se le e as de consulta dentro de um `juntar` "
            "aninhado. Sem descer na arvore, bastaria pedir a tabela declarada "
            "como lado B de uma juncao para o pedido inteiro ir para o "
            "perfil.txt em claro -- e nenhum outro teste acusa."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": TRECHO_COLHER_DESCE,
        "troca": TROCA_COLHER_DESCE,
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes_tabela_sigilosa::tabela_escondida_em_juncao_uniao_e_pivot_tambem_cega_o_arquivo",
        ],
        "seguem": [
            "profiler::testes_tabela_sigilosa::o_anel_ve_o_texto_e_o_arquivo_nao",
            "profiler::testes_tabela_sigilosa::sem_lista_o_arquivo_continua_com_o_texto",
        ],
        "prazo": 300,
    },
    {
        "id": "fase-da-telemetria-com-dado-do-usuario",
        "titulo": "a fase do SQL Check passa a carregar dado do usuário, e o furo nasce calado",
        "porque": (
            "aqui nao havia conserto -- havia risco de regressao, e do tipo "
            "que nasce parecendo melhoria. A `fase` da telemetria so recebe "
            "frase fixa escrita no codigo; no dia em que alguem escrever "
            "`fase_cancelavel(&format!(\"lendo {} \", chave))` para ajudar no "
            "diagnostico, o dado do usuario sai na resposta de `telemetria`, "
            "que nao tem portao de tabela nenhum. A guarda le o FONTE e exige "
            "que o argumento comece com aspas."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": TRECHO_FASE_FIXA,
        "troca": TROCA_FASE_FIXA,
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "telemetria::testes::a_fase_da_telemetria_so_aceita_frase_fixa",
        ],
        "seguem": [
            "telemetria::testes::toda_operacao_com_ponto_de_cancelamento_esta_na_lista",
        ],
        "prazo": 300,
    },
    {
        "id": "cache-de-derivadas-sobrevive-a-troca-de-senha",
        "titulo": "o cache de chaves derivadas responde a quem não deu a senha",
        "porque": (
            "`cofre::derivar` consulta o cache ANTES de olhar o COFRE, e "
            "responde sem nunca perguntar quem esta pedindo. Com UMA senha do "
            "processo isso e correto -- o que segura a correcao e o "
            "`definir_com` esvaziar o cache inteiro. Ha um desenho na mesa "
            "(senha do banco vinda do login, guardada na sessao) em que tirar "
            "esse esvaziamento parece a otimizacao obvia, porque evita os "
            "290 ms do PBKDF2 ao alternar de banco -- e no dia em que alguem o "
            "tirar, a garantia «so quem sabe a senha le» some SEM erro, SEM log "
            "e sem teste vermelho: o primeiro login poria a chave no cache do "
            "PROCESSO. Nao ha defeito hoje; ha uma porta, e esta e a tranca."
        ),
        "arquivo": "crates/phxsql-store/src/cofre.rs",
        "trecho": TRECHO_CACHE_ESVAZIA,
        "troca": TROCA_CACHE_ESVAZIA,
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-diarios"],
        # So UM cai, e o executor mediu isso: `arquivo_cifrado_sem_a_chave_certa`
        # continua verde porque este defeito tira o esvaziamento do
        # `definir_com` e nao o do `desligar` -- e e o do `desligar` que aquele
        # teste exercita. Listar os dois teria dado «nao pegou», que e o
        # executor fazendo o seu trabalho: teste que passa por engano e pior
        # que teste que falta.
        "caem": [
            "o_cache_de_derivadas_nao_responde_a_quem_nao_deu_a_senha",
        ],
        "seguem": [
            "arquivo_cifrado_sem_a_chave_certa_da_erro_claro",
            "arquivo_escrito_antes_da_cifra_continua_abrindo",
            "sem_configuracao_nada_muda_no_disco",
            "o_dado_some_do_disco_e_volta_pela_leitura",
        ],
        "prazo": 300,
    },
    # -----------------------------------------------------------------------
    # O indice de texto: o irmao que era um ARQUIVO
    #
    # As tres saem do mesmo achado, e o achado esta em
    # `docs/cognicao/cognicao_o-irmao-pode-ser-um-arquivo_20260907_0130.md`:
    # o `.fts` embrulha um `.ndx`, entao herdou a marca de "ficou para tras
    # numa queda" -- e nao herdou o conserto. Procurei irmao entre as FUNCOES
    # e nao achei nenhum; o irmao era o arquivo.
    # -----------------------------------------------------------------------
    {
        "id": "fts-reindexar-sem-o-irmao",
        "titulo": "o reindexar reconstrói só o .ndx, e a queda trava a tabela para sempre",
        "porque": (
            "petrea do CLAUDE.md: conserto entra no caminho que o motivou, e o "
            "caminho IRMAO fica. Aqui o irmao nao era uma funcao ao lado -- era "
            "o `.fts`, que e um `.ndx` por dentro. Enquanto a marca da queda "
            "estiver de pe, TODA operacao de indice recusa: a tabela para de "
            "gravar e o `reindexar` responde Ok. E a mensagem do proprio `.fts` "
            "manda «reconstrua o indice de texto com `reindexar`» -- ordem que o "
            "codigo nao sabia cumprir."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        self.reconstruir_fts()?;
        self.ndx.verificar()
""",
        "troca": """        // DEFEITO REPOSTO: o `reindexar` volta a olhar so o `.ndx`. O `.fts`
        // fica com a marca da queda, e nenhuma gravacao passa depois disso.
        self.ndx.verificar()
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "indice-de-texto"],
        "caem": ["a_queda_marca_o_fts_e_o_reindexar_tem_de_baixar_a_marca"],
        "seguem": [
            "acha_no_titulo_e_no_corpo",
            "excluir_de_vez_tira_do_indice",
            "o_indice_acha_o_mesmo_que_a_varredura",
        ],
    },
    {
        "id": "fts-reconstruir-sem-recriar",
        "titulo": "reconstruir o índice de texto sem recriar o arquivo não é idempotente",
        "porque": (
            "eu previ o defeito errado, e a prova real mediu: achei que a "
            "segunda passada duplicaria a ocorrencia (achar a mais, que e "
            "mentira). A arvore RECUSA -- `Duplicado(chave completa ja existe "
            "no indice)` --, e quem ia bater nessa recusa era o `reindexar`, em "
            "toda tabela com pelo menos uma linha. So a recriacao tira a marca "
            "de queda, de proposito: senao bastaria abrir e fechar para o "
            "defeito virar invisivel."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        self.fts = Some(FtsFile::recriar(
            caminho(&self.diretorio, &self.nome, EXT_FTS),
            dobra,
            texto_sobre_coluna_marcada(&self.esquema),
        )?);
""",
        "troca": """        // DEFEITO REPOSTO: varre por cima do indice que ja existe, em vez de
        // recriar o arquivo. A segunda passada bate em chave repetida.
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "indice-de-texto"],
        "caem": ["reconstruir_duas_vezes_nao_duplica"],
        "seguem": ["acha_no_titulo_e_no_corpo", "excluir_de_vez_tira_do_indice"],
    },
    {
        "id": "fts-abrir-recusa-a-tabela",
        "titulo": "o .fts ilegível derruba a tabela inteira, em vez de se refazer",
        "porque": (
            "o `.fts` e DERIVADO: perde-lo custa uma varredura e nunca custa "
            "dado. Recusar a tabela aqui manda rodar o `reindexar` numa tabela "
            "que nao abre -- a mesma ordem impossivel por outro caminho. Vale "
            "para o arquivo corrompido e para a contagem divergente, que e o "
            "caso de quem declarou um indice de texto numa tabela com dados."
        ),
        # Esta entrada JA QUEBROU uma vez, no mesmo dia em que nasceu: foi
        # provada 1/1, e horas depois a correcao da pista de leitura
        # acrescentou um brace `Err(_) if !escrever` DENTRO deste mesmo
        # `match`. Prova individual nao sobrevive a uma mudanca no trecho --
        # quem acusa e a corrida COMPLETA, e por isso ela roda no fim da
        # rodada e nao so quando a guarda entra.
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """            match FtsFile::abrir(&caminho_fts, dobra.clone()) {
                Ok(f) => Some(f),
                // **Falta de CHAVE nao cai na vala do "refaz".** A vala existe
                // para `.fts` corrompido ou divergente, onde refazer do `.reg`
                // devolve a verdade. Aqui ela devolveria um `.fts` NOVO e em
                // claro, com os termos da coluna marcada de volta ao disco
                // legiveis -- desfazendo calado a protecao do pedido 340 por
                // causa de uma senha errada no `config.json`. Recusa
                // nomeando, que e o que o `.reg` faz na mesma situacao.
                Err(e) if matches!(e, PhxError::Autorizacao(_)) => return Err(e),
                Err(_) if !escrever => {
                    return Ok(SemEscrever::PrecisaEscrever(
                        "o indice de texto .fts nao abre e seria refeito",
                    ));
                }
                Err(_) => {
                    refazer = true;
                    Some(FtsFile::recriar(&caminho_fts, dobra, selar_o_fts)?)
                }
            }
""",
        "troca": """            // DEFEITO REPOSTO: `.fts` que nao abre volta a derrubar a tabela.
            Some(FtsFile::abrir(&caminho_fts, dobra.clone())?)
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "indice-de-texto"],
        "caem": ["fts_corrompido_reconstroi_na_abertura_em_vez_de_derrubar_a_tabela"],
        "seguem": [
            "apagar_o_fts_e_reabrir_reconstroi_em_vez_de_achar_nada",
            "acha_no_titulo_e_no_corpo",
        ],
    },
    {
        "id": "fts-nasce-na-pista-de-leitura",
        "titulo": "a pista de leitura cria o .fts, e escrever sob a ficha compartilhada é o que ela existe para impedir",
        "porque": (
            "`abrir_com` recebe `escrever`, e os quatro componentes que "
            "escrevem na abertura (a troca de volume do `.reg`, a cura do "
            "`.log`, o `.trash` e o `.reason`) recusam com o motivo quando ela "
            "e falsa. O `.fts` nasceu fora dessa conferencia -- e refaze-lo "
            "escreve, com N leitores nos mesmos arquivos."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        } else if refazer {
            if !escrever {
                return Ok(SemEscrever::PrecisaEscrever(
                    "o indice de texto .fts desta tabela ainda nao existe e seria criado",
                ));
            }
            Some(FtsFile::recriar(&caminho_fts, dobra, selar_o_fts)?)
""",
        "troca": """        } else if refazer {
            // DEFEITO REPOSTO: o `.fts` volta a nascer sem olhar a ficha.
            Some(FtsFile::recriar(&caminho_fts, dobra)?)
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "indice-de-texto"],
        "caem": ["a_pista_de_leitura_recusa_criar_o_fts"],
        "seguem": [
            "acha_no_titulo_e_no_corpo",
            "apagar_o_fts_e_reabrir_reconstroi_em_vez_de_achar_nada",
        ],
    },
    {
        "id": "fts-chave-truncada-nao-se-declara",
        "titulo": "a chave truncada não se declara truncada, e a busca acha a mais",
        "porque": (
            "a chave guarda 26 bytes do termo mais o comprimento. Duas "
            "palavras de 32 letras com os mesmos 26 primeiros bytes caem na "
            "MESMA chave. Sem o aviso, a `Table` nao confere a linha e devolve "
            "as duas -- achar a mais e mentira que o cliente nao percebe."
        ),
        "arquivo": "crates/phxsql-store/src/fts.rs",
        "trecho": """        let conferir = t.len() > self.termo_len();
""",
        "troca": """        // DEFEITO REPOSTO: a chave truncada deixa de se declarar truncada, e
        // entao a `Table` nao confere a linha antes de devolver.
        let conferir = false;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "indice-de-texto"],
        "caem": ["palavra_longa_confere_a_linha_em_vez_de_achar_a_mais"],
        "seguem": [
            "acha_no_titulo_e_no_corpo",
            "o_indice_acha_o_mesmo_que_a_varredura",
        ],
    },
    {
        "id": "operacao-sem-poder-declarado",
        "titulo": "operação catalogada sem linha de poder vira administrador em silêncio",
        "porque": (
            "o `_ => Administrar` fecha a porta para o DESCONHECIDO, e isso "
            "esta certo. O que ele nao distingue e a operacao que esta no "
            "catalogo e esqueceu a linha: ela nasce exigindo o maior poder, "
            "ninguem e avisado, e o sintoma chega como «nao consigo usar isto» "
            "de quem podia. Aconteceu com o `procurar_texto`, e o conferidor "
            "achou mais 13 caindo assim."
        ),
        "arquivo": "crates/phxsql-server/src/usuarios.rs",
        "trecho": """            "procurar_texto" => Atividade::Ler,
""",
        "troca": """            // DEFEITO REPOSTO: a operacao perde a linha e cai no `_`.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "usuarios::tests::toda_operacao_do_catalogo_declara_o_poder_que_pede",
            "servidor::testes_direito_por_tabela::procurar_texto_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
        ],
        "seguem": [
            "usuarios::tests::a_memoria_pede_leitura_e_o_backup_pede_administrar",
            "servidor::testes_direito_por_tabela::a_estrela_de_tabela_vale_para_as_nao_listadas",
        ],
    },
    {
        "id": "sequencia-numero-cru-perde-precisao",
        "titulo": "id acima de 2⁵³ mandado como número cru é gravado trocado, calado",
        "porque": (
            "docs/AUTONUMBER.md bloco 19 -- o Json desta casa so tem "
            "Numero(f64), e acima de 2^53 o inteiro cru volta arredondado. O "
            "teto do formato (2^64-1) nao e o teto do fio; recusar cedo troca "
            "corrupcao silenciosa por erro lido."
        ),
        "arquivo": "crates/phxsql-server/src/valores.rs",
        "trecho": """            if j.inteiro_impreciso() {""",
        "troca": """            if false && j.inteiro_impreciso() {""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "valores::testes_inteiro_em_texto::sequencia_recusa_numero_cru_acima_do_teto_do_f64",
        ],
        "seguem": [
            "valores::testes_inteiro_em_texto::sequencia_grande_como_texto_atravessa_intacta",
        ],
    },
    {
        "id": "sequencia-grande-sai-numero-mentiroso",
        "titulo": "id acima de 2⁵³ já gravado sai do servidor como número f64 trocado",
        "porque": (
            "docs/AUTONUMBER.md bloco 19 -- um id que chegou ao .reg por "
            "replicacao ou por ajuste e maior que 2^53 nao cabe num f64; sair "
            "como numero volta trocado no fio. Texto e a unica forma honesta."
        ),
        "arquivo": "crates/phxsql-server/src/valores.rs",
        "trecho": """        (Value::UInt(n), ColumnType::Sequence) if *n > phxsql_core::json::INTEIRO_EXATO_MAX => {""",
        "troca": """        (Value::UInt(n), ColumnType::Sequence) if false && *n > phxsql_core::json::INTEIRO_EXATO_MAX => {""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "valores::testes_inteiro_em_texto::sequencia_grande_sai_como_texto_no_json",
        ],
        "seguem": [
            "valores::testes_inteiro_em_texto::sequencia_grande_como_texto_atravessa_intacta",
        ],
    },
    {
        "id": "colisao-de-sequence-calada",
        "titulo": "dois masters na mesma faixa perdem uma linha sem contar a ninguém",
        "porque": (
            "docs/AUTONUMBER.md bloco 24 -- 4 insercoes viraram 2 linhas. O "
            "conserto pleno e faixa por no (inicio/passo, muda formato); o "
            "minimo seguro e nao deixar o estrago passar CALADO. "
            "colisao_de_criacao e o olho que o ve."
        ),
        "arquivo": "crates/phxsql-server/src/bidirecional.rs",
        "trecho": """    operacao == Operacao::Inclusao
        && matches!(local, Some(t) if !t.excluido && t.origem != origem_ev)""",
        "troca": """    let _ = (operacao, origem_ev, local);
    false""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "bidirecional::testes::inclusao_de_outra_origem_sobre_chave_viva_e_colisao",
        ],
        "seguem": [
            "bidirecional::testes::nao_ha_colisao_sem_esses_tres_sinais",
        ],
    },
    {
        "id": "contador-de-sequence-atras-do-dado",
        "titulo": "contador de Sequence atrás do dado repete número, e não havia reparo",
        "porque": (
            "docs/AUTONUMBER.md bloco 16 -- ajustar_sequencia para tras repete "
            "id calado, e o CRC do cabecalho nao pega ajuste legitimo. "
            "reconciliar_sequencia e o caminho de reparo que faltava, chamado "
            "pelo reparar."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """        if alvo > self.reg.sequencia_atual() {
            self.reg.ajustar_sequencia(alvo)?;
        }""",
        "troca": """        if false && alvo > self.reg.sequencia_atual() {
            self.reg.ajustar_sequencia(alvo)?;
        }""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "reconciliar-sequencia"],
        "caem": [
            "reconciliar_empurra_o_contador_para_depois_do_maior",
        ],
        "seguem": [
            "reconciliar_nunca_recua_o_contador",
        ],
    },
    # -----------------------------------------------------------------------
    # 26. O direito por coluna que cita coluna inexistente -- pedido 235
    # -----------------------------------------------------------------------
    {
        "id": "regra-de-coluna-com-typo-carrega-calada",
        "titulo": "regra de direito por coluna que cita coluna inexistente carrega calada e não protege nada",
        "porque": (
            "docs/SEGURANCA.md 15, pedido 235 -- `colunas: {salrio: ...}` "
            "carregava sem aviso e `salario` continuava sem regra: quem "
            "escreveu o cadastro achava que restringiu e nao restringiu nada. "
            "Configuracao que nao e lida mente, e mente pior quando o assunto "
            "e quem alcanca o dado. A conferencia mora numa passada depois da "
            "carga (`Cadastro::conferir_colunas`), chamada pelo arranque e "
            "pela porta das tres operacoes de cadastro; o defeito reposto e a "
            "recusa desligada, que e exatamente o estado de 08/09."
        ),
        "arquivo": "crates/phxsql-server/src/usuarios.rs",
        "trecho": """                    if !faltam.is_empty() {""",
        "troca": """                    if false && !faltam.is_empty() {""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "usuarios::tests::coluna_com_erro_de_digitacao_recusa_nomeando",
            "servidor::testes_direito_por_coluna::arranque_recusa_regra_que_cita_coluna_que_a_tabela_nao_tem",
            "servidor::testes_direito_por_coluna::usuario_criar_recusa_regra_que_cita_coluna_que_a_tabela_nao_tem",
        ],
        # O comportamento VELHO e o que tem de sobreviver ao defeito e ao
        # conserto: cadastro sem `colunas`, cadastro cuja coluna existe, e o
        # direito por tabela sem regra nenhuma.
        "seguem": [
            "usuarios::tests::sem_colunas_a_conferencia_nao_pergunta_nada",
            "usuarios::tests::coluna_que_existe_passa_sem_aviso",
            "usuarios::tests::tabela_que_ainda_nao_existe_aceita_com_aviso",
            "servidor::testes_direito_por_coluna::sem_colunas_no_cadastro_nada_muda",
            "servidor::testes_direito_por_coluna::arranque_aceita_regra_cuja_coluna_existe_e_ela_continua_valendo",
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_nada_muda",
        ],
    },
    # C20-CONSULTA (09/09/2026): o modelo tipado e os cinco caminhos ao dado
    # -----------------------------------------------------------------------
    {
        "id": "juncao-direita-vazia-perde-colunas",
        "titulo": "LEFT JOIN com a direita vazia sai sem as colunas da direita, e a forma da linha muda",
        "porque": (
            "pedido 237 -- a linha sem casamento tinha de trazer TODAS as "
            "colunas da direita nulas; com o modelo saindo da primeira linha "
            "(direita.first()), a direita vazia nao tinha de onde tirar os "
            "nomes e as colunas sumiam. Coluna que some quebra quem le por "
            "posicao, o defeito que esta casa ja pagou tres vezes."
        ),
        "arquivo": "crates/phxsql-server/src/consultar.rs",
        "trecho": "                    nova.extend(nulos(modelo_dir));",
        "troca": """                    // DEFEITO REPOSTO: os nomes das colunas da direita saiam
                    // da PRIMEIRA linha dela -- e com a direita vazia, de lugar
                    // nenhum, entao a coluna sumia em vez de vir nula.
                    let _ = &modelo_dir;
                    if let Some(primeira) = direita.first() {
                        nova.extend(primeira.iter().map(|(n, _)| (n.clone(), Json::Nulo)));
                    }""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_consultar_juncao::a_juncao_esquerda_com_a_direita_vazia_traz_as_colunas_nulas",
            "consultar::testes::a_direita_vazia_da_colunas_nulas_pelo_modelo",
        ],
        "seguem": [
            "servidor::testes_consultar_juncao::a_juncao_esquerda_mantem_a_linha_com_nulos",
        ],
    },
    {
        "id": "decimal-do-consultar-compara-como-texto",
        "titulo": "Decimal no consultar.expressao compara como texto, e 9,50 passa por um filtro de acima de 10",
        "porque": (
            "pedido 237 -- a celula tinha de ser convertida PELO TIPO do "
            "modelo antes de a expressao ve-la; convertida pelo FORMATO do "
            "JSON, um Decimal (texto \"9.50\") comparava como texto e "
            "\"9.50\" > \"10.00\" dava verdadeiro."
        ),
        "arquivo": "crates/phxsql-server/src/consultar.rs",
        "trecho": """    match crate::valores::json_para_valor(j, tipo) {
        Ok(v) => (v, *tipo),
        Err(_) => valor_de_json(j),
    }""",
        "troca": """    // DEFEITO REPOSTO: a celula convertida pelo FORMATO do JSON, e nao
    // pelo tipo do modelo -- o Decimal volta a comparar como texto.
    let _ = tipo;
    valor_de_json(j)""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "consultar::testes::o_decimal_compara_como_numero_pelo_modelo",
            "consultar::testes::a_celula_se_converte_pelo_tipo_do_modelo",
            "servidor::testes_consultar_juncao::o_decimal_da_expressao_compara_como_numero",
        ],
        "seguem": [
            "consultar::testes::texto_contra_numero_recusa_ensinando",
            "consultar::testes::o_numero_redondo_vira_inteiro",
        ],
    },
    {
        "id": "existe-fora-do-inventario-de-tabelas",
        "titulo": "existe[].de fora de tabelas_do_pedido: quem pergunta que tabelas o consultar alcanca nao ve a de dentro do EXISTS",
        "porque": (
            "pedido 236 e a lei da casa -- quando o portao passa a olhar um "
            "campo novo, procure quem NAO tem esse campo. O existe entrou como "
            "quinto caminho ao dado; a lista que o ignora vira o inventario que "
            "deixa de valer no dia em que alguem o usa como inventario."
        ),
        "arquivo": "crates/phxsql-server/src/direito_coluna.rs",
        "trecho": '            for lista in ["juntar", "escalar", "em", "existe"] {',
        "troca": '            for lista in ["juntar", "escalar", "em"] {',
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "direito_coluna::testes::os_cinco_caminhos_do_consultar_aparecem",
        ],
        "seguem": [
            "direito_coluna::testes::as_tabelas_escondidas_aparecem",
        ],
    },
    {
        "id": "wchar-recusa-no-driver-odbc",
        "titulo": "SQL_C_WCHAR volta a recusar no driver ODBC, que agora fala UTF-16 na borda",
        "porque": (
            "pedido 238 -- o driver e ANSI (so as funcoes sem `W`), mas isso "
            "nunca proibiu o BUFFER de um parametro de ser SQL_C_WCHAR: o "
            "gestor de drivers so decide qual FUNCAO chamar, nunca que tipo C "
            "um SQLBindParameter liga. `recusa_do_tipo_c` e o UNICO portao "
            "dessa aceitacao -- ele nao toca a conversao (texto.rs::"
            "ler_texto_utf16/escrever_utf16, provada a parte), so decide se o "
            "tipo C passa da ligacao."
        ),
        "arquivo": "crates/phxsql-odbc/src/parametro.rs",
        "trecho": """        SQL_C_CHAR | SQL_C_DEFAULT | SQL_C_WCHAR | SQL_C_SSHORT | SQL_C_SHORT | SQL_C_SLONG
        | SQL_C_LONG | SQL_C_SBIGINT | SQL_C_DOUBLE | SQL_C_FLOAT | SQL_C_BIT => None,""",
        "troca": """        SQL_C_CHAR | SQL_C_DEFAULT | SQL_C_SSHORT | SQL_C_SHORT | SQL_C_SLONG | SQL_C_LONG
        | SQL_C_SBIGINT | SQL_C_DOUBLE | SQL_C_FLOAT | SQL_C_BIT => None,""",
        "pacote": "phxsql-odbc",
        "alvo": ["--lib"],
        "caem": [
            "parametro::testes::o_tipo_c_que_o_driver_le_passa_e_o_resto_recusa_nomeando",
            "testes::ligacao_recusa_na_hora_o_que_o_driver_nao_sabe_mandar",
        ],
        # As duas continuam de pe porque nao pertencem ao PORTAO de ligacao:
        # sao a CONVERSAO (parametro::ler e entregar), que este trecho nunca
        # toca. Se caissem junto, o achado seria outro: um portao que esconde
        # duas funcoes atras de si.
        "seguem": [
            "parametro::testes::wchar_de_entrada_vira_utf8_pelo_indicador_em_bytes",
            "testes::entregar_wchar_trunca_por_caractere_inteiro_e_continua",
        ],
    },
    {
        "id": "replica-insiste-na-credencial-recusada",
        "titulo": "a réplica com credencial recusada insistia a cada `reconectar_em` e bloqueava o próprio IP — derrubando o operador junto",
        "porque": (
            "Pedido 203, filmado em 07/09/2026 e medido pelo soquete em "
            "09/09 (bancada/replicacao/credencial-recusada.py): 75 tentativas "
            "por minuto, o master bloqueou o 127.0.0.1 na quinta, em 4 s, por "
            "60 min, e o login do operador do mesmo IP caiu junto. O defeito "
            "era tratar «a origem me recusou» como «a origem caiu»: a "
            "credencial recusada e deterministica, e cada tentativa a mais "
            "so gasta a tolerancia do bloqueio de la. Repor o defeito e fazer "
            "o Ritmo DORMIR e voltar em vez de ESTACIONAR."
        ),
        "arquivo": "crates/phxsql-server/src/replica.rs",
        "trecho": """            Falha::CredencialRecusada => {
                self.seguidas = 0;
                Decisao::Estacionar
            }""",
        "troca": """            Falha::CredencialRecusada => {
                self.seguidas = 0;
                Decisao::Dormir(self.base)
            }""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "replica::testes_do_ritmo::credencial_recusada_estaciona_na_primeira",
        ],
        "seguem": [
            "replica::testes_do_ritmo::rede_recua_dobrando_ate_o_teto",
            "replica::testes_do_ritmo::outra_falha_mantem_o_intervalo_fixo",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (1/9): upsert-zera-a-coluna-negada
    # -----------------------------------------------------------------------
    {
        "id": "upsert-zera-a-coluna-negada",
        "titulo": "o upsert (`inserir` com `se_existir: \"atualizar\"`) zerava a coluna que o usuário não altera — para quem não lê, para quem lê e não altera, pelo SQL `ON CONFLICT DO UPDATE` e em transação",
        "porque": (
            "Achado A1 da revisao do motor (09/09/2026, p01_upsert_direito_coluna.py): `escrita_sob_direito_por_coluna` so repunha o gravado quando `op == \"atualizar\"`, e o upsert grava a linha inteira pelo MESMO `Table::atualizar` com `op == \"inserir\"`. O defeito que motivou o direito por coluna, de volta pela porta do `se_existir`. Repor o defeito e a regra deixar de enxergar o upsert como um atualizar."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        let upsert = op == "inserir"
            && crate::upsert::SeExistir::de_texto(pedido.texto_ou("se_existir", ""))
                .ok()
                .flatten()
                == Some(crate::upsert::SeExistir::Atualizar);
""",
        "troca": """        // DEFEITO REPOSTO: o upsert nao e um `atualizar` para esta regra --
        // o `op` diz `inserir`, e a linha que ja existe e gravada inteira.
        let upsert = false;
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "servidor::testes_direito_por_coluna::o_upsert_sem_a_coluna_preserva_o_valor_gravado",
        ],
        "seguem": [
            "servidor::testes_direito_por_coluna::o_atualizar_sem_a_coluna_preserva_o_valor_gravado",
            "servidor::testes_direito_por_coluna::sem_colunas_no_cadastro_nada_muda",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (2/9): presenca-da-coluna-negada-recusa-a-ficha
    # -----------------------------------------------------------------------
    {
        "id": "presenca-da-coluna-negada-recusa-a-ficha",
        "titulo": "a presença da coluna que o usuário não altera recusava a operação inteira — e a ficha, que manda a linha inteira com a coluna como `null`, não incluía nem salvava nada",
        "porque": (
            "Acrescimo do orquestrador a revisao do motor (09/09/2026), provado na tela: um usuario com QUALQUER regra de coluna nao conseguia incluir nem salvar pela ficha, mesmo mexendo so no permitido. «Protecao que quebra todo cliente nao e protecao, e estrago» (CLAUDE.md). Decisao do dono: a coluna que nao se altera e MANTIDA, nunca recusada, e a resposta diz em `colunas_mantidas`. Repor o defeito e voltar a recusar quando a coluna esta presente."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                    Some(v) => {
                        if direito.alterar {
                            continue;
                        }
                        match valor_gravado(coluna) {
""",
        "troca": """                    Some(v) => {
                        if direito.alterar {
                            continue;
                        }
                        // DEFEITO REPOSTO: valor ou nulo explicito na coluna que
                        // nao se altera recusa a operacao inteira.
                        if !direito.alterar {
                            return Err(PhxError::Autorizacao(format!(
                                "a coluna {coluna:?} de {base}.{tabela} nao pode ser alterada por \\
                                 este usuario; tire-a do pedido e o valor gravado fica como esta"
                            )));
                        }
                        match valor_gravado(coluna) {
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "servidor::testes_direito_por_coluna::a_ficha_de_quem_tem_regra_de_coluna_inclui_e_salva",
            "servidor::testes_direito_por_coluna::a_escrita_com_valor_na_coluna_negada_mantem_o_gravado_e_diz_isso",
            "servidor::testes_direito_por_coluna::quem_le_a_coluna_e_nao_a_altera_continua_gravando",
        ],
        "seguem": [
            "servidor::testes_direito_por_coluna::sem_colunas_no_cadastro_nada_muda",
            "servidor::testes_direito_por_coluna::o_atualizar_sem_a_coluna_preserva_o_valor_gravado",
            "servidor::testes_direito_por_coluna::a_leitura_esconde_a_coluna_negada",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (3/9): set-do-on-conflict-ignorado
    # -----------------------------------------------------------------------
    {
        "id": "set-do-on-conflict-ignorado",
        "titulo": "o `SET` do `INSERT … ON CONFLICT DO UPDATE` / `ON DUPLICATE KEY UPDATE` (o campo `atualizar`) era ignorado calado, e o `VALUES` ia por cima da linha com NULL no que ele não trazia",
        "porque": (
            "Achado A2 da revisao do motor (09/09/2026, p02_sql_on_conflict_set.py): o tradutor punha o SET em `atualizar`, `op_inserir` nunca o lia. Campo de protocolo sem leitor e primo do `recursos.cache_paginas` -- «configuracao que nao e lida mente». Repor o defeito e o `op_inserir` voltar a chamar o upsert sem o SET."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                crate::upsert::aplicar(&mut t, &indice, &linha, modo, atualizar, gancho)?
""",
        "troca": """                // DEFEITO REPOSTO: o `atualizar` do pedido nao chega ao motor.
                crate::upsert::aplicar(&mut t, &indice, &linha, modo, None, gancho)?
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "servidor::testes_upsert::o_atualizar_grava_a_lida_com_o_set_por_cima_e_nao_o_values",
        ],
        "seguem": [
            "servidor::testes_upsert::atualizar_grava_por_cima_sem_criar_linha",
            "servidor::testes_upsert::ignorar_devolve_o_rowid_de_quem_ja_estava_la",
            "servidor::testes_upsert::o_atualizar_fora_do_modo_atualizar_recusa",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (4/9): filtro-do-indice-parcial-e-oraculo
    # -----------------------------------------------------------------------
    {
        "id": "filtro-do-indice-parcial-e-oraculo",
        "titulo": "o índice parcial cujo `onde` cita a coluna negada respondia sobre ela: varrer por ele devolvia exatamente quem tem `salario > 5000`",
        "porque": (
            "Achado A3 da revisao do motor (09/09/2026, p03_indice_parcial_oraculo.py): `colunas_do_indice` lia so `indices[].colunas[].coluna`; o `onde` do indice saia no `esquema` e ninguem olhava. Repor o defeito e voltar a ignorar o filtro."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            if let Some(onde) = i.campo("onde").and_then(Json::texto) {
                if !onde.trim().is_empty() {
                    let e = phxsql_core::expressao::Expressao::analisar(onde)?;
                    filtro.extend(e.colunas().iter().map(|c| c.to_string()));
                }
            }
""",
        "troca": """            // DEFEITO REPOSTO: o filtro do indice parcial nao e olhado.
            let _ = i.campo("onde");
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "servidor::testes_direito_por_coluna::o_indice_parcial_cujo_filtro_cita_a_coluna_negada_recusa",
        ],
        "seguem": [
            "servidor::testes_direito_por_coluna::perguntar_pela_coluna_negada_recusa",
            "servidor::testes_direito_por_coluna::a_leitura_esconde_a_coluna_negada",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (5/9): juncao-materializa-antes-do-teto
    # -----------------------------------------------------------------------
    {
        "id": "juncao-materializa-antes-do-teto",
        "titulo": "as junções `interno`/`esquerdo`/`direito`/`completo` materializavam a saída inteira antes de conferir o teto — 1000 × 1000 com a mesma chave custava +561 MiB para recusar contra um teto de 1000",
        "porque": (
            "Achado A4 da revisao do motor (09/09/2026, p08_juntar_memoria.py: +561,6 MiB e 1128 ms antes; +0,3 MiB e 8 ms depois). «Recusar depois de materializar e o estrago, nao a protecao» ja valia para o `cruzado`; as outras quatro param na linha teto+1 dentro do laco. Repor o defeito e tirar a parada do laco das linhas casadas -- que e por onde passa o muitos-para-muitos."
        ),
        "arquivo": "crates/phxsql-server/src/consultar.rs",
        "trecho": """                for &i in is {
                    if saida.len() >= teto {
                        return None;
                    }
                    casou_direita[i] = true;
""",
        "troca": """                for &i in is {
                    // DEFEITO REPOSTO: o teto so e conferido depois, sobre a
                    // lista pronta -- e aqui ninguem confere nada.
                    casou_direita[i] = true;
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "consultar::testes::o_teto_para_a_juncao_antes_de_materializar_o_resto",
            "servidor::testes_consultar_juncao::as_juncoes_por_par_param_no_teto_sem_materializar_o_resto",
        ],
        "seguem": [
            "consultar::testes::os_cinco_tipos_de_juncao_produzem_a_forma_certa",
            "servidor::testes_consultar_juncao::a_juncao_cruzada_e_o_produto_e_o_teto_vem_antes",
            "servidor::testes_consultar_juncao::a_juncao_interna_descarta_quem_nao_casa",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (6/9): select-da-coluna-negada-devolve-nulo
    # -----------------------------------------------------------------------
    {
        "id": "select-da-coluna-negada-devolve-nulo",
        "titulo": "`SELECT salario FROM folha` por quem não lê `salario` devolvia `{\"salario\": null}` em toda linha, em vez de recusar",
        "porque": (
            "Achado A8 da revisao do motor (09/09/2026, p04_agrupar_consultar_direito.py): o `varrer` do plano Simples saia peneirado e `projetar` punha `null` na coluna que a linha nao tinha -- certo cada um sozinho, mentira sobre o dado os dois juntos. O `consultar` (visao, juncao) ja recusava; este era o irmao. Repor o defeito e nao conferir a projecao contra o `colunas_sem_leitura` do esquema."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        recusar_projecao_sobre_coluna_negada(&plano, &esquema, &base, &tabela)?;
""",
        "troca": """        // DEFEITO REPOSTO: a projecao do plano Simples nao e conferida.
        let _ = (&esquema, &tabela);
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "servidor::testes_direito_por_coluna::o_select_da_coluna_negada_recusa_em_vez_de_devolver_nulo",
        ],
        "seguem": [
            "servidor::testes_direito_por_coluna::o_sql_herda_o_direito_por_coluna",
            "servidor::testes_direito_por_coluna::a_leitura_esconde_a_coluna_negada",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (7/9): em-engole-o-campo-ausente
    # -----------------------------------------------------------------------
    {
        "id": "em-engole-o-campo-ausente",
        "titulo": "`consultar.em` com `campo` que o sub-pedido não devolve — inclusive a coluna negada — respondia zero linhas com `ok: true`",
        "porque": (
            "Achado A14 da revisao do motor (09/09/2026, p04 e p11_consultar_contratos.py): o `filter_map` engolia a ausencia e o conjunto vazio virava «nenhum casa». `escalar` e `existe` ja resolviam o campo contra o modelo do sub-pedido; o `em` era o irmao. Repor o defeito e voltar a usar o nome cru sem resolver."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            let campo_real = resolver_ou_recusar(
                &modelo_dentro,
                &campo_alvo,
                &format!("o \\"campo\\" do \\"em\\"[{i}]"),
            )?;
""",
        "troca": """            // DEFEITO REPOSTO: o campo nao e resolvido contra o modelo, e a
            // ausencia vira um conjunto vazio.
            let _ = &modelo_dentro;
            let campo_real = campo_alvo.clone();
""",
        "pacote": "phxsql-server",
        "alvo": ['--lib'],
        "caem": [
            "servidor::testes_consultar::o_em_com_campo_que_o_sub_pedido_nao_devolve_recusa_nomeando",
            "servidor::testes_direito_por_coluna::o_em_com_campo_negado_recusa_em_vez_de_zero_linhas",
        ],
        "seguem": [
            "servidor::testes_consultar::o_em_e_o_in_de_subconsulta",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (8/9): literal-negativo-nao-parseia
    # -----------------------------------------------------------------------
    {
        "id": "literal-negativo-nao-parseia",
        "titulo": "o literal negativo não parseava em `SET`/`VALUES` («esperava um valor e veio \"-\"») enquanto `WHERE a = -5` passava pela expressão",
        "porque": (
            "Achado A10 da revisao do motor (09/09/2026, p07b_esquema_reteste.py): o lexico entrega `-` e `5` separados e `literal()` nao os juntava. Repor o defeito e tirar a juncao."
        ),
        "arquivo": "crates/phxsql-sql/src/sintaxe.rs",
        "trecho": """        if matches!(s.token, Token::Menos) {
            if let Some(Token::Numero(n)) = self.s.get(self.i + 1).map(|x| &x.token) {
                let lit = Literal::Numero(format!("-{n}"));
                self.i += 2;
                return Ok(lit);
            }
        }
""",
        "troca": """        // DEFEITO REPOSTO: `Menos` seguido de `Numero` cai no `outro`.
""",
        "pacote": "phxsql-sql",
        "alvo": ['--lib'],
        "caem": [
            "dml::testes::o_literal_negativo_vale_no_set_e_no_values",
        ],
        "seguem": [
            "dml::testes::update_escolhe_o_indice_unico_e_monta_o_buscar",
            "dml::testes::on_conflict_do_update_vira_atualizar_com_o_set_no_campo_atualizar",
        ],
    },
    # -----------------------------------------------------------------------
    # Revisao do motor, 09/09/2026 (9/9): tabela-inexistente-vaza-o-caminho
    # -----------------------------------------------------------------------
    {
        "id": "tabela-inexistente-vaza-o-caminho",
        "titulo": "a tabela que não existe respondia «nenhum volume de x.reg em /tmp/…» — o caminho absoluto do disco do servidor, a todo cliente que erra uma letra",
        "porque": (
            "Achado A13 da revisao do motor (09/09/2026, p12_existe_sql_e_mensagens.py). A mesma correcao que a chave conferida ja pagou em `table.rs`: nomear a tabela em vez de vazar o caminho, e so quando ela nao existe mesmo. Repor o defeito e abrir sem traduzir."
        ),
        "arquivo": "crates/phxsql-store/src/catalogo.rs",
        "trecho": """        Table::abrir(self.diretorio(schema)?, nome)
            .map_err(|e| self.tabela_que_nao_existe(e, schema, nome))
""",
        "troca": """        // DEFEITO REPOSTO: o erro cru do store, com o caminho, sai como esta.
        Table::abrir(self.diretorio(schema)?, nome)
""",
        "pacote": "phxsql-store",
        "alvo": ['--lib'],
        "caem": [
            "catalogo::tests::a_tabela_que_nao_existe_e_nomeada_sem_o_caminho_do_disco",
        ],
        "seguem": [
            "catalogo::tests::hierarquia_database_schema_tabela",
            "catalogo::tests::mesmo_nome_em_schemas_diferentes_nao_colide",
        ],
    },
    # -----------------------------------------------------------------------
    # Pedido 248 (16/09/2026): semaforo e teto das threads -- quatro guardas
    # -----------------------------------------------------------------------
    {
        "id": "permissao-sem-devolver-a-vaga",
        "titulo": "a permissão do semáforo morre sem devolver a vaga — o `fetch_sub` esquecido, com outro nome",
        "porque": (
            "O semaforo do `phxsql-core` existe para que a vaga volte no `Drop`, "
            "inclusive no desenrolar de um panico. Se o `Drop` nao devolve, o "
            "semaforo e o contador de mao de antes: cada thread que morre leva a "
            "vaga junto, e depois de N a porta fecha com o servidor de pe. Os "
            "testes que usam `adquirir` sem prazo foram escritos com "
            "`adquirir_ate(5 s)` de proposito, para que este defeito FALHE em "
            "vez de pendurar o binario inteiro sob o prazo do executor."
        ),
        "arquivo": "crates/phxsql-core/src/semaforo.rs",
        "trecho": """        *estado = estado.saturating_sub(1);
        // Uma vaga, um acordado. `notify_all` faria K esperadores disputarem
        // uma vaga so, e K-1 voltariam a dormir -- trabalho a toa que cresce
        // com a fila.
        self.interno.vaga.notify_one();
""",
        "troca": """        // DEFEITO REPOSTO (pedido 248): a permissao morre e a vaga NAO
        // volta -- e o `fetch_sub` esquecido, com outro nome.
        let _ = &mut estado;
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "semaforo::testes::nunca_ha_mais_permissoes_que_o_teto",
            "semaforo::testes::permissao_volta_mesmo_em_panico",
            "semaforo::testes::permissao_viaja_para_outra_thread_e_volta_quando_ela_morre",
            "semaforo::testes::adquirir_ate_acorda_quando_a_vaga_volta",
            "semaforo::testes::a_disputa_nunca_passa_do_teto",
            "semaforo::testes::teto_zero_vira_um_e_sem_teto_nao_limita",
            "semaforo::testes::mutex_envenenado_nao_derruba",
        ],
        "seguem": [
            "semaforo::testes::adquirir_ate_devolve_none_quando_o_prazo_acaba",
            "semaforo::testes::prazo_zero_nao_espera",
        ],
    },
    {
        "id": "permissao-de-dados-sem-raii",
        "titulo": "a vaga da porta de dados só volta no caminho feliz — um pânico no `atender` a leva junto",
        "porque": (
            "E o defeito de origem do pedido 248: `fetch_add` ao aceitar, "
            "`fetch_sub` DEPOIS do `atender`, e um panico no meio pulava o "
            "`fetch_sub`. Com `conexoes_max` vagas, N panicos fechavam a porta "
            "com o servidor de pe, recusando todo mundo e sem nada ter caido. "
            "O `ManuallyDrop` reproduz exatamente isso sobre a `Permissao`: a "
            "devolucao passa a ser uma chamada depois do corpo, e o panico a "
            "pula. A prova usa o laco de aceitacao DE PRODUCAO e um panico de "
            "verdade dentro da thread da conexao (`panicos_de_teste`)."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                        move |fio| {
                            // A vaga mora na thread da conexao e morre com
                            // ela -- pelo fim do `atender` ou por um panico
                            // dentro dele. Nao ha `fetch_sub` para pular.
                            let _vaga = permissao;
                            fio.fazendo(&format!("conexao de {endereco}"));
                            servidor.atender(fluxo, endereco);
                        },
""",
        "troca": """                        move |fio| {
                            // DEFEITO REPOSTO (pedido 248): a vaga so volta
                            // no caminho feliz -- o `fetch_sub` de antes com
                            // outro nome. Um panico no `atender` pula a
                            // devolucao, e a porta fecha depois de N panicos.
                            let vaga = std::mem::ManuallyDrop::new(permissao);
                            fio.fazendo(&format!("conexao de {endereco}"));
                            servidor.atender(fluxo, endereco);
                            drop(std::mem::ManuallyDrop::into_inner(vaga));
                        },
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_das_threads::panico_dentro_do_atender_devolve_a_vaga_da_porta_de_dados",
        ],
        "seguem": [
            "servidor::testes_das_threads::a_porta_de_dados_continua_recusando_na_hora_acima_do_teto",
        ],
    },
    {
        "id": "web-sem-teto",
        "titulo": "a porta web volta a nascer sem teto — uma thread por pedido, como até a 0.18",
        "porque": (
            "Ate 16/09/2026 o laco da interface confessava num comentario: «esta "
            "thread nasce SEM TETO». Medido pela `enxurrada-web.py` com 500 "
            "conexoes seguradas: 504 threads e 14,6 MiB antes, 68 threads e 7,6 "
            "MiB depois, com 436 recusas 503 e `Retry-After`. Repor o defeito e "
            "dar a cada pedido um semaforo novo e ilimitado, que e o mesmo que "
            "nenhum. O teste do comportamento VELHO (abaixo do teto nada muda) "
            "tem de continuar passando -- guarda nova entra pedida, nao imposta."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                    let Some(vaga) = servidor.vaga_http() else {
                        servidor.recusar_http_cheio(&mut fluxo, par, familia);
                        continue;
                    };
""",
        "troca": """                    // DEFEITO REPOSTO (pedido 248): a porta HTTP sem teto --
                    // uma thread por pedido, como ate a 0.18. Um semaforo novo
                    // e ilimitado por pedido e o mesmo que nenhum.
                    let vaga = Semaforo::sem_teto().adquirir();
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_das_threads::acima_do_teto_a_web_responde_503_com_retry_after_e_a_vaga_volta",
        ],
        "seguem": [
            "servidor::testes_das_threads::abaixo_do_teto_a_web_nao_muda",
            "servidor::testes_das_threads::zero_no_teto_da_web_e_sem_teto",
        ],
    },
    {
        "id": "ficha-do-fio-pulada-no-panico",
        "titulo": "a ficha da thread na telemetria fica «viva» para sempre quando o corpo entra em pânico",
        "porque": (
            "O irmao do contador de conexoes: `telemetria::subir` chamava "
            "`fio_morreu` DEPOIS do corpo, na mesma ordem em que o laco da porta "
            "de dados chamava o `fetch_sub` -- e um panico pulava os dois. Irmao "
            "e quem chama as mesmas funcoes na mesma ordem, e o conserto entra "
            "nos dois: a ficha passou a morrer no `Drop` de `FichaViva`. Repor o "
            "defeito e voltar a chamada depois do corpo."
        ),
        "arquivo": "crates/phxsql-server/src/telemetria.rs",
        "trecho": """            let _ficha = FichaViva {
                telemetria: eu,
                fio: Arc::clone(&para_thread),
            };
            corpo(para_thread);
""",
        "troca": """            // DEFEITO REPOSTO (pedido 248): a ficha morre numa chamada
            // DEPOIS do corpo -- e o panico pula a chamada.
            corpo(Arc::clone(&para_thread));
            eu.fio_morreu(&para_thread);
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "telemetria::testes::a_thread_que_entra_em_panico_tambem_deixa_de_ser_viva",
        ],
        "seguem": [
            "telemetria::testes::a_thread_que_termina_deixa_de_ser_viva",
        ],
    },
    # ------------------------------------------------ a saude do disco (249)
    {
        "id": "disco-erro-de-es-sem-aviso",
        "titulo": "o erro de E/S respondido ao cliente não avisa ninguém",
        "porque": (
            "o pedido do dono e literal: «em caso de log de erro, aviso "
            "imediato por e-mail e SMS». Ate 16/09/2026 um `PhxError::Io` "
            "(5001) so virava resposta e linha no `acessos.log`. O gancho mora "
            "no `anotar`, o unico sumidouro de resposta, e o portao e uma "
            "comparacao de inteiro ANTES de qualquer trabalho. Repor o "
            "defeito e comparar com um codigo que nunca chega."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if acesso.codigo == CODIGO_DE_ES {
""",
        "troca": """        // DEFEITO REPOSTO (pedido 249): o gancho compara com um codigo
        // que nenhum erro carrega -- o erro de E/S volta a ser so uma linha
        // no acessos.log.
        if acesso.codigo == CODIGO_DE_ES + 1 {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_da_saude_do_disco::erro_de_es_numa_gravacao_avisa_na_hora_e_uma_vez_so",
            "servidor::testes_da_saude_do_disco::o_sms_sai_pelo_gateway_da_operadora_numa_linha_sem_caminho",
            "servidor::testes_da_saude_do_disco::sem_email_ligado_o_erro_conta_e_nao_avisa",
        ],
        "seguem": [
            "servidor::testes_da_saude_do_disco::o_codigo_do_gancho_e_o_do_erro_de_es",
            "saude_do_disco::testes::a_sonda_passa_num_diretorio_gravavel_e_apaga_o_canario",
        ],
    },
    {
        "id": "disco-sonda-cega-ao-erro",
        "titulo": "a sonda canário diz «passou» num diretório que o sistema operacional recusa",
        "porque": (
            "a sonda existe para ver o disco recusar ANTES da proxima gravacao "
            "de verdade (EROFS, ENOSPC, EIO). Se o `open` que falha for "
            "engolido, ela pinta o painel de verde num disco morto. A prova e "
            "contra o sistema operacional: arquivo no lugar do diretorio "
            "(ENOTDIR) e caminho inexistente (ENOENT) -- e o `chmod 0555` so "
            "vale sem root, e o teste diz isso."
        ),
        "arquivo": "crates/phxsql-server/src/saude_do_disco.rs",
        "trecho": """    let mut arquivo = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(caminho)
        .map_err(|e| falha("abrir", e))?;
""",
        "troca": """    // DEFEITO REPOSTO (pedido 249): o abrir que falha e engolido -- a
    // sonda diz «passou» num diretorio que nao aceita escrita.
    let Ok(mut arquivo) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(caminho)
    else {
        return Ok(());
    };
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "saude_do_disco::testes::a_sonda_falha_contra_o_sistema_operacional",
        ],
        "seguem": [
            "saude_do_disco::testes::a_sonda_passa_num_diretorio_gravavel_e_apaga_o_canario",
            "saude_do_disco::testes::erofs_e_enospc_sao_reconhecidos_pelo_kind_e_pelo_numero",
        ],
    },
    {
        "id": "disco-silencio-furado",
        "titulo": "todo erro de E/S manda um aviso: cem mil linhas, cem mil e-mails",
        "porque": (
            "o aviso e imediato no PRIMEIRO evento de cada tipo, e depois cala "
            "por `alertas.disco.repetir_minutos`. Sem o silencio por chave, uma "
            "carga de cem mil linhas num disco doente vira cem mil e-mails, que "
            "e como um alerta de verdade passa despercebido. E o mesmo desenho "
            "do vigia de espaco e dos jobs (`jobs::pode_avisar`)."
        ),
        "arquivo": "crates/phxsql-server/src/saude_do_disco.rs",
        "trecho": """        if crate::jobs::pode_avisar(
            &mut silencio,
            evento.tipo.nome(),
            evento.quando_ms,
            self.silencio_ms(),
        ) {
            Some(evento)
        } else {
            None
        }
""",
        "troca": """        // DEFEITO REPOSTO (pedido 249): todo evento avisa -- sem silencio
        // por tipo, cada linha recusada e um e-mail.
        let _ = &mut silencio;
        Some(evento)
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "saude_do_disco::testes::o_primeiro_erro_avisa_e_o_segundo_na_janela_cala",
            "servidor::testes_da_saude_do_disco::erro_de_es_numa_gravacao_avisa_na_hora_e_uma_vez_so",
        ],
        "seguem": [
            "saude_do_disco::testes::a_sonda_passa_num_diretorio_gravavel_e_apaga_o_canario",
            "servidor::testes_da_saude_do_disco::sem_email_ligado_o_erro_conta_e_nao_avisa",
        ],
    },
    {
        "id": "disco-config-nao-lida",
        "titulo": "`alertas.disco.checar_segundos` está no arquivo e ninguém o lê",
        "porque": (
            "configuracao que nao e lida mente -- e o `cache_paginas` que passou "
            "tres versoes prometendo um cache que nao existia. Cada campo novo "
            "de `alertas.disco` e `alertas.sms` tem leitor e teste; repor o "
            "defeito e devolver o padrao no lugar do valor do arquivo."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """            checar_segundos: d
                .inteiro_ou("checar_segundos", padrao.checar_segundos as i64)
                .max(1) as u64,
""",
        "troca": """            // DEFEITO REPOSTO (pedido 249): o campo esta no arquivo e
            // ninguem o le -- a sonda roda no padrao, diga o que disser o
            // config.json.
            checar_segundos: padrao.checar_segundos,
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "config::testes_recursos::alertas_disco_e_sms_sao_lidos_do_arquivo",
            # Cai junto, e DEVE: a ultima asercao dele e «zero vira 1», que e
            # o mesmo leitor. Medido em 16/09/2026: com ele em `seguem` a
            # guarda saiu ESTRAGOU, e o achado foi que o teste do padrao
            # tambem prova a leitura.
            "config::testes_recursos::sem_o_bloco_a_sonda_nasce_ligada_e_o_sms_desligado",
        ],
        "seguem": [
            "config::testes_recursos::o_sms_recusa_no_arranque_o_que_nao_entregaria",
        ],
    },
    # -----------------------------------------------------------------------
    # Chutar a tomada (bancada/tomada/): quatro defeitos que um SIGKILL
    # descobriria, repostos onde um teste de unidade AINDA os pega. O que so
    # o processo morto de verdade pega esta na propria bancada, e nao aqui.
    # -----------------------------------------------------------------------
    {
        "id": "recuperacao-deixa-a-marca-orfa",
        "titulo": "a recuperação completa (ou descarta) a marca `.tx` e a deixa no disco",
        "porque": (
            "a marca e o bilhete de UM commit; completada ou descartada, ela "
            "tem de sair. Deixada, vira orfa para sempre: todo arranque a acha "
            "de novo, reaplica de novo (idempotente pelo rowid, entao a "
            "contagem NAO acusa) e imprime Recovery para um commit que ja "
            "acabou. E o teste que conta linhas passa com o defeito -- quem "
            "pega e o que olha o DISCO, `!caminho.exists()`."
        ),
        "arquivo": "crates/phxsql-server/src/transacao.rs",
        # Trecho movido em 24/09/2026 pelo pedido 426: o corpo do laco virou
        # `tratar_marca`, que o arranque e a recuperacao do COMMIT dividem.
        "trecho": """            if tratar_marca(&db, &caminho, &mut r, NoArranque::Sim) {
                let _ = std::fs::remove_file(&caminho);
            }
""",
        "troca": """            // DEFEITO REPOSTO: a marca fica no disco depois de completada ou
            // descartada -- orfa para sempre, reaplicada a cada arranque.
            let _ = tratar_marca(&db, &caminho, &mut r, NoArranque::Sim);
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::marca_que_nao_confere_e_commit_que_nunca_comecou",
        ],
        "seguem": [
            # A reaplicacao e idempotente pelo rowid: com a marca deixada, o
            # segundo arranque reaplica e NAO duplica -- este teste continua
            # verde com o defeito, e e por isso que ele sozinho nao o pega.
            "servidor::testes_transacoes::a_recuperacao_completa_o_commit_e_nao_duplica",
        ],
    },
    {
        "id": "recuperacao-nao-completa-o-commit",
        "titulo": "a recuperação conta e apaga a marca válida sem completar o commit",
        "porque": (
            "e a tomada chutada no meio do COMMIT virando perda em silencio: a "
            "marca `.tx` diz o que faltava gravar, e a recuperacao anda para a "
            "frente porque o `.reg` nunca reaproveita slot -- nao ha como "
            "desfazer, so completar. Contar `completadas` e apagar a marca sem "
            "reaplicar deixa o relatorio dizendo «1 completada» sobre zero "
            "linhas."
        ),
        "arquivo": "crates/phxsql-server/src/transacao.rs",
        # Trecho movido em 24/09/2026 pelo pedido 426: `tratar_marca`, o
        # corpo que o arranque e a recuperacao do COMMIT dividem.
        "trecho": """        Ok(Leitura::Aberta(marca)) => {
            let antes = r.impossiveis.len();
            completar(db, &marca, r);
            r.completadas += 1;
""",
        "troca": """        // DEFEITO REPOSTO: a marca valida e contada e apagada, mas o
        // commit que ela descreve nunca e completado.
        Ok(Leitura::Aberta(_marca)) => {
            let antes = r.impossiveis.len();
            r.completadas += 1;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_recuperacao_completa_o_commit_e_nao_duplica",
        ],
        "seguem": [
            # A marca ILEGIVEL continua descartada e apagada: o defeito e so
            # no ramo da marca valida.
            "servidor::testes_transacoes::marca_que_nao_confere_e_commit_que_nunca_comecou",
        ],
    },
    {
        "id": "ndx-queda-com-cabecalho-limpo",
        "titulo": "a marca de sujo do `.ndx` fica só em RAM e a queda deixa o índice atrasado em silêncio",
        "porque": (
            "e o unico contrato que torna o write-back do `.ndx` aceitavel: a "
            "queda continua possivel, mas e DETECTADA, porque a marca (byte 52) "
            "vai ao disco ANTES da primeira pagina suja existir. Sem ela, um "
            "SIGKILL no meio de um BULKINSERT deixa o `.reg` com as linhas e o "
            "`.ndx` sem as chaves, com o cabecalho dizendo «limpo»: `buscar` "
            "responde «nao existe» para linha que existe, e `inserir` aceita "
            "chave repetida. Indice atrasado se reconstroi; atrasado em "
            "silencio, nao -- ninguem sabe."
        ),
        "arquivo": "crates/phxsql-store/src/ndx.rs",
        "trecho": """        if !self.sujo {
            self.sujo = true;
            self.gravar_cabecalho()?;
        }
        self.guardar_no_cache(n, p, true)
""",
        # A primeira versao desta troca deixava `self.sujo = true` em RAM e so
        # tirava o `gravar_cabecalho()` -- e o executor respondeu NAO PEGOU: o
        # cabecalho e regravado a cada `inserir` (o `qtd_chaves` muda), e ele
        # carrega o `sujo` da memoria. A marca chegava ao disco por OUTRA
        # escrita, e o defeito reposto nao era o defeito. Repor de verdade e
        # ninguem levantar a marca.
        "troca": """        // DEFEITO REPOSTO: ninguem levanta a marca de sujo. O cabecalho no
        // disco diz «limpo» com pagina suja no cache -- a tomada chutada no
        // meio de uma carga deixa o indice atrasado EM SILENCIO.
        self.guardar_no_cache(n, p, true)
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "ndx"],
        "caem": [
            "a_queda_sem_sincronizar_e_detectada_e_nao_silenciosa",
            "a_marca_sai_depois_das_paginas_e_nao_antes",
        ],
        "seguem": [
            # Quem sincroniza ou fecha limpo continua inteiro: o `sincronizar`
            # e o `fechar` gravam o cabecalho por conta propria.
            "sincronizar_fecha_o_arquivo_de_verdade",
            "despejo_de_pagina_suja_chega_ao_arquivo",
            "lote_sobrevive_a_reabrir_o_arquivo",
            "o_cache_nao_serve_pagina_velha",
        ],
        "prazo": 300,
    },
    {
        "id": "reserva-sobrevive-a-queda-da-ligacao",
        "titulo": "a saída da conexão não solta a reserva do BULKINSERT",
        "porque": (
            "a primeira das duas redes contra reserva orfa (`carga.rs`): o "
            "cliente cai no meio da carga e a tabela ficaria reservada ate o "
            "prazo, com todo mundo recebendo EM_CARGA de uma conexao que nao "
            "existe mais. Foi a prova por SOQUETE que achou, na primeira "
            "versao, que a queda nao soltava -- e a causa era o teste "
            "(`makefile()` segurando o descritor), nao o servidor. A guarda "
            "trava o lado do servidor."
        ),
        "arquivo": "crates/phxsql-server/src/carga.rs",
        "trecho": """            .filter(|(_, r)| r.ligacao == ligacao)
""",
        "troca": """            // DEFEITO REPOSTO: a saida da conexao nao solta nada -- a
            // reserva de quem caiu fica presa ate o prazo.
            .filter(|(_, r)| r.ligacao == ligacao && r.ligacao == u64::MAX)
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_bulkinsert::a_queda_da_conexao_solta",
        ],
        "seguem": [
            # A segunda rede (o prazo) e o caminho normal continuam de pe: o
            # defeito e so na soltura pela ligacao.
            "servidor::testes_bulkinsert::o_prazo_solta",
            "servidor::testes_bulkinsert::o_dono_continua_gravando",
        ],
    },
    # -----------------------------------------------------------------------
    # Pedido 254: a marca do COMMIT feito dentro da reserva sobrevivia ao
    # «ok» do bulkinsert(false). Duas guardas porque o defeito tinha dois
    # pontos que chamam as mesmas funcoes na mesma ordem (sincronizar, tirar
    # das sujas, drenar): o `bulkinsert(false)`, que nao drenava, e o fecho da
    # janela, que voltava antes de drenar quando nao havia tabela suja.
    # -----------------------------------------------------------------------
    {
        "id": "bulkinsert-false-nao-drena-a-marca",
        "titulo": "o `bulkinsert(false)` sincroniza a tabela e deixa a marca `.tx` do COMMIT no disco",
        "porque": (
            "achado 1 da bancada «chutar a tomada» (pedido 253), hipotese "
            "escrita antes e confirmada sem queda em 16/09/2026: com a tabela "
            "reservada a janela nao fecha no COMMIT, a marca fica pendente "
            "esperando o fsync do bulkinsert(false) -- que sincronizava e "
            "tirava a tabela das sujas por conta propria, sem a drenagem do "
            "fecho. A marca de um commit ja duravel ficava no disco 300 ms "
            "depois do «ok», e a tomada chutada depois dele fazia o arranque "
            "reportar «achadas 1 / ja aplicadas 800» (3/3 corridas "
            "APOS_BULKINSERT_FALSE_MARCA_REPORTADA). Nao perde nem duplica "
            "linha: e marca que sobrevive a mais do que devia. O conserto e "
            "chamar a MESMA drenagem, na mesma ordem, sob a mesma trava."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            t.sincronizar()?;
            if let Ok(mut sujas) = self.sujas.lock() {
                sujas.remove(&format!("{database}/{tabela}"));
            }
            self.descarregar_sujas_com(&trava);
        }
""",
        "troca": """            t.sincronizar()?;
            // DEFEITO REPOSTO (pedido 254): a tabela sai das sujas sem passar
            // pela drenagem do fecho -- a marca do COMMIT feito na reserva
            // fica no disco depois do «ok».
            if let Ok(mut sujas) = self.sujas.lock() {
                sujas.remove(&format!("{database}/{tabela}"));
            }
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_marca_do_commit_na_reserva_sai_no_bulkinsert_false",
        ],
        "seguem": [
            # O irmao (o fecho da janela numa tabela so) e outro ponto: este
            # defeito nao o alcanca, e e isso que diz que sao DOIS consertos.
            "servidor::testes_transacoes::a_janela_que_fecha_numa_tabela_so_leva_a_marca_junto",
            # A soltura continua sincronizando e soltando: o defeito e so a
            # marca que fica.
            "servidor::testes_bulkinsert::o_dono_continua_gravando",
            "servidor::testes_bulkinsert::a_queda_da_conexao_solta",
        ],
    },
    {
        "id": "fecho-sem-suja-nao-drena-a-marca",
        "titulo": "o fecho da janela volta antes de drenar as marcas quando não há tabela suja",
        "porque": (
            "o irmao do pedido 254, achado procurando quem mais tira tabela "
            "das sujas sem drenar: `gravar_de_verdade` sincroniza a propria "
            "tabela, a tira das sujas e so entao chama o fecho -- que, com a "
            "lista vazia, voltava sem apagar as marcas pendentes. Em "
            "`por_lote`, um COMMIT seguido das gravacoes que fecham a janela "
            "na MESMA tabela deixava a marca de um commit ja duravel "
            "pendurada ate a proxima janela com duas tabelas sujas, ou para "
            "sempre; e o relogio de fundo tambem volta sem tabela suja. Este "
            "defeito reposto derruba OS DOIS testes, porque o "
            "`bulkinsert(false)` consertado passa por este mesmo fecho."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        // fundo nao a alcancava porque ele tambem volta sem tabela suja.
        let mut faltaram = Vec::new();
""",
        "troca": """        // fundo nao a alcancava porque ele tambem volta sem tabela suja.
        // DEFEITO REPOSTO (pedido 254, o irmao): sem tabela suja o fecho
        // volta antes de drenar as marcas pendentes.
        if lista.is_empty() {
            return;
        }
        let mut faltaram = Vec::new();
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_janela_que_fecha_numa_tabela_so_leva_a_marca_junto",
            "servidor::testes_transacoes::a_marca_do_commit_na_reserva_sai_no_bulkinsert_false",
        ],
        "seguem": [
            # Com DUAS tabelas sujas a lista nao esta vazia e a drenagem
            # sempre aconteceu: o defeito e so o caminho da lista vazia.
            "servidor::testes_janela_e_cadeia::com_todas_sincronizadas_as_marcas_saem",
            "servidor::testes_janela_e_cadeia::tabela_que_nao_sincroniza_segura_as_marcas",
            "servidor::testes_janela_e_cadeia::uma_tabela_so_grava_como_sempre",
        ],
    },
    # 37. O fsync do arquivo limpo -- pedido 258, a forma que o pedido apontou
    # -----------------------------------------------------------------------
    {
        "id": "fsync-do-arquivo-limpo",
        "titulo": "`Volumes::sincronizar` leva ao disco todo descritor aberto, sem pular o limpo",
        "porque": (
            "medido pelo nucleo em 16/09/2026 (`--example fsync-por-operacao`): "
            "num inserir em por_operacao, 4 dos 8 fsync iam a arquivo limpo "
            "(.trash .bin .memo .reason); num atualizar, 5 de 8; num excluir, "
            "3 de 9 -- e era isso, nao a arvore, que punha o padrao atras do "
            "SQLite em toda escrita com fsync (bancada CRUD, pedido 257). O "
            "conserto pula o descritor BATIZADO (levado ao disco por este "
            "processo alguma vez) e sem marca de escrita; repor o defeito e "
            "tirar o filtro dos batizados."
        ),
        "arquivo": "crates/phxsql-store/src/volume.rs",
        "trecho": """        let mut alvos: BTreeSet<u32> = self
            .abertos
            .keys()
            .copied()
            .filter(|v| !batizados.contains(v))
            .collect();
""",
        "troca": """        // DEFEITO REPOSTO (pedido 258): todo descritor aberto vai ao disco,
        // pergunte-se ou nao quem mudou -- o batizado limpo paga de novo.
        let mut alvos: BTreeSet<u32> = self.abertos.keys().copied().collect();
        let _ = batizados;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "fsync-por-operacao"],
        "caem": [
            "depois_do_primeiro_fecho_so_o_que_a_operacao_escreveu_vai_ao_disco",
            "a_tabela_reaberta_no_mesmo_processo_herda_o_batismo",
        ],
        "seguem": [
            # O primeiro fecho de uma familia nova leva tudo nos dois mundos:
            # o defeito e' so' o que vem DEPOIS dele.
            "o_primeiro_fecho_de_uma_familia_nova_leva_tudo_o_que_esta_aberto",
        ],
    },
    # 38. A versao INGENUA do mesmo pedido -- o que o parecer do DBA barrou
    # -----------------------------------------------------------------------
    {
        "id": "fsync-so-dos-escritos",
        "titulo": "o fecho confia só no registro em RAM — e o registro nasceu vazio com o processo",
        "porque": (
            "a sequencia do parecer do DBA no pedido 258: COMMIT com a marca "
            ".tx gravada, slot no .reg ainda no cache do nucleo, SIGKILL, "
            "arranque, transacao::recuperar le o slot (que esta la', vindo do "
            "cache), chama sincronizar e apaga a marca. Se sincronizar so' "
            "olhasse os escritos -- que nasceram vazios com o processo --, "
            "zero fsync no .reg e a marca sairia mesmo assim: commit "
            "confirmado perdido numa queda de energia, sem bilhete. O batismo "
            "existe para isto: o primeiro fsync de cada descritor neste "
            "processo e' incondicional."
        ),
        "arquivo": "crates/phxsql-store/src/volume.rs",
        "trecho": """        let mut alvos: BTreeSet<u32> = self
            .abertos
            .keys()
            .copied()
            .filter(|v| !batizados.contains(v))
            .collect();
""",
        "troca": """        // DEFEITO REPOSTO (a versao ingenua do pedido 258): so' quem tem
        // marca vai ao disco -- e a marca morreu com o processo anterior.
        let mut alvos: BTreeSet<u32> = BTreeSet::new();
        let _ = batizados;
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "volume::tests::o_primeiro_fecho_do_processo_nao_confia_no_registro",
            "volume::tests::volume_do_meio_que_entra_no_cache_depois_do_batismo_paga_o_primeiro_fsync",
        ],
        "seguem": [
            # A versao ingenua acerta o caso comum -- e' por isso que ela e'
            # tentadora: estes tres continuam verdes com ela.
            "volume::tests::depois_do_batismo_so_quem_foi_escrito_vai_ao_disco",
            "volume::tests::o_fecho_alcanca_o_que_outra_instancia_escreveu",
            "volume::tests::o_fecho_alcanca_volume_do_meio_de_tabela_paginada",
        ],
    },
    # -----------------------------------------------------------------------
    # O estado do gerador de UUID v7 ao alcance de um teste (pedido 247)
    # -----------------------------------------------------------------------
    {
        "id": "relogio-ao-alcance-do-teste",
        "titulo": "o estado do gerador de v7 fica ao alcance de um teste, que o escreve para trás",
        "porque": (
            "pedido 247: o `static RELOGIO` ficava solto no modulo `uuid`, e o "
            "teste do contador estourado o escrevia para tras (`ms = 5_000`) "
            "para montar o cenario. Os testes de um binario rodam em paralelo; "
            "sob carga essa escrita caiu no meio do laco do vizinho, o gerador "
            "viu «milissegundo novo» no MESMO milissegundo, re-semeou o "
            "contador na metade de baixo e o id andou para tras (contador "
            "0x8ae seguido de 0x409): 54 falhas em 1.000 corridas do modulo, "
            "medido em 16/09/2026. O gerador estava certo; o estado dele e que "
            "estava ao alcance de quem nao devia. O conserto fechou o estado "
            "num modulo privado (`relogio`) e deu aos testes da logica uma "
            "funcao pura (`avancar`) com estado local. Repor o defeito e "
            "reabrir o modulo E voltar a escrever nele: sao dois pontos, e um "
            "so nao compila -- nao existe meia reposicao aqui."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-core/src/uuid.rs",
                "trecho": """    static ESTADO: Mutex<(u64, u16)> = Mutex::new((0, 0));
""",
                "troca": """    // DEFEITO REPOSTO (1/2): o estado volta a ser alcancavel de fora.
    pub(super) static ESTADO: Mutex<(u64, u16)> = Mutex::new((0, 0));
""",
            },
            {
                "arquivo": "crates/phxsql-core/src/uuid.rs",
                "trecho": """        avancar((5_000, CONTADOR_MASCARA), 5_000)
""",
                "troca": """        // DEFEITO REPOSTO (2/2): o cenario e montado ESCREVENDO o estado do
        // gerador para tras, como o teste antigo fazia.
        *relogio::ESTADO.lock().unwrap() = (5_000, CONTADOR_MASCARA);
        relogio::passo(5_000)
""",
            },
        ],
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        # So o teste de concorrencia, e o motivo e MEDIDO: com o defeito
        # reposto ele monta o cenario 64 vezes enquanto quatro fios geram, e
        # caiu 20 vezes em 20 corridas. Os dois vizinhos que o defeito original
        # derrubava (`v7_nunca_repete_nem_anda_para_tras` e
        # `contador_estourado_empresta_do_futuro`) NAO entram em `caem` nem em
        # `seguem`: com a escrita unica do teste antigo eles caem quando o
        # escalonador quer -- 10 e 3 vezes em 20 corridas com o defeito
        # reposto, 54 em 1.000 no dia -- e uma lista que exige o que o
        # escalonador nao garante e uma guarda que mente na metade das rodadas.
        "caem": [
            "uuid::tests::a_geracao_entre_fios_nunca_anda_para_tras",
        ],
        "seguem": [
            # Nao tocam o estado do gerador, ou o tocam uma vez so e sem
            # comparar com um anterior: o defeito reposto nao os alcanca.
            "uuid::tests::v7_tem_o_layout_do_rfc_9562",
            "uuid::tests::comparar_bytes_e_comparar_tempo",
            "uuid::tests::relogio_da_maquina_para_tras_nao_leva_o_id_junto",
            "uuid::tests::texto_vai_e_volta",
            "uuid::tests::id256_cabe_um_sha256",
        ],
    },
    # -----------------------------------------------------------------------
    # Os gatilhos do upsert honram o ramo que ele virou (pedido 245, o gap da
    # G4-MOTOR) -- frente U, 16/09/2026. Entrada apensa ao FIM, depois das da
    # frente do fsync, para o integrador separar as duas.
    # -----------------------------------------------------------------------
    {
        "id": "upsert-gatilho-do-ramo",
        "titulo": "no upsert que atualiza, o BEFORE UPDATE vê a linha mesclada e o AFTER é o do ramo que ele virou",
        "porque": (
            "pedido 245, o gap nomeado pela G4-MOTOR: «no upsert com `atualizar`, "
            "o gatilho BEFORE ve a linha do VALUES, nao a mesclada». Medido em "
            "16/09/2026 com cinco testes: o BEFORE UPDATE NAO rodava no ramo que "
            "atualiza -- o unico BEFORE era o de INSERT, sobre a linha crua -- "
            "nos dois caminhos (`op_inserir` e `empilhar`), e o `op_inserir` "
            "disparava o AFTER INSERT nos TRES ramos: no que atualizou (com a "
            "linha como ficou) e no que ignorou (com a linha que ja estava la "
            "como NEW: «entrou Ana» duas vezes na auditoria por um pedido que "
            "nao gravou byte nenhum). Lei dos tres motores, aceite automatico: "
            "PostgreSQL, MariaDB e MySQL disparam o BEFORE INSERT sobre a linha "
            "proposta e, no ramo que atualiza, BEFORE UPDATE com NEW = a gravada "
            "com o SET por cima e OLD = a gravada, depois AFTER UPDATE; a linha "
            "ignorada nao dispara AFTER nenhum. O conserto entrou em tres "
            "pontos, e repor o defeito e desfazer os tres: o gancho do "
            "`upsert::aplicar` ignorado, o AFTER INSERT de volta nos tres ramos "
            "do `op_inserir`, e o BEFORE UPDATE fora do `empilhar`."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-server/src/upsert.rs",
                "trecho": """                if let (Some(gancho), Some(v)) = (antes_de_atualizar, &velha) {
                    let mut nova = gravada.take().unwrap_or_else(|| linha.to_vec());
                    gancho(&mut nova, v, t.esquema())?;
                    gravada = Some(nova);
                }
""",
                "troca": """                // DEFEITO REPOSTO (1/3, pedido 245): o gancho e ignorado -- o
                // BEFORE UPDATE do ramo que o upsert virou nao roda, e a
                // linha vai ao disco sem ele.
                let _ = antes_de_atualizar;
""",
            },
            {
                "arquivo": "crates/phxsql-server/src/servidor.rs",
                "trecho": """            if feito.ignorada {
                (&[], None)
            } else if feito.atualizada {
                (
                    &depois_upd,
                    feito
                        .velha
                        .as_deref()
                        .map(|l| linha_para_json(l, t.esquema())),
                )
            } else {
                (&depois, None)
            };
""",
                "troca": """            // DEFEITO REPOSTO (2/3, pedido 245): o AFTER INSERT roda nos tres
            // ramos -- no que atualizou e no que nao gravou byte nenhum.
            (&depois, None);
        let _ = (&depois_upd, &feito.velha);
""",
            },
            {
                "arquivo": "crates/phxsql-server/src/servidor.rs",
                "trecho": """                            if !antes_upd.is_empty() {
                                self.rodar_gatilhos_antes(
                                    &antes_upd,
                                    Some(&mut linha),
                                    Some(&velha),
                                    t.esquema(),
                                )?;
                            }
""",
                "troca": """                            // DEFEITO REPOSTO (3/3, pedido 245): dentro da
                            // transacao o unico BEFORE da instrucao e o de
                            // INSERT, sobre a linha crua.
                            let _ = &antes_upd;
""",
            },
        ],
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_gatilhos::no_upsert_que_atualiza_o_before_update_ve_a_linha_mesclada",
            "servidor::testes_gatilhos::o_before_insert_roda_primeiro_e_o_before_update_ve_o_que_ele_deixou",
            "servidor::testes_gatilhos::no_upsert_o_after_e_o_do_ramo_que_ele_virou",
            "servidor::testes_gatilhos::o_upsert_ignorado_nao_dispara_after_nenhum",
            "servidor::testes_gatilhos::dentro_da_transacao_o_before_update_do_upsert_ve_a_mesclada",
        ],
        "seguem": [
            # O BEFORE INSERT continua vendo a linha proposta, o AFTER INSERT
            # da insercao de verdade continua auditando, e o `atualizar` de
            # sempre continua vendo OLD: o defeito reposto nao alcanca quem
            # nao passa pelo upsert.
            "servidor::testes_gatilhos::before_insert_normaliza_o_campo",
            "servidor::testes_gatilhos::after_insert_audita_noutra_tabela",
            "servidor::testes_gatilhos::update_e_delete_veem_old",
            # E o upsert SEM gatilho grava exatamente o que gravava: a
            # mesclada com o SET por cima, fora e dentro da transacao.
            "servidor::testes_upsert::o_atualizar_grava_a_lida_com_o_set_por_cima_e_nao_o_values",
            "servidor::testes_upsert::dentro_da_transacao_o_upsert_empilha_a_op_que_ele_virou",
        ],
    },
    # -----------------------------------------------------------------------
    # O teste das threads do SO provava por DIFERENCA entre duas leituras do
    # total do processo (pedido 261) -- frente F261, 16/09/2026. Entrada apensa
    # ao FIM, depois da frente do upsert, para o integrador separar as duas.
    # -----------------------------------------------------------------------
    {
        "id": "threads-do-so-pela-diferenca",
        "titulo": "a prova de que o SO viu a thread subida é a diferença entre duas leituras do total do processo",
        "porque": (
            "pedido 261, irmao do 247: o teste lia o `Threads:` do "
            "`/proc/self/status` ANTES e DEPOIS de subir a thread e exigia "
            "`agora > so`. O total e do processo INTEIRO e o `libtest` roda os "
            "testes em paralelo, entao a thread de outro teste que morre entre "
            "as duas leituras come o `+1` da nossa -- e o teste acusa o motor "
            "por um movimento que e do executor. Medido em 16/09/2026: num "
            "amostrador a 2,37 milhoes de leituras durante a suite do crate, o "
            "total encolheu 651 vezes em 48 s, e 0,72% das janelas de 0,5 ms "
            "(2,32% das de 2 ms) tinham queda de pelo menos 1; o teste antigo "
            "caiu 8 vezes em 600 corridas so do modulo, e TODAS com "
            "`so=6 agora=6 presa_no_so=1` -- a thread subida ja estava na lista "
            "de tarefas do SO, entao o que faltou veio do vizinho. O conserto "
            "mede a grandeza que nao depende dos vizinhos: o NOME da thread em "
            "`/proc/self/task/*/comm`; do total so se cobra piso, que e a unica "
            "comparacao que o vizinho nao estraga. O leque de quatro vizinhas "
            "que morrem antes da medida esta na montagem do proprio teste, e e "
            "o que torna esta reposicao determinista -- com uma so, a diferenca "
            "sobreviveria a uma thread que nascesse ao lado no mesmo instante, "
            "e a guarda passaria por engano (medido: uma corrida da suite "
            "inteira deu `27 -> 25`, ou seja, um vizinho NASCEU no meio)."
        ),
        "arquivo": "crates/phxsql-server/src/telemetria.rs",
        "trecho": """        assert_eq!(
            tarefas_chamadas("presa-do-teste"),
            1,
            "a thread subida nao apareceu no SO pelo nome (o SO ve {agora} tarefas)"
        );
""",
        "troca": """        // DEFEITO REPOSTO (pedido 261): a prova volta a ser a DIFERENCA entre
        // duas leituras do total do processo, e a vizinha que morreu no meio
        // come o `+1` da thread subida.
        assert!(
            agora > so,
            "a thread subida nao apareceu no SO: {so} -> {agora}"
        );
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        # Medido com o defeito reposto: 40 em 40 corridas do modulo e 6 em 6 da
        # suite inteira sob carga (load 13,4), com quedas de 2 a 4 threads.
        "caem": [
            "telemetria::testes::as_threads_do_so_se_medem_e_nunca_sao_menos_que_as_registradas",
        ],
        "seguem": [
            # Os vizinhos que sobem e derrubam thread continuam de pe: o defeito
            # reposto e da MEDIDA do teste, nao do registro de fios.
            "telemetria::testes::a_thread_que_termina_deixa_de_ser_viva",
            "telemetria::testes::a_thread_que_entra_em_panico_tambem_deixa_de_ser_viva",
            "telemetria::testes::o_registro_de_threads_guarda_nome_e_finalidade",
            "telemetria::testes::le_a_cpu_do_proprio_processo",
        ],
        # A suite do crate leva 42 s sozinha e 91 s sob carga; o prazo cobre a
        # compilacao a frio da arvore copiada em cima disso.
        "prazo": 420,
    },
    {
        "id": "varredura-sem-o-elo",
        "titulo": "a varredura barata do diretorio perde a tabela alcancada por elo",
        "porque": (
            "petrea da integridade: nunca se mata o pai que tem filhos. A busca "
            "reversa so pergunta «tem filha?» as tabelas que a varredura ENXERGA, "
            "entao tabela que some da lista e tabela que ninguem confere -- e a orfa "
            "que nasce dai nao da erro, nasce calada."
            "A varredura do diretorio que a busca reversa usa passou a decidir "
            "«isto e arquivo?» pelo `d_type` que o `getdents64` ja trouxe, em vez "
            "de um `statx` por entrada -- 9 chamadas por exclusao viraram 1, e a "
            "varredura caiu de 9,05 para 4,43 us (16/09/2026, pedido 259). A "
            "troca tem UMA divergencia possivel, e ela e da petrea: `file_type()` "
            "NAO segue o elo simbolico e `is_file()` segue, entao um `.reg` "
            "alcancado por elo sumiria da lista -- e tabela que some da lista e "
            "tabela que ninguem pergunta se tem filha. A orfa nao daria erro: ela "
            "nasceria calada, que e a pior das duas formas."
        ),
        "arquivo": "crates/phxsql-store/src/catalogo.rs",
        "trecho": """        .filter(|e| match e.file_type() {
            Ok(t) if t.is_symlink() => e.path().is_file(),
            Ok(t) => t.is_file(),
            Err(_) => e.path().is_file(),
        })
""",
        "troca": """        // DEFEITO REPOSTO (pedido 259): o ramo do elo simbolico sai, e a
        // varredura barata volta a perder a tabela alcancada por elo.
        .filter(|e| matches!(e.file_type(), Ok(t) if t.is_file()))
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "catalogo::testes_gestao::a_varredura_barata_ve_o_mesmo_que_a_cara",
        ],
        "seguem": [
            # O outro lado do elo -- o diretorio chamado `pasta.reg`, que o
            # `d_type` tem de recusar como o `is_file()` recusava -- e uma
            # asserção DENTRO do mesmo teste, e nao um teste proprio: nao da
            # para lista-lo aqui. Os vizinhos que varrem o mesmo diretorio sem
            # elo nenhum continuam verdes com o defeito reposto, e e isso que
            # diz que a guarda mira o elo e nao a varredura inteira.
            "catalogo::testes_gestao::excluir_tabela_leva_os_arquivos_dela_e_so_os_dela",
            "catalogo::testes_gestao::pertence_nao_confunde_tabela_de_prefixo_igual",
        ],
    },
    {
        "id": "linha-vazia-na-conferencia-de-filhas",
        "titulo": "a linha descida para a conferencia de filhas vai vazia, e toda mae parece sem filha",
        "porque": (
            "petrea da integridade: nunca se mata o pai que tem filhos. Quem desce a "
            "linha adiante decide o que a conferencia enxerga, e a resposta errada "
            "aqui LIBERA a exclusao da mae -- errado na direcao errada."
            "O `excluir_de_vez` passou a ler o slot UMA vez e a descer os valores "
            "ja decodificados para a conferencia de filhas, em vez de cada um ler "
            "por conta propria (3 leituras -> 1, 4,43 -> 1,54 us). Quem desce a "
            "linha adiante decide o que a conferencia enxerga: descer a linha "
            "ERRADA, ou vazia, faz a conferencia responder «nao tem filha» -- a "
            "resposta errada na direcao errada, porque ela LIBERA a exclusao da "
            "mae. Por isso o codigo usa `expect` e nao `unwrap_or(&[])`, e por "
            "isso a sobreposicao da transacao manda: quem tem troca empilhada "
            "recebe `None` e a conferencia le por la, como sempre leu."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": """            let ja_lida = match self.troca_de(rowid) {
                None => Some(valores.as_slice()),
                Some(_) => None,
            };
""",
        "troca": """            // DEFEITO REPOSTO (pedido 259): a linha desce VAZIA, e a
            // conferencia responde «nao tem filha» para toda mae.
            let ja_lida = Some(&valores[..0]);
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "chave-estrangeira"],
        "caem": [
            "a_mae_com_filha_nao_pode_ser_apagada",
            "a_mae_sem_filha_sai_normalmente",
            "sem_indice_na_filha_a_recusa_diz_qual_indice_falta",
        ],
        "seguem": [
            # `filha_de_outra_linha_nao_tranca_esta` e o que pegaria a linha
            # ERRADA descida adiante (nao a vazia), e `sem_conferir_...` nao
            # confere nada -- os dois continuam verdes com este defeito reposto,
            # e e isso que diz que a guarda mira a linha vazia e nao a troca.
            "filha_de_outra_linha_nao_tranca_esta",
            "sem_conferir_a_mae_com_filha_sai_como_sempre",
        ],
    },
    # ------------------------------------------------ pedido 245, O2-O6
    {
        "id": "teto-de-64-bits-satura",
        "titulo": "número cru fora da faixa do `Int8` é GRAVADO saturado, e `1e21`, `1e30` e `1e300` viram todos o mesmo número",
        "porque": (
            "Conserto entra no caminho que o motivou, e o caminho IRMAO fica. A "
            "`Sequence` recusa o numero cru grande desde o bloco 19, e o "
            "comentario ao lado dela dizia que o irmao `Int8`/`UInt8` «passava "
            "por aqui ao lado sem problema». Passava gravando OUTRO numero: o "
            "`as i64` do Rust satura em vez de falhar. Medido em 16/09/2026 pelo "
            "protocolo: `{\"v\":1e21}` numa coluna `Int8` gravava "
            "9223372036854775807. Os irmaos estreitos (`Int1`/`Int2`/`Int4`) ja "
            "recusavam a faixa no `escrever_inline`; o `Int8` era o unico sem a "
            "recusa, porque o carregador E o `i64`."
        ),
        "arquivo": "crates/phxsql-server/src/valores.rs",
        "trecho": "            Value::Int(inteiro_com_sinal(j)?)\n",
        "troca": (
            "            // DEFEITO REPOSTO (pedido 245, O4): o fechamento saturante\n"
            "            // de antes -- `1e21` vira `i64::MAX` e vai para o disco.\n"
            "            Value::Int(\n"
            "                match j {\n"
            "                    Json::Texto(t) => t.trim().parse::<i64>().ok(),\n"
            "                    o => o.inteiro(),\n"
            "                }\n"
            "                .ok_or_else(|| erro(\"inteiro\"))?,\n"
            "            )\n"
        ),
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "valores::testes_teto_de_64_bits::numero_cru_fora_da_faixa_recusa_em_vez_de_saturar",
            "valores::testes_teto_de_64_bits::texto_numerico_grande_demais_fala_de_faixa_e_nao_de_tipo",
        ],
        "seguem": [
            "valores::testes_teto_de_64_bits::abaixo_do_teto_nada_muda",
            "valores::testes_teto_de_64_bits::a_sequencia_continua_com_a_mensagem_dela",
            "valores::testes_teto_de_64_bits::a_faixa_imprecisa_continua_passando_por_decisao_registrada",
        ],
    },
    {
        "id": "saida-do-direito-por-coluna",
        "titulo": "a recusa do direito por coluna manda «peça as colunas por varrer» também para o `agrupar` e para o `backup`",
        "porque": (
            "Funcionalidade que mostra texto cru redige ANALISANDO. Uma frase so "
            "servia as 28 operacoes recusadas, e ela e conselho apenas para as "
            "que devolvem LINHA: quem pediu `{\"funcao\":\"maximo\"}` nao quer a "
            "linha, e o `backup` nem devolve coluna -- leva arquivo. Conselho que "
            "nao serve gasta a confianca da mensagem inteira: quem o segue uma vez "
            "e nao chega a lugar nenhum para de ler as outras. Pedido 245, O5."
        ),
        "arquivo": "crates/phxsql-server/src/direito_coluna.rs",
        "trecho": (
            "    SAIDAS\n"
            "        .iter()\n"
            "        .find(|(n, _)| *n == op)\n"
            "        .map(|(_, s)| *s)\n"
            "        .unwrap_or(Saida::TabelaInteira)\n"
        ),
        "troca": (
            "    // DEFEITO REPOSTO (pedido 245, O5): uma frase so para todas.\n"
            "    let _ = op;\n"
            "    Saida::PelaLinha\n"
        ),
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "direito_coluna::testes::o_agrupar_nao_manda_mais_pedir_as_colunas",
        ],
        "seguem": [
            "direito_coluna::testes::a_lista_das_saidas_e_a_das_recusadas",
            "direito_coluna::testes::a_lista_e_o_catalogo_sao_a_mesma_lista",
        ],
    },
    {
        "id": "check-que-se-contradiz-no-alter",
        "titulo": "`acrescentar_coluna` aceita um `padrao` que viola o `check` declarado no MESMO comando, e todo `atualizar` da linha velha passa a recusar",
        "porque": (
            "A recusa acontece na DECLARACAO, nao na gravacao -- a mesma lei do "
            "`ao_excluir`: uma tabela nasce uma vez e grava um milhao de vezes. "
            "Medido em 16/09/2026: `v Int8 check \"v > 0\"` com `padrao = -5` numa "
            "tabela com linha era aceito, a linha velha ficava com -5, e so "
            "aparecia no `atualizar` seguinte. Pedido 245, O2."
        ),
        "arquivo": "crates/phxsql-store/src/table.rs",
        "trecho": "                if check.avaliar_bool(&resolver)? == Some(false) {\n",
        "troca": (
            "                // DEFEITO REPOSTO (pedido 245, O2): a contradicao passa.\n"
            "                if false {\n"
        ),
        "pacote": "phxsql-store",
        "alvo": ["--test", "acrescentar-coluna"],
        "caem": ["padrao_que_viola_o_proprio_check_recusa_antes_de_tocar_no_reg"],
        "seguem": [
            "o_crivo_do_check_nao_pega_quem_depende_da_linha_velha",
            "obrigatoria_sem_padrao_com_linha_e_recusada",
            "sem_padrao_a_linha_antiga_recebe_nulo",
        ],
    },
    {
        "id": "alter-com-regra-sem-aviso",
        "titulo": "`acrescentar_coluna` com `check` ou `calculada` numa tabela com linha é aceito SEM AVISO, e a linha velha fica fora da regra",
        "porque": (
            "«Aceito sem aviso» foi o defeito nomeado pelo pedido 245, O2. O CHECK "
            "novo nao e conferido contra as linhas que ja existem, e a `calculada` "
            "acrescentada as deixa NULAS -- duas verdades na mesma coluna, sem erro "
            "nenhum no caminho. O aviso nao resolve o O2 (o que fazer com a linha "
            "velha e decisao de garantia de dado, subida para o dono): ele tira a "
            "parte que era «sem aviso». O portao vem ANTES do trabalho, e coluna "
            "sem regra nao ganha campo novo na resposta."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": "        if registros > 0 && (coluna.check.is_some() || coluna.calculada.is_some()) {\n",
        "troca": (
            "        // DEFEITO REPOSTO (pedido 245, O2): nenhum aviso sai.\n"
            "        if false {\n"
        ),
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_regras_de_esquema::acrescentar_coluna_com_regra_avisa_o_que_a_linha_velha_nao_ganhou",
        ],
        "seguem": [
            "servidor::testes_regras_de_esquema::calculada_b_sai_6",
            "servidor::testes_regras_de_esquema::check_recusa_menos_5_e_aceita_5",
        ],
    },
    {
        "id": "upsert-parcial-vira-mescla",
        "titulo": "o upsert sem o campo `atualizar` passa a MESCLAR, e a sincronia do DbLink perde a única forma de gravar NULO num destino",
        "porque": (
            "Contrato, nao defeito -- e o `troca` daqui e o «conserto» que alguem "
            "vai querer fazer ao ler o O3 do pedido 245. O `crate::upsert` e o "
            "mesmo caminho da sincronia do DbLink (`aplicar_para_ca`): mesclar aqui "
            "tira dela a unica forma de gravar NULO, e sincronia que nao apaga "
            "campo deixa o destino diferente da origem, calada. Medido em "
            "16/09/2026 com este mesmo `troca`: os 15 testes de `dblink::sincronia` "
            "ficam VERDES -- o irmao que quebraria nao tem teste que o pegue, e "
            "esta guarda e hoje a unica coisa entre o conserto bem-intencionado e a "
            "regressao silenciosa. A forma segura ja existe: o campo `atualizar`."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-server/src/upsert.rs",
                "trecho": "                let velha = if atualizar.is_some() || antes_de_atualizar.is_some() {\n",
                "troca": (
                    "                // DEFEITO REPOSTO (pedido 245, O3): le sempre, para mesclar.\n"
                    "                let velha = if true {\n"
                ),
            },
            {
                "arquivo": "crates/phxsql-server/src/upsert.rs",
                "trecho": (
                    "                let mut gravada = match (atualizar, &velha) {\n"
                    "                    (Some(set), Some(v)) => Some(mesclar(v, set, t.esquema())?),\n"
                    "                    _ => None,\n"
                    "                };\n"
                ),
                "troca": (
                    "                let mut gravada = match (atualizar, &velha) {\n"
                    "                    (Some(set), Some(v)) => Some(mesclar(v, set, t.esquema())?),\n"
                    "                    // DEFEITO REPOSTO: o ausente do pedido mantem o gravado.\n"
                    "                    (None, Some(v)) => {\n"
                    "                        let mut n = linha.to_vec();\n"
                    "                        for (i, x) in n.iter_mut().enumerate() {\n"
                    "                            if x.e_null() {\n"
                    "                                if let Some(o) = v.get(i) {\n"
                    "                                    *x = o.clone();\n"
                    "                                }\n"
                    "                            }\n"
                    "                        }\n"
                    "                        Some(n)\n"
                    "                    }\n"
                    "                    _ => None,\n"
                    "                };\n"
                ),
            },
        ],
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_upsert::o_upsert_sem_o_set_grava_a_linha_inteira_e_isso_e_contrato",
        ],
        "seguem": [
            "servidor::testes_upsert::o_atualizar_grava_a_lida_com_o_set_por_cima_e_nao_o_values",
            "servidor::testes_upsert::sem_se_existir_a_chave_repetida_continua_recusando",
            "servidor::testes_upsert::ignorar_devolve_o_rowid_de_quem_ja_estava_la",
        ],
    },
    {
        "id": "direcao-do-indice-sem-saida",
        "titulo": "a recusa por direção do índice explica bem por que não dá, e não diz o que fazer",
        "porque": (
            "Pedido 245, O6. A recusa nomeia o indice e diz que a direcao esta "
            "gravada no `.ndx` -- e para por ai, num beco. A saida tem uma "
            "armadilha propria que a medicao achou: NAO ha operacao de "
            "acrescentar indice a tabela que ja existe (nem `CREATE INDEX` na "
            "camada SQL nem op no protocolo), entao um conselho generico "
            "mandaria fazer o que nao da para fazer. Por isso a frase diz «na "
            "criacao da tabela». Com o defeito reposto, o teste VELHO "
            "(`direcao_do_indice_e_a_do_ndx`) continua verde -- e e isso que "
            "mostra que ele sozinho nao bastava."
        ),
        "arquivo": "crates/phxsql-sql/src/traduzir.rs",
        "trecho": (
            "                     depois. Quem precisa das duas direcoes declara dois indices na \\\n"
            "                     criacao da tabela, um deles com a marca `desc` ({} desc)\",\n"
            "                    o.coluna,\n"
            "                    if o.desc { \"DESC\" } else { \"ASC\" },\n"
            "                    i.nome,\n"
            "                    if i.colunas[0].desc { \"DESC\" } else { \"ASC\" },\n"
            "                    o.coluna\n"
        ),
        "troca": (
            "                     depois\",\n"
            "                    o.coluna,\n"
            "                    if o.desc { \"DESC\" } else { \"ASC\" },\n"
            "                    i.nome,\n"
            "                    if i.colunas[0].desc { \"DESC\" } else { \"ASC\" }\n"
        ),
        "pacote": "phxsql-sql",
        "alvo": ["--lib"],
        "caem": ["traduzir::testes::a_recusa_da_direcao_diz_o_que_fazer_e_onde"],
        "seguem": [
            "traduzir::testes::direcao_do_indice_e_a_do_ndx",
            "traduzir::testes::recusa_o_que_nao_tem_substrato",
        ],
    },
    # -----------------------------------------------------------------------
    # 39. «CRIPTOGRAFIA SE CONFERE CONTRA VETOR OFICIAL» -- a petrea que nao
    #     tinha guarda nenhuma ate 16/09/2026
    #
    # Medido antes de escrever: ZERO entradas deste catalogo repunham defeito
    # em SHA-256, HMAC ou PBKDF2. Havia TESTE -- e teste nao e guarda. Um
    # teste diz que o codigo passa hoje; uma guarda diz que ele FALHA quando o
    # defeito volta, e so a segunda afirmacao protege alguma coisa.
    #
    # As cinco abaixo nao provam "o SHA-256 esta certo" -- isso e trabalho do
    # teste. Elas provam que CADA FAMILIA DE VETOR da lista e PORTANTE: que
    # apagar aquele vetor deixaria passar um defeito real. Por isso cada uma
    # repoe um defeito DIFERENTE, escolhido por um criterio so -- «um
    # refatorador distraido cometeria este de verdade?» --, e cada uma leva no
    # `seguem` os testes que CONTINUAM VERDES com o defeito de pe. Esse
    # `seguem` e a razao de a petrea dizer «vetor oficial» e nao «teste»:
    # auto-consistencia, ida-e-volta e propriedade sobrevivem a um motor de
    # criptografia quebrado, porque as tres perguntam ao PROPRIO MOTOR.
    #
    # Onde a producao acaba, neste arquivo: `hash.rs` tem 415 linhas e o
    # `#[cfg(test)]` comeca na 260. Os cinco trechos estao TODOS antes dela --
    # conferido por contagem, nao a olho: defeito reposto dentro do
    # `mod tests` nao e defeito reposto, o teste continua verde e a guarda
    # «nao pega».
    # -----------------------------------------------------------------------
    {
        "id": "sha256-sem-somar-o-estado",
        "titulo": "SHA-256 sem a realimentação do estado: a compressão vira permutação reversível",
        "porque": (
            "petrea do CLAUDE.md: «criptografia se confere contra vetor "
            "oficial». Defeito reposto: a soma final do bloco comprimido com "
            "o estado de ENTRADA -- a construcao de Davies-Meyer -- vira "
            "atribuicao. Escolhi este, e nao uma constante da tabela K "
            "trocada, porque o de K ninguem comete: as 64 constantes sao um "
            "bloco copiado da norma e ninguem as «arruma». Este se comete: o "
            "ponto e um `zip` com `wrapping_add` dentro de um `for`, e "
            "reduzi-lo a `*destino = valor` parece limpeza -- o fecho passa a "
            "ser «o estado E o resultado das rodadas», que e uma frase que "
            "soa certa. E o estrago e o maximo possivel: sem a realimentacao "
            "a compressao vira uma PERMUTACAO invertivel, e a "
            "unidirecionalidade -- a unica coisa que faz guardar senha como "
            "hash valer algo -- acaba. O `seguem` e o argumento da petrea "
            "inteiro: o teste de auto-consistencia (a mesma mensagem partida "
            "de 1 em 1, de 7 em 7, de 64 em 64) fica VERDE, porque pergunta "
            "ao motor quebrado e recebe a mesma resposta quebrada duas vezes."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """        for (destino, valor) in self
            .estado
            .iter_mut()
            .zip([a, b, c, d, e, f, g, h].into_iter())
        {
            *destino = destino.wrapping_add(valor);
        }
""",
        "troca": """        // DEFEITO REPOSTO: a realimentacao de Davies-Meyer vira ATRIBUICAO.
        // O estado de entrada some e a compressao passa a ser invertivel; a
        // saida continua com 32 bytes de cara aleatoria, e por isso a unica
        // coisa que acusa e o vetor publicado.
        for (destino, valor) in self
            .estado
            .iter_mut()
            .zip([a, b, c, d, e, f, g, h].into_iter())
        {
            *destino = valor;
        }
""",
        "pacote": "phxhash",
        "alvo": ["--lib"],
        "caem": [
            "hash::tests::sha256_vetores_oficiais",
            "hash::tests::sha256_um_milhao_de_letras_a",
            "hash::tests::hmac_vetores_rfc4231",
            "hash::tests::pbkdf2_vetores_conhecidos",
        ],
        "seguem": [
            # Os tres que NAO pegam, e e por isso que estao aqui: os tres
            # perguntam ao proprio motor em vez de perguntar a norma.
            "hash::tests::sha256_alimentado_em_pedacos_da_o_mesmo",
            "hash::tests::pbkdf2_sal_diferente_muda_tudo",
            "hash::tests::comparacao_em_tempo_constante",
        ],
    },
    {
        "id": "sha256-com-o-tamanho-em-little-endian",
        "titulo": "SHA-256 com o tamanho da mensagem, no padding, em little-endian",
        "porque": (
            "petrea do CLAUDE.md: «criptografia se confere contra vetor "
            "oficial». Defeito reposto: `bits.to_be_bytes()` vira "
            "`to_le_bytes()` no fecho do padding. E o erro classico de hash "
            "escrito a mao, e continua classico porque a maquina daqui e "
            "little-endian: `to_le_bytes()`/`to_ne_bytes()` e o que sai dos "
            "dedos de quem «uniformiza a serializacao do modulo» sem saber "
            "que a FIPS 180-4 fixa big-endian ali. "
            "O motivo de esta entrada existir ao lado da de cima e um numero: "
            "a mensagem VAZIA tem comprimento zero, e zero e igual nas duas "
            "ordens -- entao o vetor que todo mundo decora "
            "(`e3b0c442...7852b855`) e exatamente o que NAO pega este "
            "defeito. Quem reduzisse `sha256_vetores_oficiais` a um caso so "
            "escolheria o vazio, e a guarda morreria calada. Os quatro casos "
            "da FIPS existem por isso."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """        self.atualizar_sem_contar(&bits.to_be_bytes());
""",
        "troca": """        // DEFEITO REPOSTO: o tamanho da mensagem entra em little-endian. A
        // FIPS 180-4 fixa big-endian, e a mensagem VAZIA nao acusa -- zero e
        // zero nas duas ordens.
        self.atualizar_sem_contar(&bits.to_le_bytes());
""",
        "pacote": "phxhash",
        "alvo": ["--lib"],
        "caem": [
            "hash::tests::sha256_vetores_oficiais",
            "hash::tests::sha256_um_milhao_de_letras_a",
            "hash::tests::hmac_vetores_rfc4231",
            "hash::tests::pbkdf2_vetores_conhecidos",
        ],
        "seguem": [
            "hash::tests::sha256_alimentado_em_pedacos_da_o_mesmo",
            "hash::tests::pbkdf2_sal_diferente_muda_tudo",
            "hash::tests::comparacao_em_tempo_constante",
        ],
    },
    {
        "id": "hmac-com-a-chave-longa-truncada",
        "titulo": "HMAC com a chave maior que o bloco TRUNCADA em vez de pré-hasheada",
        "porque": (
            "petrea do CLAUDE.md: «criptografia se confere contra vetor "
            "oficial». Defeito reposto: o `if chave.len() > SHA256_BLOCO` some "
            "e a chave passa a entrar cortada em 64 bytes. Escolhi este "
            "porque e o defeito de ALCANCE ESTREITO -- e defeito de alcance "
            "estreito e o que prova que UM vetor especifico da lista e "
            "portante. Um `if`/`else` cujos dois ramos fazem "
            "`copy_from_slice` num prefixo e o convite perfeito a "
            "«simplificacao»: `let n = chave.len().min(BLOCO)` cobre os dois "
            "casos, compila e passa em toda senha que alguem digita -- as do "
            "`senha.rs` e do `cofre.rs` tem menos de 64 bytes. "
            "Na suite inteira do `phxsql-core` so DUAS asercoes usam chave "
            "maior que o bloco: o caso 6 da RFC 4231 (131 bytes de 0xaa) e o "
            "sal de 80 bytes do anexo A.2 da RFC 5869, que entra na posicao "
            "da chave do HMAC. Se alguem apagar o caso 6 «porque os outros "
            "cinco ja cobrem», o HMAC desta casa deixa de ser HMAC para toda "
            "chave longa e nada acusa."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """    if chave.len() > SHA256_BLOCO {
        chave_bloco[..SHA256_LEN].copy_from_slice(&sha256(chave));
    } else {
        chave_bloco[..chave.len()].copy_from_slice(chave);
    }
""",
        "troca": """    // DEFEITO REPOSTO: os dois ramos viram um. A chave maior que o bloco
    // entra TRUNCADA em vez de pre-hasheada -- e a RFC 2104 manda hashear.
    let n = chave.len().min(SHA256_BLOCO);
    chave_bloco[..n].copy_from_slice(&chave[..n]);
""",
        "pacote": "phxhash",
        "alvo": ["--lib"],
        "caem": [
            "hash::tests::hmac_vetores_rfc4231",
        ],
        "seguem": [
            # As chaves curtas nao sentem nada, e e esse o ponto: PBKDF2,
            # senha e os outros dois casos do anexo A continuam verdes.
            "hash::tests::sha256_vetores_oficiais",
            "hash::tests::pbkdf2_vetores_conhecidos",
            "hash::tests::pbkdf2_saida_longa_atravessa_varios_blocos",
        ],
    },
    # O mesmo defeito, provado pelo lado do core: o hash.rs mudou para a
    # `phxhash` (no_std, pedido 450) e a HKDF que o pega ficou no core.
    {
        "id": "hmac-com-a-chave-longa-truncada-pela-hkdf",
        "titulo": "HMAC com a chave maior que o bloco TRUNCADA em vez de pré-hasheada",
        "porque": (
            "petrea do CLAUDE.md: «criptografia se confere contra vetor "
            "oficial». Defeito reposto: o `if chave.len() > SHA256_BLOCO` some "
            "e a chave passa a entrar cortada em 64 bytes. Escolhi este "
            "porque e o defeito de ALCANCE ESTREITO -- e defeito de alcance "
            "estreito e o que prova que UM vetor especifico da lista e "
            "portante. Um `if`/`else` cujos dois ramos fazem "
            "`copy_from_slice` num prefixo e o convite perfeito a "
            "«simplificacao»: `let n = chave.len().min(BLOCO)` cobre os dois "
            "casos, compila e passa em toda senha que alguem digita -- as do "
            "`senha.rs` e do `cofre.rs` tem menos de 64 bytes. "
            "Na suite inteira do `phxsql-core` so DUAS asercoes usam chave "
            "maior que o bloco: o caso 6 da RFC 4231 (131 bytes de 0xaa) e o "
            "sal de 80 bytes do anexo A.2 da RFC 5869, que entra na posicao "
            "da chave do HMAC. Se alguem apagar o caso 6 «porque os outros "
            "cinco ja cobrem», o HMAC desta casa deixa de ser HMAC para toda "
            "chave longa e nada acusa."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """    if chave.len() > SHA256_BLOCO {
        chave_bloco[..SHA256_LEN].copy_from_slice(&sha256(chave));
    } else {
        chave_bloco[..chave.len()].copy_from_slice(chave);
    }
""",
        "troca": """    // DEFEITO REPOSTO: os dois ramos viram um. A chave maior que o bloco
    // entra TRUNCADA em vez de pre-hasheada -- e a RFC 2104 manda hashear.
    let n = chave.len().min(SHA256_BLOCO);
    chave_bloco[..n].copy_from_slice(&chave[..n]);
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "hkdf::testes::caso_2_do_anexo_a",
        ],
        "seguem": [
            # As chaves curtas nao sentem nada, e e esse o ponto: PBKDF2,
            # senha e os outros dois casos do anexo A continuam verdes.
            "hkdf::testes::caso_1_do_anexo_a",
            "hkdf::testes::caso_3_do_anexo_a",
            "senha::tests::cifra_e_confere",
        ],
    },
    {
        "id": "pbkdf2-com-o-contador-de-bloco-parado",
        "titulo": "PBKDF2 com o contador de bloco parado: saída longa repete o primeiro bloco",
        "porque": (
            "petrea do CLAUDE.md: «criptografia se confere contra vetor "
            "oficial». Defeito reposto: o `bloco += 1` do fim do laco vira "
            "`bloco = 1`. Escolhi este pela mesma razao do HMAC de chave "
            "longa, e num lugar ainda mais estreito: TODA derivacao desta "
            "arvore pede 32 bytes -- `senha.rs`, `cifra.rs`, `desafio.rs`, o "
            "cofre --, e 32 bytes sao UM bloco, onde o contador nunca anda. "
            "Medido: a unica asercao do `phxsql-core` que pede saida maior "
            "que 32 bytes e `pbkdf2_saida_longa_atravessa_varios_blocos` "
            "(40 bytes). Ela e a guarda inteira. E o defeito e de refatorador "
            "de verdade: um `while` com dois contadores (`pos` e `bloco`) "
            "numa funcao de doze linhas e onde a reinicializacao vai parar no "
            "lugar errado, e nada no compilador reclama. O estrago: chave "
            "derivada de 64 bytes com as duas metades IGUAIS -- metade da "
            "entropia, em silencio."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """        pos += n;
        bloco += 1;
""",
        "troca": """        pos += n;
        // DEFEITO REPOSTO: o contador do bloco volta a 1 em vez de andar.
        // Saida de ate 32 bytes -- que e toda esta arvore -- nao sente nada;
        // acima disso os blocos saem repetidos.
        bloco = 1;
""",
        "pacote": "phxhash",
        "alvo": ["--lib"],
        "caem": [
            "hash::tests::pbkdf2_saida_longa_atravessa_varios_blocos",
        ],
        "seguem": [
            "hash::tests::pbkdf2_vetores_conhecidos",
            "hash::tests::pbkdf2_sal_diferente_muda_tudo",
            "hash::tests::sha256_vetores_oficiais",
            "hash::tests::hmac_vetores_rfc4231",
        ],
    },
    {
        "id": "pbkdf2-sem-o-xor-acumulado",
        "titulo": "PBKDF2 sem o XOR acumulado: vira HMAC aplicado N vezes",
        "porque": (
            "petrea do CLAUDE.md: «criptografia se confere contra vetor "
            "oficial». Defeito reposto: dentro do laco das iteracoes, o XOR "
            "de `u` sobre `acumulado` vira `acumulado = u` -- so a ultima "
            "volta sobrevive. Escolhi este porque e o que melhor mostra o "
            "buraco que a petrea fecha: o defeito NAO quebra nada do que um "
            "teste comum pergunta. O custo continua sendo N HMACs (nada fica "
            "mais rapido, entao nem a bancada acusa), a senha continua sendo "
            "conferida, o sal continua separando duas senhas iguais, o "
            "ida-e-volta do `senha.rs` continua fechando. E o resultado "
            "simplesmente NAO E PBKDF2: perde-se a garantia da RFC 2898 de "
            "que o encadeamento nao pode ser encurtado. Um refatorador comete "
            "este de verdade -- ha um `for` de XOR dentro de outro `for`, e "
            "trocar o interno por uma atribuicao e a forma mais comum de "
            "«tirar o laco aninhado». O `seguem` desta entrada e o inventario "
            "do que ficou VERDE com o defeito de pe."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """        for _ in 1..iteracoes {
            u = hmac_sha256(senha, &u);
            for (a, b) in acumulado.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }
""",
        "troca": """        for _ in 1..iteracoes {
            u = hmac_sha256(senha, &u);
            // DEFEITO REPOSTO: o XOR acumulado vira ATRIBUICAO. So a ultima
            // volta sobra, e o PBKDF2 vira "HMAC aplicado N vezes" -- que
            // custa o mesmo, confere senha do mesmo jeito e nao e PBKDF2.
            acumulado = u;
        }
""",
        "pacote": "phxhash",
        "alvo": ["--lib"],
        "caem": [
            "hash::tests::pbkdf2_vetores_conhecidos",
            "hash::tests::pbkdf2_saida_longa_atravessa_varios_blocos",
        ],
        "seguem": [
            # O inventario do que NAO pega: propriedade, ida-e-volta e sal.
            "hash::tests::pbkdf2_sal_diferente_muda_tudo",
            "hash::tests::sha256_vetores_oficiais",
            "hash::tests::hmac_vetores_rfc4231",
        ],
    },
    # -----------------------------------------------------------------------
    # 40. «O PORTAO E UM SO -- E O CAMPO QUE ELE LE E O FURO»: as tres
    #     operacoes que a petrea NOMEIA e o catalogo nao tinha
    #
    # Medido em 16/09/2026: a arvore tem 13 provas
    # `*_nao_e_a_porta_dos_fundos*` e o catalogo repunha o defeito de DUAS --
    # `pivotar` e `procurar_texto`. O `juntar` e o `unir`, os dois que a
    # propria petrea escreve («bastaria pedir a tabela negada como o lado B
    # de uma junção»), estavam de fora. A terceira aqui, `diferencas`, e a
    # mesma familia pelo mesmo motivo mecanico: os dois nomes chegam em
    # campos (`a` e `b`) que o `despachar` nao le.
    #
    # E o risco que estas tres entradas existem para cobrir e o que o
    # CLAUDE.md ja nomeia: numa divisao do `servidor.rs`, esta conferencia
    # propria PARECE duplicacao do portao geral. Quem a apagar reabre a porta
    # dos fundos -- e ate aqui nenhuma reposicao de defeito provava que os
    # testes pegariam. Agora prova.
    # -----------------------------------------------------------------------
    {
        "id": "juntar-sem-portao",
        "titulo": "`juntar` sem conferência própria: a tabela negada entra como lado B",
        "porque": (
            "petrea do CLAUDE.md, e a frase e dela: «sem conferencia propria, "
            "bastaria pedir a tabela negada como o lado B de uma junção». O "
            "portao geral do `despachar` confere o campo `tabela` do pedido, "
            "e uma junção NAO TEM esse campo -- as duas moram em `a.tabela` e "
            "`b.tabela`. Defeito reposto: a conferencia propria sai inteira, "
            "comentario junto, e a funcao volta a comecar pela trava de "
            "dados. O comentario sai DE PROPOSITO: comentario que se declara "
            "resolvido e o motivo de ninguem olhar de novo, e deixa-lo com o "
            "codigo fora seria repor meio defeito. "
            "O `seguem` traz `unir` e `pivotar`: as tres conferencias sao "
            "independentes, e apagar uma nao derruba as outras -- que e "
            "exatamente por que a falta nunca apareceu como defeito, e por "
            "que cada uma precisa da sua entrada. Traz tambem "
            "`o_join_pelo_sql_nao_e_a_porta_dos_fundos`, medido: um JOIN de "
            "SQL vira `consultar` no tradutor, e nao `juntar` -- entao ele "
            "NAO cobre este caminho, e quem confiasse nele estaria coberto "
            "por um teste que nao passa por aqui."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        // O portao geral confere o campo `tabela` do pedido -- e uma junção
        // NAO TEM esse campo: as duas tabelas moram em `a.tabela` e
        // `b.tabela`. Sem esta conferencia, juntar seria a porta dos fundos
        // para ler uma tabela negada, bastando pedi-la como o lado B.
        if let Some(u) = &sessao.usuario {
            let base = p.texto_ou("database", "");
            for alvo in [na, nb] {
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }
""",
        "troca": """        // DEFEITO REPOSTO: a conferencia propria some, e com ela o comentario
        // que a explica -- so o portao geral decide, e ele pergunta por um
        // campo `tabela` que a junção nao tem.
        let _ = sessao;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::juntar_nao_e_a_porta_dos_fundos",
        ],
        "seguem": [
            "servidor::testes_direito_por_tabela::unir_nao_e_a_porta_dos_fundos",
            "servidor::testes_direito_por_tabela::pivotar_nao_e_a_porta_dos_fundos",
            "servidor::testes_sql_composto::o_join_pelo_sql_nao_e_a_porta_dos_fundos",
            # O teste do comportamento VELHO, que e o que mais importa numa
            # regra de permissao -- ver `regra-de-tabela-imposta`.
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_nada_muda",
        ],
    },
    {
        "id": "unir-sem-portao",
        "titulo": "`unir` sem conferência própria: a tabela negada entra na LISTA",
        "porque": (
            "petrea do CLAUDE.md: das tres operacoes que escondem tabela do "
            "portao, esta e a que guarda o campo numa LISTA (`tabelas`) e nao "
            "num objeto. Defeito reposto: o laco de conferencia sai inteiro, "
            "com o comentario. "
            "O que esta entrada guarda alem da permissao e a ORDEM, e e por "
            "isso que ela nao e copia da do `juntar`: aqui a conferencia tem "
            "de vir DEPOIS de ler `nomes` -- e a lista que diz o que conferir "
            "-- e ANTES de `travar_dados`/`abrir_qualificada`. Quem «arrumar» "
            "a funcao subindo o bloco para junto dos outros portoes confere "
            "uma lista vazia e nao nega nada; quem o descer para depois da "
            "materializacao ja leu a tabela negada do disco antes de recusar. "
            "As duas arrumacoes compilam, e as duas passam em todo teste que "
            "nao seja este."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        // A conferencia vem DEPOIS de ler a lista, e nao antes, porque e a
        // lista que diz o que precisa ser conferido: o campo `tabela` que o
        // portao geral olha nao existe numa união. Cada tabela do pedido
        // precisa da sua propria permissao -- senao unir vira a porta dos
        // fundos para ler uma tabela negada.
        if let Some(u) = &sessao.usuario {
            for alvo in &nomes {
                if !u.pode_em(base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }
""",
        "troca": """        // DEFEITO REPOSTO: a conferencia da LISTA some, e com ela o
        // comentario que explica por que ela mora aqui e nao la em cima.
        let _ = sessao;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::unir_nao_e_a_porta_dos_fundos",
        ],
        "seguem": [
            "servidor::testes_direito_por_tabela::juntar_nao_e_a_porta_dos_fundos",
            "servidor::testes_direito_por_tabela::pivotar_nao_e_a_porta_dos_fundos",
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_nada_muda",
        ],
    },
    {
        "id": "diferencas-sem-portao",
        "titulo": "`diferencas` sem conferência própria: a tabela negada entra em `a` ou em `b`",
        "porque": (
            "a quarta da familia que a petrea descreve, achada contando as 13 "
            "provas `*_nao_e_a_porta_dos_fundos*` da arvore contra as 2 que o "
            "catalogo tinha. O mecanismo e identico ao do `juntar` e ainda "
            "mais direto: as duas tabelas chegam em campos de TEXTO chamados "
            "`a` e `b`, e o portao geral le `tabela`. Defeito reposto: o "
            "bloco de conferencia sai. "
            "O `seguem` traz a irma por COLUNA "
            "(`diferencas_recusa_a_tabela_com_regra_de_coluna`): portao "
            "vizinho, campo vizinho, e ela tem de continuar de pe, senao a "
            "troca estaria provando duas coisas ao mesmo tempo e nao "
            "provaria nenhuma."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        // A CONFERENCIA PROPRIA -- ver a nota do cabecalho.
        if let Some(u) = &sessao.usuario {
            for alvo in [&na, &nb] {
                if !u.pode_em(&base, alvo, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de ler em {base}.{alvo}",
                        u.login
                    )));
                }
            }
        }
""",
        "troca": """        // DEFEITO REPOSTO: a conferencia propria some. Os nomes chegam em
        // `a` e `b`, e o portao geral le `tabela` -- entao ninguem confere.
        let _ = sessao;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_diferencas::diferencas_nao_e_a_porta_dos_fundos",
        ],
        "seguem": [
            "servidor::testes_diferencas::diferencas_recusa_a_tabela_com_regra_de_coluna",
            "servidor::testes_direito_por_tabela::juntar_nao_e_a_porta_dos_fundos",
            "servidor::testes_direito_por_tabela::sem_regra_de_tabela_nada_muda",
        ],
    },
    {
        "id": "derivado-sem-portao",
        "titulo": "o portão some do irmão `executar_derivado`: o SQL inteiro vira a porta dos fundos",
        "porque": (
            "petrea do CLAUDE.md: «portao de permissao e UM so -- e o campo "
            "que ele le e o furo», lida pelo outro lado. As tres entradas "
            "acima cuidam das operacoes que escondem a TABELA do portao; esta "
            "cuida de quem esconde o PORTAO inteiro. O `despachar` nao e o "
            "unico caminho: um `UPDATE` pelo SQL e um `buscar` -> `ler` -> "
            "`atualizar` derivados, e nenhum dos tres passa por ele -- quem "
            "confere e o irmao `executar_derivado`, e irmao aqui e quem chama "
            "`portoes_do_pedido` e depois `executar`, na mesma ordem. Defeito "
            "reposto: a chamada ao portao sai desse irmao, e so a politica "
            "fica. "
            "Escolhi este ponto e nao um portao por operacao porque e o que "
            "um refatorador faz de verdade: `executar_derivado` tem TRES "
            "linhas e parece um embrulho fino do `executar` -- inlina-lo ou "
            "«tirar a indirecao» compila, passa no `clippy` e nao derruba "
            "nenhuma prova por soquete. "
            "MEDIDO com o defeito de pe, e e por isso que esta entrada "
            "sozinha vale oito: caem as OITO provas de porta dos fundos que "
            "chegam por este caminho -- `sql`, o DML pelo SQL, o JOIN pelo "
            "SQL, os dois do `consultar`, o `existe`, a visao e o `call`. E "
            "as duas do `seguem` continuam VERDES, tambem medido: `juntar` e "
            "`procurar_texto` entram pelo `despachar`, que tem a chamada "
            "dele -- o que prova que a troca e cirurgica e que as duas "
            "familias de guarda sao mesmo independentes."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        self.politica_do_pedido(op, pedido)?;
        self.portoes_do_pedido(op, pedido, sessao)?;
""",
        "troca": """        self.politica_do_pedido(op, pedido)?;
        // DEFEITO REPOSTO: o portao sai do irmao. O `despachar` continua com
        // o dele, entao tudo o que vem pela rede parece protegido -- e o SQL,
        // o `consultar`, a visao e o `call` passam por baixo.
        let _ = sessao;
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_direito_por_tabela::o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
            "servidor::testes_direito_por_tabela::o_dml_pelo_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
            "servidor::testes_sql_composto::o_join_pelo_sql_nao_e_a_porta_dos_fundos",
            "servidor::testes_consultar::o_consultar_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
            "servidor::testes_consultar_juncao::o_consultar_nao_e_a_porta_dos_fundos_pela_juncao_nem_pelo_escalar",
            "servidor::testes_consultar_juncao::o_existe_nao_e_a_porta_dos_fundos",
            "servidor::testes_visoes::a_visao_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
            "servidor::testes_gatilhos::call_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
        ],
        "seguem": [
            # Medidos verdes com o defeito de pe: as duas entram pelo
            # `despachar`, que guarda a sua propria chamada ao portao.
            "servidor::testes_direito_por_tabela::juntar_nao_e_a_porta_dos_fundos",
            "servidor::testes_direito_por_tabela::procurar_texto_nao_e_a_porta_dos_fundos_para_a_tabela_negada",
        ],
    },
    # =======================================================================
    # A PETREA «SENHA NUNCA EM TEXTO PURO», nove entradas -- 16/09/2026
    #
    # A lei do dono: «Senha nunca em texto puro. Nem em arquivo, nem em log,
    # nem em resposta do protocolo. Ha teste que falha se a ficha de usuario
    # vazar o hash.»
    #
    # O retrato de antes, MEDIDO e nao lembrado: 40 funcoes de teste da arvore
    # afirmam que um segredo NAO aparece numa saida, e so 6 delas apareciam no
    # `caem` de alguma entrada -- as seis do `profiler-recorta` e do
    # `profiler-recorta-largo`. Trinta e quatro provas sem defeito reposto
    # nenhum, inclusive as quatro que a petrea NOMEIA (a ficha, o arquivo, a
    # resposta do protocolo e o log).
    #
    # As nove foram escolhidas para cobrirem as QUATRO SAIDAS que a petrea
    # nomeia, e nao a que fosse mais facil de escrever:
    #
    #   arquivo    `senha-em-claro-no-cadastro`, `senha-velha-fica-no-arquivo`
    #   log        `profiler-sem-a-senha-dentro-do-sql`,
    #              `comando-invalido-vira-texto-cru`,
    #              `trilha-sem-o-nome-de-segredo`, `trilha-so-olha-o-nome`
    #   protocolo  `ficha-do-usuario-devolve-o-hash`,
    #              `cifra-reserializa-a-senha`
    #   Debug      `debug-da-cifra-mostra-a-senha`
    #
    # A quinta saida -- o `Debug` -- nao esta na frase da petrea e esta aqui de
    # proposito: e a unica que NENHUM teste de `--lib` alcanca, e por isso a
    # unica entrada desta leva que roda num binario de integracao.
    # =======================================================================
    {
        "id": "ficha-do-usuario-devolve-o-hash",
        "titulo": "a ficha do usuário passa a devolver o `senha_hash` junto",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... ha teste que "
            "falha se a ficha de usuario vazar o hash». A petrea nomeia a "
            "FICHA, e esta e a entrada que a repoe. O defeito escolhido e o "
            "que um programador apressado comete de verdade, e ele ate tem um "
            "pedido legitimo por tras: a tela de edicao quer receber o usuario "
            "inteiro para salvar de volta, e falta justamente o campo da "
            "senha. Acrescentar `senha_hash` a ficha resolve a tela, compila, "
            "passa no clippy e soa defensavel -- «hash nao e a senha». So que "
            "hash que sai pela rede e hash que se quebra offline, e a petrea "
            "nao abre excecao: ela diz FICHA. "
            "O alcance e a parte que ensina: a `ficha()` e uma so, e por ela "
            "passam o `usuarios`, o `usuario` e a resposta do login -- entao "
            "um campo acrescentado aqui vaza por tres operacoes, e quem o "
            "acrescenta enxerga so a tela que estava consertando."
            "\n\nRAIO MEDIDO (sonda `espera: \"nada muda\"`, 16/09/2026): **2 dos "
            "1.103** testes do `--lib` caem -- os dois do `caem`, e mais nenhum."
        ),
        "arquivo": "crates/phxsql-server/src/usuarios.rs",
        "trecho": """            ("exige_chave", Json::Bool(self.chave_publica.is_some())),
""",
        "troca": """            ("exige_chave", Json::Bool(self.chave_publica.is_some())),
            // DEFEITO REPOSTO: a ficha leva o hash junto, «porque a tela de
            // edicao precisa devolver o usuario inteiro para salvar». Hash
            // que sai pela rede e hash que se quebra offline.
            ("senha_hash", Json::texto_de(&self.senha_hash)),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "usuarios::tests::a_ficha_nunca_devolve_a_senha",
            "servidor::testes_cadastro_de_usuarios::a_senha_nunca_aparece_no_arquivo_nem_na_resposta",
        ],
    },
    {
        "id": "senha-em-claro-no-cadastro",
        "titulo": "a senha entra no config.json em texto puro: o `cifrar` sai do caminho de gravação",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro. Nem em "
            "arquivo». Este defeito esta NOMEADO no proprio comentario do "
            "teste desde que ele nasceu -- «repor o defeito e trocar "
            "`senha::cifrar(clara)` por `clara` em `objeto_do_usuario`» --, e "
            "nunca tinha sido reposto por ninguem: a instrucao estava escrita "
            "e a maquina nao a executava. E o retrato exato que esta casa "
            "chama de teste nao provado. "
            "O `objeto_do_usuario` e o UNICO ponto por onde a senha entra no "
            "arquivo, e e por isso que o defeito cabe numa linha so: quem "
            "«simplificasse» a derivacao aqui -- por exemplo para um script "
            "de importacao que ja traz o hash pronto -- poria a senha em claro "
            "no `config.json` de todo mundo, e o caminho de leitura "
            "continuaria funcionando, porque o `extrair_hash` ainda aceita "
            "`senha` em texto puro (avisando). O servidor subiria, o login "
            "entraria, e so o arquivo saberia."
            "\n\nRAIO MEDIDO (16/09/2026): **5 ou mais dos 1.103** -- e a unica "
            "desta leva que derruba meia duzia de provas, porque quebra o LOGIN "
            "junto. Ela ensina menos sobre alcance que as vizinhas e esta aqui "
            "assim mesmo: e o defeito que o comentario do teste manda repor, e "
            "ficar sem entrada era deixar a instrucao escrita sem ninguem executar."
        ),
        "arquivo": "crates/phxsql-server/src/usuarios.rs",
        "trecho": """            por("senha_hash", Json::texto_de(senha::cifrar(clara)));
""",
        "troca": """            // DEFEITO REPOSTO: a senha vai para o arquivo como veio. O
            // login continua entrando -- o `extrair_hash` aceita `senha` em
            // texto puro --, entao nada na tela denuncia; so o arquivo.
            por("senha_hash", Json::texto_de(clara));
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_cadastro_de_usuarios::a_senha_nunca_aparece_no_arquivo_nem_na_resposta",
            "usuarios::tests::trocar_a_senha_leva_junto_a_que_estava_em_texto_puro",
        ],
    },
    {
        "id": "senha-velha-fica-no-arquivo",
        "titulo": "trocar a senha não leva junto a que estava em texto puro no arquivo",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro. Nem em "
            "arquivo». Esta entrada e a IRMA da de cima, e existe por causa da "
            "lei «conserto entra no caminho que o motivou, e o caminho IRMAO "
            "fica»: derivar a senha nova esta certo e nao basta, porque o "
            "formato ainda aceita `senha` em texto puro num usuario antigo. "
            "Sem o `retain`, o usuario que tinha a senha em claro no "
            "`config.json` troca de senha, ganha um `senha_hash` novinho -- e "
            "continua com a VELHA em claro ao lado dele, agora sem ninguem "
            "olhar, porque a tela mostra que a troca deu certo. "
            "O que esta linha ensina esta no `seguem`: o caminho do usuario "
            "NOVO nao sente nada. `pares` nasce vazio quando nao ha anterior, "
            "e `senha` nao esta em `CAMPOS_DO_USUARIO` -- entao a prova que "
            "cobre a criacao fica VERDE com o defeito de pe. Uma so das duas "
            "provas do arquivo pega isto, e e a da ALTERACAO."
            "\n\nRAIO MEDIDO (16/09/2026): **1 dos 1.103**. E a mais estreita da "
            "leva, e por isso a que mais ensina: a prova da CRIACAO fica verde."
        ),
        "arquivo": "crates/phxsql-server/src/usuarios.rs",
        "trecho": """            // A senha em texto puro que o formato ainda aceita sai JUNTO: um
            // usuario que a tinha e trocou de senha nao pode continuar com a
            // velha em claro no arquivo, sem ninguem notar.
            pares.retain(|(k, _)| k != "senha");
""",
        "troca": """            // DEFEITO REPOSTO: a senha velha em claro FICA. A troca deu
            // certo na tela, o hash novo esta la, e o `config.json` guarda a
            // senha antiga ao lado dele.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "usuarios::tests::trocar_a_senha_leva_junto_a_que_estava_em_texto_puro",
        ],
        "seguem": [
            # O caminho do usuario NOVO nao sente: `pares` nasce vazio sem
            # anterior, e "senha" nao esta em CAMPOS_DO_USUARIO.
            "servidor::testes_cadastro_de_usuarios::a_senha_nunca_aparece_no_arquivo_nem_na_resposta",
            "usuarios::tests::a_ficha_nunca_devolve_a_senha",
        ],
    },
    {
        "id": "cifra-reserializa-a-senha",
        "titulo": "o `para_json` da cifra devolve a senha de verdade em vez de «(oculta)»",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em "
            "resposta do protocolo». O `config` e uma operacao do protocolo e "
            "e o que a tela de configuracao le. O defeito tem um pedido "
            "legitimo atras dele, e e por isso que ele e plausivel: a tela le "
            "`(oculta)` e, ao SALVAR, mandaria `(oculta)` de volta como senha "
            "-- quem visse essa quebra «consertaria» mandando o valor real, e "
            "o conserto errado cabe numa linha. O certo e a tela nao reenviar "
            "o campo. "
            "A escolha do campo tambem e medida: a `Cifra` e o segredo mais "
            "antigo do `config.json`, e o `para_json` dela esta a UM `else if` "
            "de distancia do da `Definicao` do dblink e do da `CifraFio` -- os "
            "tres tem a mesma forma, e o defeito reposto aqui e o que se "
            "copiaria para os outros dois num `find`/`replace`."
            "\n\nRAIO MEDIDO (16/09/2026): **3 dos 1.103**. O terceiro nao estava "
            "previsto e nao entrou no `caem` porque nao e guarda de vazamento: "
            "`a_senha_da_cifra_pode_vir_do_ambiente` cai por tabela, porque a marca "
            "`(do ambiente)` some junto. Fica dito para que a proxima corrida nao o "
            "leia como guarda nova."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        # Reancorada em 24/09/2026: a 372 levou a decisao do rotulo para
        # `Segredo::rotulo`, e o `if`/`else if` que este trecho copiava sumiu
        # do `para_json`. O defeito reposto e o mesmo -- o valor no lugar do
        # rotulo --, agora pela UNICA saida que o `Segredo` tem para o valor.
        "trecho": """            // Nunca a senha. Nem mascarada com asteriscos do tamanho certo --
            // o tamanho ja e informacao.
            (
                "senha",
                Json::texto_de(self.senha.rotulo("(vazia)", "(oculta)", "(do ambiente)")),
            ),
""",
        "troca": """            // DEFEITO REPOSTO: a senha sai inteira, «para a tela de
            // configuracao conseguir salvar de volta sem apaga-la».
            ("senha", Json::texto_de(self.senha.valor().unwrap_or(""))),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "config::tests::a_senha_da_cifra_nunca_sai_em_json",
            "config::tests::nenhuma_credencial_do_config_sai_pela_op_config",
        ],
        "seguem": [
            # Os outros dois segredos do mesmo arquivo continuam de pe: cada
            # `para_json` guarda o seu, e por isso o generico existe.
            "config::testes_alertas::a_senha_do_rele_nunca_aparece_no_json",
            "config::tests::a_credencial_do_cluster_nao_sai_em_json",
        ],
    },
    {
        "id": "debug-da-cifra-mostra-a-senha",
        "titulo": "o `Debug` da cifra imprime a senha: um `dbg!` apressado a joga no log",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro. ... nem em "
            "log». Esta e a saida que a frase da petrea nomeia e que NENHUM "
            "teste de `--lib` alcanca: os tres testes de JSON do `config.rs` "
            "ficam verdes com este defeito de pe, porque olham o `para_json` e "
            "o `Debug` e outro caminho. O comentario acima do `impl` ja avisa "
            "-- «segredo que aparece em `Debug` vaza no dia em que alguem "
            "acrescentar um `dbg!`» --, e comentario nao e guarda. "
            "O defeito e o que se comete de verdade, e nem e por descuido: "
            "quem depura «por que o cofre nao abre com a senha certa» troca "
            "essa linha de proposito, ve o que precisava ver, e esquece de "
            "desfazer. Nao ha nada de errado no codigo que sobra -- ele "
            "continua sendo um `Debug` escrito a mao, com um campo a mais "
            "aparecendo, que e o que faz o `git diff` parecer inocente."
            "\n\nRAIO MEDIDO (16/09/2026), e e o numero desta leva: com a senha "
            "saindo no `Debug`, **ZERO dos 1.103** testes do `--lib` caem. A sonda "
            "correu o binario inteiro e nenhum veredito mudou. Quem cai e UM teste "
            "de integracao, dos 5 do `cifra-pelo-config`. Mil e cem provas e um "
            "ponto cego."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """            .field("ligada", &self.ligada)
            .field("senha", &"(oculta)")
""",
        "troca": """            .field("ligada", &self.ligada)
            // DEFEITO REPOSTO: a senha aparece no `Debug`. Quem depurou «por
            // que o cofre nao abre» trocou esta linha e nao desfez.
            .field("senha", &self.senha)
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "cifra-pelo-config"],
        "caem": [
            "a_resposta_do_protocolo_nao_leva_a_senha",
        ],
    },
    {
        "id": "profiler-sem-a-senha-dentro-do-sql",
        "titulo": "o Profiler perde a senha que está DENTRO da frase SQL, e não num campo",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em log», "
            "com o corolario que o proprio Profiler obrigou a escrever: "
            "«funcionalidade que mostra texto cru redige ANALISANDO, nunca "
            "recortando». A redacao deste arquivo e por NOME de campo, e ela "
            "esta certa -- ate chegar "
            "`{\\\"op\\\":\\\"sql\\\",\\\"texto\\\":\\\"CREATE USER c PASSWORD 'x'\\\"}`, "
            "em que a senha nao esta num campo chamado `senha`: esta no meio "
            "de uma frase, num campo chamado `texto`. Este ramo foi achado "
            "EXERCITANDO o motor vivo (`bancada/usuarios/provar.py`, parte 8) "
            "e nao lendo o codigo, e por isso ele parece um enfeite para quem "
            "le o `limpar` de cima para baixo. "
            "O `seguem` e a razao de esta entrada existir separada das duas do "
            "`profiler-recorta`: a lista `SEGREDOS` continua inteira, entao "
            "`a_senha_nunca_aparece` -- que e a prova mais completa do arquivo, "
            "com oito pedidos -- fica VERDE com o defeito de pe. Os oito "
            "pedidos dela nomeiam a senha num campo; nenhum a esconde numa "
            "frase. Uma prova pode ser a mais completa do arquivo e ainda "
            "assim nao alcancar o ramo do vizinho."
            "\n\nRAIO MEDIDO (16/09/2026): **1 dos 1.103**."
        ),
        "arquivo": "crates/phxsql-server/src/profiler.rs",
        "trecho": """                    } else if let Some(sem) = sql_sem_senha(k, v) {
                        (k.clone(), Json::Texto(sem))
                    } else {
""",
        "troca": """                    // DEFEITO REPOSTO: o ramo do SQL sai. A lista `SEGREDOS`
                    // continua inteira e todo pedido com campo `senha` sai
                    // tapado -- so a senha que mora DENTRO da frase passa.
                    } else {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::a_senha_dentro_do_texto_sql_tambem_sai",
        ],
        "seguem": [
            # A prova mais completa do arquivo -- oito pedidos -- fica verde:
            # os oito nomeiam a senha num CAMPO.
            "profiler::testes::a_senha_nunca_aparece",
            "profiler::testes::o_sql_de_sempre_continua_visivel_no_anel",
            "profiler::testes::chave_com_espaco_no_nome_ainda_e_senha",
        ],
    },
    {
        "id": "comando-invalido-vira-texto-cru",
        "titulo": "o SQL que o léxico recusa volta inteiro para o log, com a senha dentro",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em log», e "
            "o corolario: «o que nao se analisa nao vira texto -- vira o "
            "tamanho em bytes. Se a estrutura nao se le, nao ha como tapar o "
            "campo dentro dela». O `sem_a_senha` redige por analise lexica; "
            "quando o lexico recusa o texto nao ha simbolo nenhum para tapar, "
            "e a unica saida honesta e o tamanho. "
            "O defeito e plausivel porque tem um motivo bom: um comando "
            "invalido e exatamente o que o operador quer VER no log para "
            "descobrir o erro de digitacao, e `<comando invalido, 31 bytes>` "
            "nao ajuda ninguem a achar a aspas que faltou. Quem devolve o "
            "texto cru esta consertando a usabilidade do log -- e "
            "`CREATE USER c PASSWORD 'aberta` e justamente um comando que o "
            "lexico recusa POR CAUSA da aspas da senha, entao o caso que mais "
            "pede o texto cru e o que mais o proibe. "
            "Este ponto e o mesmo que o `profiler-sem-a-senha-dentro-do-sql` "
            "alcanca, mas de fora: o Profiler CHAMA esta funcao. Sao dois "
            "pacotes e dois binarios, e o defeito repousa em `phxsql-sql`, "
            "onde nenhuma prova do servidor olha."
            "\n\nRAIO MEDIDO (16/09/2026): **1 dos 253** do `phxsql-sql --lib`."
        ),
        "arquivo": "crates/phxsql-sql/src/usuario.rs",
        "trecho": """    let Ok(simbolos) = lexico::analisar(texto) else {
        return format!("<comando invalido, {} bytes>", texto.trim().len());
    };
""",
        "troca": """    let Ok(simbolos) = lexico::analisar(texto) else {
        // DEFEITO REPOSTO: o comando que o lexico recusou volta inteiro,
        // «para o operador conseguir ver o erro de digitacao no log». O
        // comando que ele mais precisa ver e o que tem a aspas da senha
        // faltando -- e ai a senha vai junto.
        return texto.trim().to_string();
    };
""",
        "pacote": "phxsql-sql",
        "alvo": ["--lib"],
        "caem": [
            "usuario::testes::o_que_nao_se_analisa_vira_o_tamanho",
        ],
        "seguem": [
            # O comando VALIDO continua redigido: o defeito so alcanca o
            # caminho em que nao ha estrutura para tapar.
            "usuario::testes::a_senha_sai_do_texto_do_comando",
            "usuario::testes::senha_sem_aspas_e_recusada_sem_ecoar_o_que_veio",
        ],
    },
    {
        "id": "trilha-sem-o-nome-de-segredo",
        "titulo": "a trilha LGPD deixa de olhar o NOME da coluna e só analisa o valor",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro. Nem em arquivo, "
            "nem em log» -- e a trilha LGPD e as duas coisas ao mesmo tempo: "
            "um arquivo que e um log. O `valor_para_trilha` tem DUAS "
            "conferencias, e esta entrada tira a primeira. "
            "O defeito e o que um refatorador comete com a melhor das "
            "intencoes, e o raciocinio dele ate soa certo: «a analise abaixo "
            "ja destrincha o hash pelo formato, entao a lista de nomes e "
            "redundante e so gera falso positivo -- uma coluna `token_fiscal` "
            "que nem segredo e». O erro esta no CASO PIOR, que e exatamente o "
            "que a petrea nomeia: senha em TEXTO PURO numa coluna `senha` nao "
            "e um hash, nao destrincha em `pbkdf2-sha256$...`, e passa inteira "
            "para a trilha. A analise so pega o que JA esta protegido; o nome "
            "e o que pega o que nao esta."
            "\n\nRAIO MEDIDO (16/09/2026): **1 dos 187** do `phxsql-store --lib`."
        ),
        "arquivo": "crates/phxsql-store/src/trilha.rs",
        "trecho": """    if nome_de_segredo(coluna) {
        return redigir(bytes_do_valor(v));
    }
""",
        "troca": """    // DEFEITO REPOSTO: o nome da coluna deixa de contar, «porque a analise
    // do valor abaixo ja pega o hash». Ela pega o hash; a senha em TEXTO
    // PURO numa coluna `senha` nao e hash, e passa inteira.
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "trilha::testes::coluna_de_senha_nao_entrega_o_valor",
        ],
        "seguem": [
            # A OUTRA conferencia continua inteira, e por isso a prova dela
            # fica verde: as duas nao se cobrem.
            "trilha::testes::hash_em_coluna_de_nome_inocente_e_redigido",
            "trilha::testes::nulo_nao_e_segredo",
            "trilha::testes::valor_comum_passa_inteiro",
        ],
    },
    {
        "id": "trilha-so-olha-o-nome-da-coluna",
        "titulo": "a trilha LGPD deixa de ANALISAR o valor e só confia no nome da coluna",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro», com o "
            "corolario «redige ANALISANDO, nunca recortando». Esta entrada e a "
            "METADE CONTRARIA da `trilha-sem-o-nome-de-segredo`, e as duas "
            "existem juntas de proposito: cada uma tira uma das duas "
            "conferencias, e a prova que cai numa fica verde na outra. "
            "E o par que mostra por que «tem teste» nao quer dizer «esta "
            "coberto»: as duas provas do arquivo tem nome parecido, moram "
            "coladas, e nenhuma das duas sozinha diz que as duas conferencias "
            "existem. "
            "O defeito e plausivel pelo custo: `senha::e_hash` roda em TODO "
            "valor de TODA coluna marcada de TODA alteracao registrada, e "
            "quem for caçar tempo na trilha olha para essa linha primeiro -- "
            "«a coluna ja diz o que e, analisar o conteudo e trabalho "
            "dobrado». O que se perde e o hash gravado numa coluna de nome "
            "inocente, que e o caso que nenhum nome alcanca."
            "\n\nRAIO MEDIDO (16/09/2026): **1 dos 187** -- e o par com a de cima "
            "cobre as duas conferencias com um teste cada, medido."
        ),
        "arquivo": "crates/phxsql-store/src/trilha.rs",
        "trecho": """        Value::Str(s) | Value::Memo(s) if phxsql_core::senha::e_hash(s) => redigir(s.len()),
""",
        "troca": """        // DEFEITO REPOSTO: a analise do valor sai, «porque a coluna ja diz o
        // que e». O hash numa coluna de nome inocente vai inteiro.
""",
        "pacote": "phxsql-store",
        "alvo": ["--lib"],
        "caem": [
            "trilha::testes::hash_em_coluna_de_nome_inocente_e_redigido",
        ],
        "seguem": [
            # A conferencia pelo NOME continua, e a prova dela fica verde.
            "trilha::testes::coluna_de_senha_nao_entrega_o_valor",
            "trilha::testes::valor_comum_passa_inteiro",
        ],
    },
    {
    "id": "debug-da-ligacao-mostra-a-senha",
    "titulo": "o `Debug` da ligação de DbLink imprime a senha e o token do outro banco",
    "porque": (
        "petrea do CLAUDE.md: «senha nunca em texto puro. ... nem em log». "
        "E a IRMA da `debug-da-cifra-mostra-a-senha`, e a prova de que aquela "
        "guarda protegia UMA struct e nao a lei: a `Cifra` e a `CifraFio` "
        "escreviam o `Debug` a mao desde sempre, e outras nove estruturas "
        "continuavam derivando -- 14 campos de segredo ao todo, achados na "
        "varredura de 16/09/2026. "
        "Os dois campos da `Definicao` traziam o comentario que declarava o "
        "problema resolvido -- «ela nunca sai em JSON nem em log» e «ele nunca "
        "sai em JSON, em log nem na tela» --, e os dois estavam certos sobre o "
        "`para_json` e errados sobre o `Debug`: comentario que se declara "
        "resolvido e o motivo de ninguem olhar de novo. "
        "O token e o pior dos dois, e o proprio campo diz por que: no PhxSql "
        "ele e o portao 1, conferido ANTES do login -- quem o tem alcanca a "
        "porta de dados do outro servidor sem usuario nenhum. E o `Registro` "
        "guarda as ligacoes num `Vec`, entao UM `dbg!` despeja a credencial de "
        "TODAS as ligacoes cadastradas de uma vez."
        "\n\nA troca desfaz o conserto por dentro em vez de devolver o "
        "`derive(Debug)`: devolver o derive esbarraria no `impl` escrito a mao "
        "e o pacote nao compilaria, e guarda que nao compila nao prova nada. "
        "As duas trocas juntas reproduzem exatamente o que o derivado fazia."
    ),
    "trocas": [
        {
            "arquivo": "crates/phxsql-server/src/dblink/mod.rs",
            "trecho": """            senha: _,
            senha_env,
            token: _,
""",
            "troca": """            // DEFEITO REPOSTO (1/2): a senha e o token voltam a ser lidos.
            senha,
            senha_env,
            token,
""",
        },
        {
            "arquivo": "crates/phxsql-server/src/dblink/mod.rs",
            "trecho": """            .field("senha", &"(oculta)")
            .field("senha_env", senha_env)
            .field("token", &"(oculto)")
""",
            "troca": """            // DEFEITO REPOSTO (2/2): e voltam a ser impressos, que e o que o
            // `derive(Debug)` fazia.
            .field("senha", senha)
            .field("senha_env", senha_env)
            .field("token", token)
""",
        },
    ],
    "pacote": "phxsql-server",
    "alvo": ["--lib"],
    # Com o MODULO na frente, porque e o nome que o `cargo test` imprime e
    # que o `julgar` do provador compara: sem ele a entrada nasceu QUEBRADA
    # («teste que o catalogo nomeia e o binario nao tem»), medido em
    # 17/09/2026 -- e a regua barata nao viu, porque ela casa pelo nome da
    # `fn` e nao pelo caminho.
    "caem": [
        "dblink::testes::o_debug_da_ligacao_nunca_mostra_a_senha_nem_o_token",
    ],
    # O `Debug` nao pode ficar CEGO: esconder o nome da variavel de ambiente
    # trocaria um vazamento por um diagnostico inutil. E o `para_json`, que ja
    # estava certo, segue verde -- a troca mexe so na saida do `Debug`.
    "seguem": [
        "dblink::testes::o_debug_da_ligacao_mantem_o_nome_da_variavel_de_ambiente",
        "dblink::testes::a_senha_da_ligacao_nunca_aparece_no_json",
    ],
},
    # =======================================================================
    # A PETREA «SENHA NUNCA EM TEXTO PURO», segunda leva -- 17/09/2026
    #
    # A primeira leva (16/09) cobriu as QUATRO saidas que a frase da petrea
    # nomeia mais o `Debug`. Esta cobre as saidas que a frase NAO nomeia e que
    # a arvore ja provava sem guarda: o FIO (o protocolo em si, cifrado), o
    # DIARIO das diretivas (um log que e arquivo), a op `config` pelo lado do
    # CLUSTER (hash + token entre nos) e pela IRMA da cifra (a privada do
    # fio), a TELA no sentido de ENTRADA (o token do REST), e a STRING DE
    # CONEXAO do ODBC (um arquivo de configuracao de aplicativo, em outro
    # pacote).
    #
    # Uma entrada por SAIDA, e nao uma por struct: onde o mesmo defeito cabe
    # em N structs, o que protege a lei e uma regua que conte os lugares, nao
    # N entradas (ver `docs/cognicao/cognicao_guarda-trava-a-struct-nao-a-lei_
    # 20260917_0010.md`). Por isso as cinco structs que ganharam `impl Debug`
    # em 17/09 sem entrada propria continuam sem -- de proposito.
    # =======================================================================
    {
        "id": "fio-cifrado-manda-o-claro-junto",
        "titulo": "o fio cifrado manda a linha em claro junto do registro selado",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em "
            "resposta do protocolo» -- e o fio E o protocolo. O `Canal` "
            "existe, diz o proprio comentario dele, para que «a decisao de "
            "cifrar seja UMA so, e a que alguem esquecesse mandaria texto "
            "claro por um fio que o cliente acha cifrado». Esta entrada repoe "
            "exatamente isso, e nao por esquecimento: por diagnostico. Quem "
            "depura o tunel quer ver no `tcpdump` o que foi selado, poe a "
            "linha em claro DEPOIS do registro -- «o cliente le o registro e "
            "ignora o resto» -- e nao desfaz. O cliente de fato le o registro; "
            "o claro fica no fio para quem escuta, e o `login` com o token vai "
            "junto. "
            "A hipotese com que esta entrada foi escrita MORREU medida: eu "
            "esperava que as provas de ida e volta ficassem verdes «lendo um "
            "registro de cada vez, com o claro sobrando no buffer». Nao ficam: "
            "`canal_leva_e_traz` cai junto, porque le a DESPEDIDA depois do "
            "pedido e o que encontra no meio e a linha em claro, que nao e "
            "Base64 de registro nenhum. O que fica verde sao as outras 346 do "
            "pacote -- inclusive `fim_e_corte_sao_vereditos_diferentes`, que "
            "sela por fora e nunca passa por `escrever`. O defeito e barulhento "
            "no protocolo e silencioso na petrea, e e isso que a entrada "
            "ensina: o ida-e-volta cai por um motivo de FORMATO, e um conserto "
            "que ensinasse o leitor a pular a linha que nao e Base64 o "
            "reverdeceria com o vazamento de pe. So a prova que olha os BYTES "
            "do fio segura o segredo."
            "\n\nRAIO MEDIDO (sonda `espera: \"nada muda\"` sobre o binario "
            "inteiro, 17/09/2026): **2 dos 348** do `phxsql-core --lib` -- a "
            "prova do vazamento e `canal_leva_e_traz`, que nao entra no `caem` "
            "porque nao e guarda da petrea: cai pelo formato, nao pelo segredo."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """            Canal::Cifrado(t) => {
                let registro = t.selar(Tipo::Pedido, linha.as_bytes())?;
                writeln!(saida, "{registro}")?;
            }
""",
        "troca": """            Canal::Cifrado(t) => {
                let registro = t.selar(Tipo::Pedido, linha.as_bytes())?;
                writeln!(saida, "{registro}")?;
                // DEFEITO REPOSTO: a linha em claro vai junto, depois do
                // registro, «para o tcpdump do suporte ler o que foi selado».
                // O cliente le o registro; o claro fica no fio.
                writeln!(saida, "{linha}")?;
            }
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::o_texto_claro_nao_aparece_no_fio",
        ],
    },
    {
        "id": "diario-das-diretivas-guarda-o-segredo-anterior",
        "titulo": "o diário das diretivas grava o valor ANTERIOR do campo sigiloso em claro",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro. Nem em arquivo, "
            "nem em log» -- e o diario das diretivas e as duas coisas: um log "
            "que e arquivo, gravado pelo servidor toda vez que alguem muda um "
            "campo pela tela. O `para_json` da `Alteracao` mascara os DOIS "
            "valores do campo sigiloso, e esta entrada destapa so o anterior. "
            "O defeito tem um incidente por tras, que e o que o faz plausivel: "
            "«alguem trocou o token da porta REST e o integrador parou; "
            "preciso do valor ANTERIOR para reverter». Destapar o anterior "
            "parece inofensivo -- «o novo continua oculto, e o velho ja nao "
            "vale» --, e esta errado nas duas metades: o token velho continua "
            "abrindo a porta ate o servidor reler o arquivo, e a senha do "
            "rele de e-mail que foi trocada por ROTACAO e a mesma que valeu "
            "por um ano. "
            "O que esta entrada ensina: o mesmo `campo_sigiloso` alimenta o "
            "`SHOW SERVER SETTINGS`, e aquela prova fica verde -- ela olha o "
            "valor VIVO, que nunca passa por este `para_json`. Duas saidas, "
            "uma lista de nomes, e so uma das duas percorre a linha destapada."
            "\n\nRAIO MEDIDO (17/09/2026): **1 dos 1.107** do `phxsql-server "
            "--lib` -- e `o_show_server_settings_nao_vaza_segredo` ficou verde, "
            "como previsto: o valor vivo nunca passa por este `para_json`."
        ),
        "arquivo": "crates/phxsql-server/src/diretivas.rs",
        "trecho": """            ("valor_anterior", valor(&self.valor_anterior)),
""",
        "troca": """            // DEFEITO REPOSTO: o valor anterior sai inteiro, «para reverter
            // a troca de um token durante um incidente». O diario e um log
            // em arquivo, e o segredo velho continua valendo ate alguem
            // trocar de novo.
            ("valor_anterior", self.valor_anterior.clone()),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "diretivas::testes::o_valor_do_campo_sigiloso_nao_vai_para_o_diario",
        ],
        "seguem": [
            # A linha continua com os nove campos e continua se lendo de
            # volta: o defeito nao quebra o diario, so o destapa.
            "diretivas::testes::os_nove_campos_estao_todos_na_linha",
            "diretivas::testes::grava_e_le_da_mais_recente_para_a_mais_antiga",
        ],
    },
    {
        "id": "cluster-devolve-a-credencial-na-tela",
        "titulo": "o resumo do cluster na op `config` leva o token entre nós e o hash do replicador",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em "
            "resposta do protocolo. Ha teste que falha se a ficha de usuario "
            "vazar o hash». O `Cluster::para_json` e o resumo que a tela le, e "
            "o comentario dele ja diz «sem token e sem hash». O defeito e o "
            "mesmo da `ficha-do-usuario-devolve-o-hash`, so que numa secao em "
            "que ninguem o esperaria: a tela do cluster quer mostrar «qual "
            "usuario este no usa para replicar», e quem acrescenta o `usuario` "
            "leva `senha_hash` e `token` no mesmo `vec!` porque «hash nao e "
            "senha» e «o token so vale entre os nos». Hash que sai pela rede "
            "e hash que se quebra offline; e o token do cluster e o que abre o "
            "fio entre os nos SEM usuario nenhum. "
            "O que esta entrada ensina, medido: o teste GENERICO do "
            "`config.rs` (`nenhuma_credencial_do_config_sai_pela_op_config`) "
            "tem o token e o hash do cluster na lista dele e cai junto. Ele "
            "existe para pegar «o campo que alguem acrescentar amanha» -- e "
            "pega, AQUI. Compare com a `cifra-do-fio-reserializa-a-privada` e "
            "a `token-do-rest-entra-pela-tela`, logo abaixo, onde a mesma "
            "lista generica nao alcanca: a lista e digitada, e envelheceu."
            "\n\nRAIO MEDIDO (17/09/2026): **2 dos 1.107** -- os dois do `caem`, "
            "e mais nenhum."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """            ("email", Json::Bool(self.email.ligado)),
            ("quorum_minimo", Json::de_u64(self.quorum_minimo)),
""",
        "troca": """            ("email", Json::Bool(self.email.ligado)),
            ("quorum_minimo", Json::de_u64(self.quorum_minimo)),
            // DEFEITO REPOSTO: a credencial do replicador sai na tela, «para
            // o operador conferir qual usuario o cluster usa» -- e o hash e
            // o token vao no mesmo `vec!` porque «hash nao e senha». Hash que
            // sai pela rede se quebra offline; o token abre o fio entre os nos.
            ("usuario", Json::texto_de(&self.usuario)),
            ("senha_hash", Json::texto_de(&self.senha_hash)),
            ("token", Json::texto_de(&self.token)),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "config::tests::a_credencial_do_cluster_nao_sai_em_json",
            "config::tests::nenhuma_credencial_do_config_sai_pela_op_config",
        ],
        "seguem": [
            # A `Cifra` continua guardando o proprio segredo: cada `para_json`
            # protege o seu, e a troca so mexe no do cluster.
            "config::tests::a_senha_da_cifra_nunca_sai_em_json",
        ],
    },
    {
        "id": "token-do-rest-entra-pela-tela",
        "titulo": "o token da porta REST passa a se gravar pela tela de configuração",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro», lida no sentido "
            "CONTRARIO ao das outras entradas: nao e o segredo SAINDO para a "
            "tela, e o segredo ENTRANDO por ela. As 8 entradas desta petrea "
            "que vieram antes repoem vazamento; esta repoe uma porta de "
            "entrada, e e a unica prova das 40 que olha esse lado. "
            "O defeito e um pedido legitimo de operacao: «trocar o token da "
            "porta REST pela tela em vez de editar o arquivo», e cabe numa "
            "linha na lista `CAMPOS_EDITAVEIS`, ao lado de sete campos do "
            "mesmo bloco que ja sao editaveis. O comentario do bloco do "
            "cluster, cinquenta linhas abaixo, ja diz por que nao: «credencial "
            "se edita no arquivo». Pela tela o token viaja em claro no corpo do "
            "pedido, para no historico do navegador e no log do proxy, e uma "
            "sessao tomada passa a trocar a fechadura -- que e a razao de o "
            "`token` do servidor nunca ter entrado nessa lista. "
            "O que esta entrada ensina: o teste generico do `config.rs` e todas "
            "as provas de `para_json` ficam VERDES, porque olham o que sai. "
            "Guarda de saida nao ve porta de entrada."
            "\n\nRAIO MEDIDO (17/09/2026): **1 dos 1.107** -- o generico e as "
            "provas de `para_json` ficaram verdes, medido e nao argumentado."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """    ("rest.swagger_ligado", TipoDoCampo::Booleano, false),
    ("rest.swagger_bind", TipoDoCampo::Texto, false),
""",
        "troca": """    ("rest.swagger_ligado", TipoDoCampo::Booleano, false),
    ("rest.swagger_bind", TipoDoCampo::Texto, false),
    // DEFEITO REPOSTO: o token da porta passa a se trocar pela tela, «para
    // nao ter de editar o arquivo». Ele viaja em claro no pedido, para no
    // historico do navegador e no log do proxy -- e uma sessao tomada passa
    // a trocar a fechadura.
    ("rest.token", TipoDoCampo::Texto, false),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "config::tests::o_token_do_rest_nao_sai_nem_entra_pela_tela",
        ],
        "seguem": [
            # Sao provas de SAIDA, e o defeito e de entrada: ficam verdes.
            "config::tests::nenhuma_credencial_do_config_sai_pela_op_config",
            "config::tests::a_lista_de_tabelas_so_aceita_lista_de_textos",
        ],
    },
    {
        "id": "cifra-do-odbc-volta-a-nascer-em-claro",
        "titulo": "a receita do driver ODBC volta a nascer em claro, e o esquecimento vira o padrao",
        "porque": (
            "decisao do dono, 18/09/2026 (pedido 373): «a comunicacao deve "
            "obrigatoriamente ser cifrada». A receita do ODBC NASCE cifrada e "
            "`CIFRA=0` e o escape escrito -- quem quer claro escreve, em vez "
            "de esquecer. "
            "O defeito e uma linha, e ele tem o disfarce mais convincente que "
            "existe: `cifra: false` no `Default` parece a escolha conservadora "
            "de quem nao quer quebrar cliente antigo. So que aqui o cliente "
            "antigo nao quebra -- ele passa a falar claro CALADO, com a senha "
            "no fio, e ninguem recebe erro nenhum. "
            "O que esta entrada ensina, e foi medido em 18/09: o padrao mora "
            "no `impl Default for Receita` e NAO no analisador da receita, "
            "porque o `SQLConnect` com `host:porta/database` monta a receita "
            "com `..Receita::default()` e nao passa pelo analisador. Guarda "
            "posta no analisador deixaria o irmao falando claro, calado -- e "
            "e a mesma lei que esta casa ja pagou tres vezes num dia: o "
            "conserto entra no caminho que o motivou, e o caminho IRMAO fica."
            "\n\nRAIO MEDIDO (18/09/2026): **5 dos 66** do "
            "`phxsql-odbc --lib`."
        ),
        "arquivo": "crates/phxsql-odbc/src/conexao.rs",
        "trecho": """            database: String::new(),
            cifra: true,
""",
        "troca": """            database: String::new(),
            // DEFEITO REPOSTO: o padrao volta a ser claro, «para nao quebrar
            // quem ja tem receita escrita». Ninguem recebe erro: o cliente
            // velho passa a falar claro calado, com a senha no fio.
            cifra: false,
""",
        "pacote": "phxsql-odbc",
        "alvo": ["--lib"],
        "caem": [
            "conexao::testes::sem_escrever_nada_a_receita_nasce_cifrada",
            "conexao::testes::o_caminho_do_sqlconnect_tambem_nasce_cifrado",
            "conexao::testes::valor_torto_na_cifra_nao_rebaixa_para_claro",
            "conexao::testes::a_mascarada_leva_o_modo_nos_dois_sentidos",
            "conexao::testes::o_aperto_recusado_ensina_a_saida",
        ],
        "seguem": [
            # O escape escrito continua valendo: quem manda CIFRA=0 fala claro
            # com o padrao ligado E com ele desligado. E' o teste do
            # comportamento pedido, e ele nao distingue os dois mundos.
            "conexao::testes::o_escape_escrito_e_o_cifra_zero",
        ],
    },
    {
        "id": "receita-odbc-devolve-a-senha",
        "titulo": "a connection string mascarada do ODBC devolve a senha inteira",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro. Nem em "
            "arquivo». A string que `receita_mascarada` devolve e a que o "
            "driver ODBC entrega de volta ao aplicativo (`SQLDriverConnect` "
            "com a OutConnectionString), e o aplicativo a grava no PROPRIO "
            "arquivo de configuracao -- e o comentario do teste que diz isso. "
            "Fora do servidor, em outro pacote, num arquivo que nao e nosso: "
            "e a saida mais longe da frase da petrea, e por isso a que menos "
            "gente lembra. "
            "O defeito e o pedido que o proprio comentario da funcao recusa: "
            "«quem quiser reconectar guarda a receita inteira» -- e quem "
            "escreve um aplicativo que reconecta acha mais simples pedir a "
            "string de volta com a senha dentro. Uma linha, e a senha do banco "
            "vai para o `.ini` de todo cliente Windows. "
            "O que esta entrada ensina: e a unica desta petrea em "
            "`phxsql-odbc`, e nenhuma prova do servidor a alcanca -- o "
            "vazamento acontece num processo que o servidor nem ve."
            "\n\nRAIO MEDIDO (18/09/2026): **1 dos 66** do `phxsql-odbc --lib` -- eram 59 em 17/09, e o numero velho e o exemplo da propria lei: raio digitado envelhece calado quando a suite cresce."
        ),
        "arquivo": "crates/phxsql-odbc/src/conexao.rs",
        "trecho": """    if !r.senha.is_empty() {
        s.push_str(";PWD=***");
    }
""",
        "troca": """    if !r.senha.is_empty() {
        // DEFEITO REPOSTO: a senha volta inteira, «porque o aplicativo
        // precisa da string completa para reconectar». A string de volta vai
        // parar no arquivo de configuracao do aplicativo -- senha em texto
        // puro num arquivo que nao e nosso.
        s.push_str(&format!(";PWD={}", r.senha));
    }
""",
        "pacote": "phxsql-odbc",
        "alvo": ["--lib"],
        "caem": [
            "conexao::testes::mascarada_nao_vaza_segredo",
        ],
        "seguem": [
            # O pino e o modo continuam como estavam: a troca so destapa a senha.
            "conexao::testes::mascarada_diz_o_modo_e_nao_o_pino",
        ],
    },
    {
        "id": "cifra-do-fio-reserializa-a-privada",
        "titulo": "o `para_json` da cifra do fio devolve a chave privada em vez de «(oculta)»",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em "
            "resposta do protocolo». E a IRMA da `cifra-reserializa-a-senha`, "
            "e existe pela lei de 03/09: «conserto entra no caminho que o "
            "motivou, e o caminho IRMAO fica». A `CifraFio` tem um `para_json` "
            "da MESMA forma que o da `Cifra` -- o mesmo `if`/`else if` com "
            "`(oculta)` e `(do ambiente)` --, e o defeito que a §15.7 repos na "
            "`Cifra` e o que se copia para ca num `find`/`replace`: a tela le "
            "`(oculta)`, mandaria `(oculta)` de volta ao salvar, e o conserto "
            "errado e devolver o valor real. So que aqui o valor real e a "
            "chave PRIVADA X25519 do servidor: quem a tem faz o aperto de mao "
            "no lugar dele. "
            "O que esta entrada ensina, medido: o teste generico do "
            "`config.rs` -- que existe para pegar «o campo que alguem "
            "acrescentar amanha» -- NAO tem a privada do fio na lista dele, e "
            "fica verde. A lista dos dez segredos e digitada, e a `CifraFio` "
            "nasceu depois dela. Receita de um numero tambem envelhece, e uma "
            "lista de segredos num teste e uma receita."
            "\n\nRAIO MEDIDO (17/09/2026): **1 dos 1.107** -- o generico ficou "
            "VERDE com a privada saindo inteira. E o numero desta leva."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        # Reancorada em 24/09/2026, pelo mesmo motivo da irma: a 372 levou o
        # rotulo para `Segredo::rotulo`.
        "trecho": """            // Nunca a privada -- nem mascarada, que o tamanho ja e informacao.
            (
                "chave_privada",
                Json::texto_de(self.chave_privada.rotulo(
                    "(do arquivo)",
                    "(oculta)",
                    "(do ambiente)",
                )),
            ),
""",
        "troca": """            // DEFEITO REPOSTO: a privada sai inteira, «para a tela conseguir
            // salvar a secao de volta sem apaga-la» -- o mesmo pedido que fez
            // a `Cifra` vizinha vazar, copiado para a irma.
            ("chave_privada", Json::texto_de(self.chave_privada.valor().unwrap_or(""))),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "config::tests::a_privada_do_fio_nunca_sai",
        ],
        "seguem": [
            # O generico fica VERDE -- e esse e o ponto: a privada do fio nao
            # esta na lista de segredos dele.
            "config::tests::nenhuma_credencial_do_config_sai_pela_op_config",
            "config::tests::a_senha_da_cifra_nunca_sai_em_json",
        ],
    },
    {
        "id": "especificacao-openapi-leva-o-token",
        "titulo": "a especificação OpenAPI, servida sem portão, passa a carregar o token da porta",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro... nem em "
            "resposta do protocolo». A especificacao e a resposta mais publica "
            "que este servidor da: `GET /openapi.json` responde ANTES de "
            "qualquer portao (o `match` do caminho vem logo depois de ler o "
            "pedido, e o comentario ali diz por que -- «documentacao presa "
            "atras de uma opcao» nao serve a quem gera cliente). Quem alcanca "
            "a porta le o documento sem token nenhum. "
            "O defeito e conveniencia de desenvolvimento, e por isso e "
            "plausivel: «o explorador pede o token toda vez que abre; poe o "
            "token na especificacao e o Swagger UI vem pre-autenticado». Cabe "
            "numa linha `x-...` no topo do documento, que nenhum validador "
            "de OpenAPI recusa -- extensao e para isso. So que o mesmo "
            "documento vai para o proxy, para o cache do navegador e para o "
            "gerador de cliente de quem nunca deveria ter o token. "
            "O que esta entrada ensina: nenhuma prova de `config.rs` a "
            "alcanca -- o `Rest::para_json` continua sem o token, e e ele "
            "que todas olham. A especificacao e OUTRA serializacao do mesmo "
            "`Rest`, num modulo que a lista generica de segredos nem conhece."
            "\n\nRAIO MEDIDO (17/09/2026): **1 dos 1.107**."
        ),
        "arquivo": "crates/phxsql-server/src/rest.rs",
        "trecho": """        (
            "security",
            Json::Lista(vec![Json::objeto(vec![("token", Json::Lista(Vec::new()))])]),
        ),
        ("components", componentes()),
""",
        "troca": """        (
            "security",
            Json::Lista(vec![Json::objeto(vec![("token", Json::Lista(Vec::new()))])]),
        ),
        // DEFEITO REPOSTO: o token vai na especificacao, «para o explorador
        // vir pre-autenticado em vez de pedir o token a cada abertura». O
        // documento e servido sem portao a quem alcancar a porta.
        ("x-token", Json::texto_de(&rest.token)),
        ("components", componentes()),
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "rest::testes::a_especificacao_nao_carrega_o_token",
        ],
        "seguem": [
            # O `para_json` do `Rest` continua limpo: e outra serializacao.
            "config::tests::o_token_do_rest_nao_sai_nem_entra_pela_tela",
            "config::tests::nenhuma_credencial_do_config_sai_pela_op_config",
        ],
    },
    {
        "id": "token-remoto-fora-da-lista-de-segredos",
        "titulo": "o `token_remoto` sai da lista de segredos: o token do OUTRO servidor vai em claro para o `perfil.txt` e para a op `profiler`",
        "porque": (
            "petrea do CLAUDE.md: «senha nunca em texto puro: nem em arquivo, nem em log, "
            "nem em resposta do protocolo». Revisao SEC de 17/09/2026, achado A1 "
            "(docs/SEGURANCA.md §17.1): `token_remoto` nasceu em 03/09 com o nome escolhido "
            "de proposito -- `token` e o portao 1 deste servidor -- e a lista, retocada em "
            "05/09, nao o ganhou. O defeito reposto NAO e um `derive`: e o NOME fora da lista, "
            "que e a forma pela qual a lista envelhece. Quatro testes caem, e a diferenca "
            "entre eles e o que a entrada ensina: o do profiler ve o VALOR vazar; o do "
            "`segredos.rs` ve o NOME faltar, cruzando a lista com os parametros do catalogo "
            "-- e e esse que cai ANTES de alguem mandar o campo pelo fio."
            "\n\nRAIO MEDIDO (17/09/2026): 4 dos 1.117 testes do `--lib` caem -- os quatro do `caem`."
        ),
        "arquivo": "crates/phxsql-server/src/segredos.rs",
        "trecho": """    "token_remoto",
    "chave",
""",
        "troca": """    // DEFEITO REPOSTO: o nome escolhido de proposito ficou fora da lista
    // enquanto ela era retocada (05/09/2026).
    "chave",
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "profiler::testes::o_token_do_outro_servidor_nunca_aparece",
            "segredos::testes::todo_parametro_com_cara_de_segredo_esta_na_lista",
            "jobs::testes::credencial_no_pedido_e_recusada_em_qualquer_profundidade",
            "segredos::testes::o_achado_nomeia_o_campo_em_qualquer_profundidade",
        ],
        "seguem": [
            "profiler::testes::a_senha_nunca_aparece",
            "profiler::testes::a_prova_e_a_assinatura_tambem_saem",
            "segredos::testes::a_lista_reconhece_os_seus_nomes_aparados_e_sem_caixa",
            "jobs::testes::pedido_com_token_e_recusado",
        ],
    },
    {
        "id": "job-recusa-um-nome-e-grava-os-outros",
        "titulo": "a guarda do job volta a recusar só `token`: `senha`/`token_remoto` vão para o `jobs.json` e voltam na ficha",
        "porque": (
            "mesma petrea, achado A2 da revisao SEC de 17/09/2026 (docs/SEGURANCA.md §17.2): "
            "«guarda que repoe defeito trava o NOME onde o defeito foi reposto, nao a lei». "
            "O `de_json` recusava `token` e deixava os outros passarem; agora recusa pela "
            "lista da casa (`segredos::achar_segredo`), em qualquer profundidade, e nomeia o "
            "campo. `pedido_com_token_e_recusado` SEGUE verde com o defeito -- e por isso ele "
            "sozinho nunca acusou nada."
            "\n\nRAIO MEDIDO (17/09/2026): 1 dos 1.117 testes do `--lib` cai."
        ),
        "arquivo": "crates/phxsql-server/src/jobs.rs",
        "trecho": """        if let Some(achado) = crate::segredos::achar_segredo(&pedido) {
            return Err(PhxError::Esquema(format!(
                "job {nome:?}: o \\"pedido\\" leva {achado}, e credencial nao entra em job -- \\
                 o cadastro fica em arquivo e volta na ficha. O `token` nao e preciso (o job \\
                 nao entra pela rede; quem manda nele e o usuario configurado); para uma \\
                 ligacao, use `senha_env`/`token_remoto_env` com o nome da variavel de ambiente"
            )));
        }
""",
        "troca": """        // DEFEITO REPOSTO: a guarda trava UM nome, nao a lei.
        if pedido.campo("token").is_some() {
            return Err(PhxError::Esquema(format!(
                "job {nome:?}: o \\"pedido\\" nao leva \\"token\\". O job nao entra pela rede; \\
                 quem manda nele e o usuario configurado"
            )));
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": ["jobs::testes::credencial_no_pedido_e_recusada_em_qualquer_profundidade"],
        "seguem": ["jobs::testes::pedido_com_token_e_recusado", "jobs::testes::o_cadastro_de_jobs_nasce_0600"],
    },
    {
        "id": "config-json-escreve-aberto-e-herda",
        "titulo": "o `config.json` volta a nascer na permissão do `umask` e a herdar o `0644` do original",
        "porque": (
            "achado A4 da revisao SEC de 17/09/2026 (docs/SEGURANCA.md §17.3): «conserto entra "
            "no caminho que o motivou, e o irmao fica». O `gravar_chave` fecha a janela desde "
            "30/08 (`d3b7d62`) e os tres irmaos escreviam aberto e apertavam DEPOIS com "
            "`let _ =`; o `config.json` ainda herdava a permissao do original. A prova e do "
            "sistema operacional: o modo que o `stat` devolve, 644 com o defeito e 600 sem."
            "\n\nRAIO MEDIDO (17/09/2026): 1 dos 1.117 testes do `--lib` cai."
        ),
        "arquivo": "crates/phxsql-server/src/config.rs",
        "trecho": """    gravar_privado(caminho, corpo.as_bytes())
        .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", caminho.display())))?;
    Ok(novo)
""",
        "troca": """    // DEFEITO REPOSTO: escreve na permissao do umask e aperta depois, herdando
    // a do original -- o 0644 de instalacao fica 0644 para sempre.
    let temporario = caminho.with_extension("tmp");
    std::fs::write(&temporario, corpo)
        .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", temporario.display())))?;
    if let Ok(meta) = std::fs::metadata(caminho) {
        let _ = std::fs::set_permissions(&temporario, meta.permissions());
    }
    std::fs::rename(&temporario, caminho)
        .map_err(|e| PhxError::Esquema(format!("nao troquei {}: {e}", caminho.display())))?;
    Ok(novo)
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": ["config::testes_gravacao::o_config_regravado_nasce_0600_sem_herdar_o_original"],
        "seguem": ["config::testes_gravacao::grava_o_pedido_e_preserva_o_resto", "config::testes_gravacao::gravar_privado_nasce_0600_mesmo_com_temporario_velho_aberto"],
    },
    # -----------------------------------------------------------------------
    # 181. «guarda nova entra pedida, nao imposta» -- replicas_autorizadas
    # -----------------------------------------------------------------------
    {
        "id": "replica-lista-e-pedida-nao-imposta",
        "titulo": "replicas_autorizadas vazia libera todos -- e so isso e' pedida, nao imposta",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026 "
            "(onda 1) e fechada na onda 3: o portao 2a-bis (servidor.rs) confere o IP da "
            "sessao contra `replicacao.replicas_autorizadas`, e a bancada de conteiner ja "
            "media o estrago sem ele -- um vizinho de rede com o mesmo token e senha_hash "
            "levando os 200 de 200 eventos do diario COM a lista preenchida. Sem esta guarda "
            "no catalogo, ninguem reprova de novo a cada rodada se o portao continua ali."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if OPS_DE_REPLICACAO.contains(&op) && !sessao.ip.is_empty() {
            let lista = &self.config.replicacao.replicas_autorizadas;
            if !lista.is_empty() && !lista.iter().any(|p| p == &sessao.ip) {
                self.violacao_leve(&sessao.ip, op, "ip fora de replicas_autorizadas");
                return Err(PhxError::Autorizacao(
                    self.msg("erro.replica_nao_autorizada", &[]),
                ));
            }
        }""",
        "troca": """        // DEFEITO REPOSTO: o portao 2a-bis nao confere mais `replicas_autorizadas`
        // -- qualquer IP com token passa, pedida ou nao a lista.""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_papel::replica_de_fora_da_lista_nao_le_o_diario",
        ],
        # O teste que mais importa aqui: o comportamento VELHO. Todo config.json
        # de hoje tem a lista vazia (ou nem tem o campo); se a ausencia da guarda
        # mudasse esse comportamento a replicacao de todo mundo pararia --
        # `sem_replicas_autorizadas_nada_muda` e `caminho_interno_sem_ip_nao_e_barrado_pela_lista`
        # continuam verdes mesmo com o portao arrancado, porque nenhum dos dois
        # exercita o ramo que a lista PREENCHIDA aperta -- e e' exatamente isso
        # que prova que a guarda so aperta quem foi pedido. A terceira nem passa
        # pelo `portoes_do_pedido`: mede so a superficie de configuracao.
        "seguem": [
            "servidor::testes_papel::sem_replicas_autorizadas_nada_muda",
            "servidor::testes_papel::caminho_interno_sem_ip_nao_e_barrado_pela_lista",
            "servidor::testes_papel::a_replicacao_aberta_se_anuncia_e_some_quando_a_lista_enche",
        ],
    },
    # -----------------------------------------------------------------------
    # 182. a posicao do diario nao encolhe em silencio (pedido 211)
    # -----------------------------------------------------------------------
    {
        "id": "posicao-nao-encolhe-em-silencio",
        "titulo": "tabela que nao abre some da soma do diario sem marcar `incompleta`",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026: antes do "
            "pedido 211, um no que nao conseguia abrir uma tabela se declarava mais atrasado "
            "do que era e perdia uma eleicao que deveria vencer, EM SILENCIO -- a posicao "
            "incompleta e sempre menor que a real, e sem a bandeira ninguem sabia por que. "
            "`posicao_do_diario` passou a devolver `(total, incompleta)` em vez de so `u64`."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            let Ok(tabelas) = db.todas_as_tabelas() else {
                incompleta = true;
                continue;
            };
            for t in tabelas {
                match db.abrir_qualificada(&t) {
                    Ok(mut tab) => match tab.eventos() {
                        Ok(n) => total += n,
                        Err(_) => incompleta = true,
                    },
                    Err(_) => incompleta = true,
                }
            }""",
        "troca": """            // DEFEITO REPOSTO: a tabela que nao abre so nao entra na soma --
            // volta ao silencio de antes do pedido 211, sem marcar `incompleta`.
            let Ok(tabelas) = db.todas_as_tabelas() else {
                continue;
            };
            for t in tabelas {
                if let Ok(mut tab) = db.abrir_qualificada(&t) {
                    if let Ok(n) = tab.eventos() {
                        total += n;
                    }
                }
            }""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_posicao_do_diario::tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio",
        ],
    },
    # -----------------------------------------------------------------------
    # 183. a eleicao prefere posicao COMPLETA (pedido 211)
    # -----------------------------------------------------------------------
    {
        "id": "eleicao-prefere-completa",
        "titulo": "`cluster::vencedor` volta a comparar so a posicao numerica, ignorando `incompleta`",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026: uma posicao "
            "INCOMPLETA e sempre menor que a real (o no nao abriu uma tabela replicada), e "
            "promover quem nao a abre poe no comando um master que nao a serve nem a replica. "
            "Sem a preferencia por completa ANTES do numero, um no com mais diario mas "
            "incompleto venceria um no com menos diario e completo."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """    vivos.iter().max_by(|a, b| {
        // `!incompleta`: completa (true) ordena acima de incompleta (false).
        (!a.incompleta)
            .cmp(&(!b.incompleta))
            .then(a.posicao.cmp(&b.posicao))
            .then(a.prioridade.cmp(&b.prioridade))
            // Invertido de proposito: no empate total, o id MENOR ganha.
            .then_with(|| b.id.cmp(&a.id))
    })""",
        "troca": """    vivos.iter().max_by(|a, b| {
        // DEFEITO REPOSTO: ignora `incompleta` e volta a comparar so a posicao.
        a.posicao
            .cmp(&b.posicao)
            .then(a.prioridade.cmp(&b.prioridade))
            .then_with(|| b.id.cmp(&a.id))
    })""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "cluster::testes::eleicao_prefere_completa_a_incompleta",
        ],
        # Comportamento velho: quando ninguem esta incompleto a eleicao continua
        # decidindo so pela maior posicao -- a preferencia nao pode inventar
        # diferenca onde nao ha.
        "seguem": [
            "cluster::testes::com_maioria_vence_a_maior_posicao",
        ],
    },
    # -----------------------------------------------------------------------
    # 184. «replica nao atende escrita» -- portao 2b-bis por PAPEL (REPLICACAO §6)
    # -----------------------------------------------------------------------
    {
        "id": "replica-nao-atende-escrita",
        "titulo": "`aplicar` pela rede deixa de exigir um papel que receba replicacao",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026 (pedido "
            "214(c)): so replica/read_replica/spare/multi recebem `aplicar` pela rede -- "
            "source e isolado, que sao o dado de producao, tem de recusar. `aplicar` fica "
            "FORA de `OPS_ESCRITA` de proposito (grava direto por `Table::aplicar_evento`, "
            "que desliga FK/CHECK/cascata -- a garantia e da origem), entao sem este portao "
            "proprio nao ha NADA que recuse `aplicar` num source ou isolado."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if op == "aplicar" && self.cluster.is_none() {
            let papel = self.papel_atual();
            let recebe_replicacao = matches!(
                papel,
                Papel::Replica | Papel::ReadReplica | Papel::Spare | Papel::Multi
            );
            if !recebe_replicacao {
                return Err(PhxError::Autorizacao(
                    self.msg("erro.aplicar_fora_de_replica", &[("papel", papel.nome())]),
                ));
            }
        }""",
        "troca": """        // DEFEITO REPOSTO: `aplicar` pela rede deixou de exigir um papel que
        // receba replicacao -- source e isolado voltam a aceita-lo.""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_papel::aplicar_num_source_trancado_por_administracao_e_recusado",
            "servidor::testes_papel::aplicar_num_source_aberto_tambem_e_recusado",
        ],
        # Comportamento velho: os quatro papeis que existem para RECEBER
        # replicacao continuam aceitando `aplicar`, trancados ou nao -- o crivo
        # e sobre o PAPEL, nao sobre `somente_leitura` (isso e' o A3, ja fixado
        # a parte em 17/09/2026 e coberto pelos proprios testes do papel B).
        "seguem": [
            "servidor::testes_papel::replica_trancada_continua_aceitando_o_diario_do_source",
            "servidor::testes_papel::replica_destrancada_continua_aceitando_o_diario_do_source",
        ],
    },
    # -----------------------------------------------------------------------
    # 185. `spare` nao atende ninguem (modo C, pedido 214)
    # -----------------------------------------------------------------------
    {
        "id": "spare-nao-atende-ninguem",
        "titulo": "o papel Spare deixa de recusar toda operacao que nao esta em OPS_NO_SPARE",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026 (modo C, "
            "pedido 214): um spare de contingencia nao atende cliente NEM DE LEITURA ate ser "
            "promovido -- servir leitura de um spare que ainda nao aplicou o ultimo lote "
            "devolveria dado velho sem avisar. So administracao, monitoramento e a propria "
            "replicacao (`OPS_NO_SPARE`) passam."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            Papel::Spare if !OPS_NO_SPARE.contains(&op) => {
                return Err(PhxError::SpareEmEspera(format!(
                    "este servidor e um spare de contingencia e nao atende \\
                     cliente (nem leitura); o primario e {}. Para assumir o \\
                     trabalho: {{\\"op\\":\\"spare_promover\\"}}",
                    self.primario()
                )));
            }""",
        "troca": """            // DEFEITO REPOSTO: o papel Spare deixou de recusar operacao nenhuma
            // -- cai direto no `_ => {}` de baixo, como qualquer outro papel.""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_papel::spare_nao_atende_cliente_nem_de_leitura",
        ],
        # A promocao continua abrindo a escrita normalmente -- o defeito e so
        # no ramo do papel Spare, e o teste de outro papel (ReadReplica) prova
        # que a mutacao nao vazou para o resto do match.
        "seguem": [
            "servidor::testes_papel::spare_promover_vira_primario_e_abre_a_escrita",
            "servidor::testes_papel::read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura",
        ],
    },
    # -----------------------------------------------------------------------
    # 186. read replica recusa escrita apontando o primario (modo D, pedido 214)
    # -----------------------------------------------------------------------
    {
        "id": "read-replica-recusa-escrita",
        "titulo": "`ReadReplica` deixa de recusar escrita e para de apontar o primario",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026 (modo D, "
            "pedido 214): uma read replica serve leitura sem fim, e escrita nela tem de "
            "devolver REDIRECIONA com o endereco do primario -- o mesmo evento que o "
            "cluster usa para mandar o cliente para o no certo em vez de so recusar."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """            Papel::ReadReplica if OPS_ESCRITA.contains(&op) => {
                return Err(PhxError::Redireciona(format!(
                    "REDIRECIONA {} -- este servidor e uma replica de leitura; \\
                     escreva no primario",
                    self.primario()
                )));
            }""",
        "troca": """            // DEFEITO REPOSTO: ReadReplica deixou de recusar escrita -- cai
            // direto no `_ => {}` de baixo, como qualquer papel que grava.""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_papel::read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura",
        ],
        # O ramo do Spare, logo acima no mesmo match, continua intacto -- prova
        # que a mutacao nao derrubou o match inteiro.
        "seguem": [
            "servidor::testes_papel::spare_nao_atende_cliente_nem_de_leitura",
        ],
    },
    # -----------------------------------------------------------------------
    # 187. o pulso de id fora da lista e recusado (pedido 217)
    # -----------------------------------------------------------------------
    {
        "id": "pulso-fora-da-lista-e-recusado",
        "titulo": "`op_cluster_pulso` deixa de conferir o id contra a lista viva de nos",
        "porque": (
            "petrea sem guarda no catalogo, achada no inventario QA de 17/09/2026 (pedido "
            "217): aceitar um id de no desconhecido no pulso inflaria o denominador da "
            "maioria com nos fantasmas e travaria toda promocao -- e e' o mesmo teste que "
            "prova a outra ponta, que um no ACRESCENTADO a quente (`cluster_no_acrescentar`) "
            "passa a ser aceito no pulso seguinte SEM reiniciar nada."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        let no = match self.no(id) {
            Some(no) if id != self.config.id => no,
            _ => {
                return Err(RecusaDoPulso::ForaDaLista {
                    e_este_no: id == self.config.id,
                })
            }
        };
""",
        # REANCORADO em 24/09/2026 (pedido 441): o crivo da lista saiu do
        # `op_cluster_pulso` e entrou no MOTOR (`conferir_identidade`), para
        # valer tambem na RESPOSTA do pulso. O defeito e o mesmo -- a lista
        # viva nao e consultada, o id proprio continua recusado --, escrito no
        # lugar novo: sem a lista nao ha `no`, entao o pulso sem prova volta
        # antes dela, que e exatamente o que o motor fazia antes do 441.
        #
        # REANCORADO em 17/09/2026 (onda 3): a frente B2 fundiu, no MESMO dia,
        # as duas recusas do pulso («fora da lista» e «e' este servidor») numa
        # so condicao (revisao SEC, A11 -- duas frases distintas faziam do
        # pulso um oraculo de ids). O defeito que esta guarda prova continua
        # existindo -- so a linha da condicao mudou; a mutacao agora tira so a
        # metade `estado.no(&id).is_none()`, preservando a conferencia de
        # id-duplicado que a linha tambem carrega.
        "troca": """        // DEFEITO REPOSTO: nao confere mais se o id esta na lista viva de
        // nos -- so continua recusando quando o id e' o proprio.
        if id == self.config.id {
            return Err(RecusaDoPulso::ForaDaLista { e_este_no: true });
        }
        if pedido.texto_ou("prova", "").trim().is_empty()
            && !self.config.exigir_prova_do_pulso
            && !self.ja_provou(id)
        {
            return Ok(Identidade::SemProva);
        }
        let no = match self.no(id) {
            Some(no) if id != self.config.id => no,
            _ => {
                return Err(RecusaDoPulso::ForaDaLista {
                    e_este_no: id == self.config.id,
                })
            }
        };
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_config_gravar::no_acrescentado_a_quente_passa_a_ser_aceito_no_pulso",
        ],
        "seguem": [
            "servidor::testes_config_gravar::origem_do_cluster_carrega_a_cifra_e_o_pino",
        ],
    },
    # -----------------------------------------------------------------------
    # 188. a chave duplicada no unico SECUNDARIO prendia o laco (pedido 292)
    # -----------------------------------------------------------------------
    {
        "id": "laco-preso-no-unico-secundario",
        "titulo": "chave duplicada num índice único secundário prende o laço do bidirecional para sempre",
        "porque": (
            "parecer do papel C 2.5/6.4, pedido 292 -- o bidirecional casa as "
            "linhas por UMA chave e a unicidade dos outros indices continua "
            "valendo na gravacao. Com primaria porId e um secundario porEmail, "
            "o evento do outro lado com e-mail repetido era recusado, o Err "
            "subia pelo `?` e o `desde = lote.ate` nunca executava: o mesmo "
            "lote voltava para sempre. Nao e linha perdida, e o par de "
            "servidores parado, sem ninguem saber. O DEFEITO e o mesmo desde "
            "17/09/2026; o que mudou foi a CURA: a parte (2) contava e seguia, "
            "e a parte (1) -- depois de a regua dos motores maduros derrubar a "
            "recusa na declaracao -- para o par naquela tabela, MARCADO, com o "
            "indice, o valor da chave e as duas linhas, e a saida e humana "
            "(`replicacao_pular`). Por isso os testes que caem mudaram de nome."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Err(PhxError::Duplicado(qual)) = &escrita {""",
        # O padrao nunca casa (`false` contra `true`), entao o bloco inteiro
        # fica inalcancavel e o `escrita?;` logo abaixo volta a subir o erro --
        # que e exatamente o defeito de origem, sem apagar linha nenhuma.
        "troca": """        // DEFEITO REPOSTO: a recusa volta a subir pelo `?` de quem chama.
        if let (Err(PhxError::Duplicado(qual)), false) = (&escrita, true) {""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "laco-do-unico-secundario"],
        "caem": [
            "o_conflito_de_unicidade_para_o_par_marcado",
            "o_pular_manual_solta_o_par_e_a_linha_seguinte_chega",
            "a_coluna_marcada_nao_vaza_no_grito_do_conflito",
        ],
        # O teste do comportamento VELHO. Sem ele, um "conserto" que parasse de
        # replicar passaria com louvor no de cima: laco que nao aplica nada
        # tambem nao fica preso em lote nenhum.
        "seguem": [
            "sem_colisao_o_laco_replica_como_sempre_e_nada_e_contado",
        ],
        # Medido: 20,2 s com o defeito reposto (o teste espera 20 s pelo laco
        # que nunca anda) contra 2,1 s com a arvore limpa. O prazo tem de ser
        # maior que a soma com o arranque, senao o executor mata a rodada antes
        # de o teste conseguir reprovar -- a licao do `trava-atras-da-rede`.
        "prazo": 120,
    },
    # -----------------------------------------------------------------------
    # 188b. o par PARADO reapresentado a cada rodada (pedido 292, parte 1)
    # -----------------------------------------------------------------------
    {
        "id": "par-parado-reapresentado-a-cada-rodada",
        "titulo": "a tabela parada por conflito volta a ser puxada a cada rodada, e o grito se repete para sempre",
        "porque": (
            "pedido 292 parte (1), item 4. O PostgreSQL fica em laco por "
            "decisao escrita no fonte (`worker.c`: nao avancar a origem e o "
            "que impede perder a transacao) e paga o preco ESTRANGULANDO "
            "(`launcher.c`: uma tentativa por `wal_retrieve_retry_interval`, "
            "5 s). Aqui o conflito nao avanca a posicao pelo mesmo motivo, "
            "entao o estrangulamento tinha de existir -- e ele e o PORTAO que "
            "vem ANTES do trabalho: a tabela parada sai do alcance sem tomar a "
            "trava de dados, sem absorver o diario local e sem uma ida e volta "
            "de rede. MEDIDO pelo soquete, contando os `replicar` servidos pelo "
            "parceiro em 10 s com o par parado: 0 com o portao, 10 sem ele "
            "(uma por segundo, o `reconectar_em` do cenario) -- e as recusas "
            "contadas do lado de ca subiram de 2 para 12 no mesmo intervalo, "
            "ou seja, uma linha de log por segundo para sempre."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if self.esta_parada(&origem.nome, &chave_tab) {""",
        "troca": """        // DEFEITO REPOSTO: o portao sai, e a tabela parada volta a ser
        // puxada a cada rodada -- rede, trava e grito, sem nada mudar.
        if false && self.esta_parada(&origem.nome, &chave_tab) {""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "laco-do-unico-secundario"],
        "caem": [
            "o_conflito_de_unicidade_para_o_par_marcado",
        ],
        # O comportamento VELHO: o par que nao colide nunca chega perto do
        # portao, e continua replicando igual. Se ele cair, o defeito reposto
        # foi longe demais.
        "seguem": [
            "sem_colisao_o_laco_replica_como_sempre_e_nada_e_contado",
        ],
        "prazo": 120,
    },
    # -----------------------------------------------------------------------
    # 188c. o grito do conflito publicando a coluna marcada (pedido 292/1)
    # -----------------------------------------------------------------------
    {
        "id": "dado-pessoal-no-grito-do-conflito",
        "titulo": "o grito do conflito de unicidade publica a coluna marcada como dado pessoal",
        "porque": (
            "petrea da casa -- dado pessoal e senha nunca em log. O grito do "
            "292 parte (1) carrega o que o PostgreSQL carrega (indice, valor "
            "da chave, linha local e linha remota), e o PostgreSQL imprime a "
            "chave crua (`Key (c)=(1)`) porque nao tem marca de dado pessoal "
            "no esquema. Nos temos: uma primaria de CPF ou um unico de e-mail "
            "sairiam no `replicacao_estado` E no diario do processo se "
            "copiassemos o comportamento dele. A redacao e por ANALISE -- cada "
            "coluna decidida pelo que o esquema diz dela --, e o que nao se "
            "analisa vira o tamanho em bytes."
        ),
        "arquivo": "crates/phxsql-server/src/bidirecional.rs",
        "trecho": """    if coluna.dado_pessoal.e_pessoal() {""",
        "troca": """    // DEFEITO REPOSTO: a coluna marcada sai por extenso no grito.
    if false && coluna.dado_pessoal.e_pessoal() {""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "laco-do-unico-secundario"],
        "caem": [
            "a_coluna_marcada_nao_vaza_no_grito_do_conflito",
        ],
        # O comportamento VELHO: a coluna NAO marcada continua legivel. Um
        # "conserto" que redigisse tudo tornaria o grito inutil, e passaria no
        # de cima com louvor.
        "seguem": [
            "o_conflito_de_unicidade_para_o_par_marcado",
        ],
        "prazo": 120,
    },
    # 189. O empilhar abria a porta de sempre so para desligar a sobreposicao
    # -----------------------------------------------------------------------
    {
        "id": "so-o-disco-vem-da-porta-e-nao-de-desligar-depois",
        "titulo": "o empilhar volta a abrir pela porta de sempre e desligar a sobreposicao na linha seguinte",
        "porque": (
            "pedido 164, commit 20d2c59 -- o caminho que EMPILHA abria a tabela "
            "pela porta que monta a sobreposicao da transacao (percorrendo o "
            "conjunto de escrita INTEIRO) e chamava `ver_so_o_disco()` na linha "
            "seguinte, jogando o mapa fora sem ninguem consultar: O(pendentes) "
            "por operacao, O(n^2) por transacao, DENTRO da trava de dados. "
            "Medido sob a trava com 1.600 escritas pendentes (`--example "
            "reparticao-do-gatilho`): 625,62 -> 40,62 us/op, 15,4x, curva plana "
            "(a razao de MIL pendentes e 8,6x -- 335-341 para 39-40 --, e o "
            "commit publicou essa por engano no lugar da de 1.600). O defeito e "
            "so de TEMPO: as duas formas deixam o `Table` no MESMO estado, entao "
            "so um contador de chamadas no fonte -- e nao um teste de resultado "
            "-- consegue pega-lo."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        # A ancora leva os tres comentarios junto porque a chamada sozinha
        # aparece DUAS vezes no arquivo (o irmao da cascata). Com o comentario,
        # ocorre uma so -- conferido antes de gravar.
        "trecho": """        // esta no `abrir_travada_sem_sobrepor` -- e ele virou porta porque
        // abrir pela de sempre e desligar depois montava o mapa da transacao
        // sob a trava para apaga-lo na linha seguinte.
        let mut t = self.abrir_travada_sem_sobrepor(&trava, p, sessao)?;
""",
        "troca": """        // DEFEITO REPOSTO: o empilhar volta a abrir pela porta de sempre e a
        // desligar a sobreposicao na linha seguinte -- o mapa da transacao
        // nasce e morre sob a trava de dados, sem ninguem consultar.
        let mut t = self.abrir_travada(&trava, p, sessao)?;
        t.ver_so_o_disco();
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_janela_e_cadeia::so_o_disco_vem_da_porta_e_nao_de_desligar_depois",
        ],
        # O comportamento VELHO: o upsert continua decidindo contra o disco do
        # mesmo jeito, porque a porta e a dispensa nao mudam o ESTADO da tabela,
        # so o custo. Se ele cair, o defeito reposto foi longe demais.
        "seguem": [
            "servidor::testes_transacoes::dentro_da_transacao_o_upsert_decide_contra_o_disco",
        ],
    },
    # 190. so o sal separava dois `.reg` cifrados, e ninguem tinha escrito isso
    # -----------------------------------------------------------------------
    # Pedido 316. A guarda existia por CONSEQUENCIA: nenhuma amarracao do slot
    # carrega identidade de arquivo -- o `aad_do_slot` e (volume, rowid,
    # versao), o `rotulo_da_prova` e (MAGIC_REG, versao, slot_size) e o tempero
    # do nonce viaja DENTRO do slot, entao viaja junto na copia. Quem separa os
    # dois arquivos e so a chave, e a chave so difere porque `Material::novo()`
    # sorteia um sal por arquivo. Consequencia que ninguem escreveu e a que
    # alguem apaga sem ver.
    {
        "id": "slot-de-outro-reg",
        "titulo": "o sal deixa de ser por arquivo: o slot cifrado de um `.reg` abre no outro",
        "porque": (
            "achado do papel SEC (17/09/2026, parecer A2), que o provou no "
            "nivel da PRIMITIVA -- com o sal de hoje e recusado, com sal "
            "compartilhado abre limpo. Faltava a prova sobre dois arquivos de "
            "verdade, e faltava a catraca: o comentario de `aad_do_slot` "
            "promete impedir a copia da linha 7 sobre a 9 «ou de outro "
            "volume», e nao diz «de outro arquivo» porque quem cobre esse "
            "caso e o sal. Medido com o defeito reposto: a linha 1 de `b` "
            "devolve `Ok(Some([Int(1), Str(\"Alice de Origem\"), ...]))` -- o "
            "conteudo do OUTRO arquivo, sem erro nenhum."
        ),
        "arquivo": "crates/phxsql-store/src/cofre.rs",
        # A ancora leva a linha do `derivar` junto porque o par
        # `let mut sal` + `copy_from_slice` aparece DUAS vezes no arquivo (o
        # irmao e o cabecalho de volume dos diarios). Com o `"<arquivo novo>"`
        # ocorre uma so -- conferido antes de gravar.
        "trecho": """        let mut sal = [0u8; SAL_LEN];
        sal.copy_from_slice(&phxsql_core::senha::bytes_aleatorios(SAL_LEN));
        let iteracoes = iteracoes_vigentes()?;
        let chave = derivar(&sal, iteracoes, "<arquivo novo>")?;
""",
        "troca": """        // DEFEITO REPOSTO: o sal deixa de ser sorteado POR ARQUIVO -- dois
        // `.reg` passam a derivar a MESMA chave, e o slot de um abre no outro.
        let sal = [0x5Au8; SAL_LEN];
        let iteracoes = iteracoes_vigentes()?;
        let chave = derivar(&sal, iteracoes, "<arquivo novo>")?;
""",
        "pacote": "phxsql-store",
        "alvo": ["--test", "cifra-dos-dados"],
        # Os dois caem, e cada um prova uma metade: o primeiro e a PREMISSA (o
        # sal e por arquivo) e o segundo e a GUARDA (o slot de fora nao abre).
        # Eles sao dois testes e nao um porque a premissa conferida dentro do
        # teste da guarda dispararia ANTES do transplante -- e ai a guarda
        # cairia sem ter provado que o slot de fora abre.
        "caem": [
            "dois_reg_novos_nascem_com_sais_diferentes",
            "transplantar_slot_entre_dois_reg_e_recusado",
        ],
        # O que tem de continuar de pe, e por que cada um: a amarracao DENTRO
        # do arquivo nao foi tocada (o slot 5 continua sem abrir no 9), a senha
        # continua mandando na chave (sal fixo nao e chave fixa), o nonce
        # continua sem repetir e a cifra continua indo e voltando. Se algum
        # destes cair, a troca quebrou mais do que devia e a guarda nao esta
        # provada -- seria um teste caindo por outro motivo.
        "seguem": [
            "trocar_o_corpo_de_uma_linha_pela_outra_nao_passa",
            "senha_errada_e_falta_de_senha_param_na_abertura",
            "regravar_a_mesma_linha_nunca_repete_o_texto_cifrado",
            "cifrada_a_tabela_funciona_igual",
        ],
        # Medido: 9,85 s com o defeito reposto e 9,81 s com a arvore limpa, com
        # `--test-threads=1` (o PBKDF2 no piso e a trava do processo dominam).
        "prazo": 120,
    },
    # 27. A identidade de quem manda o pulso do cluster -- pedido 278 (SEC A1)
    # -----------------------------------------------------------------------
    {
        "id": "pulso-sem-prova-de-identidade",
        "titulo": "o pulso do cluster aceitando identidade auto-declarada",
        "porque": (
            "a credencial da replicacao e UMA so para o cluster inteiro, entao "
            "quem a tivesse -- um no legitimo inclusive -- se declarava OUTRO no "
            "e mandava a epoca que quisesse. Medido pelo soquete antes do "
            "conserto: uma linha com id:noB, papel:master, epoca:9 rebaixou o "
            "master de verdade em 0,53 s e gravou {papel:replica,epoca:9} no "
            "cluster.estado.json, que ganha do config.json no arranque. O teto "
            "de epoca (FOLGA_DE_EPOCA) cobria o sintoma; a prova dentro do "
            "pulso cobre a causa. Reposto o defeito, o forjado passa de novo, o "
            "repetido conta duas vezes e o TOFU para de morder."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        # REANCORADO em 24/09/2026 (pedido 441): a chamada virou um `match`
        # sobre a recusa do motor, que agora traz o crivo da lista junto. O
        # defeito continua o de origem -- o motor nao e chamado --, e o
        # `Provada` fingido e o que o pulso de antes fazia: a resposta saia
        # assinada para quem tivesse pino.
        "trecho": """        let identidade = match estado.conferir_identidade(
            &id,
            &pulso,
            p,
            sessao.transcricao_do_fio.as_ref().map(|t| &t[..]),
            || self.estatica_do_fio(),
        ) {
            Ok(identidade) => identidade,
            Err(crate::cluster::RecusaDoPulso::ForaDaLista { e_este_no }) => {
                if e_este_no {
                    eprintln!(
                        "cluster: pulso com o id DESTE servidor ({id}) vindo de {:?} -- \\
                         dois nos com o mesmo id no ar?",
                        sessao.ip
                    );
                }
                if self.config.politica.contar_pulso_desconhecido && !sessao.ip.is_empty() {
                    self.violacao_leve(
                        &sessao.ip,
                        "cluster_pulso",
                        "pulso com id que nao e um no deste cluster",
                    );
                }
                return Err(PhxError::Autorizacao(
                    self.msg("erro.pulso_de_no_desconhecido", &[("id", &id)]),
                ));
            }
            Err(crate::cluster::RecusaDoPulso::Outra(e)) => return Err(e),
        };
        estado.registrar(&id, pulso);
""",
        "troca": """        // DEFEITO REPOSTO: o pulso entra sem provar quem o mandou, e o `id`
        // do corpo vale como identidade -- o A1 do pedido 278.
        let identidade = crate::cluster::Identidade::Provada;
        estado.registrar(&id, pulso);
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "um_pulso_forjado_nao_destrona_o_master",
            "um_pulso_repetido_nao_conta_duas_vezes",
            "depois_que_o_no_provou_o_pulso_sem_prova_e_recusado",
        ],
        # O que tem de CONTINUAR de pe: a guarda e pedida, entao o no de versao
        # anterior nao pode passar a ser recusado por causa dela; e o caminho
        # legitimo -- o par cifrado com a exigencia ligada -- so depende da
        # prova, que continua sendo montada no `pulsar`.
        "seguem": [
            "sem_exigencia_o_no_de_versao_anterior_continua_pulsando",
            "o_pulso_com_prova_valida_passa_e_conta",
        ],
        "prazo": 120,
    },
    # 28. O aperto de mao lendo sem teto -- pedido 312
    # -----------------------------------------------------------------------
    {
        "id": "aperto-de-mao-sem-teto",
        "titulo": "a leitura do aperto de mao fora do `Canal`, sem teto nenhum",
        "porque": (
            "o aperto acontece antes de o outro lado se identificar, e os dois "
            "clientes desta casa liam a resposta com `read_line` cru: o "
            "TETO_DO_REGISTRO mora no Canal, que so nascia DEPOIS do aperto. "
            "Medido: um source falso que nunca manda o fim de linha fez a "
            "replica guardar 192 MiB numa linha so, em 294 ms -- 1,5x o teto do "
            "registro, que e a prova de que aquele teto nao alcancava ali. "
            "Reposto o defeito, o erro que volta nem e limite: e o reset da "
            "conexao depois de a memoria ja ter sido reservada."
        ),
        "arquivo": "crates/phxsql-server/src/replica.rs",
        "trecho": """        let resposta = match self
            .canal
            .ler_ate(&mut self.leitor, phxsql_core::fio::TETO_DO_APERTO)?
        {
            Recebido::Linha(l) => l,
            Recebido::Fim => {
                return Err(PhxError::Io(std::io::Error::other(
                    "o source fechou a conexao no aperto de mao",
                )))
            }
        };
""",
        "troca": """        // DEFEITO REPOSTO: leitura crua, sem teto -- quem decide quanta
        // memoria este lado reserva e o outro lado da conexao (pedido 312).
        let mut resposta = String::new();
        {
            use std::io::BufRead as _;
            if self.leitor.read_line(&mut resposta)? == 0 {
                return Err(PhxError::Io(std::io::Error::other(
                    "o source fechou a conexao no aperto de mao",
                )));
            }
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "replica::testes_do_teto_do_aperto::o_aperto_de_mao_recusa_a_linha_sem_fim",
        ],
        # O comportamento VELHO nao pode cair junto: uma resposta de aperto do
        # tamanho de sempre continua sendo lida e analisada dos dois jeitos.
        "seguem": [
            "replica::testes_do_teto_do_aperto::resposta_curta_do_source_continua_passando",
        ],
        "prazo": 300,
    },
    # 29. O erro do pulso publicando o mapa de quais nos nao tem pino -- 435
    # -----------------------------------------------------------------------
    {
        "id": "erro-do-pulso-mapeia-quem-nao-tem-pino",
        "titulo": "a recusa da prova do pulso dizendo quais nós ainda não têm pino",
        "porque": (
            "achado SEC A2 de 23/09/2026. A prova invalida devolvia DUAS "
            "frases: «cluster.nos[X].chave_do_fio esta vazio neste no» quando X "
            "nao tem pino, e «a prova de X nao fecha» quando tem. E «X nao tem "
            "pino» e o mesmo que «X nunca entra no `provaram`, logo um pulso "
            "SEM prova dizendo-se X continua passando»: a mensagem do conserto "
            "do 278 enumerava para o atacante onde o 278 NAO pega. O bit nao "
            "esta no veredito -- os dois casos recusam --, entao o texto era o "
            "unico canal, e por isso a guarda compara os dois textos do FIO em "
            "vez de conferir um veredito. Medido na mesma rodada: colapsar so a "
            "frase comprava ZERO, porque o campo `ms` da resposta separava os "
            "dois em 40 de 40 corridas (1 ms contra 0) -- o caminho sem pino "
            "voltava antes do X25519. Com o pino cego, 0 a 1 de 40. Repor o "
            "defeito e voltar a recusar ANTES do `pulso::conferir`, o que "
            "reabre os dois canais de uma vez."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        let publica = pino.unwrap_or(phxsql_core::x25519::BASE);
""",
        "troca": """        // DEFEITO REPOSTO (435): o no SEM pino volta antes do X25519 e do
        // HMAC, com a frase que nomeia o `chave_do_fio` vazio. Volta o mapa
        // pelo texto E pelo relogio, que e como o achado A2 o encontrou.
        let Some(publica) = pino else {
            return Err(RecusaDoPulso::Outra(PhxError::Autorizacao(format!(
                "o pulso de {id:?} traz prova, mas cluster.nos[{id}].chave_do_fio \
                 esta vazio neste no: sem a chave publica dele nao ha como \
                 conferir. Preencha o pino ou tire a prova do outro lado"
            ))));
        };
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "o_pulso_nao_diz_quais_nos_tem_pino",
        ],
        # O 278 inteiro tem de seguir de pe: fechar o oraculo nao pode ter
        # custado a recusa do pulso forjado nem a passagem do legitimo.
        "seguem": [
            "um_pulso_forjado_nao_destrona_o_master",
            "o_pulso_com_prova_valida_passa_e_conta",
        ],
        "prazo": 120,
    },
    # 30. O pino cego do 435 sem a recusa incondicional -- a volta do 435
    # -----------------------------------------------------------------------
    {
        "id": "pino-cego-sem-a-recusa-do-no-sem-pino",
        "titulo": "a forja contra o pino cego entrando pelo nó sem pino",
        "porque": (
            "o pino cego do 435 (`x25519::BASE` no lugar do pino ausente) fecha "
            "o canal do relogio -- `ms` 1 contra 0 em 40 de 40 -- e abre um "
            "caminho que nao existia: `segredo(minha, BASE)` e a MINHA publica, "
            "que todo par do cluster tem como `chave_do_fio`, entao a prova "
            "conferida contra o ponto-base FECHA para qualquer membro, sem "
            "privada nenhuma. Achado do integrador na revisao do 435, e o "
            "comentario do conserto chamava a recusa de «dia impossivel». A "
            "recusa incondicional do no sem pino, depois do `conferir`, e a "
            "garantia INTEIRA; apagada, a forja e aceita E marca o no provado "
            "(o legitimo, que nao sabe provar, passa a ser recusado pelo TOFU). "
            "O teste do texto do fio NAO pega: o intruso dele assina com a "
            "chave errada, e nao com a derivada do ponto-base."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        if pino.is_none() {
            return Err(self.recusa_da_prova(
                id,
                &format!(
                    "cluster.nos[{id}].chave_do_fio esta vazio neste no: sem a \\
                     chave publica dele nao ha como conferir. Preencha o pino \\
                     ou tire a prova do outro lado"
                ),
            ));
        }
""",
        "troca": """        // DEFEITO REPOSTO (435, volta): a recusa do no sem pino apagada --
        // a forja contra o ponto-base passa no `conferir` e segue ate o
        // `marcar_provado`.
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "cluster::testes::forja_contra_o_pino_cego_nao_entra_nem_marca_provado",
        ],
        # A prova LEGITIMA e a do terceiro com a chave errada nao dependem da
        # linha: seguem verdes, e mostram que o defeito e so a porta do no sem
        # pino, e nao um estrago na conferencia inteira.
        "seguem": [
            "pulso::testes::o_que_a_assina_b_confere",
            "pulso::testes::um_terceiro_no_nao_consegue_se_passar_por_a",
        ],
        "prazo": 300,
    },
    # 31. O nonce do pulso sem regua de BYTES -- pedido 436 (SEC M1)
    # -----------------------------------------------------------------------
    {
        "id": "nonce-do-pulso-sem-regua-de-bytes",
        "titulo": "o nonce do pulso retido do tamanho que o remetente escolheu",
        "porque": (
            "achado SEC M1 de 23/09/2026. `NONCES_POR_NO = 512` e teto de "
            "CONTAGEM: o que a antirrepeticao guarda e o texto do fio, e o "
            "tamanho de cada nonce continuava de quem manda, ate o teto da "
            "linha -- a licao do 312 pela metade, dentro do proprio 278. E o "
            "primeiro lugar da base em que um nonce do fio e RETIDO. A regua "
            "nao e nova: e o `desafio::NONCE_LEN` em hexadecimal, o que o "
            "`pulso::nonce()` sorteia. Os dois testes medem o DANO (bytes na "
            "fila), e nao o veredito; o de `cluster` entra pelo "
            "`conferir_identidade` com um pulso ASSINADO pelo par, que e o "
            "insider do achado. Medido com o defeito reposto: a fila vai de "
            "32 para 1.048.608 bytes."
        ),
        "arquivo": "crates/phxsql-server/src/pulso.rs",
        "trecho": """        if !nonce_no_formato(nonce) {
            return Err(PhxError::Autorizacao(format!(
                "o pulso de {de:?} traz nonce fora do formato ({} bytes): o \\
                 nonce do pulso tem {NONCE_HEX} caracteres hexadecimais \\
                 minusculos, e sem ele a prova serviria duas vezes",
                nonce.len()
            )));
        }
""",
        "troca": """        // DEFEITO REPOSTO (436, M1): so o vazio e recusado -- o tamanho do
        // nonce volta a ser de quem manda.
        if nonce.trim().is_empty() {
            return Err(PhxError::Autorizacao(format!(
                "o pulso de {de:?} veio sem nonce: sem ele a prova serve \\
                 duas vezes"
            )));
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "pulso::testes::o_nonce_fora_do_formato_nao_fica_guardado",
            "cluster::testes::o_nonce_gigante_de_um_par_com_a_chave_nao_fica_guardado",
        ],
        # O emissor de verdade e a repeticao seguem: a regua recusa o torto,
        # e nao o nonce que o proprio cluster manda.
        "seguem": [
            "pulso::testes::o_nonce_do_emissor_passa_no_crivo",
            "pulso::testes::o_mesmo_nonce_nao_conta_duas_vezes",
        ],
        "prazo": 300,
    },
    # 32. A antirrepeticao envenenada virando «pulso inedito» -- 436 (SEC M2)
    # -----------------------------------------------------------------------
    {
        "id": "antirrepeticao-envenenada-vira-pulso-inedito",
        "titulo": "a antirrepetição do pulso desligada, calada, por uma trava envenenada",
        "porque": (
            "achado SEC M2 de 23/09/2026. `let Ok(mut v) = vistos.lock() else "
            "{ return Ok(()) }`: trava envenenada = «pulso fresco e inedito». "
            "Veneno de `Mutex` e permanente, entao um panico com a trava na mao "
            "desligava a antirrepeticao para o resto da vida do processo, sem "
            "uma linha de log. O teste do dano mede o que o achado descreve -- "
            "o MESMO nonce aceito de novo depois do veneno --, e o do stderr "
            "reexecuta o binario e conta a linha do aviso."
        ),
        "arquivo": "crates/phxsql-server/src/pulso.rs",
        "trecho": """        let mut v = self.vistos.travar();
""",
        "troca": """        // DEFEITO REPOSTO (436, M2): trava envenenada = «pulso fresco e
        // inedito», calado.
        let Ok(mut v) = self.vistos.trava.lock() else {
            return Ok(());
        };
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "pulso::testes::trava_envenenada_nao_aceita_nonce_repetido",
            "pulso::testes::trava_envenenada_nao_passa_calada",
        ],
        "seguem": [
            "pulso::testes::o_mesmo_nonce_nao_conta_duas_vezes",
        ],
        "prazo": 300,
    },
    # 33. A trava da guarda recuperada CALADA -- 436 (SEC M2)
    # -----------------------------------------------------------------------
    {
        "id": "trava-da-guarda-recupera-calada",
        "titulo": "a trava envenenada da guarda recuperada sem dizer nada",
        "porque": (
            "a metade do M2 que o teste do dano NAO pega: recuperar o estado "
            "pelo `into_inner` sem o aviso -- o precedente do `semaforo.rs`, "
            "que ali e certo e aqui nao, porque esta e trava de SEGURANCA e "
            "quem faz menos do que promete tem de dizer que fez menos. Com a "
            "troca, os dois testes de dano seguem verdes (a guarda continua "
            "valendo) e so o do stderr cai: e ele que carrega a outra metade."
        ),
        "arquivo": "crates/phxsql-server/src/pulso.rs",
        "trecho": """        self.trava.lock().unwrap_or_else(|veneno| {
            if !self.veneno_dito.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "cluster: a trava {} estava ENVENENADA por um panico em \\
                     outra thread -- estado recuperado, e segue valendo o que \\
                     ja estava anotado nela. O panico esta acima deste aviso \\
                     no log; este aviso sai uma vez por trava, e nao a cada \\
                     pulso",
                    self.nome
                );
            }
            veneno.into_inner()
        })
""",
        "troca": """        // DEFEITO REPOSTO (436, M2): recupera, e cala.
        self.trava.lock().unwrap_or_else(|veneno| veneno.into_inner())
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "pulso::testes::trava_envenenada_nao_passa_calada",
        ],
        "seguem": [
            "pulso::testes::trava_envenenada_nao_aceita_nonce_repetido",
            "cluster::testes::o_tofu_com_a_trava_envenenada_continua_recusando",
        ],
        "prazo": 300,
    },
    # 34. O TOFU envenenado virando «nunca provou» -- 436 (SEC M2)
    # -----------------------------------------------------------------------
    {
        "id": "tofu-envenenado-vira-nunca-provou",
        "titulo": "o TOFU do pulso desligado, calado, por uma trava envenenada",
        "porque": (
            "achado SEC M2 de 23/09/2026, a segunda trava: "
            "`provaram.lock().map(|p| p.contains(id)).unwrap_or(false)` -- "
            "trava envenenada = «este no nunca provou», e todo par volta a ser "
            "ouvido sem prova. O tipo `TravaDaGuarda` fechou a porta (o `Mutex` "
            "de dentro e privado de `pulso.rs`), entao repor o defeito pede "
            "DUAS trocas: reabrir um `lock` que devolve `None` no veneno, e "
            "usa-lo no `ja_provou` com o `unwrap_or(false)` de origem."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-server/src/pulso.rs",
                "trecho": """    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
""",
                "troca": """    /// DEFEITO REPOSTO (436, M2): a porta que o tipo fechou, reaberta.
    pub(crate) fn tentar(&self) -> Option<MutexGuard<'_, T>> {
        self.trava.lock().ok()
    }

    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
""",
            },
            {
                "arquivo": "crates/phxsql-server/src/cluster.rs",
                "trecho": """        self.provaram.travar().contains(id)
""",
                "troca": """        self.provaram.tentar().map(|p| p.contains(id)).unwrap_or(false)
""",
            },
        ],
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "cluster::testes::o_tofu_com_a_trava_envenenada_continua_recusando",
        ],
        "seguem": [
            "cluster::testes::forja_contra_o_pino_cego_nao_entra_nem_marca_provado",
            "pulso::testes::trava_envenenada_nao_aceita_nonce_repetido",
        ],
        "prazo": 300,
    },
    # 35. A guarda do 278 inerte e MUDA -- pedido 436 (SEC M3)
    # -----------------------------------------------------------------------
    {
        "id": "guarda-do-pulso-inerte-e-muda",
        "titulo": "o pulso sem prova aceito sem deixar rastro no log",
        "porque": (
            "achado SEC M3 de 23/09/2026. O caminho «sem prova, "
            "`exigir_prova_do_pulso` no padrao, par que nunca provou» era um "
            "`return Ok(())` mudo: o cluster que nunca preencheu `chave_do_fio` "
            "esta tao exposto quanto antes do 278, e nada em execucao deixava "
            "quem opera descobrir. Documento nao e evidencia de instalacao. O "
            "teste reexecuta o binario numa sonda que sobe um no e le o STDERR "
            "dele -- que e o que o operador tem --, e nao um contador que so o "
            "teste enxergaria."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """            self.anunciar_que_passou_sem_prova(&no);
            return Ok(Identidade::SemProva);
""",
        "troca": """            // DEFEITO REPOSTO (436, M3): o `return Ok(())` mudo.
            return Ok(Identidade::SemProva);
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "aceitar_pulso_sem_prova_deixa_rastro",
        ],
        # O comportamento velho: o pulso sem prova continua PASSANDO. O aviso
        # diz que a guarda esta inerte; nao a liga.
        "seguem": [
            "sem_exigencia_o_no_de_versao_anterior_continua_pulsando",
            "depois_que_o_no_provou_o_pulso_sem_prova_e_recusado",
        ],
        "prazo": 300,
    },
    # 36. O aviso da guarda inerte a cada pulso -- pedido 436 (SEC M3)
    # -----------------------------------------------------------------------
    {
        "id": "guarda-do-pulso-inerte-aviso-por-pulso",
        "titulo": "o aviso da guarda inerte repetido a cada pulso",
        "porque": (
            "o outro vermelho do M3: sem a memoria do que ja foi dito, o aviso "
            "sai a cada pulso -- uma linha por segundo por par, para sempre, "
            "que e o aviso perpetuo que o `config.rs` recusa por escrito porque "
            "gasta a confianca do aviso verdadeiro. A sonda manda tres pulsos "
            "do mesmo par; o certo e uma linha."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        if anunciados.contains(id) {
            return;
        }
""",
        "troca": """        // DEFEITO REPOSTO (436, M3): sem a memoria do que ja foi dito.
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "aceitar_pulso_sem_prova_deixa_rastro",
        ],
        "seguem": [
            "sem_exigencia_o_no_de_versao_anterior_continua_pulsando",
        ],
        "prazo": 300,
    },
    # 37. A RESPOSTA do pulso sem o crivo da lista -- pedido 441 (SEC A1)
    # -----------------------------------------------------------------------
    {
        "id": "resposta-do-pulso-sem-crivo-da-lista",
        "titulo": "a resposta do pulso com id fantasma rebaixando o master",
        "porque": (
            "achado SEC A1 de 23/09/2026. O crivo «fora da lista, ou o id DESTE "
            "no» morava so no `op_cluster_pulso`; o laco do pulso le a RESPOSTA "
            "do par e chama o mesmo `conferir_identidade`, que no ramo sem prova "
            "voltava `Ok` ANTES de olhar a lista. Medido pela revisao por "
            "soquete: um par que responde `{\"id\":\"fantasma\",\"papel\":"
            "\"master\",\"epoca\":1}` poe o master em `replica`, epoca 1, "
            "gravado no `cluster.estado.json` -- com todos os nos com pino, desde "
            "que a exigencia esteja no padrao. Quarta vez de «o conserto entra no "
            "caminho que o motivou, e o irmao fica». A troca repoe o motor que "
            "responde `Ok` ao sem-prova antes do crivo."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        let no = match self.no(id) {
            Some(no) if id != self.config.id => no,
            _ => {
                return Err(RecusaDoPulso::ForaDaLista {
                    e_este_no: id == self.config.id,
                })
            }
        };
""",
        "troca": """        // DEFEITO REPOSTO (441): sem prova, o id que nunca provou volta `Ok`
        // ANTES de a lista ser olhada -- o motor de antes, que so o PEDIDO
        // protegia porque o chamador dele conferia a lista.
        if pedido.texto_ou("prova", "").trim().is_empty()
            && !self.config.exigir_prova_do_pulso
            && !self.ja_provou(id)
        {
            return Ok(Identidade::SemProva);
        }
        let no = match self.no(id) {
            Some(no) if id != self.config.id => no,
            _ => {
                return Err(RecusaDoPulso::ForaDaLista {
                    e_este_no: id == self.config.id,
                })
            }
        };
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "a_resposta_de_um_no_fantasma_nao_rebaixa_o_master",
            "a_resposta_com_o_id_deste_no_nao_rebaixa_o_master",
        ],
        # O pedido forjado de um id DA lista nao depende do crivo -- quem o
        # segura e a prova --, e o comportamento velho do sem-prova segue.
        "seguem": [
            "um_pulso_forjado_nao_destrona_o_master",
            "sem_exigencia_o_no_de_versao_anterior_continua_pulsando",
        ],
        "prazo": 300,
    },
    # 38. A resposta ao pulso SEM prova assinada so para quem tem pino -- 435 (SEC A2)
    # -----------------------------------------------------------------------
    {
        "id": "resposta-sem-prova-assinada-so-com-pino",
        "titulo": "a resposta de sucesso a um pulso sem prova dizendo quais nós têm pino",
        "porque": (
            "achado SEC A2 de 23/09/2026: o 435 reaberto. O bit «X tem pino "
            "aqui?» que o 435 fechou no ramo de ERRO continuava no de SUCESSO: "
            "a resposta a um pulso SEM prova assinava sempre que havia pino do "
            "remetente alegado, e `prova`/`nonce`/`quando`/`para` apareciam se "
            "e so se X tinha pino -- 291 B contra 137 B, medido pela revisao "
            "com uma sonda de `posicao` 1e16 que o `registrar` descarta. A troca "
            "repoe a resposta que assina sem olhar se o pedido provou."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        match pedido {
            Identidade::SemProva => None,
            Identidade::Provada => self.campos_da_prova(estatica, destino, canal),
        }
""",
        "troca": """        // DEFEITO REPOSTO (435 reaberto): assina a resposta sem olhar se o
        // pedido provou.
        let _ = pedido;
        self.campos_da_prova(estatica, destino, canal)
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "o_pulso_sem_prova_nao_diz_quais_nos_tem_pino",
        ],
        # A resposta ao pulso PROVADO continua assinada, e o ramo de erro do
        # 435 nao depende desta linha.
        "seguem": [
            "o_pulso_com_prova_valida_passa_e_conta",
            "o_pulso_nao_diz_quais_nos_tem_pino",
        ],
        "prazo": 300,
    },
    # 39. A resposta sem prova que assina e esconde -- 435 reaberto (SEC A2)
    # -----------------------------------------------------------------------
    {
        "id": "resposta-sem-prova-assina-e-esconde",
        "titulo": "a resposta a um pulso sem prova igual na forma e diferente no relógio",
        "porque": (
            "a regua do proprio 435: antes de fechar um canal, medir se o "
            "vizinho entrega o mesmo bit. Calar os campos da resposta e metade: "
            "se o servidor ainda ASSINA para quem tem pino e so joga fora, o "
            "`ms` separa os dois -- medido no binario de teste em 24/09/2026, "
            "40, 40 e 40 de 40 sondas pares, contra 1, 0, 0 e 0 com o conserto. "
            "A troca repoe exatamente isso: forma igual, trabalho diferente."
        ),
        "arquivo": "crates/phxsql-server/src/cluster.rs",
        "trecho": """        match pedido {
            Identidade::SemProva => None,
            Identidade::Provada => self.campos_da_prova(estatica, destino, canal),
        }
""",
        "troca": """        // DEFEITO REPOSTO (435 reaberto, a variante do relogio): assina a
        // resposta de TODO pulso e so esconde os campos do sem prova -- a
        // forma fica igual, e o X25519 a mais so para quem tem pino fica.
        let assinada = self.campos_da_prova(estatica, destino, canal);
        match pedido {
            Identidade::SemProva => None,
            Identidade::Provada => assinada,
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "identidade-do-pulso"],
        "caem": [
            "o_pulso_sem_prova_nao_diz_quais_nos_tem_pino",
        ],
        "seguem": [
            "o_pulso_com_prova_valida_passa_e_conta",
            "o_pulso_nao_diz_quais_nos_tem_pino",
        ],
        "prazo": 300,
    },
    # 37. A reescrita que congela a mae com a transacao viva na filha -- 426
    # -----------------------------------------------------------------------
    {
        "id": "reescrita-sem-portao-na-trava",
        "titulo": "a migração congela a tabela que o COMMIT de uma transação aberta vai abrir",
        "porque": (
            "pedido 426. O portao das travas de transacao rodava FORA da trava "
            "global e lia so o campo `tabela` do pedido; a transacao que "
            "escrevia na FILHA nao travava a MAE, a migracao congelava a mae, "
            "e o COMMIT abria a mae na conferencia da chave depois da marca: "
            "uma tabela gravada e a outra nao, 4006 com `repetir: true`, e a "
            "marca aplicando o resto no arranque. Medido pelo soquete, sem "
            "corrida nenhuma. Quem cede e a reescrita (D5 dos quatro motores)."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Some(recado) = self.transacao_na_vizinhanca(&dados, &database, &tabela, &t)? {
            return Err(PhxError::EmTransacao(recado));
        }
""",
        "troca": """        // DEFEITO REPOSTO (426): a migracao nao pergunta, com a trava na mao,
        // quem a transacao viva alcanca.
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "commit-pelo-soquete"],
        "caem": [
            "a_migracao_cede_a_transacao_que_a_alcanca_pela_chave",
        ],
        # O comportamento velho (tabela sem ligacao nao segura nada) e o irmao
        # continuam de pe: o defeito e so desta porta.
        "seguem": [
            "transacao_em_tabela_sem_ligacao_nao_segura_a_reescrita",
            "o_acrescentar_coluna_tambem_cede_a_transacao",
        ],
        "prazo": 420,
    },
    # 38. O irmao: acrescentar_coluna, mesmas funcoes na mesma ordem -- 426
    # -----------------------------------------------------------------------
    {
        "id": "acrescentar-coluna-sem-portao",
        "titulo": "o acrescentar_coluna congela a tabela que o COMMIT de uma transação aberta vai abrir",
        "porque": (
            "o IRMAO do `reescrita-sem-portao-na-trava`: `op_acrescentar_coluna` "
            "chama `travar_dados` -> `abrir_travada` -> `congelar` -> solta a "
            "trava, na mesma ordem do `op_migrar_esquema`. Conserto que "
            "entrasse so na migracao deixaria este com o defeito inteiro."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Some(recado) = self.transacao_na_vizinhanca(
            &dados,
            p.texto_ou("database", ""),
            p.texto_ou("tabela", ""),
            &t,
        )? {
            return Err(PhxError::EmTransacao(recado));
        }
""",
        "troca": """        // DEFEITO REPOSTO (426): o irmao sem a pergunta.
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "commit-pelo-soquete"],
        "caem": [
            "o_acrescentar_coluna_tambem_cede_a_transacao",
        ],
        "seguem": [
            "a_migracao_cede_a_transacao_que_a_alcanca_pela_chave",
        ],
        "prazo": 420,
    },
    # 39. O COMMIT sem a rede antes da marca -- 426
    # -----------------------------------------------------------------------
    {
        "id": "commit-sem-rede-antes-da-marca",
        "titulo": "o COMMIT grava a marca com uma tabela do alcance congelada",
        "porque": (
            "a rede do 426 para o congelamento que nasce por um caminho que "
            "ninguem previu: sem ela, a passada bate na tabela congelada "
            "DEPOIS da marca -- 1 de 2 linhas no disco, e o arranque aplicando "
            "a outra. Com ela, a recusa vem antes da marca e a transacao "
            "continua ativa (o `SQLITE_BUSY` no COMMIT)."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        let dir = trava.abrir_database(database)?.caminho().to_path_buf();
        if phxsql_store::congelamento::quantas() == 0 {
            return Ok(dir);
        }
""",
        "troca": """        let dir = trava.abrir_database(database)?.caminho().to_path_buf();
        // DEFEITO REPOSTO (426): sem a rede antes da marca.
        if phxsql_store::congelamento::quantas() < usize::MAX {
            return Ok(dir);
        }
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "commit-pelo-soquete"],
        "caem": [
            "o_commit_contra_a_tabela_congelada_nao_sai_pela_metade",
        ],
        "seguem": [
            "a_escrita_na_vizinha_da_congelada_recusa_na_instrucao",
        ],
        "prazo": 420,
    },
    # 40. A escrita na vizinha da congelada entrando na lista -- 426 (D3)
    # -----------------------------------------------------------------------
    {
        "id": "instrucao-na-vizinha-da-congelada",
        "titulo": "a escrita ligada pela chave a uma tabela congelada entra na lista da transação",
        "porque": (
            "D3 dos quatro motores: «repita» se diz na INSTRUCAO, com zero "
            "aplicado -- o lugar do 1412 `ER_TABLE_DEF_CHANGED` do MySQL. O "
            "`empilhar` nao confere chave estrangeira, entao a filha de uma "
            "mae congelada entrava na lista e o problema so aparecia no COMMIT."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if let Some(recado) = self.congelada_no_alcance(
            &trava,
            &database,
            phxsql_store::catalogo::separar_qualificado(&tabela)
                .0
                .as_deref(),
            t.diretorio(),
            &[t.nome().to_string()],
        )? {
            return Err(PhxError::EmMigracao(recado));
        }
""",
        "troca": """        // DEFEITO REPOSTO (426): sem a recusa na instrucao.
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "commit-pelo-soquete"],
        "caem": [
            "a_escrita_na_vizinha_da_congelada_recusa_na_instrucao",
        ],
        "seguem": [
            "o_commit_contra_a_tabela_congelada_nao_sai_pela_metade",
        ],
        "prazo": 420,
    },
    # 41. A recuperacao do braco de erro que nunca rodava -- 426 (c)
    # -----------------------------------------------------------------------
    {
        "id": "braco-de-erro-retrava",
        "titulo": "a passada do COMMIT quebra depois da marca e a recuperação da hora não roda",
        "porque": (
            "camada (c) do 426: o braco de erro pedia `travar_dados()` com a "
            "trava do topo ainda viva, a trava nao e reentrante, o `if let Ok` "
            "engolia a recusa e a recuperacao NUNCA rodava -- enquanto o "
            "comentario acima dizia que rodava. Envolver nao e substituir. O "
            "conserto completa com a MESMA trava."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                let r = crate::transacao::completar_marca(&trava, database, marca);
                drop(trava);
""",
        "troca": """                // DEFEITO REPOSTO (426 c): toma a trava de novo com a do topo viva.
                let r = match self.travar_dados() {
                    Ok(t) => crate::transacao::completar_marca(&t, database, marca),
                    Err(_) => crate::transacao::Relatorio::default(),
                };
                drop(trava);
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_passada_que_quebra_depois_da_marca_nao_deixa_meia_transacao",
            "servidor::testes_transacoes::a_recuperacao_do_braco_de_erro_roda_de_verdade",
        ],
        "seguem": [
            "servidor::testes_transacoes::a_quebra_de_acesso_antes_de_qualquer_byte_devolve_a_transacao",
            "servidor::testes_transacoes::o_commit_que_quebra_depois_da_marca_nao_manda_repetir",
        ],
    },
    # 42. A recuperacao da hora apagando a marca do impossivel passageiro -- 426
    # -----------------------------------------------------------------------
    {
        "id": "completar-apaga-a-marca-impossivel",
        "titulo": "a recuperação do COMMIT apaga a marca de uma operação que só estava congelada",
        "porque": (
            "achado ao consertar o (c) do 426: o `recuperar` do arranque apaga "
            "a marca mesmo com operacao impossivel -- la o impossivel e "
            "permanente. Com o servidor de pe ele pode ser passageiro (a tabela "
            "congelada), e consertar so o (c) trocaria «completa no proximo "
            "arranque» por «perdida para sempre»."
        ),
        "arquivo": "crates/phxsql-server/src/transacao.rs",
        "trecho": """            arranque == NoArranque::Sim || r.impossiveis.len() == antes
""",
        "troca": """            // DEFEITO REPOSTO (426): apaga a marca do impossivel passageiro.
            {
                let _ = (arranque, antes);
                true
            }
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_quebra_que_a_recuperacao_nao_vence_fica_na_marca_ate_o_arranque",
        ],
        "seguem": [
            "servidor::testes_transacoes::a_recuperacao_do_braco_de_erro_roda_de_verdade",
            "servidor::testes_transacoes::marca_que_nao_confere_e_commit_que_nunca_comecou",
        ],
    },
    # 43. O AFTER que grava no COMMIT sumindo calado -- 262, etapa 1
    # -----------------------------------------------------------------------
    {
        "id": "after-no-commit-some-calado",
        "titulo": "o AFTER disparado no COMMIT grava numa lista já descartada e some sem aviso",
        "porque": (
            "pedido 262: com a sessao em `COMMITTING`, a escrita do gatilho "
            "caia no `empilhar` de uma lista que o COMMIT ja tinha tirado, "
            "depois da marca selada, e ia ao chao no descarte -- `gravadas: 1`, "
            "auditoria vazia, nenhum `gatilhos_avisos`. Calado no sucesso, "
            "barulhento so no erro. Trocar o estado para `Ativa` mediria zero."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """        if estado == crate::transacao::Estado::Confirmando
            && (OPS_EMPILHAVEIS.contains(&op) || OPS_ESCRITA.contains(&op))
""",
        "troca": """        // DEFEITO REPOSTO (262): `COMMITTING` passa direto para o `empilhar`.
        if false
            && estado == crate::transacao::Estado::Confirmando
            && (OPS_EMPILHAVEIS.contains(&op) || OPS_ESCRITA.contains(&op))
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "commit-pelo-soquete"],
        "caem": [
            "o_after_que_grava_no_commit_nao_some_calado",
        ],
        # C2 e o comportamento VELHO -- o mais importante dos cinco.
        "seguem": [
            "o_commit_sem_gatilho_nao_muda_nada",
            "o_after_do_commit_nao_roda_no_rollback",
            "o_rowid_da_auditoria_nao_ganha_buraco",
        ],
        "prazo": 420,
    },
    # 44. A quebra de acesso antes de qualquer byte, e a marca que fica -- 426
    # -----------------------------------------------------------------------
    {
        "id": "commit-zero-aplicado-vira-committed",
        "titulo": "a passada que quebra antes de qualquer byte da lista responde COMMITTED",
        "porque": (
            "426: sem nada aplicado e com erro de ACESSO, a marca pode sair e "
            "a transacao volta a ACTIVE -- repetir o COMMIT e verdade. Sem o "
            "ramo, a recuperacao completaria por baixo e o cliente ouviria "
            "COMMITTED de uma transacao cuja escrita bateu numa tabela fechada."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                if do_pedido && aplicadas == 0 && std::fs::remove_file(marca).is_ok() {
""",
        "troca": """                // DEFEITO REPOSTO (426): sem o ramo do zero aplicado.
                if false && do_pedido && aplicadas == 0 && std::fs::remove_file(marca).is_ok() {
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::a_quebra_de_acesso_antes_de_qualquer_byte_devolve_a_transacao",
        ],
        "seguem": [
            "servidor::testes_transacoes::p0_filha_antes_do_pai_no_commit_ainda_recusa",
            "servidor::testes_transacoes::a_filha_antes_do_pai_nao_deixa_marca_para_o_arranque",
        ],
    },
    # 45. O erro do dado no meio da lista, calado sobre o que ficou -- 426
    # -----------------------------------------------------------------------
    {
        "id": "commit-meio-sem-dizer-o-que-ficou",
        "titulo": "a chave que falha no meio da passada vira COMMITTED sem a escrita que falhou",
        "porque": (
            "a lacuna que sobra do 426: a chave estrangeira so e conferida na "
            "passada, depois da marca. Sem o ramo do erro do DADO, a recuperacao "
            "completaria o resto e responderia COMMITTED de uma transacao "
            "invalida; com ele, a marca sai, a resposta diz quantas ficaram e "
            "o arranque nao muda o passado."
        ),
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "trecho": """                if do_pedido && !de_acesso {
                    let _ = std::fs::remove_file(marca);
""",
        "troca": """                // DEFEITO REPOSTO (426): sem o ramo do erro do dado no meio.
                if false && do_pedido && !de_acesso {
                    let _ = std::fs::remove_file(marca);
""",
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "servidor::testes_transacoes::o_erro_do_dado_no_meio_da_lista_diz_o_que_ficou_e_o_arranque_nao_muda",
        ],
        "seguem": [
            "servidor::testes_transacoes::a_filha_antes_do_pai_nao_deixa_marca_para_o_arranque",
        ],
    },
    # 46. O `de_hex` que fatia TEXTO por byte -- pedido 446, o motor
    # -----------------------------------------------------------------------
    {
        "id": "de-hex-fatia-texto-por-byte",
        "titulo": "o de_hex em pânico com hexadecimal que corta um caractere de vários bytes",
        "porque": (
            "achado da frente 436 em 24/09/2026, medido pelo integrador: "
            "`&t[i..i + 2]` com a paridade conferida por `len() % 2`, que conta "
            "BYTES. `\"a€\"` tem quatro bytes, passa, e o corte cai no meio do "
            "`€` -- panico, e nao `None`. O `from_str_radix` de dentro e o "
            "segundo furo da mesma linha: aceita `+`, e `\"+f+f\"` virava "
            "`[15, 15]`. Esta entrada prova o MOTOR; o dano pelo soquete esta "
            "nas duas seguintes."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """    let mut saida = Vec::with_capacity(t.len() / 2);
    for par in t.chunks_exact(2) {
        saida.push((digito_hex(par[0])? << 4) | digito_hex(par[1])?);
    }
    Some(saida)
""",
        "troca": """    // DEFEITO REPOSTO (446): fatia o TEXTO por byte, pelo from_str_radix.
    let t = core::str::from_utf8(t).unwrap();
    (0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok())
        .collect()
""",
        "pacote": "phxhash",
        "alvo": ["--lib"],
        "caem": [
            "hash::tests::de_hex_com_caractere_de_varios_bytes_recusa_sem_panico",
            "hash::tests::de_hex_recusa_o_sinal_que_o_from_str_radix_aceita",
        ],
        "seguem": [
            "hash::tests::hex_vai_e_volta",
        ],
        "prazo": 300,
    },
    # 47. O mesmo `de_hex`, visto pelo SOQUETE -- pedido 446
    # -----------------------------------------------------------------------
    {
        "id": "prova-do-pulso-derruba-a-conexao",
        "titulo": "a prova do pulso que corta um caractere derruba a conexão e mata o laço do pulso",
        "porque": (
            "o alcance do 446, medido pelo soquete com o servidor de pe. O "
            "PEDIDO `cluster_pulso` (credencial do cluster, id da lista, chave "
            "estatica neste no) com `prova` = `\"a€\"` x16: a conexao cai sem "
            "resposta; o no segue atendendo e nenhuma trava envenena. A "
            "RESPOSTA do pulso passa pelo mesmo `de_hex`, e ali o dano e maior: "
            "a thread de pulso para aquele par morre e nunca mais sobe, porque "
            "so se desmarca do `pulsando` pelo caminho normal -- o par recebe "
            "1 pulso e mais nada. E o `hex_para_bytes` passou a chamar o motor, "
            "entao o `Bin` do `inserir` cai junto: a trava de dados envenena."
        ),
        "arquivo": "crates/phxhash/src/hash.rs",
        "trecho": """    let mut saida = Vec::with_capacity(t.len() / 2);
    for par in t.chunks_exact(2) {
        saida.push((digito_hex(par[0])? << 4) | digito_hex(par[1])?);
    }
    Some(saida)
""",
        "troca": """    // DEFEITO REPOSTO (446): fatia o TEXTO por byte, pelo from_str_radix.
    let t = core::str::from_utf8(t).unwrap();
    (0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok())
        .collect()
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "hexadecimal-do-fio"],
        "caem": [
            "a_prova_do_pulso_que_corta_um_caractere_e_recusada_com_resposta",
            "a_prova_torta_na_resposta_nao_mata_o_laco_do_pulso",
            "binario_que_corta_um_caractere_nao_envenena_a_trava_de_dados",
        ],
        # A web nao passa pelo `de_hex`: le o digito do motor byte a byte.
        "seguem": [
            "percent_seguido_de_multibyte_no_idiomas_responde",
        ],
        "prazo": 300,
    },
    # 48. A COPIA do `de_hex` que lia o `Bin` do `inserir` -- pedido 446
    # -----------------------------------------------------------------------
    {
        "id": "copia-do-de-hex-envenena-a-trava-de-dados",
        "titulo": "o binário que corta um caractere envenena a trava global de dados",
        "porque": (
            "o irmao mais caro do 446, achado na varredura: "
            "`carga::hex_para_bytes` era copia do `de_hex`, com o mesmo corte, "
            "e e ela que o `json_para_valor` chama para a coluna `Bin` -- "
            "DEPOIS de o `op_inserir` tomar a trava global de dados. Medido "
            "pelo soquete: a conexao do `inserir` cai, e o `inserir` seguinte, "
            "por OUTRA conexao, recebe «uma operacao anterior entrou em panico "
            "e deixou a trava suja». Veneno de `RwLock` e permanente: qualquer "
            "usuario com direito de inserir numa tabela com coluna binaria "
            "parava a base de todos ate o reinicio. A copia virou chamada ao "
            "motor (a petrea «funcao e comando vem do mesmo motor»)."
        ),
        "arquivo": "crates/phxsql-core/src/carga.rs",
        "trecho": """    crate::hash::de_hex(t).ok_or_else(|| PhxError::Tipo(format!("hexadecimal invalido: {hex:?}")))
""",
        "troca": """    // DEFEITO REPOSTO (446): a copia do de_hex, com o corte por byte.
    (0..t.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|_| PhxError::Tipo(format!("hexadecimal invalido: {hex:?}")))
        })
        .collect()
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "hexadecimal-do-fio"],
        "caem": [
            "binario_que_corta_um_caractere_nao_envenena_a_trava_de_dados",
        ],
        "seguem": [
            "a_prova_do_pulso_que_corta_um_caractere_e_recusada_com_resposta",
            "percent_seguido_de_multibyte_no_idiomas_responde",
        ],
        "prazo": 300,
    },
    # 49. O `%XX` da porta web fatiado por byte, antes do login -- pedido 446
    # -----------------------------------------------------------------------
    {
        "id": "percent-da-web-fatia-texto-por-byte",
        "titulo": "o %XX da porta web em pânico com caractere de vários bytes, sem login",
        "porque": (
            "o irmao do 446 na porta HTTP, achado na varredura das fatias "
            "`[i..i + 2]`: o `desescapar` fatiava `bruto[i + 1..i + 3]`, e o "
            "`GET /idiomas` e servido SEM credencial (a tela de entrada precisa "
            "dos rotulos). Medido pelo soquete: `/idiomas?idioma=%€` fecha a "
            "conexao sem uma linha de resposta. A vaga da web volta (a "
            "`Permissao` morre no desenrolar, pedido 248) e a porta segue "
            "atendendo: o dano e a thread, e o que custa e zero credencial."
        ),
        "arquivo": "crates/phxsql-server/src/http.rs",
        "trecho": """            b'%' if i + 2 < bytes.len() => {
                match (digito_hex(bytes[i + 1]), digito_hex(bytes[i + 2])) {
                    (Some(alto), Some(baixo)) => {
                        saida.push((alto << 4) | baixo);
                        i += 3;
                    }
                    _ => {
                        saida.push(b'%');
                        i += 1;
                    }
                }
            }
""",
        "troca": """            // DEFEITO REPOSTO (446): a fatia do texto por byte.
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&bruto[i + 1..i + 3], 16) {
                Ok(b) => {
                    saida.push(b);
                    i += 3;
                }
                Err(_) => {
                    saida.push(b'%');
                    i += 1;
                }
            },
""",
        "pacote": "phxsql-server",
        "alvo": ["--test", "hexadecimal-do-fio"],
        "caem": [
            "percent_seguido_de_multibyte_no_idiomas_responde",
        ],
        "seguem": [
            "a_prova_do_pulso_que_corta_um_caractere_e_recusada_com_resposta",
            "binario_que_corta_um_caractere_nao_envenena_a_trava_de_dados",
        ],
        "prazo": 300,
    },
    # 50. O mapa do cluster envenenado virando «ninguem vivo» -- pedido 447
    # -----------------------------------------------------------------------
    {
        "id": "mapa-do-cluster-envenenado-vira-vazio",
        "titulo": "o mapa de pulsos envenenado devolvido vazio: a eleição trava",
        "porque": (
            "achado da frente 436, lido pelo integrador: "
            "`self.nos.lock().map(|m| m.clone()).unwrap_or_default()`. Medido "
            "com a trava envenenada num cluster de tres: `vivos()` via 1 de 3, "
            "o master perdia a maioria e recusava toda escrita, e a replica "
            "envenenada que a outra elegia (2 de 3, vencedora ela) nao se "
            "promovia (1 de 3, nenhum vencedor) -- cluster sem master, sem "
            "prazo; o `registrar` seguinte sumia calado. O `Mutex` virou "
            "`TravaDaGuarda` (o motor do 436), entao repor o defeito pede as "
            "DUAS trocas da entrada 34: reabrir um `lock` que devolve `None` "
            "no veneno, e usa-lo no `mapa()` com o `unwrap_or_default` de origem."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-server/src/pulso.rs",
                "trecho": """    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
""",
                "troca": """    /// DEFEITO REPOSTO (447): a porta que o tipo fechou, reaberta.
    pub(crate) fn tentar(&self) -> Option<MutexGuard<'_, T>> {
        self.trava.lock().ok()
    }

    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
""",
            },
            {
                "arquivo": "crates/phxsql-server/src/cluster.rs",
                "trecho": """        self.nos.travar().clone()
""",
                "troca": """        self.nos.tentar().map(|m| m.clone()).unwrap_or_default()
""",
            },
        ],
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "cluster::testes::o_mapa_com_a_trava_envenenada_continua_vendo_os_vivos",
        ],
        "seguem": [
            "cluster::testes::master_de_epoca_velha_nao_conta",
            "cluster::testes::a_familia_das_travas_do_cluster_recupera_o_veneno",
        ],
        "prazo": 300,
    },
    # 51. A lista viva envenenada voltando ao config do arranque -- pedido 447
    # -----------------------------------------------------------------------
    {
        "id": "lista-do-cluster-envenenada-volta-ao-arranque",
        "titulo": "a lista viva de nós envenenada respondida pelo config.json do arranque",
        "porque": (
            "o irmao do `mapa()` que o proprio pedido 447 nomeia: a `lista()` "
            "caia no `config.nos` quando a trava envenenava -- a MESMA falha "
            "com outra resposta. Medido: o no acrescentado a quente deixava de "
            "existir para quem le a lista, `acrescentar` e `remover` diziam "
            "`false` e o denominador da maioria voltava ao do arranque. O teste "
            "da familia cobre as seis travas; esta entrada repoe a que o pedido "
            "descreve, pelas mesmas duas trocas da anterior."
        ),
        "trocas": [
            {
                "arquivo": "crates/phxsql-server/src/pulso.rs",
                "trecho": """    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
""",
                "troca": """    /// DEFEITO REPOSTO (447): a porta que o tipo fechou, reaberta.
    pub(crate) fn tentar(&self) -> Option<MutexGuard<'_, T>> {
        self.trava.lock().ok()
    }

    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
""",
            },
            {
                "arquivo": "crates/phxsql-server/src/cluster.rs",
                "trecho": """        self.lista.travar().clone()
""",
                "troca": """        self.lista
            .tentar()
            .map(|l| l.clone())
            .unwrap_or_else(|| self.config.nos.clone())
""",
            },
        ],
        "pacote": "phxsql-server",
        "alvo": ["--lib"],
        "caem": [
            "cluster::testes::a_familia_das_travas_do_cluster_recupera_o_veneno",
        ],
        "seguem": [
            "cluster::testes::o_mapa_com_a_trava_envenenada_continua_vendo_os_vivos",
            "cluster::testes::replica_redireciona_para_o_master",
        ],
        "prazo": 300,
    },
]
