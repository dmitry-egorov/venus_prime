extern crate vp_shared;
extern crate bincode;
extern crate byteorder;

use std::thread;
use std::sync::mpsc::channel;
use std::net::TcpStream;
use std::io::{Read, Write, BufRead, BufReader, BufWriter};

use bincode::SizeLimit;
use bincode::rustc_serialize::{encode, decode};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use vp_shared::*;

fn main()
{
    let stream = match TcpStream::connect("192.168.1.52:8000")
    {
        Ok(stream) => stream,
        Err(e) =>
        {
            println!("Unable to connect to the server. {}", e);
            return;
        }
    };

    let (tx, rx) = channel();

    let read_stream = stream.try_clone().unwrap();
    thread::spawn(move ||
    {
        let mut reader = BufReader::new(read_stream);

        loop
        {
            //thread::sleep_ms(3000);
            let message = read_message(&mut reader).unwrap();
            let events = match decode::<Vec<Event>>(&message)
            {
                Ok(x) => x,
                Err(e) => { println!("Error reading events: {}", e); vec![] }
            };

            tx.send(events).unwrap();
        }
    });

    thread::spawn(move ||
    {
        let count_step = 100;
        let mut next_step = count_step;
        let mut total_events = 0;

        loop
        {
            let events = rx.recv().unwrap();
            //println!("{:?}", events);

            total_events += events.len();
            if total_events >= next_step
            {
                next_step += count_step;
                println!("Events: {}", total_events);
                println!("Sample: {:?}", events);
            }
        }
    });

    let mut writer = BufWriter::new(stream);
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

        let commands = vec![command; 1000];
        let encoded = encode(&commands, SizeLimit::Infinite).unwrap();
        writer.write_u32::<BigEndian>(encoded.len() as u32).unwrap();
        writer.write(&encoded).unwrap();
        writer.flush().unwrap();
    }
}

fn read_message<R: ReadBytesExt>(reader: &mut R) -> std::io::Result<Vec<u8>>
{
    let length = try!(reader.read_u32::<BigEndian>()) as usize;
    let mut buf = Vec::with_capacity(length);
    unsafe {buf.set_len(length);}
    try!(reader.read(&mut buf));

    Ok(buf)
}
