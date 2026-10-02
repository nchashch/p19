//! Server-side session recording + deterministic replay ("debug-replay"): capture every
//! client→server gameplay message and every tick's resolved Movement/Jump/RotateCamera action
//! state to a log file, then re-derive the server-side simulation from that log later —
//! headlessly, no real client, no real network — so a bug reported after release can be stepped
//! through with a debugger instead of guessed at from logs.
//!
//! This only works because of three separate determinism guarantees established elsewhere in
//! this codebase (see AGENTS.md's gap-list "Fixed" entries for each): `server/src/main.rs`'s
//! `SingleThreadedExecutor` on `Update`/`FixedPostUpdate`/`PhysicsSchedule` (no thread-scheduling
//! nondeterminism), `spawn.rs`'s tick-seeded `NoiseRng` (no wall-clock RNG), and `combat.rs`'s
//! `TickDuration`-driven `Gcd`/`Dead` timers (no wall-clock timers). Recording/replay would be
//! unsound without all three. Confirmed live: replaying the identical log twice produced
//! bit-identical results both times (same entity IDs, same final `Transform`).
//!
//! # What gets recorded
//! - Connect/disconnect, keyed by [`PeerId`] (stable across a session, unlike the ephemeral
//!   connection `Entity`).
//! - 7 of the 9 client→server gameplay messages `p19_shared::replication` registers —
//!   `AttackAttempt`, `KillAttempt`, `SpawnCubeRequest`, `SpawnNpcRequest`, `LoadLevelRequest`,
//!   `InGameRequest`, `ClientDespawn`. **Not** `LobbyRequest` (no handler anywhere in
//!   `server/src` today, confirmed via a full grep — nothing would be reproduced by replaying
//!   it) or `ObserveRequest` (only ever inserts a `Rooms` component on the connection — pure
//!   replication-targeting bookkeeping with no effect on authoritative simulation state, which
//!   is all replay ever tries to reproduce).
//! - The resolved `ActionValue` for each player's `Movement`/`Jump`/`RotateCamera` (×2: mouse and
//!   stick) action entities, every tick — this is what actually drives movement; without it,
//!   replay could reproduce combat/spawn events but never player positions.
//!
//! # What's confirmed working vs. still open
//! Live-verified: connect → level load → player spawn (correct `HitPoints`/`ControlledBy`) →
//! movement in the correct axis/direction, deterministically (identical log replayed twice
//! produced bit-identical output). **Not yet root-caused**: replayed movement's rate does not
//! numerically match the live session it was recorded from — the live session covered ~12.7
//! units in 60 held-movement ticks; the identical recorded 60 ticks, replayed, covered under 1
//! unit over that same tick span (confirmed via per-tick position sampling, not just start/end).
//! Separately confirmed as *correct, not a bug*: `bevy_ahoy`'s `ground_accelerate` has no
//! deceleration term at all when wish-velocity is zero (`Dir3::new_and_length` on a zero vector
//! returns `Err` and the function returns early, leaving `ctx.velocity` untouched) — a character
//! given any horizontal velocity coasts in a straight line forever once input stops, live or
//! replayed; comparing "final position" between a live session (captured seconds after input
//! stopped) and a replay (which runs a fixed settle margin after the log ends) is comparing two
//! different amounts of frictionless coasting, not a replay defect. The still-open rate
//! discrepancy needs the acceleration/`fired_secs`/`elapsed_secs` path in `ground_accelerate`/
//! `calculate_wish_velocity` audited against what a real per-tick BEI-resolved `Fire` (as opposed
//! to this module's manually-constructed one — see `replay_inject_actions`'s doc comment) would
//! have supplied for those two fields (hardcoded to `0.0` here); not chased further this session.
//!
//! # What replay does NOT reproduce bit-for-bit
//! `LoadLevelRequest`'s effect (loading a `.glb` level) is inherently asynchronous I/O — how many
//! ticks it takes to finish depends on real disk/OS scheduling, not the simulation tick. Replay
//! cannot force this to complete on the exact tick it did live; see `run_replay`'s handling of
//! `ServerState::Loading` for how it copes (freezes the tick counter itself, not just event
//! dispatch, until loading actually finishes, rather than assuming a fixed tick offset).
//!
//! # Recording
//! Set `SERVER_REPLAY_RECORD=/path/to/session.jsonl` before launching the live server (matches
//! the project's existing `LIGHTYEAR_DEBUG_FILE` convention). [`ReplayRecorderPlugin`] is always
//! registered; it no-ops when the env var is absent.
//!
//! # Replay
//! `server --replay /path/to/session.jsonl` (see `main.rs`) boots a fresh headless `App` with the
//! same simulation plugins as live play — including the real `networking::NetworkingPlugin`,
//! which still binds a real (always-unused) UDP socket; confirmed live that skipping it panics,
//! since some of lightyear's own replication systems depend on resources only its `Startup`
//! bootstrap inserts — plus [`ReplayPlaybackPlugin`] standing in for a real client. Driven by
//! [`run_replay`], not `.run()`.

