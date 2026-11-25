//! Export server-owned catalog metadata from an operator's reference installation.
use rerbr_probe::{content::Selection, driving::content_manager::Catalog};
use std::{env, fs, path::Path};
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
fn main() {
    let documents = env::args().nth(1).expect("directory containing RBR/");
    let catalog = Catalog::scan_root(Path::new(&documents), &Selection::default());
    for entry in catalog.vehicles {
        let d = entry.descriptor;
        println!("insert into leaderboard_private.vehicles values ({},{}) on conflict (id) do update set name=excluded.name;",quote(&d.id),quote(&d.name));
    }
    for entry in catalog.stages {
        let d = entry.descriptor;
        let maps = Path::new(&documents).join(&d.documents_maps);
        let file = format!("{}.dls", d.stem);
        let Some(data) = [
            maps.join(&file),
            Path::new(&documents).join("RBR/Maps").join(&file),
        ]
        .iter()
        .find_map(|p| fs::read(p).ok()) else {
            continue;
        };
        let Ok(markers) = rerbr_sim::rbr_formats::markers::parse(&data) else {
            continue;
        };
        let clock = rerbr_sim::rbr_gameplay::timing::TimeKeeper::load(&markers);
        println!("insert into leaderboard_private.stages values ({},{},{}) on conflict (id) do update set name=excluded.name,splits=excluded.splits;",quote(&d.id),quote(&d.name),clock.card.splits().len());
    }
}
