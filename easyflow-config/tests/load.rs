use anyhow::Result;
use std::{fs, path::Path};

#[test]
fn parse_graph_test() -> Result<()> {
    let config_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/config-examples"));

    for entry in fs::read_dir(config_dir)? {
        let entry = entry?;
        let path = entry.path();

        // filter out files which extensions are not ".json5"
        let Some(ext) = path.extension() else {
            continue;
        };
        if ext != "json5" {
            continue;
        }

        // let _graph = Dataflow::open(&path)?;
    }

    Ok(())
}
