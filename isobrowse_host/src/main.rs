use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use std::thread;
use std::time::{Instant, Duration};
use std::sync::Arc;
use sysinfo::System;

enum UserEvent {
    WasmSurfRender { html: String, url: String, cpu_ms: u128, ram_kb: usize, blocked_count: usize },
    IpcMessage(String),
    UpdateTerminal(String),
    UpdateOsTelemetry { cpu: f32, ram_mb: u64 }, 
}

static WASM_ENGINE_GHOST: &[u8] = include_bytes!("../../target/wasm32-wasip1/release/runtime_surf.wasm");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    
    let telemetry_proxy = proxy.clone();
    thread::spawn(move || {
        let mut sys = System::new_all();
        if let Ok(pid) = sysinfo::get_current_pid() {
            loop {
                sys.refresh_processes();
                if let Some(process) = sys.process(pid) {
                    let cpu = process.cpu_usage();
                    let ram_mb = process.memory() / (1024 * 1024);
                    let _ = telemetry_proxy.send_event(UserEvent::UpdateOsTelemetry { cpu, ram_mb });
                }
                thread::sleep(Duration::from_secs(1));
            }
        }
    });
    
    let http_client = Arc::new(
        reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15")
            .cookie_store(true)
            .timeout(Duration::from_secs(10)) 
            .build()
            .unwrap()
    );

    let window = WindowBuilder::new()
        .with_title("IsoBrowse WebAssembly Pipeline Runtime")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 950.0))
        .build(&event_loop)?;

    let init_script = r##"
    

        window.isoCatalogData = [];

        window.injectCmd = function(modName) {
            let host = document.getElementById('isobrowse-shadow-host');
            if (host && host.shadowRoot) {
                let termInput = host.shadowRoot.getElementById('iso-url');
                if (termInput) {
                    let currentVal = termInput.value.trim();
                    if (currentVal !== '' && !currentVal.endsWith('|')) {
                        termInput.value = currentVal + ' | /run ' + modName + ' ';
                    } else {
                        termInput.value = currentVal + ' /run ' + modName + ' ';
                    }
                    setTimeout(() => { termInput.focus(); }, 50);
                }
            }
        };

        window.renderCatalogCards = function(data) {
            const container = document.getElementById('modules-container');
            if(!container) return;
            container.innerHTML = '';
            if(data.length === 0) {
                container.innerHTML = '<div style="color:#ff3366; text-align:center; width:100%; grid-column: 1 / -1;"><br>No modules found matching your search.</div>';
                return;
            }
            data.forEach(mod => {
                const card = document.createElement('div');
                card.style.cssText = 'background: #111; border: 1px solid #333; padding: 15px; border-radius: 8px; display: flex; flex-direction: column; justify-content: space-between;';
                card.innerHTML = `
                    <div>
                        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:10px;">
                            <h3 style="color:#fff; margin:0; font-size:20px;">${mod.name}</h3>
                            <span style="background:#333; color:#aaa; padding:3px 8px; font-size:11px; border-radius:4px; font-weight:bold; letter-spacing:1px;">${mod.category}</span>
                        </div>
                        <p style="color:#888; font-size:14px; min-height:40px; line-height: 1.5;">${mod.desc}</p>
                    </div>
                    <button onclick="window.injectCmd('${mod.name}')" onmouseover="this.style.background='#00ff41'; this.style.color='#000';" onmouseout="this.style.background='#003300'; this.style.color='#00ff41';" style="width: 100%; background: #003300; color: #00ff41; border: 1px solid #00ff41; padding: 8px; cursor: pointer; font-family: monospace; font-weight: bold; border-radius: 4px; transition: 0.2s; margin-top: 15px;">[ INJECT /RUN ${mod.name.toUpperCase()} ]</button>
                `;
                container.appendChild(card);
            });
        };

        window.filterCatalog = function(term) {
            term = term.toLowerCase().trim();
            if(term === '') { window.renderCatalogCards(window.isoCatalogData); return; }
            const filtered = window.isoCatalogData.filter(m =>
                (m.name && m.name.toLowerCase().includes(term)) ||
                (m.tags && m.tags.toLowerCase().includes(term)) ||
                (m.category && m.category.toLowerCase().includes(term))
            );
            window.renderCatalogCards(filtered);
        };

        window.loadCatalogData = function() {
            const fallbackData = [
                { name: 'grep', category: 'TEXT', desc: 'Filters standard input based on a given regex pattern.', tags: 'text unix search filter regex' },
                { name: 'lowercase', category: 'TEXT', desc: 'Converts all standard input text to lowercase letters.', tags: 'text lower case format' },
                { name: 'base64', category: 'CRYPTO', desc: 'Encodes or decodes standard input data using Base64.', tags: 'crypto encode decode security' },
                { name: 'md2html', category: 'WEB', desc: 'Converts Markdown text into clean HTML code instantly.', tags: 'web markdown render' }
            ];

            fetch('https://raw.githubusercontent.com/igtumt/isomodules/main/catalog.json?v=' + new Date().getTime())
                .then(r => r.ok ? r.json() : Promise.reject('HTTP Error ' + r.status))
                .then(data => {
                    window.isoCatalogData = data;
                    window.renderCatalogCards(data);
                })
                .catch(e => {
                    console.log('GitHub fetch failed, using fallback data.', e);
                    window.isoCatalogData = fallbackData;
                    window.renderCatalogCards(fallbackData);
                });
        };

        window.addEventListener('keydown', function(e) {
            try {
                let host = document.getElementById('isobrowse-shadow-host');
                let shadowRoot = host ? host.shadowRoot : null;
                let activeEl = shadowRoot ? shadowRoot.activeElement : null;
                if (!activeEl) activeEl = document.activeElement;
                
                let isInput = (activeEl && (activeEl.tagName === 'TEXTAREA' || activeEl.tagName === 'INPUT'));

                // 🛡️ MACOS ÇÖKME ENGELLEYİCİ 1: Sınır Dışı Ok Tuşları
                if (isInput && (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
                    if (e.key === 'ArrowLeft' && activeEl.selectionStart === 0) { e.preventDefault(); return; }
                    if (e.key === 'ArrowRight' && activeEl.selectionStart === activeEl.value.length) { e.preventDefault(); return; }
                }

                // 🛡️ MACOS ÇÖKME ENGELLEYİCİ 2: Boşta Basılan Harfler
                if (!isInput) {
                    // Sayfa kaydırmak için Boşluk (Space) ve Ok tuşlarına izin ver, gerisini YUT!
                    if (e.key === 'Backspace' || (e.key.length === 1 && e.key !== ' ' && !e.metaKey && !e.ctrlKey)) {
                        e.preventDefault(); // macOS bu tuşu hiç görmeyecek!
                        
                        // Terminal görünürse harfi zorla komut satırına yazdır (Global Typing)
                        let isTerminalVisible = shadowRoot && shadowRoot.getElementById('terminal-input-line') && window.getComputedStyle(shadowRoot.getElementById('terminal-input-line')).display !== 'none';
                        if (isTerminalVisible && e.key.length === 1 && !window.isoIsRunning) {
                            let term = shadowRoot.getElementById('iso-url');
                            let spot = document.getElementById('spotlight-input');
                            if (spot && window.getComputedStyle(document.getElementById('iso-spotlight-home')).display !== 'none') {
                                spot.focus(); spot.value += e.key;
                            } else if (term) {
                                term.focus(); term.value += e.key;
                            }
                        }
                        return;
                    }
                }

                // 🛡️ KLASİK KOPYALA / YAPIŞTIR (Sorunsuz Versiyon)
                // 🛡️ KLASİK KOPYALA / YAPIŞTIR (macOS Crash Korumalı Kesin Sürüm)
                if (e.metaKey || e.ctrlKey) {
                    let k = e.key.toLowerCase();
                    
                    // İşletim sisteminin kısayolları çökertmesini KESİNLİKLE engelle!
                    if (k === 'c' || k === 'v' || k === 'x' || k === 'a') {
                        e.preventDefault(); // İŞTE HAYAT KURTARAN SATIR! (macOS bu tuşları hiç görmeyecek)
                        
                        if (k === 'c') {
                            if (window.isoIsRunning) {
                                window.isoIsRunning = false; window.isoCancelFlag = true;
                                if (shadowRoot) {
                                    let histDiv = shadowRoot.getElementById('terminal-history');
                                    if (histDiv) histDiv.innerHTML += `<div style="color:#ff3366;font-weight:bold;">^C (Terminated)</div>`;
                                    let inputLine = shadowRoot.getElementById('terminal-input-line');
                                    if (inputLine) { inputLine.style.display = 'flex'; setTimeout(() => { shadowRoot.getElementById('iso-url').focus(); }, 50); }
                                    let stat = shadowRoot.getElementById('iso-engine-status');
                                    if (stat) { stat.innerText = 'STANDBY'; stat.style.color = '#00ccff'; }
                                }
                            } else {
                                // Manuel Kopyalama
                                let text = isInput ? activeEl.value.substring(activeEl.selectionStart, activeEl.selectionEnd) : window.getSelection().toString();
                                if (text) navigator.clipboard.writeText(text).catch(()=>{});
                            }
                        } else if (k === 'v' && isInput) {
                            // Manuel Yapıştırma
                            navigator.clipboard.readText().then(text => {
                                let start = activeEl.selectionStart; let end = activeEl.selectionEnd;
                                activeEl.value = activeEl.value.substring(0, start) + text + activeEl.value.substring(end);
                                activeEl.selectionStart = activeEl.selectionEnd = start + text.length;
                            }).catch(()=>{});
                        } else if (k === 'a') {
                            // Manuel Tümünü Seçme
                            if (isInput) activeEl.select();
                            else {
                                let s = window.getSelection(); let r = document.createRange();
                                r.selectNodeContents(document.body); s.removeAllRanges(); s.addRange(r);
                            }
                        } else if (k === 'x' && isInput) {
                            // Manuel Kesme
                            let text = activeEl.value.substring(activeEl.selectionStart, activeEl.selectionEnd);
                            if (text) {
                                navigator.clipboard.writeText(text).catch(()=>{});
                                activeEl.value = activeEl.value.substring(0, activeEl.selectionStart) + activeEl.value.substring(activeEl.selectionEnd);
                                activeEl.selectionStart = activeEl.selectionEnd = activeEl.selectionStart;
                            }
                        }
                    }
                }

            } catch(err) { console.error('Keyboard Shield Error:', err); }
        }, { capture: true, passive: false });








    document.addEventListener('DOMContentLoaded', () => {
        let shield = document.createElement('style');
        shield.innerHTML = 'iframe:not(#isobrowse-ghost-canvas) { pointer-events: none !important; }';
        document.documentElement.appendChild(shield);
    });
    
    setInterval(() => {
        let iframes = document.getElementsByTagName('iframe');
        for(let i=0; i<iframes.length; i++) {
            if(iframes[i].id !== 'isobrowse-ghost-canvas') {
                iframes[i].style.pointerEvents = 'none';
            } else {
                iframes[i].style.pointerEvents = 'auto';
            }
        }
    }, 1000);

        try { 
            window.open = function(url) { 
                if (url && typeof url === 'string') { window.top.location.href = url; } 
                return null; 
            }; 
        } catch(e) {}

        setInterval(function() {
            let links = document.querySelectorAll('a[target="_blank"], a[target="_new"], a[target="_top"]');
            for (let i = 0; i < links.length; i++) {
                links[i].setAttribute('target', '_self');
            }
        }, 250); 
        
        document.addEventListener('click', function(e) {
            let a = e.target.closest('a');
            if (a && a.href && a.href.startsWith('http')) {
                if (a.getAttribute('target') === '_blank' || a.getAttribute('target') === '_new') {
                    a.setAttribute('target', '_self');
                }
                if (window !== window.top) { 
                    e.preventDefault(); 
                    e.stopPropagation();
                    window.parent.postMessage({type: 'SURF_NAVIGATE', url: a.href}, '*');
                }
            }
        }, true);

        if (window === window.top) {
            window.isoCurrentRam = 0; 
            window.isoCurrentCpu = 0;
            window.isoIsTyping = false;
            window.isoIsRunning = false; 
            window.isoCancelFlag = false; 
            
            window.isoCmdHistory = [];
            window.isoCmdIndex = -1;

            window.addToCmdHistory = (cmd) => {
                if(cmd && cmd.trim() !== '') {
                    if(window.isoCmdHistory[window.isoCmdHistory.length - 1] !== cmd) {
                        window.isoCmdHistory.push(cmd);
                    }
                    window.isoCmdIndex = window.isoCmdHistory.length;
                }
            };

            window.addEventListener('message', (e) => {
                // YENİ EKLENEN KOD: Iframe içinden gelen gizli kopyalama sinyalini yakalar
                if (e.data && e.data.type === 'COPY_TEXT') {
                    navigator.clipboard.writeText(e.data.payload).catch(()=>{});
                }
                
                // BUNLAR ZATEN VARDI
                if (e.data && (e.data.type === 'SURF_NAVIGATE' || e.data.type === 'NAVIGATE')) {
            
                    let safeUrl = "/nojs " + e.data.url;
                    if(document.getElementById('isobrowse-shadow-host')) {
                        document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById('iso-url').value = safeUrl;
                    }
                    if(window.ipc) window.ipc.postMessage("RUN_PIPELINE:" + safeUrl);
                }
                if (e.data && e.data.type === 'INJECT_CMD') {
                    let host = document.getElementById('isobrowse-shadow-host');
                    if (host && host.shadowRoot) {
                        let termInput = host.shadowRoot.getElementById('iso-url');
                        if (termInput) {
                            let currentVal = termInput.value.trim();
                            if (currentVal !== '' && !currentVal.endsWith('|')) {
                                termInput.value = currentVal + ' | /run ' + e.data.cmd + ' ';
                            } else {
                                termInput.value = currentVal + ' /run ' + e.data.cmd + ' ';
                            }
                            setTimeout(() => { termInput.focus(); }, 100);
                        }
                    }
                }
            });

            window.openHelpModal = function() {
                let host = document.getElementById('isobrowse-shadow-host');
                if(host) {
                    let modal = host.shadowRoot.getElementById('iso-help-modal');
                    if(modal) modal.style.display = 'flex';
                }
            };

            window.openExamplesModal = function() {
                let host = document.getElementById('isobrowse-shadow-host');
                if(host) {
                    let modal = host.shadowRoot.getElementById('iso-examples-modal');
                    if(modal) modal.style.display = 'flex';
                }
            };

            window.injectExample = function(cmd) {
                let spot = document.getElementById('iso-spotlight-home');
                let host = document.getElementById('isobrowse-shadow-host');
                
                if (host && host.shadowRoot) {
                    let modal = host.shadowRoot.getElementById('iso-examples-modal');
                    if (modal) modal.style.display = 'none';
                }

                if (spot && window.getComputedStyle(spot).display !== 'none') {
                    let sIn = document.getElementById('spotlight-input');
                    if (sIn) { sIn.value = cmd; setTimeout(() => { sIn.focus(); }, 50); }
                } else if (host && host.shadowRoot) {
                    let tIn = host.shadowRoot.getElementById('iso-url');
                    if (tIn) { tIn.value = cmd; setTimeout(() => { tIn.focus(); }, 50); }
                }
            };

            const injectIsoBrowseUI = () => {
              try {
                if (document.getElementById('isobrowse-shadow-host')) return;

                let isHome = window.location.hostname.includes('captive.apple.com');

                if (isHome) {
                    let w_html = `
                        <div id="iso-spotlight-home" style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; font-family: monospace; background: #050505;">
                            <h1 style="color: #00ccff; text-shadow: 0 0 20px #00ccff55; font-size: 48px; margin-bottom: 5px; letter-spacing: 2px;">⚡ IsoBrowse Pipeline</h1>
                            <p style="color: #888; margin-top: 0; font-size: 14px;">Secure & Isolated Stdin/Stdout Data Processor</p>
                            
                            <div style="width: 80%; max-width: 900px; margin-top: 40px; position: relative;">
                                <textarea id="spotlight-input" placeholder="/get api.com/data.json | /run jq '.'" autocomplete="off" spellcheck="false" style="width: 100%; padding: 25px 120px 25px 30px; font-size: 24px; line-height: 1.6; letter-spacing: 0.5px; font-family: monospace; background: #0a0a0a; color: #00ff41; border: 1px solid #333; border-radius: 12px; outline: none; box-shadow: 0 10px 30px rgba(0, 255, 65, 0.05); transition: all 0.3s ease; resize: none; height: 120px; word-break: break-all; white-space: pre-wrap;"></textarea>
                                <button id="spotlight-run" style="position: absolute; right: 10px; top: 10px; bottom: 10px; background: #00ff41; color: #000; border: none; font-size: 18px; font-weight: bold; font-family: monospace; padding: 0 35px; border-radius: 8px; cursor: pointer; transition: 0.2s;">RUN</button>
                            </div>

                            <div style="margin-top: 30px; display: flex; gap: 15px; color: #555; font-size: 14px; align-items: center; flex-wrap: wrap; justify-content: center;">
                                <span>OFFICIAL WORKERS:</span>
                                <div class="fav-btn" style="color: #b366ff; border: 1px dashed #b366ff; padding: 8px 15px; border-radius: 6px; cursor: pointer; background: #111; transition:0.2s;" onmouseover="this.style.background='#0a0a0a'" onmouseout="this.style.background='#111'" onclick="document.getElementById('spotlight-input').value += ' /run base64 '">🔐 Base64</div>
                                <div class="fav-btn" style="color: #00ff41; border: 1px dashed #00ff41; padding: 8px 15px; border-radius: 6px; cursor: pointer; background: #111; transition:0.2s;" onmouseover="this.style.background='#0a0a0a'" onmouseout="this.style.background='#111'" onclick="document.getElementById('spotlight-input').value += ' /run csv2json '">📊 Csv2Json</div>
                                <div class="fav-btn" style="color: #ff3366; border: 1px dashed #ff3366; padding: 8px 15px; border-radius: 6px; cursor: pointer; background: #111; transition:0.2s;" onmouseover="this.style.background='#0a0a0a'" onmouseout="this.style.background='#111'" onclick="document.getElementById('spotlight-input').value += ' /run bytecount '">📏 ByteCount</div>
                                <div class="fav-btn" style="color: #aaa; border: 1px dashed #666; padding: 8px 15px; border-radius: 6px; cursor: pointer; background: #111; transition:0.2s;" onmouseover="this.style.background='#0a0a0a'" onmouseout="this.style.background='#111'" onclick="document.getElementById('spotlight-input').value = '/upload | ' + document.getElementById('spotlight-input').value">📂 Upload</div>
                                <div class="fav-btn" style="color: #00ccff; border: 1px solid #00ccff; padding: 8px 15px; border-radius: 6px; cursor: pointer; background: #002233; font-weight:bold; transition:0.2s;" onclick="window.openHelpModal()">[ CODEX ]</div>
                                <div class="fav-btn" style="color: #ffcc00; border: 1px solid #ffcc00; padding: 8px 15px; border-radius: 6px; cursor: pointer; background: #332b00; font-weight:bold; transition:0.2s;" onclick="window.openExamplesModal()">[ 💡 ] EXAMPLES</div>
                            </div>
                        </div>
                    `;

                    document.body.innerHTML = w_html;
                    document.body.style.backgroundColor = '#050505';
                    document.body.style.margin = '0';
                    document.body.style.overflow = 'hidden';
                } else {
                    document.body.style.marginTop = '40px';
                    document.body.style.marginBottom = '200px'; 
                    const gravityMotor = () => {
                        let fixedElements = document.querySelectorAll('header, nav, #masthead-container, ytd-masthead, tp-yt-app-drawer, #header, .navbar');
                        fixedElements.forEach(el => {
                            let st = window.getComputedStyle(el);
                            if (st.position === 'fixed' || st.position === 'sticky') {
                                if (st.top === '0px' || el.style.top === '0px') {
                                    el.style.setProperty('top', '40px', 'important');
                                }
                                if (st.bottom === '0px' || el.style.bottom === '0px') {
                                    el.style.setProperty('bottom', '200px', 'important');
                                }
                            }
                        });
                    };
                    gravityMotor(); 
                    setInterval(gravityMotor, 1000); 
                }

                const host = document.createElement('div');
                host.id = 'isobrowse-shadow-host';
                host.style.cssText = 'position:fixed; top:0; left:0; width:100%; height:100vh; z-index:2147483647; background:transparent; pointer-events:none;';
                document.documentElement.appendChild(host);

                const shadow = host.attachShadow({mode: 'open'});

                const style = document.createElement('style');
                style.innerHTML = `
                    @keyframes iso-blink { 0% { opacity: 1; } 50% { opacity: 0.3; color: #fff; } 100% { opacity: 1; } }
                    .iso-alarm-active { animation: iso-blink 1s infinite; color: #ff3366 !important; font-weight: bold; }
                    * { box-sizing: border-box; font-family: monospace; margin: 0; padding: 0; }
                    
                    #top-bar {
                        position: absolute; top: 0; width: 100%; height: 40px; background: #050505; border-bottom: 1px solid #333; 
                        pointer-events: auto; display: flex; align-items: center; justify-content: space-between; padding: 0 15px;
                        box-shadow: 0 5px 15px rgba(0,0,0,0.5); font-size: 13px;
                    }
                    #bottom-bar {
                        position: absolute; bottom: 0; width: 100%; height: 200px; background: #0a0a0a; border-top: 1px solid #00ccff; 
                        pointer-events: auto; display: flex; flex-direction: column; padding: 10px 25px; box-shadow: 0 -5px 20px rgba(0,204,255,0.05);
                    }
                    #terminal-content {
                        flex-grow: 1; overflow-y: auto; display: flex; flex-direction: column; padding-right: 15px; 
                        font-family: monospace; font-size: 19px; line-height: 1.6; letter-spacing: 0.5px;
                    }
                    #terminal-content::-webkit-scrollbar { width: 10px; }
                    #terminal-content::-webkit-scrollbar-track { background: #000; border-radius: 4px; }
                    #terminal-content::-webkit-scrollbar-thumb { background: #333; border-radius: 4px; }
                    #terminal-content::-webkit-scrollbar-thumb:hover { background: #00ccff; }

                    button { cursor: pointer; font-weight: bold; outline: none; transition: 0.2s; }
                    .gap5 { display: flex; gap: 8px; align-items: center; }
                    .gap20 { display: flex; gap: 20px; align-items: center; width: 60%; }
                `;
                shadow.appendChild(style);

                const topBar = document.createElement('div');
                topBar.id = 'top-bar';
                if (isHome) { topBar.style.display = 'none'; }
                topBar.innerHTML = `
                    <div class="gap20" style="color: #888;">
                        <span style="font-weight:bold; letter-spacing:1px; color:#fff;">⚡ IsoBrowse</span>
                        <span>STATE: <span id="iso-engine-status" style="color:#00ff41; font-weight:bold;">SECURE_PIPELINE</span></span>
                        <span><span id="iso-cpu-label">SYS_CPU:</span> <span id="iso-cpu" style="color:#ffcc00;">0.0 %</span></span>
                        <span>RAM: <span id="iso-ram" style="color:#ff3366;">0 MB</span></span>
                    </div>
                    <div class="gap5">
                        <button id="iso-home-btn" style="color:#00ccff; border: 1px solid #00ccff; background: #050505; padding: 4px 12px; border-radius: 4px;">[ TERMINAL HOME ]</button>
                    </div>
                `;
                shadow.appendChild(topBar);

                const bottomBar = document.createElement('div');
                bottomBar.id = 'bottom-bar';
                if (isHome) { bottomBar.style.display = 'none'; }
                bottomBar.innerHTML = `
                    <div style="display: flex; justify-content: space-between; width: 100%; border-bottom: 1px dashed #333; padding-bottom: 5px; margin-bottom: 10px;">
                        <span style="color: #888; font-weight: bold; letter-spacing: 1px; font-size: 13px;">INTEGRATED TERMINAL</span>
                        <div style="display:flex; gap: 10px;">
                            <button id="iso-help-btn" style="background: #111; color: #00ccff; font-size: 13px; border: 1px dashed #00ccff; border-radius: 4px; padding: 4px 12px;" onclick="window.parent.openHelpModal()">[ CODEX ]</button>
                            <button id="iso-examples-btn" style="background: #111; color: #ffcc00; font-size: 13px; border: 1px dashed #ffcc00; border-radius: 4px; padding: 4px 12px;" onclick="window.parent.openExamplesModal()">[ 💡 ] EXAMPLES</button>
                            <button id="iso-go" style="background: #00ccff; color: #000; font-size: 13px; border: none; border-radius: 4px; padding: 4px 20px;">EXECUTE</button>
                        </div>
                    </div>
                    <div id="terminal-content">
                        <div id="terminal-history" style="display: flex; flex-direction: column; gap: 8px; margin-bottom: 10px; color: #aaa;"></div>
                        <div id="terminal-input-line" style="display: flex; align-items: flex-start; width: 100%;">
                            <span style="color: #00ccff; font-weight: bold; margin-right: 15px; white-space: nowrap;">isobrowse@local:~$</span>
                            <textarea id="iso-url" spellcheck="false" autocomplete="off" placeholder="/help (Display commands)" style="flex-grow: 1; background: transparent; color: #00ff41; border: none; font-size: 19px; line-height: 1.6; letter-spacing: 0.5px; font-family: monospace; outline: none; resize: none; min-height: 80px; word-break: break-all; white-space: pre-wrap;"></textarea>
                        </div>
                    </div>
                `;
                shadow.appendChild(bottomBar);

                const helpModal = document.createElement('div');
                helpModal.id = 'iso-help-modal';
                helpModal.style.cssText = 'display:none; position:fixed; top:0; left:0; width:100%; height:100%; background:rgba(0,0,0,0.85); z-index:2147483648; align-items:center; justify-content:center; backdrop-filter: blur(5px); pointer-events:auto;';
                
                helpModal.innerHTML = `
                    <div style="background:#0a0a0a; border:2px solid #00ccff; padding:30px; border-radius:12px; max-width:800px; width:90%; color:#fff; font-family:monospace; position:relative; box-shadow: 0 0 30px rgba(0,204,255,0.15); max-height: 90vh; overflow-y: auto;">
                        <button onclick="this.parentElement.parentElement.style.display='none'" style="position:absolute; right:20px; top:20px; background:transparent; border:none; color:#ff3366; font-size:20px; cursor:pointer; font-weight:bold;">X</button>
                        <h2 style="color:#00ccff; margin-top:0; border-bottom:1px dashed #333; padding-bottom:10px; font-size:24px;">⚡ ISOBROWSE CODEX</h2>
                        
                        <div style="display:flex; flex-direction:column; gap:20px; font-size:14px; margin-top: 20px;">
                            <div>
                                <h3 style="color:#888; font-size:12px; letter-spacing:2px; margin-bottom:10px;">[ DATA SOURCES ]</h3>
                                <div style="display:flex; flex-direction:column; gap:8px;">
                                    <div style="background:#111; padding:8px; border-left:3px solid #00ff41;"><strong style="color:#00ff41;">/read &lt;file&gt;</strong> - Reads a local file.</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #00ccff;"><strong style="color:#00ccff;">/cat &lt;file&gt;</strong> - Reads a local file (Alias).</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #00ccff;"><strong style="color:#00ccff;">/get &lt;url&gt;</strong> - Fetches raw data (JSON/MD) into pipeline.</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #00ccff;"><strong style="color:#00ccff;">/nojs &lt;url&gt;</strong> - Fetches raw DOM from a website securely.</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #ffcc00;"><strong style="color:#ffcc00;">/echo &lt;text&gt;</strong> - Passes raw text directly into pipeline.</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #00ff41;"><strong style="color:#00ff41;">/upload</strong> - Opens native file picker.</div>
                                </div>
                            </div>

                            <div>
                                <h3 style="color:#888; font-size:12px; letter-spacing:2px; margin-bottom:10px;">[ WORKERS & TOOLS ]</h3>
                                <div style="display:flex; flex-direction:column; gap:8px;">
                                    <div style="background:#111; padding:8px; border-left:3px solid #00ccff;"><strong style="color:#00ccff;">/catalog</strong> - Opens the IsoModules Catalog.</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #ff9900;"><strong style="color:#ff9900;">/run &lt;name_or_url&gt; &lt;args&gt;</strong> - Executes a WASM worker.</div>
                                    <div style="background:#111; padding:8px; border-left:3px solid #ff3366;"><strong style="color:#ff3366;">/rhai &lt;script&gt;</strong> - Executes fast native scripting.</div>
                                </div>
                            </div>
                        </div>
                    </div>
                `;
                shadow.appendChild(helpModal);

                const examplesModal = document.createElement('div');
                examplesModal.id = 'iso-examples-modal';
                examplesModal.style.cssText = 'display:none; position:fixed; top:0; left:0; width:100%; height:100%; background:rgba(0,0,0,0.85); z-index:2147483648; align-items:center; justify-content:center; backdrop-filter: blur(5px); pointer-events:auto;';
                
                examplesModal.innerHTML = `
                    <div style="background:#0a0a0a; border:2px solid #ffcc00; padding:30px; border-radius:12px; max-width:800px; width:90%; color:#fff; font-family:monospace; position:relative; box-shadow: 0 0 30px rgba(255,204,0,0.15); max-height: 90vh; overflow-y: auto;">
                        <button onclick="this.parentElement.parentElement.style.display='none'" style="position:absolute; right:20px; top:20px; background:transparent; border:none; color:#ff3366; font-size:20px; cursor:pointer; font-weight:bold;">X</button>
                        <h2 style="color:#ffcc00; margin-top:0; border-bottom:1px dashed #333; padding-bottom:10px; font-size:24px;">💡 QUICK START EXAMPLES</h2>
                        <p style="color:#888; margin-bottom: 20px;">Click on any example below to instantly copy it into your terminal.</p>
                        
                        <div style="display:flex; flex-direction:column; gap:12px; font-size:14px;">
                            
                            <div data-cmd="/echo 'hello isobrowse' | /run uppercase" style="background:#111; padding:15px; border-left:4px solid #00ff41; cursor:pointer; transition:0.2s; border-radius:4px;" onmouseover="this.style.background='#222'" onmouseout="this.style.background='#111'" onclick="window.parent.injectExample(this.getAttribute('data-cmd'))">
                                <div style="color:#aaa; font-size:12px; margin-bottom:6px;">1. Basic Text Manipulation (Convert to Uppercase)</div>
                                <strong style="color:#00ff41; font-size: 16px;">&gt; /echo "hello isobrowse" | /run uppercase</strong>
                            </div>
                            
                            <div data-cmd="/get news.ycombinator.com | /run htmlclean | /run linkextract | /run sort" style="background:#111; padding:15px; border-left:4px solid #00ccff; cursor:pointer; transition:0.2s; border-radius:4px;" onmouseover="this.style.background='#222'" onmouseout="this.style.background='#111'" onclick="window.parent.injectExample(this.getAttribute('data-cmd'))">
                                <div style="color:#aaa; font-size:12px; margin-bottom:6px;">2. Extract & Sort All Links From a Website</div>
                                <strong style="color:#00ccff; font-size: 16px;">&gt; /get news.ycombinator.com | /run htmlclean | /run linkextract | /run sort</strong>
                            </div>
                            
                            <div data-cmd="/get example.com | /run linkextract" style="background:#111; padding:15px; border-left:4px solid #ffcc00; cursor:pointer; transition:0.2s; border-radius:4px;" onmouseover="this.style.background='#222'" onmouseout="this.style.background='#111'" onclick="window.parent.injectExample(this.getAttribute('data-cmd'))">
                                <div style="color:#aaa; font-size:12px; margin-bottom:6px;">3. Extract All Links from a Website</div>
                                <strong style="color:#ffcc00; font-size: 16px;">&gt; /get example.com | /run linkextract</strong>
                            </div>

                            <div data-cmd="/get jsonplaceholder.typicode.com/todos/1 | /run jq '.'" style="background:#111; padding:15px; border-left:4px solid #ff3366; cursor:pointer; transition:0.2s; border-radius:4px;" onmouseover="this.style.background='#222'" onmouseout="this.style.background='#111'" onclick="window.parent.injectExample(this.getAttribute('data-cmd'))">
                                <div style="color:#aaa; font-size:12px; margin-bottom:6px;">4. Fetch API Data and Format JSON</div>
                                <strong style="color:#ff3366; font-size: 16px;">&gt; /get jsonplaceholder.typicode.com/todos/1 | /run jq '.'</strong>
                            </div>

                            <div data-cmd="/echo 'my secret data' | /run base64" style="background:#111; padding:15px; border-left:4px solid #b366ff; cursor:pointer; transition:0.2s; border-radius:4px;" onmouseover="this.style.background='#222'" onmouseout="this.style.background='#111'" onclick="window.parent.injectExample(this.getAttribute('data-cmd'))">
                                <div style="color:#aaa; font-size:12px; margin-bottom:6px;">5. Instant Data Encryption (Base64)</div>
                                <strong style="color:#b366ff; font-size: 16px;">&gt; /echo "my secret data" | /run base64</strong>
                            </div>
                            
                        </div>
                    </div>
                `;
                shadow.appendChild(examplesModal);

                const ghostFrame = document.createElement('iframe');
                ghostFrame.id = 'isobrowse-ghost-canvas';
                ghostFrame.sandbox = 'allow-same-origin allow-scripts allow-forms'; 
                ghostFrame.style.cssText = 'position:fixed; top:40px; left:0; width:100%; height:calc(100vh - 240px); border:none; background:#fff; z-index:2147483646; display:none;';
                document.documentElement.appendChild(ghostFrame);

                const getEl = (id) => shadow.getElementById(id);
                const urlInput = getEl('iso-url');

                const setupInput = (inputEl) => {
                    if(!inputEl) return;
                    
                    inputEl.addEventListener('focus', () => { window.isoIsTyping = true; });
                    inputEl.addEventListener('blur', () => { window.isoIsTyping = false; });
                    
                    inputEl.addEventListener('keydown', (e) => {
                        e.stopPropagation();

                        if (e.key === 'ArrowUp') {
                            if (inputEl.selectionStart === 0) {
                                e.preventDefault();
                                if (window.isoCmdHistory.length > 0 && window.isoCmdIndex > 0) {
                                    window.isoCmdIndex--;
                                    inputEl.value = window.isoCmdHistory[window.isoCmdIndex];
                                }
                            }
                        } else if (e.key === 'ArrowDown') {
                            if (inputEl.selectionStart === inputEl.value.length) {
                                e.preventDefault();
                                if (window.isoCmdIndex < window.isoCmdHistory.length - 1) {
                                    window.isoCmdIndex++;
                                    inputEl.value = window.isoCmdHistory[window.isoCmdIndex];
                                } else {
                                    window.isoCmdIndex = window.isoCmdHistory.length;
                                    inputEl.value = '';
                                }
                            }
                        } else if (e.key === 'Enter') {
                            if (!e.shiftKey) { 
                                e.preventDefault();
                                inputEl.blur();
                                let spot = document.getElementById('iso-spotlight-home');
                                let btn = (spot && window.getComputedStyle(spot).display !== 'none') ? document.getElementById('spotlight-run') : getEl('iso-go');
                                if(btn) btn.click();
                            }
                        }
                    });
                };

                setupInput(urlInput);

                window.updateTerminal = (msg) => { 
                    if(window.isoCancelFlag) return; 
                    let host = document.getElementById('isobrowse-shadow-host');
                    if(host && host.shadowRoot) {
                        let histDiv = host.shadowRoot.getElementById('terminal-history');
                        if (histDiv) {
                            let newEntry = document.createElement('div');
                            let color = msg.includes('[ERROR]') ? '#ff3366' : '#888';
                            newEntry.innerHTML = `<span style="color: ${color};">${msg}</span>`;
                            histDiv.appendChild(newEntry);
                            let tc = host.shadowRoot.getElementById('terminal-content');
                            if(tc) tc.scrollTop = tc.scrollHeight;
                        }
                    }
                };

                window.updateOsTelemetry = (cpuVal, ramMB) => {
                    let clbl = getEl('iso-cpu-label');
                    if (clbl && clbl.innerText !== 'EXEC_TIME:') {
                        getEl('iso-cpu').innerText = cpuVal.toFixed(1) + ' %';
                        getEl('iso-ram').innerText = ramMB + ' MB';
                        window.isoCurrentRam = ramMB; window.isoCurrentCpu = cpuVal;
                    }
                };

                const navigate = () => {
                    let spot = document.getElementById('iso-spotlight-home');
                    let isSpotlightVisible = spot && window.getComputedStyle(spot).display !== 'none';
                    let targetInput = isSpotlightVisible ? document.getElementById('spotlight-input') : getEl('iso-url');
                    let target = targetInput.value.trim();

                    if (target === '') return;
                    
                    window.addToCmdHistory(target);
                    window.isoIsRunning = true; 
                    window.isoCancelFlag = false;

                    let safeTargetForRust = target.replace(/\\\n/g, ' ').replace(/\\\r\n/g, ' ').replace(/\n/g, ' ');

                    let histDiv = getEl('terminal-history');
                    if (histDiv) {
                        let newEntry = document.createElement('div');
                        newEntry.innerHTML = `<span style="color: #00ccff; font-weight: bold; margin-right: 15px;">isobrowse@local:~$</span><span style="color: #fff; white-space: pre-wrap;">${target}</span>`;
                        histDiv.appendChild(newEntry);
                        targetInput.value = ''; 
                        getEl('iso-url').value = ''; 
                        
                        let inputLine = getEl('terminal-input-line');
                        if (inputLine) inputLine.style.display = 'none';

                        let tc = getEl('terminal-content');
                        if(tc) tc.scrollTop = tc.scrollHeight;
                    }
                    window.updateTerminal("> [SYSTEM]: Sequence initiated...");

                    getEl('top-bar').style.display = 'flex';
                    getEl('bottom-bar').style.display = 'flex';
                    if(spot) spot.style.display = 'none';

                    let gc = document.getElementById('isobrowse-ghost-canvas');
                    if (gc) { 
                        gc.style.display = 'none'; 
                        gc.removeAttribute('srcdoc');
                        gc.src = 'about:blank';
                    }
                    
                    let ws = document.getElementById('iso-workspace-view');
                    if (!ws) {
                        ws = document.createElement('div');
                        ws.id = 'iso-workspace-view';
                        ws.style.cssText = 'position:fixed; top:40px; left:0; width:100%; height:calc(100vh - 200px); background:#050505; overflow-y:auto; z-index:2147483645; color:#00ff41; padding-bottom: 20px; box-sizing: border-box;';
                        document.documentElement.appendChild(ws);
                    }
                    ws.style.display = 'block';
                    
                    ws.innerHTML = `
                        <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; height:100%; background:#050505; font-family:monospace;'>
                            <div style='color:#00ff41; font-size:22px; margin-bottom: 20px;' class='iso-alarm-active'>[ SYSTEM PIPELINE ACTIVE ]</div>
                            <div style='width:300px; height:6px; background:#111; border:1px solid #333; margin-top:20px; position:relative; overflow:hidden;'>
                                <div style='position:absolute; height:100%; width:30%; background:#00ff41; box-shadow:0 0 10px #00ff41; animation:iso-load 1s infinite linear;'></div>
                            </div>
                            <style>@keyframes iso-load { 0% { left:-30%; } 100% { left:100%; } }</style>
                            <p style='color:#888; font-size:14px; margin-top:25px;'>> Allocating memory sandbox & executing pipeline...</p>
                        </div>
                    `;

                    if(window.ipc) window.ipc.postMessage("RUN_PIPELINE:" + safeTargetForRust); 
                };

                getEl('iso-home-btn').onclick = () => {
                    let spot = document.getElementById('iso-spotlight-home');
                    if (spot) {
                        let stat = getEl('iso-engine-status');
                        if (stat) { stat.innerText = 'SECURE_PIPELINE'; stat.style.color = '#00ff41'; }
                        
                        let clbl = getEl('iso-cpu-label');
                        if (clbl) clbl.innerText = 'SYS_CPU:';

                        spot.style.display = 'flex';
                        getEl('top-bar').style.display = 'none';
                        getEl('bottom-bar').style.display = 'none';
                        let ws = document.getElementById('iso-workspace-view');
                        if (ws) ws.style.display = 'none';
                        document.getElementById('isobrowse-ghost-canvas').style.display = 'none';
                        let sIn = document.getElementById('spotlight-input');
                        if(sIn) { sIn.value = ''; sIn.focus(); }
                    }
                };

                getEl('iso-go').onclick = navigate;

                if (isHome) {
                    setTimeout(() => {
                        let sIn = document.getElementById('spotlight-input');
                        let sBtn = document.getElementById('spotlight-run');
                        if (sIn && sBtn) {
                            setupInput(sIn);
                            sIn.focus();
                            sBtn.onclick = () => { navigate(); };
                        }
                    }, 200);
                }
              } catch (e) {
                  document.body.innerHTML = '<div style="color:#ff3366; font-family:monospace; padding:50px;"><h2>UI Injection Failed!</h2><p>' + e.toString() + '</p></div>';
              }
            };

            window.renderSurfMode = (html, url, cpu, ram, blocked) => {
                if (window.isoCancelFlag) return; 

                const getEl = (id) => document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById(id);

                window.isoIsRunning = false; 
                let inputLine = getEl('terminal-input-line');
                if (inputLine) {
                    inputLine.style.display = 'flex';
                    let tc = getEl('terminal-content');
                    if (tc) tc.scrollTop = tc.scrollHeight;
                    setTimeout(() => { getEl('iso-url').focus(); }, 50);
                }

                let tBar = getEl('top-bar');
                if(tBar) tBar.style.display = 'flex';
                let bBar = getEl('bottom-bar');
                if(bBar) bBar.style.display = 'flex';
                let spot = document.getElementById('iso-spotlight-home');
                if(spot) spot.style.display = 'none';

                let clbl = getEl('iso-cpu-label');
                if (clbl) clbl.innerText = 'EXEC_TIME:';
                let secs = (cpu / 1000).toFixed(2);
                getEl('iso-cpu').innerText = secs + "s";
                getEl('iso-ram').innerText = ram + " KB";

                if (url.startsWith('isobrowse://sandbox/catalog_native')) {
                    let gc = document.getElementById('isobrowse-ghost-canvas');
                    if (gc) { gc.style.display = 'none'; gc.removeAttribute('srcdoc'); gc.src = 'about:blank'; }
                    
                    let ws = document.getElementById('iso-workspace-view');
                    if (!ws) {
                        ws = document.createElement('div');
                        ws.id = 'iso-workspace-view';
                        ws.style.cssText = 'position:fixed; top:40px; left:0; width:100%; height:calc(100vh - 200px); background:#050505; overflow-y:auto; z-index:2147483645; color:#00ff41; padding-bottom: 20px; box-sizing: border-box;';
                        document.documentElement.appendChild(ws);
                    }
                    ws.style.display = 'block';
                    ws.innerHTML = `
                        <div style='display:flex; flex-direction:column; align-items:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; padding: 20px;'>
                            <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55; margin-bottom: 10px;'>🧰 IsoModules Catalog</h1>
                            <p style='color:#888; margin-top:0; margin-bottom: 20px;'>Live Official Modules loaded from GitHub</p>
                            <input type="text" oninput="window.filterCatalog(this.value)" placeholder="Search modules (e.g. json, crypto, text)..." style="width: 80%; max-width: 600px; padding: 15px; background: #0a0a0a; border: 1px solid #00ccff; color: #fff; font-family: monospace; font-size: 16px; border-radius: 8px; outline: none; margin-bottom: 30px; box-shadow: 0 0 15px rgba(0,204,255,0.1);">
                            <div id="modules-container" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; width: 90%; max-width: 1200px;">
                                <div style="color:#00ccff; text-align:center; width:100%; grid-column: 1 / -1; font-size: 18px;"><br><br>⚡ Fetching live catalog from GitHub...</div>
                            </div>
                        </div>
                    `;
                    window.loadCatalogData();
                    window.updateTerminal("> [SYSTEM]: Catalog Interface Active.");
                }
                else if (url.startsWith('isobrowse://sandbox/')) {
                    if (url.includes('/chart')) {
                        let ws = document.getElementById('iso-workspace-view');
                        if (ws) ws.style.display = 'none';
                        let gc = document.getElementById('isobrowse-ghost-canvas');
                        if (gc) {
                            gc.style.display = 'block';
                            gc.srcdoc = html;
                        }
                        window.updateTerminal("> [SYSTEM]: Pipeline Execution Complete. Live View Active.");
                    } else if (!url.includes('/catalog_native')) {
                        let gc = document.getElementById('isobrowse-ghost-canvas');
                        if (gc) { gc.style.display = 'none'; gc.removeAttribute('srcdoc'); gc.src = 'about:blank'; }
                        let ws = document.getElementById('iso-workspace-view');
                        if (!ws) {
                            ws = document.createElement('div');
                            ws.id = 'iso-workspace-view';
                            ws.style.cssText = 'position:fixed; top:40px; left:0; width:100%; height:calc(100vh - 200px); background:#050505; overflow-y:auto; z-index:2147483645; color:#00ff41; padding-bottom: 20px; box-sizing: border-box;';
                            document.documentElement.appendChild(ws);
                        }
                        ws.style.display = 'block';
                        ws.innerHTML = html; 
                        window.updateTerminal("> [SYSTEM]: Worker Execution Complete. Output ready.");
                    }
                } else {
                    let ws = document.getElementById('iso-workspace-view');
                    if (ws) ws.style.display = 'none';
                    let gc = document.getElementById('isobrowse-ghost-canvas');
                    if (gc) {
                        gc.style.display = 'block';
                        gc.srcdoc = html;
                    }
                    window.updateTerminal("> [SYSTEM]: Secure Render Complete. Web Shield Active.");
                }
            };

            const checkAndInject = () => {
                if (document.body) { injectIsoBrowseUI(); } 
                else { requestAnimationFrame(checkAndInject); }
            };
            checkAndInject();
        }
    "##;
    

    let webview = WebViewBuilder::new(&window)
        .with_initialization_script(init_script) 
        .with_ipc_handler({
            let proxy_ipc = proxy.clone();
            move |request| { let _ = proxy_ipc.send_event(UserEvent::IpcMessage(request.body().to_string())); }
        })
        .with_url("https://captive.apple.com/hotspot-detect.html") 
        .build()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::UpdateOsTelemetry { cpu, ram_mb }) => {
                let js_code = format!("if(window.updateOsTelemetry) window.updateOsTelemetry({}, {});", cpu, ram_mb);
                let _ = webview.evaluate_script(&js_code);
            }
            Event::UserEvent(UserEvent::UpdateTerminal(msg)) => {
                let js_code = format!("if(window.updateTerminal) window.updateTerminal('{}');", msg);
                let _ = webview.evaluate_script(&js_code);
            }
            Event::UserEvent(UserEvent::IpcMessage(msg)) => {
                
                if msg.starts_with("RUN_PIPELINE:") {

                    let raw_url = msg.replace("RUN_PIPELINE:", "");
                    let p_i = proxy.clone();
                    let client = Arc::clone(&http_client);
                    
                    thread::spawn(move || {
                        let start_time = Instant::now();
                        let raw_input = raw_url.trim();

                        if raw_input.starts_with("/help") || raw_input.starts_with("/catalog") || raw_input.starts_with("/explore") || raw_input.starts_with("/run ") || raw_input.starts_with("/fetch ") || raw_input.starts_with("/get ") || raw_input.starts_with("/rhai ") || raw_input.starts_with("/read ") || raw_input.starts_with("/cat ") || raw_input.starts_with("/echo ") || raw_input.starts_with("/upload") || raw_input.starts_with("/nojs ") {

                            let commands: Vec<&str> = raw_input.split('|').collect();
                            let mut pipe_data = String::new();
                            let total_commands = commands.len();
                            let mut final_ram_kb = 0;

                            for (index, part) in commands.iter().enumerate() {
                                let cmd = part.trim().to_string();
                                let is_last = index == total_commands - 1;

                                if cmd.starts_with("/help") {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [SYSTEM]: Loading IsoBrowse Codex...")));
                                    
                                    pipe_data = "IsoBrowse Commands\n\n/read <file>\n/cat <file>\n/get <url>\n/echo <text>\n/upload\n/nojs <url>\n/catalog\n/run <name_or_url>\n/rhai <script>".to_string();
                                    final_ram_kb += 1;
                                    
                                    if is_last {
                                        let success_html = format!("
                                        <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                            <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55;'>⚡ IsoBrowse Codex</h1>
                                            <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #00ff4111;'>
                                                <div style='background:#000; border:1px solid #00ff41; padding:20px; max-height: 400px; overflow-y: auto;'>
                                                    <div style='color:#00ff41; font-size:19px; line-height:1.6; letter-spacing:0.5px; white-space: pre-wrap;'>{}</div>
                                                </div>
                                            </div>
                                        </div>", pipe_data);
                                        let _ = p_i.send_event(UserEvent::WasmSurfRender { html: success_html, url: "isobrowse://sandbox/help".to_string(), cpu_ms: start_time.elapsed().as_millis(), ram_kb: final_ram_kb, blocked_count: 0 });
                                    }
                                }
                                else if cmd.starts_with("/catalog") || cmd.starts_with("/explore") {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [SYSTEM]: Connecting to GitHub and fetching live catalog...")));
                                    
                                    if is_last {
                                        let _ = p_i.send_event(UserEvent::WasmSurfRender { 
                                            html: "".to_string(), 
                                            url: "isobrowse://sandbox/catalog_native".to_string(), 
                                            cpu_ms: start_time.elapsed().as_millis(), 
                                            ram_kb: 5, 
                                            blocked_count: 0 
                                        });
                                    }
                                }
                                else if cmd.starts_with("/read ") || cmd.starts_with("/cat ") {
                                    let prefix = if cmd.starts_with("/read ") { "/read " } else { "/cat " };
                                    let path = cmd.strip_prefix(prefix).unwrap_or("").trim().trim_matches('"');
                                    let expanded_path = path.replace("~", &std::env::var("HOME").unwrap_or_default());
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [LOCAL READ]: Accessing {}...", expanded_path)));

                                    match std::fs::read_to_string(&expanded_path) {
                                        Ok(content) => {
                                            pipe_data = content.trim().to_string();
                                            final_ram_kb += pipe_data.len() / 1024;
                                            if is_last {
                                                let success_html = format!("
                                                <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                                    <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55;'>⚡ Local File Access Complete</h1>
                                                    <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #00ff4111;'>
                                                        <p style='color:#888;'>Target: <span style='color:#fff;'>{}</span></p>
                                                        <hr style='border:1px dashed #333; margin:15px 0;'>
                                                        <div style='background:#000; border:1px solid #00ff41; padding:20px; margin-top:10px; max-height: 400px; overflow-y: auto;'>
                                                            <div style='display:flex; justify-content:space-between; align-items:center; border-bottom:1px dashed #333; padding-bottom:10px; margin-bottom:10px;'>
                                                                <span style='color:#888; font-size:10px;'>[FILE CONTENTS]</span>
                                                                <button onclick='let val=this.parentElement.nextElementSibling.innerText; try{{navigator.clipboard.writeText(val);}}catch(e){{let ta=document.createElement(`textarea`);ta.value=val;document.body.appendChild(ta);ta.select();document.execCommand(`copy`);ta.remove();}} let oldBg=this.style.background; this.style.background=`#00ff41`; this.style.color=`#000`; this.innerText=`[ COPIED! ]`; setTimeout(()=>{{this.style.background=oldBg; this.style.color=`#00ff41`; this.innerText=`[ COPY DATA ]`;}}, 800);' style='background:#003300; color:#00ff41; border:1px solid #00ff41; cursor:pointer; padding:4px 10px; font-family:monospace; font-weight:bold; outline:none; transition:0.2s;'>[ COPY DATA ]</button>
                                                            </div>
                                                            <div style='color:#00ff41; font-size:16px; white-space: pre-wrap; line-height:1.5;'>{}</div>
                                                        </div>
                                                    </div>
                                                </div>
                                            ", expanded_path, pipe_data);
                                                let _ = p_i.send_event(UserEvent::WasmSurfRender { html: success_html, url: "isobrowse://sandbox/read".to_string(), cpu_ms: start_time.elapsed().as_millis(), ram_kb: final_ram_kb, blocked_count: 0 });
                                            }
                                        },
                                        Err(e) => {
                                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Read failed: {}", e)));
                                            let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>{}</p></div>", e);
                                            let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                            break;
                                        }
                                    }
                                }
                                else if cmd.starts_with("/get ") {
                                    let raw_url = cmd.strip_prefix("/get ").unwrap_or("").trim();
                                    let data_url = if raw_url.starts_with("http") { raw_url.to_string() } else { format!("https://{}", raw_url) };
                                    
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [PIPELINE GET]: Downloading raw data from {}...", data_url)));

                                    match client.get(&data_url).send() {
                                        Ok(resp) => {
                                            if let Ok(text) = resp.text() {
                                                pipe_data = text.trim().to_string();
                                                final_ram_kb += pipe_data.len() / 1024;
                                                
                                                if is_last {
                                                    let success_html = format!("
                                                    <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                                        <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55;'>⚡ Remote Data Fetched</h1>
                                                        <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #00ff4111;'>
                                                            <p style='color:#888;'>Source: <span style='color:#fff;'>{}</span></p>
                                                            <hr style='border:1px dashed #333; margin:15px 0;'>
                                                            <div style='background:#000; border:1px solid #00ff41; padding:20px; margin-top:10px; max-height: 400px; overflow-y: auto;'>
                                                                <div style='color:#00ff41; font-size:14px; white-space: pre-wrap; line-height:1.5;'>{}</div>
                                                            </div>
                                                        </div>
                                                    </div>", data_url, pipe_data);
                                                    let _ = p_i.send_event(UserEvent::WasmSurfRender { html: success_html, url: "isobrowse://sandbox/get".to_string(), cpu_ms: start_time.elapsed().as_millis(), ram_kb: final_ram_kb, blocked_count: 0 });
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Get failed: {}", e)));
                                            let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>Download Failed:<br>{}</p></div>", e);
                                            let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                            break;
                                        }
                                    }
                                }
                                else if cmd.starts_with("/nojs ") {
                                    let target_site = cmd.strip_prefix("/nojs ").unwrap_or("").trim();
                                    let data_url = if target_site.starts_with("http") { target_site.to_string() } else { format!("https://{}", target_site) };
                                    
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [PIPELINE GET]: Pulling raw DOM from {}...", data_url)));

                                    match client.get(&data_url).send() {
                                        Ok(resp) => {
                                            let final_url = resp.url().as_str().to_string();
                                            
                                            if let Ok(text) = resp.text() {
                                                pipe_data = text;
                                                final_ram_kb += pipe_data.len() / 1024;
                                                
                                                if is_last {
                                                    let lower_html = pipe_data.to_lowercase();
                                                    let blocked_trackers = lower_html.matches("<script").count() + lower_html.matches("<iframe").count() + lower_html.matches("google-analytics").count();

                                                    let html = pipe_data.clone()
                                                        .replace("<script", "<template").replace("<SCRIPT", "<template")
                                                        .replace("</script>", "</template>").replace("</SCRIPT>", "</template>")
                                                        .replace("<iframe", "<template").replace("<IFRAME", "<template")
                                                        .replace("</iframe>", "</template>").replace("</IFRAME>", "</template>")
                                                        .replace("<noscript", "<div class=\"iso-noscript\"").replace("<NOSCRIPT", "<div class=\"iso-noscript\"")
                                                        .replace("</noscript>", "</div>").replace("</NOSCRIPT>", "</div>");

                                                    let mut config = wasmtime::Config::new();
                                                    config.consume_fuel(true);
                                                    config.static_memory_maximum_size(500 * 1024 * 1024);
                                                    
                                                    if let Ok(engine) = wasmtime::Engine::new(&config) {
                                                        let mut linker = wasmtime::Linker::<WasiP1Ctx>::new(&engine);
                                                        let _ = preview1::add_to_linker_sync(&mut linker, |t| t);

                                                        let pr = p_i.clone();
                                                        let f_url = final_url.clone();
                                                        
                                                        let _ = linker.func_wrap("env", "render_html", move |mut c: wasmtime::Caller<'_, WasiP1Ctx>, ptr: i32, len: i32| {
                                                            let mem = c.get_export("memory").unwrap().into_memory().unwrap();
                                                            let mut d = vec![0u8; len as usize]; mem.read(&c, ptr as usize, &mut d).unwrap();
                                                            let final_output = String::from_utf8_lossy(&d).to_string();
                                                            let _ = pr.send_event(UserEvent::WasmSurfRender { html: final_output, url: f_url.clone(), cpu_ms: start_time.elapsed().as_millis(), ram_kb: final_ram_kb, blocked_count: blocked_trackers });
                                                        });
                                                        let _ = linker.func_wrap("env", "send_to_ui", |_c: wasmtime::Caller<'_, WasiP1Ctx>, _ptr: i32, _len: i32| {});

                                                        let wasi = wasmtime_wasi::WasiCtxBuilder::new().build_p1();
                                                        let mut store = wasmtime::Store::new(&engine, wasi);
                                                        let _ = store.set_fuel(u64::MAX); 

                                                        if let Ok(module) = wasmtime::Module::from_binary(&engine, WASM_ENGINE_GHOST) {
                                                            if let Ok(instance) = linker.instantiate(&mut store, &module) {
                                                                if let Ok(alloc) = instance.get_typed_func::<i32, i32>(&mut store, "alloc") {
                                                                    if let Ok(on_d) = instance.get_typed_func::<(i32, i32), ()>(&mut store, "on_data_received") {
                                                                        let h_b = html.as_bytes();
                                                                        if let Ok(h_p) = alloc.call(&mut store, h_b.len() as i32) {
                                                                            if instance.get_memory(&mut store, "memory").unwrap().write(&mut store, h_p as usize, h_b).is_ok() {
                                                                                let _ = on_d.call(&mut store, (h_p, h_b.len() as i32));
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Download failed: {}", e)));
                                            let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>{}</p></div>", e);
                                            let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                            break;
                                        }
                                    }
                                }
                                else if cmd.starts_with("/upload") {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal("> [SYSTEM]: Waiting for user to select a file...".to_string()));

                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        let expanded_path = path.display().to_string();
                                        let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [LOCAL READ]: Accessing {}...", expanded_path)));

                                        match std::fs::read_to_string(&path) {
                                            Ok(content) => {
                                                pipe_data = content.trim().to_string();
                                                final_ram_kb += pipe_data.len() / 1024;
                                                
                                                if is_last {
                                                    let success_html = format!("
                                                    <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                                        <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55;'>⚡ Local File Upload Complete</h1>
                                                        <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #00ff4111;'>
                                                            <p style='color:#888;'>Target: <span style='color:#fff;'>{}</span></p>
                                                            <hr style='border:1px dashed #333; margin:15px 0;'>
                                                            <div style='background:#000; border:1px solid #00ff41; padding:20px; margin-top:10px; max-height: 400px; overflow-y: auto;'>
                                                                <div style='color:#00ff41; font-size:19px; line-height:1.6; letter-spacing:0.5px; white-space: pre-wrap;'>{}</div>
                                                            </div>
                                                        </div>
                                                    </div>
                                                    ", expanded_path, pipe_data);

                                                    let _ = p_i.send_event(UserEvent::WasmSurfRender { html: success_html, url: "isobrowse://sandbox/upload".to_string(), cpu_ms: start_time.elapsed().as_millis(), ram_kb: final_ram_kb, blocked_count: 0 });
                                                }
                                            },
                                            Err(e) => {
                                                let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Read failed: {}", e)));
                                                let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>{}</p></div>", e);
                                                let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                                break;
                                            }
                                        }
                                    } else {
                                        let _ = p_i.send_event(UserEvent::UpdateTerminal("> [SYSTEM]: File selection cancelled.".to_string()));
                                        let err_html = format!("<div style='color:#ffcc00; font-family:monospace; text-align:center; padding:50px;'><h2>⚠️ PIPELINE CANCELLED</h2><p>File selection was aborted.</p></div>");
                                        let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                        break; 
                                    }

                                } 
                                else if cmd.starts_with("/echo ") {
                                    let text = cmd.strip_prefix("/echo ").unwrap_or("").trim();
                                    let mut unquoted = text;
                                    if (unquoted.starts_with('"') && unquoted.ends_with('"')) || (unquoted.starts_with('\'') && unquoted.ends_with('\'')) {
                                        unquoted = &unquoted[1..unquoted.len()-1];
                                    }
                                    pipe_data = unquoted.replace("\\n", "\n").replace("\\t", "\t");
                                
                                    // EĞER BU BORU HATTINDAKİ SON VEYA TEK KOMUTSA EKRANA BAS
                                    if is_last {
                                        let safe_pipe_data = pipe_data.replace("<", "&lt;").replace(">", "&gt;");
                                        let success_html = format!("
                                        <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                            <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55;'>⚡ Sandbox Execution Complete</h1>
                                            <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #00ff4111;'>
                                                <div style='background:#000; border:1px solid #00ff41; padding:20px; max-height: 400px; overflow-y: auto;'>
                                                    <div style='display:flex; justify-content:space-between; align-items:center; border-bottom:1px dashed #333; padding-bottom:10px; margin-bottom:10px;'>
                                                        <span style='color:#888; font-size:10px;'>[PIPELINE OUTPUT TERMINAL]</span>
                                                        <button onclick='let val=this.parentElement.nextElementSibling.innerText; try{{navigator.clipboard.writeText(val);}}catch(e){{let ta=document.createElement(`textarea`);ta.value=val;document.body.appendChild(ta);ta.select();document.execCommand(`copy`);ta.remove();}} let oldBg=this.style.background; this.style.background=`#00ff41`; this.style.color=`#000`; this.innerText=`[ COPIED! ]`; setTimeout(()=>{{this.style.background=oldBg; this.style.color=`#00ff41`; this.innerText=`[ COPY DATA ]`;}}, 800);' style='background:#003300; color:#00ff41; border:1px solid #00ff41; cursor:pointer; padding:4px 10px; font-family:monospace; font-weight:bold; outline:none; transition:0.2s;'>[ COPY DATA ]</button>
                                                    </div>
                                                    <div style='color:#00ff41; font-size:19px; line-height:1.6; letter-spacing:0.5px; white-space: pre-wrap;'>{}</div>
                                                </div>
                                            </div>
                                        </div>", safe_pipe_data);
                                
                                        let _ = p_i.send_event(UserEvent::WasmSurfRender { 
                                            html: success_html, 
                                            url: "isobrowse://sandbox/pipeline".to_string(), 
                                            cpu_ms: start_time.elapsed().as_millis(), 
                                            ram_kb: pipe_data.len() / 1024, 
                                            blocked_count: 0 
                                        });
                                    }
                                }
                                
                                else if cmd.starts_with("/rhai ") {
                                    let script = cmd.strip_prefix("/rhai ").unwrap_or("").trim();
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [RHAI ENGINE]: Executing sandbox script...")));

                                    let engine = rhai::Engine::new();
                                    let mut scope = rhai::Scope::new();
                                    
                                    if index > 0 && !pipe_data.is_empty() {
                                        scope.push("pipe_data", pipe_data.clone());
                                    }

                                    let step_time = Instant::now();

                                    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script) {
                                        Ok(result) => {
                                            pipe_data = result.to_string();
                                            if is_last {
                                                let success_html = format!("
                                                    <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                                        <h1 style='color:#ffcc00; text-shadow: 0 0 10px #ffcc0055;'>⚡ Rhai Native Execution Complete</h1>
                                                        <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #ffcc0011;'>
                                                            <p style='color:#888;'>Engine: <span style='color:#fff;'>Rhai Embedded Sandbox</span></p>
                                                            <p style='color:#888;'>Execution Time: <span style='color:#ffcc00;'>{} ms</span></p>
                                                            <hr style='border:1px dashed #333; margin:15px 0;'>
                                                            <div style='background:#000; border:1px solid #ffcc00; padding:20px; margin-top:10px; max-height: 400px; overflow-y: auto;'>
                                                                <span style='color:#888; font-size:10px;'>[INPUT SCRIPT]</span><br>
                                                                <span style='color:#fff; font-size:16px; line-height:1.6;'>{}</span><br><br>
                                                                <div style='display:flex; justify-content:space-between; align-items:center; border-bottom:1px dashed #333; padding-bottom:10px; margin-bottom:10px; margin-top:15px;'>
                                                                    <span style='color:#888; font-size:10px;'>[PIPELINE OUTPUT TERMINAL]</span>
                                                                    <button onclick='let val=this.parentElement.nextElementSibling.innerText; try{{navigator.clipboard.writeText(val);}}catch(e){{let ta=document.createElement(`textarea`);ta.value=val;document.body.appendChild(ta);ta.select();document.execCommand(`copy`);ta.remove();}} let oldBg=this.style.background; this.style.background=`#ffcc00`; this.style.color=`#000`; this.innerText=`[ COPIED! ]`; setTimeout(()=>{{this.style.background=oldBg; this.style.color=`#ffcc00`; this.innerText=`[ COPY DATA ]`;}}, 800);' style='background:#332b00; color:#ffcc00; border:1px solid #ffcc00; cursor:pointer; padding:4px 10px; font-family:monospace; font-weight:bold; outline:none; transition:0.2s;'>[ COPY DATA ]</button>
                                                                </div>
                                                                <div style='color:#ffcc00; font-size:22px; font-weight:bold; letter-spacing:1px; white-space: pre-wrap;'>{}</div>
                                                            </div>
                                                        </div>
                                                    </div>
                                                ", step_time.elapsed().as_millis(), script, pipe_data);
                                                let _ = p_i.send_event(UserEvent::WasmSurfRender { html: success_html, url: "isobrowse://sandbox/rhai".to_string(), cpu_ms: start_time.elapsed().as_millis(), ram_kb: 12, blocked_count: 0 });
                                            }
                                        },
                                        Err(e) => {
                                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [RHAI ERROR]: {}", e)));
                                            let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>{}</p></div>", e);
                                            let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                            break;
                                        }
                                    }
                                } else if cmd.starts_with("/run ") || cmd.starts_with("/fetch ") {
                                    
                                    let prefix = if cmd.starts_with("/run ") { "/run " } else { "/fetch " };
                                    let raw_args = cmd.strip_prefix(prefix).unwrap_or("").trim();
                                    
                                    let mut parsed_tokens = Vec::new();
                                    let mut current_arg = String::new();
                                    let mut in_quotes = false;
                                    
                                    for c in raw_args.chars() {
                                        if c == '"' || c == '\'' {
                                            in_quotes = !in_quotes; 
                                        } else if c.is_whitespace() && !in_quotes {
                                            if !current_arg.is_empty() {
                                                parsed_tokens.push(current_arg.clone());
                                                current_arg.clear();
                                            }
                                        } else {
                                            current_arg.push(c);
                                        }
                                    }
                                    if !current_arg.is_empty() {
                                        parsed_tokens.push(current_arg);
                                    }
                                    
                                    let mut wasm_url = parsed_tokens.get(0).unwrap_or(&"".to_string()).clone();

                                    let original_name = wasm_url.clone();
                                    if !wasm_url.is_empty() && !wasm_url.starts_with("http") && !wasm_url.starts_with('/') && !wasm_url.starts_with('.') && !wasm_url.starts_with('~') && !wasm_url.contains(":\\") && !wasm_url.ends_with(".wasm") {
                                        wasm_url = format!("https://raw.githubusercontent.com/igtumt/isomodules/main/{}.wasm", wasm_url);
                                        let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [CATALOG]: Alias '{}' detected. Routing to official repository...", original_name)));
                                    }

                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [WORKER ENGINE]: Running worker from {}...", wasm_url)));

                                    let download_result = if wasm_url.starts_with("http") {
                                        match client.get(&wasm_url).send() {
                                            Ok(resp) => {
                                                if resp.status().is_success() {
                                                    resp.bytes().map(|b| b.to_vec()).map_err(|e| e.to_string())
                                                } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
                                                    Err(format!("Module not found (404). Please verify the name: '{}'", parsed_tokens.get(0).unwrap_or(&"".to_string())))
                                                } else {
                                                    Err(format!("HTTP Error: {}", resp.status()))
                                                }
                                            },
                                            Err(e) => Err(e.to_string())
                                        }
                                    } else {
                                        std::fs::read(&wasm_url).map_err(|e| e.to_string())
                                    };



                                    match download_result {
                                        Ok(wasm_bytes) => {
                                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [WASM]: {} bytes loaded. Compiling VM sandbox...", wasm_bytes.len())));
                                            final_ram_kb += wasm_bytes.len() / 1024;

                                            let mut config = wasmtime::Config::new();
                                            config.consume_fuel(true);

                                            if let Ok(engine) = wasmtime::Engine::new(&config) {
                                                let mut linker = wasmtime::Linker::<WasiP1Ctx>::new(&engine);
                                                let _ = preview1::add_to_linker_sync(&mut linker, |t| t);

                                                let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
                                                let stdout_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024);
                                                builder.stdout(stdout_pipe.clone());

                                                let pipe_bytes = if index > 0 && !pipe_data.is_empty() { pipe_data.clone().into_bytes() } else { Vec::new() };
                                                let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(pipe_bytes);
                                                builder.stdin(stdin_pipe);

                                                let mut app_args = vec!["isobrowse_worker.wasm".to_string()];
                                                if parsed_tokens.len() > 1 {
                                                    app_args.extend_from_slice(&parsed_tokens[1..]);
                                                }
                                                let _ = builder.args(&app_args);

                                                let model_path = format!("{}/.isobrowse_models", std::env::var("HOME").unwrap_or_default());
                                                let _ = std::fs::create_dir_all(&model_path);
                                                let _ = builder.preopened_dir(&model_path, "/models", wasmtime_wasi::DirPerms::all(), wasmtime_wasi::FilePerms::all());

                                                let wasi = builder.build_p1();
                                                let mut store = wasmtime::Store::new(&engine, wasi);
                                                let _ = store.set_fuel(u64::MAX);

                                                match wasmtime::Module::new(&engine, &wasm_bytes) {
                                                    Ok(module) => {
                                                        if let Ok(instance) = linker.instantiate(&mut store, &module) {
                                                            let start_func = instance.get_typed_func::<(), ()>(&mut store, "_start")
                                                                .unwrap_or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "").unwrap());

                                                            let _ = start_func.call(&mut store, ());

                                                            let output_bytes = stdout_pipe.contents();
                                                            let mut wasm_output = String::from_utf8_lossy(&output_bytes).to_string();

                                                            wasm_output = wasm_output.trim().to_string();
                                                            if wasm_output.is_empty() { wasm_output = "[No Output Generated]".to_string(); }
                                                            pipe_data = wasm_output;

                                                            if is_last {
                                                            
                                                                let is_chart = pipe_data.contains("\"iso_chart\": true");

                                                                let success_html = if is_chart {
                                                                    format!("
                                                                    <html><head>
                                                                        <script src='https://cdn.jsdelivr.net/npm/chart.js'></script>
                                                                        <style>
                                                                            body {{ background:#050505; color:#fff; font-family:monospace; display:flex; flex-direction:column; align-items:center; justify-content:center; margin:0; padding: 20px; height: 100vh; box-sizing: border-box; }}
                                                                        </style>
                                                                    </head><body>
                                                                        <h1 id='chartTitle' style='color:#00ccff; text-shadow: 0 0 10px #00ccff55; margin-bottom: 20px;'>⚡ Analyzing Data...</h1>
                                                                        <div style='background:#111; border:1px solid #333; padding:20px; width:100%; max-width:900px; height:60vh; min-height: 400px; box-shadow: 0 0 15px rgba(255,255,255,0.05); position:relative; border-radius: 8px;'>
                                                                            <canvas id='myChart'></canvas>
                                                                        </div>
                                                                        <div id='debug' style='color: #ff3366; margin-top: 10px; font-size: 12px;'></div>
                                                                        
                                                                        <script id='chart-data' type='application/json'>{}</script>
                                                                        
                                                                        <script>
                                                                            try {{
                                                                                const rawText = document.getElementById('chart-data').textContent;
                                                                                const wsData = JSON.parse(rawText);
                                                                                
                                                                                document.getElementById('chartTitle').innerText = '⚡ ' + (wsData.title || 'Analysis Complete');
                                                                                const ctx = document.getElementById('myChart');
                                                                                new Chart(ctx, {{
                                                                                    type: wsData.chart_type || 'bar',
                                                                                    data: {{
                                                                                        labels: wsData.labels,
                                                                                        datasets: [{{
                                                                                            label: wsData.dataset_name || 'Data Set',
                                                                                            data: wsData.data,
                                                                                            backgroundColor: wsData.color_main || 'rgba(0, 255, 65, 0.5)',
                                                                                            borderColor: wsData.color_border || '#00ff41',
                                                                                            borderWidth: 2,
                                                                                            borderRadius: 5
                                                                                        }}]
                                                                                    }},
                                                                                    options: {{
                                                                                        responsive: true,
                                                                                        maintainAspectRatio: false,
                                                                                        scales: {{ y: {{ beginAtZero: true, ticks: {{ color: '#aaa' }} }}, x: {{ ticks: {{ color: '#aaa' }} }} }},
                                                                                        plugins: {{ legend: {{ labels: {{ color: '#fff' }} }}, title: {{ display: true, text: wsData.title, color: '#00ccff', font: {{ size: 18 }} }} }}
                                                                                    }}
                                                                                }});
                                                                            }} catch(e) {{ document.getElementById('debug').innerText = 'Chart Parse Error: ' + e; }}
                                                                        </script>
                                                                    </body></html>
                                                                    ", pipe_data)
                                                                } else {
                                                                    let safe_pipe_data = pipe_data.replace("<", "&lt;").replace(">", "&gt;");

                                                                    format!("
                                                                    <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; min-height:100%; background:#050505; color:#00ff41; font-family:monospace; text-align:center;'>
                                                                        <h1 style='color:#00ccff; text-shadow: 0 0 10px #00ccff55;'>⚡ Sandbox Execution Complete</h1>
                                                                        <div style='background:#111; border:1px solid #333; padding:20px; text-align:left; max-width:800px; margin-top:20px; width: 90%; box-shadow: 0 0 15px #00ff4111;'>
                                                                            <div style='background:#000; border:1px solid #00ff41; padding:20px; max-height: 400px; overflow-y: auto;'>
                                                                                <div style='display:flex; justify-content:space-between; align-items:center; border-bottom:1px dashed #333; padding-bottom:10px; margin-bottom:10px;'>
                                                                                    <span style='color:#888; font-size:10px;'>[PIPELINE OUTPUT TERMINAL]</span>
                                                                                    <button onclick='let val=this.parentElement.nextElementSibling.innerText; try{{navigator.clipboard.writeText(val);}}catch(e){{let ta=document.createElement(`textarea`);ta.value=val;document.body.appendChild(ta);ta.select();document.execCommand(`copy`);ta.remove();}} let oldBg=this.style.background; this.style.background=`#00ff41`; this.style.color=`#000`; this.innerText=`[ COPIED! ]`; setTimeout(()=>{{this.style.background=oldBg; this.style.color=`#00ff41`; this.innerText=`[ COPY DATA ]`;}}, 800);' style='background:#003300; color:#00ff41; border:1px solid #00ff41; cursor:pointer; padding:4px 10px; font-family:monospace; font-weight:bold; outline:none; transition:0.2s;'>[ COPY DATA ]</button>
                                                                                </div>
                                                                                <div style='color:#00ff41; font-size:19px; line-height:1.6; letter-spacing:0.5px; white-space: pre-wrap;'>{}</div>
                                                                            </div>
                                                                        </div>
                                                                    </div>", safe_pipe_data)

                                                                };

                                                                let target_url = if is_chart { "isobrowse://sandbox/chart".to_string() } else { "isobrowse://sandbox/pipeline".to_string() };
                                                                let _ = p_i.send_event(UserEvent::WasmSurfRender { html: success_html, url: target_url, cpu_ms: start_time.elapsed().as_millis(), ram_kb: final_ram_kb, blocked_count: 0 });

                                                            }
                                                        }
                                                    },
                                                    Err(e) => {
                                                        let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: WASM Compilation failed: {}", e)));
                                                        let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>WASM Engine Compilation Failed:<br>{}</p></div>", e);
                                                        let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                                        break;
                                                    }
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Worker Download failed: {}", e)));
                                            let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 PIPELINE ERROR</h2><p>Download Failed (Are you offline or is the link broken?):<br>{}</p></div>", e);
                                            let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                            break;
                                        }
                                    }
                                } else {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Invalid command. Use /nojs, /run, /get, etc.")));
                                    let err_html = format!("<div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'><h2>🚨 COMMAND NOT FOUND</h2><p>Terminal did not understand the input: {}<br>Use <b>/help</b> to see available pipeline commands.</p></div>", cmd);
                                    let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                                    break;
                                }
                            }
                        } else {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Command not recognized. Please check the Examples menu.")));
                            
                            // Güvenliği ihlal etmemek için kullanıcının girdiği rastgele metni HTML taglerinden temizliyoruz
                            let safe_raw_input = raw_input.replace("<", "&lt;").replace(">", "&gt;");
                            
                            let err_html = format!("
                            <div style='color:#ff3366; font-family:monospace; text-align:center; padding:50px;'>
                                <h2>🚨 COMMAND NOT FOUND</h2>
                                <p style='color:#aaa; font-size:16px;'>IsoBrowse did not understand the input: <span style='color:#fff; background:#222; padding:2px 6px; border-radius:4px;'>{}</span></p>
                                
                                <div style='background:#111; border:1px dashed #ffcc00; padding:25px; text-align:left; max-width:650px; margin:30px auto; color:#fff; border-radius:8px; box-shadow: 0 0 20px rgba(255,204,0,0.05);'>
                                    <div style='color:#ffcc00; font-weight:bold; font-size:18px; margin-bottom:15px;'>💡 QUICK TIPS</div>
                                    <p style='color:#aaa; margin-bottom: 20px;'>Please check the <b>[ 💡 ] EXAMPLES</b> button in the terminal to see how pipelines work.</p>
                                    
                                    <div style='margin-bottom:15px;'>
                                        <span style='color:#00ff41; font-weight:bold;'>📂 Secure Local File Read:</span><br>
                                        <code style='color:#888; background:#000; padding:4px 8px; border-radius:4px; display:inline-block; margin-top:5px; border:1px solid #333;'>/read ~/Desktop/server.log | /run head 10</code>
                                    </div>
                                    
                                    <div>
                                        <span style='color:#00ccff; font-weight:bold;'>🌐 Safe Web Browsing (Zero-Trust):</span><br>
                                        <code style='color:#888; background:#000; padding:4px 8px; border-radius:4px; display:inline-block; margin-top:5px; border:1px solid #333;'>/nojs news.ycombinator.com</code>
                                    </div>
                                </div>
                            </div>", safe_raw_input);
                            
                            let _ = p_i.send_event(UserEvent::WasmSurfRender { html: err_html, url: "isobrowse://sandbox/error".to_string(), cpu_ms: 0, ram_kb: 0, blocked_count: 0 });
                        }
                        
                    });
                }
            }

            Event::UserEvent(UserEvent::WasmSurfRender { html, url, cpu_ms, ram_kb, blocked_count }) => {
                
                let fallback_css = "<style>
                    .ad, .ads, .ad-slot, .ad-container, [id^='ad-'], [class^='ad-'],
                    [class*='taboola'], [class*='outbrain'],
                    [class*='popup'], [id*='popup'], [class*='modal'], [id*='modal'],
                    [class*='overlay'], [id*='overlay'], [class*='cookie'], [id*='cookie'],
                    [class*='consent'], [id*='consent'], [class*='newsletter'], [id*='newsletter'],
                    .fc-consent-root, #cmpbox,
                    .sp_veil, [id^='sp_message'], .fc-ab-root, .privacy-prompt, #privacy-prompt,
                    .veil, .backdrop, .dialog-backdrop, [class*='backdrop'] {
                        display: none !important; visibility: hidden !important; opacity: 0 !important;
                    }
                    html, body { overflow: auto !important; position: static !important; }
                    template, style, script, title, link, meta { display: none !important; opacity: 0 !important; visibility: hidden !important; }
                    .iso-noscript { display: block !important; opacity: 1 !important; visibility: visible !important; }
                </style>";

                // İŞTE YENİ ZIRHIMIZ: Web sitesinin içine gizlice sızan ajan script!
                let safe_script = r#"<script>
                document.addEventListener('keydown', function(e) {
                    let activeEl = document.activeElement;
                    let isInput = (activeEl && (activeEl.tagName === 'TEXTAREA' || activeEl.tagName === 'INPUT'));
                    
                    // ZIRH 1: Ok Tuşları Koruması
                    if (isInput && (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
                        if (e.key === 'ArrowLeft' && activeEl.selectionStart === 0) { e.preventDefault(); return; }
                        if (e.key === 'ArrowRight' && activeEl.selectionStart === activeEl.value.length) { e.preventDefault(); return; }
                    }

                    // ZIRH 2: Harf Tuşları Koruması
                    if (!isInput) {
                        if (e.key === 'Backspace' || (e.key.length === 1 && e.key !== ' ' && !e.metaKey && !e.ctrlKey)) {
                            e.preventDefault();
                            return;
                        }
                    }
                    
                    // 🛡️ ZIRH 3: MACOS KOPYALA ÇÖKME ENGELLEYİCİSİ (Iframe İçi)
                    if (e.metaKey || e.ctrlKey) {
                        let k = e.key.toLowerCase();
                        if (k === 'c' || k === 'v' || k === 'x' || k === 'a') {
                            e.preventDefault(); // macOS, bu tuşları GÖREMEZSİN!
                            
                            if (k === 'c') {
                                let text = isInput ? activeEl.value.substring(activeEl.selectionStart, activeEl.selectionEnd) : window.getSelection().toString();
                                if (text) {
                                    navigator.clipboard.writeText(text).catch(() => {
                                        // Fallback Copy
                                        let ta = document.createElement('textarea'); ta.value = text;
                                        document.body.appendChild(ta); ta.select(); document.execCommand('copy'); ta.remove();
                                    });
                                }
                            } else if (k === 'a') {
                                if (isInput) activeEl.select();
                                else {
                                    let s = window.getSelection(); let r = document.createRange();
                                    r.selectNodeContents(document.body); s.removeAllRanges(); s.addRange(r);
                                }
                            }
                        }
                    }
                }, { capture: true, passive: false });
            </script>"#;



                
                let base_tag = format!("<base href=\"{}\" target=\"_self\">", url);
                
                // Zırhı, HTML kodunun arasına sıkıştırıyoruz
                let final_srcdoc = format!("{}\n{}\n{}\n{}", base_tag, fallback_css, safe_script, html);
                
                let js = format!("window.renderSurfMode({}, '{}', {}, {}, {})", 
                    serde_json::to_string(&final_srcdoc).unwrap_or_else(|_| "\"\"".to_string()), url, cpu_ms, ram_kb, blocked_count);
                let _ = webview.evaluate_script(&js);
            }

            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
