//! Prints a preset as the JSON that `new` takes.
//!
//!   cargo run -q -p codeblue-core --features serde --example config -- demo
//!   cargo run -q -p codeblue-core --features serde --example config -- live
//!
//! Further arguments override single values, for a deployment that needs a
//! shorter life than the preset gives: `-- demo no_flow_max_blocks=60`.
use codeblue_core::Config;

fn main() {
    let mut args = std::env::args().skip(1);
    let preset = args.next().unwrap_or_else(|| "demo".into());
    let config = match preset.as_str() {
        "demo" => Config::demo(),
        "live" => Config::live(),
        other => {
            eprintln!("unknown preset {other}: use demo or live");
            std::process::exit(2);
        }
    };
    let mut json = serde_json::to_value(&config).expect("a config is plain data");
    for pair in args {
        let (key, value) = pair.split_once('=').expect("overrides look like key=value");
        let slot = json.get_mut(key).unwrap_or_else(|| panic!("no parameter named {key}"));
        *slot = if slot.is_string() {
            serde_json::Value::String(value.to_string())
        } else {
            serde_json::from_str(value).expect("a number or a boolean")
        };
    }
    let config: Config = serde_json::from_value(json).expect("the overrides keep the config well-formed");
    config.validate().unwrap_or_else(|e| panic!("invalid config: {e}"));
    println!("{}", serde_json::to_string_pretty(&config).unwrap());
}
