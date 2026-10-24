# chores

Background work a feature hands over and the service does now and then, apart from any request.

A **`Chore`** has a name, says how often it comes round, `every()`, and does its work once in `carry_out()`, answering how many things it cleared. A feature hands its chores over through `wiring`, as it does its listeners. **`Chores::started(chores)`** gives each chore a round of its own: done at once, then again each time its `every` has passed, so a chore that comes round hourly never waits on one that comes round daily. What a chore cleared is logged; a chore that fails is warned of and done again next time, keeping no other chore from its round. **`stop()`** ends every round at once, not waiting for a slow chore's next turn.

A chore is for upkeep nobody asked for at a given moment, forgetting what is long settled, say. Work that must happen before the server stops is `wiring::Settling` instead.
