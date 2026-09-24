# A tela do PhxZip, exercitada

A tela (`crates/phxzip-web/ui/`) consome o contrato de `docs/PHXZIP-WEB.md`.
Enquanto o servidor de verdade não existe, ela se prova aqui, no Chromium,
contra um servidor **falso** que segue o contrato à letra.

```bash
node testes-web/phxzip/exercitar.mjs                     # os casos todos, com capturas
node testes-web/phxzip/exercitar.mjs --so espiar,estado  # só os casos cujo nome contém um dos pedaços
node testes-web/phxzip/prova-das-guardas.mjs             # cada guarda, nos dois sentidos
python3 testes-web/phxzip/servidor_falso.py --porta 7799 # o falso sozinho, para abrir no navegador
```

O `playwright` entra pelo caminho absoluto (`/opt/node22/lib/node_modules/…`),
como no `docs/dossie/olhar.mjs`. O roteiro sobe o falso numa porta só dele
(7799, e 7797 para o caso sem dreno) e o derruba **pelo PID do filho que
criou**. Não use `pgrep -f`/`pkill -f` com o nome do script: o padrão casa
também a linha de comando do shell que o chama, e o `kill` derruba o shell.
Medido nesta frente.

## O que há aqui

| arquivo | o que é |
|---|---|
| `servidor_falso.py` | o contrato respondido com dados de exemplo, **inclusive os erros**. Biblioteca padrão do Python, preso ao `127.0.0.1`. Decide o cenário pelo **conteúdo** do pacote (assinatura do 7z + `\0CENARIO:<nome>\n`), nunca pelo nome do arquivo. Registra linha de pedido e cabeçalhos (nunca o corpo) no `--log`, para o roteiro procurar a senha lá |
| `exercitar.mjs` | os casos: compactar, soltar, `.phz`, senhas, progresso, cancelar, teto, abrir, senha errada, listar, baixar uma, baixar tudo (conferido pelo `tar` do sistema), testar, espiar (JSON válido, inválido com a linha marcada, cortado, binário), os estados de erro, nomes hostis, o `413` com e sem dreno, a senha fora da URL, as cores e o contraste nos dois temas, os seis idiomas, o pseudoidioma e 360 px |
| `prova-das-guardas.mjs` | repõe, numa **cópia** da tela, o defeito que motivou cada guarda, e exige que a guarda falhe com a frase esperada e passe sem ele |
| `capturas/` | as telas da última corrida completa, e `resultado.json` / `prova-das-guardas.json` com a data |

## As guardas, e o defeito de cada uma

A lista viva, com o trecho que se repõe, é o `MUTACOES` do
`prova-das-guardas.mjs`. As que nasceram **nesta** tela, exercitando:

- **texto solto em flex**: «11,0 KiB  von  16,0 MiB  , die der Server
  annimmt». Filho direto de container flex perde os espaços das bordas e ganha
  o `gap` no lugar.
- **limpar a lista leva a senha junto**: a senha ficava no campo escondido, e a
  lista seguinte sairia cifrada sem ninguém ver.
- **o zero do plural**: o CLDR põe o 0 no singular em português («0 pasta»).
  Virou chave própria («nenhuma pasta»).
- **porcentagem pequena**: 21 bytes de 399 KiB saíam «0%».

As que vieram da tela do PhxSql, e aqui se provam de novo: `find` no lugar de
`filter`, dado em caixa alta, `[hidden]` vencido por `display`, contorno ×
fundo cheio, contraste no papel, chave morta × chave faltando.

## O que o falso não faz

Não comprime em 7z: o «pacote» dele começa com a assinatura do 7z e guarda os
itens num formato só dele (JSON + zlib), para a volta compactar → abrir →
espiar → baixar fechar no navegador. O 7-Zip não o abre, e nenhum número de
compressão que ele mostra vale para o PhxZip.
