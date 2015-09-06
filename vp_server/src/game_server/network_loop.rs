use std::net::SocketAddr;
use std::io;
use std::io::{Error, ErrorKind, Cursor, Read, Seek, SeekFrom};
use std::sync::mpsc::Sender;
use std::collections::VecDeque;

use mio::{Token, EventLoop, EventSet, PollOpt, Handler, TryRead, TryWrite};
use mio::util::Slab;
use mio::tcp::{TcpListener, TcpStream};
use mio::Sender as MioSender;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};

pub type ClientId = usize;

pub enum NetworkEvent
{
    ClientConnected(ClientId),
    ClientDisconnected(ClientId),
    ClientDataReceived(ClientId, Vec<u8>)
}

pub enum NetworkCommand
{
    Send(Vec<(ClientId, Vec<u8>)>)
}

pub struct NetworkLoop
{
    address: SocketAddr,
    max_clients: usize,
    network_sender: Sender<NetworkEvent>,
    event_loop: EventLoop<NetworkHandler>,
}

struct NetworkHandler
{
    listener_token: Token,
    listener: TcpListener,
    client_connections: Slab<ClientConnection>,
    sender: Sender<NetworkEvent>,
}

struct ClientConnection
{
    stream: TcpStream,
    token: Token,
    send_queue: VecDeque<Vec<u8>>,
    read_buffer: Vec<u8>
}

impl NetworkLoop
{
    pub fn new(address: SocketAddr, max_clients: usize, sender: Sender<NetworkEvent>) -> NetworkLoop
    {
        NetworkLoop
        {
            address: address,
            max_clients: max_clients,
            network_sender: sender,
            event_loop: EventLoop::new().ok().expect("Failed to create event loop")
        }
    }

    pub fn channel(&self) -> MioSender<NetworkCommand>
    {
        self.event_loop.channel()
    }

    pub fn run(mut self)
    {
        NetworkHandler::bind(self.address, self.max_clients, self.network_sender).run(&mut self.event_loop);
    }
}

impl NetworkHandler
{
    fn bind(address: SocketAddr, max_clients: usize, sender: Sender<NetworkEvent>) -> NetworkHandler
    {
        let listener = TcpListener::bind(&address).ok().expect("Failed to bind address");
        let listener_token = Token(1);
        let slab = Slab::new_starting_at(Token(2), max_clients);
        NetworkHandler { listener_token: listener_token, listener: listener, client_connections: slab, sender: sender }
    }

    fn run(&mut self, event_loop: &mut EventLoop<NetworkHandler>)
    {
        event_loop.register_opt
        (
            &self.listener,
            self.listener_token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        )
        .ok()
        .expect("Failed to register server with event loop");

        event_loop.run(self)
        .ok()
        .expect("Failed to start event loop");
    }

    fn process_listener_events(&mut self, event_loop: &mut EventLoop<NetworkHandler>, events: EventSet)
    {
        if events.is_error()
        {
            error!("Error on listening socket.");
            event_loop.shutdown();
        }
        else if events.is_readable()
        {
            self.accept_client(event_loop);
        }
        else
        {
            error!("Unexpected server event.");
            event_loop.shutdown();
        }
    }

    fn process_client_events(&mut self, event_loop: &mut EventLoop<NetworkHandler>, token: Token, events: EventSet)
    {
        if events.is_hup()
        {
            debug!("Hup event for {:?}", token);
            self.disconnect_client(token);
        }
        else if events.is_error()
        {
            debug!("Error event for {:?}", token);
            self.disconnect_client(token);
        }
        else
        {
            if events.is_readable()
            {
                match self.find_connection(token).read(event_loop)
                {
                    Ok(messages) =>
                    {
                        for message in messages.into_iter()
                        {
                            self.sender.send(NetworkEvent::ClientDataReceived(token.0, message)).unwrap()
                        }
                    },
                    Err(e) =>
                    {
                        error!("Failed to read buffer for token {:?}, error: {}", token, e);
                        self.disconnect_client(token);
                    }
                }
            }

            if events.is_writable()
            {
                match self.find_connection(token).write(event_loop)
                {
                    Ok(_) => {},
                    Err(e) =>
                    {
                        error!("Failed to write buffer for token {:?}, error: {}", token, e);
                        self.disconnect_client(token);
                    }
                }
            }
        }
    }

    fn process_command(&mut self, event_loop: &mut EventLoop<NetworkHandler>, msg: NetworkCommand)
    {
        match msg
        {
            NetworkCommand::Send(sends) => self.process_send_command(event_loop, sends)
        }
    }

    fn process_send_command(&mut self, event_loop: &mut EventLoop<NetworkHandler>, sends: Vec<(ClientId, Vec<u8>)>)
    {
        for (client_id, data) in sends
        {
            let token = Token(client_id);

            match self.find_connection(token).enqueue_data(event_loop, data)
            {
                Ok(_) => {},
                Err(e) =>
                {
                    error!("Failed to enqueue data for token {:?}, error: {}", token, e);
                    self.disconnect_client(token);
                }
            }
        }
    }

    fn accept_client(&mut self, event_loop: &mut EventLoop<NetworkHandler>)
    {
        match self.listener.accept()
        {
            Ok(Some(new_stream)) => { self.process_new_client_stream(new_stream, event_loop); },
            Ok(None) => { error!("Failed to accept new socket"); },
            Err(e) => { error!("Failed to accept new socket, {}", e); },
        };

        self.reregister_listener(event_loop);
    }

