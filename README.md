<img width="1512" height="964" alt="isobrowse_new_frontpage" src="https://github.com/user-attachments/assets/66b1b670-df9a-4571-9e71-89e7264ee1f9" />


https://github.com/user-attachments/assets/2a91ad9c-3496-4ae5-b008-4a8ef73a3939


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

## ⚖️ Limitations

* **Text & Data Only:** Works with text, JSON, HTML, and basic math. No 3D, graphics, or heavy UI.  
* **Not a full browser:** No SPAs. Just fetch & process raw data.  
* **Sandboxed:** Modules cannot touch local files or make network calls—only what you pipe in.  
* **Experimental:** Built as a personal experiment. ~80 modules exist, but edge cases may break.

## 🤝 Community & Future

IsoBrowse started as my personal tool for safe, local data pipelines.  
It works for me daily, and I hope the community can help improve it.  
Contributions, feedback, and PRs are welcome!

---

## License

MIT + Apache 2.0
