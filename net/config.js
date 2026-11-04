// Online settings: the only file in the project that names an address on
// another origin (kits/online). Read only by web/net/rtc.js, which loads
// only after a player presses Host or Join (BO-7).
//
// crates/core/tests/online_fence.rs holds this file to three things:
//   - it is the only file outside web/net/vendor/ that names a wss:// address;
//   - every page's CSP connect-src is 'self' plus exactly `relays` below;
//   - `protocol` equals PROTOCOL in crates/core/src/online.rs.
//
// Numbers are allowed here: each is a transport timing, byte or id, not a
// rule of the game.

export const CONFIG = {
  // Keeps this game's rooms apart from every other app on the same relays.
  appId: "countless-days-bosu-p2p",

  // The core's PROTOCOL. The room name is appId, this tag and the code, never
  // the build: a stale tab and a fresh one meet, and the hello tells the
  // stale one to reload.
  protocol: 1,

  // The vendored Trystero bundle, pinned in vendor/SHA256SUMS. Imported only
  // when a player hosts or joins by room code.
  bundle: "net/vendor/trystero-nostr-0.25.4.js",

  // Encrypts session descriptions while they cross relays this project does
  // not run. It ships here, so it is not a secret; game traffic is
  // DTLS-encrypted end to end regardless.
  password: "countless-days",

  // The Nostr relays that introduce players by room code. Each answered a
  // WebSocket upgrade on 2026-11-02; check again before a release.
  relays: [
    "wss://nos.lol",
    "wss://nostr.mom",
    "wss://nostr-01.yakihonne.com",
    "wss://nostr.sathoarder.com",
    "wss://relay.mostro.network",
  ],

  // STUN tells a browser its public address. Two players both behind strict
  // NATs need TURN, which this design has not; online.no_path says so.
  iceServers: [
    { urls: "stun:stun.l.google.com:19302" },
    { urls: "stun:global.stun.twilio.com:3478" },
  ],

  // The negotiated data channel's stream id. Not 0 or 1: Trystero opens its
  // own in-band channel from 0 up.
  channelId: 40,

  // The first byte of every frame: who the sender is, an acknowledgement,
  // or data.
  frame: { host: 0x48, joiner: 0x4a, ack: 0x41, data: 0x44 },

  // How often a link repeats its role until the other end acknowledges it.
  helloRepeatMs: 250,

  // How long to gather ICE candidates before a pasted code is shown anyway.
  gatherTimeoutMs: 6000,

  // How long a room code waits for any relay before reporting online.no_relay.
  relayTimeoutMs: 15000,

  // A WebSocket's readyState when open.
  socketOpen: 1,
};

// `?ice=none` drops the STUN servers, so tabs on one machine connect on host
// candidates alone and nothing leaves the origin. tests/browser/online.py
// uses it; a player never needs it.
export function iceServers() {
  const ice = new URLSearchParams(location.search).get("ice");
  return ice === "none" ? [] : CONFIG.iceServers;
}
