# Bancada do ciclo do phxSERVERMAIL

Exercita o **banco de verdade** ao longo do ciclo do server mail que o dono
pediu: instalar → configurar a base → UUID v7 do usuario → integridade
empresa/funcionarios → coligacao (confianca entre empresas) → replicacao →
DNS → portao da WX → cluster (eleicao/promocao) → envio empresa↔empresa.

Nao e maquete: grava e le do disco real (`.reg`/`.ndx`), com as chaves
estrangeiras **conferidas** na gravacao, e sobe `phxsqld` de verdade para a
replicacao e o cluster.

```bash
# a bateria inteira, re-rodavel de ponta a ponta:
bash bancada/servermail/rodar.sh [dir_de_trabalho] [n_linhas_replicacao]
```

Sem argumento, `rodar.sh` cria um diretorio temporario e o limpa no fim.

## Os arquivos

| arquivo | passo | o que faz |
|---|---|---|
| `rodar.sh` | 1–9 | orquestra a bateria inteira e grava `resultados.json` |
| `instalar.sh` | 1 | gera `config.json`, gera `senha_hash` (sem senha em claro), sobe o servidor, confere que escuta e que a base nasce, derruba pelo PID |
| `servermail-ciclo` (exemplo Rust) | 2,3,4,7 | o **banco de verdade**: UUID v7 no `.reg`, empresa←funcionario por chave conferida (RESTRICT), coligacao entre empresas, portao WX. Placar com `PROVA VERDE`, sai 1 se falhar. Fonte em `crates/phxsql-store/examples/servermail-ciclo.rs` |
| `replicar.py` | 5 | sobe master + 3 slaves (reusa `bancada/replicacao/montar.py`), semeia, espera convergir e prova o **SHA-256 por linha** identico nos quatro. Derruba por caminho |
| `dns.py` | 6 | resolve `phxsql.com.br`, `phxmail.com.br`, `wxsolucoes.com.br` e conclui honesto |
| (usa `bancada/cluster/provar.py`) | 8 | eleicao, promocao, particao sem maioria, cura sozinho |
| (usa `correio-e2e` em phxsql-core) | 9 | portao de dominio + envio intra/inter empresa (o MODELO do correio) |

## O que roda de verdade AQUI, e o que e passo de implantacao

- **Passos 1,2,3,4,5,7,8,9 rodam de verdade neste sandbox** — servidores
  `phxsqld` reais, tabelas no disco, chaves conferidas, replicacao e cluster
  medidos agora, com SHA-256 por linha.
- **Passo 6 (DNS)** e honesto: `phxsql.com.br` e `phxmail.com.br` **ainda nao
  resolvem** — e passo de implantacao (registrar o dominio, apontar A/MX no
  provedor). O portao de dominio do correio ja funciona (passo 9); falta o
  dominio existir no mundo. `wxsolucoes.com.br` resolve, mas a bateria **nao se
  conecta nela** — e producao do dono.
- **Passo 7 (portao WX)** prova o gate LOCALMENTE: o cadastro so libera depois
  de o v7 do usuario ter sido armazenado. O armazenamento remoto na WX
  (177.69.238.17) e **simulado de proposito** — nao se toca a producao.

## Onde o id do usuario fica guardado

No arquivo **`.reg`** da tabela, dentro da base do server mail
(`<base>/<banco>/funcionarios.reg` etc.). O `.reg` e um *heap* de slots de
largura fixa na **ordem de digitacao**; a coluna `id` do tipo `Uuid` ocupa 16
bytes inline no slot (`rowid` = numero do slot). O `.ndx` guarda a B+tree do
indice `porId`. Ver `docs/FORMATO.md`.

## As leis que esta bancada honra

- **Nunca se mata o pai que tem filho**: `ao_excluir` so aceita `restringir`,
  a chave **nasce conferida**, e ha **indice dos dois lados**. Provado nos dois
  sentidos: apagar recusa com filho, passa sem.
- **Prova real nos dois sentidos**: cada invariante mostra a recusa E a
  passagem. A prova de que o motor recusa quando deve (o defeito reposto) vive
  nos testes unitarios do motor (`valores.rs`, `cluster.rs`, `uuid.rs`); esta
  bancada exercita as duas pontas ponta a ponta, no disco.
- **Zero dependencias externas**: so `std` + a cripto ja conferida contra vetor.
- **Sobe e derruba por caminho, nunca por nome**; nao mata processo de vizinho.