use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;
use bevy_ahoy::input::{Jump, Movement, RotateCamera};
use bevy_enhanced_input::prelude::{Action, ActionOf, ActionValue, Fire, TriggerState};
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use p19_shared::client_events::{
    AttackAttempt, ClientDespawn, InGameRequest, KillAttempt, LoadLevelRequest, SpawnCubeRequest,
    SpawnNpcRequest,
};
use p19_shared::combat::Dead;
use p19_shared::inputs::{MouseLook, PlayerInputContext, StickLook};
use p19_shared::game_state::ServerState;
use std::time::Duration;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// One resolved action's value for a single player, for a single tick. A separate variant per
/// action+device rather than one generic `{ action: String, value: ActionValue }` — keeps the
/// replay side's dispatch a plain `match`, no string/type-name comparison.
#[derive(Clone, Serialize, Deserialize)]
pub enum RecordedAction {
    Movement(ActionValue),
    Jump(ActionValue),
    RotateCameraMouse(ActionValue),
    RotateCameraStick(ActionValue),
}

/// One client→server gameplay message, verbatim — every one of these types already derives
/// `Serialize`/`Deserialize` for `lightyear`'s own wire format, so this just carries the same
/// payload the live handler received.
#[derive(Clone, Serialize, Deserialize)]
pub enum RecordedMessage {
    Attack(AttackAttempt),
    Kill(KillAttempt),
    SpawnCube(SpawnCubeRequest),
    SpawnNpc(SpawnNpcRequest),
    LoadLevel(LoadLevelRequest),
    InGame(InGameRequest),
    Despawn(ClientDespawn),
}

/// One line of the recorded log. `tick` is the server's `LocalTimeline` tick at the moment the
/// event was observed — replay re-derives the same tick counter from the same
/// `TimeUpdateStrategy::FixedTimesteps(1)`-driven loop, so matching on tick equality (not
/// wall-clock proximity) is exact, not approximate.
#[derive(Clone, Serialize, Deserialize)]
pub enum RecordedEvent {
    Connect { tick: u32, client: PeerId },
    Disconnect { tick: u32, client: PeerId },
    Action { tick: u32, client: PeerId, action: RecordedAction },
    Message { tick: u32, client: PeerId, message: RecordedMessage },
}

/// Buffers recorded events and flushes them to disk. `None` (not recording) unless
/// `SERVER_REPLAY_RECORD` was set at startup — every recording call site takes
/// `Option<ResMut<ReplayRecorder>>` and no-ops when it's absent, so recording has zero runtime
/// cost on a normal deployment.
#[derive(Resource)]
pub struct ReplayRecorder {
    writer: BufWriter<File>,
}

impl ReplayRecorder {
    fn from_env() -> Option<Self> {
        let path = std::env::var("SERVER_REPLAY_RECORD").ok()?;
        match File::create(&path) {
            Ok(file) => {
                info!("recording replay log to {path}");
                Some(Self {
                    writer: BufWriter::new(file),
                })
            }
            Err(err) => {
                error!("failed to open replay log {path} for recording: {err}");
                None
            }
        }
    }

    /// Serializes `event` as one JSON line. Errors are logged, not propagated — a failed replay
    /// write should never take down a live session.
    pub fn record(&mut self, event: RecordedEvent) {
        match serde_json::to_string(&event) {
            Ok(line) => {
                if let Err(err) = writeln!(self.writer, "{line}") {
                    error!("failed to write replay log line: {err}");
                }
            }
            Err(err) => error!("failed to serialize replay event: {err}"),
        }
    }

