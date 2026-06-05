fn main() -> anyhow::Result<()> {
    let greeting = logic::greet("world")?;
    println!("{greeting}");
    Ok(())
}
