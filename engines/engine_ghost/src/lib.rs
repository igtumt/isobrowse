use std::slice;
use std::str;

extern "C" {
    fn send_to_ui(ptr: *const u8, len: usize);
    fn render_html(ptr: *const u8, len: usize);
}

#[no_mangle]
pub extern "C" fn on_data_received(ptr: *mut u8, len: usize) {
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    let raw_html = str::from_utf8(slice).unwrap_or("");
    
    let script_count = raw_html.to_lowercase().matches("<script").count();
    let iframe_count = raw_html.to_lowercase().matches("<iframe").count();
    
    let ghost_html = raw_html
        .replace("<script", "<script type='application/ghost' style='display:none;'")
        .replace("onclick", "data-blocked-click")
        .replace("target=\"_blank\"", "")
        .replace("target='_blank'", "")
        .replace("<iframe", "<iframe sandbox='' style='opacity: 0.3; pointer-events: none;' ");

    let safe_css = r#"
        <style>
            .ad, .advertisement, [id*="ad-"], [class*="ad-"], [id*="banner"], [class*="banner"] { display: none !important; }
            body { margin: 0; padding: 0; background-color: #fff; }
        </style>
    "#;

    let interceptor_js = r#"
        <script>
            document.addEventListener('click', function(e) {
                const target = e.target.closest('a');
                if (target && target.href) {
                    e.preventDefault();
                    e.stopPropagation();
                    window.parent.postMessage({ type: 'NAVIGATE', url: target.href }, '*');
                }
            }, true);
            
            document.addEventListener('submit', function(e) {
                e.preventDefault();
                e.stopPropagation();
            }, true);
        </script>
    "#;

    // Sağ alt köşedeki şık Mod 1 (Ghost Mode) rozeti
    let info_badge = r#"
        <div id="iso-warning-badge" style="position: fixed; bottom: 50px; right: 20px; background: rgba(0, 20, 0, 0.95); color: #00ff41; border: 1px solid #00ff41; padding: 12px 18px; font-family: monospace; font-size: 11px; border-radius: 4px; z-index: 2147483647; box-shadow: 0 0 10px rgba(0, 255, 65, 0.2);">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                <strong style="font-size: 13px;">[!] GHOST MODE AKTİF</strong>
                <button onclick="document.getElementById('iso-warning-badge').style.display='none'" style="background: none; border: none; color: #00ff41; cursor: pointer; font-weight: bold; font-size: 14px; margin-left: 20px;">X</button>
            </div>
            <span style="color: #aaa; line-height: 1.5;">JS ve İframe'ler kilitlendi.<br>Site boş veya hatalıysa, bu site<br>Mod 2'ye (Standart) ihtiyaç duyar.</span>
        </div>
    "#;
    
    let final_html = format!("{}{}{}{}", safe_css, ghost_html, interceptor_js, info_badge);

    let log_msg = format!("GHOST_MODE // {} script, {} iframe kilitlendi.", script_count, iframe_count);
    unsafe { send_to_ui(log_msg.as_ptr(), log_msg.len()); }

    unsafe { render_html(final_html.as_ptr(), final_html.len()); }
}

#[no_mangle] pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle] pub extern "C" fn set_mode(_ptr: i32, _len: i32) {}
