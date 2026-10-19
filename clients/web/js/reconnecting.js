const RECONNECT_FIRST = 100;
const RECONNECT_AT_MOST = 2500;
const STEADY_AFTER = 3000;

function address(path) {
  const scheme = window.location.protocol === "https:" ? "wss:" : "ws:";

  return `${scheme}//${window.location.host}${path}`;
}

export class ReconnectingSocket {
  constructor(path, { binary = false, opened = () => {}, heard = () => {}, dropped = () => {} }) {
    this.path = path;
    this.binary = binary;
    this.opened = opened;
    this.heard = heard;
    this.dropped = dropped;
    this.open = false;
    this.closed = false;
    this.failures = 0;
    this.connect();
  }

  connect() {
    if (this.closed) {
      return;
    }
    const socket = new WebSocket(address(this.path));
    if (this.binary) {
      socket.binaryType = "arraybuffer";
    }
    this.socket = socket;

    socket.onopen = () => {
      this.open = true;
      this.steady = setTimeout(() => {
        this.failures = 0;
      }, STEADY_AFTER);
      this.opened();
    };
    socket.onmessage = (event) => this.heard(event.data);
    socket.onclose = () => {
      clearTimeout(this.steady);
      this.open = false;
      this.dropped();
      if (!this.closed) {
        const wait = Math.min(RECONNECT_FIRST * 2 ** this.failures, RECONNECT_AT_MOST);
        this.failures += 1;
        setTimeout(() => this.connect(), wait);
      }
    };
  }

  send(data) {
    if (this.open) {
      this.socket.send(data);
    }
  }

  close() {
    this.closed = true;
    this.open = false;
    this.socket.close();
  }
}
