// O tsc nao copia o icone SVG do no; o n8n o procura ao lado do .node.js em dist/.
const fs = require("fs");
const path = require("path");
const de = path.join(__dirname, "nodes", "PhxClaw", "phxclaw.svg");
const para = path.join(__dirname, "dist", "nodes", "PhxClaw", "phxclaw.svg");
fs.mkdirSync(path.dirname(para), { recursive: true });
fs.copyFileSync(de, para);
console.log("icone copiado para", path.relative(__dirname, para));
