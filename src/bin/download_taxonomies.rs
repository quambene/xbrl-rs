use xbrl_rs::TaxonomyLoader;

fn main() -> Result<(), String> {
    let destination_root = "test_data/taxonomies_downloaded";
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
        "http://www.xbrl.de/taxonomies/de-gcd-2022-05-02/de-gcd-2022-05-02-shell.xsd",
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2022-05-02/de-gaap-ci-2022-05-02-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-bra-2022-05-02/de-bra-2022-05-02-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-fi-2022-05-02/de-fi-2022-05-02-shell-staffelform-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-ins-2022-05-02/de-ins-2022-05-02-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-pi-2022-05-02/de-pi-2022-05-02-shell-staffelform-fiscal.xsd",
    ];
    let loader = TaxonomyLoader::new().map_err(|err| format!("Failed to create loader: {err}"))?;

    loader
        .download_all(entry_urls, destination_root)
        .map_err(|err| format!("Download failed: {err}"))?;

    println!("Downloaded taxonomy files to {destination_root}");

    Ok(())
}
