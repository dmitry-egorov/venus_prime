use na::Vec2;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum PlayerCommand
{
    ChangeMovementDirection(Option<Direction>)
}

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum Event
{
    PlayerCreated(PlayerId, Player),
    PlayerRemoved(PlayerId),
    PlayerActed(PlayerId, PlayerAction)
}

pub struct World
{
    players: HashMap<PlayerId, Player>
}

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub struct Player
{
    movement_direction: Option<Direction>,
    position: Position
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum Direction
{
    Up,
    Down,
    Left,
    Right
}

type PlayerId = usize;

type Position = Vec2<f32>;

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
enum PlayerAction
{
    ChangedMovementDirection(Option<Direction>),
    Moved(Position)
}

use self::Event::*;
use self::PlayerAction::*;

impl World
{
    pub fn new() -> World
    {
        World { players: HashMap::new() }
    }

    pub fn create_player(&self, player_id: PlayerId) -> Vec<Event>
    {
        vec![PlayerCreated(player_id, Player {movement_direction: None, position: Vec2::new(0.0, 0.0)})]
    }

    pub fn remove_player(&self, player_id: PlayerId) -> Vec<Event>
    {
        vec![PlayerRemoved(player_id)]
    }

    pub fn process_player_command(&self, player_id: PlayerId, command: PlayerCommand) -> Vec<Event>
    {
        match self.players.get(&player_id)
        {
            Some(player) =>
            {
                player.process_command(command)
                    .into_iter()
                    .map(|pe| Event::PlayerActed(player_id, pe))
                    .collect()
            },
            None => vec![]
        }
    }

    pub fn update(&self, elapsed_seconds: f32) -> Vec<Event>
    {
        self.all_players(|player| player.update(elapsed_seconds))
    }

    pub fn get_snapshot(&self) -> Vec<Event>
    {
        self.players.iter().map(|(player_id, player)| Event::PlayerCreated(player_id.clone(), player.clone())).collect()
    }

    pub fn apply_events(&mut self, events: &[Event])
    {
        for event in events
        {
            self.apply_event(*event);
        }
    }

    fn apply_event(&mut self, event: Event)
    {
        match event
        {
            PlayerCreated(player_id, player)      => { self.players.insert(player_id, player); },
            PlayerRemoved(player_id)              => { self.players.remove(&player_id); },
            PlayerActed(player_id, player_action) =>
            {
                self.players.get_mut(&player_id).map(|player| player.apply_event(player_action));
            }
        }
    }

    fn all_players<F>(&self, f: F) -> Vec<Event>
        where F: Fn(&Player) -> Vec<PlayerAction>
    {
        self.players
        .iter()
        .flat_map(|(player_id, player)|
        {
            f(player)
            .into_iter()
            .map(move |e| PlayerActed(*player_id, e))
        })
        .collect()
    }
}

impl Player
{
    fn process_command(&self, command: PlayerCommand) -> Vec<PlayerAction>
    {
        match command
        {
            PlayerCommand::ChangeMovementDirection(direction) =>
            {
                if self.movement_direction != direction
                {
                    vec![ChangedMovementDirection(direction)]
                }
                else
                {
                    vec![]
                }
            }
        }
    }

    fn update(&self, elapsed_seconds: f32) -> Vec<PlayerAction>
    {
        let player_speed = 2.0;

        match self.movement_direction
        {
            Some(direction) =>
            {
                let new_position = self.position + direction.to_vec2() * player_speed * elapsed_seconds;

                vec![Moved(new_position)]
            },
            None => vec![]
        }
    }

    fn apply_event(&mut self, event: PlayerAction)
    {
        match event
        {
            ChangedMovementDirection(new_direction) => self.movement_direction = new_direction,
            Moved(new_position) => self.position = new_position,
        }
    }
}

impl Direction
{
    fn to_vec2(&self) -> Vec2<f32>
    {
        match self
        {
            &Direction::Up    => Vec2::new( 0.0,  1.0),
            &Direction::Down  => Vec2::new( 0.0, -1.0),
            &Direction::Right => Vec2::new( 1.0,  0.0),
            &Direction::Left  => Vec2::new(-1.0,  0.0),
        }
    }
}
