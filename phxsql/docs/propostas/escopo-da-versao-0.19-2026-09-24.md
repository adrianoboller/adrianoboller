# Escopo da versão 0.19 — proposta de classificação dos pedidos abertos (24/09/2026)

Papel G (QA), só leitura. Base: `docs/PENDENCIAS.md` em `HEAD` `f1597be`. **Proposta — o integrador revisa e aplica; nada aqui edita o PENDENCIAS.**

O pedido falava em 142 abertos (19 ◐ + 123 ☐). Medido em `f1597be`: **141** (19 ◐ + 122 ☐) — o 483 fechou em `f1597be` (08:47) e já está ☑️. Feitos: **342**. Total: **483**.

Critério aplicado à letra: FICA se (1) defeito ativo, (2) bloqueia a entrega, (3) o texto diz «pedido/ordem/decisão do dono», ou (4) ◐ cuja parte que falta é 1/2/3. O resto vai para `⏸`. Na dúvida, FICA e vai marcado «dúvida». `FECHAR?` = o fonte já não tem o defeito (arquivo e linha na coluna do motivo, conferidos por `git grep` em `HEAD`).

## 1. Totais (gerados do quadro abaixo)

| | pedidos |
|---|---:|
| FICA na versão | **84** |
| … dos quais por dúvida | 24 |
| vão para `⏸` | **51** |
| parecem fechados (`FECHAR?`) | **6** |
| soma (tem de dar 141) | 141 |

Critério dos que ficam (um pedido pode contar em mais de um): (1) 33, (2) 6, (3) 29, (4) 14, (dúvida) 24.

| porcentagem que falta | conta | valor |
|---|---|---:|
| hoje, sem `⏸` | 141 / 483 | 29,2% |
| **proposta, `FECHAR?` confirmados** | fica / (feitos + fechados + fica) = 84 / (342 + 6 + 84) | **19,4%** |
| à letra do pedido, `FECHAR?` fora da conta | fica / (feitos + fica) = 84 / (342 + 84) | 19,7% |
| conservadora, `FECHAR?` ainda abertos | (84 + 6) / (342 + 84 + 6) | 20,8% |

## 2. O quadro

