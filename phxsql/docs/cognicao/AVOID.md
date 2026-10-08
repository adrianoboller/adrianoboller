# AVOID -- o que ja falhou aqui, e por que

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** INFRUTIFERO` -- 10 hoje, de 428 cognicoes no total.

## «O branch da frente é ancestral do HEAD» não prova que o trabalho dela foi integrado

- Causa: a limpeza de cópias de trabalho decidia «integrada» por `git merge-base --is-ancestor worktree-agent-X HEAD`. As frentes NÃO comitam — a regra da casa é que só o integrador comita —, então o branch de toda frente fica parado no commit de onde ela saiu, e esse commit é SEMPRE ancestral do HEAD. A conferência dizia «integrada» para frente cujo trabalho inteiro morava só na árvore de trabalho, e o `git worktree remove -f -f` o apagou: em 01/10/2026, quatro frentes prontas e com portões verdes (513 passo 1, 329+331, a catraca do 335, 600+601+245) sumiram assim. A prova de «nenhum processo usa» (cwd, descritores, mapas) passou — ela responde outra pergunta.
- Prevencao: antes de apagar uma cópia de trabalho, exigir as DUAS coisas: `git -C <copia> status --porcelain` vazio (nada fora de commit) E o commit da ponta alcançável do HEAD. Árvore suja nunca é «integrada», seja qual for o grafo. E nunca `-f -f` numa cópia que a conferência não provou limpa: o `-f` existe para pular exatamente a proteção que teria parado o erro. Por ordem do dono, além disso, o trabalho de toda frente é salvo como objeto do git (`phxsql/salvar-frentes.sh`, `refs/salvas/`) a cada 10 minutos e antes de toda limpeza, e só `phxsql/limpar-frentes.sh` apaga cópia de frente.
- Arquivo: [cognicao_ancestral-do-head-nao-prova-que-o-trabalho-foi-integrado_20261001_0500.md](cognicao_ancestral-do-head-nao-prova-que-o-trabalho-foi-integrado_20261001_0500.md)

## A conferência da réplica fiel não mora na função que o bidirecional também chama

- Causa: a conferência da linhagem entrou em `garantir_tabela_da_replica` pelo nome da função, e o `abrir_para_bidi` também a chama — 6 testes de soquete caíram (5 do bidirecional, onde a linhagem diverge legitimamente).
- Prevencao: antes de pôr uma conferência numa função, listar quem a chama; a da réplica fiel mora em `recusa_da_linhagem`, chamada só do `abrir_para_replicar`, e sai pelo `romper_continuidade`.
- Arquivo: [cognicao_conferencia-da-replica-fiel-nao-mora-na-funcao-que-o-bidi-tambem-chama_20261001_1500.md](cognicao_conferencia-da-replica-fiel-nao-mora-na-funcao-que-o-bidi-tambem-chama_20261001_1500.md)

## Contradição consertada à mão sobrevive no arquivo que ninguém releu

- Causa: o conserto das seis contradições do parecer (commit `2c77e6fe`) foi feito por leitura humana das frases que o parecer citou, e a busca parou nas redações citadas; a mesma capacidade dita com outra forma («traduz um `SELECT` simples» no `FORMATO.md`, «Compactação … | pendente» numa tabela do `README.md`) não era uma das frases citadas e ficou.
- Prevencao: conserto de contradição de contrato fecha com `python3 docs/dossie/catraca-prosa-x-celula.py` verde, e não com a lista do parecer riscada; a catraca roda no fecho do `portao-dos-geradores.py` e no `bancada/catracas/todas.py`.
- Arquivo: [cognicao_contradicao-consertada-a-mao-sobrevive-fora-do-arquivo-lido_20261001_0413.md](cognicao_contradicao-consertada-a-mao-sobrevive-fora-do-arquivo-lido_20261001_0413.md)

## Derrubar um sistema de arquivos pelo CAMINHO derruba o do contêiner quando a montagem falha calada

- Causa: para medir o pedido 605 (o `fsync` de criar tabela protege um terceiro?), a frente escreveu `bancada/catastrofes/terceiro-605.sh`: monta um ext4 sobre loop e simula a queda com `FS_IOC_SHUTDOWN` + `EXT4_GOING_FLAGS_NOLOGFLUSH` no ponto de montagem. O script não tinha `set -e` nem conferia a montagem. A montagem falhou sem ninguém ver, o ponto de montagem continuou sendo um diretório comum do `/dev/vda`, e o ioctl derrubou o ext4 RAIZ do contêiner sem descarregar o cache. Todo `open` passou a devolver EIO, para todas as frentes e para o integrador, até o contêiner ser reiniciado.
- Prevencao: o alvo de todo ioctl destrutivo se confere pelo DISPOSITIVO, não pelo caminho: `st_dev` do alvo diferente do `st_dev` de `/` e igual ao do loop recém-montado, e `mountpoint -q` antes de cada derrubada, com `set -e`. Sem essas três conferências o script não roda. Melhor ainda: queda simulada só em VM descartável. O script ficou DESARMADO (primeira linha sai com 99) no ramo da frente, e não se integra sem as conferências.
- Arquivo: [cognicao_derrubar-o-fs-pelo-caminho-derruba-o-do-conteiner_20261001_0930.md](cognicao_derrubar-o-fs-pelo-caminho-derruba-o-do-conteiner_20261001_0930.md)

## Dono de arquivo é sinal FORTE, não um palpite como data ou conteúdo

- Causa: a hipótese «dono diferente do processo e do parceiro = terceiro» tomou o sinal que não se FORJA pelo sinal que IDENTIFICA o intruso. O root também é outro dono, e é o administrador: na instalação do MANUAL (§7.4), `sudo 7z x` para trocar um token vazado deixa o `.json` do root ao lado do `.phz` do serviço, e o serviço subia do `.phz` VELHO com o token revogado valendo — a revisão SEC provou pelo sistema operacional, e o teste `crates/phxsql-server/tests/config-phz.rs::o_json_do_root_ao_lado_do_phz_do_servico_recusa_o_arranque` cai com esta regra reposta («ainda rodava depois de 20 s: subiu como servidor», o servidor como uid 65534). O sticky bit, que é o que torna um nome alheio «plantado», nem era conferido. E a régua citada media outra coisa: MySQL e MariaDB ignoram por MODO (gravável por todos); medido, o `mysqld` 8.0.46 LÊ um `my.cnf` de outro dono com 0644.
- Prevencao: antes de usar um metadado como prova de intruso, liste QUEM MAIS produz o mesmo sinal legitimamente (o root, o dono da pasta, o próprio serviço) e exija a condição do sistema operacional que torna o sinal exclusivo do intruso (aqui: sticky bit E pasta gravável por outros, com o root e o dono da pasta fora da conta de terceiro); e ao citar outro motor na régua, cite o CRITÉRIO dele medido pelo binário, não só o comportamento.
- Arquivo: [cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md](cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md)

## «Mesmo julgamento do `conferir_filhas`» copiado espalha a falha aberta

- Causa: a decisão «irmã que não abre fica de fora, para uma tabela quebrada não trancar o banco» respondia «ninguém aponta para mim» pela irmã que não responde; ela estava escrita em quatro lugares (`irmas.rs`, `planejar_ao_alterar_com`, `quem_aponta_para`, e o comentário do `fks_que_apontam_para_mim`), três deles justificados pelo comentário «mesmo julgamento do `conferir_filhas`» — e a falha aberta viajou junto com a cópia. Medido: com o `.reg` da filha truncado a 16 bytes, a mãe com filha saiu de vez, suave, pelo `excluir_tabela` e pela troca de chave.
- Prevencao: a decisão mora numa função só (`irmas::abrir_irma`), que recusa nomeando a irmã e a causa; as três buscas passam por ela, e cada uma tem guarda própria no catálogo (`irma-que-nao-abre-some-do-*`). A saída da recusa é o `excluir_tabela` da própria irmã, que pula a si mesma; a irmã em troca interrompida abre pelo `RegFile::abrir` (guarda `irma-em-troca-vira-recusa-eterna`).
- Arquivo: [cognicao_mesmo-julgamento-copiado-espalha-a-falha-aberta_20261001_1910.md](cognicao_mesmo-julgamento-copiado-espalha-a-falha-aberta_20261001_1910.md)

## Rodada de mutantes com a base vermelha: onze «vermelhos» que não provavam nada

- Causa: o roteiro de mutantes chamava `cargo test -p phxsql-core --lib tls aes` — dois filtros posicionais, que o `cargo` recusa com erro de uso. Todo mutante saía com código ≠ 0 e era contado como «vermelho».
- Prevencao: o roteiro roda o MESMO comando sem mutante primeiro e para se a base não sair verde (`assert base.returncode==0`), imprimindo o `test result` da base. Hoje em `/tmp/claude-0/mut_aes.py`; a regra vale para todo roteiro de mutante.
- Arquivo: [cognicao_mutante-com-base-vermelha-mente-verde_20260930_1720.md](cognicao_mutante-com-base-vermelha-mente-verde_20260930_1720.md)

## Portão verde não substitui o parecer do DBA num lote de risco

- Causa: o integrador comitou o lote integridade 2 (`18f1575`) com os quatro portões verdes na árvore exata e a revisão do DBA ainda correndo. O parecer chegou 35 minutos depois e bloqueou o 540, medido: pela sincronia do DbLink o índice único da mãe se corrompia em 5 de 5 rodadas (0 de 5 na base). Nenhum portão podia ver isso: a suíte não tinha teste que inserisse e cascateasse no mesmo punho, que era justamente o terceiro chamador que a frente não olhou.
- Prevencao: lote que toca formato em disco, concorrência ou garantia de dado só vai ao commit depois do parecer do papel que o revisa; a pressão para comitar se atende comitando o que não depende dele (registro do juiz, MODELOS, pareceres), por caminho. Se o parecer bloquear depois de um commit, o pedido volta a parcial no commit seguinte, com o número, em vez de continuar dizendo feito.
- Arquivo: [cognicao_portao-verde-nao-substitui-o-parecer-do-lote-de-risco_20260924_2036.md](cognicao_portao-verde-nao-substitui-o-parecer-do-lote-de-risco_20260924_2036.md)

## O rascunho da sessão é dividido entre agentes: diretório com nome genérico é de outro

- Causa: extraí um `git archive` do `HEAD` em `scratchpad/antes/` supondo o rascunho só meu; ele já tinha uma árvore `antes/phxsql` de outra frente (datas de 10:58, com `target/`), e o `tar -x` sobrescreveu o `crates/`, o `Cargo.toml` e o `Cargo.lock` dela. O mesmo rascunho tinha `arvore-antes`, `mutantes.py`, `provar.sh` e uma montagem minha esquecida em `mnt/`.
- Prevencao: antes de escrever no rascunho, listar o que já está lá; e trabalhar sempre num subdiretório com o número do pedido (`p498/`), nunca num nome genérico como `antes/`, `mnt/` ou `prova.txt`. Montagem de teste se desmonta no mesmo passo em que nasce.
- Arquivo: [cognicao_rascunho-da-sessao-e-dividido-entre-agentes_20260930_1655.md](cognicao_rascunho-da-sessao-e-dividido-entre-agentes_20260930_1655.md)

## Pipe escapado duas vezes numa regex: três fechamentos colaram no título

- Causa: para achar a linha do pedido no `PENDENCIAS.md`, escrevi `r"^\\| [^|]+ \\| 350 \\|.*$"` num script Python dentro de um heredoc de shell. Numa string crua, `\\|` vira `\\` seguido de `|`: uma barra literal **ou** o resto do padrão. O ramo vazio casou o começo do arquivo, e o `re.sub` colou o fechamento na linha 1. Isso aconteceu três vezes (pedidos 350, 255 e 575), e os três continuaram ☐.
- Prevencao: - O `pagina-dos-pedidos.py` recusa, com o motivo, estado de pedido grudado fora do começo da linha, e também título que não esteja na linha 1 (`ESTADO_GRUDADO`). A guarda foi provada contra o arquivo quebrado do commit `163cdeff`, que ela recusa, e contra o consertado, que ela aceita. - Nos scripts de edição, o padrão sai de uma string crua de **uma** barra (`r"^\| "`). Depois de escrever, confere-se o estado relido da própria linha. Não se confia no fato de o script não ter dado erro.
- Arquivo: [cognicao_regex-de-pipe-escapado-duas-vezes-colou-no-titulo_20260930_1620.md](cognicao_regex-de-pipe-escapado-duas-vezes-colou-no-titulo_20260930_1620.md)