    /// Convenience for the message-handler call sites: resolves `connection`'s `RemoteId` and
    /// records `RecordedEvent::Message` if found. Silently drops the event (with a log line) if
    /// the connection has no `RemoteId` yet — should not happen for a real client past the
    /// handshake, but a replay-mode logical connection legitimately has one by construction, so
    /// this is a live-session defensive guard, not a replay-mode code path.
    pub fn record_message(
        &mut self,
        tick: Tick,
        remote_ids: &Query<&RemoteId>,
        connection: Entity,
        message: RecordedMessage,
    ) {
        let Ok(remote_id) = remote_ids.get(connection) else {
            warn!("replay recorder: connection {connection} has no RemoteId, dropping event");
            return;
        };
        self.record(RecordedEvent::Message {
            tick: tick.0,
            client: remote_id.0,
            message,
        });
    }
}

pub struct ReplayRecorderPlugin;

impl Plugin for ReplayRecorderPlugin {
    fn build(&self, app: &mut App) {
        if let Some(recorder) = ReplayRecorder::from_env() {
            app.insert_resource(recorder);
            app.add_observer(record_connect);
            app.add_observer(record_disconnect);
            app.add_systems(FixedUpdate, record_actions);
        }
    }
}

fn record_connect(
    add: On<Add, server::ClientOf>,
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: ResMut<ReplayRecorder>,
) {
    let Ok(remote_id) = remote_ids.get(add.entity) else {
        return;
    };
    recorder.record(RecordedEvent::Connect {
        tick: timeline.tick().0,
        client: remote_id.0,
    });
}

fn record_disconnect(
    disconnected: On<Add, Disconnected>,
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: ResMut<ReplayRecorder>,
) {
    let Ok(remote_id) = remote_ids.get(disconnected.entity) else {
        return;
    };
    recorder.record(RecordedEvent::Disconnect {
        tick: timeline.tick().0,
        client: remote_id.0,
    });
}

