# Projeto de exemplo FATURAMENTO (PHP 7.4 procedural)

O segundo exemplo do plugin, e o que prova a parte **E/OU** da regra do legado:
aqui **não há nada de WINDEV**. É um sistema PHP de 2009, procedural, com
`mysqli`, SQL concatenado e HTML no meio do código — o que a maioria dos
clientes chama de «o sistema antigo».

Todo o conteúdo é sintético. Nenhum dado real de pessoa ou empresa.

## O que tem

```text
inputs/
  legado-php/            o código-fonte, que aqui É a evidência central
    config.php           parâmetros por filial; senha só por variável de ambiente
    lib/regras.php       as cinco regras do financeiro (BR-101 a BR-105)
    lib/db.php           conexão única e consultas concatenadas (dívida conhecida)
    fatura_gerar.php     tela de emissão, com HTML e regra misturados
    titulo_baixar.php    baixa com multa e juros, sem transação (defeito real)
  banco.sql              DDL MySQL 5.7: 6 tabelas, 1 view, FKs
  dados-de-amostra/
    amostra.sql          dados sintéticos
    resultados-esperados.json   golden master CAPTURADO rodando o legado
  marca/                 dois logotipos SVG
questionario.json        as 60 respostas deste projeto (produtos: ["php"])
gerar-questionario.py    deriva o questionário do exemplo WX, trocando só o que muda
capturar-golden.php      roda o legado e escreve o golden master
```

## As regras que ele contém (para o G2 achar)

| ID | Regra | Onde está |
| --- | --- | --- |
| BR-101 | multa de 2 % mais juros de 1 % ao mês pro rata die, a partir do dia seguinte ao vencimento | `calcula_encargos` |
| BR-102 | 5 % de desconto à vista; forma desconhecida **não** ganha desconto | `desconto_por_forma` |
| BR-103 | atraso acima de 30 dias bloqueia; até 30 dias fatura com aviso | `situacao_do_cliente` |
| BR-104 | parcelas iguais a cada 30 dias, diferença na última, teto de 12 | `gera_parcelas` |
| BR-105 | CNPJ pelos dois dígitos; todos iguais é inválido | `valida_cnpj` |

Há uma **divergência plantada** para o G2 encontrar: a view `v_inadimplencia`
existe no banco e **nenhum PHP a consulta** — sobrou de uma tela removida em
2018. É um `GAP-*` legítimo.

E há um **defeito real do legado**, de propósito: `titulo_baixar.php` grava a
baixa e o histórico em duas consultas sem transação. Quem converte tem de
decidir se preserva o comportamento ou conserta — e registrar a decisão.

## O golden master não foi digitado

`capturar-golden.php` **roda as regras do legado** e escreve o
`resultados-esperados.json`. É a regra do projeto aplicada ao exemplo: número
visível sai de medição. Se alguém mexer no PHP, o esperado muda junto e a
diferença aparece no diff, em vez de o teste passar contra um número velho.

```bash
php exemplos/faturamento-php/capturar-golden.php > exemplos/faturamento-php/inputs/dados-de-amostra/resultados-esperados.json
```

## Como usar

```bash
cd exemplos/faturamento-php
python3 ../../skills/conversao-wx/scripts/aplicar_questionario.py --questionario questionario.json --project-root . --plugin-root ../..
python3 ../../skills/conversao-wx/scripts/wx_preflight.py --manifest .wx-migration/wx-inputs.manifest.json --allowed-evidence-root ./inputs --workspace-root . --output .wx-migration/preflight
```

Resultado medido: **`CONDITIONAL`, classe `FORENSIC`, zero erros bloqueantes** —
o mesmo do exemplo WX, por caminho diferente.

## O que este exemplo achou

Ele nasceu de um pedido («use um projeto PHP de origem») e **achou um defeito
na primeira execução**: o G0 bloqueava o projeto com oito erros, todos por
supor WINDEV — cobrava `wx_version` de quem não usa WINDEV, chamava `php` de
«produto WX inválido», exigia os PDFs de código, telas e queries que um sistema
PHP nunca teve, e reclamava que o corpus do Help não cobre `php`.

É o padrão que o projeto já conhece: **quando o portão passa a olhar um campo
novo, procure quem não tem esse campo**. O questionário aceitava o legado E/OU
desde a 3.26.0; o portão continuou julgando todo mundo como WINDEV.

O conserto tem duas metades, e a segunda é a que importa:

1. Sem nenhum produto WX, a evidência central deixa de ser o PDF e passa a ser
   o **código-fonte** (`native_project_sources`) mais o esquema do banco; o
   `aplicar_questionario.py` agora varre a raiz do legado e lista os arquivos,
   com linguagem e número de linhas.
2. **Projeto WINDEV continua exatamente como era** — `wx_version` obrigatório,
   PDFs centrais, Help cobrado. O teste que trava isso é o do comportamento
   *velho*, e é ele que impede o conserto de virar um buraco.
