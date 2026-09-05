---
name: modelos-locais
description: "Rodar parte do trabalho em modelo local pelo Magnitude, sem mandar código do cliente para fora e sem gastar token pago."
metadata:
  short-description: Modelo local pelo Magnitude, para o trabalho leve
  origem: escrita para o plugin WX Claude Code; documenta o Magnitude (Apache 2.0)
---

# Modelos locais com o Magnitude

Numa conversão WX há duas coisas que o modelo pago faz mal empregado: **tarefa mecânica em volume** (renomear, formatar, extrair lista de um texto já limpo, resumir arquivo) e **tarefa com dado do cliente** que ninguém quer ver saindo da máquina. O Magnitude resolve as duas: ele roda um servidor de inferência local, escolhe o modelo que cabe no hardware, e o Claude Code passa a falar com ele.

Fonte: [magnitudedev/magnitude](https://github.com/magnitudedev/magnitude), **Apache 2.0**. O plugin **não redistribui** o Magnitude — ele é instalado pelo npm, e esta skill diz como usá-lo aqui. Os fatos abaixo saíram da documentação do repositório (commit `83adef8`, 4 de setembro de 2026); confira em `docs.magnitude.dev` antes de confiar num detalhe que mudou.

## Quando vale, e quando não

| tarefa do fluxo | onde rodar | por quê |
| --- | --- | --- |
| Extrair lista de nomes de um PDF já convertido em markdown | local | mecânico, volumoso, sem julgamento |
| Renomear variáveis conforme a linguagem ubíqua | local | regra fixa, muitos arquivos |
| Resumir uma sprint fechada | local | texto já estruturado |
| Traduzir textos de tela (i18n) | local | volume, e o teto de qualidade é o glossário |
| **Extrair regra de negócio do legado** | **pago** | é onde o erro custa caro e a origem tem de ser localizável |
| **Decidir arquitetura, gate, `DEC-*`** | **pago** | julgamento; o aprovador vai assinar embaixo |
| **Golden master e qualquer prova** | **pago** | prova errada é pior que prova ausente |
| **Dado pessoal ou fiscal do cliente** | **pago, e melhor** | regra antiga do roteador: sinal delicado *sobe* o modelo. Manter o dado na máquina é argumento a favor do local, e a troca seria decisão sua — hoje o código escala, e diz por quê |

A regra é a do resto do plugin: **o barato entra onde o erro é barato**. Nada que produza `BR-*`, `DEC-*` ou prova muda de modelo sem o aprovador saber.

## Instalar

```bash
npm install -g @magnitudedev/cli
magnitude setup
```

O `setup` mede o processador, a memória e a banda, ordena as combinações de modelo, quantização e contexto que cabem, mostra a troca entre velocidade, inteligência e memória, baixa o que você escolher e conecta ao harness. Para o Claude Code, a conexão depende do **serviço em segundo plano**, que o setup registra para subir no login.

Sistemas: macOS e Linux; **Windows só por WSL** — vale saber, porque o público do WINDEV está no Windows.

## Comandos que importam

| comando | para quê |
| --- | --- |
| `magnitude setup` | escolha guiada do modelo e conexão ao harness |
| `magnitude models status` | qual modelo está carregado agora |
| `magnitude models load` / `stop` | subir e derrubar o modelo à mão |
| `magnitude catalog list` / `pull` / `remove` | modelos baixados |
| `magnitude connections list` / `add` / `sync` | as ligações com os harnesses |
| `magnitude service status` / `start` / `stop` | o serviço em segundo plano |
| `magnitude docs onboarding` | o roteiro que o próprio agente segue |

O serviço escuta em `http://127.0.0.1:10100`, com API compatível com OpenAI (`/inference/v1`) e com Anthropic (`/inference/anthropic`). O `setup` configura a interface certa; saber o endereço só serve para diagnosticar.

## Como isto conversa com o roteador do plugin

O `rotear_modelo.py` já pesa a tarefa e escolhe o modelo. Com `J.modelos_locais.ativar`, o grau mais leve passa a apontar para o local, e o roteador **diz na saída** que a tarefa foi para fora do modelo pago. Três travas continuam valendo:

1. Tarefa que produz regra, decisão ou prova **nunca** cai no local, mesmo pesando leve. O mesmo vale para os sinais que já subiam o modelo — conflito, fiscal, dinheiro, permissão, decisão humana, falhou antes —, e para `dado-pessoal`, que continua subindo: o local só recebe o que já cairia no degrau mais baixo.
2. Sem o serviço no ar, o roteador **volta para o modelo pago** e avisa — não deixa a tarefa parada.
3. O que rodou onde fica no registro de operações, para o laudo de tokens bater.

## O que medir antes de acreditar

Modelo local é mais barato, não melhor. Antes de mover uma etapa para lá, rode a mesma tarefa nos dois e compare a saída — é a regra do projeto: **hipótese sem medição não vira plano**. Meça também o tempo: o primeiro pedido depois de uma inatividade inclui o carregamento do modelo, e numa máquina apertada isso pode custar mais do que o token economizado.
