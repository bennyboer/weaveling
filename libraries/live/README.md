# live

What lets an open browser hear that something changed, without asking again and again.

**A `Broadcaster<C>` hands frames to whoever hears one of its channels,** and its type says what a channel is: a feature's own, named for what changes and holding whose: `Broadcaster<InvitationChanges>`, invitations to an address; `Broadcaster<IdeaChanges>`, a project's ideas; `Broadcaster<BoardChanges>`, a board; `Broadcaster<MembershipChanges>`, the one channel every member hears, sifted for each. Any `Clone + PartialEq` type that is `Send` and `Sync` is a `Channel`. `tell(channel, frame)` sends one, `hear(channel)` gives a `Hearing` to read them from; nobody listening is no error, and a hearer that falls too far behind is let go rather than hold up the rest. It is in memory and per process, which is all one server needs.

A board's changes are told from what was published, followed by whoever may read the board, and heard in a test as the browser hears them:

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

**The frames come from the outbox, not from the request that made the change.** `BroadcastChangesListener` is an ordinary listener on a feature's events, given the channels a published change is told on (a board's own; a project's own and the list of every project; the one of every membership), and `frame_of` turns what was published into what goes on the wire. So a frame tells only what was stored and published, in the order it was, and a change made by a listener reaches the browser the same way as one made by a click. `Teller` numbers frames itself, for what has no published version to carry.

**A frame has one shape,** `FrameDTO` from [`live-contract`](../live-contract): what it is `of`, its `version`, when it happened, `at`, and its `event`. The server reads every frame it hears as one before telling it, so what it writes and what the client reads cannot drift apart.

**One socket for everything live, `live_socket`,** served at `/live` behind signing in; it is the only way to follow anything. Over it the client subscribes and unsubscribes, `AskedDTO` from `live-contract`: `subscribe` with an id of its own choosing, a topic and what it is about, `unsubscribe` with the id. Subscribing again under an id replaces what it followed; closing the socket ends them all.

**A feature hands over a `Topic`,** one a feature, `named` as the client asks for it. Its `subscribe` is given the agent and what the subscription is `About` (`{ "board": … }`, `{ "project": … }`, `{ "mine": "true" }`) and answers either `Refused` (`NotThere`, told alike whether there is nothing or nothing this agent may follow, or `Failed`, when that could not be told) or a `Subscribed`: the `Hearing`, and as it needs

- **`standing_at`** the version it stands at, told first, so that a change made between reading and subscribing is not lost (boards and outlines begin hearing, then read, and say where they stand);
- **`while_admitted`** an `Admission`, so that it follows only while it may: asked again whenever access may have changed, ended `access-lost` once it no longer may, told the notice `access-changed` when it still may, as its role may be another now;
- **`sifted`** a `Sieve`, which lets each frame through or holds it back, so followers of one channel each hear only their own part of it (the list of projects, one's own members).

**A `Gate` decides who may follow,** about what is followed as text, a project's id or a board's: `admits` answers yes, no, or an error when it cannot tell, and `watch` what wakes when that may have changed (by default nothing). **`admitted(gate, agent, about)`** watches first and then asks, so that no change slips in between, and answers the `Admission`, or `Refused`. **A `GatedTopic`** is the topic of a feature that only asks a gate: its name, its broadcaster, how to read the channel from what it is about, and the gate; ideas, scenes, their quotation restylings, word counts and one project are such topics. Boards and outlines ask theirs the same way, and also say where they stand.

**Each subscription follows on its own,** so one that falls behind, loses its access or cannot be sifted ends alone, `ToldDTO::Ended` saying why (`behind`, `access-lost`, `failed`, `not-there`, `unknown-topic`), and the socket and the others go on. Whatever a subscription hears is told as a `ToldDTO::Frame` under its id. **When the session ends** (`acting::SessionEnd`, put into the request by accounts' gate), the socket is closed with `SESSION_ENDED` (4401, in `live-contract`), and the client asks no more but signs out.

**A feature's tests follow as the browser does,** with `testing::Subscriber` (the `testing` feature): `on` a server serving `live_socket`, `subscribe` under an id, then `frame`, `ended` (why), `told` (whatever came), `silent` or `nothing`, and `closed_as` for the code the socket was closed with.

A feature decides what its topics are and who may follow them; this only carries the frames.
