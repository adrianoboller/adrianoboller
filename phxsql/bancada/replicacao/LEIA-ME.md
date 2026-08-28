# Bancada de replicação — quatro servidores

Master + três espelhos, no ar de verdade, com a medição do que chega e em
quanto tempo. Como toda medição aqui, é para ser **refeita**.

```bash
cargo build --release
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```

`montar.py --cascata` põe o Slave03 puxando do Slave01 em vez do master, para
medir o segundo salto.

| Arquivo | O que é |
|---|---|
| `montar.py` | escreve os quatro `config.json` e sobe os quatro processos |
| `medir.py` | a bancada: atraso por tipo de escrita, vazão, queda e retomada |
| `resultados.json` | a última corrida completa |

## A topologia

```
Master 5800 ──┬──► Slave01 5801
              ├──► Slave02 5802
              └──► Slave03 5803
```

Quem procura é a réplica; o master não empurra nada. É o desenho do MySQL(R), e
existe por causa do firewall: o master abre **uma** porta de entrada e não
precisa alcançar ninguém de volta.

## O que ela compara — e por que não é «quantas linhas»

Duas tabelas com o mesmo número de linhas podem ter conteúdo diferente. A
bancada tira um **SHA-256 de cada linha inteira**, com `rowid` e `rownum`
juntos, lendo pelo cursor. Se um único campo atravessar errado, o retrato muda.

O `rowid` entrar na conta é o ponto: ele **não é transmitido**. O `.reg` nunca
reaproveita slot e o rowid é sempre `slot_count + 1`, então uma réplica que
aplicou todos os eventos na ordem chega ao mesmo número sozinha. Se não chegar,
divergiu — e a replicação para ali em vez de espalhar.

## A senha não fica em claro

O `montar.py` chama `phxsqld --senha` para gerar o `senha_hash`, e é dele que a
réplica deriva a chave do desafio-resposta. Não há senha em claro em nenhum
`config.json` que ele escreve.

## O que a última corrida mediu

| | |
|---|---|
| Master, com a imagem no diário | 18.773 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
| Atraso de uma escrita até as três | 1,3 s a 2,1 s |
| Réplica derrubada: voltar a atender | 343 ms |
| Réplica derrubada: alcançar 4.000 eventos | 1,0 s |
| Retrato das quatro, no fim | idênticos |

O atraso é dominado pelo `reconectar_em` (2 s aqui), e não pelo trabalho: é
quanto tempo a réplica dorme entre uma pergunta e outra. Baixar o intervalo
baixa o atraso e sobe o tráfego de perguntas em vão.

**A réplica aplica mais devagar do que o master escreve** — 4.273/s contra
18.773/s. Sob carga sustentada as réplicas ficam para trás. A razão está no
caminho: aplicar decodifica a imagem para `Value` e **reencoda** o payload, em
vez de gravar os bytes que vieram. Está anotado em `docs/PENDENCIAS.md`.

## O que a imagem custa no master

Mesma tabela, mesmas 100.000 linhas, só o interruptor mudando:

| `imagem_da_linha` | linhas/s | bytes por evento | `.log` |
|---|---:|---:|---:|
| desligada | 21.740 | 44 | 4,4 MB |
| ligada | 19.531 | 223 | 22,3 MB |

**10% mais devagar, e um diário 5,1× maior.** É o preço de a réplica receber a
linha e não só o aviso de que ela mudou. Quem só quer auditoria deixa desligado.
