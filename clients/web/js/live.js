import { ReconnectingSocket } from "./reconnecting.js";

const LIVE = "/api/live";
const ASKED_AGAIN_FIRST = 100;
const ASKED_AGAIN_AT_MOST = 2500;

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

class LiveConnection {
  constructor() {
    this.subscriptions = new Map();
    this.counted = 0;
    this.socket = null;
  }

  subscribe(topic, about, heard, opened) {
    this.counted += 1;
    const subscription = { id: String(this.counted), topic, about, heard, opened, failures: 0 };
    this.subscriptions.set(subscription.id, subscription);
    if (this.socket) {
      this.ask(subscription);
    } else {
      this.socket = new ReconnectingSocket(LIVE, {
        opened: () => this.subscriptions.forEach((each) => this.ask(each)),
        heard: (data) => this.heard(String(data)),
      });
    }

    return subscription.id;
  }

  ask(subscription) {
    if (!this.socket.open) {
      return;
    }
    this.socket.send(
      JSON.stringify({
        do: "subscribe",
        id: subscription.id,
        topic: subscription.topic,
        about: subscription.about,
      }),
    );
    subscription.opened();
  }

  heard(data) {
    let told;
    try {
      told = JSON.parse(data);
    } catch {
      return;
    }
    const subscription = this.subscriptions.get(told.id);
    if (!subscription) {
      return;
    }
    if (told.told === "frame") {
      subscription.failures = 0;
      subscription.heard(JSON.stringify(told.frame));
    } else if (told.told === "ended") {
      this.ended(subscription, told.why);
    }
  }

  ended(subscription, why) {
    if (why === "behind") {
      this.ask(subscription);
    } else if (why === "not-there" || why === "failed") {
      const wait = Math.min(ASKED_AGAIN_FIRST * 2 ** subscription.failures, ASKED_AGAIN_AT_MOST);
      subscription.failures += 1;
      subscription.again = setTimeout(() => {
        if (this.subscriptions.get(subscription.id) === subscription) {
          this.ask(subscription);
        }
      }, wait);
    }
  }

  unsubscribe(id) {
    const subscription = this.subscriptions.get(id);
    if (!subscription) {
      return;
    }
    clearTimeout(subscription.again);
    this.subscriptions.delete(id);
    this.socket.send(JSON.stringify({ do: "unsubscribe", id }));
    if (this.subscriptions.size === 0) {
      this.socket.close();
      this.socket = null;
    }
  }
}

const connection = new LiveConnection();

export class LiveSubscription {
  constructor(topic, about, heard, opened) {
    this.id = connection.subscribe(
      topic,
      JSON.parse(about),
      (frame) => heard(frame),
      () => opened(),
    );
  }

  close() {
    connection.unsubscribe(this.id);
  }
}
