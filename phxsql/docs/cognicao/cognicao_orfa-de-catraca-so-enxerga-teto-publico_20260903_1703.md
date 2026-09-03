# O detector de catraca orfa do `medir.py` so enxerga `pub const TETO*`

**Descoberto em 03/09/2026, 17:03**, fazendo o inventario completo das
catracas do repositorio (`docs/CATRACAS.md`).

## 1. O que aconteceu

`docs/qa/medir.py` ja existia e ja resolve bem o problema central: acha as
catracas SEM lista digitada, varrendo `crates/*/examples/*.rs` atras de quem
imprime `catraca:` com `--numeros`. Ele tambem tenta achar o outro lado --
constante `TETO*` que existe no codigo e que NENHUM conferidor mede -- com
esta funcao:

```python
def constantes_teto():
    for arq in RAIZ.glob("crates/*/src/**/*.rs"):
        for n, linha in enumerate(...):
            m = re.match(r"\s*pub const (TETO\w*)\s*:", linha)
```

A tarefa mandava varrer `TETO`, `MAX` e `LIMITE` -- nao so `TETO` -- e ler o
comentario ao lado de cada constante para separar catraca de limite de
funcionamento. Fazendo essa varredura mais larga, apareceram 27 constantes de
teto que NAO sao catraca. O `medir.py` reportou UMA delas como "orfa"
(`TETO_DO_REGISTRO`). As outras 26 nao apareceram na lista de orfas -- nao
porque tenham conferidor, mas porque o regex delas nunca as alcanca.

## 2. O que eu concluí primeiro, e estava errado

Ao ver a saida do `medir.py` dizendo "Constantes TETO* que NENHUM conferidor
reporta: `TETO_DO_REGISTRO`", quase escrevi no `docs/CATRACAS.md` que essa
era a lista completa de tetos sem medidor -- e que as outras 26 constantes
que eu tinha achado no meu proprio grep eram, por exclusao, cobertas por
algum conferidor que eu ainda nao tinha achado. Fui conferir cada uma antes
de escrever isso (`grep -n "cfg(test)\|assert" perto de cada constante`) e
nenhuma das 26 tem teste que a compare contra uma contagem do codigo-fonte --
sao limites de funcionamento de verdade, sem conferidor NENHUM, publicado ou
nao. O "orfa" do `medir.py` nao e a lista completa; e um subconjunto dela.

## 3. O que a medição disse

Das 27 constantes de limite (nao-catraca) achadas pela varredura larga:

- **1** e `pub const TETO_DO_REGISTRO` -- essa o regex alcanca, e o
  `medir.py` a relata como orfa corretamente.
- **7** sao `const TETO_*` **privado** (`TETO_DA_CASCATA`, `TETO_PIVOT`,
  `TETO_JUNCAO`, `TETO_DO_LOTE_SERVIDO`, `TETO_DO_CAMPO`, `TETO_DO_ERRO`,
  `TETO_DO_CABECALHO`) -- o regex exige `pub const`, entao ficam invisiveis
  por causa da visibilidade.
- **1** e `static TETO` (`phxsql-core/src/paralelo.rs`) -- o regex so casa
  `const`, entao fica invisivel por causa da PALAVRA-CHAVE, mesmo sendo
  nomeada exatamente `TETO`.
- **18** nao comecam com o prefixo `TETO` (`MAX_*`, `LIMITE_*`, `VALOR_MAX`,
  `OFFSET_MAXIMO`, `CADEIA_MAXIMA`...) -- invisiveis por causa do NOME,
  independente de visibilidade.

Total: **26 de 27** limites de funcionamento sao invisiveis para o detector
de orfas do `medir.py`, so por causa da forma da constante (privada, palavra-
chave diferente, ou nome fora do prefixo), nao porque tenham medidor.

## 4. A regra

**Um detector de "catraca sem medidor" so vale o que a forma da constante que
ele procura permite enxergar.** Um regex que exige `pub const TETO\w*` nao
mede "toda catraca sem conferidor" -- mede "toda catraca **publica e
nomeada com este prefixo** sem conferidor". A diferenca so aparece quando
alguem varre por fora, com uma regra mais larga (aqui, `TETO|MAX|LIMITE`,
publico ou privado) -- exatamente a mesma licao do `TETO_TABELA_NA_MAO`
subcontando antes do conferidor aprender a ver o ajudante `tabela(`, so que
desta vez o alcance estreito e da ferramenta que ACHA a lacuna, nao da
catraca em si.

Isto nao e um defeito do `medir.py` a corrigir sozinho: nenhuma das 26
invisiveis e catraca-que-deveria-ter-conferidor-e-nao-tem -- todas sao
limites de funcionamento legitimos, e a intencao original do detector
("catraca `TETO*` publica esquecida") continua servindo esse proposito
estreito. O aprendizado e o ALCANCE: quem ler "so achei uma orfa" no
`medir.py` nao pode concluir "so existe uma constante de teto sem
conferidor" -- so pode concluir "so existe uma constante `pub const TETO*`
sem conferidor".

## 5. Como está guardado hoje

`docs/CATRACAS.md` (secao "Os limites de funcionamento encontrados") lista
as 27 constantes com a varredura larga, e explica quais das 26 restantes
escapam ao `constantes_teto()` do `medir.py` e por que. O `medir.py` em si
**nao foi alterado** nesta rodada -- ampliar o regex dele para pegar `const`
privado, `static` e nomes `MAX_*`/`LIMITE_*` misturaria limites de
funcionamento na lista de "orfas", que hoje so promete achar catraca
esquecida, nao limite sem documentacao. Ampliar essa promessa e decisao de
quem mantem o QA-PDCA, nao desta rodada. **Fica registrado como falta, e nao
como feito**: se um dia alguem quiser um detector que tambem ache limite de
funcionamento sem `///` explicando o numero, o material desta cognicao e o
ponto de partida.
