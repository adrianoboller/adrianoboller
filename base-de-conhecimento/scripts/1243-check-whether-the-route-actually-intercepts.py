# Check whether the route actually intercepts
# 30/08 04:21

import io
p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/prova-atropelo.mjs"
s=io.open(p,encoding="utf-8").read()
s=s.replace("await p.route('**/api', async (rota) => {",
            "let segurados = 0;\nawait p.route('**/api', async (rota) => {")
s=s.replace("    await new Promise(r => setTimeout(r, 2500));",
            "    segurados++;\n    await new Promise(r => setTimeout(r, 2500));")
s=s.replace("console.log(r);","console.log('pedidos segurados:', segurados);\nconsole.log(r);")
io.open(p,"w",encoding="utf-8").write(s)
