# Compose the page with the grid and add the export flag
# 27/08 21:51

p='crates/phxsql-server/src/http.rs'
s=open(p).read()
s=s.replace('''pub const PAGINA: &str = include_str!("../ui/index.html");''',
'''pub const PAGINA: &str = include_str!("../ui/index.html");

/// O phx-grid, do ecossistema Phoenix: ES5 estrito, zero dependencia,
/// arquivo unico. Entra no cabecalho para estar definido antes de a pagina
/// rodar o proprio script. Fonte e historico em `ui/grid/`.
const GRID_CSS: &str = include_str!("../ui/grid/phx-grid.css");
const GRID_JS: &str = include_str!("../ui/grid/phx-grid.js");''')
s=s.replace('''pub fn montar_pagina() -> String {
    format!(
        "<!doctype html>\\n<html lang=\\"pt-BR\\">\\n<head>\\n<meta charset=\\"utf-8\\">\\n</head>\\n<body>\\n{PAGINA}\\n</body>\\n</html>\\n"
    )
}''','''pub fn montar_pagina() -> String {
    format!(
        "<!doctype html>\\n<html lang=\\"pt-BR\\">\\n<head>\\n<meta charset=\\"utf-8\\">\\n\\
         <style>\\n{GRID_CSS}\\n</style>\\n<script>\\n{GRID_JS}\\n</script>\\n\\
         </head>\\n<body>\\n{PAGINA}\\n</body>\\n</html>\\n"
    )
}''')
open(p,'w').write(s)
