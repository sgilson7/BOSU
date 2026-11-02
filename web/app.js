// The page draws what the core returns and decides nothing.
//
// The core (crates/core/src/game.rs) says which screen shows, what its
// panels hold, what the canvas draws each frame, and what the page should
// do next: load a chart, decode a song, start it at an audio time, pause.
// This file carries those out, and hands every press and move to the core
// with the browser's own timestamps and the audio clock, so a judgement
// never depends on when a frame ran (BO-2).
import init, { Game, draw_assets } from "./pkg/tool_shim.js";
import { createRenderer } from "./render2d.js";

const el = (id) => document.getElementById(id);
const make = (tag, text) => {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
};

let game = null;
let assets = null;
let renderer = null;
let ui = null;
let audio = null;
let song = null;
let clicks = [];
let clickBuffer = null;
const decoded = new Map();
const pointer = { x: 0, y: 0 };
const tileSpec = new URLSearchParams(location.search).get("tile");

// The audio clock: the context time being heard now, and the performance
// time it was heard at. Before any sound, both are zero.
function clock() {
  if (!audio) return [0, 0];
  const out = audio.ctx.getOutputTimestamp();
  return [out.contextTime, out.performanceTime];
}

// Browsers start audio only after a press, so the context is made on the
// first one.
function ensureAudio() {
  if (audio) return;
  const ctx = new AudioContext({ latencyHint: "interactive" });
  const gain = ctx.createGain();
  gain.connect(ctx.destination);
  audio = { ctx, gain };
}

function act(name, value = "") {
  const result = JSON.parse(game.act(name, value));
  el("message").textContent = result.error || "";
  runCommands();
}

async function loadChart(url) {
  try {
    const response = await fetch(url);
    if (!response.ok) throw new Error(response.statusText);
    act("chart.loaded", await response.text());
  } catch (e) {
    act("load.failed", e.message);
  }
}

async function loadAudio(url) {
  try {
    ensureAudio();
    if (!decoded.has(url)) {
      const response = await fetch(url);
      if (!response.ok) throw new Error(response.statusText);
      decoded.set(url, await audio.ctx.decodeAudioData(await response.arrayBuffer()));
    }
    act("audio.ready", url);
  } catch (e) {
    act("load.failed", e.message);
  }
}

function stopAll() {
  if (song) song.stop();
  song = null;
  clicks.forEach((c) => c.stop());
  clicks = [];
}

function startAudio(url, atUs) {
  stopAll();
  song = audio.ctx.createBufferSource();
  song.buffer = decoded.get(url);
  song.connect(audio.gain);
  song.start(atUs / assets.us_per_s);
}

async function startClicks(c) {
  ensureAudio();
  if (!clickBuffer) clickBuffer = await audio.ctx.decodeAudioData(game.click_wav().buffer);
  stopAll();
  for (let k = 0; k < c.count; k += 1) {
    const click = audio.ctx.createBufferSource();
    click.buffer = clickBuffer;
    click.connect(audio.gain);
    click.start((c.at_us + k * c.every_us) / assets.us_per_s);
    clicks.push(click);
  }
}

const handlers = {
  load_chart: (c) => loadChart(c.url),
  load_audio: (c) => loadAudio(c.url),
  start_audio: (c) => startAudio(c.url, c.at_us),
  stop_audio: () => stopAll(),
  suspend: () => audio && audio.ctx.suspend(),
  resume: () => audio && audio.ctx.resume(),
  clicks: (c) => startClicks(c),
  volume: (c) => {
    ensureAudio();
    audio.gain.gain.value = c.pct / assets.percent;
  },
  ui: () => render(),
};

// Each command is a name, or an object with one key: the name, holding
// its arguments.
function runCommands() {
  for (const command of JSON.parse(game.commands())) {
    if (typeof command === "string") handlers[command]({});
    else for (const [name, args] of Object.entries(command)) handlers[name](args);
  }
}

// A label whose text is a glossary term shows the definition on hover.
function mark(node) {
  const hint = ui.hints.find((h) => h.term === node.textContent);
  if (hint) {
    node.dataset.term = hint.term;
    node.title = hint.definition;
  }
}

