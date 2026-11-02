// The transport: browsers, one reliable channel each, and no server of ours.
//
// From Raiders of the Lost Mart's web/net/rtc.js (Vagrancy's, ported from
// Floodline's quad_rtc.js), with every number moved into config.js so the
// page scan still holds this file to 0 and 1 (kits/online), and the error
// keys renamed to this project's strings (online.no_relay, online.no_path).
//
// A star: the host links to each joiner; a joiner links to the host only.
// Two introductions reach the same link: a room code over the Nostr relays
// in config.js (Trystero), or pasted codes that carry the whole handshake.
// Everything above this file sees only `onPeer`, `onLeft`, `onMessage`,
// `onError` and `send`, and decides nothing about the game.

import { CONFIG, iceServers } from "./config.js";

// Every frame's first byte says what it is (config.js, `frame`). A role is
// repeated until acknowledged, and data waits until the other side's role
// has arrived, which proves its end of the negotiated channel exists.
const ROLE_HOST = CONFIG.frame.host;
const ROLE_JOINER = CONFIG.frame.joiner;
const ACK = CONFIG.frame.ack;
const DATA = CONFIG.frame.data;

function rtcConfig() {
  return { iceServers: iceServers() };
}

export function open({ isHost, mode, room, build, onPeer, onLeft, onMessage, onError }) {
  const s = { isHost, closed: false, trystero: null, pending: null, hostPeer: null, nextId: 1, nextCode: 1 };

  // A peer is the other browser, whatever connections come and go under it.
  // Trystero may replace a connection to the same browser (a renegotiation,
  // or a retry after a failed attempt); the first version treated the new
  // connection as a stranger, turned it away as "full", and kept sending to
  // the dead one, so Start never arrived and the match froze (reported by
  // Sam between two computers). Now a peer keeps one id, its current link is
  // swapped in place, and what cannot be sent yet waits in its queue.
  const peers = new Map(); // key -> { id, key, link, queue, reported }
  const byId = new Map();
  function peerFor(key) {
    if (!peers.has(key)) {
      const peer = { id: s.nextId++, key, link: null, queue: [], reported: false };
      peers.set(key, peer);
      byId.set(peer.id, peer);
    }
    return peers.get(key);
  }

  function open(link) {
    return link && !link.gone && link.ready && link.ch.readyState === 'open';
  }

  function flush(peer) {
    while (peer.queue.length && open(peer.link)) {
      const body = peer.queue[0];
      const framed = new Uint8Array(body.length + 1);
      framed[0] = DATA;
      framed.set(body, 1);
      try { peer.link.ch.send(framed); } catch (e) { return; }
      peer.queue.shift();
    }
  }

  function attach(pc, mine, key) {
    const peer = peerFor(key);
    const link = { pc, mine, ch: null, role: 0, ready: false, acked: false, gone: false, peer, timer: null, early: [] };
    const ch = pc.createDataChannel('lobby', { negotiated: true, id: CONFIG.channelId, ordered: true });
    ch.binaryType = 'arraybuffer';
    link.ch = ch;
    const sayRole = () => {
      if (link.gone || link.acked || ch.readyState !== 'open') return;
      try { ch.send(new Uint8Array([isHost ? ROLE_HOST : ROLE_JOINER])); } catch (e) { /* closing */ }
    };
    ch.onopen = () => {
      sayRole();
      link.timer = setInterval(sayRole, CONFIG.helloRepeatMs);
    };
    ch.onmessage = (e) => {
      const bytes = new Uint8Array(e.data);
      const kind = bytes[0];
      if (kind === ACK) {
        link.acked = true;
        clearInterval(link.timer);
        return;
      }
      if (kind === ROLE_HOST || kind === ROLE_JOINER) {
        try { ch.send(new Uint8Array([ACK])); } catch (err) { /* closing */ }
        if (link.ready) return; // a repeat
        link.role = kind;
        // The star: a host talks to joiners; a joiner to one host.
        const ok = isHost ? kind === ROLE_JOINER : (kind === ROLE_HOST && (!s.hostPeer || s.hostPeer === peer));
        if (!ok) { hangUp(link); return; }
        if (!isHost) s.hostPeer = peer;
        link.ready = true;
        if (peer.link && peer.link !== link) hangUp(peer.link);
        peer.link = link;
        flush(peer);
        if (!peer.reported) { peer.reported = true; onPeer(peer.id); }
        // Data that beat this side's copy of the role here is delivered now.
        for (const b of link.early.splice(0)) onMessage(peer.id, b);
        return;
      }
      if (kind === DATA) {
        // The sender has heard our role, so its end exists; its own role may
        // still be a repeat away. Keep what it sent until then.
        if (link.ready) onMessage(peer.id, bytes.subarray(1));
        else link.early.push(bytes.slice(1));
      }
    };
    const lost = () => {
      if (link.gone) return;
      link.gone = true;
      clearInterval(link.timer);
      if (peer.link === link) peer.link = null;
      // By pasted code there is no relay to say a peer left; a closed link
      // is the leaving. By room code, Trystero says so (onPeerLeave), and a
      // closed link may be a connection being replaced.
      if (mode === 'code' || link.mine) leave(peer);
    };
    ch.onclose = lost;
    pc.addEventListener('connectionstatechange', () => {
      if (pc.connectionState === 'failed' || pc.connectionState === 'closed') lost();
    });
    return link;
  }

  function leave(peer) {
    if (peer.reported && !peer.left) { peer.left = true; onLeft(peer.id); }
  }

  function hangUp(link) {
    link.gone = true;
    clearInterval(link.timer);
    try { link.ch.close(); if (link.mine) link.pc.close(); } catch (e) { /* already closing */ }
  }

  // ---- a room code, through Trystero ----------------------------------------

  async function startRoom() {
    let lib;
    try {
      lib = await import(new URL(CONFIG.bundle, document.baseURI).href);
    } catch (e) {
      console.warn('the signalling bundle did not load', e);
      onError('online.no_relay');
      return;
    }
    if (s.closed) return;
    // The room is named by the protocol, not the build: two builds a deploy
    // apart still meet, and the host's hello check turns a mismatched one
    // away with a sentence. (Named by build, a friend with a page cached
    // from an earlier deploy looked for another room and waited for ever.)
    const name = `${CONFIG.protocol}-${room}`;
    let r;
    try {
      r = lib.joinRoom({ appId: CONFIG.appId, password: CONFIG.password, rtcConfig: rtcConfig(), relayConfig: { urls: CONFIG.relays } }, name, {
        onJoinError: (d) => {
          // A failed attempt to reach a browser we are already playing with
          // is Trystero retrying under a working link, not the end of it.
          const known = d && d.peerId && peers.get(d.peerId);
          if (known && known.reported && !known.left) { console.warn('a connection attempt failed under a working link', d); return; }
          onError(d && d.peerId ? 'online.no_path' : 'online.no_relay');
        },
      });
    } catch (e) {
      console.warn(e);
      onError('online.no_relay');
      return;
    }
    s.trystero = r;
    // Trystero 0.25.4 takes these as properties, as Floodline assigns them.
    // A peer is keyed by Trystero's id for it, so a replaced connection to
    // the same browser is the same peer.
    r.onPeerJoin = (pid) => {
      const pc = r.getPeers()[pid];
      if (pc) attach(pc, false, pid);
    };
    r.onPeerLeave = (pid) => {
      const peer = peers.get(pid);
      if (peer) leave(peer);
    };
    setTimeout(() => {
      if (s.closed || [...peers.values()].some((p) => p.reported)) return;
      let open = 0;
      try {
        const sockets = lib.getRelaySockets();
        for (const k in sockets) if (sockets[k] && sockets[k].readyState === CONFIG.socketOpen) open++;
      } catch (e) { /* the strategy may not expose them */ }
      if (open === 0) onError('online.no_relay');
    }, CONFIG.relayTimeoutMs);
  }

  // ---- a pasted code ----------------------------------------------------------

  function gathered(pc) {
    if (pc.iceGatheringState === 'complete') return Promise.resolve();
    return new Promise((resolve) => {
      let done = false;
      const finish = () => { if (!done) { done = true; resolve(); } };
      pc.addEventListener('icegatheringstatechange', () => { if (pc.iceGatheringState === 'complete') finish(); });
      setTimeout(finish, CONFIG.gatherTimeoutMs);
    });
  }

  async function invitation() {
    const pc = new RTCPeerConnection(rtcConfig());
    // each invitation is its own browser: Vagrancy keyed every pasted-code
    // link 'code', which with two seats was one peer, and with four made a
    // second joiner's link replace (and hang up) the first's
    s.pending = attach(pc, true, `code-${s.nextCode++}`);
    await pc.setLocalDescription(await pc.createOffer());
    await gathered(pc);
    return pack('O', pc.localDescription.sdp);
  }

  async function reply(code) {
    const sdp = await unpack('O', code);
    const pc = new RTCPeerConnection(rtcConfig());
    s.pending = attach(pc, true, 'code');
    await pc.setRemoteDescription({ type: 'offer', sdp });
    await pc.setLocalDescription(await pc.createAnswer());
    await gathered(pc);
    return pack('A', pc.localDescription.sdp);
  }

  async function accept(code) {
    const sdp = await unpack('A', code);
    await s.pending.pc.setRemoteDescription({ type: 'answer', sdp });
  }

  if (mode === 'room') startRoom();

  return {
    // Host: the invitation to send. Joiner: given an invitation, the reply.
    invitation,
    reply,
    // Host: the joiner's reply.
    accept,
    // Never drops: what cannot go now waits for the peer's open link.
    send(id, bytes) {
      const peer = byId.get(id);
      if (!peer) return false;
      peer.queue.push(bytes);
      flush(peer);
      return true;
    },
    close() {
      s.closed = true;
      if (s.trystero) { try { s.trystero.leave(); } catch (e) { /* gone */ } }
      peers.forEach((p) => p.link && hangUp(p.link));
      peers.clear();
      byId.clear();
    },
  };
}

