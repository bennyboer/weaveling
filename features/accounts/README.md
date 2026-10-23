# accounts

Who someone is on a Weaveling server: signing up, signing in, and the operator who looks after the instance.

Only a server with `WEAVELING_ACCOUNTS=on` has accounts. Local mode has none, and everything is done by the one person at the keyboard, `Agent::Local`; nothing in this feature runs there.

**`Account` is an aggregate**, event-sourced as every feature is: `SignedUp`, `EmailVerified`, `PasswordReset`, `BecameOperator`, `StoppedBeingOperator`, `Locked`, `Unlocked`. Its events say *that* a password was set or reset, never the password; **the secrets are kept beside the events**, latest only, so an old log holds no old hash. Beside them stand the address an account holds, its password's Argon2id hash, the pending codes, the sessions (by the digest of their token), the tries counted by [`throttling`](../../libraries/throttling) who is appointed an operator, and the tickets for invitations (by the digest of the link's token, with the address invited and until when), each behind a port of its own in `SecretStores`. A ticket is taken once, and ends when taken or when its invitation is withdrawn; an ended ticket stays ended, so an issue told again, as messages may be, opens no link that was closed. The tickets come from the invitations' messages, by the listener `keep-invitation-tickets`, so accounts never ask invitations anything.

**Sessions end by comparison, not by sweeping.** An account records when its sessions last stopped counting, `sessions_since`, set by a new password or a lock; a session begun before that is over wherever it is kept. A locked account is admitted with no session at all.

**Nothing tells a stranger whether an address has an account.** Signing in with an address nobody has does the same Argon2 work and is refused in the same words; signing up with a taken address is answered as any sign-up, and its owner gets a mail; throttles count per address whether or not one stands behind it. A locked account hears so only once its password is right.

**The operator** is whoever entered the code the server logs at start while there is none; an operator may make other accounts operators, the last one always kept, and lists, locks and unlocks accounts. See [the instance](../../ROADMAP.md#milestone-17--accounts-members-and-roles) in the roadmap for the rest of slice 3.

**Crates:** `core` (aggregate, secret stores, catalog port, service, tokens and codes) · `contract` (DTOs, refusals and routing keys, shared with the client) · `adapters/store` (stored-event codec) · `adapters/secrets` (the secret stores in memory, on PostgreSQL and on SQLite) · `adapters/catalog` (the list of accounts) · `adapters/messaging` (event publisher, catalog projector) · `adapters/rest` (routes, the gate, the cookie) · `wiring` · `tests`.
