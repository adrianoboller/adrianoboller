# Frente travada nao e frente vazia: o transcrito dela e recuperavel

Descoberto em 17/09/2026, 11:35. A frente 164-F (papel F, largada 09:35)
aparecia como `running` no `ListAgents` e nao devolvia nada ha duas horas.

## 1. O que aconteceu

O integrador segurou a rodada inteira esperando a frente: o backup ficou 214
minutos atrasado, o zelador parado, e a arvore com um **defeito reposto de
proposito** no `crates/phxsql-server/src/servidor.rs` — a frente o repos para
medir o par antes-e-depois, e por isso nada podia ser commitado.

As 11:35 a frente foi declarada parada. As 11:52, antes de desfazer o defeito,
o transcrito dela foi **minerado** em vez de descartado. O que estava la
dentro:

| o que a frente ja tinha feito | quando |
|---|---|
| mapa da trava em quatro commits, JSON guardado | 09:36–09:37 |
| build limpo, `rc=0` | 09:41:06 |
| **a medicao inteira sob o `quieta.Vigia`, com veredito** | 09:44:21 |
| copia byte a byte do `servidor.rs` limpo | 09:45 |
| defeito reposto nos dois chamadores, e o build dele | 09:45:30 — congelou aqui |

Ou seja: o «falta so» do pedido 164 — a medicao final em maquina parada —
**estava pronto e no disco** desde 09:44, guardado em
`scratchpad/corrida1.log`. A frente congelou montando o OUTRO lado da prova.

## 2. O que eu concluí primeiro, e estava errado

Duas coisas, e as duas plausiveis.

**A primeira:** que o `pgrep` vazio bastava para decidir. Escrevi o criterio de
desistencia como «`pgrep` de `cargo`/`rustc`/`python3 bancada` vazio em DUAS
batidas seguidas E o `ListAgents` dizendo `running`». Só que a primeira batida
com esse criterio veio **falsa**: `pgrep -f 'cargo|rustc'` casou a propria
linha de comando do shell que o executava, e respondeu «tem processo». O
criterio que eu escrevi para nao me enganar me enganou na estreia.

**A segunda, e a que custava mais:** que frente travada nao entregou. O plano
que escrevi no elo das 11:35 dizia «o 164 vira pedido aberto com o que ela
deixou medido» — assumindo que ela tinha deixado pouco. Tinha deixado quase
tudo, e a diferenca entre as duas leituras e um pedido fechado contra um
pedido reaberto.

## 3. O que a medição disse

**Sobre a frente estar travada** — tres medidas independentes, e a mais forte
nao era o `pgrep`:

| medida | valor |
|---|---|
| `cargo`/`rustc`/`python3 bancada` vivos (com padrao que nao se casa) | **0** |
| ultimo evento no `agent-a461f03f06efc2a6c.jsonl` | **09:46:43** — 121 min, e um `tool_use` sem resultado |
| arquivos escritos na arvore e em `target/` na ultima hora | **0** |

A segunda e a terceira dizem a mesma coisa que a segunda batida do `pgrep`
diria — «ela nao esta trabalhando» — e dizem **antes**, sem esperar quinze
minutos. Trabalho nao roda sem tocar arquivo, e agente nao pensa sem escrever
evento.

**Sobre o que ela tinha medido** — o par antes-e-depois do pedido 164, agora
completo, com o `Vigia` aprovando as duas corridas («quieta o bastante»). Piso
do `empilhar` por tamanho da transacao:

| ops na transacao | COM o defeito | SEM o defeito | razao |
|---:|---:|---:|---:|
| 1600 | 593,75 us | 41,25 us | **14,39x** |
| 800 | 261,25 us | 38,75 us | 6,74x |
| 400 | 127,50 us | 37,50 us | 3,40x |
| 200 | 75,00 us | 40,00 us | 1,88x |
| 100 | 60,00 us | 40,00 us | 1,50x |

E o controle que a propria medicao carrega: o `op_inserir`, que o defeito nao
tocava, ficou **50,00 us (49,00..53,50) com o defeito contra 52,50 us
(46,25..53,00) sem ele** — faixas que se cruzam, efeito nenhum. Medida que
mexe no que devia mexer e nao mexe no que nao devia e medida especifica.

## 4. A regra

**Antes de descartar uma frente travada, minere o transcrito dela: a linha do
tempo das ferramentas diz ate onde ela chegou, e o `mtime` dos arquivos diz o
que ela deixou no disco.**

E a regra irma, que e o alcance: **`mtime` de transcrito parado e ausencia de
escrita valem mais que `pgrep`, porque nao se enganam sozinhos.**

## 5. Como está guardado hoje

A receita, que e o que se reaproveita:

```bash
# 1. ate onde ela chegou -- a linha do tempo das chamadas
python3 - "$JSONL" <<'PY'
import json,sys
for ln in open(sys.argv[1],encoding='utf-8',errors='replace'):
    try: o=json.loads(ln)
    except Exception: continue
    m=o.get('message') or {}
    for c in (m.get('content') or []) if isinstance(m.get('content'),list) else []:
        if isinstance(c,dict) and c.get('type')=='tool_use' and c.get('name')=='Bash':
            print(o.get('timestamp','')[11:19], (c.get('input') or {}).get('description',''))
PY

# 2. o que ela deixou no disco, pela JANELA de tempo em que trabalhou
find "$SCRATCHPAD" -maxdepth 1 -newermt '<largada>' ! -newermt '<congelou>' -printf '%TH:%TM %8s %p\n' | sort

# 3. e a copia limpa que ela guardou, CONFERIDA contra o HEAD antes de usar
git show HEAD:./caminho/do/arquivo > /tmp/HEAD.rs
cmp -s "$SCRATCHPAD/arquivo.ORIGINAL" /tmp/HEAD.rs && echo IDENTICO
```

O passo 3 e o que permite desfazer o defeito **sem** `git checkout --`,
`restore` ou `reset` na arvore compartilhada: prova-se que a copia e o `HEAD`
e copia-se por cima, por caminho explicito.

**Onde o buraco ficou:** o passo 2 depende de a frente ter escrito no
scratchpad, e a copia dela usou `cp -p` — que preserva o `mtime` da origem,
entao o arquivo mais importante **nao aparece** na janela de tempo e so se
acha pelo nome. Nao ha guarda para isso: a receita acima e um roteiro, nao um
conferidor.

E a segunda: os lacos de espera que o proprio orquestrador cria
(`until ! pgrep -f "<padrao>"; do sleep 3; done` por `Bash run_in_background`)
sofrem do **mesmo** `pgrep` que se acha — tres deles estavam nessa sessao,
presos para sempre, e foram parados com `TaskStop`. O portao «esta medindo?»
ja cobre o `pgrep` das bancadas; **nao cobre** o que o orquestrador digita na
hora. Um laco de espera confere um arquivo (`until grep -q '^rc=' log`), nunca
uma lista de processos.