// ---- the blob: a data-channel SDP is nearly all boilerplate --------------------
//
// Only a handful of lines say anything the other end cannot infer, so the
// code carries those and `grow` rebuilds a valid description around them
// (Floodline quad_rtc.js `shrink`/`grow`).

// Each kept line: its SDP prefix and the one-letter tag the code uses.
const KEPT = [
  ["a=ice-ufrag:", "u"],
  ["a=ice-pwd:", "p"],
  ["a=fingerprint:", "f"],
  ["a=setup:", "s"],
  ["a=mid:", "m"],
  ["a=sctp-port:", "P"],
  ["a=max-message-size:", "M"],
];
const CANDIDATE = "a=candidate:";

function shrink(sdp) {
  const keep = [];
  for (const l of sdp.split(/\r\n|\n/)) {
    const kept = KEPT.find(([prefix]) => l.startsWith(prefix));
    if (kept) keep.push(kept[1] + l.slice(kept[0].length));
    else if (l.startsWith(CANDIDATE) && / (udp|UDP) /.test(l)) keep.push("c" + l.slice(CANDIDATE.length));
  }
  return keep.join("\n");
}

function grow(min) {
  const v = { u: "", p: "", f: "", s: "actpass", m: "0", P: "5000", M: "262144" };
  const cands = [];
  for (const line of min.split("\n")) {
    const k = line[0];
    if (k === "c") cands.push(CANDIDATE + line.slice(1));
    else if (k in v) v[k] = line.slice(1);
  }
  return ["v=0", "o=- 0 0 IN IP4 127.0.0.1", "s=-", "t=0 0", "a=group:BUNDLE " + v.m, "a=msid-semantic: WMS",
    "m=application 9 UDP/DTLS/SCTP webrtc-datachannel", "c=IN IP4 0.0.0.0", ...cands,
    "a=ice-ufrag:" + v.u, "a=ice-pwd:" + v.p, "a=ice-options:trickle", "a=fingerprint:" + v.f,
    "a=setup:" + v.s, "a=mid:" + v.m, "a=sctp-port:" + v.P, "a=max-message-size:" + v.M, ""].join("\r\n");
}

const b64url = (bytes) => btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
function unb64url(text) {
  const s = atob(text.replace(/-/g, '+').replace(/_/g, '/'));
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}
async function through(stream, bytes) {
  const w = stream.writable.getWriter();
  w.write(bytes);
  w.close();
  return new Uint8Array(await new Response(stream.readable).arrayBuffer());
}

// "O" for an invitation, "A" for a reply, outside the compressed part, so a
// code pasted into the wrong box is refused with a sentence.
async function pack(kind, sdp) {
  return kind + b64url(await through(new CompressionStream('deflate-raw'), new TextEncoder().encode(shrink(sdp))));
}

export class BadCode extends Error {}

async function unpack(kind, code) {
  const text = (code || '').replace(/\s+/g, '');
  if (text[0] !== kind) throw new BadCode('wrong kind');
  try {
    return grow(new TextDecoder().decode(await through(new DecompressionStream('deflate-raw'), unb64url(text.slice(1)))));
  } catch (e) {
    throw new BadCode(String(e));
  }
}
