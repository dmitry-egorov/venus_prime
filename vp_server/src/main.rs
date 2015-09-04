#![feature(append)]

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
use std::collections::HashSet;

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
        let mut command_execution_events = get_command_execution_events(&world, &frame);
        world.apply_events(&command_execution_events);
        let mut update_events = world.update(frame.elapsed_seconds);
        world.apply_events(&update_events);

        let mut frame_events = Vec::new();
        frame_events.append(&mut command_execution_events);
        frame_events.append(&mut update_events);
        let sends = get_sends(&frame_events, &world, &frame);

        GameServerCommand::Continue(sends)
    });
}

fn get_command_execution_events(world: &World, frame: &Frame) -> Vec<Event>
{
    frame
    .messages
    .iter()
    .flat_map(|message| match message
    {
        &GameServerMessage::ClientConnected(client_id) => world.create_player(client_id),
        &GameServerMessage::ClientDisconnected(client_id) => world.remove_player(client_id),
        &GameServerMessage::ClientDataReceived(client_id, ref data) =>
        {
            let commands = deserialize_command(data);
            commands
                .into_iter()
                .flat_map(|command| world.process_player_command(client_id, command))
                .collect()
        },
    })
    .collect()
}

fn get_sends(frame_events: &Vec<Event>, world: &World, frame: &Frame) -> Vec<(ClientId, Vec<u8>)>
{
    let connected_clients = frame.get_just_connected_clients::<HashSet<ClientId>>();

    let mut snapshots = if connected_clients.len() != 0
    {
        let snapshot = world.get_snapshot();
        let serialized_snapshot = serialize_events(&snapshot);

        connected_clients
            .iter()
            .cloned()
            .map(|client_id| (client_id, serialized_snapshot.clone()))
            .collect()
    }
    else
    {
        vec![]
    };

    let mut updates = if frame_events.len() != 0
    {
        let broadcast = serialize_events(frame_events);
        frame
            .currently_connected_clients
            .iter()
            .cloned()
            .filter(|client_id| !connected_clients.contains(client_id))
            .map(|client_id| (client_id, broadcast.clone()))
            .collect()
    }
    else
    {
        vec![]
    };

    let mut events = Vec::new();
    events.append(&mut snapshots);
    events.append(&mut updates);

    events
}

fn serialize_events(events: &Vec<Event>) -> Vec<u8>
{
    //String::from(format!("{:?}\n", events)).into_bytes()

    encode(events, SizeLimit::Infinite).unwrap()
}

fn deserialize_command(data: &[u8]) -> Vec<PlayerCommand>
{
    let c = match String::from_utf8_lossy(data).as_ref().trim()
    {
        "u" => PlayerCommand::ChangeMovementDirection(Some(Direction::Up)),
        "d" => PlayerCommand::ChangeMovementDirection(Some(Direction::Down)),
        "l" => PlayerCommand::ChangeMovementDirection(Some(Direction::Left)),
        "r" => PlayerCommand::ChangeMovementDirection(Some(Direction::Right)),
        _ => PlayerCommand::ChangeMovementDirection(None),
    };

    vec![c]
    /*
    match decode(data)
    {
        Ok(commands) => commands,
        Err(e) =>
        {
            error!("Error decoding commands, error: {}", e);
            vec![]
        }
    }
    */
}
