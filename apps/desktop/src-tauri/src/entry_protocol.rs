//! The `odentry` URI scheme: every Entry document and every resource it loads.
//!
//! Each Dictionary gets its own path prefix, `<base>/<dictionary key>/`. Its Entry for a
//! headword is the document at that prefix (`?headword=…`); its resources sit beneath it,
//! so the relative paths in Entry HTML (`img/a.png`, `style.css`) resolve without rewriting.
//! Every response carries a content security policy that sandboxes the document, keeps it
//! to its own Dictionary's prefix and, unless the reader allowed it for that Dictionary,
//! off the network.

use tauri::http::{Method, Request, Response, StatusCode, header};

use crate::state::AppState;

/// The scheme name. On Windows WebView2 serves it as `http://odentry.localhost/`.
pub const SCHEME: &str = "odentry";

/// Where the scheme's URLs start, as the page sees them.
pub fn base_url() -> &'static str {
    if cfg!(windows) {
        "http://odentry.localhost/"
    } else {
        "odentry://localhost/"
    }
}

/// The URL of a Dictionary's Entry document for `headword`.
pub fn entry_url(dictionary_id: &str, headword: &str) -> String {
    format!(
        "{}?headword={}",
        dictionary_base(dictionary_id),
        percent_encoding::utf8_percent_encode(headword, percent_encoding::NON_ALPHANUMERIC)
    )
}

/// Answers one request made to the scheme.
pub fn respond(state: &AppState, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = request.uri().path().trim_start_matches('/');
    let Some((key, rest)) = path.split_once('/') else {
        return plain(StatusCode::NOT_FOUND, &content_policy("", false));
    };
    let Some(id) = dictionary_id(key) else {
        return plain(StatusCode::NOT_FOUND, &content_policy("", false));
    };
    let policy = content_policy(&id, state.allows_network(&id));
    if request.method() != Method::GET {
        return plain(StatusCode::METHOD_NOT_ALLOWED, &policy);
    }

    if rest.is_empty() {
        let headword = query_value(request.uri().query().unwrap_or(""), "headword");
        return match state.entry_html(&id, &headword) {
            Ok(Some(html)) => Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-store")
                .headers(&policy)
                .body(entry_document(&html, state.appearance().text_size).into_bytes())
                .expect("valid response"),
            Ok(None) => plain(StatusCode::NOT_FOUND, &policy),
            Err(_) => plain(StatusCode::INTERNAL_SERVER_ERROR, &policy),
        };
    }

    let Ok(resource_path) = percent_encoding::percent_decode_str(rest).decode_utf8() else {
        return plain(StatusCode::NOT_FOUND, &policy);
    };
    match state.resource(&id, &resource_path) {
        Ok(Some(bytes)) => Response::builder()
            .header(header::CONTENT_TYPE, content_type(&resource_path))
            // Fonts (and module scripts) load in CORS mode, and the Entry frame's origin is
            // opaque. No frame can read these with fetch: connect-src never allows the scheme.
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .headers(&policy)
            .body(bytes)
            .expect("valid response"),
        Ok(None) => plain(StatusCode::NOT_FOUND, &policy),
        Err(_) => plain(StatusCode::INTERNAL_SERVER_ERROR, &policy),
    }
}

/// The answer before the Library has been loaded.
pub fn not_ready() -> Response<Vec<u8>> {
    plain(StatusCode::SERVICE_UNAVAILABLE, &content_policy("", false))
}

/// Sets the given headers on a response builder.
trait WithHeaders {
    fn headers(self, headers: &[(header::HeaderName, String)]) -> Self;
}

impl WithHeaders for tauri::http::response::Builder {
    fn headers(mut self, headers: &[(header::HeaderName, String)]) -> Self {
        for (name, value) in headers {
            self = self.header(name, value);
        }
        self
    }
}