| pedido | estado atual | proposto | critério | motivo |
|---|:-:|:-:|:-:|---|
| **175** Índice da chave: havia uma terceira saída — CRIAR — e nós não a vimos | ☐ | FICA | 3 | «DECISÃO do dono 04/09: MEDIR PRIMEIRO»; medido 0 porque não há base real — aberto pela própria decisão |
| **190** Bateria de testes de todos os botões | ◐ | FICA | 4(3) | ordem do dono 05/09 (bateria de todos os botões); faltam 119 sem prova (`TETO_BOTAO_SEM_PROVA`) |
| **197** Senha por banco vinda do login: o parecer com os dois defeitos que só… | ◐ | FICA | 4(3) | «DECISÃO DO DONO 17/09: FAZER» (senha por banco); nada implementado — nenhum `senha_banco` fora de `segredos`/Profiler |
| **207** Transação com quórum — replicar a gravação esperando N servidores con… | ◐ | FICA | 4(3) | decidido pelo dono 07/09 e 17/09 (quórum pelo canal aberto, «ok» = aplicado e em disco); escrita com quórum não existe |
| **209** O lixo que os examples/ deixam em /tmp — contado, não medido | ☐ | ⏸ | nenhum | lixo dos `examples/` em `/tmp`: higiene de medição, sem defeito de produto |
| **229** Auto number: tres defeitos de produto medidos pela F8 | ◐ | FICA | 4(1,3) | (a) bidirecional na mesma faixa ainda apaga a linha perdedora (só conta e grita); itens de formato por decisão do dono |
| **238** SQL_C_WCHAR e parâmetro de SAÍDA recusam no driver ODBC | ◐ | FICA | 4(3) | «DECISÃO DO DONO 17/09»: sonda viva OUT/INOUT medindo o disco antes — não rodada |
| **239** Nível de isolamento acima de READ COMMITTED e TLS no transporte ficam… | ◐ | FECHAR? | — | as duas metades entregues: leitura repetível pela trava (16/09) e a receita do proxy TLS em `docs/SEGURANCA.md:940` (§7.1, «pedido 239») |
| **245** Seis observações da revisão do motor, menores, para varrer numa rodad… | ◐ | FICA | 4(3) | O2: dono decidiu 17/09 RECUSAR a declaração nomeando quantas linhas violam; o código só avisa (teste `acrescentar_coluna_com_regra_avisa_o_que_a_linha_velha_nao_ganhou`) |
| **249** Saúde do disco do banco: sonda canário, EROFS, erro de E/S imediato,… | ◐ | FICA | 4(3) | dono decidiu 17/09 sonda a 5 min; `config.rs:1122` ainda `checar_segundos: 60`; meio do SMS aguarda parecer próprio |
| **251** P2P: a identidade sem domínio bate no AAD do selo por endereço — deci… | ☐ | FICA | 3 | «parado por decisão do dono» (AAD do selo × identidade sem domínio: formato do correio) |
| **255** Tomada no meio de BULKINSERT ou de reindexar deixa a tabela recusando… | ☐ | FICA | 3 | «parado por decisão do dono»; `.ndx` sujo sem marca segue calado para quem opera |
| **259** O excluir do padrão custa 24–28 µs mesmo sem fsync, contra 3,7–4,4 do… | ◐ | FICA | 4(3) | dono decidiu 17/09 pular a busca reversa quando ninguém declara chave; `table.rs:1924` ainda abre o `.reg` de cada irmã |
| **262** Gatilho AFTER que grava pela mesma sessão dentro do COMMIT não chega… | ◐ | FICA | 4(1) | etapa 2 em 0%: a gravação do gatilho AFTER no COMMIT continua não acontecendo (agora recusa com aviso); os 4 motores convergem 10×0 que devia gravar |
| **263** Onze guardas do catálogo estavam QUEBRADAS — o trecho que repõe o def… | ◐ | FICA | 4(3) | dono decidiu 17/09 rodar o provador INTEIRO; só 20 de 142 rodaram (disco), falta a corrida em lotes |
| **268** A migração Criptografar/Descriptografar — declarar em cifra.tabelas n… | ☐ | FICA | dúvida | `cifra.tabelas` declara mais do que o disco cumpre para tabela que já existe (a tela avisa); garantia de cifra que não vale no gravado, e formato entra cedo |
| **272** Newtype Segredo(String) com Debug redator — o compilador conta o que… | ☐ | ⏸ | nenhum | newtype `Segredo`: endurecimento sem defeito; o `==` do Bearer já virou `token_confere` (`servidor.rs:8822`) |
| **273** A sexta forma de QUEBRADA que o trecho-vivo.py não vê: caem/seguem se… | ◐ | ⏸ | nenhum | a régua entrou (`TETO_TESTE_SEM_MODULO`); resta o `--raiz` do provador, ferramenta |
| **276** O catalogo.rs declara token onde a operação lê token_remoto, e não de… | ☐ | FICA | dúvida | `catalogo.rs:1590` declara só `token` no `replicacao_testar`, que lê `token_remoto` primeiro (`origem_da_sonda`): a spec publicada da op está errada; `prova`/`assinatura` fora do `LEXICO` (`debug-com-segredo.py:133`) |
| **277** Os irmãos que a frente de design nomeou e não pôde tocar: pagina-dos-… | ☐ | ⏸ | nenhum | cor fora da marca em página de documentação (`pagina-dos-pedidos.py:357`, dossiê); não é dado nem produto |
| **284** SEC A7 (media) — replicas_autorizadas tranca pelo par do soquete, e c… | ☐ | FICA | 1 | `replicas_autorizadas` colapsa num IP atrás de proxy/NAT e `posicao`/`replicar`/`aplicar` entram pelo `/api`; `REPLICACAO.md` §7 promete a tranca sem a ressalva |
| **290** DECISAO DO DONO (C, 2/6) — inicio/passo por no para a Sequence, no me… | ☐ | FICA | 1,3 | decidido pelo dono; o `inicio` do nó tem zero chamadores de produção (`no.rs:77`, só `tests/carimbo-e-faixa.rs:430/458`): faixas colidem 100% |
| **293** DECIDIDO PELO DONO 17/09 07:10 (recusar no motor agora) — ESPERA ENGE… | ☐ | FICA | 1,3 | decidido pelo dono 17/09: recusar no motor; não implementado — réplica sem cofre grava 63 B cifrados como conteúdo (`reg.rs:1986`) |
| **294** DECIDIDO PELO DONO 17/09 07:10 (manter a soma, medir por tabela AO LA… | ☐ | FICA | 3 | decidido pelo dono: manter a soma e medir por tabela ao lado; não implementado |
| **296** Garantia que nao vale, nao coberta acima (C) — a versao viaja na imag… | ☐ | FECHAR? | — | o leitor da réplica tem `versao` (`replica.rs:373`, `:484`) e `diario_local_continua` a compara; C não descreveu outra quebra |
| **297** Garantia que nao vale, nao coberta acima (C) — o .trash/.reason diver… | ☐ | FICA | dúvida | `INTEGRIDADE.md` §3 promete fidelidade e não diz que `.trash`/`.reason` divergem na réplica (zero menção a lixeira no arquivo); o conserto mínimo é uma frase |
| **298** Garantia que nao vale, nao coberta acima (C) — o .log da replica perd… | ☐ | FECHAR? | — | consertado em `49a3af7` (17/09): `aplicar_lote_da_replica` chama `forcar_proximo_evento` (`servidor.rs:3064`) |
| **299** Garantia que nao vale, nao coberta acima (C) — atomicidade de commit… | ☐ | ⏸ | nenhum | atomicidade entre tabelas na réplica: preço declarado (`INTEGRIDADE.md` §3); conserto é capacidade nova recusada por C |
| **300** Garantia que nao vale, nao coberta acima (C) — a posicao somada do cl… | ☐ | FICA | 1 | (3) a posição do cluster soma tabela não replicada (`servidor.rs:4077`) e decide eleição; sem contador de órfãs. O (4) deixou de ser calado: `diario_local_continua` rompe (`servidor.rs:3048`) |
| **302** QA (G) — crates/phxsql-server/src/cluster.rs tem ZERO entradas no cat… | ☐ | FECHAR? | — | `cluster.rs` já tem entradas no catálogo (`bancada/guardas/catalogo.py:8243`, `:8399`, `:8831`); a catraca por arquivo sugerida é endurecimento (⏸ se ficar) |
| **304** J — aproveitar a IDEIA do <dataBar> do Excel Studio no XLSX: barra pr… | ☐ | ⏸ | nenhum | capacidade nova (barra de dados no XLSX) |
| **305** J — aproveitar a IDEIA do construtor visual de consulta com N filtros… | ☐ | ⏸ | nenhum | capacidade nova (construtor de consulta com N filtros) |
| **306** F (bateria) — a guarda de queda do trava.py enfraquece sozinha quando… | ☐ | ⏸ | nenhum | a guarda de queda ainda corta (4) e passa; risco futuro e o «dez cortes» velho do `LEIA-ME.md:96` — higiene de bancada |
| **307** F (bateria) — dois blocos do replicacao/resultados.json não têm gerad… | ☐ | ⏸ | nenhum | blocos sem gerador no `resultados.json` não são publicados hoje; higiene |
| **308** F (bateria) — quatro resultados.json de replicação/cluster não trazem… | ☐ | ⏸ | nenhum | quatro `resultados.json` sem data; a página já marca que caiu no `mtime` |
| **309** Via (b) do pedido 291 — a réplica honrar o rownum que vem na imagem,… | ☐ | FICA | 3 | o texto pede «Decisão do dono» (via b dos buracos históricos de `rownum`); pergunta pendente |
| **310** ler dentro de uma transação paga O(pendentes) sob a trava global — 38… | ☐ | ⏸ | nenhum | desempenho sem regressão (`ler` em transação é O(pendentes)) |
| **311** O assistente de replicação mostra «PhxSql» com um buraco no lugar da… | ☐ | FICA | 1 | a tela escreve `PhxSql ${s.versao}` e `${s.ms} ms` (`ui/index.html:13256-13257`) e `sondar_origem` não devolve nenhum dos dois (`servidor.rs:~25038`) |
| **313** O vencedor promove o menos atrasado mesmo que ele esteja arbitrariame… | ☐ | ⏸ | nenhum | teto de atraso na eleição: endurecimento; o failover assíncrono já é declarado não-Raft |
| **317** O zelador nunca alcanca o maior consumidor da maquina: ele e tudo-ou-… | ◐ | ⏸ | nenhum | corte do `incremental` e vigia entraram; resta o provador apagar o próprio cache e o `/tmp` — higiene de ambiente |
| **318** A parada do par por tabela nao aparece na tela, e nao ha botao de pul… | ☐ | ⏸ | nenhum | tela nova (parada por tabela e botão de pular); o dado já está no protocolo |
| **319** O grito do unidirecional e pobre: nao nomeia tabela, chave nem linha,… | ☐ | ⏸ | nenhum | mensagem do unidirecional pobre: melhoria de diagnóstico |
| **320** Ninguem mediu quantas tabelas do acervo carregam hoje dois ids de col… | ☐ | ⏸ | nenhum | inventário de ids de coluna repetidos, sem defeito demonstrado |
| **321** O teste de continuidade da replica cai com a maquina carregada, e pas… | ☐ | FICA | 1,2 | teste que floca sob carga (`esperar_eventos`→`exigir`, `continuidade-da-replica.rs:163-172`) e já barrou o `numeros-do-projeto.py` |
| **322** O Portão 4 (tabela reservada para carga) olha um campo só, e quatro o… | ☐ | FICA | 1 | Portão 4 lê só `tabela` (`servidor.rs:10560-10566`): a reserva do BULKINSERT se contorna pelo lado B do `juntar` — a promessa de `carga.rs:8-9` não vale |
| **323** O comunicacao.sh diz «nada compilando nem rodando agora» com um GERAD… | ☐ | ⏸ | nenhum | a metade do gerador em `docs/` foi consertada (`bc86187`, `esta-medindo.sh:99-130`); resta o crivo de subagente, ferramenta de sessão |
| **324** DECISÃO DO DONO — adiar o .ndx e reconstruir EM THREAD: a carga ganha… | ☐ | FICA | 3 | «DECISÃO DO DONO»/pedido do dono (adiar o `.ndx` em thread): escolha de SLA pendente |
| **325** PEDIDO DO DONO — 20 caixas de supermercado e 1 servidor: o servidor c… | ☐ | FICA | 1,3 | PEDIDO DO DONO (20 caixas); a perna (5) perde venda hoje (229a) |
| **326** O dossiê compartilhado mostra uma versão FIXADA e antiga: quem abre o… | ☐ | FICA | dúvida | o link compartilhado mostra versão fixada e velha; nossa metade existe (`numeros-do-projeto.py:214`, `selo_da_capa`), resta ação do dono no compartilhamento |
| **328** O fecho de rodada nao tem comando: e um ritual de oito passos que so… | ☐ | ⏸ | nenhum | comando de fecho de rodada: ferramenta de processo |
| **329** O hash_id do bidirecional tem 16 bits e a conferencia de colisao e po… | ☐ | FICA | dúvida | perna do pedido do dono 325; colisão do `hash_id` de 16 bits (`bidirecional.rs:70`) só morde em N nós, ainda não suportado |
| **330** A primeira rodada do bidirecional apos cada arranque reabsorve o diar… | ☐ | FICA | dúvida | perna do 325 («imediata»); custo do 1º arranque ainda a medir |
| **331** Chave COMPOSTA nao replica no modo bidirecional: a tabela e recusada… | ☐ | FICA | dúvida | perna do 325 (itens de venda com chave composta); hoje recusa com motivo escrito (`bidirecional.rs:158`) |
| **332** O extrair.py conta docs/.md — e docs/TECNOLOGIAS.md E UM DELES: o ger… | ☐ | FECHAR? | — | consertado pelo pedido 404: `docs/tecnologias/extrair.py:528-541` exclui o `TECNOLOGIAS.md` da própria conta — duplicado |
| **333** PEDIDO DO DONO — chat estilo WhatsApp e robô de mensagens estilo bot… | ☐ | FICA | 3 | PEDIDO DO DONO (chat/robô/IA, onze sprints) |
| **334** PARECER TECNICO EXTERNO 17/09/2026 — o ERRO DE ARITMETICA da soma das… | ☐ | ⏸ | nenhum | o erro de aritmética foi corrigido em 17/09; resta guarda nova de aritmética |
| **335** PARECER EXTERNO — SEIS contradicoes no dossie publicado: o contrato n… | ◐ | FICA | 4(1) | o dossiê ainda se contradiz: «o espaço volta com compactação explícita» (`dossie-phxsql-0.18.html:477`, `:6065`) × «o comando não existe» (`:6012`); a catraca está na mesa do dono |
| **336** PARECER EXTERNO — SIGKILL nao e queda de energia: falta a matriz de c… | ☐ | ⏸ | nenhum | matriz de classes de falha: «obrigação de prova, não defeito afirmado» (texto do pedido) |
| **337** DECISAO DO DONO — o parecer externo corrige o RACIOCINIO do nosso ACI… | ☐ | FICA | 3 | «DECISÃO DO DONO»: a metade (2) entrou no `CLAUDE.md` em 23/09; a (1), o raciocínio do ACID («só o I»), segue lá |
| **338** PARECER EXTERNO — a ordem das prioridades esta INVERTIDA: congelar co… | ☐ | FECHAR? | — | a recomendação (congelar conveniência) é o que o dono decidiu em 24/09 (`c7b3dce`, `CLAUDE.md` «escopo congelado»); conferir antes se cada P0 citado tem pedido próprio |
| **339** PARECER EXTERNO — achados de seguranca e de alcance: chave da API no… | ◐ | FICA | 4(3) | (a) fechou e a §11.3 foi refeita; resta (c), a `.lgpd` como conformidade, que o texto chama de «decisão do dono» |
| **344** SEC 339(b) — replicar coluna EXTERNA marcada esta quebrado mesmo com… | ☐ | FICA | 1 | réplica sem cofre grava o texto cifrado como conteúdo, calada (`reg.rs:1986`, `return Ok(guardado.to_vec())`); e o erro acusa adulteração que não houve |
| **345** SEC 339(b) — o .fts nasce 644 e o .lgpd nasce 600: o apertar_permissa… | ☐ | FICA | 1 | `apertar_permissao` só existe em `trilha.rs:843`; `.fts`, `.reg`, `.ndx`, `.log` e `.trash` nascem 644, legíveis por todo usuário da máquina |
| **348** SEC 339(a) — o esc do explorador.js nao escapa apostrofo: divergencia… | ☐ | ⏸ | nenhum | divergência entre irmãos sem furo hoje (`explorador.js:37-41`) |
| **349** SEC 339(a) PLAUSIVEL — a escotilha do c.formato da grade: o retorno e… | ☐ | ⏸ | nenhum | o único caso aberto (`ultimo`) é `instante_iso` de `i64` (`acesso.rs:119`): sem vetor; resta risco de manutenção |
| **350** SEC/343 — jobs devolve o PEDIDO salvo inteiro, e ele pode carregar va… | ☐ | FICA | dúvida | `jobs` devolve o pedido salvo com valor de coluna marcada a administrador com a coluna negada; pode ser decisão, mas o motivo não está escrito |
| **351** SEC/343 — siscolunas e sistabelas nao trazem colunas_sem_leitura, e a… | ☐ | ⏸ | nenhum | cortesia faltando; «não vaza nada» (texto do pedido) |
| **352** Corrida no teste do par marcado: o parceiro de REPLICACAO cria o data… | ☐ | FICA | 1 | teste que floca (corrida com o parceiro de replicação criando o database) |
| **357** SEC MEDIO-ALTO — a trilha .lgpd redige por NOME de coluna e por anali… | ☐ | FICA | 1 | `SEGURANCA.md` §11.7 diz que a trilha nunca grava o claro de coluna marcada, e `trilha.rs:427` não consulta a marca |
| **359** SEC BAIXOS — o literal de SQL guardado em claro: visoes, rotinas e o… | ☐ | FICA | 1 | `visoes` é `PorColuna::Nenhum` (`direito_coluna.rs:126`) e `Ler` (`usuarios.rs:155`): quem tem `ler` lê o literal da coluna negada |
| **363** O que a frente do 361 nomeou e nao fechou: gravacao do --json nao e a… | ☐ | ⏸ | nenhum | ferramentas do provador (gravação atômica, autoteste na bateria, nota da tranca) |
| **364** Nao existe caminho para REDECLARAR indice de texto numa tabela que ja… | ☐ | FICA | dúvida | `reconstruir_fts` devolve `Ok(0)` anunciando sucesso sobre `.fts` órfão; a redeclaração é capacidade nova |
| **365** O {"op":"sql"} nomeia a tabela DENTRO da frase, e nem a lista nem a m… | ☐ | FICA | 1 | `perfil.txt` publica o INSERT do `sql` com valor de tabela cifrada — o Profiler só cega por `tabela` (`profiler.rs:884`) |
| **368** PARECER DO DBA 18/09 -- o .lgpd nao tem expurgo, e o PRAZO e' decisao… | ☐ | FICA | 3 | o prazo do expurgo do `.lgpd` é «decisão do dono» (título); pendente |
| **369** PARECER DO DBA 18/09 -- baldes[].registros esta na CLASSE DE DIREITO… | ☐ | FICA | 1 | `op_esquema` (`Estrutura`, `direito_coluna.rs:82`) publica `baldes[].registros` (`servidor.rs:18867-18890`): histograma do dado a quem tem a coluna negada |
| **377** As 12 bancadas de replicacao passam a medir COM o tunel no caminho, e… | ☐ | ⏸ | nenhum | mudança de premissa da bancada (túnel no caminho); decisão de quem manda nela |
| **381** Linha com .memo corrompido nao se consegue ALTERAR por cliente que om… | ☐ | FICA | dúvida | `atualizar` sem `softdeleted` é recusado por `.memo` corrompido e com ele grava (`op_atualizar`, `t.ler(rowid)?`): possível recusa indevida; pergunta do DBA |
| **384** O inventario que existe para dizer quantas catracas ha conta 23; a ve… | ◐ | ⏸ | nenhum | o inventário já separa a catraca sem `--numeros`; falta só o número medido do `TETO_TXT_CRU_EM_HTML` |
| **385** A guarda que falta: «cliente novo nasce pedindo o tunel» -- e a forma… | ☐ | ⏸ | nenhum | guarda nova (censo do discador): endurecimento |
| **386** O aviso do batimento tem DUAS respostas para TRES estados, e a tercei… | ☐ | ⏸ | nenhum | `comunicacao.sh` lê mal o estado do batimento: ferramenta de sessão |
| **387** A tela do DbLink nao oferece cifra nem o pino -- a ligacao nao MENTE,… | ☐ | ⏸ | nenhum | tela do DbLink sem `cifra`/pino: «não mente, não conta» |
| **388** O comunicacao.sh manda adiar o PUSH enquanto houver compilação, e iss… | ☐ | ⏸ | nenhum | `comunicacao.sh`: conselho largo demais |
| **390** dblink_ligar e dblink_sincronizar conferem tabela por conta própria e… | ☐ | ⏸ | nenhum | a conferência própria existe e está certa; falta a guarda de porta dos fundos (ver §4) |
| **391** A lei do CLAUDE.md diz «7 das 116 operações» e o medido hoje é 11 de… | ☐ | FICA | 3 | a lei «7 das 116» do `CLAUDE.md` só muda por «decisão dele» (o dono); duplica 420 e 429 |
| **395** O .ndx pode usar o MESMO mecanismo do .fts selado, e a decisao de NAO… | ☐ | ⏸ | nenhum | selar o `.ndx`: pesquisa com premissa a medir; formato de toda tabela (papel C) |
| **397** Gerador para as versoes de arquivo em disco — o CONTRATO-1.0.md publi… | ☐ | FICA | 1 | `CONTRATO-1.0.md:249` e `:494` publicam `PSCH` 7; o código está em 10 (`schema.rs:79`) — o contrato mente sobre o formato |
| **398** Falta um estado no proprio PENDENCIAS.md: «decidido, espera engenhari… | ☐ | ⏸ | nenhum | estado novo no PENDENCIAS: processo (o ⏸ de hoje é da mesma família) |
| **399** O zelador erra por medidor: nao varre o scratchpad, e le worktree com… | ☐ | ⏸ | nenhum | zelador: motivo errado numa recusa certa; ferramenta |
| **400** Versao digitada a mao em tres lugares que o portao da versao nao alca… | ☐ | FICA | 2 | bump da 0.19: `bancada/docker/montar-dois.py:25` e `dblink-mariadb.py:76` fixam `phxsql:0.18.0`, e o `CAPABILITIES.json` publica a versão velha até o gerador rodar |
| **401** Um teste floca sob paralelismo, e «flocado» nao e causa | ☐ | FICA | 1 | teste que floca (corrida do `porta_livre`); frente viva mexendo nisso agora (26 arquivos sujos em `tests/`) |
| **410** O gancho de parada pede commit sem poder ver frente viva — 19 acusaco… | ☐ | ⏸ | nenhum | gancho de parada fora do repositório (`~/.claude`) |
| **411** O dossie principal tem 2,64 MiB — 5,9x o teto de republicacao que o 4… | ☐ | FICA | 2,3 | o dossiê caiu para 460.347 B — 453 B abaixo do teto de 460.800 — e não há guarda de teto no gerador dele; «a decisão do dono passou a bloquear» |
| **412** As duas paginas de status listam os ultimos commits, entao NENHUMA de… | ☐ | FICA | 2 | o portão dos geradores responde VELHO para as duas páginas de status em todo commit; não há o modo `sem-ultimo-commit` em `portao-dos-geradores.py` |
| **413** O desenho do modulo log.rs contradiz o proprio codigo, e o REPLICACAO… | ☐ | FICA | dúvida | `log.rs:13-16` desenha `res u16`/`res u32` que o código gasta, e `REPLICACAO.md:178` promete o campo reservado: documento que mente sobre formato |
| **414** O irmao do 407: acrescentar_coluna TEM porta e NAO tem vista previa —… | ☐ | ⏸ | nenhum | vista prévia do custo no `acrescentar_coluna`: melhoria |
| **415** O alarme do batimento no comunicacao.sh mede QUANDO O SCRIPT RODOU e… | ☐ | ⏸ | nenhum | `comunicacao.sh`: alarme mede o script, não o batimento |
| **416** O DELETE replicado tem o mesmo defeito do 405 e NAO se conserta do me… | ☐ | FICA | dúvida | a exclusão replicada tem a corrida que o 405 fechou na alteração (apaga linha de outra origem); conserto pede formato, papel C, cedo |
| **417** O portao dos geradores roda com cargo vivo e publica o retrato de uma… | ☐ | ⏸ | nenhum | portão dos geradores com frente viva: endurecimento de procedimento |
| **420** A lei do portao de permissao lista «7 das 116 operacoes»; medido sao… | ☐ | FICA | 3 | «Alterar o CLAUDE.md é decisão do dono»: lei «7 das 116» contra 10 de 141 medidos; duplica 391 |
| **421** Suite verde nao quer dizer portoes verdes: as catracas em Python fica… | ☐ | FICA | 2 | catraca em Python fora do portão de commit: o 476 (`6c3fa7d`) criou `bancada/catracas/todas.py` para bateria e batimento, mas o «Antes de commitar» do `CLAUDE.md` segue fmt/clippy/test — conferir se fecha |
| **422** migrar_esquema congela o SERVIDOR INTEIRO por 2,76 s a 2 milhoes de s… | ◐ | FICA | dúvida | a perda de escrita fechou (`97dffc6`); `declarar_fk`/`excluir_fk`/`marcar_lgpd` seguram a trava GLOBAL por segundos, alcançável por quem declara chave; a catraca está verde porque não vê caminho condicional |
| **424** A recusa do ledger mora na DECLARACAO, e a replicacao nao declara: a… | ☐ | FICA | dúvida | a combinação proibida do ledger se espalha por réplica, e `ledger.rs:63`/`FORMATO.md:675` dizem «não desfaz» sem «e acompanha a réplica» |
| **425** A marca do pedido envelhece nos DOIS sentidos, e eu montei a lista co… | ☐ | ⏸ | nenhum | conferidor PENDENCIAS × `git log`: ferramenta (esta classificação achou 6 candidatos a fechar à mão) |
| **427** O retrato da FASE A nao ve volume que NASCE durante ela — e a hipotes… | ☐ | FICA | dúvida | o retrato da FASE A não vê volume novo; no ext4 é fail-stop e o congelamento segura a escrita, mas NTFS/FAT não foram medidos e o produto compila para Windows |
| **428** Tres achados MEDIO do 422: a chave do congelamento nao normaliza CAIX… | ☐ | FICA | 1 | `reg.rs:2629` devolve o caminho absoluto do servidor no erro; a chave do congelamento não normaliza caixa (NTFS/APFS) |
| **429** O inventario do pedido 420 ja nasceu curto: sao seis sitios a mais, e… | ☐ | ⏸ | nenhum | o código está certo nos 6 sítios; o conserto é gerador — funde com 420/391 |
| **430** Ordem de atomicos no congelamento funciona pelo motivo ERRADO, e nenh… | ☐ | ⏸ | nenhum | ordem de atômicos certa hoje pela trava global: comentário, sem defeito |
| **431** A oitava pagina nasceu IMUTAVEL: 2.258.328 B contra um teto de republ… | ☐ | FICA | 2,3 | `console-em-imagens.html` tem 2.258.328 B (5× o teto) e não se republica — publica 2.767 testes contra 2.802; (b) é «decisão do dono» |
| **433** Tres pontas que a frente do 339 mediu e deixou fora do alcance dela,… | ☐ | ⏸ | nenhum | pontas menores: chave gravada na aba antes do teste, guarda de caixa alta em 13 de 114 telas, (c) já destravada |
| **436** Os cinco MEDIOS e os seis BAIXOS da revisao SEC da noite de 23/09 | ◐ | FICA | 4(1) | M5: `perguntar()` (`claude.js:318`) não consulta `oficial(c)` — endpoint na própria origem manda o `x-api-key` ao PhxSql; B5: chave privada Ed25519 fica no DOM após login recusado |
| **437** O teste das threads do SO cai no --workspace e nao cai no binario soz… | ☐ | FICA | 1 | teste que floca sob disputa entre binários de teste (duas funções já) |
| **438** O truncado da composicao existe no protocolo e NENHUMA tela o mostra… | ☐ | FICA | 1 | resultado cortado aparece inteiro na tela e no ODBC: o servidor diz `truncado`, ninguém mostra |
| **440** A catraca de textos nao ve rotulo passado como literal dentro de ${…}… | ☐ | ⏸ | nenhum | a régua de textos não vê rótulo literal em `${…}`: régua nova aposenta a catraca (nunca sobe o 880) |
| **443** MEDIO SEC -- o DbLink MySQL reserva quantas vindo do par, ate 2^64, e… | ☐ | FICA | 1 | `dblink/mysql.rs` reserva `Vec::with_capacity(quantas)` vindo do par: aborto do processo inteiro por par malicioso ou fio em claro |
| **444** Anonimo enche o disco pelo acessos.log: 266 B de log por 2 B recebido… | ☐ | FICA | 1,3 | decidido pelo dono 24/09: rodízio nasce ligado; `config.rs:4324-4325` ainda `sem_rodizio()` — anônimo enche o disco (133×) |
| **445** Os sete BAIXOS da revisao SEC de 434 e 435 | ☐ | FICA | dúvida | B4 publica `tem_pino` na op `config` abaixo do comentário que o recusa; B6 porta web recusa sem linha no `acessos.log` |
| **448** A chave estrangeira da transacao so e conferida DEPOIS da marca -- fi… | ☐ | FICA | 1 | commit com filha órfã no meio da lista grava parte e para: transação confirmada pela metade |
| **449** Durante QUALQUER migracao, todo empilhar do servidor paga a leitura d… | ☐ | ⏸ | nenhum | custo não medido durante migração: desempenho |
| **451** Qualquer panico DENTRO da trava global de dados a deixa envenenada pa… | ☐ | FICA | dúvida | pânico dentro da trava global a envenena: a base inteira responde CORROMPIDO até reiniciar; gatilho conhecido fechado (446) |
| **452** A thread de pulso que morre por panico nao se desmarca do pulsando: o… | ☐ | FICA | dúvida | thread de pulso morta por pânico não se desmarca: o par nunca mais é pulsado; gatilho conhecido fechado (446) |
| **454** PEDIDO DO DONO, 24/09/2026 -- PhxZip e PRODUTO: interface web numa po… | ☐ | FICA | 3 | PEDIDO DO DONO 24/09 (PhxZip como produto) |
| **455** PEDIDO DO DONO, 24/09/2026 -- o pacote so do PhxZip completo, a inter… | ◐ | FICA | 4(3) | PEDIDO DO DONO 24/09 (pacote, interface e manual do PhxZip): faltam servidor, cmd, manual e pacote |
| **458** O SP000010 sai com a MESMA frase de 85 pontos em 14 Mutex do servidor… | ☐ | FICA | dúvida | pânico com `transacoes` na mão mata toda transação de toda conexão até reiniciar (família do 451) |
| **459** FK conferida na INSTRUCAO, e nao no COMMIT -- o segundo passo do 448,… | ☐ | ⏸ | nenhum | FK na instrução em vez do COMMIT: alinhamento de semântica depois do 448, que já segura a integridade |
| **460** A trava na MAE durante a transacao da filha -- quem perde hoje e a fi… | ☐ | ⏸ | nenhum | quem perde (mãe × filha) diverge dos maduros; a integridade fica com o 448 |
| **461** JSON malformado nao conta como tentativa leve: o ataque do 444 nem ch… | ☐ | ⏸ | nenhum | contador de anti-abuso novo, nasce desligado: endurecimento |
| **462** A recusa da expressao ecoa o texto INTEIRO do onde que veio do fio | ☐ | FICA | dúvida | a recusa ecoa o `onde` inteiro, que vai à resposta, ao `acessos.log` e ao Profiler (`SEGURANCA.md` §22.3): amplificação e literal de coluna marcada no log |
| **463** O cliente SMTP nao tem teto de QUANTAS linhas de continuacao (250-...… | ☐ | FICA | dúvida | relé SMTP que manda continuação sem fim segura a thread de aviso para sempre: o alarme de disco cala |
| **464** Dado pessoal curto continua citado na recusa de conversao | ☐ | FICA | 1 | valor curto de coluna marcada sai inteiro no erro de conversão, que vai ao `acessos.log` e ao Profiler (`SEGURANCA.md` §22.3) |
| **465** O maior consumidor de disco que sobra nao e cache: sao 27 binarios de… | ☐ | ⏸ | nenhum | 27 binários de teste somam 1.906 MiB: desenho da suíte, disco |
| **466** Cadastro do DbLink ilegivel derruba o MOTOR inteiro: o DbLink e acess… | ☐ | FICA | 1 | cadastro do DbLink ilegível impede o `phxsqld` de subir (`servidor.rs:1285`, `Registro::abrir_com(...)?`) |
| **467** Nenhum rename da casa faz fsync do DIRETORIO depois: a troca atomica… | ☐ | FICA | 1 | nenhum `rename` faz `fsync` do diretório: a troca atômica não é durável (config, dblink, marca a medir) |
| **468** Trocar a chave mestra do DbLink nao tem caminho: recifrar com a velha… | ☐ | FICA | dúvida | com a chave mestra do DbLink perdida não há saída pela tela e toda credencial nova em claro é recusada; C julgou aceitável como pedido |
| **469** A migracao do dblink.json so acontece na primeira gravacao: declarar… | ☐ | ⏸ | nenhum | migração do `dblink.json` só na 1ª gravação: comportamento decidido e avisado no arranque |
| **470** Trocar o host de uma ligacao do DbLink sem mandar a senha HERDA a sen… | ☐ | FICA | 1 | `dblink_salvar` com host novo e sem senha herda a senha (`servidor.rs:22856-22857`): sessão roubada leva a credencial para fora |
| **472** O irmao do 456 no .fts: panico entre o .reg e o indexar_texto deixa a… | ☐ | FICA | dúvida | pânico entre o `.reg` e o `indexar_texto` deixa o `.fts` atrás e marcado limpo: a busca perde a linha calada; sem gatilho conhecido |
| **473** A recusa do ao_alterar diz «o arquivo esta SAO: nao repare nada» quan… | ☐ | FICA | 1 | `table.rs:2461` (e `:1804`) manda «o arquivo está SÃO: não repare nada» justamente quando reparar é o conserto |
| **479** Nada confere a permissao nem o dono do config no arranque -- e a aute… | ☐ | ⏸ | nenhum | conferir permissão e dono do config no arranque: endurecimento, «já era assim com o claro» |
| **480** O par config.<ext> ↔ config.phz nao e simetrico fora de .json: a edic… | ☐ | ⏸ | nenhum | assimetria do par fora de `.json`: «alcance hoje zero» |
| **481** O CONFLITO do par vira negacao de servico em pasta com sticky bit, e… | ☐ | FICA | dúvida | o CONFLITO do par (novo nesta versão, 450) vira negação de serviço em pasta com sticky bit; falta medir se alguma instalação documentada a usa |
| **482** O log dos jobs deriva o nome por with_extension("log"): um cadastro c… | ☐ | FICA | 1 | `jobs.rs:574` `with_extension("log")`: cadastro `agenda.log` usa o próprio arquivo como log e se corrompe |
| **483** O phxsqld ignora flag que nao conhece e sobe como SERVIDOR: o binario… | ☑️ | — | — | fechado em `f1597be` depois do pedido desta classificação; fora das contas |

## 3. As dúvidas (24) — ficam na conta até alguém decidir

- **268** — `cifra.tabelas` declara mais do que o disco cumpre para tabela que já existe (a tela avisa); garantia de cifra que não vale no gravado, e formato entra cedo
- **276** — `catalogo.rs:1590` declara só `token` no `replicacao_testar`, que lê `token_remoto` primeiro (`origem_da_sonda`): a spec publicada da op está errada; `prova`/`assinatura` fora do `LEXICO` (`debug-com-segredo.py:133`)
- **297** — `INTEGRIDADE.md` §3 promete fidelidade e não diz que `.trash`/`.reason` divergem na réplica (zero menção a lixeira no arquivo); o conserto mínimo é uma frase
- **326** — o link compartilhado mostra versão fixada e velha; nossa metade existe (`numeros-do-projeto.py:214`, `selo_da_capa`), resta ação do dono no compartilhamento
- **329** — perna do pedido do dono 325; colisão do `hash_id` de 16 bits (`bidirecional.rs:70`) só morde em N nós, ainda não suportado
- **330** — perna do 325 («imediata»); custo do 1º arranque ainda a medir
- **331** — perna do 325 (itens de venda com chave composta); hoje recusa com motivo escrito (`bidirecional.rs:158`)
- **350** — `jobs` devolve o pedido salvo com valor de coluna marcada a administrador com a coluna negada; pode ser decisão, mas o motivo não está escrito
- **364** — `reconstruir_fts` devolve `Ok(0)` anunciando sucesso sobre `.fts` órfão; a redeclaração é capacidade nova
- **381** — `atualizar` sem `softdeleted` é recusado por `.memo` corrompido e com ele grava (`op_atualizar`, `t.ler(rowid)?`): possível recusa indevida; pergunta do DBA
- **413** — `log.rs:13-16` desenha `res u16`/`res u32` que o código gasta, e `REPLICACAO.md:178` promete o campo reservado: documento que mente sobre formato
- **416** — a exclusão replicada tem a corrida que o 405 fechou na alteração (apaga linha de outra origem); conserto pede formato, papel C, cedo
- **422** — a perda de escrita fechou (`97dffc6`); `declarar_fk`/`excluir_fk`/`marcar_lgpd` seguram a trava GLOBAL por segundos, alcançável por quem declara chave; a catraca está verde porque não vê caminho condicional
- **424** — a combinação proibida do ledger se espalha por réplica, e `ledger.rs:63`/`FORMATO.md:675` dizem «não desfaz» sem «e acompanha a réplica»
- **427** — o retrato da FASE A não vê volume novo; no ext4 é fail-stop e o congelamento segura a escrita, mas NTFS/FAT não foram medidos e o produto compila para Windows
- **445** — B4 publica `tem_pino` na op `config` abaixo do comentário que o recusa; B6 porta web recusa sem linha no `acessos.log`
- **451** — pânico dentro da trava global a envenena: a base inteira responde CORROMPIDO até reiniciar; gatilho conhecido fechado (446)
- **452** — thread de pulso morta por pânico não se desmarca: o par nunca mais é pulsado; gatilho conhecido fechado (446)
- **458** — pânico com `transacoes` na mão mata toda transação de toda conexão até reiniciar (família do 451)
- **462** — a recusa ecoa o `onde` inteiro, que vai à resposta, ao `acessos.log` e ao Profiler (`SEGURANCA.md` §22.3): amplificação e literal de coluna marcada no log
- **463** — relé SMTP que manda continuação sem fim segura a thread de aviso para sempre: o alarme de disco cala
- **468** — com a chave mestra do DbLink perdida não há saída pela tela e toda credencial nova em claro é recusada; C julgou aceitável como pedido
- **472** — pânico entre o `.reg` e o `indexar_texto` deixa o `.fts` atrás e marcado limpo: a busca perde a linha calada; sem gatilho conhecido
- **481** — o CONFLITO do par (novo nesta versão, 450) vira negação de serviço em pasta com sticky bit; falta medir se alguma instalação documentada a usa

## 4. Ordens e decisões do dono que eu adiaria — e que NÃO proponho como `⏸`

Ficam na conta pelo critério (3). 29 pedidos têm (3); 22 ficam SÓ por ele, e destes 15 não carregam defeito medido. A decisão é do dono, não desta classificação:

- **construir capacidade nova** — 333, 207, 197, 294, 324, 454, 455: o 333 o próprio parecer externo (338) já rebaixou a P2; 454/455 são do mesmo dia do congelamento — só o dono diz se o PhxZip é da 0.19.
- **pergunta ou parada pelo próprio dono** — 175, 251, 255, 309, 368: uma linha do dono («depois da versão») as tira; nenhuma tem defeito medido no dado de hoje.
- **prova e processo** — 190, 238, 263: prova melhor de coisa que funciona (botões, sonda viva ODBC, provador inteiro).
- As pernas do 325 (329, 330, 331) estão como «dúvida» e seguem o 325; o 325 fica de qualquer jeito pelo (1) — a perna (5) perde venda hoje (229a, 290).
- **Baratos e já decididos, que eu NÃO adiaria**: 245 (O2), 249 (60 → 300 s em `config.rs:1122`), 259, 337 (1), 339 (c), 391/420 (o texto do `CLAUDE.md`).

Se o dono mover os 15: fica 69 / (342 + 6 + 69) = **16,5%**.

## 5. Duplicatas e famílias (fundir reduz a conta sem esconder nada)

- **391 = 420 = 429** — a mesma lei «7 das 116», recontada três vezes com três números (11/140, 10/141, +6 sítios). O conserto que as três pedem é um gerador.
- **332 = 404** (☑️) — o `extrair.py` que se conta; `FECHAR?`.
- **421 ≈ 476** (☑️) — catraca Python fora do portão; o 476 entregou o comando, falta ele estar no «Antes de commitar».
- **321, 352, 401, 437** — teste que floca: espera por relógio contra trabalho de outra máquina, porta sorteada e solta, disputa de CPU entre binários. Quatro linhas, três causas.
- **451, 452, 458, 472** — pânico que deixa trava envenenada ou marca limpa mentindo; o gatilho conhecido fechou no 446, a fragilidade não.

## 6. QA (papel G): a mudança em revisão contra as catracas e as guardas

**Catraca que subiria:** nenhuma. Nenhuma constante `TETO*` conta pedido aberto (`git grep TETO` nos geradores de `docs/dossie`, `docs/pmo`, `docs/status`: só os tetos de republicação). O que muda é a **régua da porcentagem**, que passa a medir menos: 29,2% → 19,4% sem uma linha de código. Pela lei da catraca isso não é descida — é régua nova, e a página tem de dizer que o denominador mudou em 24/09, senão a série antiga e a nova se comparam como se fossem a mesma. E o caminho inverso fica nomeado: um `⏸` que volte a `☐` por virar defeito sobe a porcentagem, e isso é achado, não regressão.

**Onde o `⏸` quebra calado — conferir ANTES de aplicar:** o estado mora em dois leitores, não em um.

- `docs/dossie/pagina-dos-pedidos.py:70-82` (`ESTADOS`, `LINHA`) — com `⏸` fora da legenda, a guarda do pedido 150 (`QUALQUER`) **para** o gerador. Certo; o PMO, a página de status e a planilha importam este `ler()` e herdam a guarda.
- `docs/tecnologias/extrair.py:816-834` (`LINHA_PEDIDO`, `ler_pedidos`) — **cópia** do mesmo leitor, com os três estados e **sem** a guarda: a linha `⏸` some calada, e o `TECNOLOGIAS.md` publica «tem N pedidos numerados» com N curto. É o defeito do pedido 150 pela porta irmã, e é a pétrea «função e comando não se duplicam» (23/09) já violada antes do `⏸`: o conserto é o `extrair.py` importar o `ler()`, não ganhar um quarto símbolo na sua regex.
- `docs/dossie/prova-do-leitor-de-pedidos.py` usa `⏳` como símbolo fora da legenda; continua valendo com `⏸` dentro. Prova real exigida: `⏸` fora da legenda para os dois leitores; dentro dela, a contagem muda só para quem o usa (é o 398).

**Pétreas sem guarda que a leitura dos 141 expôs** — não é o inventário guarda × pétrea completo, que **não foi feito** nesta frente:

- «Portão de permissão é UM só»: `dblink_ligar`/`dblink_sincronizar` conferem certo e **não têm** o teste de porta dos fundos (390); e a lista da própria lei está curta (391/420/429).
- «Função e comando não se duplicam»: o leitor do PENDENCIAS em duas cópias (acima); o Portão 4 lendo um campo só (322) tem de reusar `tabelas_do_pedido`, nunca uma quinta lista.
- «Catraca só desce»: catracas Python fora do portão de commit (421); `alcancam-fsync-2` verde com três irmãs segurando a trava, porque o mapeador não vê caminho condicional (422); `TETO_ROTULOS_E_CRASE` cego a literal em `${…}` — régua nova aposenta a catraca, nunca sobe o 880 (440); o dossiê a **453 B** do teto de republicação sem teto no gerador dele (411) e a `console-em-imagens.html` sem teto nenhum (431).
- «Redige ANALISANDO, nunca recortando»: o `sql` no Profiler (365), o eco de erro no `acessos.log` (462, 464) e a trilha que redige por nome (357) — nenhuma guarda cobre o caminho do `sql`.
- «Guarda nova entra pedida»: o 444 inverte um padrão por decisão do dono; o teste do comportamento velho (`sem_as_secoes_acessos_e_diretivas_nascem_sem_rodizio`) muda de lado e tem de sair aposentado com o motivo, não editado calado.
- «Só existe filho se o pai existir»: commit que grava metade da lista (448) e réplica sem contador de órfãs (300).

**Achado de processo, com número:** esta classificação achou à mão 6 candidatos a `FECHAR?` entre 141 abertos, e o 483 fechou no meio dela — é o número que sustenta o 425 (conferidor PENDENCIAS × `git log`).

