#!/bin/bash
MODE=${1:-mod1} 

# 1. Dynamic WASI SDK Path Configuration
# Set WASI_SDK_PATH dynamically if it's not already set by the user
if [ -z "$WASI_SDK_PATH" ]; then
    export WASI_SDK_PATH="$HOME/wasi-sdk-20.0"
fi

# 2. LLVM and Compiler Settings (macOS Homebrew Default)
LLVM_PATH="/opt/homebrew/opt/llvm/bin"
export CC_wasm32_wasip1="$LLVM_PATH/clang"
export AR_wasm32_wasip1="$LLVM_PATH/llvm-ar"
export ZSTD_NO_ASM=1
export CFLAGS_wasm32_wasip1="--target=wasm32-wasi --sysroot=$WASI_SDK_PATH/share/wasi-sysroot"

echo -e "\033[0;34m> [1/4] Building Surf Runtime (Mod 1)... \033[0m"
cargo +nightly build --target wasm32-wasip1 -p runtime_surf --release
if [ $? -ne 0 ]; then echo -e "\033[0;31m[!] FATAL: Failed to compile Surf Runtime!\033[0m"; exit 1; fi

echo -e "\033[0;34m> [2/4] Building Standard Runtime (Mod 2)... \033[0m"
cargo +nightly build --target wasm32-wasip1 -p runtime_standard --release
if [ $? -ne 0 ]; then echo -e "\033[0;31m[!] FATAL: Failed to compile Standard Runtime!\033[0m"; exit 1; fi

echo -e "\033[0;34m> [3/4] Building Host Engine (Interface)... \033[0m"
cargo +nightly build -p isobrowse_host
if [ $? -ne 0 ]; then echo -e "\033[0;31m[!] FATAL: Failed to compile Host Engine!\033[0m"; exit 1; fi

echo -e "\033[0;32m> [4/4] Initializing IsoBrowse Pipeline... \033[0m"
cargo +nightly run -p isobrowse_host
