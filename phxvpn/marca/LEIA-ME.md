# Marca do phxvpn

Derivada da marca Phoenix do PhxSql (paleta, Exo 2, fundo `#010418`), com
duas trocas decididas pelo dono em 24/09/2026:

- **cadeado no peito** da fênix, no lugar do cilindro de dados;
- **cano nos pés**: o túnel, com setas nos dois sentidos e trilhas de circuito.

## Um desenho só

`simbolo.svg` é a única fonte do desenho. Tudo mais sai dele:

| Arquivo | Gerado por | Uso |
|---|---|---|
| `logo-horizontal.svg`, `logo-vertical.svg`, `icone-app.svg` | `python3 gerar.py` | documentos, instalador, site |
| `folha.html` | `python3 gerar.py` | folha de marca para revisão |
| `png/*.png` (32 a 2400 px, fundo transparente) | `node renderizar.mjs` | onde SVG não entra |
| ícone e favicon da janela e do painel | `include_str!` em `src/web.rs` | o binário serve `/simbolo.svg` |

Os logos com texto embutem a Exo 2 (SIL OFL) em base64, para renderizar
igual em qualquer lugar.

## Pendente, decisão do dono

- **Lema:** «Built to connect. Engineered to protect.» é proposta, no molde
  do «Built to store. Engineered to scale.» do PhxSql.
- **Ícone da bandeja do Windows:** ainda sai de `pixels_do_icone()` em
  `bandeja.rs`, desenhado em código. Trocar pelo símbolo pede converter o PNG
  de 32 px em pixels no binário.
