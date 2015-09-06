use std::sync::mpsc::Receiver;
use std::thread;
use std::cmp::max;
use std::collections::HashSet;

use time::{Duration, PreciseTime};
use mio::Sender as MioSender;

use game_server::{GameServerCommand, Frame};
use game_server::network_loop::{NetworkEvent, ClientId, NetworkCommand};

pub struct GameLoop
{
    target_frame_time: Duration,
    network_receiver: Receiver<NetworkEvent>,
    network_sender: MioSender<NetworkCommand>,
    currently_connected_clients: HashSet<ClientId>,
}

struct TimeGuard
{
    previous_wake_time: PreciseTime,
    next_target_elapsed: Duration,
    target_elapsed: Duration,
}

impl GameLoop
{
    pub fn new(target_frame_time: Duration, network_receiver: Receiver<NetworkEvent>, network_sender: MioSender<NetworkCommand>) -> GameLoop
    {
        GameLoop
        {
            target_frame_time: target_frame_time,
            network_receiver: network_receiver,
            network_sender: network_sender,
            currently_connected_clients: HashSet::new()
        }
    }

    pub fn run<F>(&mut self, mut frame_processor: F)
        where F: FnMut(Frame) -> GameServerCommand
    {
        let mut time_guard = TimeGuard::start(self.target_frame_time);

        loop
        {
            time_guard.wait_for_time();

            let frame = self.create_frame();

            match frame_processor(frame)
            {
                GameServerCommand::Continue(per_client_data) => self.schedule_send(per_client_data),
                GameServerCommand::Exit => return,
            }
        }
    }

    fn create_frame(&mut self) -> Frame
    {
        let mut messages = Vec::new();

        loop
        {
            match self.network_receiver.try_recv()
            {
                Ok(message) =>
                {
                    match message
                    {
                        NetworkEvent::ClientConnected(client_id) => {self.currently_connected_clients.insert(client_id);},
                        NetworkEvent::ClientDisconnected(client_id) => {self.currently_connected_clients.remove(&client_id);},
                        _ => {}
                    };
                    messages.push(message);
                },
                Err(_) => { break; }
            }
        }

        Frame
        {
            messages: messages,
            currently_connected_clients: self.currently_connected_clients.iter().cloned().collect(),
            elapsed_seconds: self.target_frame_time.num_microseconds().unwrap() as f32 / 1_000_000.0,
        }
    }

    fn schedule_send(&mut self, sends: Vec<(ClientId, Vec<u8>)>)
    {
        self.network_sender.send(NetworkCommand::Send(sends)).unwrap();
    }
}

impl TimeGuard
{
    fn start(target_elapsed: Duration) -> TimeGuard
    {
        let start_time = PreciseTime::now();
        TimeGuard {previous_wake_time: start_time, target_elapsed: target_elapsed, next_target_elapsed: target_elapsed}
    }

    fn wait_for_time(&mut self)
    {
        let frame_time = PreciseTime::now();
        let elapsed = self.previous_wake_time.to(frame_time);
        let time_to_sleep = self.next_target_elapsed - elapsed;

        if time_to_sleep > Duration::zero()
        {
            thread::sleep_ms(time_to_sleep.num_milliseconds() as u32);
            //TODO: spin wait last 2 ms?
        }

        let wake_time = PreciseTime::now();
        let time_slept = frame_time.to(wake_time);
        self.next_target_elapsed = max(Duration::zero(), self.target_elapsed + (time_to_sleep - time_slept));
        self.previous_wake_time = wake_time;
    }
}
