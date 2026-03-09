use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{bail, Result};

use zellij_status::schema::ConfigSchema;

fn main() -> Result<()> {
    let mut check = false;
    let mut output_path: Option<PathBuf> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--output" => {
                let Some(path) = args.next() else {
                    bail!("--output requires a path argument");
                };
                output_path = Some(PathBuf::from(path));
            }
            other => bail!("unknown argument: {other}"),
        }
    }

    let schema = ConfigSchema::load_default()?;
    let rendered = schema.render_reference();
    let output_path = output_path.unwrap_or_else(ConfigSchema::default_reference_path);

    if check {
        let existing = match fs::read_to_string(&output_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                bail!(
                    "generated config reference is missing: {}",
                    output_path.display()
                )
            }
            Err(error) => {
                bail!(
                    "failed to read generated config reference {}: {}",
                    output_path.display(),
                    error
                )
            }
        };
        if existing != rendered {
            bail!(
                "generated config reference is out of date: {}",
                output_path.display()
            );
        }
        return Ok(());
    }

    fs::write(&output_path, rendered)?;
    println!("wrote {}", output_path.display());
    Ok(())
}