/// The headers every response from a Dictionary's prefix carries, document or not: a
/// resource opened as a document must be just as boxed in as an Entry.
fn content_policy(dictionary_id: &str, allow_network: bool) -> Vec<(header::HeaderName, String)> {
    let own = dictionary_base(dictionary_id);
    let network = if allow_network { " https: http:" } else { "" };
    let connect = if allow_network {
        "https: wss:"
    } else {
        "'none'"
    };
    let csp = format!(
        "sandbox allow-scripts; default-src 'none'; \
         script-src {own} 'unsafe-inline'{network}; style-src {own} 'unsafe-inline'{network}; \
         img-src {own} data:{network}; media-src {own} data:{network}; font-src {own} data:{network}; \
         connect-src {connect}; frame-src 'none'; child-src 'none'; worker-src 'none'; \
         object-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'"
    );
    vec![
        (header::CONTENT_SECURITY_POLICY, csp),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        (header::REFERRER_POLICY, "no-referrer".to_owned()),
        (header::X_DNS_PREFETCH_CONTROL, "off".to_owned()),
    ]
}

/// Reports the document's height to the app so the frame can fit its content, hands the
/// app the reader's clicks on `entry://` and `sound://` links, which the frame cannot
/// follow itself, passes Esc on (the Mini Window closes on it) and reports shortcut keys
/// (below). The app accepts only these five message shapes, only from the frame it made,
/// and checks every field (see EntryFrame.tsx). A link to an anchor in the same Entry
/// (`entry://#sense2`) stays in the frame. A click the Dictionary's own scripts
/// already handled (`preventDefault`) is left alone.
///
/// Keys pressed in an Entry stay in its document, so the script also reports the keys of
/// the window's shortcuts (Alt + digit, P, ← or →; Ctrl/⌘ + K, L, D or zoom; `/`), and
/// keeps them from the frame. Never while the reader types in a field of the Entry or an
/// IME composes, nor for keys the Dictionary's scripts handled. The app checks each key
/// against its keyboard map before pressing it again (lib/keymap.ts).
///
/// It also takes a new base text size from the app while the Entry is open, so the reader
/// can change it without reloading the Entry. It accepts that only from the app window
/// (another Dictionary's frame is a different `source`), and only a whole number of pixels
/// in range; all it changes is the text-size rule just before the script.
const FRAME_SCRIPT: &str = r#"(()=>{
const post=()=>{const b=document.body;parent.postMessage({opendict:"size",height:Math.ceil(Math.max(document.documentElement.getBoundingClientRect().height,b?b.scrollHeight:0))},"*")};
new ResizeObserver(post).observe(document.documentElement);addEventListener("load",post);
addEventListener("click",e=>{
if(e.defaultPrevented||!(e.target instanceof Element))return;
const a=e.target.closest("a[href]");if(!a)return;
const href=a.getAttribute("href").trim(),m=/^(entry|sound):\/\//i.exec(href);if(!m)return;
e.preventDefault();
const kind=m[1].toLowerCase(),rest=href.slice(m[0].length);
if(kind==="entry"&&rest.charAt(0)==='#'){let id=rest.slice(1);try{id=decodeURIComponent(id)}catch{}
const t=document.getElementById(id)||document.getElementsByName(id)[0];if(t)t.scrollIntoView();return}
parent.postMessage({opendict:kind==="entry"?"link":"sound",href},"*")});
addEventListener("keydown",e=>{
if(e.defaultPrevented||e.isComposing||e.keyCode===229||!e.isTrusted)return;
const t=e.target,typing=t instanceof Element&&(t.isContentEditable||/^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
const alt=e.altKey&&!e.ctrlKey&&!e.metaKey&&!e.shiftKey&&/^(Digit[1-9]|Numpad[1-9]|KeyP|ArrowLeft|ArrowRight)$/.test(e.code);
const mod=(e.ctrlKey||e.metaKey)&&!e.altKey&&(/^[kld=+0-]$/i.test(e.key)||/^Numpad(Add|Subtract|0)$/.test(e.code));
const slash=e.key==="/"&&!e.ctrlKey&&!e.metaKey&&!e.altKey;
if(typing||!(alt||mod||slash))return;
e.preventDefault();
parent.postMessage({opendict:"key",key:e.key,code:e.code,altKey:e.altKey,ctrlKey:e.ctrlKey,metaKey:e.metaKey,shiftKey:e.shiftKey},"*")});
const ts=document.currentScript.previousElementSibling;
addEventListener("message",e=>{const d=e.data;if(e.source!==parent||!d||d.opendict!=="textSize")return;
const n=d.size;if(Number.isInteger(n)&&n>=10&&n<=32)ts.textContent=":where(html){font-size:"+n+"px}"});
addEventListener("keydown",e=>{if(e.key==="Escape"&&!e.isComposing&&!e.defaultPrevented)parent.postMessage({opendict:"escape"},"*")});
})();"#;

