# IsoBrowse

A simple, local-first WebAssembly (WASM) pipeline runtime.

Not a browser.
Not a CLI.
Something in between.

IsoBrowse lets you process data securely on your own machine by chaining small, isolated WASM modules together — just like classic Unix pipelines.

---

## Why?

Developers constantly manipulate data (JSON, HTML, logs, CSVs).

Today, that usually means:

1. **Online formatters** → paste data into ad-heavy websites
2. **CLI tools (`jq`, `awk`, Python`)** → setup, dependencies, full system access

IsoBrowse tries a simpler approach:

* **Local & Private:** Everything runs locally in memory
* **Sandboxed:** Each tool runs inside a Wasmtime WASM sandbox
* **No setup:** If IsoBrowse runs, tools run
* **Pipeline-first:** `stdin → stdout`, composable

---

## How it works

You pipe data from one module to another.

### Basic example

```
/echo "hello world" | /run uppercase
HELLO WORLD
```

### Under the hood

```
/echo "hello world" | /run https://yoururl.com/uppercase.wasm
```

### Running scripts (Python in WASM)

```
/echo "print('hello')" | /run python
```

### Fetch + parse

```
/get news.ycombinator.com | /run htmlclean | /run linkextract | /run sort
```

### JSON

```
/get https://api.example.com | /run jq "name"
```

---

## 🧩 Modules

I’ve built ~80 small WASM tools so far (text, parsing, hashing, etc):

👉 https://github.com/igtumt/isomodules

They follow a simple idea:

> do one thing, and do it well

---

## 🚀 Build your own tools

IsoBrowse is built around small WASM modules.

You can run any tool from a URL:

```
/run https://your-tool.wasm
```

So you can also build your own and share them.

* No install
* No packaging
* No ecosystem barriers

Just compile to WASM and it works.

---

## ⚡ 5-minute WASM tool

Here is the simplest possible example:

### 1. Create a Rust project

```
cargo new mytool
cd mytool
```

### 2. Replace `main.rs`

```rust
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    print!("{}", input.to_uppercase());
}
```

### 3. Build to WASM

```
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release
```

Output:

```
target/wasm32-wasip1/release/mytool.wasm
```

### 4. Run it

```
/echo "hello" | /run ./mytool.wasm
```

That’s it.

---

## 🖥 Architecture

* Rust core
* Wasmtime sandbox
* WebView UI

Flow:

```
Input → Pipeline → WASM → Output
```

Everything runs in memory.

---

## ⚖️ Limitations

* Not a full browser (no SPA support yet)
* WASM ecosystem is still growing

---

## Getting Started

```
git clone https://github.com/igtumt/isobrowse
cd isobrowse
cargo run
```

---

## 🧭 Final Thought

I built IsoBrowse as a small experiment.

It turned into something I use daily.

Maybe it’s useful for you too.

> A safe place to run data.

---

## License

MIT + Apache 2.0
