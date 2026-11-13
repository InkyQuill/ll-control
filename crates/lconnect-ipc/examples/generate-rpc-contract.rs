use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/lib/rpc-contract.ts");
    let expected = lconnect_ipc::methods::typescript_contract();
    if std::env::args().any(|argument| argument == "--write") {
        std::fs::write(path, expected)?;
        return Ok(());
    }
    let actual = std::fs::read_to_string(path)?;
    if actual != expected {
        return Err("desktop/src/lib/rpc-contract.ts is stale; run cargo run -p lconnect-ipc --example generate-rpc-contract -- --write".into());
    }
    println!(
        "Validated {} registered RPC methods and TypeScript contract",
        lconnect_ipc::methods::METHODS.len()
    );
    Ok(())
}
