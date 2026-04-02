# IsoBrowse

A simple, local-first WebAssembly (WASM) pipeline runtime.

Not a browser.  
Not a CLI.  
Something in between.

![IsoBrowse UI](https://github.com/user-attachments/assets/620a2d84-8507-437e-b03d-007396520258)

> Everything you see runs locally inside a sandboxed WASM environment. IsoBrowse lets you process data securely on your own machine by chaining small, isolated WASM modules — similar to Unix pipelines.

---

## ⚡ Quick Examples

```bash
/echo "hello world" | /run uppercase
HELLO WORLD
```

```bash
/get news.ycombinator.com | /run htmlclean | /run linkextract | /run sort
```

```bash
/get https://api.example.com | /run jq "name"
```

```bash
/echo "print('hello')" | /run python
```

---

## 🧠 How it works

Everything is a simple flow:
`data → pipeline → wasm → output`

* `/read`, `/get`, `/echo` → provide data
* `/run` → executes a WASM tool
* `|` → connects everything

<img width="1512" height="972" alt="codex" src="[https://github.com/user-attachments/assets/0c0e3586-1bac-41c8-b454-0e8194173293](https://github.com/user-attachments/assets/0c0e3586-1bac-41c8-b454-0e8194173293)" />

---

## 💡 Why?

Working with data usually means:
* Copy/paste into ad-heavy websites
* Installing CLI tools and managing dependencies

IsoBrowse takes a different approach:
* **Local-first:** your data never leaves your machine
* **Sandboxed:** tools run in isolated WASM containers
* **No setup:** if IsoBrowse runs, tools run
* **Composable:** everything works through pipelines

---

## 🧩 Modules (WASM TOOLS)

I’ve built ~80 small WASM tools (text, parsing, hashing, etc):
👉 [https://github.com/igtumt/isomodules](https://github.com/igtumt/isomodules)

They follow a simple idea: 
do one thing, and do it well.

---

## 🚀 Build your own tools

You can run any WASM tool directly:

```bash
/run https://your-tool.wasm
```

No install. 
No packaging. 
Just compile to WASM and run.

---

## ⚡ 5-minute WASM tool

### 1. Create a project
```bash
cargo new mytool
cd mytool
```

### 2. Replace main.rs
```rust
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    print!("{}", input.to_uppercase());
}
```

### 3. Build
```bash
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release
```

### 4. Run
```bash
/echo "hello" | /run ./mytool.wasm
```

---

## 🎬 Demo

[https://github.com/user-attachments/assets/2a91ad9c-3496-4ae5-b008-4a8ef73a3939](https://github.com/user-attachments/assets/2a91ad9c-3496-4ae5-b008-4a8ef73a3939)

---

## 🖥 Architecture

* Rust core
* Wasmtime sandbox
* WebView UI

Everything runs in memory.

---

## 🚀 Getting Started

### 📥 macOS (MVP)
1. Download from Releases: `IsoBrowse-v1.0-Mac.zip`
2. Extract the file
3. Remove macOS quarantine:
```bash
xattr -cr /path/to/IsoBrowse.app
```
4. Open `IsoBrowse.app`

### ⚙️ From source
```bash
git clone https://github.com/igtumt/isobrowse.git
cd isobrowse
sh run.sh
```

---

## ⚖️ Limitations

* Text & data focused (JSON, HTML, logs)
* Not a full browser (no SPA support)
* Strict sandbox (no direct file/network access)
* Experimental — some edge cases may break

---

## 🧭 Final Thought

I built IsoBrowse as a small experiment.

It turned into something I use daily. 

Maybe it’s useful for you too. 

A safe place to run data.

---

## License

MIT + Apache 2.0
