# Produção de vídeo por agentes no PhxClaw — especificação funcional (sala limpa, fase 1)

Papel J, 09/10/2026. Decisão do dono do mesmo dia (`docs/diretivas/DECISOES_DO_DONO_20261009.md`,
última seção): reimplementar em Rust, **por sala limpa**, o que o OpenMontage faz. Este documento é
a **única** entrada da equipe de implementação: ela não abre o repositório de origem. Tudo aqui
descreve **comportamento e requisito** com palavras nossas; nenhum código, prompt, YAML, nome de
função, nome de campo ou trecho de texto de origem foi reproduzido (§7).

Convenções: **medido** = rodado nesta máquina ou lido em fonte primária hoje, com o comando ou a
URL ao lado; **raciocinado** = diz que é, e nomeia o que decidiria o número na bancada.

## 0. Hipóteses escritas antes de medir

| # | Hipótese | Veredito |
|---|---|---|
| H1 | A primeira entrega útil (narração + imagens/clipes + legenda + render) sai **só com o FFmpeg** como processo, sem motor de composição em navegador | **SE SUSTENTA** — bancada de 28 s em 1080p rendeu e passou no `ffprobe` (§6.2) |
| H2 | Existe TTS **local** de licença permissiva com voz **pt-BR** | **MORREU** — medido nas licenças (§4.4): todo caminho local de pt-BR achado passa por fonemizador GPL ou é GPL |
| H3 | Todo acervo livre **sem chave** devolve a licença **por item** | **PARCIAL** — Wikimedia e Internet Archive sim; NASA não (§3.4) |
| H4 | Normalizar o volume numa passada só acerta o alvo | **MORREU** — alvo −16 LUFS, saída medida −20,5 LUFS (§6.2) |
| H5 | Com o grafo de filtros gerado por nós, a duração do MP4 é a planejada **ao quadro** | **SE SUSTENTA em 1 caso** — 28,000 s e 840 quadros = 28 × 30 (§6.2). Prova geral é critério de aceite (SP000041) |

---

## 1. Visão funcional — o que é produzir um vídeo por agentes

Produzir um vídeo é transformar um **pedido em linguagem natural** num **arquivo MP4 conferido**,
passando por artefatos intermediários que um humano consegue ler, aprovar ou recusar. O agente LLM
faz o trabalho **criativo e textual** (entender o pedido, pesquisar, escrever, decupar, escolher);
o código determinístico faz o **resto** (buscar, baixar, sintetizar, montar, renderizar, conferir).
Cada etapa produz **um artefato com esquema** e a seguinte só começa com o anterior válido.

### 1.1 As etapas, de ponta a ponta

| # | Etapa | Entra | Sai (artefato nosso) | Quem decide | Aprovação humana |
|---|---|---|---|---|---|
| 1 | **Briefing** | pedido livre + material do operador (opcional) | `briefing`: finalidade, público, destino (plataforma → proporção e duração), tom, idioma, duração-alvo, o que é obrigatório, teto de custo, voz sim/não, música sim/não, legenda sim/não | LLM pergunta só o que falta; o resto vem do pedido | não (é a fala do próprio humano) |
| 2 | **Pesquisa** (só nos tipos que informam) | `briefing` | `pesquisa`: fatos com fonte (URL, data), ângulos possíveis, o que o público já sabe | LLM com `web.search`/`web.browse` | não |
| 3 | **Proposta** | `briefing` + `pesquisa` | `proposta`: 1 a 3 conceitos, o escolhido, **contrato de entrega** (§1.3), custo estimado por linha, fontes de mídia previstas | LLM propõe; motor calcula o custo pela tabela do operador | **SIM — portão 1**: nada pago começa antes |
| 4 | **Roteiro** | `proposta` | `roteiro`: blocos de texto falado com duração estimada, indicações de entonação, pronúncia de nomes, fonte de cada afirmação | LLM; motor estima a duração pela velocidade de leitura | sim nos tipos com narração (pode-se fundir com o portão 1) |
| 5 | **Decupagem** | `roteiro` | `decupagem`: cenas com início/fim, o que se vê, enquadramento/movimento, o ativo necessário (gerar, buscar no acervo, capturar, do operador) e 2–3 consultas de busca por cena | LLM; motor confere soma das durações contra a duração-alvo | opcional (padrão: junto do portão 2) |
| 6 | **Obtenção de ativos** | `decupagem` | `ficha_de_ativos`: um registro por arquivo, com **proveniência e licença obrigatórias** (§3.4) | motor busca/baixa/gera; LLM escolhe entre candidatos pela miniatura e pela descrição | **SIM — portão 2**: ativos e licenças, antes de montar |
| 7 | **Narração, música, efeitos** | `roteiro` + escolha de voz | arquivos de áudio na ficha; a duração **real** da narração realimenta a duração das cenas | motor (provedor de voz/música) | não (já coberto pelo portão 1 em custo) |
| 8 | **Legendas** | narração + texto do roteiro | `.srt` (e `.ass` quando estilizada), tempos por segmento | motor: tempo pela transcrição da própria narração ou pela proporção de caracteres | não |
| 9 | **Montagem** | `decupagem` + `ficha_de_ativos` + áudio + legendas | `linha_do_tempo`: cortes (fonte, entrada, saída, camada, transição), sobreposições, trilhas de áudio com volumes e abaixamento, legenda queimada ou em faixa | LLM propõe; **motor valida** (durações, referências a ativos existentes, sobreposição) | não |
| 10 | **Render** | `linha_do_tempo` | MP4 | motor gera o grafo de filtros e chama o FFmpeg | não |
| 11 | **Conferência** | MP4 + `contrato de entrega` + `linha_do_tempo` | `laudo`: o que se mediu no arquivo (§3.6), quadros de amostra, aprovado/reprovado com motivo | **motor, nunca o LLM que produziu** | — |
| 12 | **Entrega** | MP4 + `laudo` + créditos | arquivo na pasta da tarefa; publicação opcional | humano | **SIM — portão 3** se for publicar (`site.publish` já é capacidade protegida) |

