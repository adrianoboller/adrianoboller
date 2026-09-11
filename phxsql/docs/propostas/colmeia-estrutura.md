# A estrutura do banco hive (colmeia) — proposta de formato

Companheira de `colmeia.md`: aquela diz **por que** e **o que medir antes**;
esta diz **como seria a estrutura em disco**. É **proposta de formato** — pela
pétrea, *o formato se decide com o dono antes de gravar o primeiro byte*, e a
premissa (`colmeia.md` §1) se mede antes de construir. Nada aqui está gravado
nem decidido; é o desenho posto na mesa.

O desenho é **inspirado** no REGF do Registro do Windows (bloco base + bins +
células endereçadas por offset, a árvore nascendo de uma célula raiz), e cada
peça passa pelo crivo das nossas pétreas — é isso que o torna nosso, não cópia.

## 1. Três níveis: database → colmeias → árvore montada

O dono descreveu certo: *«cada arquivo é uma colmeia, e o Regedit junta várias
colmeias numa árvore só».* São três níveis:

```
base/
└── config/                     ← um database do tipo hive
    ├── _database.json          {"tipo":"hive","versao":1}   (já existe hoje)
    ├── montagem.json           a camada de montagem: caminho → colmeia
    ├── sistema.hive            uma colmeia (um arquivo PSHV)
    └── usuarios.hive           outra colmeia
```

- **Database hive** = um diretório com o marcador `_database.json` (tipo=hive) —
  a infraestrutura que já existe. Dentro dele, uma ou mais **colmeias**.
- **Colmeia** = um arquivo `.hive` no formato **PSHV** (§3). É a unidade de
  armazenamento: um armazém hierárquico chave→valor, autossuficiente.
- **Árvore montada** = a `montagem.json` mapeia um prefixo de caminho a uma
  colmeia, como o Windows monta `SYSTEM`, `SOFTWARE` e `NTUSER.DAT` sob uma
  árvore só. `/​sistema/...` vem de `sistema.hive`; `/​usuarios/<id>/...` de
  `usuarios.hive`. É o que dá a «árvore única» sobre arquivos separados.

A montagem ser um JSON à parte (e não gravada dentro das colmeias) é decisão de
DBA: montar/desmontar uma colmeia não reescreve colmeia nenhuma, e uma colmeia
viaja para outro database sem carregar a topologia de onde estava.

## 2. A árvore lógica: chaves, subchaves e valores

Dentro de uma colmeia, o modelo é o do Registro — uma árvore de **chaves**, cada
chave com **subchaves** e **valores**:

```
(raiz da colmeia)
├── impressora/                 ← chave
│   ├── modelo    = "Phoenix X1"     ← valor (nome = dado)
│   ├── dpi       = 1200             ← valor
│   └── rede/                    ← subchave
│       └── ip    = "10.0.0.9"
└── locale/
    ├── idioma    = "pt-BR"
    └── moeda     = "BRL"
```

Uma **chave** é um nó: tem nome, subchaves e valores. Um **valor** é um par
nome→conteúdo tipado. Caminho `/impressora/rede/ip` desce da raiz por
`impressora`, `rede`, e lê o valor `ip`. É exatamente a forma que serve
configuração — hierárquica, lida o tempo todo, escrita raramente.

## 3. O arquivo PSHV — bloco base, bins e células

O formato segue a convenção da casa (cabeçalho de 128 bytes, little-endian,
CRC-32 no cabeçalho — como `.reg`/`.ndx`), com o miolo inspirado no REGF.

```
┌─────────────────────────────────────────────────────────────┐
│ BLOCO BASE (128 bytes)                                        │
│  assinatura "PHXHIV\0\0" · versão · CRC-32 do cabeçalho       │
│  offset da célula RAIZ · tamanho da área de células           │
│  n.º de sequência de escrita (detecção de sujo/recuperação)   │
│  flags (cifrado?) · reservado (zeros)                         │
├─────────────────────────────────────────────────────────────┤
│ BIN 0  (bloco de 4 KiB, alinhado)                             │
│  cab. do bin: assinatura · offset relativo · tamanho          │
│  ┌──────── célula ────────┐ ┌──── célula ────┐ ...            │
│  │ tam(i32) │ status │ …  │ │ tam │ status │…│                │
│  └────────────────────────┘ └────────────────┘               │
├─────────────────────────────────────────────────────────────┤
│ BIN 1 …                                                       │
└─────────────────────────────────────────────────────────────┘
```

Uma **célula** é `tamanho (i32) + status + conteúdo`. Cada célula é um dos
registros:

| Célula | O que guarda | Inspirada no REGF |
|---|---|---|
| **NÓ** (chave/subchave) | flags, offset do pai, (contagem + offset) da lista de subchaves, (contagem + offset) da lista de valores, offset de segurança, carimbo de tempo, **nome em UTF-8** | `nk` |
| **VALOR** | tipo, comprimento, dado inline (se pequeno) ou offset para célula de dado, nome em UTF-8 | `vk` |
| **LISTA DE SUBCHAVES** | array de (hash CRC-32 do nome + offset do NÓ), ordenado para busca binária | `lf`/`lh` |
| **LISTA DE VALORES** | array de offsets para VALOR | value-list |
| **DADO GRANDE** | segmentos de um valor grande (ou aponta para o `.bin`/`.memo` da casa — decisão a fechar com o dono) | `db` |
| **SEGURANÇA** | os **nossos** direitos + a marca de cifra em repouso, compartilhável entre nós por refcount | `sk` (mas o descriptor do Windows sai) |

