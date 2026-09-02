# Tabela de prova sem a coluna que importa não distingue os dois caminhos

- **Quando:** 2026-09-02, 22:30
- **Onde:** `Table::visivel` (o `filtrar` da varredura por índice), regressão
  entrada na SP000006
- **Custo:** 2,5× no caminho quente da varredura por índice, e 2,8× na espera
  de todas as outras conexões — durante quatro horas, sem ninguém ver

## O que aconteceu

Na SP000006 eu troquei o corpo do `filtrar` por uma chamada ao `visivel`, que
resolve a sobreposição do read-your-own-writes. No caminho sem sobreposição —
que é o de todo mundo — ele caía em `ler_do_disco`, e `ler_do_disco`
decodifica **com os anexos**: `decodificar(payload, true)`.

O código anterior usava `decodificar(&p, false)`. Ou seja: para olhar **um
bit** da coluna de sistema, cada linha filtrada passou a pagar a leitura do
`.bin` e do `.memo`.

O `filtrar` é o laço quente da varredura por índice, e ele roda uma vez por
linha da **tabela inteira**. A multiplicação é imediata.

## O que eu concluí primeiro, e estava errado

Que a suíte verde, o `clippy` limpo e a medição do RYOW cobriam a mudança. Os
três estavam certos sobre o que mediam. Nenhum deles media **custo em tabela
com coluna externa**, porque nenhuma tabela de prova tinha uma.

E medi de novo hoje, com a bancada da concorrência, e **ainda não vi**: a
primeira tabela — `id`, `nome`, `cidade` — também não tinha coluna externa. Só
apareceu quando acrescentei um `Memo` de propósito.

## O que a medição disse

Mesma tabela, 50.000 linhas, com `Memo` de 400 bytes:

| | a varredura por índice dura | o vizinho que toma a trava espera |
|---|---:|---:|
| com o defeito | 101,9 ms | 96,9 ms |
| **sem o defeito** | **40,9 ms** | **34,7 ms** |

E sem a coluna `Memo`, os dois lados dão praticamente o mesmo número. A tabela
de prova decidia se o defeito existia.

## A regra

**A tabela de prova precisa ter a coluna que faz os dois caminhos divergirem.**
Trocar «decodificar sem anexos» por «decodificar com anexos» é invisível numa
tabela sem anexo — e é a diferença inteira numa tabela com. Antes de medir uma
mudança no caminho de leitura, pergunte: *que coluna faz o antes e o depois
darem números diferentes?* Se a resposta não estiver na tabela, a medição vai
dizer «igual» e estar certa sobre a tabela errada.

É a irmã da lição do «Blumenau»: um dado todo em maiúscula na origem não
provaria o `text-transform`. Aqui, uma tabela sem anexo não prova o anexo.

## Como está guardado hoje

O `bancada/concorrencia/custo-da-varredura.py` cria a tabela **com** a coluna
`Memo`, e o comentário ao lado diz por quê — para ninguém «simplificar» a
tabela de prova e apagar a única coluna que faz a medição enxergar.

**O buraco que fica:** não há guarda que reprove uma regressão de custo. A
medição existe e é reproduzível, mas ninguém a roda sozinho. É a mesma pétrea
que a QA já listou como sem guarda — «instrumentação desligada custa zero» é
medida e não travada —, e agora com um segundo caso concreto para justificá-la.
