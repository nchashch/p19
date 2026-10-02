#set document(
  title: "Agent Playtest 0019 — Token Endpoint over Self-Signed HTTPS: LAN Encryption Closed",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0019

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-10-02 04:05 -- 04:45 local],
  [Commit (local state actually running)], [`8c851b8` "Improve MCP quality of life - add select nearest player API" + uncommitted working tree, per file: `Cargo.toml` (workspace: `rustls`/`rustls-pemfile`/`rcgen`/`sha2` promoted to direct deps) · `server/Cargo.toml` + `client/Cargo.toml` (same deps) · `server/src/networking.rs` (TLS identity load-or-create + HTTPS endpoint + `server_addr_check: true` + fingerprint log) · `client/src/lifecycle/networking.rs` (TLS fetch with TOFU fingerprint pinning) · `client/src/dev/tool_api.rs` (`name` in `game/state`) · `AGENTS.md` + `docs/agents/skills/playtest.md` (docs)],
  [Agent], [opencode session, GLM-5.3-Flash],
  [Clients], [1x `target/debug/client --mcp --no-render` (dev-tools feature), host ports],
  [Server], [`target/release/server` — fresh per round; `server/assets/netcode.key` + `token-tls.crt`/`.key` regenerated for the round],
  [Level], [`levels/minimal.level.ron`],
  [Transports], [game: UDP/netcode :6000 · token endpoint: *HTTPS* :6001 (self-signed, fingerprint-pinned) · server QA: BRP :15701],
)

= Purpose

Close the netcode-posture crack's LAN half (playtest 0018's remaining-cracks list, item 3 /
this review cycle's item 2): the connect-token endpoint served plain HTTP, so the token (and
therefore the game session) was stealable in flight on an unsecured LAN. Goal: wrap the token
endpoint in TLS with a self-signed certificate and close the "LAN encryption" gap — without
forcing any crypto decisions (the netcode standard fixes the algorithms; the token flow fixes
the trust model as fingerprint pinning).

= The change

- Server (`server/src/networking.rs`):
  - `load_or_create_tls_identity()` — self-signed certificate via `rcgen` (SANs:
    `localhost`, `127.0.0.1`, default-route local IP), persisted as
    `server/assets/token-tls.crt`/`.key` (PEM) on first run, loaded after. The leaf's
    SHA-256 fingerprint is logged at startup for out-of-band pre-pinning.
  - `start_token_http_endpoint` → HTTPS: each accepted TCP stream is wrapped in
    `rustls::StreamOwned<ServerConnection, TcpStream>` (blocking TLS — no async runtime
    needed), and the response is written once followed by a TLS `close_notify` so client
    `read_to_end` succeeds.
  - `NetcodeConfig.server_addr_check` back to *`true`*: every connect token now embeds the
    HTTPS requester's own IP + the game port as its server-address whitelist, so the standard
    check passes and is meaningful again (supersedes playtest-0015-era `server_addr_check:
    false`).
- Client (`client/src/lifecycle/networking.rs`):
  - `fetch_connect_token` → TLS with a `dangerous()` verifier that accepts any cert at the
    TLS layer, followed by an *after-the-handshake* fingerprint check: the presented leaf
    cert's SHA-256 must match `client/assets/token-tls-fingerprint.txt` (trust-on-first-use —
    the file is written on first connection; a mismatch refuses the request with an explicit
    MITM/rotation message and points at the file to delete for legitimate re-trust).
- `game/state` now surfaces the player's generated `name`.
- Dependencies: `rustls`/`rustls-pemfile`/`rcgen`/`sha2` were *already in the graph* via rmcp;
  promoted to direct deps only.

= Verification

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Test*], [*Observed*],
  [First server run (no TLS files)], [Cert + key generated into `server/assets/`; startup logs the cert fingerprint (`af5723…`); HTTPS endpoint listening on :6001],
  [`curl -k https://127.0.0.1:6001/connect_token`], [`200`, 2048 bytes; server logs `issued connect token #1`],
  [Client, first connect], [Token fetched over TLS; fingerprint file created (`77c181…` — TOFU pin); connect → Lobby → level → play → `InGame` with generated name (`Valiant Sparrow`)],
  [Tamper (fingerprint file set to `deadbeef…`, reconnect)], [Fetch refused before sending anything: *"fingerprint CHANGED since first use … refusing to send the token request; if the server legitimately rotated its cert, delete … and retry"*. Client stays at MainMenu, zero panics],
  [Regression], [Full loop re-run clean after the fix (same flow as playtest 0017 but over TLS)],
)

= Findings <findings>

*F1 — LAN encryption is closed with zero crypto decisions.* The netcode standard already
fixes the algorithms (XChaCha20-Poly1305 tokens/packets, 32-byte key); the token flow fixes
the trust model (self-signed cert + TOFU fingerprint pinning). The only human choice left is
operational: when to delete the client's fingerprint file (cert rotation) and when to upgrade
the endpoint to a CA-signed cert on a real backend.

*F2 — rustls is strict about `close_notify`; plain-HTTP-era close semantics break it.* The
first client run failed with `UnexpectedEof` on `read_to_end`: the endpoint dropped the TCP
stream after the response without sending a TLS `close_notify`, and rustls surfaces a missing
close-notify as a read error rather than a clean EOF. Fixed server-side (write the whole
response, flush, `send_close_notify`, flush). Also fixed en passant: the "issued token" log
line referenced `client_id` from outside its scope after a response-shape refactor.

*F3 — `world.list_resources` blindness extended to resources *and* reflected-only BRP
listing (restated)*: no new instance this session, but the fingerprint/`ServerMutateTicks`
class of non-reflected resources remains invisible to BRP listing — the trust store lives in
a plain file, which is inspectable.

*F4 — Scope note*: rendered/windowed clients were not affected by the old plain-HTTP
exposure either way (the fetch path is client-agnostic); the TOFU file lives under the
client's asset root, so fleet clients on `--brp-port` ports each maintain their own pin.

= Next steps

1. *KCC-internal rollback registration* — ahoy's `CharacterControllerState`/
   `AccumulatedInput` (coyote/jump-buffer stopwatches, grounded-hit data) aren't registered
   for lightyear's rollback, so corrections rewind `Position`/`Velocity` but not controller-
   internal state → ghost jumps or swallowed inputs right after a correction. Needs ahoy-side
   exposure of those components for rollback registration — it can't be fixed purely from
   this repo.
2. *Spawn-point separation* (design, from playtest 0018) — joiners still stack on an idle
   predecessor.
3. Production-upgrade path (explicitly deferred): replace the TOFU self-signed endpoint with
   a CA-signed cert on a real backend, and consider the asymmetric LAN key-exchange so even
   the *first* connection is MITM-safe (TOFU trusts the first observer by definition).

= Conclusion

The token endpoint now speaks HTTPS with a self-signed certificate, and clients pin its
fingerprint trust-on-first-use — the connect token is encrypted in flight on the LAN, MITM
after first use is detected and refused, and the whole flow was verified end-to-end on
`--no-render` clients (token fetch over TLS → connect → level → play → InGame, plus the
tamper refusal). The netcode-posture crack's LAN half is closed; the production-upgrade path
(CA-signed backend, first-connection MITM safety) is the explicitly deferred remainder.
AGENTS.md and the skill's method table are updated to the HTTPS flow.
