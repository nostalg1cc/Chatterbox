// Release builds run away from the repo, so the project's public Supabase address and
// publishable key are built in, along with the app icon.
fn main() {
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-changed=assets/nitro.ico");
    if let Ok(text) = std::fs::read_to_string("../.env") {
        for (name, built_in) in [
            ("VITE_SUPABASE_URL", "NITRO_SUPABASE_URL"),
            ("VITE_SUPABASE_ANON_KEY", "NITRO_SUPABASE_KEY"),
        ] {
            let value = text.lines().find_map(|line| {
                line.trim().strip_prefix(name)?.strip_prefix('=').map(|rest| rest.trim().trim_matches('"').to_string())
            });
            if let Some(value) = value {
                println!("cargo:rustc-env={built_in}={value}");
            }
        }
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/nitro.ico");
        resource.set("ProductName", "Nitro");
        resource.set("FileDescription", "Nitro");
        resource.compile().expect("the app icon must compile");
    }
}
