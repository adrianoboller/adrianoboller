# As atividades de gestão do banco, medidas

Medido em **08/09/2026 14:58** contra o motor vivo — `phxsqld 0.18.0
(8222dad37471-sujo) x86_64-unknown-linux-gnu`. Cada linha desta página subiu o
servidor, exercitou a atividade e olhou o **efeito**: inseriu e leu de volta,
excluiu e conferiu que sumiu, gerou o backup e o **restaurou com outro nome**
para ler a linha de dentro da cópia.

**15 ok · 0 parcial · 0 planejado**, mais replicação e cluster, que têm
bancada própria e entram aqui com a data da medição delas.

> Refaça com `python3 bancada/gestao/medir.py` e depois
> `python3 bancada/gestao/documento.py`. **Este arquivo não se edita.**

---

## 1. O portão que este medidor tem, e por que ele existe

Antes de medir, ele confere **15 operações** contra o catálogo do próprio
servidor: o nome existe? os parâmetros obrigatórios estão todos sendo
mandados? Só então começa.

A primeira corrida publicou **cinco** «planejado» que eram defeito meu, não do
motor: inventei `contar` e `excluir_de_vez` (não existem), mandei `linha` onde
o contrato pede `valores`, chamei `bulkinsert` achando que era `inserir_lote`,
e esqueci o `destino` obrigatório do `backup`. Cada erro voltou como recusa —
e **recusa não lida vira ausência publicada.** Com o portão, isso vira uma
parada com o nome do erro em vez de uma linha na tabela.


---

## 2. Os quatro verbos, pelo protocolo

| | atividade | o que foi medido |
|---|---|---|
| ✅ | INSERT — `inserir` | inseriu e a linha voltou com o valor gravado |
| ✅ | INSERT em lote — `inserir_lote` | 200 gravadas, 0 recusadas; a tabela ficou com 201 |
| ✅ | SELECT — `varrer` e `buscar` pelo índice | varrer devolveu 5 linhas; buscar pelo indice achou 1 |
| ✅ | SELECT com filtro — `varrer` com `onde` | o WHERE cortou para 101 de 201 |
| ✅ | UPDATE — `atualizar` | alterou e a leitura seguinte trouxe o valor novo |
| ✅ | DELETE suave — some da grade, fica no arquivo | marca e some da grade: 200 visiveis de 201 registros, e a lixeira NAO e usada (0) -- a linha continua inteira no `.reg` |
| ✅ | DELETE suave → `restaurar` | voltou com o MESMO rowid: 201 visiveis, lixeira em 0 |
| ✅ | DELETE de vez — `excluir` com `fisico` | destroi a linha (200 visiveis) e guarda o conteudo inteiro na lixeira antes (1) -- inclusive o `.bin` e o `.memo`, que esta mesma exclusao vai liberar |
| ✅ | Integridade: nunca matar o pai que tem filhos | recusou a mae COM filha e deixou passar a SEM filha |

---

## 3. Os quatro verbos, pela camada SQL

| | atividade | o que foi medido |
|---|---|---|
| ✅ | SELECT pela camada SQL | a camada SQL executa |
| ✅ | INSERT pela camada SQL | a camada SQL executa |
| ✅ | UPDATE pela camada SQL | a camada SQL executa |
| ✅ | DELETE pela camada SQL | a camada SQL executa |

---

## 4. Backup e restauração

| | atividade | o que foi medido |
|---|---|---|
| ✅ | Backup do banco pelo protocolo | gerou /tmp/phx-gestao-2dpby35h/bkp/g_adm_2026-09-08_1458.zip |
| ✅ | Restauração do backup, provada por leitura | restaurou com outro nome e a linha 1 voltou: 'ANA' |

---

## 5. Replicação e cluster

Estas duas pedem vários servidores e têm bancada própria. Os números abaixo
saem do `resultados.json` de cada uma, **com a data da corrida** — juntar
medições de dias diferentes sem dizer quando publica um retrato que nunca
existiu.

### ✅ Replicação — medida em 2026-09-07

- `master_linhas_s` — **33883**
- `replica_eventos_s` — **37311**
- `alcance_s` — **2.7**
- `retomada_subiu_ms` — **335**
- `retomada_alcance_s` — **0.3**
- `iguais_no_fim` — **True**
- `linhas` — **105001**
- `atraso_ms_inclui` — **o sono do laco da replica (2 s nesta bancada) MAIS o transporte. Nao e o custo de levar o dado: esse esta medido separado em bancada/quorum/resultados.json (levar_ms)**
- `versao` — **0.18.0**
- `maquina` — **quatro processos phxsqld em 127.0.0.1, no mesmo container**
- `topologia` — **master -> slave01, slave02, slave03**
- `maquina_ocupada` — **False**

### ✅ Cluster — medida em 2026-09-07 16:34 *(data do mtime do arquivo)*

- `redireciona` — **[SP000028] REDIRECIONA 127.0.0.1:5310 -- este no e replica; o master do cluster e no1 (epoca 0)**
- `promocao_s` — **4.3**
- `escrita_aceita_s` — **4.3**
- `emails_promocao` — **1**
- `emails_degradacao` — **6**
- `no3_nao_promoveu` — **True**
- `epoca_do_isolado` — **1**
- `linhas_no_fim` — **3801**
- `sem_cluster_nada_muda` — **True**

---

## 6. O que esta página NÃO diz

1. **Não é veredito de produção.** Ela diz que a atividade funciona,
   não que o conjunto está pronto para carga real com gente dentro.
   O que falta para isso está no `docs/COMPARATIVO.md` e no
   `docs/PENDENCIAS.md`.
2. **`ok` aqui é «faz o que promete», não «faz rápido».** Custo é
   assunto da `bancada/`, que mede outra coisa.
3. **A tela tem caminho próprio, e ele é filmado.** O vídeo de
   `testes-web/video-gestao.mjs` clica os botões de verdade — porque
   interface só se prova exercitando, e o protocolo passar não
   garante que a tela chegue lá.
