use boson::{
    ImmutableBuilder,
    dht::Node,
};

/// Builds an immutable value from `data` and announces (stores) it to the
/// network through `node`.
pub(crate) async fn announce(node: &Node, data: &str) {
    let value = match ImmutableBuilder::new(data.as_bytes()).build() {
        Ok(v) => v,
        Err(e) => {
            println!("Building value failed: {e}");
            return;
        }
    };

    println!("Announcing value {} ({} bytes) ...", value.id(), value.data().len());
    match node.store_value(&value, -1, true).await {
        Ok(_) => println!("\x1b[32mValue announced successfully.\x1b[0m"),
        Err(e) => println!("\x1b[31mFailed to announce value: {}\x1b[0m", e),
    }
}
