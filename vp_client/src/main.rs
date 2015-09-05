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
    let mut stream = match TcpStream::connect("192.168.1.52:8000")
    {
        Ok(stream) => stream,
        Err(e) =>
        {
            println!("Unable to connect to the server. {}", e);
            return;
        }
    };

    let mut read_stream = stream.try_clone().unwrap();
    thread::spawn(move ||
    {
        let mut buf = [0; 2048];

        loop
        {
            thread::sleep_ms(3000);
            for _ in 0..9
            {
                let count = read_stream.read(&mut buf).unwrap();

                let events: Vec<Event> = decode(&buf[0..count]).unwrap();
                println!("{:?}", events);
            }
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
