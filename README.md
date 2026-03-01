# IsoBrowse - Dual-Core Security Vault & Eco-Friendly Reader 🛡️🌱

IsoBrowse is an experimental, dual-core web browser engineered from scratch using Rust and WebAssembly (Wasm). It is designed to bridge the gap between **hardcore Web3 security** and **ultra-fast, eco-friendly everyday browsing**. 

Whether you are navigating dangerous DApps or just trying to read the daily news without being bombarded by ads and trackers, IsoBrowse adapts to your needs with a single click.

## The Dual-Core Architecture

IsoBrowse operates on two distinct engines, giving users absolute control over their digital perimeter:

### 👻 Mod 1 (WASM_GHOST) - The Eco-Friendly Sterile Reader
Designed for everyday reading, news browsing, and researching unknown links.
* **Physical Shredding:** Instead of just hiding ads, Mod 1 physically strips JavaScript loops, trackers, and hidden `iframe`s from the HTML before the page even renders.
* **Zero-Execution Environment:** 100% safe for clicking suspicious links or reading news on ad-heavy websites without any risk of script execution.
* **Green Tech & CO2 Saver:** By killing heavy scripts and preventing megabytes of junk data from loading, IsoBrowse significantly reduces CPU/RAM usage and battery drain. It even calculates your saved CO2 emissions in real-time! 🌱

### 🛡️ Mod 2 (WEB3_VAULT) - The Unrestricted Safe
Powered by native OS web rendering (WebKit/WebView2), this mode allows full Web3 interaction and wallet connections.
* **Heuristic Fingerprinting:** Monitors live DOM spikes, memory leaks, and idle CPU anomalies.
* **Fail-Deadly Mechanism:** If a zero-day drainer, hidden cryptojacker, or phishing injection is detected, the system immediately alerts the user with a red perimeter and warns against wallet connection.

## Installation (macOS)
1. Download the latest `.dmg` or `.app` release from the project owner. *(Windows release coming soon!)*
2. Open and start browsing securely and cleanly.

## Development (Build from Source)
Ensure you have Rust and Cargo installed on your system.

```bash
git clone [https://github.com/igtumt/isobrowse.git](https://github.com/igtumt/isobrowse.git)
cd isobrowse
cargo run --release
