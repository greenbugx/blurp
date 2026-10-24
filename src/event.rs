use crate::contract::{Draft, DraftBatch};
use crate::ollama::OllamaError;
use color_eyre::eyre::OptionExt;
use crossterm::event::Event as CrosstermEvent;
use futures::{FutureExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;

const TICK_FPS: f64 = 30.0;

#[derive(Debug)]
pub enum Event {
    Tick,
    Crossterm(CrosstermEvent),
    App(AppEvent),
}

#[derive(Debug)]
pub enum AppEvent {
    Quit,
    DraftResult {
        id: u64,
        result: Result<DraftBatch, OllamaError>,
    },
    RedraftResult {
        id: u64,
        draft_id: String,
        result: Result<Draft, OllamaError>,
    },
}

#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
    actor: Option<EventTask>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        Self {
            sender,
            receiver,
            actor: Some(actor),
        }
    }

    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        if let Some(actor) = self.actor.take() {
            tokio::spawn(async move { actor.run().await });
        }
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    #[cfg(test)]
    pub(crate) fn try_next_app(&mut self) -> Option<AppEvent> {
        loop {
            match self.receiver.try_recv() {
                Ok(Event::App(app_event)) => return Some(app_event),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    }

    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.sender.clone()
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct EventTask {
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    async fn run(self) -> color_eyre::Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              _ = tick_delay => {
                self.send(Event::Tick);
              }
              Some(Ok(evt)) = crossterm_event => {
                self.send(Event::Crossterm(evt));
              }
            };
        }
        Ok(())
    }

    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}
