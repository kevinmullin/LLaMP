//! 8d — opt-in and the network grant happen before any socket.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use llamp_plugin_api::{LyricQuery, LyricsProvider, Manifest};
use llamp_plugin_host::lyrics::{compile_lrclib_wasm, HostHttp, HttpGet, WasmLyrics};

fn manifest() -> Manifest {
    Manifest::parse(include_str!("../../../plugins/llamp-lyrics-lrclib/llamp-plugin.json")).expect("manifest")
}

struct MockHttp {
    hits: AtomicUsize,
    body: String,
}

impl HttpGet for MockHttp {
    fn get(&self, host: &str, path: &str, query: &str) -> Result<String, String> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        assert_eq!(host, "lrclib.net");
        assert_eq!(path, "/api/get");
        assert!(query.contains("track_name="), "{query}");
        assert!(query.contains("artist_name="), "{query}");
        assert!(query.contains("album_name="), "{query}");
        assert!(query.contains("duration="), "{query}");
        Ok(self.body.clone())
    }
}

fn query() -> LyricQuery {
    LyricQuery {
        artist: "Artist".into(),
        title: "Title".into(),
        album: "Album".into(),
        duration_seconds: 200,
    }
}

#[test]
fn lookup_is_denied_before_opt_in() {
    let http = HostHttp {
        manifest: manifest(),
        opted_in: false,
        inner: Arc::new(MockHttp {
            hits: AtomicUsize::new(0),
            body: String::new(),
        }),
    };
    let err = http
        .get("lrclib.net", "/api/get", "track_name=Title")
        .expect_err("opt-in");
    assert!(err.contains("opt-in"), "{err}");
}

#[test]
fn undeclared_host_is_denied_before_the_client() {
    let mock = Arc::new(MockHttp {
        hits: AtomicUsize::new(0),
        body: String::new(),
    });
    let http = HostHttp {
        manifest: manifest(),
        opted_in: true,
        inner: Arc::clone(&mock) as Arc<dyn HttpGet>,
    };
    let err = http.get("evil.example", "/api/get", "q=1").expect_err("host");
    assert!(err.contains("undeclared"), "{err}");
    assert_eq!(mock.hits.load(Ordering::SeqCst), 0);
}

#[test]
fn wasm_provider_parses_lrclib_through_host_http() {
    let wasm = compile_lrclib_wasm().expect("compile lrclib wasm");
    let mock = Arc::new(MockHttp {
        hits: AtomicUsize::new(0),
        body: r#"{"syncedLyrics":"[00:01.00]From LRCLIB\n","plainLyrics":"plain","instrumental":false}"#.into(),
    });
    let provider = WasmLyrics {
        http: HostHttp {
            manifest: manifest(),
            opted_in: true,
            inner: Arc::clone(&mock) as Arc<dyn HttpGet>,
        },
        wasm,
    };
    let body = provider.fetch(&query()).expect("fetch");
    assert!(body.contains("From LRCLIB"), "{body}");
    assert_eq!(mock.hits.load(Ordering::SeqCst), 1);
}
