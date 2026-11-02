//! tool_shim: forwards values between the page and `tool_core::game`.
//!
//! Builder requirement: the shim decides nothing. Each function is one call
//! into the core. `crates/core/tests/boundary.rs` fails if a branch, a loop or
//! the `?` operator appears here; if a feature seems to need one, the rule
//! belongs in the core.
//!
//! Times arrive from the page as the browser gives them (seconds of audio
//! context time, milliseconds of performance time) and leave as whole
//! microseconds: one multiplication and a cast each, a conversion and not a
//! decision (kits/realtime).

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Game {
    inner: tool_core::game::Game,
}

#[wasm_bindgen]
pub fn draw_assets() -> String {
    tool_core::draw::assets()
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(strings_json: &str, glossary_md: &str, index_json: &str) -> Result<Game, JsValue> {
        tool_core::game::Game::new(strings_json, glossary_md, index_json)
            .map(|inner| Game { inner })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// `{"ok": true}` or `{"error": "..."}`, then the page asks for `ui()`.
    pub fn act(&mut self, name: &str, value: &str) -> String {
        tool_core::game::result_json(self.inner.act(name, value))
    }

    pub fn ui(&self) -> String {
        self.inner.ui_json()
    }

    pub fn commands(&mut self) -> String {
        self.inner.commands()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn input(&mut self, kind: u8, button: u8, x: f64, y: f64, w: f64, h: f64, ctx_s: f64, perf_ms: f64, stamp_ms: f64) {
        self.inner.input(
            kind,
            button,
            x as i32,
            y as i32,
            w as i32,
            h as i32,
            (ctx_s * 1_000_000.0) as i64,
            (perf_ms * 1000.0) as i64,
            (stamp_ms * 1000.0) as i64,
        )
    }

    pub fn frame(&mut self, ctx_s: f64, perf_ms: f64, now_ms: f64) -> Vec<i32> {
        self.inner.frame((ctx_s * 1_000_000.0) as i64, (perf_ms * 1000.0) as i64, (now_ms * 1000.0) as i64)
    }

    pub fn tile(&mut self, chart_json: &str, at_ms: f64) -> String {
        tool_core::game::result_json(self.inner.tile(chart_json, at_ms as i64))
    }

    pub fn draw(&self) -> Vec<i32> {
        self.inner.draw()
    }

    pub fn judged(&self) -> u32 {
        self.inner.judged()
    }

    pub fn kept(&self) -> String {
        self.inner.kept_json()
    }

    pub fn restore(&mut self, kept_json: &str) {
        self.inner.restore(kept_json)
    }

    pub fn click_wav(&self) -> Vec<u8> {
        tool_core::sound::click_wav()
    }
}
