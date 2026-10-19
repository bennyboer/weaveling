import * as encoding from "lib0/encoding";
import * as decoding from "lib0/decoding";
import * as awarenessProtocol from "y-protocols/awareness";
import * as syncProtocol from "y-protocols/sync";

const JOIN = 0;
const LEAVE = 1;
const CARRY = 2;
const REFUSED = 3;

const MESSAGE_SYNC = 0;
const MESSAGE_AWARENESS = 1;
const MESSAGE_QUERY_AWARENESS = 3;

const RECONNECT_FIRST = 100;
const RECONNECT_AT_MOST = 2500;

function endpoint() {
  const scheme = window.location.protocol === "https:" ? "wss:" : "ws:";

  return `${scheme}//${window.location.host}/api/scenes/sync`;
}

function enveloped(kind, scene, frame) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, kind);
  encoding.writeVarString(encoder, scene);
  if (frame) {
    encoding.writeVarUint8Array(encoder, frame);
  }

  return encoding.toUint8Array(encoder);
}

export class SyncConnection {
  constructor() {
    this.scenes = new Map();
    this.open = false;
    this.closed = false;
    this.failures = 0;
    this.connect();
  }

  connect() {
    const socket = new WebSocket(endpoint());
    socket.binaryType = "arraybuffer";
    this.socket = socket;

    socket.onopen = () => {
      this.open = true;
      this.failures = 0;
      for (const scene of this.scenes.values()) {
        this.begin(scene);
      }
    };
    socket.onmessage = (event) => this.hear(new Uint8Array(event.data));
    socket.onclose = () => {
      this.open = false;
      for (const scene of this.scenes.values()) {
        scene.disconnected();
      }
      if (!this.closed) {
        const wait = Math.min(RECONNECT_FIRST * 2 ** this.failures, RECONNECT_AT_MOST);
        this.failures += 1;
        setTimeout(() => this.connect(), wait);
      }
    };
  }

  begin(scene) {
    this.socket.send(enveloped(JOIN, scene.id));
    scene.connected();
  }

  join(scene) {
    this.scenes.set(scene.id, scene);
    if (this.open) {
      this.begin(scene);
    }
  }

  leave(scene) {
    if (this.scenes.get(scene.id) !== scene) {
      return;
    }
    this.scenes.delete(scene.id);
    if (this.open) {
      this.socket.send(enveloped(LEAVE, scene.id));
    }
  }

  send(scene, frame) {
    if (this.open && this.scenes.get(scene.id) === scene) {
      this.socket.send(enveloped(CARRY, scene.id, frame));
    }
  }

  hear(bytes) {
    const decoder = decoding.createDecoder(bytes);
    const kind = decoding.readVarUint(decoder);
    const scene = this.scenes.get(decoding.readVarString(decoder));

    if (!scene) {
      return;
    }
    if (kind === CARRY) {
      scene.receive(decoding.readVarUint8Array(decoder));
    } else if (kind === REFUSED) {
      scene.refused();
    }
  }

  close() {
    this.closed = true;
    this.open = false;
    this.socket.close();
  }
}

export class SceneSync {
  constructor(connection, id, doc, onConnected) {
    this.connection = connection;
    this.id = id;
    this.doc = doc;
    this.awareness = new awarenessProtocol.Awareness(doc);
    this.onConnected = onConnected;
    this.isConnected = false;
    this.synced = false;
    this.waitingForSync = [];
    this.saving = () => {};
    this.changes = 0;
    this.stored = 0;
    this.asked = [];
    this.wasSaving = false;

    this.onUpdate = (update, origin) => {
      if (origin === this) {
        return;
      }
      this.send(MESSAGE_SYNC, (encoder) => syncProtocol.writeUpdate(encoder, update));
      this.changes += 1;
      this.tellSaving();
      this.askWhatIsStored();
    };
    this.onAwareness = ({ added, updated, removed }, origin) => {
      if (origin === this) {
        return;
      }
      this.sendAwareness(added.concat(updated, removed));
    };

    doc.on("update", this.onUpdate);
    this.awareness.on("update", this.onAwareness);
    connection.join(this);
  }

  send(kind, write) {
    const encoder = encoding.createEncoder();
    encoding.writeVarUint(encoder, kind);
    write(encoder);
    this.connection.send(this, encoding.toUint8Array(encoder));
  }

  sendAwareness(clients) {
    this.send(MESSAGE_AWARENESS, (encoder) =>
      encoding.writeVarUint8Array(
        encoder,
        awarenessProtocol.encodeAwarenessUpdate(this.awareness, clients),
      ),
    );
  }

  connected() {
    this.isConnected = true;
    this.asked = [this.stored];
    this.send(MESSAGE_SYNC, (encoder) => syncProtocol.writeSyncStep1(encoder, this.doc));
    if (this.awareness.getLocalState() !== null) {
      this.sendAwareness([this.doc.clientID]);
    }
    this.onConnected(true);
  }

  disconnected() {
    this.isConnected = false;
    this.asked = [];
    const others = Array.from(this.awareness.getStates().keys()).filter(
      (client) => client !== this.doc.clientID,
    );
    awarenessProtocol.removeAwarenessStates(this.awareness, others, this);
    this.onConnected(false);
  }

  refused() {
    this.isConnected = false;
    this.asked = [];
    this.onConnected(false);
  }

  receive(frame) {
    const decoder = decoding.createDecoder(frame);
    const kind = decoding.readVarUint(decoder);

    if (kind === MESSAGE_SYNC) {
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, MESSAGE_SYNC);
      const step = syncProtocol.readSyncMessage(decoder, encoder, this.doc, this);

      if (encoding.length(encoder) > 1) {
        this.connection.send(this, encoding.toUint8Array(encoder));
      }
      if (step === syncProtocol.messageYjsSyncStep2) {
        this.answered();
        this.becomeSynced();
      }
    } else if (kind === MESSAGE_AWARENESS) {
      awarenessProtocol.applyAwarenessUpdate(
        this.awareness,
        decoding.readVarUint8Array(decoder),
        this,
      );
    } else if (kind === MESSAGE_QUERY_AWARENESS) {
      this.sendAwareness(Array.from(this.awareness.getStates().keys()));
    }
  }

  becomeSynced() {
    if (this.synced) {
      return;
    }
    this.synced = true;
    for (const waiting of this.waitingForSync.splice(0)) {
      waiting();
    }
  }

  whenSynced(then) {
    if (this.synced) {
      then();
    } else {
      this.waitingForSync.push(then);
    }
  }

  onSaving(saving) {
    this.saving = saving;
    saving(this.isSaving());
  }

  askWhatIsStored() {
    if (this.asked.length > 0 || !this.isConnected || this.stored === this.changes) {
      return;
    }
    this.asked.push(this.changes);
    this.send(MESSAGE_SYNC, (encoder) => syncProtocol.writeSyncStep1(encoder, this.doc));
  }

  answered() {
    const covered = this.asked.shift();

    if (covered === undefined) {
      return;
    }
    this.stored = Math.max(this.stored, covered);
    this.tellSaving();
    this.askWhatIsStored();
  }

  isSaving() {
    return this.stored < this.changes;
  }

  tellSaving() {
    const saving = this.isSaving();

    if (saving !== this.wasSaving) {
      this.wasSaving = saving;
      this.saving(saving);
    }
  }

  destroy() {
    awarenessProtocol.removeAwarenessStates(this.awareness, [this.doc.clientID], "leaving");
    this.connection.leave(this);
    this.doc.off("update", this.onUpdate);
    this.awareness.off("update", this.onAwareness);
    this.awareness.destroy();
  }
}