### 1.2 Decisões que atravessam as etapas

- **Retomada.** Cada etapa grava o artefato antes de anunciar «feita»; recomeçar uma produção pula
  o que já tem artefato válido e não paga de novo o que já foi pago (chave de idempotência por
  pedido a provedor, §3.1).
- **Volta atrás.** Recusa num portão devolve à etapa que produziu o artefato recusado, com o
  motivo escrito pelo humano. Há **teto de voltas por etapa** (configurável); estourado, a
  produção para dizendo onde parou — nunca «segue com avisos» calada.
- **Sem troca silenciosa.** Se a etapa não consegue cumprir o que o contrato de entrega promete
  (ex.: prometeu clipes com movimento e só achou fotos), ela **para e pergunta**; não rebaixa o
  meio por conta própria.
- **Registro de decisões.** Toda escolha com alternativas (provedor, voz, conceito, fonte de
  acervo) vira uma entrada de evidência com as opções, a escolhida e o motivo — pelo
  `phxclaw-evidence-ledger` que já existe, não por um registro paralelo.

### 1.3 O contrato de entrega

Fixado na proposta e aprovado no portão 1. Campos (nossos): proporção e resolução; quadros por
segundo; duração-alvo e tolerância; idioma do áudio e das legendas; legenda **queimada** ou **em
faixa**; narração sim/não e de onde (voz humana do operador, voz local, provedor); música sim/não;
**meio dominante** (clipes com movimento × imagens paradas com movimento de câmera × telas
desenhadas); teto de custo; política de licença aceita (§3.4).

Ele é a régua do `laudo`: cada campo verificável no arquivo é verificado (§3.6). Diverge da origem
— lá a promessa é classificada e cobrada pelo próprio agente que produz; aqui é **struct tipada
conferida por código contra o `ffprobe`**, porque a nossa regra é «saída de terceiro só entra
depois de conferida» e o LLM que montou é parte interessada.

---

## 2. Tipos de produção (pelo resultado, não pela configuração de origem)

Cada tipo é **um modelo de fluxo** na galeria (`fluxo_modelos.rs`), montado com as mesmas
ferramentas. Coluna «sem chave?» = se dá para entregar sem provedor pago.

| Tipo | O que sai | Entradas | Etapas que muda/acrescenta | Ativos necessários | Sem chave? |
|---|---|---|---|---|---|
| **Documentário de acervo** | montagem temática de imagens/clipes reais, com música e (opcional) narração e cartão final | tema, tom, duração | pula pesquisa longa; decupagem orientada a «vagas» (cada cena descreve o que procurar); obtenção = busca em acervo livre | clipes/fotos de acervo, música livre, voz | **sim** (acervo sem chave + voz do operador ou local) |
| **Explicativo animado** | vídeo didático narrado com ilustrações, diagramas, gráficos, cartões de texto | tema; dados opcionais | pesquisa + proposta completas; ativos majoritariamente **desenhados** (SVG → PNG pelo Chromium que já existe), animados por movimento de câmera e transições | imagens geradas ou desenhadas, voz, música | **sim** em versão «telas desenhadas»; com imagem gerada, depende do provedor |
| **Apresentador / avatar** | pessoa falando para a câmera, com apoio gráfico simples | roteiro curto; foto ou avatar | ativo central é um **vídeo de rosto falando** gerado por provedor | avatar/sincronia labial (provedor), voz | **não** (exige provedor de avatar) — ⏸ |
| **Cabeça falante (gravação real)** | gravação de uma pessoa, limpa e legendada | vídeo bruto do operador | transcrever → cortar silêncios e vícios → reenquadrar (16:9 → 9:16 pelo rosto) → legendar → normalizar áudio | só o vídeo do operador | **sim** |
| **Demonstração de tela** | tutorial de software com zoom, destaques e legenda | captura de tela real, ou roteiro de terminal | captura (`ffmpeg_record_command` já existe no `phxclaw-system-automation`) ou terminal sintetizado como imagens; zoom em regiões; destaque de clique | captura, voz opcional | **sim** |
| **Podcast em vídeo** | audiograma (onda + legenda + capa) e cortes curtos | áudio ou vídeo de podcast | transcrever → escolher trechos (LLM) → legendar → compor com capa e forma de onda (`showwaves` do FFmpeg) | o áudio do operador, uma capa | **sim** |
| **Fábrica de cortes** | N vídeos curtos independentes a partir de um longo | webinar, live, palestra | transcrever → LLM aponta N trechos com gancho → cortar, reenquadrar, legendar cada um → N laudos | o vídeo do operador | **sim** |
| **Localização / dublagem** | legendas traduzidas por idioma; dublagem opcional | vídeo + idiomas-alvo | transcrever → traduzir (LLM) → ajustar tempo das legendas → (opcional) voz por idioma e remix → um laudo por idioma | o vídeo; voz por idioma | legenda **sim**; dublagem pt-BR depende de voz (§4.4) |
| **Cinemático** | trailer/filme de marca, ritmo e cor | material do operador ou gerado | ênfase em cortes no ritmo da música (detecção de batida), correção de cor uniforme (LUT) | clipes, música | **sim** com material do operador; com geração de vídeo, **não** |
| **Animação de personagem** | personagem 2D articulado | roteiro | desenho do personagem, articulação, poses, linha de ações | — | **RECUSADO por ora**: exige motor de animação próprio; não há demanda medida |

