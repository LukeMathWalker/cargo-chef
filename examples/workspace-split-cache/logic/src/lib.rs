use model::Greeting;

pub fn greet(name: &str) -> anyhow::Result<String> {
    let g = Greeting {
        name: name.to_string(),
    };
    Ok(serde_json::to_string(&g)?)
}
