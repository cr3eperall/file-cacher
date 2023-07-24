
use file_cacher::Cacher;

#[tokio::main]
async fn main() {
    let mut cacher = Cacher::new(None);
    let res = cacher.get("https://speed.hetzner.de/100MB.bin", "100mb.zip").await;
    println!("{:#?}", res);
    let res = cacher.save();
    println!("{:#?}", res);
}
