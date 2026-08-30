# Resolve http.rs conflicts additively
# 29/08 19:11

import io
p="phxsql/crates/phxsql-server/src/http.rs"
s=io.open(p,encoding="utf-8").read()

a = """<<<<<<< HEAD
/// A tela de telemetria, em arquivo proprio -- estilo e desenho."""
assert a in s
s = s.replace("""<<<<<<< HEAD
/// A tela de telemetria""", "/// A tela de telemetria")
s = s.replace("""const TELEMETRIA_JS: &str = include_str!("../ui/telemetria.js");
=======
/// A integracao com a Claude""", """const TELEMETRIA_JS: &str = include_str!("../ui/telemetria.js");

/// A integracao com a Claude""")
s = s.replace("""pub const ORIGEM_ANTHROPIC: &str = "https://api.anthropic.com";
>>>>>>> worktree-agent-a62d10a150809c2a5""", """pub const ORIGEM_ANTHROPIC: &str = "https://api.anthropic.com";""")

s = s.replace("""<<<<<<< HEAD
         <style>\\n{TELEMETRIA_CSS}\\n</style>\\n\\
         <script>\\n{TELEMETRIA_JS}\\n</script>\\n\\
=======
         <script>\\n{CLAUDE_JS}\\n</script>\\n\\
>>>>>>> worktree-agent-a62d10a150809c2a5""", """         <style>\\n{TELEMETRIA_CSS}\\n</style>\\n\\
         <script>\\n{TELEMETRIA_JS}\\n</script>\\n\\
         <script>\\n{CLAUDE_JS}\\n</script>\\n\\""")
assert "<<<<<<<" not in s and ">>>>>>>" not in s and "\n=======\n" not in s, [l for l in s.split("\n") if l.startswith(("<<<","===",">>>"))]
io.open(p,"w",encoding="utf-8").write(s)
print("resolvido")