O que **não** entra (decidido aqui, com motivo): análise de vídeo de referência baixado de
YouTube/TikTok (termos das plataformas e direitos; o estudo de 09/10 já recusou, b10); pontuação
de «risco de parecer apresentação de slides» por pesos digitados (mesma régua que recusou a
pontuação de provedores, b7: peso não medido é palpite com casas decimais); revisor-LLM que avalia
o próprio trabalho (substituído pelo `laudo` determinístico + portões humanos).

---

## 3. Contratos funcionais (termos nossos)

### 3.1 Provedor de mídia

Um provedor é **um jeito de produzir ou obter um arquivo de mídia**. Todos cumprem o mesmo
contrato, para que a etapa não ramifique por provedor.

**Identidade (declarada, conferível):** nome estável; **tipo** (imagem, vídeo, voz, música,
efeito sonoro, transcrição, busca em acervo); **onde roda** (processo local no sandbox, ou HTTP);
licença do motor e do modelo; **o que o termo do provedor diz sobre a saída** (pode uso
comercial? exige atribuição?) — texto declarado, com URL e data.

**Disponibilidade, sem rede e sem custo:** responde «posso rodar agora?» olhando só o ambiente —
chave concedida pelo broker (por nome), binário presente, hash do modelo conferido
(`verify_sha256` que já existe). A resposta lista o que falta, pelo nome da configuração.

**Cotação:** preço por unidade (chamada, segundo de saída, caractere, imagem) vem **do arquivo de
preços do operador** (extensão do `custo.precos` de R2), com moeda, data e fonte. Sem cotação =
«não medido», nunca zero.

**Parâmetros essenciais por tipo:**

| Tipo | Pedido mínimo | Resposta mínima |
|---|---|---|
| Imagem | descrição (teto de caracteres), proporção ou tamanho, quantidade, semente opcional, imagem de referência opcional (confinada à pasta da tarefa) | arquivo PNG/JPEG; dimensões |
| Vídeo | descrição, duração, proporção, resolução, imagem inicial opcional, semente opcional | arquivo; duração e dimensões **medidas** (não as declaradas) |
| Voz | texto (teto), voz, idioma, velocidade, formato de saída (WAV PCM) | WAV conferido pelo cabeçalho (`voz.rs` já faz); duração medida |
| Música | clima/descrição, duração, instrumental sim/não | áudio; duração medida |
| Efeito sonoro | descrição, duração | áudio |
| Transcrição | áudio, idioma, granularidade (segmento ou palavra) | segmentos com início/fim e texto; idioma detectado |
| Acervo | consulta, tipo (vídeo/foto), filtros (orientação, duração mín./máx., resolução mín.), página | candidatos: id estável, página de origem, URL de download, dimensões, duração, autor, **licença declarada**, miniatura |

**Trabalho assíncrono:** geração de vídeo costuma ser «envia → consulta → baixa». O contrato
expõe o estado (enviado, processando, pronto, falhou), o prazo e o id remoto, gravados no
progresso da tarefa: reinício **consulta** o trabalho em vez de reenviar (e pagar de novo).
Chave de idempotência = SHA-256 do pedido canônico.

**Erros, cada um com «retentável sim/não»:** indisponível (falta chave/binário — não);
entrada inválida (não); recusado por política de conteúdo do provedor (não; mostra o motivo);
limite de taxa (sim, com espera crescente e teto); saldo/cota esgotado (não); tempo esgotado
(sim, uma vez); **saída inválida** — o arquivo não passou na conferência de formato (sim, uma
vez); custo real acima da reserva (não; para a produção).

### 3.2 Custo e orçamento (aqui vira código, não prosa)

Medido no estudo de 09/10: na origem o rastreador de custo existe mas **nenhuma ferramenta o
chama**, e o orçamento do manifesto não tem leitor. Aqui:

1. **Estimar** na proposta: cada linha (provedor × quantidade × preço do operador); total ou
   «não medido».
2. **Reservar antes** de cada chamada paga: debita a estimativa da conta da tarefa (a mesma conta
   encadeada do `orcamento.rs`, que soma descendentes). Reserva que não cabe = **recusa antes de
   chamar**. Diverge do R3 de hoje (que corta *depois* da chamada; excesso máx. = 1 chamada):
   uma chamada de vídeo pode ser o orçamento inteiro.
3. **Conciliar** com o custo real (do cabeçalho/resposta do provedor quando houver; senão a
   própria cotação) e **estornar** a diferença.
4. **Primeiro uso pago de cada provedor** numa tarefa pede aprovação humana; acima de um limiar
   por ação (configurável), também.
5. **Sem preço = sem chave**: o broker só concede chave de provedor que tem cotação no arquivo do
   operador.
6. Total do vídeo = tokens do LLM (R2) + mídia; basta uma parcela «não medida» para o total ser
   «não medido». Moeda única por tabela; sem câmbio.

### 3.3 Proveniência e licença por ativo

Todo arquivo que entra numa produção tem, **obrigatoriamente**: origem (provedor ou «operador»),
URL da página de origem, URL de download, autor (texto **sanitizado** — a Wikimedia devolve o
autor em HTML, medido abaixo), **licença declarada pela fonte** (texto + URL da licença),
«exige atribuição?», data da obtenção, SHA-256 do arquivo, e, se gerado: provedor, modelo,
semente, pedido. Para material do operador: «declarado pelo operador» + quem declarou.

Licença é **texto declarado, nunca booleano «livre»**, e se classifica pelo
`phxclaw-provenance-core` (que já existe: permissiva / copyleft / proprietária / desconhecida →
permitir / quarentena / negar). Política padrão (decisão do pesquisador, conservadora):

| Licença declarada | Decisão padrão |
|---|---|
| Domínio público, CC0, licença da própria fonte que permita uso comercial sem atribuição | entra |
| CC BY (qualquer versão) | entra **com crédito obrigatório** no arquivo de créditos e no cartão final |
| CC BY-SA | **quarentena**: só com aprovação no portão 2, avisando que o vídeo pode herdar a obrigação de compartilhar pela mesma licença (raciocinado; não é parecer jurídico) |
| NC (não comercial) ou ND (sem derivados) | **negada** (montar é derivar; uso pelo cliente pode ser comercial) |
| Ausente, ilegível, «com ressalvas» sem texto | **negada** na composição; aparece no portão 2 com o motivo |

