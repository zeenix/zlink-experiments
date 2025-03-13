use serde::{Deserialize, Serialize};
use serde_json; // 1.0.138

struct Service {
    connection: Connection,
}

impl Service {
    async fn run<'ser, Impl>(mut self, service_impl: &'ser mut Impl)
    where
        for<'de> Impl: ServiceImpl<'de, 'ser>,
    {
        loop {
            let service = unsafe { &mut *(&mut self as *mut Self) };
            let service_impl = unsafe { &mut *(service_impl as *mut Impl) };
            if let Err(_) = service.handle_next(service_impl).await {
                break;
            }
        }
    }

    async fn handle_next<'de, 'ser, Impl>(
        &'de mut self,
        service_impl: &'ser mut Impl,
    ) -> Result<(), ()>
    where
        Impl: ServiceImpl<'de, 'ser>,
    {
        let call: Impl::MethodCall =
            serde_json::from_str(self.connection.read_json_from_socket()).unwrap();
        let _: Impl::Reply = service_impl.handle(&self.connection, call).await;

        Ok(())
    }
}

trait ServiceImpl<'de, 'ser> {
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

impl<'de, 'ser> ServiceImpl<'de, 'ser> for Wizard {
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

    let mut person = serde_json::from_str::<Wizard>(data).expect("Failed to deserialize JSON");

    println!("Deserialized struct: {person:?}");

    let service = Service {
        connection: Connection,
    };

    let _ = service.run(&mut person).await;
}
