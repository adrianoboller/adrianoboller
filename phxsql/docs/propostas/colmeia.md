# Proposta: o segundo tipo de banco — a colmeia (hive), inspirada no REGF

Proposta **antes do código** (como `docs/SOMBRA.md` e `docs/MEMORIA.md`),
pedida pelo dono em 11/09/2026 depois de medirmos a velocidade do Registro do
Windows. Ideia: o PhxSql passa a ter **dois tipos de banco** —

1. **Padrão** — o que já temos: modelo de arquivos separados do HFSQL, tabelas
   relacionais, índices, integridade referencial, transações. Bom para dado
   **transacional e relacional**.
2. **Colmeia (hive)** — um armazém **hierárquico chave→valor**, mapeado em
   memória, inspirado no formato REGF do Registro do Windows. Cada arquivo é uma
   colmeia; uma camada de montagem junta várias colmeias numa árvore só (como o
   Regedit junta hives). Bom para **configuração e metadados** — lido o tempo
   todo, escrito raramente.

Isto **não é trocar** um pelo outro: são dois tipos para dois trabalhos.

## 1. A premissa, medida antes de virar plano

A lei manda medir a receita de fora contra o nosso gargalo antes de aceitá-la.
Nós medimos o Registro real (`bancada/registro/resultados.json`, Windows
WXSOLUCOES, 11/09):

| | Registro (medido) | PhxSql padrão (medido 08/09) |
|---|---|---|
| ler 1 ponto | **~2–5 µs/op** | 7,94 µs/op |
| gravar 1 ponto | **~255–495 µs/op** | 12,8 µs/op |

**O que a medição decide de cara:** a colmeia é um desenho **rápido para ler,
caro para escrever**. Ganha na leitura (2–4×) e **perde feio na escrita**
(20–40×). Isso não condena a ideia — **condena usá-la para a coisa errada**.
Para configuração (escreve-se um punhado de vezes, lê-se milhões), o custo de
escrita é irrelevante e a leitura rápida é ouro. Para dado transacional, o
padrão ganha e continua sendo a escolha.

**A premissa da NOSSA colmeia, agora MEDIDA (12/09/2026).** Os números acima são
do Registro (C, sobre o `mmap` do Windows). Faltava saber se a *nossa* colmeia,
em Rust e com o nosso cache, leria tão rápido. Medimos com um protótipo PSHV
mínimo (`crates/phxsql-store/examples/custo-da-colmeia.rs`,
`bancada/colmeia/resultados.json`), máquina parada (`esta-medindo.sh`: carga
0,20, 4 núcleos), **comparação justa** — ler o mesmo par chave→valor por busca de
ponto → busca de ponto nos dois lados (descida de árvore de células na colmeia ×
`buscar` por índice único + `ler` no Padrão de verdade), os dois quentes,
mediana de 15 repetições:

| N pontos | colmeia (µs/op) | Padrão (µs/op) | razão | faixas cruzam? |
|---:|---:|---:|---:|---|
| 1.000 | 0,162 [0,155; 0,193] | 1,716 [1,703; 1,751] | **10,6×** | não |
| 10.000 | 0,213 [0,199; 0,241] | 2,563 [2,188; 3,163] | **12,1×** | não |
| 100.000 | 0,289 [0,256; 0,360] | 3,968 [2,945; 5,436] | **13,7×** | não |

**A premissa confirma medida, não morre:** a colmeia lê config-shaped
**10,6×–13,7× mais rápido** que o Padrão, e a vantagem cresce com N (a árvore do
`.ndx` do Padrão fica mais alta; a colmeia, config-shaped, quase não aprofunda).
Ressalva honesta para quem repetir: parte do ganho é o protótipo devolver
`&[u8]` sem alocar contra o Padrão decodificar em `Vec<Value>` — isso é
*inerente* à diferença dos dois desenhos (config sem decodificação relacional é
o que um hive-store compra), não artifício de bancada. Isso **justifica seguir
ao desenho de formato** — mas continua só a premissa: nada aqui autoriza
`TipoDatabase::Hive` a sair de `motor_pronto()==false`; V3+ do formato PSHV segue
atrás do P0 e do aval do dono.

## 2. As divergências que as nossas pétreas FORÇAM — e é isto que a torna nossa

