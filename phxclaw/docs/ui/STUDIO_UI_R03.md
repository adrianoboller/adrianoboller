# Studio e templates (UI-R03) — decisões do papel J, 01/10/2026

Pesquisa feita antes de qualquer código. Nada desta página está implementado.

## Medido antes de decidir

- PHX JSON não existe como código: 0 linhas, 0 schemas; só o nome no manifesto do agente 029 e em
  `project.phx.json` num documento de ciclo de projeto.
- O UI-IR está em uso: versões 1 a 3, lido por 11 arquivos, desenhado por 6 adaptadores; tem 25
  listas e a `Section` não tem `id`.
- O motor responsivo bate com o Chromium: 0 de 5 seções fora do IR em 13 larguras; 0 de 30
  fronteiras de contêiner divergentes (6,07 s a corrida).

## Decisões

| Decisão | Venceu | Morreu, e por quê |
|---|---|---|
| Onde mora o Studio | aba da interface web do PhxClaw; phx-grid como inspetor; prévias em iframe com divisor | app separado — repetiria tema, idiomas e autenticação, e o agente da UI-R04 perderia a tela do humano |
| Prévia por largura | uma iframe por prévia (viewport próprio para a regra de base janela) | div lado a lado — a regra de base janela não reage (raciocinado pela especificação, falta medir 575/576 dentro da iframe) |
| Prever o efeito | híbrido: o navegador mede a largura do painel, o motor Rust decide e explica (`responsivo::explicar`); discordância DOM × motor em vermelho | só o motor (não sabe a largura do painel); só o Chromium headless (~150 ms por desenho); explicação em JS (duplica a decisão) |
| Herança de template | patch esparso por UUIDv7 do nó + propriedade, guardando o valor de base (comparação a três vias) | RFC 7396 (não altera parte de lista; com 25 listas apagaria mudanças irmãs); RFC 6902 (endereça por índice: seção inserida desloca a sobrescrita, calado) |
| PHX JSON × UI-IR | PHX JSON vira o ENVELOPE de intercâmbio (`formato`, `versao`, `uuid`); UI-IR, template e instância são tipos dentro dele | adotar nome/forma (não há forma a adotar; renomear custa 11 arquivos); conversor (dois formatos com o mesmo significado, sem dado do outro lado) |
| Identificador | UUIDv7 por `phxclaw_types::new_uuid_v7` + campo `versao` inteiro | ordenar versões pelo relógio do UUID (a RFC 9562 §6.12 manda tratá-lo como opaco) |

Convergência de WinDev, Figma e Clarion, aceita sem pergunta: a sobrescrita local sobrevive à
atualização do template; a propagação é explícita (propor/publicar/regenerar); a instância não muda
a estrutura. Divergências nossas, com a restrição que as causou: sobrescrita localizada pelo UUIDv7
do nó (o Figma casa por nome e perde ao renomear); por propriedade e com o valor de base (o WinDev
sobrescreve o controle inteiro); sobrescrita órfã vira conflito declarado (no Clarion fica calada).

## Contrato proposto

```json
{"formato":"phx","tipo":"ui-template","uuid":"0199a3f0-6c1e-7b2a-9d4e-1f2a3b4c5d6e","versao":2,
 "base_versao":1,
 "conteudo":{"ir_version":4,"sections":[
   {"id":"0199a3f0-6c1e-7c00-8a11-000000000001","title":"Dados","fields":["nome","email"],
    "layout":{"tipo":"grade","gap":"md","colunas":1,
      "responsivo":{"base":"conteiner","alvo":"tela","regras":[{"min_largura_px":576,"colunas":2}]}}}]}}
```
```json
{"formato":"phx","tipo":"ui-instancia","uuid":"0199a3f1-0000-7d00-b000-0000000000aa",
 "template":"0199a3f0-6c1e-7b2a-9d4e-1f2a3b4c5d6e","template_versao":1,
 "sobrescritas":[{"no":"0199a3f0-6c1e-7c00-8a11-000000000001","prop":"layout.gap","valor":"lg","base":"md"}]}
```

Propagação por (nó, propriedade), com T0 = template velho, T1 = novo, I = sobrescrita:

| Situação | Resultado |
|---|---|
| sem sobrescrita | recebe T1 |
| com sobrescrita, T1 = T0 | mantém I |
| com sobrescrita, T1 = I | sobrescrita redundante sai, e o relatório diz |
| com sobrescrita, T1 ≠ T0 e T1 ≠ I | conflito: a instância fica na versão anterior; a sobrescrita nunca se apaga |
| nó sumiu em T1 e havia sobrescrita | órfã = conflito |

Publicar aplica só às instâncias com zero conflitos que passam `de_json` + `validar` + testes;
rejeitar deixa as instâncias idênticas byte a byte; versão publicada não se regrava.

## Lotes

| Lote | Entrega | Prova |
|---|---|---|
| L1 Contrato | IR v4 com `id` UUIDv7 na Section; template, instância, versão; envelope PHX; `responsivo::explicar` | sobrescrita segue o id quando o template insere seção antes; v1–v3 leem igual |
| L2 Propagação | `impacto(velha, nova, instâncias)` → aplicáveis, conflitos, órfãs, redundantes | merge ingênuo apaga a sobrescrita (vermelho); aplicar duas vezes dá o mesmo |
| L3 Portão de publicação | validar, desenhar nos 6 adaptadores, regras, `ui responsivo` nas telas afetadas | rejeitar não muda o hash das instâncias; versão que quebra a validação é recusada |
| L4 Studio | aba, prévias em iframe com divisor, inspetor com editor por linha | arrastar até 420 px dá a regra certa; DOM = motor; dois temas; catraca de idiomas não sobe; nada de `col-` nem HTML no que o Studio grava |
| L5 Fluxo | editar → afetadas → diferenças e conflitos → testes → publicar ou rejeitar | ponta a ponta com uma instância em conflito presa |

⏸ depois da versão: template de template, instância que acrescenta nó, templates guardados na PhxSql.

## Sobe ao dono

Existe fora deste repositório uma especificação ou dado do PHX JSON do Phoenix? Se sim, volta um
conversor só para importar; se não, vale a decisão acima.

Fontes: WinDev (doc.windev.com ?9000087, help.windev.com ?9000086), Figma («Apply changes to
instances»), Clarion (clarion.help, lesson 15; clarionhub «orphaned embeds», comunidade), RFC 7386,
7396, 6902, 9562.