    fn process_new_client_stream(&mut self, new_stream: TcpStream, event_loop: &mut EventLoop<NetworkHandler>)
    {
        match self.client_connections.insert_with(|token| ClientConnection::new(new_stream, token))
        {
            Some(token) => match self.find_connection(token).register(event_loop)
            {
                Ok(_) =>
                {
                    debug!("New client {:?} registered with event loop", token);
                    self.sender.send(NetworkEvent::ClientConnected(token.0)).unwrap();
                },
                Err(e) =>
                {
                    error!("Failed to register {:?} connection with event loop, {:?}", token, e);
                    self.client_connections.remove(token);
                }
            },
            None => { error!("Failed to insert connection into slab"); }
        };
    }

    fn reregister_listener(&mut self, event_loop: &mut EventLoop<NetworkHandler>)
    {
        event_loop.reregister
        (
            &self.listener,
            self.listener_token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        )
        .unwrap_or_else(|e|
        {
            error!("Failed to reregister server {:?}, {:?}", self.listener_token, e);
            event_loop.shutdown();
        });
    }

    fn disconnect_client(&mut self, token: Token)
    {
        self.client_connections.remove(token);
        self.sender.send(NetworkEvent::ClientDisconnected(token.0)).unwrap();
    }

    fn find_connection<'a>(&'a mut self, token: Token) -> &'a mut ClientConnection
    {
        &mut self.client_connections[token]
    }
}

impl Handler for NetworkHandler
{
    type Timeout = ();
    type Message = NetworkCommand;

    fn ready(&mut self, event_loop: &mut EventLoop<NetworkHandler>, token: Token, events: EventSet)
    {
        if token == self.listener_token
        {
            self.process_listener_events(event_loop, events);
        }
        else
        {
            self.process_client_events(event_loop, token, events);
        };
    }

    fn notify(&mut self, event_loop: &mut EventLoop<NetworkHandler>, msg: NetworkCommand)
    {
        self.process_command(event_loop, msg);
    }
}

impl ClientConnection
{
    fn new(stream: TcpStream, token: Token) -> ClientConnection
    {
        ClientConnection
        {
            stream: stream,
            token: token,
            send_queue: VecDeque::new(),
            read_buffer: Vec::new()
        }
    }

    fn register(&mut self, event_loop: &mut EventLoop<NetworkHandler>) -> io::Result<()>
    {
        event_loop.register_opt
        (
            &self.stream,
            self.token,
            EventSet::error() | EventSet::hup() | EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        )
    }

    fn reregister(&mut self, event_loop: &mut EventLoop<NetworkHandler>) -> io::Result<()>
    {
        let mut event_set = EventSet::error() | EventSet::hup() | EventSet::readable();
        if self.send_queue.len() != 0
        {
            event_set.insert(EventSet::writable());
        }

        event_loop.reregister
        (
            &self.stream,
            self.token,
            event_set,
            PollOpt::edge() | PollOpt::oneshot()
        )
    }

    fn read(&mut self, event_loop: &mut EventLoop<NetworkHandler>) -> io::Result<Vec<Vec<u8>>>
    {
        try!(self.stream.try_read_buf(&mut self.read_buffer));

        let (messages, remainder) = try!(self.read_messages());

        //TODO: use drain instead?
        self.read_buffer = remainder;

        try!(self.reregister(event_loop));

        Ok(messages)
    }

    fn read_messages(&mut self) -> io::Result<(Vec<Vec<u8>>, Vec<u8>)>
    {
        let mut messages = Vec::new();

        let mut cursor = Cursor::new(&self.read_buffer[..]);

        loop
        {
            if cursor.position() as usize > self.read_buffer.len() - 4
            {
                break;
            }

            let length = try!(cursor.read_u32::<BigEndian>()) as usize;
            if (self.read_buffer.len() - cursor.position() as usize) < length
            {
                cursor.seek(SeekFrom::Current(-4)).unwrap();
                break;
            }

            let mut message = Vec::with_capacity(length);
            unsafe {message.set_len(length);}
            try!(cursor.read(&mut message));
            messages.push(message);
        }

        let mut remainder = Vec::new();
        cursor.read_to_end(&mut remainder).unwrap();
        Ok((messages, remainder))
    }

    fn enqueue_data(&mut self, event_loop: &mut EventLoop<NetworkHandler>, data: Vec<u8>) -> io::Result<()>
    {
        let mut length_arr = Vec::with_capacity(4);
        length_arr.write_u32::<BigEndian>(data.len() as u32).unwrap();
        self.send_queue.push_back(length_arr);
        self.send_queue.push_back(data);
        self.reregister(event_loop)
    }

    fn write(&mut self, event_loop: &mut EventLoop<NetworkHandler>) -> io::Result<()>
    {
        self.send_queue.pop_front()
            .ok_or(Error::new(ErrorKind::Other, "Could not pop send queue"))
            .and_then(|data|
            {
                match self.stream.try_write(&data)
                {
                    Ok(None) =>
                    {
                        //would block
                        self.send_queue.push_front(data);
                        Ok(())
                    },
                    Ok(Some(n)) =>
                    {
                        if n < data.len()
                        {
                            let remainder = data.into_iter().skip(n).collect();
                            self.send_queue.push_front(remainder);
                        }
                        Ok(())
                    },
                    Err(e) => Err(e)
                }
            })
            .and_then(|_| self.reregister(event_loop))
    }
}
