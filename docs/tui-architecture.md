# TUI Architecture

Elm Architecture adapted for a ratatui terminal client.

## Purpose

Define the runtime loop, state architecture, and message routing patterns for
the TUI crate. These are framework-level conventions; page-specific details
belong in their own docs.

## Runtime

The main loop follows standard TEA:

```
loop {
    draw:       view(&model, &mut view_state, f)

    key:        handle_key_event(&mut view_state, key) → Option<Message>
                  └─ if Some(msg): update(model, msg) → (Model, Cmd)
                  └─ if None:     (navigation mutated ViewState directly)

    async:      rx.try_recv() → update(model, msg) → (Model, Cmd)
}
```

A single `Runtime<Message>` (see `runtime.rs`) owns the tokio channel. Async
commands (`Cmd`) are `Pin<Box<dyn Future<Output = Msg>>>` futures spawned onto
tokio; their results flow back through the same `update` function.

## Model vs ViewState

|            | Model                     | ViewState                                                      |
| ---------- | ------------------------- | -------------------------------------------------------------- |
| Mutable in | `update` only             | `view` and `handle_key_event`                                  |
| Holds      | Business data, page state | Ratatui widget states (`TableState`, `ListState`, focus, etc.) |
| Reference  | `&` (immutable)           | `&mut`                                                         |

Ratatui requires mutable access to widget states during rendering (e.g.,
`f.render_stateful_widget(…, &mut table_state)`). Passing `&mut Model` to
`view` would let rendering code mutate business data — violating TEA's
single-source-of-truth for state transitions.

`ViewState` solves this: the runtime owns it alongside `Model`, passes it to
`view` as `&mut`, and it never enters `update`.

## Message routing

Two input sources feed `update`:

1. **Key events** — `handle_key_event(&mut ViewState, key) → Option<Message>`.
   Navigation that only affects widget state mutates `ViewState` directly and
   returns `None`. Actions that need the async command pipeline return messages.

2. **Completed async commands** — `Cmd` futures that produce `Message` values,
   fed back through the runtime channel.

`update` is the single path for all business state transitions. Two call sites
for it in the main loop (keys + async) is standard TEA.

## Per-page conventions

Each page module exposes the same function signatures so pages are
interchangeable from the top-level `Model` / `handle_key_event` / `view`:

```rust
pub fn init() -> (Model, Cmd<Message>, ViewState);

pub fn update(model: Model, msg: Message) -> (Model, Cmd<Message>);

pub fn view(model: &Model, view_state: &mut ViewState, area: Rect, f: &mut Frame);

pub fn handle_key(key: KeyCode, view_state: &mut ViewState) -> Option<Message>;
```

The top-level `handle_key_event` delegates to the active page's `handle_key`
after checking global bindings. Global bindings always take priority and
short-circuit.

## Design decisions

1. **ViewState separation** — `TableState` / `ListState` / focus are rendering
   concerns, not business state. Mirroring them in `Model` creates two sources
   of truth and requires explicit sync. A separate struct owned by the runtime
   keeps `Model` pure and avoids sync.

2. **Navigation mutates ViewState directly** — routing widget navigation
   through `Message → update` would turn ephemeral UI interactions into
   business state transitions. Widget position is not domain data; it belongs
   at the event boundary.

3. **`handle_key_event` returns `Option<Message>`** — keeps the function pure:
   it maps raw input to domain messages. All state transitions go through
   `update`. Returning `Cmd` directly would mix synchronous UI control with
   async effects, and would require access to `&mut Model` to invoke effectful
   commands.

4. **Both `view` and `handle_key_event` share `&mut ViewState`** — ratatui
   widget states need mutable access in both contexts. The borrow is
   short-lived (per frame, per key event) and the two never overlap in the main
   loop.

## Key files

| File                 | Role                                                           |
| -------------------- | -------------------------------------------------------------- |
| `tui/src/main.rs`    | CLI, init, main loop                                           |
| `tui/src/app.rs`     | Top-level Model/Message/update/view, key routing, commands     |
| `tui/src/runtime.rs` | `Cmd` (async effects) and `Runtime` (tokio channel dispatcher) |
