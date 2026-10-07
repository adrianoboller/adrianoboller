# PhxZip: o que existe e o que falta (pedidos 454 e 455) — papel E, 07/10/2026

Só leitura. Nada compilado nem medido nesta rodada: os números abaixo são
`wc`/`grep` do dia, e os da prova (35/35, 12/12, contraste) são os que o 455
registra, **não re-rodados aqui**.

## 1. O que existe

| peça | estado | onde |
|---|---|---|
| biblioteca (no_std, 7z: Copy/LZMA/LZMA2/7zAES, leitor e escritor, `.phz`) | existe | `crates/phxzip/src` (`Leitor::abrir/entradas/extrair/percorrer`, `Escritor::novo/arquivo/pasta/terminar`, `phz::empacotar/desempacotar`) |
| tela web | existe, provada contra servidor FALSO | `crates/phxzip-web/ui/` — `index.html` 225 l, `phxzip.css` 504 l, `phxzip.js` 1426 l, `textos.json` 1389 l (173 chaves × 6 idiomas), Exo 2 local + `OFL.txt` |
| contrato HTTP | existe | `docs/PHXZIP-WEB.md` (413 l; rotas `/api/{estado,idiomas,compactar,listar,testar,extrair}`, envelope `PZW1`, só 127.0.0.1) |
| prova no navegador | existe | `testes-web/phxzip/` (`exercitar.mjs`, `prova-das-guardas.mjs`, `servidor_falso.py`) |
| servidor web de verdade | **NÃO existe** — `crates/phxzip-web/` só tem `ui/`, sem `Cargo.toml` nem `src/` | — |
| PhxZipCmd (terminal) | **NÃO existe** | — |
| manual | **NÃO existe** (e só se escreve com as 3 peças medidas) | — |
| pacote só do PhxZip | **NÃO existe**; `empacotar.sh` é do PhxSql | — |

A tela já cumpre a marca: `#010418`, Exo 2 servida pela própria porta, vermelhão
`#C63C0A` e as cinco cores da ação escurecidas no tema claro (`:root[data-tema="claro"]`),
`.acao.{incluir,alterar,marcar,excluir,consultar,neutra}` em contorno com fundo só no
`:hover`, nenhum seletor global de controle, nenhum `text-transform` em `.dado`,
`[hidden]` vencendo qualquer `display`, `prefers-reduced-motion`, quebras em 820/640 px.
O que o pedido 454 chama de «desenho da tela» está, portanto, **feito**; falta
ligá-la a um servidor real e fechar três dívidas que o próprio 455 registra.

## 2. O que falta para cada pedido fechar

**454 (☐)** — duas das três peças não existem.
1. Servidor web: crate nova, HTTP **extraído** de `phxsql-server/src/http.rs` +
   `fio.rs` (`ler_pedido_medindo`, `responder*` já são `pub`), não copiado; drenar o
   corpo após o `413` (sem dreno o Chromium vê `Failed to fetch`); `PORTA_PADRAO`
   constante única (sugestão 7700); presa a 127.0.0.1; `Host`/`Origin`/`Content-Type`
   conferidos; zip-slip recusado no motor (`conferir_nome`).
2. PhxZipCmd: binário de terminal sobre a **mesma** API pública (`compactar`,
   `listar`, `testar`, `extrair`), sem segunda implementação; senha por
   variável de ambiente/stdin, nunca argumento.
3. Recusa nomeada, sem código morto, de Deflate/Deflate64/BZip2/PPMd/BCJ/ZIP/ZipCrypto,
   MD5 e SHA-1. **A auditar:** `crates/phxzip/tests/fixtures/legado-{bzip2,deflate,ppmd,bcj}.phz`
   sugerem que o leitor ainda decodifica legado — conferir se isso é recusa ou
   código vivo (conflita com o 454 §1).

**455 (◐ — interface entrou)** — falta o (2), o (3) e o (4):
- acessor público de bloco (`Entrada::lugar` é `pub(crate)`);
- `Json::analisar` devolvendo a posição como número (a tela marca a linha do erro);
- fábrica de idiomas levada ao `phxsql-core` (hoje `textos.json` é cópia
  declarada), com o conferidor `textos-fora-da-fabrica` medindo `crates/phxzip-web/ui/`
  (isentos `Zip`/`PhxZip`) e o laço chave-pedida × chave-existente virando teste Rust;
- escritor de `.tar` ustar/pax (o «baixar tudo»);
- paleta/bandeiras: extrair tokens para um arquivo comum da marca;
- manual (depois das peças, com número medido);
- `empacotar-phxzip.sh`: binários por plataforma, manual, licença (+ `OFL.txt`),
  manifesto SHA-256, fontes das crates `phxzip*`; o script desempacota e confere.

## 3. Fatias para o engenheiro (cada uma, eu exercito depois)

| # | fatia | pronta quando | o que eu olho no navegador |
|---|---|---|---|
| F1 | extrair o HTTP de `http.rs`/`fio.rs` para o `phxsql-core` | `phxsql-server` continua verde, sem cópia | — (sem tela) |
| F2 | crate `phxzip-web` mínima: `GET /`, estáticos por `include_bytes!`, `/api/estado`, `/api/idiomas` | tela abre em 127.0.0.1:7700 sem rede, fonte local | cabeçalho, endereço, 6 idiomas, tema claro/escuro, 360 px |
| F3 | fábrica de idiomas no `phxsql-core` + `textos.json` por chave + teste do laço + conferidor | catraca só desce | nenhum texto cravado; chave morta/faltando |
| F4 | `/api/listar` e `/api/testar` | `exercitar.mjs --so abrir,listar,testar` verde contra o servidor real | senha errada, cabeçalho cifrado, nomes hostis |
| F5 | `/api/compactar` + acessor de bloco + `Json::analisar` com posição | `--so compactar,espiar` | progresso, cancelar, JSON inválido com linha marcada |
| F6 | `/api/extrair` + tar + dreno do `413` | caso «teto» com e sem dreno | mensagem do `413`, não «Failed to fetch» |
| F7 | PhxZipCmd | mesmos vetores que a web, mesmo motor | — |
| F8 | manual + `empacotar-phxzip.sh` | script desempacota e confere o manifesto | manual lido no tema claro |

Ponto de troca: a prova `testes-web/phxzip/` hoje fala com o falso na 7799;
a F2 deve aceitar `--alvo` para o roteiro rodar **igual** contra o real (mesmos
35 casos e 12 guardas), e o falso vira só regressão do contrato.

## 4. Regras que o engenheiro não pode quebrar na tela

- Rótulo por chave (`data-txt*`, `t("…")`), dado por `textContent`; nome de
  arquivo nunca em caixa alta nem passando pela fábrica.
- Cores da ação em contorno; fundo só no `hover`; claro escurece (menor contraste
  registrado: 5,30:1 escuro, 5,45:1 claro — a F2 re-mede).
- Servidor não lê caminho de disco; senha só no corpo, nunca em URL/log.
- Peça nova no fim de uma lista: procurar `find(...)` onde devia ser `filter(...)`.