Ler `/impressora/rede/ip`: bloco base → NÓ raiz → sua LISTA DE SUBCHAVES
(busca binária por hash de `impressora`) → NÓ `impressora` → subchaves → NÓ
`rede` → sua LISTA DE VALORES → VALOR `ip`. Leitura pura, sem escrita —
o caminho quente da colmeia.

## 4. As divergências que as pétreas FORÇAM — e são a prova de que é nosso

- **Sem reúso de célula — append-only.** No REGF, `tamanho` positivo marca
  célula livre, que é **reusada**. A nossa pétrea *«a ordem de digitação é
  sagrada, nunca reaproveita slot»* proíbe: o campo **status** marca a célula
  como viva ou morta, e **morta nunca volta a ser alocada**. Célula nova sempre
  **nasce no fim** (append). Compactar (um `VACUUM`) é operação **explícita**
  que reescreve a colmeia, jamais silenciosa. Consequência boa: o «undelete»
  que o Registro dá por acidente forense vira **propriedade projetada** aqui.
- **Mapa de leitura é o NOSSO cache, não `mmap`.** O REGF é mapeado pelo
  Configuration Manager do kernel. Nós somos zero-dependência e a `std` não tem
  `mmap`; então a colmeia é lida por página para o **mesmo cache** que já
  comprou 2,40× no `.ndx`. Isto muda a premissa a medir (`colmeia.md` §1): a
  leitura rápida do Registro é do `mmap` dele; a nossa depende do nosso cache, e
  isso **se mede antes de prometer**.
- **UTF-8, escrito à mão, só `std`.** O REGF é UTF-16LE. Gravamos o formato à
  mão em UTF-8, como o SHA-256 e o SQL da casa. O hash da lista de subchaves é o
  **nosso** CRC-32, que já existe.
- **Durabilidade pela nossa máquina.** Nada de `.LOG1`/`.LOG2`: a colmeia usa a
  marca `.tx` write-ahead e o `fsync` que o padrão já tem, e herda o conserto de
  atomicidade do P0 quando ele entrar. O n.º de sequência do bloco base é só
  para **detecção de sujo/recuperação**, não é um segundo diário.
- **Integridade primordial, de graça, na árvore.** *Nunca se mata o pai que tem
  filhos*: apagar uma chave que tem subchaves **recusa** (o `Restrict` da casa),
  como a mãe com filhas. Na hierarquia isso é por construção — o NÓ sabe se tem
  lista de subchaves não-vazia.
- **Segurança pela nossa cifra e direitos.** Onde o REGF tem o descriptor do
  Windows, o registro de SEGURANÇA carrega os direitos do PhxSql e a cifra em
  repouso da casa.

## 5. As operações do protocolo (esboço)

Roteadas pelo `TipoDatabase::Hive` — a infraestrutura que já existe recusa hoje
com «motor em construção»; quando o motor entrar, ela passa a despachar para cá:

- `hive_ler` (caminho → valor) — o caminho quente.
- `hive_gravar` (caminho, valor) — append de célula; nunca reusa.
- `hive_listar` (caminho → subchaves + valores).
- `hive_apagar` (caminho) — **Restrict** se a chave tem subchaves.
- `hive_compactar` — o `VACUUM` explícito que recolhe as células mortas.
- `hive_montar` / a `montagem.json` — mapeia prefixo → colmeia.

## 6. O que fica para o dono decidir antes de qualquer byte

Pela pétrea, o formato se fecha com o dono. Os pontos abertos:

1. **Tamanho do bloco base** — 128 bytes (convenção da casa) ou 4 KiB (como o
   REGF, alinhado à página). Recomendo 128, pela coerência com os outros nossos.
2. **Dado grande** — dentro da própria colmeia (célula DADO GRANDE) ou
   reaproveitando o `.bin`/`.memo` que as tabelas já usam. Reaproveitar poupa
   código; embutir mantém a colmeia autossuficiente para viajar.
3. **Alinhamento dos bins** — 4 KiB (localidade de leitura) confirmado?
4. **Extensão** — `.hive` está bom, ou seguimos outro nome?

## 7. A ordem, que não muda

1. **Primeiro o P0** de atomicidade e os P1 de SQL — o núcleo sólido que o
   parecer exige.
2. **Depois, medir a premissa** (`colmeia.md` §1): um protótipo mínimo de
   colmeia (ler/gravar um valor por caminho) contra o padrão, para dado de
   configuração, com a disciplina da `bancada/comparacao`. Se a nossa colmeia
   não ler bem mais rápido que o padrão, a ideia morre medida.
3. **Só então**, com a premissa passada e este formato aprovado pelo dono, o
   motor da colmeia entra — construído contra as divergências da §4.
