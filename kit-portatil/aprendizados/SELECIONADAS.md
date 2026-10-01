# Aprendizados que servem a qualquer projeto — com a evidência já escrita

Curadoria de 01/10/2026 sobre a triagem mecânica `CANDIDATAS.md` (gerada por
`python3 kit-portatil/extrair.py`; os números de lá saem do extrator, não
daqui).

**O que entrou:** cognição cujo TEXTO já traz a evidência — teste ou commit
que existe e foi conferido pelo extrator, **e** a medição nos dois sentidos
escrita (o que caiu com o defeito reposto, o que passou com o conserto) — e
cuja regra não depende do motor de dados.

**O que isto NÃO é:** promoção. No projeto de origem, 350 das 363 seguem
PENDENTE; o estado delas só muda no próprio arquivo, por quem tiver a prova
validada e escrita no campo `**Evidência:**`. Aqui a marca é só «a evidência
está no texto»; quem copiar para outro projeto **re-prova lá**.

Prefixo dos links: `../../phxsql/docs/cognicao/`.

## Reuse — regras com prova escrita (35)

### Como provar que a prova pega

| # | Cognição | Estado na origem | Por que serve fora do PhxSql |
|---|---|---|---|
| 1 | [Rodada de mutantes com a base vermelha](../../phxsql/docs/cognicao/cognicao_mutante-com-base-vermelha-mente-verde_20260930_1720.md) | INFRUTÍFERO | Todo teste de mutação precisa do comando sem mutante verde na mesma corrida; vale para qualquer ferramenta de teste. |
| 2 | [Defeito reposto que não repõe](../../phxsql/docs/cognicao/cognicao_defeito-reposto-que-nao-repoe_20260916_0801.md) | PENDENTE | Repor um defeito é repor o ESTADO, não apagar a linha que o evita; vale para qualquer mutação manual. |
| 3 | [A prova real passou verde: defeito reposto em território de teste](../../phxsql/docs/cognicao/cognicao_repor-o-defeito-depois-do-cfg-test-nao-e-repor_20260916_1325.md) | PENDENTE | Em linguagem com teste no mesmo arquivo, o fim do arquivo é teste; mutar ali não muta produção. |
| 4 | [A premissa conferida DENTRO do teste da guarda dispara antes do dano](../../phxsql/docs/cognicao/cognicao_premissa-dentro-do-teste-da-guarda-dispara-antes-do-dano_20260917_0902.md) | PENDENTE | Premissa em teste próprio; senão o vermelho vem da premissa e o relatório diz «provada». |
| 5 | [O vermelho saiu pela guarda vizinha, e escondeu o dano](../../phxsql/docs/cognicao/cognicao_o-vermelho-saiu-pela-guarda-vizinha-e-escondeu-o-dano_20260924_0140.md) | PENDENTE | Medir o dano depois de cada passo, antes do veredito; vale para qualquer teste de várias etapas. |
| 6 | [Prova real que morre no preparo](../../phxsql/docs/cognicao/cognicao_prova-real-que-morre-no-preparo-nao-prova-o-caminho_20260918_1311.md) | PENDENTE | A linha do vermelho tem de ser a do caminho sob prova; preparo por outro caminho. |
| 7 | [A recusa TARDIA encobre o portão ausente](../../phxsql/docs/cognicao/cognicao_recusa-tardia-encobre-portao-ausente_20260923_0440.md) | PENDENTE | Teste de controle de acesso escolhe o caso sem segunda barreira, senão passa com o portão fora. |
| 8 | [Atalho de desempenho sem teste que o distingue](../../phxsql/docs/cognicao/cognicao_atalho-de-desempenho-sem-teste-que-o-distingue_20260924_1050.md) | PENDENTE | Guarda só entra depois de VER o vermelho à mão; otimização pura pode não ter teste capaz de sentir a falta. |
| 9 | [A régua do catálogo conferia o conteúdo e nunca a forma](../../phxsql/docs/cognicao/cognicao_a-regua-do-catalogo-nao-olhava-a-forma-da-entrada_20260916_1752.md) | PENDENTE | Validador de catálogo confere a forma da entrada antes do conteúdo, com as chaves tiradas do próprio catálogo. |
| 10 | [A última prova vermelha virar verde quebrou o conferidor](../../phxsql/docs/cognicao/cognicao_a-ultima-vermelha-verde-quebra-o-conferidor-das-vermelhas_20260910.md) | PENDENTE | Detector se prova contra fixture sintético, não contra a árvore — senão pune o sucesso de zerar. |
| 11 | [Catraca que nasce em zero não distingue régua morta](../../phxsql/docs/cognicao/cognicao_catraca-que-nasce-em-zero-nao-distingue-regua-morta_20260917_0038.md) | PENDENTE | Toda régua numérica roda o autoteste antes de responder; zero é também o que régua quebrada devolve. |
| 12 | [Conferidor que discorda de conferidor acalma](../../phxsql/docs/cognicao/cognicao_conferidor-que-discorda-de-conferidor-acalma_20260904_0058.md) | PENDENTE | Duas ferramentas discordando: a verde é a suspeita, não o álibi. |
| 13 | [Guarda que só se confere em uma hora](../../phxsql/docs/cognicao/cognicao_guarda-que-so-se-confere-em-uma-hora_20260916_1245.md) | PENDENTE | Catálogo de mutação envelhece com o código; conferir em lote periódico e não remendar o trecho sem re-provar. |
| 14 | [Guarda de diretório só pega o que o teste fabrica](../../phxsql/docs/cognicao/cognicao_guarda-de-diretorio-so-pega-o-que-o-teste-fabrica_20260907_1650.md) | PENDENTE | «Confere o diretório, não a lista» só cobre o que o setup cria; extensão nova fica invisível. |
| 15 | [A guarda do teto provava a máquina, e não a constante](../../phxsql/docs/cognicao/cognicao_guarda-do-teto-prova-a-maquina-e-nao-a-constante_20260917_0605.md) | PENDENTE | Teto testado com valor passado à mão prova o mecanismo; a prova entra pela porta dos chamadores. |
| 16 | [O método existia e ninguém o chamava](../../phxsql/docs/cognicao/cognicao_o-metodo-existia-e-ninguem-o-chamava_20260907_1420.md) | PENDENTE | Toda guarda de ausência («o segredo não sai») precisa da irmã de presença («o campo sai»). |
| 17 | [Corpo de falso positivo de uma fonte só mede essa fonte](../../phxsql/docs/cognicao/cognicao_corpo-de-falso-positivo-de-uma-fonte-so-mede-essa-fonte_20260924_1340.md) | FRUTÍFERO | Corpo de casos legítimos sai do repositório inteiro por extrator, não de uma fonte escolhida. |
| 18 | [Prova diferencial contra o HEAD expira no próprio commit](../../phxsql/docs/cognicao/cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md) | FRUTÍFERO | Prova «antes × depois» fixa o antes num commit com nome, nunca no `HEAD`. |

