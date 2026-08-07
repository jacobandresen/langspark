//! Downloads the Japanese dictionary (JMdict + Kanjidic) to the standard XDG
//! data directory, matching the layout `langspark-gui`'s `config::AppDirs`
//! expects (`<data_dir>/dictionaries/<code>.json`). Used by
//! `scripts/install.sh` so a fresh install has the dictionary ready
//! immediately, without needing to open Preferences first.
//!
//! Safe to re-run: `install_jmdict`/`install_kanjidic` always fetch the
//! latest release and overwrite the destination file.

fn main() -> anyhow::Result<()> {
    let dirs = directories::ProjectDirs::from("", "", "langspark")
        .ok_or_else(|| anyhow::anyhow!("couldn't determine a home directory for the dictionaries dir"))?;
    let dict_dir = dirs.data_dir().join("dictionaries");
    std::fs::create_dir_all(&dict_dir)?;

    let jmdict_dest = dict_dir.join("ja.json");
    let kanjidic_dest = dict_dir.join("kanjidic.json");

    print!("Downloading JMdict... ");
    let version = langspark_core::install_jmdict(&jmdict_dest, &|_, _| {})?;
    println!("done ({version})");

    print!("Downloading Kanjidic... ");
    langspark_core::install_kanjidic(&kanjidic_dest, &|_, _| {})?;
    println!("done");

    println!("Dictionary installed to {}", dict_dir.display());
    Ok(())
}
