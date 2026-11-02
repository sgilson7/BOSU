// Replays the core's draw list onto a 2D canvas. Part of the canvas kit
// (kits/canvas/README.md), after Peter's renderer for Mandible.
//
// It holds no rule and no number. The op codes, the palette's role names,
// the shapes, the scales and the canvas size come from the core's assets
// (crates/core/src/draw.rs, `assets()`), read once. Colours come from the
// stylesheet: role `ink` is the CSS custom property `--ink`, so the dark
// theme needs no change here.
//
//   const renderer = createRenderer(canvas, JSON.parse(draw_assets()));
//   renderer.draw(game.draw_list());   // an Int32Array, every frame

function toPath(polygons) {
  const path = new Path2D();
  for (const polygon of polygons) {
    const [first, ...rest] = polygon;
    path.moveTo(first[0], first[1]);
    for (const point of rest) path.lineTo(point[0], point[1]);
    path.closePath();
  }
  return path;
}

export function createRenderer(canvas, assets) {
  const tau = assets.tau_milli / assets.unit;
  const ctx = canvas.getContext("2d");
  const paths = assets.shapes.map(toPath);
  const [width, height] = assets.size;
  let colors = [];
  let family = "";
  let list = [];
  let at = 0;

  // Reads the palette and the font from the stylesheet. Called again when
  // the colour scheme changes.
  function readStyle() {
    const style = getComputedStyle(canvas);
    colors = assets.palette.map((role) => style.getPropertyValue(`--${role}`).trim());
    family = style.fontFamily;
  }

  // The stylesheet sizes the canvas to the window; the backing store
  // follows its size times the screen's density, so lines stay sharp, and
  // the core's playfield is scaled to fit. (Changed from the kit, which
  // drew at a fixed size: R3.5.)
  function resize() {
    const density = window.devicePixelRatio || 1;
    canvas.width = canvas.clientWidth * density;
    canvas.height = canvas.clientHeight * density;
    const fit = canvas.width / width;
    ctx.setTransform(fit, 0, 0, fit, 0, 0);
  }

  const next = () => {
    const value = list[at];
    at += 1;
    return value;
  };

  function points() {
    const count = next();
    ctx.beginPath();
    for (let k = 0; k < count; k += 1) {
      const x = next();
      ctx.lineTo(x, next());
    }
  }

  const ops = [];
  ops[assets.ops.clear] = () => {
    ctx.fillStyle = colors[next()];
    ctx.fillRect(0, 0, width, height);
  };
  ops[assets.ops.rect] = () => {
    const x = next();
    const y = next();
    const w = next();
    const h = next();
    ctx.fillStyle = colors[next()];
    ctx.fillRect(x, y, w, h);
  };
  ops[assets.ops.shape] = () => {
    const path = paths[next()];
    const x = next();
    const y = next();
    // shape points are in thousandths of the size, and the size is in
    // thousandths of a unit
    const scale = next() / assets.unit / assets.unit;
    ctx.fillStyle = colors[next()];
    ctx.strokeStyle = colors[next()];
    const lineWidth = next();
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(scale, scale);
    // the line width is in canvas pixels, so undo the shape's scale for it
    ctx.lineWidth = lineWidth / scale;
    ctx.fill(path);
    ctx.stroke(path);
    ctx.restore();
  };
  ops[assets.ops.line] = () => {
    ctx.strokeStyle = colors[next()];
    ctx.lineWidth = next();
    points();
    ctx.stroke();
  };
  ops[assets.ops.text] = () => {
    const x = next();
    const y = next();
    const size = next();
    ctx.fillStyle = colors[next()];
    ctx.textAlign = assets.aligns[next()];
    const count = next();
    const codes = [];
    for (let k = 0; k < count; k += 1) codes.push(next());
    ctx.font = `${size}px ${family}`;
    ctx.fillText(String.fromCodePoint(...codes), x, y);
  };

  ops[assets.ops.circle] = () => {
    const x = next();
    const y = next();
    const r = next();
    const fill = next();
    const stroke = next();
    ctx.lineWidth = next();
    ctx.beginPath();
    ctx.arc(x, y, r, 0, tau);
    if (fill !== assets.none) {
      ctx.fillStyle = colors[fill];
      ctx.fill();
    }
    if (stroke !== assets.none) {
      ctx.strokeStyle = colors[stroke];
      ctx.stroke();
    }
  };
  ops[assets.ops.alpha] = () => {
    ctx.globalAlpha = next() / assets.unit;
  };

  function draw(drawList) {
    list = drawList;
    at = 0;
    ctx.globalAlpha = 1;
    ctx.textBaseline = "middle";
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    while (at < list.length) ops[next()]();
  }

  readStyle();
  resize();
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", readStyle);

  return { draw, resize, readStyle };
}
