extern crate nalgebra as na;
extern crate rustc_serialize;

use na::Vec2;

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum PlayerCommand
{
    ChangeMovementDirection(Option<Direction>)
}

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum Event
{
    PlayerCreated(PlayerId, PlayerState),
    PlayerRemoved(PlayerId),
    PlayerActed(PlayerId, PlayerAction)
}

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub struct PlayerState
{
    pub movement_direction: Option<Direction>,
    pub position: Position
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum Direction
{
    Up,
    Down,
    Left,
    Right
}

pub type PlayerId = usize;

pub type Position = Vec2<f32>;

#[derive(Clone, Copy, Debug, RustcEncodable, RustcDecodable)]
pub enum PlayerAction
{
    ChangedMovementDirection(Option<Direction>),
    Moved(Position)
}

impl Direction
{
    pub fn to_vec2(&self) -> Vec2<f32>
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
