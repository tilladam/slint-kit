fn main() {
    let entries = ["Open file", "Close window", "Open folder"];
    let matches = palette_rank::rank(&entries, "opfi", 10, |s| s.to_string());
    assert_eq!(matches, ["Open file"]);
    println!("{matches:?}");
}
