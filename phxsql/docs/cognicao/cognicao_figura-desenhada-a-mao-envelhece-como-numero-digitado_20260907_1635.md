# Figura desenhada à mão envelhece como número digitado à mão

**07/09/2026, 16:35** — rodada das 26 perguntas, papel A.

## 1. O que aconteceu

O dono perguntou: *«Onde fica os .fts? por que o dossiê não tem no gráfico:
Organograma dos arquivos?»*. Medido antes de mexer:

- **Figura 1** («Organograma dos arquivos» — *cada tabela é: SEMPRE os sete,
  SÓ ÀS VEZES três*): sem `.fts`.
- **Figura 8** («O caminho de uma inserção»): desenhava **8 extensões**, sem
  `.fts`.
- **`docs/FORMATO.md`, tabela-mestra de arquivos**: **9 linhas**, sem `.fts`.
- Ocorrências de `.fts` no dossiê inteiro: **4**, nenhuma em figura.

O `.fts` entrou no motor em 07/09/2026 (pedido 200), com `docs/FTS.md` de 300
linhas, bancada, catraca e página de testes — e **nenhum dos três lugares que
dizem «uma tabela tem estes arquivos» soube dele**.

## 2. O que eu concluí primeiro, e estava errado

Li a pergunta, achei a Figura 8 pelo `grep` e a consertei: pendurei o `.fts` no
`.ndx`, tracejado, com a nota certa. Concluí que **a pergunta estava
respondida** e passei a escrever a resposta.

Só ao listar os `<h2>` do dossiê para outro fim vi que o dono tinha nomeado a
seção: «Organograma dos arquivos» é a **Figura 1**, não a 8. Eu tinha
consertado a figura errada primeiro — não porque a 8 não precisasse (precisava),
mas porque **procurei o `.fts` onde eu sabia que a inserção o tocava, em vez
de procurar onde o dono olhou**. A figura que ele apontou lista os arquivos
*por existência*; a que eu consertei lista *por caminho de gravação*. São
inventários diferentes do mesmo fato, e o `.fts` faltava nos dois.

E o terceiro lugar — a tabela do `FORMATO.md` — só apareceu porque, ao
escrever a linha do `.fts` na figura, fui buscar a assinatura dele e não a achei
na tabela onde toda extensão tem a sua.

## 3. O que a medição disse

| inventário | antes | depois |
|---|---:|---:|
| Figura 1, condicionais | 3 (`.lgpd .bkp .pag`) | **4** |
| Figura 8, extensões | 8 | **9** |
| `FORMATO.md`, linhas da tabela | 9 | **10**, + §17 |
| lugares escritos à mão que listam os arquivos de uma tabela | **3** | 3 — nenhum sai do código |
| a lista que sai do código | `arquivos_da_tabela`, `catalogo.rs` | idem |

As duas figuras foram provadas no navegador depois do conserto — o texto
`.fts` lido de dentro de cada SVG.

## 4. A regra

**Figura desenhada à mão envelhece como número digitado à mão — e o inventário
que mora em três lugares diverge no terceiro.** A lei já existia para número
(*todo número visível sai de um gerador, ou está errado e ninguém percebeu
ainda*); o que este arquivo registra é o **alcance**: ela vale para **lista**
também, e uma figura é uma lista desenhada.

E o corolário que decide o que fazer: quando a figura carrega **texto de
decisão** em cada caixa («se o espelho está ligado», «fora do desfazer, de
propósito»), gerá-la inteira do código perde o texto — então o conserto de raiz
não é o gerador, é a **guarda**: um teste que leia a lista do código e confira
que toda extensão dela aparece em cada um dos três lugares. Guarda que compara
a lista viva com as cópias à mão é o que faz a cópia à mão poder existir.

## 5. Como está guardado hoje

- As três cópias estão **corrigidas**: Figura 1, Figura 8 e a tabela +
  §17 do `FORMATO.md`, no mesmo commit.
- A guarda **ainda não existe** — está nomeada no pedido 213 do
  `PENDENCIAS.md`, com a lista que ela lê (`arquivos_da_tabela`) e os três
  lugares que ela confere. Enquanto ela não existir, o próximo arquivo novo
  repõe este defeito — e é por isso que o pedido está aberto e não fechado com
  «figura corrigida».
