pub mod network_loop;
mod game_loop;

use std::net::SocketAddr;
use std::sync::mpsc::channel;
use std::iter::FromIterator;

use time::Duration;

use self::network_loop::NetworkLoop;
use self::game_loop::GameLoop;
use game_server::network_loop::{NetworkEvent, ClientId};

pub struct Frame
{
    pub messages: Vec<NetworkEvent>,
    pub currently_connected_clients: Vec<ClientId>,
    pub elapsed_seconds: f32
}

pub enum GameServerCommand
{
    Continue(Vec<(ClientId, Vec<u8>)>),
    Exit
}

pub fn game_server(target_frame_time: Duration, address: SocketAddr, max_clients: usize) -> (GameLoop, NetworkLoop)
{
    let (messages_tx, messages_rx) = channel();
    let network_loop = NetworkLoop::new(address, max_clients, messages_tx);
    let game_loop = GameLoop::new(target_frame_time, messages_rx, network_loop.channel());

    (game_loop, network_loop)
}

impl Frame
{
    pub fn get_just_connected_clients<T>(&self) -> T
        where T: FromIterator<ClientId>
    {
        self.messages.iter().filter_map(|message| match message
        {
            &NetworkEvent::ClientConnected(client_id) => Some(client_id),
            _ => None
        })
        .collect()
    }
}
