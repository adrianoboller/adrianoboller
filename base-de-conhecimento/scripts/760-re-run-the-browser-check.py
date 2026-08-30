# Re-run the browser check
# 28/08 19:54

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/salto/ui.mjs")
s = p.read_text()
s = s.replace("""  await p.waitForFunction(n => +document.querySelector('#pgIr').value === n, a, {timeout:5000});
  await p.waitForTimeout(60);""",
"""  await p.waitForTimeout(80);
  await p.waitForSelector('#pgIr');
  await p.waitForFunction(n => { const e = document.querySelector('#pgIr'); return e && +e.value === n; },
                          a, {timeout:5000});""")
p.write_text(s)
