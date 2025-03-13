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
    async fn handle<'s, 'c, MethodCall, Reply>(
        &'s mut self,
        conn: &'c Connection,
        method: MethodCall,
    ) -> Reply
    where
        MethodCall: Deserialize<'c>,
        Reply: Serialize + 's;
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

impl ServiceImpl for Wizard {
    async fn handle<'s, 'c, MethodCall, Reply>(
        &'s mut self,
        conn: &'c Connection,
        method: MethodCall,
    ) -> Reply
    where
        MethodCall: Deserialize<'c>,
        Reply: Serialize + 's,
    {
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
