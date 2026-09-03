"""Baixa as faces do Google Fonts e as EMBUTE como data URI numa copia do HTML.

Existe porque o PDF sai pelo Chromium sem rede confiavel: o pedido a
fonts.googleapis.com falhou na primeira corrida e o PDF nasceu com fonte de
fallback -- e a marca MANDA. `document.fonts.check()` respondeu `true` mesmo
assim, porque ele aceita o substituto: quem quer saber se a fonte chegou
precisa contar `document.fonts.size`, e nao perguntar ao `check`.
"""
import base64, pathlib, re, subprocess, sys, urllib.parse

CSS = ("https://fonts.googleapis.com/css2?"
       "family=Exo+2:wght@400;500;600;700"
       "&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600"
       "&family=IBM+Plex+Mono:wght@400;500;600&display=swap")
# Sem User-Agent de navegador o Google devolve TTF; com ele devolve woff2, que
# e ~4x menor. Num documento com 20 capturas embutidas isso importa.
UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
      "Chrome/120.0 Safari/537.36")


def buscar(url, binario=False):
    r = subprocess.run(["curl", "-sSL", "-A", UA, url], capture_output=True)
    if r.returncode != 0 or not r.stdout:
        raise RuntimeError(f"nao baixou {url[:60]}: {r.stderr.decode()[:120]}")
    return r.stdout if binario else r.stdout.decode()


def main(entrada, saida):
    css = buscar(CSS)
    urls = sorted(set(re.findall(r"url\((https://fonts\.gstatic\.com/[^)]+)\)", css)))
    print(f"faces apontadas pela CSS: {len(urls)}")
    baixadas = 0
    for u in urls:
        try:
            dados = buscar(u, binario=True)
        except RuntimeError as e:
            print(f"  FALTOU {u[-40:]}"); continue
        tipo = "font/woff2" if u.endswith(".woff2") else "font/ttf"
        css = css.replace(u, f"data:{tipo};base64,{base64.b64encode(dados).decode()}")
        baixadas += 1
    print(f"faces embutidas: {baixadas}/{len(urls)}")
    if baixadas < len(urls):
        print("REPROVA: face faltando deixaria o PDF em fallback calado")
        return 1

    html = pathlib.Path(entrada).read_text()
    # Troca o <link> pela folha ja com as faces dentro.
    alvo = re.search(r'<link rel="stylesheet" href="https://fonts\.googleapis\.com[^>]*>', html)
    if not alvo:
        print("REPROVA: nao achei o <link> das fontes no HTML"); return 1
    html = html.replace(alvo.group(0), f"<style>\n{css}\n</style>")
    pathlib.Path(saida).write_text(html)
    print(f"copia com as fontes dentro: {saida} "
          f"({pathlib.Path(saida).stat().st_size/1048576:.2f} MiB)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