### Concorrência, processo e sistema operacional

| # | Cognição | Estado na origem | Por que serve fora do PhxSql |
|---|---|---|---|
| 19 | [A linha lida de outro processo só vale com o `\n`](../../phxsql/docs/cognicao/cognicao_eprintln-nao-e-uma-escrita-so-a-linha-so-vale-com-o-fim_20260930_2017.md) | FRUTÍFERO | Ler saída de processo filho: só a linha terminada vale; a janela é rara e se prova com servidor falso. |
| 20 | [Estado global ao alcance do teste vizinho](../../phxsql/docs/cognicao/cognicao_estado-global-ao-alcance-do-teste-vizinho_20260916_1045.md) | PENDENTE | Suíte paralela: teste monta estado local, nunca escreve o global do módulo (54 falhas em 1.000 sob carga). |
| 21 | [Estado global de processo entre testes do mesmo binário](../../phxsql/docs/cognicao/cognicao_estado-global-entre-testes-do-mesmo-binario_20260909_0610.md) | PENDENTE | Valor EXATO de global de processo só se confere numa função de teste só. |
| 22 | [Prova por diferença num contador do processo](../../phxsql/docs/cognicao/cognicao_prova-por-diferenca-em-contador-do-processo_20260916_1329.md) | PENDENTE | Contador do processo inteiro só admite piso; «apareceu a minha» se prova nomeando a sua. |
| 23 | [Teste unitário não prova laço preso](../../phxsql/docs/cognicao/cognicao_teste-unitario-nao-prova-laco-preso_20260917_0741.md) | PENDENTE | Laço que pode travar se prova com os dois lados no ar e a linha seguinte chegando. |
| 24 | [Falha no meio se prova com diretório no nome](../../phxsql/docs/cognicao/cognicao_unlink-que-falha-no-meio-se-prova-com-diretorio-no-nome_20260924_1225.md) | PENDENTE | Antes de gancho de teste no código, use o que o kernel recusa sozinho. |
| 25 | [Cache de compilador se prova pela trava, não pelo `cwd`](../../phxsql/docs/cognicao/cognicao_uso-de-cache-de-compilador-se-prova-pela-trava-do-cargo-nao-pelo-cwd_20260924_0300.md) | PENDENTE | Limpeza de disco segura a trava da ferramenta dona do diretório; `cwd` só diz quem pode usar. |
| 26 | [Defeito sem fundo não se repõe no binário inteiro](../../phxsql/docs/cognicao/cognicao_defeito-sem-fundo-nao-se-repoe-no-binario-inteiro_20260924_1828.md) | PENDENTE | Defeito que não termina se prova no caso limitado; o executor filtra para não levar a máquina junto. |
| 27 | [Chave por caminho não segue o `rename`](../../phxsql/docs/cognicao/cognicao_chave-por-caminho-nao-segue-o-rename_20260930_1930.md) | FRUTÍFERO | Todo estado em memória chaveado por caminho segue o `rename` ou sai com o arquivo. |

