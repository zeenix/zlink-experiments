use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Service<Impl> {
    service: Impl,
    connection: Connection,
}

impl<Impl> Service<Impl>
where
    Impl: ServiceImpl,
{
    async fn handle_next<'s, MethodCall, Reply>(&'s mut self) -> Result<(), ()>
    where
        MethodCall: Deserialize<'s>,
        Reply: Serialize + 's,
    {
        let call: MethodCall = serde_json::from_str("{ \"x\": 32 }").unwrap();
        let _: Reply = self.service.handle(&self.connection, call).await;

        Ok(())
    }
}

trait ServiceImpl {
    for<'de> type MethodCall: Deserialize<'de>;
    for<'ser> type Reply: Serialize + ser;
{
    async fn handle(&'ser mut self, conn: &'de Connection, method: MethodCall) -> Reply;
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

impl ServiceImpl<(), &str> for Wizard {
    async fn handle<'ser, 'de, M, R>(
        &'ser mut self,
        conn: &'de Connection,
        method: (),
    ) -> &'ser str {
        unimplemented!()
    }
}

fn main() {
    let data = r#"
        {
            "name": "Harry Potter",
            "age": 17,
            "unknown_extra_field": "This unknown field is extra and will be ignored"
        }
    "#;

    let mut person = serde_json::from_str::<Wizard>(data).expect("Failed to deserialize JSON");

    println!("Deserialized struct: {person:?}");

    let mut service = Service;

    let _ = service.handle_next(async |s, m| person.handle(s, m).await);
}
