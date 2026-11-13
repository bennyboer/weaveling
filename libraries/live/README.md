# live

Lets an open browser learn that something changed, without asking again and again.

**A `Broadcaster<C>` sends frames to everyone who listens to one of its channels.** Its type says what a channel is. Each feature has its own channel type. The type is named after what changes, and it holds whose changes they are:

- `Broadcaster<InvitationChanges>`: invitations to an address.
- `Broadcaster<IdeaChanges>`: a project's ideas.
- `Broadcaster<BoardChanges>`: a board.
- `Broadcaster<MembershipChanges>`: the one channel that every member listens to. It is filtered for each member.

Any `Clone + PartialEq` type that is `Send` and `Sync` is a `Channel`. `tell(channel, frame)` sends a frame. `hear(channel)` returns a `Hearing` to read frames from. It is not an error if nobody is listening. A listener that falls too far behind is dropped, so it does not hold up the others. It is in memory and per process. That is all one server needs.

Here, a board's changes are sent based on what was published. Everyone who may read the board can follow them. A test receives them the same way the browser does:

```rust
use std::sync::Arc;

use live::testing::Subscriber;
use live::{About, BroadcastChangesListener, Broadcaster, Refused, Subscribed, Topic, Topics};

let broadcaster = Broadcaster::new();
let broadcast = BroadcastChangesListener::new(LIVE, EVERY_BOARD, broadcaster.clone(), |published| {
    vec![BoardChanges(published.aggregate.id.clone())]
});

#[async_trait]
impl Topic for BoardsTopic {
    fn named(&self) -> &'static str {
        "boards"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        let board = about.get("board").ok_or(Refused::NotThere)?;
        let hearing = self.broadcaster.hear(BoardChanges(board.clone()));
        let admission = live::admitted(Arc::new(BoardsGate(self.boards.clone())), agent, board).await?;
        let standing = self.boards.read(board, agent).await.map_err(|failed| refused(&failed))?;

        Ok(Subscribed::hearing(hearing)
            .standing_at(standing.version.count())
            .while_admitted(admission))
    }
}

let socket = live::live_socket(Topics::of(vec![Arc::new(BoardsTopic::new(boards, broadcaster))]));

let mut ada = Subscriber::on(&server).await;
ada.subscribe("board", "boards", &[("board", &board)]).await;
assert_eq!(ada.frame().await["version"], 1);
```

**The frames come from the outbox, not from the request that made the change.** `BroadcastChangesListener` is an ordinary listener on a feature's events. It is given the channels that a published change is sent on. Examples: a board's own channel; a project's own channel and the list of all projects; the channel of all memberships. `frame_of` turns what was published into what goes on the wire. So a frame only reports what was stored and published, in the order it happened. A change made by a listener reaches the browser the same way as a change made by a click. `Teller` numbers frames itself. It is for things that have no published version to carry.

**A frame has one shape:** `FrameDTO` from [`live-contract`](../live-contract). It holds what the frame is `of`, its `version`, when it happened (`at`), and its `event`. The server parses every frame it receives as a `FrameDTO` before sending it on. So what the server writes and what the client reads cannot drift apart.

**There is one socket for everything live: `live_socket`.** It is served at `/live`, and it requires signing in. It is the only way to follow anything. Over it, the client subscribes and unsubscribes with `AskedDTO` from `live-contract`:

- `subscribe`, with an id the client chooses, a topic, and what the subscription is about,
- `unsubscribe`, with the id.

Subscribing again under the same id replaces what that id followed. Closing the socket ends all subscriptions.

**A feature hands over a `Topic`.** There is one topic per feature. It is `named` the way the client asks for it. Its `subscribe` receives the agent and what the subscription is `About`, for example `{ "board": … }`, `{ "project": … }` or `{ "mine": "true" }`. It returns one of two things:

- `Refused`. This is either `NotThere` or `Failed`. `NotThere` is returned both when the thing does not exist and when this agent may not follow it, so the two cases look the same. `Failed` is returned when this could not be decided.
- `Subscribed`. This holds the `Hearing`, plus, as needed:
  - **`standing_at`**: the version the subscription starts at. It is sent first, so that a change made between reading and subscribing is not lost. Boards and outlines start listening first, then read, and then report their version.
  - **`while_admitted`**: an `Admission`, so the subscription follows only while it is allowed to. The admission is checked again whenever access may have changed. Once the agent is no longer allowed, the subscription ends with `access-lost`. If the agent is still allowed, it receives the notice `access-changed`, because its role may now be different.
  - **`sifted`**: a `Sieve`. A sieve lets each frame through or holds it back. So the followers of one channel each receive only their own part of it. Examples: the list of projects, and one's own members.

**A `Gate` decides who may follow.** It receives what is followed as text, such as a project's id or a board's id. `admits` answers yes, no, or an error when it cannot decide. `watch` returns something that wakes up when the answer may have changed. By default it never wakes up. **`admitted(gate, agent, about)`** first starts watching and then asks, so no change can slip in between. It returns the `Admission`, or `Refused`.

A subscription waits only for the watch, next to its frames. Once the watch wakes up, the gate is asked inside the loop's own body. So a frame that arrives while the gate is answering cannot cancel the question. *Fixed on 2026-11-02, from the integrity audit.* The bug was: a removed member kept receiving frames when a frame won the race against the gate's answer. This happened mostly when a frame arrived on the same channel.

**A `GatedTopic`** is the topic of a feature that only asks a gate. It holds its name, its broadcaster, how to get the channel from what the subscription is about, and the gate. These are such topics: ideas, scenes, their quotation restylings, word counts, and a single project. Boards and outlines ask their gates the same way, and also report their version.

**Each subscription runs on its own.** A subscription that falls behind, loses its access, or cannot be filtered ends alone. It receives `ToldDTO::Ended` with the reason: `behind`, `access-lost`, `failed`, `not-there` or `unknown-topic`. The socket and the other subscriptions continue. Everything a subscription receives is sent as a `ToldDTO::Frame` under its id.

**When the session ends or the server closes** (`ending::Ends`, taken from the request), the socket is closed:

- with `SESSION_ENDED` (4401, in `live-contract`) when the session ends. The client then stops asking and signs out.
- with `SERVER_CLOSING` (1001) when the server closes. The client then opens the socket again later.

**A subscription is confirmed once it is following.** *Since 2026-11-04:* an admitted subscription first receives `following` under its id, before anything else. This is sent once it is listening to its channel. Only then does the client read the followed data over REST. It does not read it when it sends the subscription. A `GatedTopic` starts listening before it asks its gate. So a change made while the gate is being asked is sent, and is not lost between the read and the start of listening.

**A feature's tests follow the same way the browser does,** with `testing::Subscriber` (behind the `testing` feature):

- `on`: connect to a server that serves `live_socket`,
- `subscribe`: subscribe under an id,
- `following`: wait for the confirmation,
- `frame`: the next frame,
- `ended`: the reason a subscription ended,
- `told`: whatever came, except `following`,
- `silent` or `nothing`: nothing arrives for a short while (`silent` returns it, `nothing` asserts it),
- `closed_as`: the code the socket was closed with.

A feature decides what its topics are and who may follow them. This library only carries the frames.
