import { ReconnectingSocket } from "./reconnecting.js";

export class LiveFollowing {
  constructor(path, heard, opened) {
    this.socket = new ReconnectingSocket(path, {
      opened: () => opened(),
      heard: (data) => heard(String(data)),
    });
  }

  close() {
    this.socket.close();
  }
}
