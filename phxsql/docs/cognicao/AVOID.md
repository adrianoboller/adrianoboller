# AVOID -- o que ja falhou aqui, e por que

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** INFRUTIFERO` -- 5 hoje, de 357 cognicoes no total.

## Dono de arquivo é sinal FORTE, não um palpite como data ou conteúdo

- Causa: a hipótese «dono diferente do processo e do parceiro = terceiro» tomou o sinal que não se FORJA pelo sinal que IDENTIFICA o intruso. O root também é outro dono, e é o administrador: na instalação do MANUAL (§7.4), `sudo 7z x` para trocar um token vazado deixa o `.json` do root ao lado do `.phz` do serviço, e o serviço subia do `.phz` VELHO com o token revogado valendo — a revisão SEC provou pelo sistema operacional, e o teste `crates/phxsql-server/tests/config-phz.rs::o_json_do_root_ao_lado_do_phz_do_servico_recusa_o_arranque` cai com esta regra reposta («ainda rodava depois de 20 s: subiu como servidor», o servidor como uid 65534). O sticky bit, que é o que torna um nome alheio «plantado», nem era conferido. E a régua citada media outra coisa: MySQL e MariaDB ignoram por MODO (gravável por todos); medido, o `mysqld` 8.0.46 LÊ um `my.cnf` de outro dono com 0644.
- Prevencao: antes de usar um metadado como prova de intruso, liste QUEM MAIS produz o mesmo sinal legitimamente (o root, o dono da pasta, o próprio serviço) e exija a condição do sistema operacional que torna o sinal exclusivo do intruso (aqui: sticky bit E pasta gravável por outros, com o root e o dono da pasta fora da conta de terceiro); e ao citar outro motor na régua, cite o CRITÉRIO dele medido pelo binário, não só o comportamento.
- Arquivo: [cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md](cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md)

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
