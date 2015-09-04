extern crate nalgebra as na;
extern crate time;
extern crate mio;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate bincode;

mod game_server;
mod vp_world;

use std::str::FromStr;
use std::thread;

use time::Duration;
use bincode::SizeLimit;
use bincode::rustc_serialize::{encode, decode};

use game_server::{GameServerCommand, GameServerMessage, Frame, ClientId};
use vp_world::{World, Event, PlayerCommand, Direction};

fn main()
{
    env_logger::init().ok().expect("Failed to init logger");

    info!("Starting game server...");

    let addr = FromStr::from_str("127.0.0.1:8000").ok().expect("Failed to parse host:port string");

    let (mut game_loop, network_loop) = game_server::game_server(Duration::seconds(1), addr, 128);

    thread::spawn(move ||
    {
        info!("Listening for incoming connections...");
        network_loop.run()
    });

    info!("Running game world...");
    let mut world = World::new();
    game_loop.run(|frame|
    {
        let frame_events = get_frame_events(&world, &frame);

        let snapshot_sends = get_snapshot_sends(&world, &frame);
        let broadcast_sends = get_broadcast_sends(&frame_events, &frame);
        let sends = snapshot_sends.into_iter().chain(broadcast_sends).collect();

        world.apply_events(&frame_events);

        GameServerCommand::Continue(sends)
    });
}

fn get_frame_events(world: &World, frame: &Frame) -> Vec<Event>
{
    let server_events =
        frame
        .messages
        .iter()
        .flat_map(|message| match message
        {
            &GameServerMessage::ClientConnected(client_id) => world.spawn_player(client_id),
            &GameServerMessage::ClientDisconnected(client_id) => world.remove_player(client_id),
            &GameServerMessage::ClientDataReceived(client_id, ref data) =>
            {
                let commands = deserialize_command(data);
                commands
                    .into_iter()
                    .flat_map(|command| world.process_player_command(client_id, command))
                    .collect()
            },
        });

    let update_events = world.update(frame.elapsed_seconds);

    server_events
    .chain(update_events)
    .collect::<Vec<Event>>()
}

fn get_snapshot_sends(world: &World, frame: &Frame) -> Vec<(ClientId, Vec<u8>)>
{
    let connected_clients = frame.messages.iter().filter_map(|message| match message
    {
        &GameServerMessage::ClientConnected(client_id) => Some(client_id),
        _ => None
    })
    .collect::<Vec<_>>();

    if connected_clients.len() != 0
    {
        let snapshot = world.get_snapshot();
        let serialized_snapshot = serialize_events(&snapshot);
        connected_clients
            .into_iter()
            .map(|client_id| (client_id, serialized_snapshot.clone()))
            .collect()
    }
    else
    {
        vec![]
    }
}

fn get_broadcast_sends(frame_events: &[Event], frame: &Frame) -> Vec<(ClientId, Vec<u8>)>
{
    if frame_events.len() == 0
    {
        vec![]
    }
    else
    {
        let broadcast = serialize_events(frame_events);
        frame
            .currently_connected_clients
            .iter()
            .cloned()
            .map(|client_id| (client_id, broadcast.clone()))
            .collect()
    }
}

fn serialize_events(events: &[Event]) -> Vec<u8>
{
    //TODO: do this without copying?
    let copy = events.iter().cloned().collect::<Vec<_>>();
    encode(&copy, SizeLimit::Infinite).unwrap()
}

fn deserialize_command(data: &[u8]) -> Vec<PlayerCommand>
{
    match decode(data)
    {
        Ok(commands) => commands,
        Err(e) =>
        {
            error!("Error decoding commands, error: {}", e);
            vec![]
        }
    }
}