function button(a) {
  const b = make("button");
  b.type = "button";
  b.append(make("span", a.label));
  if (a.detail) b.append(make("small", a.detail));
  b.disabled = !a.enabled;
  if (a.selected) b.setAttribute("aria-pressed", "true");
  b.dataset.action = a.name;
  b.dataset.value = a.value;
  b.addEventListener("click", () => {
    ensureAudio();
    act(a.name, a.value);
  });
  return b;
}

function field(f) {
  const label = make("label", f.label);
  const input = make("input");
  input.type = "text";
  input.value = f.value;
  input.maxLength = f.max;
  input.autocomplete = "nickname";
  input.addEventListener("change", () => act(f.action, input.value));
  label.append(input);
  const box = make("div");
  box.className = "field";
  box.append(label, make("p", f.note));
  return box;
}

function panel(p) {
  const box = make("section");
  box.id = p.id;
  if (p.heading) box.append(make("h2", p.heading));
  (p.lines || []).forEach((line) => box.append(make("p", line)));
  if (p.field) box.append(field(p.field));
  if (p.actions) {
    const row = make("div");
    row.className = "actions";
    p.actions.forEach((a) => row.append(button(a)));
    box.append(row);
  }
  return box;
}

function render() {
  ui = JSON.parse(game.ui());
  document.title = ui.title;
  el("title").textContent = ui.title;
  el("panels").replaceChildren(...ui.panels.map(panel));
  document.querySelectorAll("#panels h2, #panels button > span").forEach(mark);
  document.body.dataset.screen = ui.screen;
  document.body.dataset.canvas = ui.canvas;
  document.body.dataset.paused = ui.paused;
  if (ui.canvas) renderer.resize();
  if (ui.message) el("message").textContent = ui.message;
}

function frame() {
  const [contextTime, performanceTime] = clock();
  const list = game.frame(contextTime, performanceTime, performance.now());
  if (ui.canvas) renderer.draw(list);
  const judged = String(game.judged());
  if (document.body.dataset.judged !== judged) document.body.dataset.judged = judged;
  runCommands();
  if (tileSpec && ui.canvas) document.body.dataset.tile = "drawn";
  requestAnimationFrame(frame);
}

// kind and button codes come from the core (assets.kinds, assets.keys)
function send(kind, buttonId, e) {
  const [contextTime, performanceTime] = clock();
  const canvas = el("field");
  game.input(kind, buttonId, pointer.x, pointer.y, canvas.clientWidth, canvas.clientHeight, contextTime, performanceTime, e.timeStamp);
}

function track(e) {
  pointer.x = e.offsetX;
  pointer.y = e.offsetY;
}

function listen() {
  const canvas = el("field");
  canvas.addEventListener("pointermove", (e) => {
    track(e);
    send(assets.kinds.move, assets.mouse, e);
  });
  canvas.addEventListener("pointerdown", (e) => {
    track(e);
    canvas.setPointerCapture(e.pointerId);
    send(assets.kinds.press, assets.mouse, e);
    e.preventDefault();
  });
  canvas.addEventListener("pointerup", (e) => {
    track(e);
    send(assets.kinds.release, assets.mouse, e);
  });
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());
  const keyButtons = assets.keys;
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") act("key.escape");
    const id = keyButtons[e.key.toLowerCase()];
    if (id === undefined || e.repeat || e.target.tagName === "INPUT") return;
    ensureAudio();
    send(assets.kinds.press, id, e);
  });
  window.addEventListener("keyup", (e) => {
    const id = keyButtons[e.key.toLowerCase()];
    if (id !== undefined) send(assets.kinds.release, id, e);
  });
  window.addEventListener("resize", () => ui.canvas && renderer.resize());
  document.addEventListener("visibilitychange", () => document.hidden && act("page.hidden"));
}

async function main() {
  await init();
  const [strings, glossary, index] = await Promise.all(
    ["data/strings.json", "data/glossary.md", "data/charts/index.json"].map((u) => fetch(u).then((r) => r.text())),
  );
  game = new Game(strings, glossary, index);
  assets = JSON.parse(draw_assets());
  renderer = createRenderer(el("field"), assets);
  listen();
  render();
  if (tileSpec) act("tile.spec", tileSpec);
  document.body.dataset.ready = "true";
  requestAnimationFrame(frame);
}

main().catch((e) => {
  el("message").textContent = e.message || String(e);
});
