# Fix challenge restore, UI health probe, run tests
# 27/08 19:45

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
velho = '''    const r = await fetch("/api", { method:"POST",
      headers:{"Content-Type":"application/json"},
      body: JSON.stringify({op:"ping"}) });
    await r.json();
    est.demo = false;'''
novo = '''    // /saude nao pede token e nao conta tentativa: e so o sinal de vida.
    const r = await fetch("/saude");
    if (!r.ok) throw new Error("sem servidor");
    await r.json();
    est.demo = false;'''
assert s.count(velho)==1
s = s.replace(velho,novo)
open(p,'w').write(s)
print("ui ok")
