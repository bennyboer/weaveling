import type { Page, WebSocketRoute } from "@playwright/test";

const LIVE = /\/api\/live$/;

type About = Record<string, string>;

type Subscribed = { id: string; topic: string; about: About };

const matches = (subscribed: Subscribed, topic: string, about: About) =>
  subscribed.topic === topic &&
  Object.entries(about).every(([key, value]) => subscribed.about[key] === value);

type Heard =
  | { sent: Record<string, string>; topics: Map<string, Subscribed> }
  | { received: Record<string, string>; topics: Map<string, Subscribed> };

const followed = new WeakMap<Page, Set<(heard: Heard) => void>>();

export function followingLive(page: Page): Set<(heard: Heard) => void> {
  let listeners = followed.get(page);
  if (!listeners) {
    const every = new Set<(heard: Heard) => void>();
    listeners = every;
    followed.set(page, every);
    page.on("websocket", (socket) => {
      if (!LIVE.test(socket.url())) {
        return;
      }
      const topics = new Map<string, Subscribed>();
      socket.on("framesent", ({ payload }) => {
        const sent = JSON.parse(String(payload));
        if (sent.do === "subscribe") {
          topics.set(sent.id, { id: sent.id, topic: sent.topic, about: sent.about });
        }
        every.forEach((listener) => listener({ sent, topics }));
      });
      socket.on("framereceived", ({ payload }) => {
        const received = JSON.parse(String(payload));
        every.forEach((listener) => listener({ received, topics }));
      });
    });
  }

  return listeners;
}

function onLive(
  page: Page,
  sent: (asked: Record<string, string>, topics: Map<string, Subscribed>) => void,
  received: (told: Record<string, string>, topics: Map<string, Subscribed>) => void,
) {
  followingLive(page).add((heard) =>
    "sent" in heard ? sent(heard.sent, heard.topics) : received(heard.received, heard.topics),
  );
}

export function watchingLive(page: Page) {
  const subscribed: string[] = [];
  onLive(
    page,
    (asked) => {
      if (asked.do === "subscribe") {
        subscribed.push(asked.topic);
      }
    },
    () => {},
  );

  return { topics: () => subscribed };
}

export function heardOn(page: Page, topic: string, heard: () => void, about: About = {}) {
  onLive(
    page,
    () => {},
    (told, topics) => {
      const subscription = topics.get(told.id);
      if (told.told === "frame" && subscription && matches(subscription, topic, about)) {
        heard();
      }
    },
  );
}

export function subscribing(page: Page, topic: string): Promise<void> {
  return new Promise((resolve) =>
    onLive(
      page,
      (asked) => {
        if (asked.do === "subscribe" && asked.topic === topic) {
          resolve();
        }
      },
      () => {},
    ),
  );
}

export function unsubscribedOn(page: Page, topic: string, about: About, done: () => void) {
  onLive(
    page,
    (asked, topics) => {
      const subscription = topics.get(asked.id);
      if (asked.do === "unsubscribe" && subscription && matches(subscription, topic, about)) {
        done();
      }
    },
    () => {},
  );
}

type Routing = {
  withheld?: string[];
  held?: string[];
  heard?: Record<string, (told: string, pass: () => void, about: About) => void>;
  connecting?: (socket: WebSocketRoute, nth: number) => boolean;
};

export async function routingLive(page: Page, routing: Routing = {}) {
  const subscriptions = new Map<string, { topic: string; about: About; socket: WebSocketRoute }>();
  const held: { topic: string; send: () => void }[] = [];
  const sockets: WebSocketRoute[] = [];

  await page.routeWebSocket(LIVE, (socket) => {
    sockets.push(socket);
    if (routing.connecting && !routing.connecting(socket, sockets.length)) {
      return;
    }
    const server = socket.connectToServer();
    socket.onMessage((message) => {
      const asked = JSON.parse(String(message));
      if (asked.do === "subscribe") {
        subscriptions.set(asked.id, { topic: asked.topic, about: asked.about, socket });
        if (routing.withheld?.includes(asked.topic)) {
          return;
        }
        if (routing.held?.includes(asked.topic)) {
          held.push({ topic: asked.topic, send: () => server.send(message) });
          return;
        }
      }
      server.send(message);
    });
    server.onMessage((message) => {
      const told = JSON.parse(String(message));
      const subscription = subscriptions.get(told.id);
      const hearing = subscription ? routing.heard?.[subscription.topic] : undefined;
      if (told.told === "frame" && subscription && hearing) {
        hearing(String(message), () => socket.send(message), subscription.about);
      } else {
        socket.send(message);
      }
    });
  });

  return {
    sockets,
    release(topic: string) {
      for (const waiting of held.filter((each) => each.topic === topic)) {
        waiting.send();
      }
    },
    tell(topic: string, frame: object) {
      for (const [id, subscription] of subscriptions) {
        if (subscription.topic === topic) {
          subscription.socket.send(JSON.stringify({ told: "frame", id, frame }));
        }
      }
    },
    end(topic: string, why: string) {
      for (const [id, subscription] of subscriptions) {
        if (subscription.topic === topic) {
          subscription.socket.send(JSON.stringify({ told: "ended", id, why }));
        }
      }
    },
    again(told: string) {
      const { id } = JSON.parse(told);
      subscriptions.get(id)?.socket.send(told);
    },
  };
}
