use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Service<Impl> {
    service: Impl,
    connection: Connection,
}

impl<'service, Impl> Service<Impl>
where
    Impl: ServiceImpl<'service>,
{
    async fn handle_next(&'service mut self) -> Result<(), ()> {
        let call: Impl::MethodCall = serde_json::from_str("{ \"x\": 32 }").unwrap();
        let _: Impl::Reply = self.service.handle(call).await;

        Ok(())
    }
}

trait ServiceImpl<'service> {
    type MethodCall: Deserialize<'service>;
    type Reply: Serialize + 'service;

    async fn handle(&'service mut self, method: Self::MethodCall) -> Self::Reply;
}

pub struct Connection;

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
    type MethodCall = ();
    type Reply = &'service str;

    async fn handle(&'service mut self, _method: ()) -> &'service str {
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

    let mut service = Service {
        service: person,
        connection: Connection,
    };

    let _ = service.handle_next().await;
}
