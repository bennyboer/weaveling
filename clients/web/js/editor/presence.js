import {
  defaultCursorBuilder,
  defaultSelectionBuilder,
  yCursorPlugin,
} from "y-prosemirror";

const COLOUR = /^#[0-9a-fA-F]{6}$/;
const COLOUR_UNTOLD = "#ffa500";
const LONGEST_NAME = 64;

let holdingTheCaret = null;

function asTold(user) {
  return {
    name: String(user.name ?? "").slice(0, LONGEST_NAME),
    color: COLOUR.test(user.color) ? user.color : COLOUR_UNTOLD,
  };
}

export class Presence {
  constructor(awareness, client, account, connection) {
    this.awareness = awareness;
    this.client = client;
    this.account = account;
    this.connection = connection;
    this.present = () => {};
    this.tellPresent = () => this.present(JSON.stringify(this.othersHere()));
    this.awareness.setLocalState(null);
    this.awareness.on("change", this.tellPresent);
  }

  onPresent(present) {
    this.present = present;
    this.tellPresent();
  }

  holdTheCaret() {
    if (holdingTheCaret === this) {
      return;
    }
    holdingTheCaret?.letGoOfTheCaret();
    holdingTheCaret = this;
    this.awareness.setLocalState(this.account ? { account: this.account } : {});
  }

  letGoOfTheCaret() {
    this.awareness.setLocalState(null);
  }

  othersHere() {
    const here = new Set();
    for (const [client, state] of this.awareness.getStates()) {
      if (client !== this.client && typeof state?.account === "string") {
        here.add(state.account);
      }
    }

    return [...here];
  }

  whoIs(client) {
    const account = this.awareness.getStates().get(client)?.account;
    const person = this.connection.personOf(account);
    const color = person
      ? getComputedStyle(document.documentElement)
          .getPropertyValue(`--${person.tint}`)
          .trim()
      : "";

    return { name: person?.name ?? "", color };
  }

  cursors() {
    return yCursorPlugin(this.awareness, {
      cursorBuilder: (_, client) =>
        defaultCursorBuilder(asTold(this.whoIs(client)), client),
      selectionBuilder: (_, client) =>
        defaultSelectionBuilder(asTold(this.whoIs(client)), client),
    });
  }

  destroy() {
    if (holdingTheCaret === this) {
      holdingTheCaret = null;
    }
    this.awareness.off("change", this.tellPresent);
  }
}
