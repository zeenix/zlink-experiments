use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Server;

impl Server {
    async fn run<Srv>(&mut self, mut service_impl: Srv)
    where
        for<'de, 'ser> Srv: Service<'de, 'ser>,
    {
        loop {
            // This will be create from listener.
            let mut connection = Connection;

            // Safety: TODO:
            let service_impl = unsafe { &mut *(&mut service_impl as *mut Srv) };
            if let Err(_) = service_impl.handle_next(&mut connection).await {
                break;
            }
        }
    }
}

trait Service<'de, 'ser> {
    type MethodCall: Deserialize<'de>;
    type Reply: Serialize + 'ser;

    async fn handle(&'ser mut self, method: Self::MethodCall) -> Self::Reply;

    async fn handle_next(&'ser mut self, connection: &'de mut Connection) -> Result<(), ()> {
        let call: Self::MethodCall =
            serde_json::from_str(connection.read_json_from_socket()).unwrap();
        let _: Self::Reply = self.handle(call).await;

        Ok(())
    }
}

pub struct Connection;

impl Connection {
    fn read_json_from_socket(&self) -> &str {
        "{ \"x\": 32 }"
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
    type MethodCall = &'de str;
    type Reply = &'ser str;

    async fn handle(&'ser mut self, _method: Self::MethodCall) -> Self::Reply {
        self.name()
    }
}

#[tokio::main]
async fn main() {
    let data = r#"
        {
            "name": "Harry Potter",
            "age": 17,
            "unknown_extra_field": "This unknown field is extra and will be ignored"
        }
    "#;

    let person = serde_json::from_str::<Wizard>(data).expect("Failed to deserialize JSON");

    println!("Deserialized struct: {person:?}");

    let mut service = Server;

    let _ = service.run(person).await;
}
