// Online play, as the page sees it (kits/online): opens the transport
// (web/net/rtc.js) when the core says so, hands the core every peer,
// departure, message and error, and sends what the core leaves to send.
// The core decides everything; this file only carries.
import * as rtc from "./net/rtc.js?v=0d5896f56faf";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function makeOnline(game, after) {
  let link = null;

  function close() {
    if (link) link.close();
    link = null;
  }

  function open(c) {
    close();
    link = rtc.open({
      isHost: c.host,
      mode: c.mode,
      room: c.room,
      onPeer: (id) => {
        game.wire_peer(id);
        after();
      },
      onLeft: (id) => {
        game.wire_left(id);
        after();
      },
      onMessage: (id, bytes) => {
        game.wire_message(id, decoder.decode(bytes));
        after();
      },
      onError: (key) => {
        game.wire_error(key);
        after();
      },
    });
  }

  // a pasted-code step: the transport's promise, its text to the core
  async function step(promise, action) {
    try {
      game.act(action, await promise);
    } catch (e) {
      game.wire_error(e instanceof rtc.BadCode ? "online.bad_code" : "online.signal_failed");
    }
    after();
  }

  return {
    open,
    close,
    invitation: () => link && step(link.invitation(), "online.invitation"),
    reply: (text) => link && step(link.reply(text), "online.reply"),
    accept: (text) => link && step(link.accept(text).then(() => ""), "online.accepted"),
    send: (peer, text) => link && link.send(peer, encoder.encode(text)),
  };
}
