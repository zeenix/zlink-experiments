use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Server {
    connection: Connection,
}

impl Server {
    async fn run<Srv>(mut self, mut service_impl: Srv)
    where
        for<'de, 'ser> Srv: Service<'de, 'ser>,
    {
        loop {
            // Safety: TODO:
            let service_impl = unsafe { &mut *(&mut service_impl as *mut Srv) };
            if let Err(_) = self.handle_next(service_impl).await {
                break;
            }
        }
    }

    async fn handle_next<'de, 'ser, Srv>(
        &'de mut self,
        service_impl: &'ser mut Srv,
    ) -> Result<(), ()>
    where
        Srv: Service<'de, 'ser>,
    {
        let call: Srv::MethodCall =
            serde_json::from_str(self.connection.read_json_from_socket()).unwrap();
        let _: Srv::Reply = service_impl.handle(&self.connection, call).await;

        Ok(())
    }
}

trait Service<'de, 'ser> {
    type MethodCall: Deserialize<'de>;
    type Reply: Serialize + 'ser;

    async fn handle(
        &'ser mut self,
        connection: &'de Connection,
        method: Self::MethodCall,
    ) -> Self::Reply;
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
    type MethodCall = String;
    type Reply = &'ser str;

    async fn handle(
        &'ser mut self,
        _connection: &'de Connection,
        _method: Self::MethodCall,
    ) -> Self::Reply {
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

    let service = Server {
        connection: Connection,
    };

    let _ = service.run(person).await;
}
