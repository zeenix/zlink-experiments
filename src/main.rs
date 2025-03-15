use futures_util::{
    Stream, StreamExt, pin_mut,
    stream::{Repeat, Take, repeat},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

struct Server<L> {
    listener: L,
}

impl<L> Server<L>
where
    L: Listener,
{
    async fn run<Srv>(&mut self, mut service: Srv)
    where
        for<'ser> Srv: Service,
        for<'de, 'ser> <Srv as Service>::MethodCall<'de>: Deserialize<'de>,
    {
        let mut connection = self.listener.accept().await;
        loop {
            // Safety: TODO:
            let service = unsafe { &mut *(&mut service as *mut Srv) };
            match service.handle_next(&mut connection).await {
                Ok(Some(stream)) => {
                    pin_mut!(stream);
                    while let Some(r) = stream.next().await {
                        println!("Streamed reply: {:?}", r);
                    }
                }
                Ok(None) => (),
                Err(_) => break,
            }
        }
    }
}

pub trait Service
where
    <Self::ReplyStream as Stream>::Item: Serialize + core::fmt::Debug,
{
    type MethodCall<'de>: Deserialize<'de>;
    type ReplyParams<'ser>: Serialize
    where
        Self: 'ser;
    type ReplyStream: Stream;

    fn handle<'de, 'ser>(
        &'ser mut self,
        method: Call<Self::MethodCall<'de>>,
    ) -> impl Future<Output = Reply<Option<Self::ReplyParams<'ser>>, Self::ReplyStream>>;

    fn handle_next<'de, 'ser, Sock>(
        &'ser mut self,
        connection: &'de mut Connection<Sock>,
    ) -> impl Future<Output = Result<Option<Self::ReplyStream>, ()>>
    where
        Sock: Socket,
    {
        async {
            let reply = {
                // Safety: The compiler doesn't know that we write to different fields
                //         in `read` and `write` so doesn't like us borrowing it twice.
                let connection = unsafe { &mut *(connection as *mut Connection<Sock>) };
                let json = connection.read_json_from_socket().await?;
                let call: Call<Self::MethodCall<'de>> = serde_json::from_str(json).unwrap();
                self.handle(call).await
            };
            match reply {
                Reply::Single(reply) => {
                    let reply = match reply {
                        Some(reply) => json!({"parameters": reply}),
                        None => json!({}),
                    };
                    let json = serde_json::to_string(&reply).unwrap();
                    let _: usize = connection.write_json_to_socket(&json).await?;

                    Ok(None)
                }
                Reply::Multi(stream) => Ok(Some(stream)),
            }
        }
    }
}
/// A method call.
#[derive(Debug, Serialize, Deserialize)]
pub struct Call<M> {
    #[serde(flatten)]
    method: M,
    #[serde(skip_serializing_if = "Option::is_none")]
    more: Option<bool>,
}

#[derive(Debug)]
pub enum Reply<Params, ReplyStream> {
    Single(Params),
    Multi(ReplyStream),
}

pub trait Listener {
    type Socket: Socket;

    fn accept(&mut self) -> impl Future<Output = Connection<Self::Socket>>;
}

// Thsi would be a `tokio::net::UnixListener`.
impl Listener for () {
    type Socket = SocketNext;

    async fn accept(&mut self) -> Connection<Self::Socket> {
        Connection {
            socket: SocketNext::GetName,
            buf: [0; 1024],
        }
    }
}

pub trait Socket {
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, ()>>;
    fn write(&mut self, buf: &[u8]) -> impl Future<Output = Result<usize, ()>>;
}

pub struct Connection<Socket> {
    socket: Socket,
    buf: [u8; 1024],
}

impl<Sock> Connection<Sock>
where
    Sock: Socket,
{
    async fn read_json_from_socket(&mut self) -> Result<&str, ()> {
        let len = self.socket.read(&mut self.buf).await?;
        let json = std::str::from_utf8(&self.buf[..len]).unwrap();
        Ok(json)
    }

    async fn write_json_to_socket(&mut self, json: &str) -> Result<usize, ()> {
        println!("writing back the reply: {json}");
        self.socket.write(json.as_bytes()).await?;
        Ok(json.len())
    }
}

pub enum SocketNext {
    GetName,
    SetName,
    GetAge,
    GetNameStream,
    TheEnd,
}

impl Socket for SocketNext {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        let json = match self {
            SocketNext::GetName => {
                *self = SocketNext::SetName;
                r#"{"method":"org.zeenix.Person.GetName"}"#
            }
            SocketNext::SetName => {
                *self = SocketNext::GetAge;
                r#"{"method":"org.zeenix.Person.SetName","params":{"name":"Saruman"}}"#
            }
            SocketNext::GetAge => {
                *self = SocketNext::GetNameStream;
                r#"{"method":"org.zeenix.Person.GetAge"}"#
            }
            SocketNext::GetNameStream => {
                *self = SocketNext::TheEnd;
                r#"{"method":"org.zeenix.Person.GetName","more":true}"#
            }
            SocketNext::TheEnd => return Err(()),
        };
        (&mut buf[..json.len()]).copy_from_slice(json.as_bytes());

        Ok(json.len())
    }

    async fn write(&mut self, _buf: &[u8]) -> Result<usize, ()> {
        Ok(0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Wizard {
    name: String,
    age: u8,
}

impl Wizard {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Service for Wizard {
    type MethodCall<'de> = Methods<'de>;
    type ReplyParams<'ser> = Replies<'ser>;
    type ReplyStream = Take<Repeat<StreamedReplies>>;

    async fn handle<'de, 'ser>(
        &'ser mut self,
        method: Call<Self::MethodCall<'de>>,
    ) -> Reply<Option<Self::ReplyParams<'ser>>, Self::ReplyStream> {
        println!("Handling method: {:?}", method);
        let ret = match method.method {
            Methods::GetName => {
                if method.more.unwrap_or(false) {
                    Reply::Multi(
                        repeat(StreamedReplies::Name {
                            name: self.name.clone(),
                        })
                        .take(5),
                    )
                } else {
                    Reply::Single(Some(Replies::GetName { name: self.name() }))
                }
            }
            Methods::SetName { name } => {
                self.name = name.to_string();
                Reply::Single(None)
            }
            Methods::GetAge => Reply::Single(Some(Replies::GetAge { age: self.age })),
        };
        println!("Returning: {:?}", ret);

        ret
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "method", content = "params")]
enum Methods<'m> {
    #[serde(rename = "org.zeenix.Person.GetName")]
    GetName,
    #[serde(rename = "org.zeenix.Person.SetName")]
    SetName { name: &'m str },
    #[serde(rename = "org.zeenix.Person.GetAge")]
    GetAge,
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
enum Replies<'r> {
    GetName { name: &'r str },
    GetAge { age: u8 },
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
enum StreamedReplies {
    Name { name: String },
}

#[tokio::main]
async fn main() {
    let person = Wizard {
        name: "Gandalf".to_string(),
        age: 100,
    };

    let mut service = Server { listener: () };

    let _ = service.run(person).await;
}
