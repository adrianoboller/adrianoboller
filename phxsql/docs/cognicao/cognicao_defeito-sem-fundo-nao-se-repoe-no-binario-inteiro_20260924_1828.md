# Defeito sem fundo não se repõe no binário inteiro — a prova vermelha escolhe o caso limitado

**Estado:** PENDENTE

Data da descoberta: 24/09/2026, ~18:28 UTC (frente «fáceis D», pedido 530).

## 1. O que aconteceu

O pedido 530: um job cujo pedido é `job_rodar` de si mesmo sobe uma corrida
aninhada por nível, sem teto (o papel C mediu 45 níveis vivos até o limite de
1,5 GB que ele mesmo impôs). O conserto recusa `job_rodar` em thread da família
`corrida`. A prova natural é o job de si mesmo.

## 2. O que eu concluí primeiro, e estava errado

Que bastava escrever o teste do job de si mesmo e repor o defeito para ver o
vermelho, como nos outros três itens da frente. Com o defeito de pé esse teste
**não termina**: cada nível é uma thread nova, e sem teto de endereçamento ele
só para no limite de threads da máquina — que é compartilhada com as outras
frentes. E o `provar-guardas.py` roda o binário de teste INTEIRO do `alvo`
(`cargo test -p … --test jobs`, sem filtro): uma guarda ingênua levaria o teste
sem fundo para dentro de toda rodada do provador.

## 3. O que a medição disse

O caso limitado prova o mesmo defeito: o job `primeiro` roda o `segundo`, que é
um `ping`. Reposto, a resposta é `ok:true` com o `segundo` rodado dentro do
`primeiro` (duas filhas aninhadas, e fim); com o conserto, `ok:false` com «job
nao dispara job» e zero corridas do `segundo`. O de si mesmo fica só no
sentido verde. A guarda `job-dispara-job` leva `--exact` no `alvo`, com os dois
nomes — o catálogo já tinha o precedente (`["--test", "config-phz", "--",
"--include-ignored"]`).

## 4. A regra

Quando o defeito não tem fundo, o vermelho se mede no caso limitado que passa
pelo mesmo ponto, e a guarda filtra o binário para não levar o sem-fundo junto.

## 5. Como está guardado hoje

`crates/phxsql-server/tests/jobs.rs` (`job_que_roda_job_recebe_a_recusa`, o
limitado; `job_que_roda_a_si_mesmo_para_no_primeiro_nivel`, só verde) e a
guarda `job-dispara-job`. **Buraco:** o vermelho do caso sem fundo continua
sendo o número do papel C, não um rerodado aqui.
