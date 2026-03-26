# IsoBrowse

IsoBrowse is a lightweight, local-first WebAssembly (WASM) pipeline environment. It is designed for developers who need to process data quickly and securely, combining the composability of classic Unix pipelines with the modern sandboxing of WASM.

Everything runs entirely on your machine. No cloud, no trace.

---

## 💡 The Problem It Solves

When manipulating data (JSON, HTML, logs, CSVs), we usually rely on traditional CLI tools (`jq`, `awk`, `sed`, `grep`, Python scripts, etc.). However, these tools:
* Require environment setup and dependency management.
* Give tools full access to your host OS and file system.
* Can be hard to compose uniformly across different operating systems.

**IsoBrowse** approaches this differently:
* **Sandboxed by Default:** Every tool runs inside an isolated WASM container.
* **Zero Dependencies:** If you have IsoBrowse, you have the tools. 
* **Pipeline-First:** Everything streams via `stdin` to `stdout`.

---

## ⚙️ Core Concepts

### 1. Pipeline Execution
Chain commands together just like you would in a Unix terminal. Each step receives input from the previous one, processes it, and pipes it forward.

```bash
> /echo "Hello World" | /run lowercase | /run slugify
```

### 2. WASM Sandbox
There is no direct OS access. Modules cannot read your arbitrary files or make hidden network calls. They only know what you feed them through the pipeline.

---

## 🧪 Example Workflows

IsoBrowse thrives on composition. Here are a few examples of what you can do without leaving the sandbox:

**Text Processing:**
```bash
> /echo "HELLO WORLD" | /run lowercase
```

**Fetch & Parse:**
```bash
> /get example.com | /run html2text
```

**JSON Extraction:**
```bash
> /get api.example.com/users | /run jq "name"
```

**Advanced Pipeline (Web Scraping):**
```bash
> /get news.ycombinator.com | /run htmlclean | /run linkextract | /run sort | /run uniq
```

---

## 🧩 The Modules

IsoBrowse relies on standalone, single-purpose WASM modules. We maintain a growing standard library of tools (text formatting, hashing, scraping, data parsing) here:

👉 **[igtumt/isomodules](https://github.com/igtumt/isomodules)**

Each module is written in Rust, compiled to WASI, and strictly follows the "do one thing well" philosophy.

---

## 🖥 Architecture

IsoBrowse is a hybrid system built for speed and security:

* **Backend:** Rust (Handles the runtime and WASM engine execution).
* **UI:** WebView (A lightweight, terminal-like interface).
* **Execution:** WASM Workers (Where the actual data processing happens).

**The Flow:**
`User Input` → `Pipeline Parser` → `WASM Execution` → `Render Output`

---

## ⚖️ Trade-offs & Limitations

We believe in being honest about what this tool is *not*:
* **It is not a web browser.** It's a programmable terminal and data pipeline engine.
* **Slight Overhead:** While WASM is fast, it's still running in a sandbox, which carries a small overhead compared to raw native binaries.
* **Ecosystem:** Not all of your favorite CLI tools exist as WASM modules yet.
* **Memory:** Chaining very large pipelines might increase memory usage depending on the data size.

---

## 🔮 Future Ideas

This is an evolving experiment. Some things we are exploring:
* A WASM module registry to easily add third-party tools (like `/run sqlite` or `/run python`).
* Local-first AI integration for text analysis within the pipeline.
* A broader community ecosystem for custom plugins.

---

## 🚀 Getting Started

To try IsoBrowse locally on your machine:

```bash
git clone https://github.com/igtumt/isobrowse
cd isobrowse
cargo run
```

---

## 🤝 Contributing

Contributions, feedback, and new WASM module ideas are always welcome. Whether it's a performance tweak, a UI enhancement, or a new tool for `isomodules`, feel free to open an issue or a pull request.

---

## 📄 License

This project is dual-licensed under the MIT and Apache 2.0 licenses, aligning with the standard Rust ecosystem.

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
