@echo off
set PORT=8080
if not "%1"=="" set PORT=%1
echo PhxMindSetSQL: http://localhost:%PORT%
py -m http.server %PORT%
pause