/// Wraps Entry HTML in a document of its own. The app sets no type inside it (the
/// Hands-Off Entry Rule): it removes the default page margin and gives the reader's text
/// size as the base size, the way a browser's default font size works. That rule has no
/// specificity and comes before the Entry, so any size the Dictionary sets itself wins.
fn entry_document(html: &str, text_size: u32) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0}}</style>\
         <style>:where(html){{font-size:{text_size}px}}</style>\
         <script>{FRAME_SCRIPT}</script></head><body>{html}</body></html>"
    )
}

fn plain(status: StatusCode, policy: &[(header::HeaderName, String)]) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .headers(policy)
        .body(status.canonical_reason().unwrap_or("").as_bytes().to_vec())
        .expect("valid response")
}

fn dictionary_base(dictionary_id: &str) -> String {
    format!("{}{}/", base_url(), key(dictionary_id))
}

/// A Dictionary id as one URL path segment: lower-case hex of its UTF-8 bytes.
fn key(dictionary_id: &str) -> String {
    dictionary_id.bytes().map(|b| format!("{b:02x}")).collect()
}

fn dictionary_id(key: &str) -> Option<String> {
    if key.is_empty() || !key.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..key.len())
        .step_by(2)
        .map(|i| {
            key.get(i..i + 2)
                .filter(|pair| pair.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect();
    String::from_utf8(bytes?).ok()
}

fn query_value(query: &str, name: &str) -> String {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| {
            let value = value.replace('+', " ");
            percent_encoding::percent_decode_str(&value)
                .decode_utf8_lossy()
                .into_owned()
        })
        .unwrap_or_default()
}

fn content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "spx" => "audio/ogg",
        "wav" => "audio/wav",
        "m4a" | "aac" => "audio/aac",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use opendict_mdict_fixture::{MddBuilder, MdxBuilder};

    use super::*;

    const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\n\0sample";

    struct Fixture {
        _folder: tempfile::TempDir,
        state: AppState,
        /// The Dictionaries' keys.
        illustrated: String,
        plain: String,
    }

    fn fixture() -> Fixture {
        let folder = tempfile::tempdir().unwrap();
        let library = folder.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        MdxBuilder::new()
            .title("Illustrated")
            .entry(
                "cat",
                r#"<img src="img/cat.png"><p>a small cat (sample)</p>"#,
            )
            .entry(
                "meow",
                r#"<a href="sound://meow.mp3">meow</a> <a href="sound://purr.mp3">purr</a>"#,
            )
            .write_to(library.join("illustrated.mdx"))
            .unwrap();
        MddBuilder::new()
            .resource("\\img\\cat.png", PICTURE)
            .write_to(library.join("illustrated.mdd"))
            .unwrap();
        MdxBuilder::new()
            .title("Plain")
            .entry("cat", "<p>cat</p>")
            .write_to(library.join("plain.mdx"))
            .unwrap();
        let state = AppState::load(&folder.path().join("data"), Some(library));
        let id = |title: &str| {
            let view = state.library();
            view.dictionaries
                .into_iter()
                .find(|d| d.title == title)
                .unwrap()
                .id
        };
        let (illustrated, plain) = (id("Illustrated"), id("Plain"));
        Fixture {
            _folder: folder,
            state,
            illustrated,
            plain,
        }
    }

    fn get(state: &AppState, url: &str) -> Response<Vec<u8>> {
        respond(state, &Request::get(url).body(Vec::new()).unwrap())
    }

    fn csp(response: &Response<Vec<u8>>) -> &str {
        response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
    }

    #[test]
    fn serves_an_entry_as_a_sandboxed_document_with_no_network_access() {
        let f = fixture();

        let response = get(&f.state, &entry_url(&f.illustrated, "cat"));

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        let body = String::from_utf8(response.body().clone()).unwrap();
        assert!(body.contains(r#"<img src="img/cat.png"><p>a small cat (sample)</p>"#));
        let policy = csp(&response);
        assert!(policy.starts_with("sandbox allow-scripts;"), "{policy}");
        assert!(policy.contains("connect-src 'none'"), "{policy}");
        assert!(!policy.contains("https:"), "{policy}");
        assert!(policy.contains(&format!("img-src {}", dictionary_base(&f.illustrated))));
    }

    #[test]
    fn serves_a_resource_beneath_its_dictionary() {
        let f = fixture();
        let url = format!("{}img/cat.png", dictionary_base(&f.illustrated));

        let response = get(&f.state, &url);

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.body(), PICTURE);
        assert!(csp(&response).starts_with("sandbox allow-scripts;"));
    }

    #[test]
    fn a_resource_is_served_only_beneath_its_own_dictionary() {
        let f = fixture();

        let elsewhere = get(
            &f.state,
            &format!("{}img/cat.png", dictionary_base(&f.plain)),
        );
        let missing = get(
            &f.state,
            &format!("{}img/dog.png", dictionary_base(&f.illustrated)),
        );
        let unknown = get(
            &f.state,
            &format!("{}img/cat.png", dictionary_base("nope.mdx")),
        );
        let no_entry = get(&f.state, &entry_url(&f.plain, "dog"));

        for response in [elsewhere, missing, unknown, no_entry] {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert!(csp(&response).starts_with("sandbox allow-scripts;"));
        }
    }

    #[test]
    fn allowing_network_access_opens_the_network_for_that_dictionary_only() {
        let f = fixture();

        f.state.set_allow_network(&f.illustrated, true).unwrap();

        let allowed = get(&f.state, &entry_url(&f.illustrated, "cat"));
        assert!(csp(&allowed).contains("connect-src https: wss:"));
        assert!(csp(&allowed).contains("img-src") && csp(&allowed).contains("https: http:"));
        let other = get(&f.state, &entry_url(&f.plain, "cat"));
        assert!(csp(&other).contains("connect-src 'none'"));
    }

    #[test]
    fn an_entry_document_takes_the_readers_text_size_as_its_base_size() {
        let f = fixture();
        f.state.set_text_size(20).unwrap();

        let response = get(&f.state, &entry_url(&f.plain, "cat"));

        let body = String::from_utf8(response.body().clone()).unwrap();
        // Zero specificity, ahead of the Entry: any size the Dictionary sets itself wins.
        assert!(body.contains(":where(html){font-size:20px}"), "{body}");
        assert!(body.find(":where(html)").unwrap() < body.find("<body>").unwrap());
    }

    #[test]
    fn look_up_says_which_dictionaries_show_as_authored_in_the_dark_theme() {
        let f = fixture();
        f.state.set_as_authored(&f.plain, true).unwrap();

        let shown: Vec<_> = f
            .state
            .look_up("cat")
            .unwrap()
            .into_iter()
            .map(|e| (e.dictionary_title, e.as_authored))
            .collect();

        assert_eq!(
            shown,
            [
                ("Illustrated".to_owned(), false),
                ("Plain".to_owned(), true)
            ]
        );
    }

    #[test]
    fn look_up_offers_the_first_audio_of_each_entry_for_pronouncing() {
        let f = fixture();

        let meow = f.state.look_up("meow").unwrap();
        let cat = f.state.look_up("cat").unwrap();

        assert_eq!(meow[0].sound.as_deref(), Some("meow.mp3"));
        assert_eq!(cat[0].sound, None);
    }

    #[test]
    fn look_up_points_each_entry_at_its_document() {
        let f = fixture();
        f.state.set_allow_network(&f.plain, true).unwrap();

        let entries = f.state.look_up("cat").unwrap();

        let shown: Vec<_> = entries
            .iter()
            .map(|e| (e.dictionary_title.as_str(), e.src.as_str(), e.allow_network))
            .collect();
        assert_eq!(
            shown,
            [
                (
                    "Illustrated",
                    entry_url(&f.illustrated, "cat").as_str(),
                    false
                ),
                ("Plain", entry_url(&f.plain, "cat").as_str(), true),
            ]
        );
    }
}
