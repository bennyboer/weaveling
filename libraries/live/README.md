# live

What lets an open browser hear that something changed, without asking again and again.

**A `Broadcaster` hands frames to whoever is following a topic,** a project's board, an outline, the list of projects. `tell(about, frame)` sends one, `hear(about)` gives a `Hearing` to read them from; nobody listening is no error, and a follower that falls too far behind is let go rather than holding up the rest. It is in memory and per process, which is all one server needs.

**The frames come from the outbox, not from the request that made the change.** `BroadcastChangesListener` is an ordinary listener on a feature's events, and `frame_of` turns what was published into what goes on the wire, the version and the event. So a frame tells only what was stored and published, in the order it was, and a change made by a listener reaches the browser the same way as one made by a click.

**`followed_at` and `followed_from` are the routes a browser opens,** a WebSocket upgraded on a path, the topic read from the query. `followed_from` first tells the version the follower stands at, read only after its hearing began, so a change made between loading the page and opening the socket is not lost. `Teller` numbers frames itself, for what has no published version to carry.

**A socket may be sifted.** `streaming_sifted` passes each frame through a `Sieve` first, which lets it through or holds it back, so followers of one topic may each hear only their own part of it; a sieve that cannot tell ends the socket, as falling behind does, rather than skipping a frame the follower may have needed.

A feature decides what its topics are and who may follow them; this only carries the frames.
