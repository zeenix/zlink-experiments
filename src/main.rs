use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Service;

impl Service {
    async fn handle_next<'h, Handler, MethodCall, Reply>(
        &'h mut self,
        mut handler: Handler,
    ) -> Result<(), ()>
    where
        Handler: AsyncFnMut(&'h mut Self, MethodCall) -> Reply,
        MethodCall: Deserialize<'h>,
        for<'r> Reply: Serialize + 'r,
    {
        let call: MethodCall = serde_json::from_str("{ \"x\": 32 }").unwrap();
        let _: Reply = handler(self, call).await;

        Ok(())
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

    async fn handle(&mut self, _service: &mut Service, _method: ()) -> &str {
        &self.name
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

    let _ = service.handle_next(async move |s, m| person.handle(s, m).await);
}