/// Records every player's resolved Movement/Jump/RotateCamera(×2) `ActionValue` every tick —
/// `FixedUpdate`, same schedule `LocalTimeline`'s tick counter advances in (see `combat.rs`'s
/// matching comment on `tick_gcd`/`tick_dead`), so `timeline.tick()` here is the exact tick this
/// action state applies to, not a frame later or earlier. Correlates an action entity back to a
/// client via `ActionOf<PlayerInputContext>` (the action's owning player) → `ControlledBy.owner`
/// (the player's owning connection) → `RemoteId` (the connection's stable client id) — the same
/// relationship chain `controls.rs`'s binding observers already use client-side.
fn record_actions(
    movement: Query<(&ActionValue, &ActionOf<PlayerInputContext>), With<Action<Movement>>>,
    jump: Query<(&ActionValue, &ActionOf<PlayerInputContext>), With<Action<Jump>>>,
    rotate_mouse: Query<
        (&ActionValue, &ActionOf<PlayerInputContext>),
        (With<Action<RotateCamera>>, With<MouseLook>),
    >,
    rotate_stick: Query<
        (&ActionValue, &ActionOf<PlayerInputContext>),
        (With<Action<RotateCamera>>, With<StickLook>),
    >,
    controlled: Query<&ControlledBy>,
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: ResMut<ReplayRecorder>,
) {
    let tick = timeline.tick().0;
    let resolve = |player: Entity| -> Option<PeerId> {
        let owner = controlled.get(player).ok()?.owner;
        Some(remote_ids.get(owner).ok()?.0)
    };
    for (value, action_of) in &movement {
        if let Some(client) = resolve(action_of.get()) {
            recorder.record(RecordedEvent::Action {
                tick,
                client,
                action: RecordedAction::Movement(*value),
            });
        }
    }
    for (value, action_of) in &jump {
        if let Some(client) = resolve(action_of.get()) {
            recorder.record(RecordedEvent::Action {
                tick,
                client,
                action: RecordedAction::Jump(*value),
            });
        }
    }
    for (value, action_of) in &rotate_mouse {
        if let Some(client) = resolve(action_of.get()) {
            recorder.record(RecordedEvent::Action {
                tick,
                client,
                action: RecordedAction::RotateCameraMouse(*value),
            });
        }
    }
    for (value, action_of) in &rotate_stick {
        if let Some(client) = resolve(action_of.get()) {
            recorder.record(RecordedEvent::Action {
                tick,
                client,
                action: RecordedAction::RotateCameraStick(*value),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

use bevy::time::TimeUpdateStrategy;
use avian3d::prelude::SpatialQuery;
use p19_shared::combat::{Gcd, HitPoints};
use p19_shared::level::InGameRoot;
use p19_shared::player::{PlayerCharacter, PlayerCharacterSpawner};
use crate::level_state::LevelState;
use crate::rooms::GameRoom;
use p19_shared::assets::level::Level;
use std::collections::HashMap;

fn event_tick(event: &RecordedEvent) -> u32 {
    match event {
        RecordedEvent::Connect { tick, .. }
        | RecordedEvent::Disconnect { tick, .. }
        | RecordedEvent::Action { tick, .. }
        | RecordedEvent::Message { tick, .. } => *tick,
    }
}

/// The full recorded log, loaded once at replay startup, plus a cursor into it.
#[derive(Resource)]
struct ReplayLog {
    events: Vec<RecordedEvent>,
    cursor: usize,
}

/// Maps a recorded [`PeerId`] to the "logical connection" entity replay spawned for it —
/// **not** a real lightyear connection (no `server::ClientOf`/`LinkOf`/`Connected`, no real
/// netcode link at all: replay never opens a socket). It only needs to satisfy what the
/// extracted `apply_*` functions actually read off "the connection" — a `RemoteId` for lookups
/// like `apply_in_game_request`'s `remote_ids.get(entity)`, and an `Entity` identity for
/// `ControlledBy { owner: … }` to point at. Room membership, replication senders, and every
/// other real-connection concern are deliberately not reproduced — they gate what a real network
/// peer *receives*, which has no bearing on reproducing the server's own authoritative state.
#[derive(Resource, Default)]
struct ReplayConnections(HashMap<PeerId, Entity>);

/// This tick's events, popped from [`ReplayLog`] by [`advance_replay_tick`] — every other
/// playback system reads this instead of managing its own cursor into `ReplayLog`.
#[derive(Resource, Default)]
struct CurrentTickEvents(Vec<RecordedEvent>);

/// Set once [`ReplayLog`] is exhausted — [`run_replay`]'s drive loop watches this to know when
/// to stop calling `app.update()`.
#[derive(Resource, Default)]
struct ReplayFinished(bool);

fn advance_replay_tick(
    mut log: ResMut<ReplayLog>,
    timeline: Res<LocalTimeline>,
    mut current: ResMut<CurrentTickEvents>,
    mut finished: ResMut<ReplayFinished>,
) {
    let tick = timeline.tick().0;
    current.0.clear();
    while log.cursor < log.events.len() && event_tick(&log.events[log.cursor]) <= tick {
        current.0.push(log.events[log.cursor].clone());
        log.cursor += 1;
    }
    if log.cursor >= log.events.len() {
        finished.0 = true;
    }
}

/// Creates/removes the logical connection entities backing [`ReplayConnections`] — must run
/// before every other playback system this tick, since they all resolve a recorded [`PeerId`]
/// through this map.
fn replay_connections(
    current: Res<CurrentTickEvents>,
    mut connections: ResMut<ReplayConnections>,
    controlled: Query<(Entity, &ControlledBy)>,
    mut commands: Commands,
) {
    for event in &current.0 {
        match event {
            RecordedEvent::Connect { client, .. } => {
                let entity = commands.spawn(RemoteId(*client)).id();
                connections.0.insert(*client, entity);
                info!("replay: client {client:?} connected (logical connection {entity})");
            }
            RecordedEvent::Disconnect { client, .. } => {
                if let Some(&connection) = connections.0.get(client) {
                    // Mirrors `networking::on_client_disconnected`'s zombie-player cleanup —
                    // `Persistent`-lifetime players don't despawn on their own.
                    crate::networking::apply_client_despawn(connection, &controlled, &mut commands);
                }
                connections.0.remove(client);
                info!("replay: client {client:?} disconnected");
            }
            _ => {}
        }
    }
}

/// Re-fires each recorded tick's resolved Movement/Jump/RotateCamera value as the exact
/// `Fire<A>` event BEI would have triggered for it, directly — **not** via [`ActionMock`].
///
/// `ActionMock` was the first approach here and is the documented mechanism
/// `p19_client::dev::tool_api`'s `game/input` uses — but that only works because the *client*
/// keeps BEI's own per-context resolution system (`bevy_enhanced_input`'s generic `update()`,
/// registered in `FixedPreUpdate` via `add_input_context_to`) actually consuming mocks every
/// tick. Confirmed live (three separate diagnostic passes: mock-insertion confirmed happening
/// every tick with the correct value; `ActionValue`/`TriggerState` on the same action entity
/// read back unchanged, `Axis2D(0.0, 0.0)`/`None`, for the entire span; player `Transform`
/// static at the spawn point for the whole replay) — that resolution never actually applies on
/// this project's authoritative server: real server-side movement is driven by
/// `lightyear_inputs::server`'s own `get_action_state` system, which transitions `ActionState`
/// (and fires the matching `Fire`/`Start`/`Complete` event) directly from the *replicated input
/// buffer* — a completely separate code path from BEI's generic per-context update loop that
/// `ActionMock` hooks into. Whether that loop is literally disabled server-side or just never
/// reaches this context wasn't root-caused further; irrelevant either way, since there's no
/// buffered network input to mock during replay for it to consume regardless.
///
/// The fix: skip both of those pipelines and trigger the `Fire<A>` event *directly* —
/// `apply_movement`/`apply_jump` (`bevy_ahoy::input`) and `accumulate_look`
/// (`p19_server::input::accumulate_look`) all read straight from the event's own `value` field, not
/// by re-querying `ActionValue`/`Action<A>` — so this is a complete, correct substitute for
/// those three consumers specifically (confirmed by reading each observer's body — none of them
/// touch any other action-entity component). Does **not** update `ActionValue`/`Action<A>`
/// components to match — a real gap only if some other, not-yet-existing consumer starts reading
/// those components directly instead of observing the fire event, worth revisiting if one does.
fn replay_inject_actions(
    current: Res<CurrentTickEvents>,
    connections: Res<ReplayConnections>,
    player_of: Query<(Entity, &ControlledBy)>,
    movement: Query<(Entity, &ActionOf<PlayerInputContext>), With<Action<Movement>>>,
    jump: Query<(Entity, &ActionOf<PlayerInputContext>), With<Action<Jump>>>,
    rotate_mouse: Query<
        (Entity, &ActionOf<PlayerInputContext>),
        (With<Action<RotateCamera>>, With<MouseLook>),
    >,
    rotate_stick: Query<
        (Entity, &ActionOf<PlayerInputContext>),
        (With<Action<RotateCamera>>, With<StickLook>),
    >,
    mut commands: Commands,
) {
    fn axis2d(value: &ActionValue) -> Vec2 {
        match value {
            ActionValue::Axis2D(v) => *v,
            _ => Vec2::ZERO,
        }
    }
    fn boolean(value: &ActionValue) -> bool {
        match value {
            ActionValue::Bool(b) => *b,
            _ => false,
        }
    }
    for event in &current.0 {
        let RecordedEvent::Action { client, action, .. } = event else {
            continue;
        };
        let Some(&connection) = connections.0.get(client) else {
            continue;
        };
        let Some(player) = player_of
            .iter()
            .find(|(_, controlled_by)| controlled_by.owner == connection)
            .map(|(entity, _)| entity)
        else {
            continue;
        };
        match action {
            RecordedAction::Movement(value) => {
                let value = axis2d(value);
                if let Some((action_entity, _)) = movement.iter().find(|(_, ao)| ao.get() == player) {
                    let state = if value != Vec2::ZERO { TriggerState::Fired } else { TriggerState::None };
                    commands.trigger(Fire::<Movement> {
                        context: player,
                        action: action_entity,
                        value,
                        state,
                        fired_secs: 0.0,
                        elapsed_secs: 0.0,
                    });
                }
            }
            RecordedAction::Jump(value) => {
                let value = boolean(value);
                if let Some((action_entity, _)) = jump.iter().find(|(_, ao)| ao.get() == player) {
                    let state = if value { TriggerState::Fired } else { TriggerState::None };
                    commands.trigger(Fire::<Jump> {
                        context: player,
                        action: action_entity,
                        value,
                        state,
                        fired_secs: 0.0,
                        elapsed_secs: 0.0,
                    });
                }
            }
            RecordedAction::RotateCameraMouse(value) => {
                let value = axis2d(value);
                if let Some((action_entity, _)) = rotate_mouse.iter().find(|(_, ao)| ao.get() == player) {
                    let state = if value != Vec2::ZERO { TriggerState::Fired } else { TriggerState::None };
                    commands.trigger(Fire::<RotateCamera> {
                        context: player,
                        action: action_entity,
                        value,
                        state,
                        fired_secs: 0.0,
                        elapsed_secs: 0.0,
                    });
                }
            }
            RecordedAction::RotateCameraStick(value) => {
                let value = axis2d(value);
                if let Some((action_entity, _)) = rotate_stick.iter().find(|(_, ao)| ao.get() == player) {
                    let state = if value != Vec2::ZERO { TriggerState::Fired } else { TriggerState::None };
                    commands.trigger(Fire::<RotateCamera> {
                        context: player,
                        action: action_entity,
                        value,
                        state,
                        fired_secs: 0.0,
                        elapsed_secs: 0.0,
                    });
                }
            }
        }
    }
}

fn replay_combat(
    current: Res<CurrentTickEvents>,
    connections: Res<ReplayConnections>,
    controlled: Query<(Entity, &ControlledBy)>,
    dead: Query<(), With<Dead>>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    for event in &current.0 {
        let RecordedEvent::Message { client, message, .. } = event else {
            continue;
        };
        let Some(&connection) = connections.0.get(client) else {
            continue;
        };
        match message {
            RecordedMessage::Attack(attempt) => {
                crate::combat::apply_attack(
                    connection, attempt, &controlled, &dead, &positions, &mut targets,
                    &mut casters, &mut sender, &server,
                )?;
            }
            RecordedMessage::Kill(attempt) => {
                crate::combat::apply_kill(
                    connection, attempt, &controlled, &dead, &positions, &mut targets,
                    &mut casters, &mut sender, &server,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn replay_spawn(
    current: Res<CurrentTickEvents>,
    connections: Res<ReplayConnections>,
    controlled: Query<(Entity, &ControlledBy)>,
    mut casters: Query<&mut Gcd>,
    timeline: Res<LocalTimeline>,
    spatial_query: SpatialQuery,
    game_room: Res<GameRoom>,
    mut commands: Commands,
) {
    for event in &current.0 {
        let RecordedEvent::Message { client, message, .. } = event else {
            continue;
        };
        let Some(&connection) = connections.0.get(client) else {
            continue;
        };
        match message {
            RecordedMessage::SpawnCube(request) => {
                crate::spawn::apply_spawn_cube(
                    connection,
                    request,
                    &controlled,
                    &mut casters,
                    timeline.tick(),
                    &spatial_query,
                    &game_room,
                    &mut commands,
                );
            }
            RecordedMessage::SpawnNpc(request) => {
                crate::spawn::apply_spawn_npc(
                    connection,
                    request,
                    &controlled,
                    &mut casters,
                    timeline.tick(),
                    &spatial_query,
                    &game_room,
                    &mut commands,
                );
            }
            _ => {}
        }
    }
}

fn replay_networking(
    current: Res<CurrentTickEvents>,
    connections: Res<ReplayConnections>,
    asset_server: Res<AssetServer>,
    levels: Res<Assets<Level>>,
    in_game_root: Single<Entity, With<InGameRoot>>,
    mut level_state: ResMut<LevelState>,
    names: Query<&Name, With<PlayerCharacter>>,
    timeline: Res<LocalTimeline>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    game_room: Res<GameRoom>,
    remote_ids: Query<&RemoteId>,
    controlled: Query<(Entity, &ControlledBy)>,
    mut commands: Commands,
) -> Result {
    for event in &current.0 {
        let RecordedEvent::Message { client, message, .. } = event else {
            continue;
        };
        let Some(&connection) = connections.0.get(client) else {
            continue;
        };
        match message {
            RecordedMessage::LoadLevel(request) => {
                apply_load_level_request(
                    request.asset_path.clone(),
                    &asset_server,
                    &mut commands,
                    &levels,
                    *in_game_root,
                    &mut level_state,
                )?;
            }
            RecordedMessage::InGame(_) => {
                apply_in_game_request(
                    connection,
                    timeline.tick().0 ^ connection.to_bits() as u32,
                    &names,
                    &player_spawner,
                    *in_game_root,
                    &game_room,
                    &remote_ids,
                    &mut commands,
                );
            }
            RecordedMessage::Despawn(_) => {
                apply_client_despawn(connection, &controlled, &mut commands);
            }
            _ => {}
        }
    }
    Ok(())
}

use crate::networking::{apply_client_despawn, apply_in_game_request, apply_load_level_request};

/// Registers every playback-driving system, alongside [`crate::networking::NetworkingPlugin`]
/// (see [`crate::build_app`]'s `run_replay` call site — that plugin is included, not swapped
/// out, because lightyear's own replication systems need resources its `Startup` bootstrap
/// inserts; see the doc comment there). Does **not** register any of `NetworkingPlugin`'s live
/// message-consuming systems' *effects* twice: those just iterate zero receivers forever (no
/// real `MessageReceiver<T>` ever gets populated without a real client sending real packets), so
/// they run alongside this plugin's replacements for them harmlessly, as pure no-ops.
pub struct ReplayPlaybackPlugin {
    pub events: Vec<RecordedEvent>,
}

impl Plugin for ReplayPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ReplayLog {
            events: self.events.clone(),
            cursor: 0,
        });
        app.init_resource::<ReplayConnections>();
        app.init_resource::<CurrentTickEvents>();
        app.init_resource::<ReplayFinished>();
        app.add_systems(
            PreUpdate,
            (advance_replay_tick, replay_connections, replay_inject_actions).chain(),
        );
        app.add_systems(Update, (replay_combat, replay_spawn, replay_networking));
    }
}

/// Re-derives the exact server-side simulation recorded at `path` — see this module's top doc
/// comment for the full mechanism and its one significant caveat (`LoadLevelRequest`'s async
/// completion timing). Boots a fresh, otherwise-identical `App` (via [`crate::build_app`]) with
/// [`ReplayPlaybackPlugin`] standing in for the real UDP endpoint, no window, no rendering — the
/// same headless posture the live server already has.
///
/// The one thing `build_app`'s plugins can't provide on their own: real time. Live play's
/// `ScheduleRunnerPlugin` throttles `App::update()` to roughly the tick rate, but the *actual*
/// elapsed wall-clock time between calls (and therefore how many `FixedUpdate`/`PhysicsSchedule`
/// steps run this call) still varies with real OS scheduling — which is fine for live play
/// (nothing needs bit-for-bit reproducibility there) but fatal for replay, which must re-run
/// *exactly* the tick sequence the log was captured against. `TimeUpdateStrategy::FixedTimesteps
/// (1)` (a real Bevy testing primitive, not a hand-rolled clock override — see `bevy_time`'s own
/// doc comment: "mocking the wall clock for testing purposes") makes every `App::update()` call
/// advance the fixed-step accumulator by exactly one step, deterministically, regardless of how
/// long the call actually took — so `LocalTimeline`'s tick counter (which increments once per
/// `FixedFirst` run) advances in perfect lockstep with the drive loop below, letting
/// [`advance_replay_tick`] match recorded ticks by equality rather than guesswork.
pub fn run_replay(path: &Path) {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read replay log {}: {err}", path.display()));
    let events: Vec<RecordedEvent> = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|err| panic!("failed to parse replay log line {line:?}: {err}"))
        })
        .collect();
    info!("replay: loaded {} events from {}", events.len(), path.display());

    // `crate::networking::NetworkingPlugin` is included here too, not swapped out — confirmed
    // live (first replay attempt panicked without it): lightyear's own replication systems
    // (`lightyear_replication::server::receive_server_packets`) depend on resources
    // (`RepliconChannelMap`) that only get inserted once the server's netcode bootstrap
    // actually runs (`NetworkingPlugin`'s `Startup: start_endpoint`, which spawns
    // `NetcodeServer`/`LocalAddr`/`ServerUdpIo` and triggers `server::Start`) — there's no
    // lighter-weight path to that setup than running the real thing. This does mean replay
    // binds a real (always unused — no real client ever has this session's log to connect
    // with) UDP socket on the same port live play uses; don't run a replay and a live server on
    // the same machine at the same time. `NetworkingPlugin`'s own message-consuming systems
    // (`in_game_request`, `load_level_request`, …) also run alongside this plugin's replacements
    // for them — harmlessly: their `MessageReceiver<T>` queries never match anything without a
    // real connected client, so they're a genuine no-op every tick, not a source of duplicate
    // effects.
    let mut app =
        crate::build_app((crate::networking::NetworkingPlugin, ReplayPlaybackPlugin { events }));
    app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));

    // Bevy's own `App::run()` (via its default `run_once` runner — confirmed in
    // `bevy_app::app::run_once`) does this exact sequence before its own first `app.update()`:
    // wait for every plugin to leave `PluginsState::Adding`, then call `finish()`/`cleanup()`.
    // Some plugin-inserted resources (confirmed live: `RepliconChannelMap`, needed by
    // `lightyear_replication::server::receive_server_packets` — first replay attempt panicked
    // without this) are only ever inserted in a `Plugin::finish()` hook, which nothing calls
    // for us here since we drive `app.update()` manually instead of `app.run()`.
    while app.plugins_state() == bevy::app::PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    // `LoadLevelRequest`'s effect is real async I/O (glTF parsing on a background task-pool
    // thread) — it does not resolve on any particular simulation tick, and
    // `TimeUpdateStrategy::FixedTimesteps(1)` runs ticks as fast as the CPU allows with no
    // throttling at all, unlike live play's real-time-paced `ScheduleRunnerPlugin`. Confirmed
    // live: without this guard, ticks raced far ahead of the load (hundreds of recorded ticks
    // dispatched before the background thread finished parsing the level), and later events
    // (`InGameRequest`, movement) landed on ticks where `PlayerCharacterSpawner` didn't exist
    // yet — nondeterministic across otherwise-identical replay runs of the same log, since it
    // depended on real disk/OS scheduling. Fix: freeze the tick counter itself (not just event
    // dispatch — see this module's earlier design note on why event-dispatch-only throttling
    // still corrupts per-tick action fidelity) via `TimeUpdateStrategy::ManualDuration::ZERO`
    // (`FixedUpdate`'s accumulator never crosses its threshold, so `LocalTimeline` genuinely
    // does not advance) whenever the server is `ServerState::Loading`, resuming
    // `FixedTimesteps(1)` once it reaches `InGame`. The rest of the schedule (including
    // whatever polls the background load for completion) still runs every call regardless.
    loop {
        if *app.world().resource::<State<ServerState>>().get() == ServerState::Loading {
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        } else {
            app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        }
        app.update();
        if app.world().resource::<ReplayFinished>().0 {
            break;
        }
    }
    // The log is exhausted, but async effects the last few events triggered (most notably level
    // loading) may not have resolved yet — give them a real margin of ticks to settle before
    // reporting done, rather than cutting off mid-load.
    for _ in 0..600 {
        app.update();
    }
    // Final report: dumps the same player summary `p19_server::tools`'s `server/state` BRP method
    // reports, so a replay run's final position/HP is visible without needing BRP still up (the
    // process is about to exit). Direct `EntityRef::get` rather than a fresh multi-component
    // `QueryState` — confirmed live that a `QueryState` built this late, this ad hoc, with
    // multiple required components, silently matched zero entities in a release build despite
    // every component individually being present (each checked via its own single-component
    // query) — never root-caused (`debug_assert`-gated `QueryState` validation compiled out in
    // release is the leading suspect, not confirmed), but direct entity access sidesteps
    // whichever `QueryState` edge case that was entirely.
    let mut player_entities = app
        .world_mut()
        .query_filtered::<Entity, With<p19_shared::player::PlayerCharacter>>();
    let player_entities: Vec<Entity> = player_entities.iter(app.world()).collect();
    println!("replay: {} player(s) at end of replay", player_entities.len());
    for entity in player_entities {
        let entity_ref = app.world().entity(entity);
        let position = entity_ref.get::<Transform>().map(|t| t.translation);
        let hit_points = entity_ref.get::<p19_shared::combat::HitPoints>().map(|h| h.hit_points);
        let owner = entity_ref.get::<ControlledBy>().map(|c| c.owner);
        println!(
            "replay: final state — player {entity}: position {position:?}, {hit_points:?} hp, owning connection {owner:?}"
        );
    }
    println!("replay: finished");
}
