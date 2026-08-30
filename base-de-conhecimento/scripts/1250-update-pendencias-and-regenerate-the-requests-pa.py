# Update PENDENCIAS and regenerate the requests page
# 30/08 05:25

p='docs/PENDENCIAS.md'
s=open(p,encoding='utf-8').read()
velho="conta **258 textos na fábrica e 1.994 ainda cravados em português**, com arquivo e linha de cada um."
novo=("conta **2.068 ainda cravados em português**, com arquivo e linha de cada um — e esse "
      "2.068 é a **primeira medida sobre a interface inteira**. As anteriores (1.994, depois 1.999) "
      "eram medidas sobre cinco sextos dela: o `FONTES` do conferidor é uma lista digitada, o "
      "`multitela.js` entrou no `http.rs` sem entrar nela, e **69 textos cravados em 1.474 linhas de "
      "interface servida nunca foram contados**. É a lição do KiB da interface de novo — *quando um "
      "gerador depende de uma lista, a lista tem de sair do código* — e agora a guarda "
      "`a_lista_cobre_tudo_que_o_http_serve` lê o fonte do `http.rs` e reprova o arquivo servido que "
      "ninguém mede. Dos 69, cerca de 30 traduzem-se limpo; os outros **39 não traduzem de jeito "
      "nenhum como estão**, porque são uma frase picada pela marcação: `\"funcionam em\"` + "
      "`<b>qualquer navegador</b>` + `\"— é layout. Destacar em janela também, com\"`. Fragmento não "
      "se traduz — a ordem das palavras muda de idioma para idioma —, então a frase inteira tem de "
      "virar uma chave só, com o destaque como marcador dentro dela.")
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("PENDENCIAS item 10 atualizado com o numero medido e o achado da frase picada")
