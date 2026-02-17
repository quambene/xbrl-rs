use env_logger::Env;
use log::info;
use xbrl_rs::TaxonomyLoader;

fn main() -> Result<(), String> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let destination_root = "test_data/taxonomies";
    let entry_urls = [
        "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd",
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-fi-2020-04-01/de-fi-2020-04-01-shell-staffelform-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-ins-2020-04-01/de-ins-2020-04-01-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-pi-2020-04-01/de-pi-2020-04-01-shell-staffelform-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-gcd-2021-04-14/de-gcd-2021-04-14-shell.xsd",
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2021-04-14/de-gaap-ci-2021-04-14-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-bra-2021-04-14/de-bra-2021-04-14-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-fi-2021-04-14/de-fi-2021-04-14-shell-staffelform-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-ins-2021-04-14/de-ins-2021-04-14-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-pi-2021-04-14/de-pi-2021-04-14-shell-staffelform-fiscal.xsd",
    ];
    let loader = TaxonomyLoader::new().map_err(|err| format!("Failed to create loader: {err}"))?;

    info!("Downloadeding taxonomy files to {destination_root}");

    loader
        .download_all(entry_urls, destination_root)
        .map_err(|err| format!("Download failed: {err}"))?;

    info!("Downloaded taxonomy files to {destination_root}");

    Ok(())
}
