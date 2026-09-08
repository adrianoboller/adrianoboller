<?php
require_once __DIR__ . '/config.php';
// PHP 8.1+ passou a lancar excecao no mysqli; este codigo e de 2011 e trata o retorno.
mysqli_report(MYSQLI_REPORT_OFF);
function conectar() {
    static $cx = null;
    if ($cx === null) {
        $cx = mysqli_connect(DB_HOST, DB_USER, DB_PASS, DB_NAME);
        if (!$cx) { die('Sem banco: ' . mysqli_connect_error()); }
        mysqli_set_charset($cx, 'utf8mb4');
    }
    return $cx;
}
