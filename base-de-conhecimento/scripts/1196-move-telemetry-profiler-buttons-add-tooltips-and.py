# Move telemetry/profiler buttons, add tooltips and menu entries
# 29/08 19:05

import io,sys
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
orig=s

# 1. tira as duas do grupo 3
velho = '''  { ico:"pulso",    rot:"Profiler",   cor:"var(--acao-consultar)", faz:verProfiler },
  { ico:"medidor",  rot:"Telemetria", cor:"var(--memo)",   faz:telaTelemetria },
  { ico:"escudo",   rot:"Diretivas",  cor:"var(--laranja)",faz:verDiretivas },
'''
novo = '''  { ico:"escudo",   rot:"Diretivas",  cor:"var(--laranja)",faz:verDiretivas },
'''
assert s.count(velho)==1, s.count(velho)
s=s.replace(velho,novo)

# 2. poe as duas no grupo 2, ao lado de Conexoes
velho2 = '''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verSessoes },
  { ico:"engrenagem", rot:"Config",   cor:"var(--laranja)",faz:verConfigServidor },
'''
novo2 = '''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verSessoes },
  // Vizinhas de "Conexoes" de proposito: as tres respondem a mesma pergunta --
  // o que esta acontecendo AGORA. Estavam no grupo do ocasional, entre coisas
  // que se fazem uma vez por mes, e ninguem as achava: o pedido chegou como
  // "falta o botao do SQL Check" com o botao ja no ar ha semanas. Lugar errado
  // na barra e o mesmo que nao existir.
  { ico:"medidor",  rot:"Telemetria", cor:"var(--memo)",   faz:telaTelemetria,
    dica:"gráficos bolha ordenados por peso, no molde do SQL Check da Idera®" },
  { ico:"pulso",    rot:"Profiler",   cor:"var(--acao-consultar)", faz:verProfiler,
    dica:"o que chega pela porta de dados, antes de virar dado" },
  { ico:"engrenagem", rot:"Config",   cor:"var(--laranja)",faz:verConfigServidor },
'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# 3. o comentario da ordem tem de continuar verdadeiro
s=s.replace("     2. administracao corrente ... bancos, gente, conexoes, configuracao",
            "     2. administracao corrente ... gente, conexoes, o que roda AGORA\n"
            "                                   (telemetria e profiler), configuracao")
s=s.replace("     3. o ocasional .............. salvaguarda, cadastro fino, observacao",
            "     3. o ocasional .............. salvaguarda, cadastro fino, o registro")

# 4. a dica vai para o title do botao
velho4 = '''      title="${esc(rotulo)}${pendente ? " · ainda não existe" : ""}">'''
novo4  = '''      title="${esc(rotulo)}${f.dica ? " — " + esc(f.dica) : ""}${
        pendente ? " · ainda não existe" : ""}">'''
assert s.count(velho4)==1
s=s.replace(velho4,novo4)

# 5. o subtitulo da tela cita a referencia -- o nome que o Adriano procura
velho5 = '''  folha("Telemetria",
        "o que o servidor está fazendo agora — séries, atividades e threads",'''
novo5 = '''  folha("Telemetria",
        "o que o servidor está fazendo agora — séries, atividades e threads, "
        + "em gráficos bolha no molde do SQL Check da Idera®",'''
assert s.count(velho5)==1
s=s.replace(velho5,novo5)

# 6. o menu Ferramentas promete ser "a mesma lista pelo teclado" e nao tinha as duas
velho6 = '''    { rot:"Replicação",           ico:"⇉", faz:verReplicacao },
    { rot:"Reparar…",             ico:"⛨", faz:repararPeloMenu },
'''
novo6 = '''    { rot:"Replicação",           ico:"⇉", faz:verReplicacao },
    "sep",
    { rot:"Telemetria ao vivo…",  ico:"◔", faz:telaTelemetria },
    { rot:"Profiler…",            ico:"∿", faz:verProfiler },
    "sep",
    { rot:"Reparar…",             ico:"⛨", faz:repararPeloMenu },
'''
assert s.count(velho6)==1
s=s.replace(velho6,novo6)

io.open(p,"w",encoding="utf-8").write(s)
print("ok", len(orig), "->", len(s))