Créditos: arquivo `creditos.txt` (e, se pedido, cartão final) gerado **da ficha**, nunca digitado.

### 3.4 Fontes de acervo sem chave (medido hoje, 09/10/2026)

| Fonte | Licença por item? | Medido |
|---|---|---|
| Wikimedia Commons (API `action=query`, `prop=imageinfo`, `extmetadata`) | **sim**: nome curto da licença, «atribuição exigida», autor (**em HTML** — sanitizar), duração, dimensões, MIME | consulta «ocean» em vídeo: 1º resultado CC BY 3.0, `AttributionRequired=true`, 1920×1080, 83 s, 24.312.520 bytes, `video/webm` |
| Internet Archive (`advancedsearch.php`, campo `licenseurl`) | **sim** (quando o depositante preencheu) | coleção Prelinger: 3 de 3 itens com `licenseurl` de domínio público |
| NASA Image and Video Library (`images-api.nasa.gov/search`) | **não**: o item traz centro, data, descrição, palavras-chave, tipo, id, título — **sem campo de licença** | a licença sai da **política da fonte** (pessoas, logotipos e material de terceiros têm ressalvas), registrada como «política da fonte, não do item» |
| Pexels, Pixabay, Unsplash | exigem **chave gratuita** e têm licença **própria** (não é domínio público) | não consultadas hoje; entram depois, com a licença de cada uma lida e citada |

### 3.5 Legendas

- Formatos: **SRT** (sempre; universal) e **ASS** (quando há estilo: fonte, cor, contorno,
  posição; e é o que o filtro de legenda do FFmpeg/libass queima). Em MP4 como faixa: `mov_text`
  com idioma ISO 639-2 (medido: `ffmpeg -c:s mov_text -metadata:s:s:0 language=por` gera a faixa
  em 0,12 s e o `ffprobe` a lista com `language=por`).
