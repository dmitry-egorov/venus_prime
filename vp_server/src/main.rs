#![feature(append)]

extern crate nalgebra as na;
extern crate time;
extern crate mio;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate bincode;
extern crate vp_shared;
extern crate byteorder;

mod game_server;
mod vp_world;

use std::str::FromStr;
use std::thread;
use std::collections::HashSet;

use time::Duration;
use bincode::SizeLimit;
use bincode::rustc_serialize::{encode, decode};

use game_server::{GameServerCommand, Frame};
use game_server::network_loop::{NetworkEvent, ClientId};
use vp_shared::{Event, PlayerCommand};
use vp_world::World;

fn main()
{
    env_logger::init().ok().expect("Failed to init logger");

    info!("Starting game server...");

    let addr = FromStr::from_str("0.0.0.0:8000").ok().expect("Failed to parse host:port string");

    let (mut game_loop, network_loop) = game_server::game_server(Duration::milliseconds(20), addr, 128);

    thread::spawn(move ||
    {
        info!("Listening for incoming connections...");
        network_loop.run()
    });

    info!("Running game world...");
    let mut world = World::new();
    game_loop.run(|frame|
    {
        let command_execution_events = get_command_execution_events(&world, &frame);
        world.apply_events(&command_execution_events);
        let update_events = world.update(frame.elapsed_seconds);
        world.apply_events(&update_events);

        let mut frame_events = Vec::new();
        frame_events.extend(command_execution_events.iter());
        frame_events.extend(update_events.iter());
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
        &NetworkEvent::ClientConnected(client_id) => world.create_player(client_id),
        &NetworkEvent::ClientDisconnected(client_id) => world.remove_player(client_id),
        &NetworkEvent::ClientDataReceived(client_id, ref data) =>
        {
            let commands = deserialize_commands(data);
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
    let just_connected_clients = frame.get_just_connected_clients::<HashSet<ClientId>>();

    let mut snapshots = if just_connected_clients.len() != 0
    {
        let snapshot = world.get_snapshot();
        let serialized_snapshot = serialize_events(&snapshot);

        just_connected_clients
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
                      && just_connected_clients.len() != frame.currently_connected_clients.len()
    {
        let broadcast = serialize_events(frame_events);
        frame
            .currently_connected_clients
            .iter()
            .cloned()
            .filter(|client_id| !just_connected_clients.contains(client_id))
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
    //debug!("Sending: {:?}", events);
    encode(events, SizeLimit::Infinite).unwrap()
}

fn deserialize_commands(data: &[u8]) -> Vec<PlayerCommand>
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
