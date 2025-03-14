use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Server<L> {
    listener: L,
}

impl<L> Server<L>
where
    L: Listener,
{
    async fn run<Srv>(&mut self, mut service_impl: Srv)
    where
        for<'de, 'ser> Srv: Service<'de, 'ser>,
    {
        let mut connection = self.listener.accept().await;
        loop {
            // Safety: TODO:
            let service_impl = unsafe { &mut *(&mut service_impl as *mut Srv) };
            if let Err(_) = service_impl.handle_next(&mut connection).await {
                break;
            }
        }
    }
}

pub trait Service<'de, 'ser> {
    type MethodCall: Deserialize<'de>;
    type Reply: Serialize + 'ser;

    fn handle(&'ser mut self, method: Self::MethodCall) -> impl Future<Output = Self::Reply>;

    fn handle_next<Sock>(
        &'ser mut self,
        connection: &'de mut Connection<Sock>,
    ) -> impl Future<Output = Result<(), ()>>
    where
        Sock: Socket,
    {
        async {
            let json = connection.read_json_from_socket().await?;
            let call: Self::MethodCall = serde_json::from_str(json).unwrap();
            let _: Self::Reply = self.handle(call).await;

            Ok(())
        }
    }
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
}

pub enum SocketNext {
    GetName,
    SetName,
    GetAge,
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
                *self = SocketNext::TheEnd;
                r#"{"method":"org.zeenix.Person.GetAge"}"#
            }
            SocketNext::TheEnd => return Err(()),
        };
        buf.copy_from_slice(json.as_bytes());

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

impl<'de, 'ser> Service<'de, 'ser> for Wizard {
    type MethodCall = Methods<'de>;
    type Reply = Replies<'ser>;

    async fn handle(&'ser mut self, method: Self::MethodCall) -> Self::Reply {
        println!("Handling method: {:?}", method);
        let ret = match method {
            Methods::GetName => Replies::GetName { name: self.name() },
            Methods::SetName { name } => {
                self.name = name.to_string();
                Replies::SetName
            }
            Methods::GetAge => Replies::GetAge { age: self.age },
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
    SetName,
    GetAge { age: u8 },
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
