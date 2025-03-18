use futures_util::{
    Stream, StreamExt, pin_mut,
    stream::{Repeat, Take, repeat},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

struct Server<Listen, Srv> {
    listener: Listen,
    service: Srv,
}

impl<Listen, Srv> Server<Listen, Srv>
where
    Listen: Listener,
    Srv: Service,
{
    async fn run(&mut self) {
        // Wait for the first connection.
        let (mut reader, mut writer) = self.listener.accept_next().await;
        loop {
            match self.service.handle_next(&mut reader, &mut writer).await {
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
    type ReplyError<'ser>: Serialize
    where
        Self: 'ser;

    fn handle_next<'de, 'ser, Read, Write>(
        &'ser mut self,
        read_conn: &'de mut Connection<Read>,
        write_conn: &mut Connection<Write>,
    ) -> impl Future<Output = Result<Option<Self::ReplyStream>, ()>>
    where
        Read: SocketRead,
        Write: SocketWrite,
    {
        async {
            let reply = {
                let json = read_conn.read_json_from_socket().await?;
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
                    let _: usize = write_conn.write_json_to_socket(&json).await?;

                    Ok(None)
                }
                Reply::Error(err) => {
                    let json = serde_json::to_string(&err).unwrap();
                    let _: usize = write_conn.write_json_to_socket(&json).await?;

                    Err(())
                }
                Reply::Multi(stream) => Ok(Some(stream)),
            }
        }
    }

    fn handle<'de>(
        &mut self,
        method: Call<Self::MethodCall<'de>>,
    ) -> impl Future<
        Output = Reply<Option<Self::ReplyParams<'_>>, Self::ReplyStream, Self::ReplyError<'_>>,
    >;
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
pub enum Reply<Params, ReplyStream, ReplyError> {
    Single(Params),
    Error(ReplyError),
    Multi(ReplyStream),
}

pub trait Listener {
    type Socket: Socket;

    fn accept_next(
        &mut self,
    ) -> impl Future<
        Output = (
            Connection<<Self::Socket as Socket>::Read>,
            Connection<<Self::Socket as Socket>::Write>,
        ),
    > {
        async {
            let socket = self.accept().await;
            let (read, write) = socket.split();
            (
                Connection {
                    socket: read,
                    buf: [0; 1024],
                },
                Connection {
                    socket: write,
                    buf: [0; 1024],
                },
            )
        }
    }

    fn accept(&mut self) -> impl Future<Output = Self::Socket>;
}

// Thsi would be a `tokio::net::UnixListener`.
impl Listener for () {
    type Socket = SocketNext;

    async fn accept(&mut self) -> Self::Socket {
        SocketNext::GetName
    }
}

pub trait Socket {
    type Read: SocketRead;
    type Write: SocketWrite;

    fn split(self) -> (Self::Read, Self::Write);
}

pub trait SocketRead {
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, ()>>;
}

pub trait SocketWrite {
    fn write(&mut self, buf: &[u8]) -> impl Future<Output = Result<usize, ()>>;
}

pub struct Connection<SocketHalf> {
    socket: SocketHalf,
    buf: [u8; 1024],
}

impl<Read> Connection<Read>
where
    Read: SocketRead,
{
    async fn read_json_from_socket(&mut self) -> Result<&str, ()> {
        let len = self.socket.read(&mut self.buf).await?;
        let json = std::str::from_utf8(&self.buf[..len]).unwrap();
        Ok(json)
    }
}

impl<Write> Connection<Write>
where
    Write: SocketWrite,
{
    async fn write_json_to_socket(&mut self, json: &str) -> Result<usize, ()> {
        println!("writing back the reply: {json}");
        self.socket.write(json.as_bytes()).await?;
        Ok(json.len())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SocketNext {
    GetName,
    SetName,
    GetAge,
    GetNameStream,
    TheEnd,
}

impl Socket for SocketNext {
    type Read = Self;
    type Write = Self;

    fn split(self) -> (Self::Read, Self::Write) {
        (self, self)
    }
}

impl SocketRead for SocketNext {
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
}

impl SocketWrite for SocketNext {
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
    type ReplyError<'ser> = Errors;

    async fn handle<'de>(
        &mut self,
        method: Call<Self::MethodCall<'de>>,
    ) -> Reply<Option<Self::ReplyParams<'_>>, Self::ReplyStream, Self::ReplyError<'_>> {
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
            Methods::Fail => Reply::Error(Errors::NotFound),
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
    #[serde(rename = "org.zeenix.Person.Fail")]
    Fail,
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

#[derive(Debug, Serialize)]
enum Errors {
    NotFound,
}

#[tokio::main]
async fn main() {
    let person = Wizard {
        name: "Gandalf".to_string(),
        age: 100,
    };

    let mut service = Server {
        listener: (),
        service: person,
    };

    let _ = service.run().await;
}
