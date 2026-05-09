use std::{future::Future, pin::Pin};

use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Command — Elm-style managed effects
// ---------------------------------------------------------------------------

/// A set of async side-effects that each produce an `Msg`. Modeled after
/// Elm's `Cmd` / Lustre's `Effect`: `init` and `update` return commands to be
/// executed, and the runtime feeds their results back as messages.
///
/// `Cmd::none()` is the unit value — an empty command the runtime skips
/// without branching.
pub struct Cmd<Msg>(Vec<Pin<Box<dyn Future<Output = Msg> + Send>>>);

impl<Msg> Cmd<Msg> {
    pub fn none() -> Self {
        Self(Vec::new())
    }

    pub fn from(fut: impl Future<Output = Msg> + Send + 'static) -> Self {
        Self(vec![Box::pin(fut)])
    }

    pub fn batch(cmds: impl IntoIterator<Item = Cmd<Msg>>) -> Self {
        Self(cmds.into_iter().flat_map(|c| c.0).collect())
    }

    pub(self) fn into_futures(self) -> Vec<Pin<Box<dyn Future<Output = Msg> + Send>>> {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// Spawns [`Cmd`] futures on tokio and delivers their results through a
/// channel.
pub struct Runtime<Msg> {
    tx: mpsc::UnboundedSender<Msg>,
}

impl<Msg: Send + 'static> Runtime<Msg> {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Msg>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn spawn(&self, cmd: Cmd<Msg>) {
        for fut in cmd.into_futures() {
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let msg = fut.await;
                let _ = tx.send(msg);
            });
        }
    }
}
