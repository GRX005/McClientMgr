use std::fs;
use std::io::Error;

pub fn makeFolders() ->Result<(), Error> {
    fs::create_dir_all("libraries")?;
    fs::create_dir_all("natives")?;
    fs::create_dir_all("assets")?;
    Ok(())
}