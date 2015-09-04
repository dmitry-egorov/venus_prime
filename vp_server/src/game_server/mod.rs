mod network_loop;
mod game_loop;

use std::net::SocketAddr;
use std::sync::mpsc::channel;
use time::Duration;

use self::network_loop::NetworkLoop;
use self::game_loop::GameLoop;

pub type ClientId = usize;

pub struct Frame
{
    pub messages: Vec<GameServerMessage>,
    pub currently_connected_clients: Vec<ClientId>,
    pub elapsed_seconds: f32
}

pub enum GameServerCommand
{
    Continue(Vec<(ClientId, Vec<u8>)>),
    Exit
}

pub enum GameServerMessage
{
    ClientConnected(ClientId),
    ClientDisconnected(ClientId),
    ClientDataReceived(ClientId, Vec<u8>)
}

pub enum NetworkCommand
{
    Send(Vec<(ClientId, Vec<u8>)>)
}

pub fn game_server(target_frame_time: Duration, address: SocketAddr, max_clients: usize) -> (GameLoop, NetworkLoop)
{
    let (messages_tx, messages_rx) = channel();

    let handler = NetworkLoop::new(address, max_clients, messages_tx);

    let server = GameLoop::new(target_frame_time, messages_rx, handler.channel());


    (server, handler)
}
