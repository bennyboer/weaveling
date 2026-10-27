# live

What lets an open browser hear that something changed, without asking again and again.

**A `Broadcaster` hands frames to whoever is following a topic,** a project's board, an outline, the list of projects. `tell(about, frame)` sends one, `hear(about)` gives a `Hearing` to read them from; nobody listening is no error, and a follower that falls too far behind is let go rather than holding up the rest. It is in memory and per process, which is all one server needs.

**The frames come from the outbox, not from the request that made the change.** `BroadcastChangesListener` is an ordinary listener on a feature's events, and `frame_of` turns what was published into what goes on the wire, the version and the event. So a frame tells only what was stored and published, in the order it was, and a change made by a listener reaches the browser the same way as one made by a click.

**`followed_at` is the route a browser opens,** a WebSocket upgraded on a path, the topic read from the query. A feature whose route must first read what is followed, to ask whether the follower may and to say where it stands, writes its own handler: it begins hearing, then reads, and answers **`streaming_from(version)`**, which tells that version first, so a change made between loading the page and opening the socket is not lost; boards and outlines do so. `Teller` numbers frames itself, for what has no published version to carry.

**A frame has one shape,** `FrameDTO` from [`live-contract`](../live-contract), so what the server writes and the client reads cannot drift apart.

**A socket may be sifted.** `streaming_sifted` reads each frame and passes it through a `Sieve` first, ending the socket on one it cannot read, which lets it through or holds it back, so followers of one topic may each hear only their own part of it; a sieve that cannot tell ends the socket, as falling behind does, rather than skipping a frame the follower may have needed.

**A socket may be guarded.** `streaming_if_admitted` asks a `Gate` once on connecting whether the follower may hear the topic, answering 404 when not and 500 when it cannot tell, and `followed_if` is a route doing so; each feature's REST adapter has its own gate, `IdeasGate` and the like, asking its core whether the agent may follow. **A follower is let in while admitted.** A gate may also `watch` its topic; a socket opened through `followed_if` subscribes to that watch before asking, and asks the gate again whenever it wakes, closing with `NO_LONGER_ADMITTED` (4403, in `live-contract`) when the answer is no or cannot be had, and following on when it is yes, telling the follower first with a `NoticeDTO` of `ACCESS_CHANGED`, as its role may be another now. A gate that does not watch keeps the default, which never wakes.

A feature decides what its topics are and who may follow them; this only carries the frames.