### Segurança e fronteiras

| # | Cognição | Estado na origem | Por que serve fora do PhxSql |
|---|---|---|---|
| 28 | [TLS: confira o tipo do registro antes do tamanho](../../phxsql/docs/cognicao/cognicao_tls-o-tipo-antes-do-tamanho_20260930_1500.md) | FRUTÍFERO | Qualquer protocolo com cabeçalho de tamanho: confere o que dá no cabeçalho antes de acreditar no tamanho. |
| 29 | [O `..` depois de um link sobe pelo destino](../../phxsql/docs/cognicao/cognicao_o-ponto-ponto-depois-do-link-sobe-pelo-destino_20260924_0330.md) | PENDENTE | Caminho que decide segurança vai inteiro ao SO antes de o texto mexer; lê-se o caminho conferido. |
| 30 | [A grafia com `..` escapa num sentido só](../../phxsql/docs/cognicao/cognicao_grafia-com-ponto-ponto-escapa-num-sentido-so_20260924_1800.md) | PENDENTE | Prova de «mesma coisa por outra grafia» é a matriz inteira; prefixo não é simétrico. |
| 31 | [A senha dentro da frase escapa da lista de nomes](../../phxsql/docs/cognicao/cognicao_a-senha-dentro-da-frase-escapa-da-lista-de-nomes_20260907_1758.md) | PENDENTE | Redação por nome de campo não alcança segredo dentro de texto livre (log, consulta). |
| 32 | [Um sentinela que significa duas coisas mente](../../phxsql/docs/cognicao/cognicao_sentinela-que-significa-duas-coisas-mente-para-o-segundo-chamador_20260918_1340.md) | PENDENTE | `null` de «vazio» e `null` de «não carreguei»: quem reaproveita herda o byte, não o significado. |

### Processo de trabalho

| # | Cognição | Estado na origem | Por que serve fora do PhxSql |
|---|---|---|---|
| 33 | [O conserto entrou no caminho que o motivou, e o irmão ficou](../../phxsql/docs/cognicao/cognicao_o-conserto-entrou-no-caminho-que-o-motivou_20260903_1140.md) | PENDENTE | Conserto procura no mesmo commit quem chama as mesmas funções na mesma ordem. |
| 34 | [A porta nova herda o defeito velho](../../phxsql/docs/cognicao/cognicao_a-porta-nova-herda-o-defeito-velho_20260907_1752.md) | PENDENTE | Porta nova que reusa função antiga não leva os consertos que moram na leitura da resposta. |
| 35 | [O gerador certo chamado pela metade](../../phxsql/docs/cognicao/cognicao_o-gerador-certo-chamado-pela-metade_20260907_0846.md) | PENDENTE | Gerador que faz menos do que o nome promete diz que fez menos; o alvo nunca é nome digitado. |

## Avoid — falhas com causa e prevenção (4, além do nº 1 acima)

| # | Cognição | Por que serve fora do PhxSql |
|---|---|---|
| 36 | [Portão verde não substitui o parecer do lote de risco](../../phxsql/docs/cognicao/cognicao_portao-verde-nao-substitui-o-parecer-do-lote-de-risco_20260924_2036.md) | Revisão humana de risco mediu 5/5 rodadas com o defeito com todos os portões verdes. |
| 37 | [Rascunho da sessão é dividido entre agentes](../../phxsql/docs/cognicao/cognicao_rascunho-da-sessao-e-dividido-entre-agentes_20260930_1655.md) | Diretório temporário compartilhado: escreva só em subdiretório com o nome da tarefa. |
| 38 | [Dono de arquivo é sinal forte, não palpite](../../phxsql/docs/cognicao/cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md) | Antes de usar metadado como prova de intruso, liste quem mais produz o mesmo sinal legitimamente. |
| 39 | [Pipe escapado duas vezes numa regex](../../phxsql/docs/cognicao/cognicao_regex-de-pipe-escapado-duas-vezes-colou-no-titulo_20260930_1620.md) | Script de edição «sem erro» não prova a edição: releia o estado da linha escrita. |

## Ficaram de fora da curadoria, com o motivo

- `guarda-que-so-cobre-quem-a-escreveu`, `o-ignore-que-deveria-lembrar...`,
  `guarda-que-enfraquece-quando-o-motor-melhora`: regra genérica e boa, mas a
  medição dos dois sentidos não está escrita no texto (o extrator casou a
  frase em outro parágrafo).
- `o-contra-exemplo-da-sonda...`: o próprio arquivo declara o buraco («a sonda
  não roda no `cargo test`»).
- `extrator-conta-o-proprio-arquivo-que-grava`: receita de um gerador
  específico.
- As demais FORTE de `CANDIDATAS.md`: regra presa ao motor de dados (trava,
  índice, cascata, replicação, formato em disco).
