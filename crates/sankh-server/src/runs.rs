//! Background runs and their event streams.
//!
//! `POST /api/run` starts a run and returns its id; the UI then reads
//! `GET /api/runs/{id}/events` as server-sent events. Events are buffered, so
//! a client that connects late still sees the whole run.

use axum::response::sse::Event;
use futures::Stream;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

const MAX_RUNS: usize = 50;

pub struct RunHandle {
    events: Mutex<Vec<Value>>,
    done: Mutex<bool>,
    tx: watch::Sender<usize>,
}

impl RunHandle {
    pub fn new() -> Arc<RunHandle> {
        let (tx, _) = watch::channel(0);
        Arc::new(RunHandle {
            events: Mutex::new(Vec::new()),
            done: Mutex::new(false),
            tx,
        })
    }

    pub fn push(&self, event: Value) {
        let len = {
            let mut ev = self.events.lock().unwrap();
            ev.push(event);
            ev.len()
        };
        self.tx.send_replace(len);
    }

    pub fn finish(&self) {
        *self.done.lock().unwrap() = true;
        let len = self.events.lock().unwrap().len();
        self.tx.send_replace(len + 1);
    }

    fn snapshot(&self, from: usize) -> (Vec<Value>, bool) {
        let ev = self.events.lock().unwrap();
        let done = *self.done.lock().unwrap();
        (
            ev.get(from..).map(<[Value]>::to_vec).unwrap_or_default(),
            done,
        )
    }

    pub fn stream(self: Arc<Self>) -> impl Stream<Item = Result<Event, Infallible>> {
        let rx = self.tx.subscribe();
        futures::stream::unfold(
            (self, 0usize, rx, VecDeque::<Value>::new(), false),
            |(handle, mut idx, mut rx, mut pending, mut finished)| async move {
                loop {
                    if let Some(v) = pending.pop_front() {
                        let event = Event::default().data(v.to_string());
                        return Some((Ok(event), (handle, idx, rx, pending, finished)));
                    }
                    if finished {
                        return None;
                    }
                    let (new, done) = handle.snapshot(idx);
                    idx += new.len();
                    pending.extend(new);
                    if done {
                        finished = true;
                        continue;
                    }
                    if pending.is_empty() && rx.changed().await.is_err() {
                        return None;
                    }
                }
            },
        )
    }
}

/// Run handles by id, plus insertion order for pruning.
type RunMap = (HashMap<String, Arc<RunHandle>>, VecDeque<String>);

#[derive(Default)]
pub struct Runs {
    map: Mutex<RunMap>,
}

impl Runs {
    pub fn insert(&self, id: String, handle: Arc<RunHandle>) {
        let mut guard = self.map.lock().unwrap();
        let (map, order) = &mut *guard;
        map.insert(id.clone(), handle);
        order.push_back(id);
        while order.len() > MAX_RUNS {
            if let Some(old) = order.pop_front() {
                map.remove(&old);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<RunHandle>> {
        self.map.lock().unwrap().0.get(id).cloned()
    }
}
