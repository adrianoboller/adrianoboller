<?php
// Copiar para cada filial e ajustar. A senha do banco fica na variavel de
// ambiente FATURAMENTO_DB_PASS desde 2021 - nao escrever senha aqui.
define('DB_HOST', getenv('FATURAMENTO_DB_HOST') ?: 'localhost');
define('DB_NAME', getenv('FATURAMENTO_DB_NAME') ?: 'faturamento');
define('DB_USER', getenv('FATURAMENTO_DB_USER') ?: 'faturamento_app');
define('DB_PASS', getenv('FATURAMENTO_DB_PASS') ?: '');
define('EMPRESA', 'Boller Sistemas Ltda');