- Regras de legibilidade (fonte primária: guias de legenda da Netflix): **≤ 42 caracteres por
  linha, ≤ 2 linhas, ≤ 17 caracteres por segundo** em pt-BR
  (<https://partnerhelp.netflixstudios.com/hc/en-us/articles/215600497>); **duração mínima de 20
  quadros e intervalo mínimo de 2 quadros** entre legendas
  (<https://partnerhelp.netflixstudios.com/hc/en-us/articles/360051554394>). Todos configuráveis.
- Validação: ordem crescente, sem sobreposição, dentro da duração do vídeo, texto não vazio,
  quebra que não corta palavra.
- Segurança: texto vindo do LLM ou da transcrição **escapa** os metacaracteres de cada formato
  (em ASS, chaves de marcação e barra invertida; em SRT, a seta de tempo e a linha em branco que
  encerra o bloco). Texto nunca entra no grafo de filtros (§4.2).

### 3.6 O que conferir no MP4 final (o `laudo`)

O conferidor roda no **hospedeiro**, sobre o arquivo final, com o `ffprobe`/`ffmpeg` do
operador **no sandbox sem rede**, mais leitura direta das caixas do MP4 em Rust.

| # | Conferência | Como | Reprova quando |
|---|---|---|---|
| 1 | Contêiner | ler as caixas de topo em Rust: `ftyp` primeiro; `moov` **antes** de `mdat` (início rápido na web) | caixa ausente, tamanho de caixa além do arquivo, `moov` no fim quando o contrato pede web |
| 2 | Faixas | `ffprobe -show_streams` | ≠ 1 faixa de vídeo; faixa de áudio ausente quando prometida; faixa de legenda ausente quando prometida em faixa, ou sem idioma |
| 3 | Vídeo | codec, largura × altura, quadros/s, formato de pixel (`yuv420p` p/ compatibilidade), número de quadros | qualquer valor ≠ contrato; quadros ≠ duração × fps (± 1) |
| 4 | Duração | `format.duration` contra a soma da `linha_do_tempo` | desvio > 2 quadros (o grafo é nosso, então a duração é **calculada**, não estimada) |
| 5 | Áudio | codec, taxa (48 kHz), canais prometidos; **volume integrado** e pico real por `ebur128` | fora do alvo ± 1 LU, ou pico acima do teto (alvo e teto configuráveis por destino; padrão provisório −16 LUFS / −1 dBTP, **raciocinado** — a norma primária, AES TD1008 / EBU R128 s2, ainda não foi lida) |
| 6 | Imagem | `blackdetect` e amostra de N quadros (PNG) para o portão humano e, se configurado, para a ferramenta de visão | trecho preto > 0,5 s não planejado |
| 7 | Silêncio | `silencedetect` nos trechos com narração planejada | silêncio > 2 s dentro de cena narrada |
| 8 | Legenda | arquivo SRT revalidado (§3.5); se queimada: registrar «queimada — conferida só por amostra de quadro» | SRT inválido; tempo além da duração |
| 9 | Proveniência | todo ativo da `linha_do_tempo` existe na ficha com licença aceita; créditos contêm todos os que exigem atribuição | ativo sem licença ou negado; crédito faltando |
| 10 | Integridade | SHA-256 do MP4 no laudo; tamanho ≤ teto | — |

O laudo diz **o que foi medido e o que não foi** (sem `ffprobe` = «duração não conferida», nunca
«ok»). MP4 reprovado **não entra** na pasta da tarefa (mesmo molde do WAV truncado em `voz.rs`).

---

## 4. Mapa para o PhxClaw

### 4.1 O que já existe e se reaproveita (não se duplica)

| Peça | Onde | Papel na produção de vídeo |
|---|---|---|
| Motor de fluxo (DAG, retomada com `fluxo_sha256`, `esperar` que descarrega para o disco, versão publicada × rascunho, galeria) | `fluxos.rs`, `fluxo_versoes.rs`, `fluxo_modelos.rs` | as etapas da §1 são passos; os portões são `esperar`; cada tipo da §2 é um modelo da galeria |
| Portão único de capacidade | `Agent::call_tool` | toda ferramenta nova passa por ele; capacidade nova `media.video` **fora** de `CAPACIDADES_PADRAO` |
| Sandbox (bwrap sem rede, binário e entradas só leitura, saída em pasta temporária) | `phxclaw-sandbox`, `visao::isolado_com` | FFmpeg e ffprobe rodam aqui |
| SecretBroker (concessão por nome, curta) | `phxclaw-secret-broker` | chaves de provedores; nunca em argv |
| Custo R2 e orçamento R3 | `custo.rs`, `orcamento.rs` | base da reserva-antes (§3.2) |
| Voz (comando externo com marcadores, ElevenLabs, WAV conferido) | `voz.rs`, `elevenlabs.rs` | provedor de voz da §3.1, já pronto |
| Transcrição (whisper.cpp com SHA-256 conferido) | `phxclaw-media-intelligence`, `visao.rs` | tempos das legendas; cabeça falante, podcast, cortes, localização |
| Imagem (OpenAI-compatível, ComfyUI, Nano Banana, SVG pelo Chromium) | `midia.rs`, `nanobanana.rs`, `visao.rs` | provedor de imagem; telas desenhadas do explicativo |
| Nó HTTP + egress por origem | `fluxo_http.rs`, `phxclaw-egress-broker` | as buscas de acervo correm **no nosso processo**, com lista de origens — onde o filtro por destino já existe |
| Firewall de proveniência | `phxclaw-provenance-core`, ADR-0060 | classificação de licença por ativo (§3.3) |
| Esquema (todos os erros de uma vez, com caminho) | `esquema.rs` | validar cada artefato da §1.1 |
| Evidência | `phxclaw-evidence-ledger` | registro de decisões e de custo |
| Captura de tela | `phxclaw-system-automation::ffmpeg_record_command` | demonstração de tela |

### 4.2 O que precisa nascer

Novo crate **`phxclaw-video`** (Rust, sem dependência nova fora do workspace; nenhuma GPL), e
ferramentas finas em `phxclaw-agent/src/video.rs` que o registram no portão.

| Peça | Requisito | Critério de aceite testável |
|---|---|---|
| **Tipos dos artefatos** (`briefing` … `laudo`, contrato de entrega) | structs com serde + esquema JSON para o `esquema.rs`; versão no artefato | artefato inválido recusado com **todos** os erros e caminhos; ida e volta JSON sem perda |
| **Linha do tempo → grafo de filtros → argv** | função pura: recebe a linha do tempo e os caminhos **já copiados para a pasta de trabalho com nomes gerados por nós** (`a0001.png`…), devolve `Vec<String>` de argumentos; nada de shell; texto nunca entra no grafo (legenda vai por arquivo) | (a) teste de ouro do argv para 3 linhas do tempo de referência; (b) ativo com nome original contendo `;`, `[`, `'`, `,` e `:` não altera o grafo; (c) duração do MP4 = soma planejada ± 2 quadros em 5 linhas do tempo diferentes (imagens, clipes, misto, com transição, com legenda queimada) |
| **Operações de imagem** | imagem parada com movimento de câmera (aproximar/afastar/deslocar), ajuste à proporção (cortar ou preencher), duração por cena | quadros = duração × fps exatos; sem distorção (SAR 1:1) |
| **Operações de clipe** | recorte por entrada/saída, mudança de velocidade, ajuste de proporção, retirada do áudio original ou mistura | duração do recorte ± 1 quadro |
| **Transições** | lista **fechada e pequena** (corte seco, fusão, fade de/para preto); a transição consome tempo e o cálculo de duração sabe disso | duração total com N transições = soma − N × t (medido no MP4) |
| **Áudio** | trilhas (narração, música, efeitos) com volume, fade relativo **à trilha** (não à linha do tempo), atraso, abaixamento da música sob a voz, normalização **em duas passadas** (medir, depois aplicar) | volume integrado dentro de ± 1 LU do alvo (a uma passada falhou: −20,5 contra −16, §6.2) |
| **Legendas** | gerar SRT/ASS dos segmentos; validar (§3.5); queimar (filtro de legenda) ou anexar como faixa `mov_text` com idioma | SRT aceito pelo FFmpeg; texto com `{\`, `-->` e linha vazia escapado; legenda em faixa listada pelo `ffprobe` com idioma |
| **Execução do FFmpeg** | sempre pelo `isolado_com`: sem rede, entradas só leitura, saída na pasta temporária, `-protocol_whitelist file` (o FFmpeg aceita listas de reprodução que apontam para URL ou arquivo local — **raciocinado**, a bancada prova o bloqueio), teto de tempo e de disco | arquivo de entrada que é uma lista de reprodução apontando para `/etc/passwd` ou `http://` **falha**; sem FFmpeg instalado, a ferramenta diz qual variável configurar |
| **Conferidor (`laudo`)** | §3.6 inteira; caixas MP4 lidas em Rust; `ffprobe`/`ebur128`/`blackdetect`/`silencedetect` no sandbox | MP4 bom aprovado; **cada** defeito reposto reprova pelo motivo certo: truncado no meio do `mdat`, sem áudio, resolução errada, duração errada, `moov` no fim, volume fora do alvo, trecho preto, legenda além do fim |
| **Entrada de mídia não confiável** | todo arquivo baixado ou recebido passa por `ffprobe` no sandbox e precisa ser de formato da lista (MP4/MOV/WebM/Matroska, PNG/JPEG/WebP, WAV/MP3/AAC/Opus/FLAC); teto de bytes e de duração por ativo | lista de reprodução, HTML ou executável disfarçado de `.mp4` recusado antes da montagem |
| **Contratos de provedor** | o traço da §3.1 (identidade, disponibilidade, cotação, pedido, resposta, erros, assíncrono) — em `phxclaw-media-intelligence`, que já guarda os provedores de fala/transcrição; os de hoje (`voz.rs`, `midia.rs`, ElevenLabs, Nano Banana) passam a implementá-lo **sem segunda cópia** | os provedores existentes respondem ao mesmo contrato; um provedor falso em loopback exercita cada classe de erro |
| **Acervo livre** | adaptadores Wikimedia, Internet Archive, NASA pelo cliente HTTP do nosso processo e o egress por origem; candidatos normalizados com proveniência; download com teto de bytes | servidor falso em loopback; candidato sem licença não entra na ficha; autor com `<script>` sai sanitizado nos créditos |
| **Reserva de custo** | §3.2 no portão | provedor falso que conta chamadas: orçamento insuficiente → **0 chamadas**; reinício de trabalho assíncrono → 0 reenvios |

### 4.3 O que **não** se traz, e a restrição nossa que causou a divergência

| Na origem | Aqui | Restrição que causou |
|---|---|---|
| Composição por React (Remotion) ou HTML num navegador | grafo de filtros do FFmpeg gerado em Rust | licença comercial do Remotion acima de 3 pessoas; «Rust sempre que possível» (dono, 09/10); a duração passa a ser **calculada** |
| Orquestração em prosa que o LLM lê (~76 mil tokens de roteiro, estudo §1) | fluxo declarativo do nosso motor; o LLM só escreve artefatos com esquema | rede neural local pequena (dono, 09/10): o roteiro de produção não cabe; o determinístico não precisa de modelo |
| O agente roda scripts com rede aberta e chave no ambiente | buscas no nosso processo pelo egress por origem; FFmpeg sem rede | sandbox sem filtro de destino para processo-filho (estudo §3) |
| Custo como conselho | reserva antes, no portão | «configuração que não é lida mente» |
| Revisão pelo próprio agente | `laudo` por código + portões humanos | saída de terceiro só entra conferida |
| Licença como texto livre sem política | classe + decisão pelo firewall de proveniência | ADR-0060 (falha fechada) |
| Dezenas de provedores pagos de vídeo, voz e imagem | contrato único; começa por voz/imagem que já existem + acervo sem chave | catálogo de 62 ferramentas de API a manter (estudo b7) contra 5 nossas |

### 4.4 Voz sem GPL — licenças medidas hoje

| Motor | Licença do código (arquivo lido) | pt-BR? | Veredito |
|---|---|---|---|
| `piper-tts` atual (OHF-Voice/piper1-gpl) | GPL-3.0 (`COPYING`) | sim | **não entra** embarcado |
| Piper antigo (rhasspy/piper) | MIT (`LICENSE.md`) — mas o fonemizador usa espeak-ng | sim, via espeak-ng | **não entra** embarcado: espeak-ng é GPL-3.0 (`COPYING`) |
| Kokoro (hexgrad/kokoro) | código Apache-2.0; pesos Apache-2.0 (cartão do modelo) | **sim, pelo espeak-ng**: no fonte (`kokoro/pipeline.py`), `pt-br` cai no fonemizador do espeak-ng | só como **comando externo do operador** |
| kokoro-onnx | MIT | idem (mesmo fonemizador) | idem |
| sherpa-onnx | Apache-2.0 — mas traz o espeak-ng ligado estaticamente (medido em 01/10, `voz.rs`) | sim | idem |
| MeloTTS | MIT | **não** (README: inglês, espanhol, francês, chinês, japonês, coreano) | não resolve pt-BR |
| Flite | BSD-like (`COPYING`) | **não** (vozes inglesas); vem **dentro do FFmpeg** desta máquina (`--enable-libflite`) | serve para teste e para inglês |
| RHVoice | GPL-2.0 (`LICENSE.md`) | — | não entra |

**Recomendação (decidida):** o PhxClaw **não embarca motor de voz**. A narração vem, em ordem de
preferência para a primeira entrega: (1) **voz humana gravada pelo operador** (WAV na pasta da
tarefa); (2) o **comando externo** que o operador já configura no `voz.rs` (a licença é dele, como
hoje); (3) provedor HTTP com chave (ElevenLabs já existe). Os testes automáticos usam o Flite
pelo próprio FFmpeg (inglês, BSD-like, zero instalação).

---

## 5. Fatiamento em sprints

Numeração **provisória**: o registro `docs/absorcao/SPRINTS.md` termina em SP000036 e as sessões
já usaram SP000037/38; o integrador confirma. Cada sprint entrega **prova real** (o teste falha
com o defeito reposto) e nenhuma depende de chave paga, exceto a SP000045.

| Sprint | Entrega | Prova real | Depende |
|---|---|---|---|
| **SP000039** | crate `phxclaw-video`: tipos dos artefatos + contrato de entrega + **conferidor de MP4** (§3.6) | MP4 bom aprovado; 8 defeitos repostos reprovam cada um pelo motivo certo | — |
| **SP000040** | legendas SRT/ASS: geração dos segmentos, validação, escape, faixa `mov_text` | arquivo aceito pelo FFmpeg; casos de escape; regras de legibilidade | SP000039 |
| **SP000041** | **compositor**: linha do tempo → argv do FFmpeg, imagens com movimento, clipes, 3 transições, legenda queimada/faixa, áudio com abaixamento e normalização em 2 passadas, execução no sandbox | duração ± 2 quadros em 5 linhas do tempo; injeção por nome de arquivo inerte; lista de reprodução maliciosa recusada; volume ± 1 LU | SP000039–40 |
| **SP000042** | **acervo livre** sem chave (Wikimedia, Internet Archive, NASA) + proveniência + política de licença + créditos | servidor falso em loopback; sem licença = fora; autor HTML sanitizado; egress recusa origem fora da lista | SP000039 |
| **SP000043** | **narração** pelo contrato de provedor (voz do operador, `voz.rs`, ElevenLabs); tempo das legendas pela transcrição da própria narração | duração real da narração realimenta as cenas; legenda alinhada (desvio medido e registrado) | SP000040 |
| **SP000044** | **primeira entrega útil**: modelo de fluxo «documentário de acervo narrado» na galeria — briefing → roteiro → decupagem → portão 1 → acervo → portão 2 → narração → legendas → montagem → render → laudo → entrega | vídeo de 30–60 s **sem chave paga**, laudo aprovado; reinício durante o `esperar` retoma sem refazer etapa; ativo negado no portão 2 não aparece no MP4 | SP000041–43 |
| **SP000045** | **reserva de custo** de provedor de mídia (§3.2) no portão, para os provedores pagos que já existem | provedor falso: orçamento insuficiente → 0 chamadas; primeiro uso pago pede aprovação; reenvio após reinício = 0 | R3 |
| **SP000046** | **cabeça falante + fábrica de cortes** (mesmas peças: transcrever, cortar silêncio, reenquadrar 9:16, legendar, N laudos) | vídeo do operador → N cortes conferidos | SP000044 |
| SP000047 ⏸ | podcast em vídeo (audiograma) e demonstração de tela | — | SP000046 |
| SP000048 ⏸ | localização (legendas traduzidas; dublagem só com provedor) | — | SP000046 |
| SP000049 ⏸ | geração paga de vídeo/música/avatar | só depois do **proxy por origem** para processo-filho (estudo §3) | SP000045 |

Entram na conta do que falta: **SP000039–SP000046** (8). As ⏸ ficam visíveis e fora da
porcentagem (regra «escopo congelado» do dono, 24/09). Estado de hoje: especificação 1/1
entregue; implementação **0 de 8** (100% por fazer).

---

## 6. Riscos

### 6.1 Tabela

| Risco | Medido / raciocinado | Mitigação |
|---|---|---|
| **Direitos do acervo** | Medido: licença por item só em 2 de 3 fontes sem chave; Wikimedia devolve CC BY-SA e CC BY com atribuição obrigatória; NASA só por política da fonte | §3.3: falha fechada; BY-SA em quarentena; NC/ND negadas; créditos gerados; aviso «licença declarada pela fonte, não validada por nós» |
| **Pessoas, marcas e obras dentro do material livre** | Raciocinado: licença da foto não libera direito de imagem de quem aparece nem marca registrada | campo «tem pessoa identificável/marca?» na ficha (marcado pelo LLM pela miniatura, confirmado no portão 2) |
| **Custo de provedor pago** | Medido no estudo: na origem o custo é prosa; uma chamada de vídeo pode valer o orçamento inteiro | reserva antes (SP000045); `pago` fora da primeira entrega |
| **Conteúdo gerado** | Raciocinado: imagem/voz gerada pode imitar pessoa real ou conter texto enganoso | gerado marcado na ficha e nos créditos («gerado por IA: provedor, modelo»); voz clonada de terceiro recusada sem declaração do operador |
| **Disco** | Medido: 4,2 GB livres nesta máquina (89% usado); 1 vídeo de acervo de 83 s = 24,3 MB; saída de 28 s em 1080p = 4,87 MB | teto de bytes por ativo e por produção; pasta de trabalho apagada ao fim, mantidos MP4, laudo, ficha e créditos |
| **Tempo de render** | Medido (§6.2): 1,5× a 2,0× a duração em 1080p num host de 4 vCPU compartilhado | `teto_ms` por passo; 720p como padrão de rascunho (0,7× medido) |
| **FFmpeg como superfície de ataque** | Raciocinado: decodificar mídia baixada da internet; listas de reprodução que apontam para outros arquivos/URLs | sandbox sem rede, entradas só leitura, lista branca de protocolo e de formato, `ffprobe` antes |
| **Licença do FFmpeg e patentes de codec** | Medido: o FFmpeg desta máquina é compilado com `--enable-gpl` e `libx264`. Raciocinado: processo externo não contamina; **embarcar** no instalador seria decisão de produto (mesmo caso do `voz.rs`); H.264 tem licenciamento de patente | FFmpeg do operador; codec configurável (AV1 por `libsvtav1` e VP9 disponíveis no mesmo binário) |
| **Contaminação AGPL** | o risco desta fase | §7 |

### 6.2 Bancada desta especificação (medido, n pequeno, máquina compartilhada com outras frentes)

Entrada: 5 imagens 1920×1080, narração Flite de 7,89 s, música sintética, SRT de 3 blocos.
Saída: H.264 `veryfast` CRF 23 + AAC 128 kb/s, `+faststart`. FFmpeg 6.1.1, 4 vCPU.

| Variante | Relógio | Arquivo |
|---|---|---|
| 1080p30, movimento de câmera nas 5 imagens, 4 fusões, legenda queimada, abaixamento + normalização (n=3) | **41,0 / 43,7 / 55,6 s** para 28 s de vídeo | 4.871.613 bytes |
| 1080p30 sem movimento de câmera, sem música (n=1) | 32,9 s | 824.998 bytes |
| 720p30 com movimento de câmera (n=1) | 19,3 s | 2.297.833 bytes |

`ffprobe` da 1ª variante: H.264 1920×1080 30/1, **840 quadros**, AAC 48 kHz **mono**, duração
**28,000000** (planejado 5 × 6 − 4 × 0,5 = 28). `ebur128`: **−20,5 LUFS** com alvo −16 numa
passada (H4 morreu). Sem trecho preto, sem silêncio ≥ 2 s. Comandos e entradas na pasta de
rascunho da sessão (não versionados); a SP000041 refaz com o compositor nosso.

O que decidiria os números de verdade: corrida em máquina dedicada, n ≥ 5, com clipes reais de
acervo (não imagens de teste), em 720p e 1080p.

---

## 7. Higiene da sala limpa

**Declaração.** Este documento **não reproduz** código, prompts, instruções de agente, arquivos
YAML, esquemas JSON, nomes de função, nomes de campo nem trechos de texto do OpenMontage
(AGPL-3.0, `calesthio/OpenMontage`, commit `9327439`). Ele descreve **comportamento observável e
requisitos** com vocabulário nosso. Os nomes dos artefatos (`briefing`, `pesquisa`, `proposta`,
`roteiro`, `decupagem`, `ficha_de_ativos`, `linha_do_tempo`, `laudo`, contrato de entrega) são
escolha nossa em português; termos de ofício do audiovisual (roteiro, decupagem, corte, fusão,
legenda) são vocabulário da indústria. Limiares numéricos usados aqui vêm de **fonte pública
citada** (guias da Netflix) ou da **nossa bancada** (§6.2), não do código de origem. Os nomes de
tipos de produção na §2 descrevem gêneros de vídeo, não identificadores de lá.

**Regra para a equipe de implementação:** não abrir o repositório de origem; o commit de cada
frente diz «implementado só a partir de `docs/propostas/openmontage-especificacao.md`». Dúvida de
comportamento volta ao papel J, que responde **em palavras**, nunca colando trecho.

**O que o pesquisador leu (para auditoria), no clone em
`/tmp/…/scratchpad/openmontage/repo` @ `9327439`:**

- listagens de pastas: raiz, `pipeline_defs/`, `skills/` (e subpastas), `tools/` (e subpastas
  `video/`, `video/stock_sources/`, `subtitle/`, `audio/`, `analysis/`, `publishers/`,
  `provider_contracts/`), `lib/`, `schemas/`;
- `pipeline_defs/*.yaml`: um manifesto inteiro (montagem documental) e, dos demais, só nome,
  descrição, etapas, entradas/saídas de cada etapa, aprovação padrão e ferramentas por etapa,
  extraídos por script;
- `schemas/artifacts/*.json` e `schemas/checkpoints/checkpoint.schema.json`: só os nomes dos
  campos de primeiro nível, extraídos por script;
- `lib/delivery_promise.py` (inteiro), `lib/slideshow_risk.py` (cabeçalho);
- `tools/video/stock_sources/base.py` (inteiro); `tools/subtitle/subtitle_gen.py` e
  `tools/audio/audio_mixer.py` (busca por padrões); `tools/video/video_compose.py` (cabeçalho,
  lista de funções e a seção de revisão final);
- `skills/meta/checkpoint-protocol.md` (primeiras 80 linhas), `skills/meta/reviewer.md`
  (títulos e a seção de decisão), `skills/meta/creative-intake.md` (perguntas iniciais),
  `AGENT_GUIDE.md` (títulos e a seção do orquestrador), `skills/pipelines/localization-dub/`
  (títulos);
- e o estudo anterior, `docs/propostas/openmontage-2026-10.md`.

Nada foi copiado para o repositório do PhxClaw; o clone continua fora dele.

---

## 8. Matriz de evidência, lacunas e o que sobe ao dono

| Fonte | O que resolve | Custo de obter |
|---|---|---|
| OpenMontage @ `9327439` (lista da §7) | etapas, tipos, onde há aprovação, o que a conferência deles cobre | ler (já pago) |
| Bancada FFmpeg 6.1.1 nesta máquina | H1, H4, H5; tempo e tamanho de render | 4 corridas, ~3 min |
| API Wikimedia Commons, Internet Archive, NASA (consultas de hoje) | H3; campos de licença por fonte | 3 requisições |
| `COPYING`/`LICENSE` de piper1-gpl, piper, espeak-ng, kokoro, kokoro-onnx, misaki, sherpa-onnx, MeloTTS, flite, RHVoice (raw.githubusercontent.com); README do kokoro e do MeloTTS; cartão do Kokoro-82M no Hugging Face | H2 | ~14 requisições |
| Guias de legenda da Netflix (pt-BR e tempos) | limites de legibilidade e de tempo | 2 requisições |
| PhxClaw: `fluxos.rs`, `fluxo_modelos.rs`, `voz.rs`, `midia.rs`, `custo.rs`, `orcamento.rs`, `phxclaw-provenance-core`, `phxclaw-media-intelligence`, `phxclaw-sandbox`, ADR-0060 | §4.1 | ler |

**Lacunas (não medido):** alvo de volume na norma primária (AES TD1008 / EBU R128 s2 — só fonte
secundária vista); qualidade e velocidade de um motor de voz pt-BR do operador; tempo de render
com clipes reais e em máquina dedicada; se o FFmpeg do operador no Windows tem `libass` (sem ele
não há legenda queimada); licenças próprias de Pexels/Pixabay/Unsplash (lidas na sprint que as
trouxer); o bloqueio de listas de reprodução pelo `-protocol_whitelist` (prova na SP000041).

**O que sobe ao dono:** **nada bloqueante.** A primeira entrega não depende de decisão dele.
Ficam registradas, para quando alguém as propuser: (produto) **embarcar** no instalador um FFmpeg
ou um motor de voz GPL; (produto) oferecer vídeo gerado ao cliente — preço, SLA, quem arca com
provedor pago e com direito de imagem do acervo; (fora do alcance da pesquisa) parecer jurídico
sobre CC BY-SA em obra derivada e sobre patentes de H.264.

*Nenhum código escrito, nada instalado, nada comitado.*
