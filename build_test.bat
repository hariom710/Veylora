@echo off
set PATH=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64;%PATH%
set LIB=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\lib\onecore\x64;%LIB%
set INCLUDE=C:\Program Files\Microsoft Visual Studio\18\Community\SDK\ScopeCppSDK\vc15\VC\include;%INCLUDE%
cd D:\projets\Veylora\src-tauri
cargo test --test integration -- --list 2>&1 | findstr "test " | head -20