import { ReconnectingSocket } from "./reconnecting.js";

export class LiveFollowing {
  constructor(path, heard, opened) {
    this.socket = new ReconnectingSocket(path, {
      opened: () => opened(),
      heard: (data) => {
        let frame;
        try {
          frame = JSON.parse(data);
        } catch {
          return;
        }
        if (typeof frame.version === "number") {
          heard(frame.version);
        }
      },
    });
  }

  close() {
    this.socket.close();
  }
}
