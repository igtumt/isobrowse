#!/bin/bash
MODE=${1:-mod1} 

# 1. KRİTİK: WASI SDK yolunu buraya yaz
export WASI_SDK_PATH="/Users/isilaygamze/wasi-sdk-20.0" 

# 2. LLVM ve Derleyici Ayarları
LLVM_PATH="/opt/homebrew/opt/llvm/bin"
export CC_wasm32_wasip1="$LLVM_PATH/clang"
export AR_wasm32_wasip1="$LLVM_PATH/llvm-ar"
export ZSTD_NO_ASM=1
export CFLAGS_wasm32_wasip1="--target=wasm32-wasi --sysroot=$WASI_SDK_PATH/share/wasi-sysroot"

echo -e "\033[0;34m> [1/4] Ghost Engine (Mod 1) Derleniyor... \033[0m"
cargo +nightly build --target wasm32-wasip1 -p engine_ghost
if [ $? -ne 0 ]; then echo -e "\033[0;31mHATA: Ghost Engine derlenemedi!\033[0m"; exit 1; fi

echo -e "\033[0;34m> [2/4] Standard Engine (Mod 2) Derleniyor... \033[0m"
cargo +nightly build --target wasm32-wasip1 -p engine_standard
if [ $? -ne 0 ]; then echo -e "\033[0;31mHATA: Standard Engine derlenemedi!\033[0m"; exit 1; fi

echo -e "\033[0;34m> [3/4] Host Uygulaması (Arayüz) Derleniyor... \033[0m"
cargo +nightly build -p isobrowse_host
if [ $? -ne 0 ]; then echo -e "\033[0;31mHATA: Arayüz derlenemedi!\033[0m"; exit 1; fi

echo -e "\033[0;32m> [4/4] IsoBrowse Başlatılıyor... \033[0m"
cargo +nightly run -p isobrowse_host
