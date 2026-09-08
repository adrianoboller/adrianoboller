<?php
// Configuracao do legado: lida do ambiente, nunca gravada aqui.
define('DB_HOST', getenv('LOJA_DB_HOST') ?: 'localhost');
define('DB_NAME', getenv('LOJA_DB_NAME') ?: 'loja');
define('DB_USER', getenv('LOJA_DB_USER') ?: 'loja');
define('DB_PASS', getenv('LOJA_DB_PASS') ?: '');
