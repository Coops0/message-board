use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=templates/");
    println!("cargo:rerun-if-changed=assets/user-script.js");
    
    let status = Command::new("./tailwindcss")
        .args(["-i", "assets/index.css", "-o", "assets/ts.css", "-m"])
        .status()
        .unwrap();
    
    assert!(status.success());
}