Lógica que saiu diferente da de origem não é cópia, e a prova é a divergência.
Onde a colmeia do PhxSql diverge do REGF, e por qual restrição nossa:

- **Sem reúso de célula livre — append-only.** O REGF reutiliza célula apagada
  (tamanho positivo entra na lista livre). A nossa pétrea *«a ordem de digitação
  é sagrada, nunca reaproveita slot»* **proíbe**. A colmeia do PhxSql marca a
  célula apagada e **nunca a reusa**; compactar é operação explícita (um
  `VACUUM`), nunca silenciosa. Consequência boa: o «undelete» que o Registro dá
  por acidente forense vira **propriedade projetada** aqui.
- **Durabilidade pela nossa máquina, não pelo log do Registro.** O REGF usa
  `.LOG1`/`.LOG2`. Nós já temos a marca `.tx` write-ahead e a disciplina de
  `fsync`. A colmeia usa **a mesma** máquina de transação do padrão — e herda o
  conserto de atomicidade do P0 (o commit pai+filho) quando ele entrar. Não se
  reinventa o log do Registro.
- **Integridade, se houver relação.** O Registro não tem FK. Uma colmeia de
  config é quase plana, mas a pétrea primordial vale na árvore por construção:
  **não se apaga uma chave que tem subchaves** sem tratar as filhas — que é o
  `Restrict` da casa, de graça, numa hierarquia.
- **UTF-8 e zero dependência.** O REGF é UTF-16LE; nós gravamos o formato à mão,
  em UTF-8, só com a `std` — o mesmo método que deu o SHA-256 e o SQL daqui.
- **Segurança pela nossa cifra.** Onde o REGF tem o `sk` (descriptor Windows), a
  colmeia usa a cifra em repouso e os direitos que o PhxSql já tem.

O desenho interno pode ser **inspirado** no REGF (bloco base + bins + células
endereçadas por offset, a árvore nascendo de uma célula raiz), porque é um bom
desenho para leitura mapeada — mas cada decisão passa pelo crivo acima. O que
não passar, não entra.

## 3. Onde ela mora: o Phoenix OS

O parecer do Sprint 0010 já apontou: o PhxSql pode dar ao Phoenix OS a
persistência de **configurações, tarefas, catálogo e contexto**. Isso é
exatamente o trabalho de uma colmeia — hierárquico, lido o tempo todo, escrito
raramente —, e é o que o Registro É para o Windows. O padrão relacional fica
para o dado de negócio; a colmeia, para o estado do sistema.

## 4. O que isto NÃO é, e a ordem certa

- **Não é para agora.** O mesmo parecer que sugeriu a integração mandou
  **pausar a ampliação de funcionalidades até o núcleo estar sólido** — e o
  núcleo tem um **P0 de atomicidade aberto** (o commit pai+filho que deixa
  estado parcial visível). Construir um segundo tipo de banco antes de consertar
  o commit do primeiro é exatamente o anti-padrão que o parecer alertou.
- **Não é cópia.** Ver §2: as divergências forçadas pelas nossas pétreas são a
  prova de que o desenho passou pela nossa cabeça.
- **Não é decidido.** É proposta. O que a destrava é **medir a premissa**: um
  protótipo mínimo de colmeia (ler/gravar um valor por caminho) medido contra o
  padrão, para config-shaped data, com a mesma disciplina da
  `bancada/comparacao` (mesmo trabalho, faixa min–máx, data).

## 5. Recomendação

1. **Primeiro o P0** (atomicidade do commit) e os P1 de SQL — o núcleo sólido
   que o parecer exige.
2. **Depois, medir a premissa** da colmeia (protótipo mínimo × padrão), antes de
   qualquer formato novo. Se a nossa colmeia não ler bem mais rápido que o
   padrão para config, a ideia morre medida — e a recusa com número impede que
   ela volte sem medição.
3. **Se a premissa passar**, a colmeia entra como frente de formato (PSCH-like
   para colmeia, a camada de montagem que junta várias numa árvore), decidida
   contra as restrições da §2, com o dono aprovando o formato antes de gravar.

O desenho concreto da estrutura em disco — bloco base, bins, células, os três
níveis (database → colmeias → árvore montada) e as divergências que as pétreas
forçam — está em [`colmeia-estrutura.md`](colmeia-estrutura.md). É proposta de
formato: nada gravado, nada decidido, à espera do dono e da premissa medida.
