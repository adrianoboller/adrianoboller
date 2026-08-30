# Wire the submit path
# 28/08 18:55

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''    if (r.particionada) {
      const porPeriodo = r.particao && r.particao !== "quantidade";
      if (porPeriodo && !r.particao_coluna)
        return avisar("a partição por período precisa de uma coluna de data obrigatória", true);
      if (!porPeriodo && !r.pag)
        return avisar("informe quantos registros cabem em cada arquivo", true);
      pedido.registros_por_arquivo = r.pag || 1000000;
      pedido.digitos = r.dig;
      pedido.max_arquivos = r.max;   // zero = o que couber no sufixo
      if (porPeriodo) {
        pedido.particao = r.particao;
        pedido.particao_coluna = r.particao_coluna;
      }
    }'''
novo='''    if (r.particionada) {
      const porLetra = r.particao === "letra";
      const porPeriodo = r.particao && r.particao !== "quantidade" && !porLetra;
      if (porLetra && !r.particao_coluna)
        return avisar("a partição alfanumérica precisa de uma coluna de referência obrigatória", true);
      if (porPeriodo && !r.particao_coluna)
        return avisar("a partição por período precisa de uma coluna de data obrigatória", true);
      if (!porPeriodo && !porLetra && !r.pag)
        return avisar("informe quantos registros cabem em cada arquivo", true);
      pedido.registros_por_arquivo = r.pag || 1000000;
      if (porLetra) {
        // Nem `digitos` nem `max_arquivos` vão: os 37 baldes e o sufixo de
        // letra são o formato desta partição, e não uma escolha da tela.
        pedido.particao = "letra";
        pedido.particao_coluna = r.particao_coluna;
      } else {
        pedido.digitos = r.dig;
        pedido.max_arquivos = r.max;   // zero = o que couber no sufixo
        if (porPeriodo) {
          pedido.particao = r.particao;
          pedido.particao_coluna = r.particao_coluna;
        }
      }
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
