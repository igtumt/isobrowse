use std::slice;
use std::str;

unsafe extern "C" {
    fn send_to_ui(ptr: *const u8, len: usize);
    fn render_html(ptr: *const u8, len: usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn on_data_received(ptr: *mut u8, len: usize) {
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    let raw_html = str::from_utf8(slice).unwrap_or("");
    
    let standard_html = raw_html
        .replace("target=\"_blank\"", "target=\"_self\"")
        .replace("target='_blank'", "target='_self'");

    // YENİ: Agresif Mod 2 Ad-Blocker CSS
    // Reklam iframelerinin tıklanmasını ve görünmesini tamamen imkansız hale getirir.
    let adblock_css = r#"
        <style>
            iframe[src*="doubleclick"], iframe[src*="ads"], iframe[src*="syndication"], iframe[src*="taboola"],
            .ad, .ads, .advertisement, [id*="google_ads"], [class*="banner"], [class*="ad-container"] {
                display: none !important;
                pointer-events: none !important;
                width: 0 !important;
                height: 0 !important;
                opacity: 0 !important;
            }
        </style>
    "#;

    let interceptor_js = r#"
        <script>
            window.open = function(url) {
                if (url && url.startsWith('http')) {
                    window.parent.postMessage({ type: 'NAVIGATE', url: url }, '*');
                }
                return null;
            };
        </script>
    "#;

    let info_badge = r#"
        <div id="iso-standard-badge" style="position: fixed; bottom: 50px; right: 20px; background: rgba(0, 20, 50, 0.95); color: #00ccff; border: 1px solid #00ccff; padding: 12px 18px; font-family: monospace; font-size: 11px; border-radius: 4px; z-index: 2147483647; box-shadow: 0 0 10px rgba(0, 204, 255, 0.2);">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                <strong style="font-size: 13px;">[*] STANDARD MODE (MOD 2)</strong>
                <button onclick="document.getElementById('iso-standard-badge').style.display='none'" style="background: none; border: none; color: #00ccff; cursor: pointer; font-weight: bold; font-size: 14px; margin-left: 20px;">X</button>
            </div>
            <span style="color: #aaa; line-height: 1.5;">Dinamik JS Kum Havuzu Aktif.<br>İşlem ve formlar kullanılabilir.</span>
        </div>
    "#;
    
    // CSS HTML'e dahil edildi
    let final_html = format!("{}{}{}{}", adblock_css, standard_html, interceptor_js, info_badge);

    let log_msg = "STANDARD_MODE // Agresif AdBlocker ve Host Kalkanı Aktif.".to_string();
    unsafe { send_to_ui(log_msg.as_ptr(), log_msg.len()); }

    unsafe { render_html(final_html.as_ptr(), final_html.len()); }
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn set_mode(_ptr: i32, _len: i32) {}
