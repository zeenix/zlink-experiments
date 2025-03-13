use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Service<Impl> {
    service: Impl,
    connection: Connection,
}

impl<'service, Impl> Service<Impl>
where
    Impl: ServiceImpl<'service> + 'service,
{
    async fn run(mut self) {
        loop {
            let service = unsafe { &mut *(&mut self as *mut Self) };
            if let Err(_) = service.handle_next().await {
                break;
            }
        }
    }

    async fn handle_next(&'service mut self) -> Result<(), ()> {
        let call: Impl::MethodCall =
            serde_json::from_str(self.connection.read_json_from_socket()).unwrap();
        let _: Impl::Reply = self.service.handle(&self.connection, call).await;

        Ok(())
    }
}

trait ServiceImpl<'service> {
    type MethodCall: Deserialize<'service>;
    type Reply: Serialize;

    async fn handle(
        &'service mut self,
        connection: &'service Connection,
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

impl<'service> ServiceImpl<'service> for Wizard {
    type MethodCall = &'service str;
    type Reply = &'service str;

    async fn handle(
        &'service mut self,
        _connection: &'service Connection,
        _method: &'service str,
    ) -> &'service str {
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

    let service = Service {
        service: person,
        connection: Connection,
    };

    let _ = service.run().await;
}
