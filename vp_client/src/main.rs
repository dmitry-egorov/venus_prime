extern crate vp_shared;
extern crate bincode;

use std::thread;
use std::net::TcpStream;
use std::io::{Read, Write, BufRead};

use bincode::SizeLimit;
use bincode::rustc_serialize::{encode, decode};

use vp_shared::*;

fn main()
{
    let mut stream = TcpStream::connect("127.0.0.1:8000").unwrap();

    let mut read_stream = stream.try_clone().unwrap();
    thread::spawn(move ||
    {
        loop
        {
            let mut buf = [0; 2048];
            read_stream.read(&mut buf).unwrap();

            let events: Vec<Event> = decode(&buf).unwrap();
            println!("{:?}", events);
        }
    });

    let stdin = std::io::stdin();
    for line in stdin.lock().lines()
    {
        let line = line.unwrap();
        let command = match line.trim()
        {
            "q" => std::process::exit(0),
            "u" => PlayerCommand::ChangeMovementDirection(Some(Direction::Up)),
            "d" => PlayerCommand::ChangeMovementDirection(Some(Direction::Down)),
            "l" => PlayerCommand::ChangeMovementDirection(Some(Direction::Left)),
            "r" => PlayerCommand::ChangeMovementDirection(Some(Direction::Right)),
            _ => PlayerCommand::ChangeMovementDirection(None),
        };

        println!("Sending command: {:?}", command);

        let encoded = encode(&vec![command], SizeLimit::Infinite).unwrap();
        stream.write(&encoded).unwrap();
    }
}
