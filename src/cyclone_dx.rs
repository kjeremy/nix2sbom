use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{de::Deserialize, ser::Serialize};

use serde_cyclonedx::cyclonedx::v_1_4::{
    Component, ComponentBuilder, CycloneDxBuilder, Metadata, ToolBuilder,
};

pub fn dump(derivations: &crate::nix::Derivations, packages: &crate::nix::Packages) -> String {
    let mut metadata = Metadata::default();
    let now = SystemTime::now();
    let now: DateTime<Utc> = now.into();
    metadata.timestamp = Some(now.to_rfc3339());

    metadata.tools = Some(vec![ToolBuilder::default()
        .vendor("louib".to_string())
        .name("nix2sbom".to_string())
        .version(env!("CARGO_PKG_VERSION"))
        .build()
        .unwrap()]);

    let mut components: Vec<Component> = vec![];
    for (derivation_path, derivation) in derivations.iter() {
        components.push(dump_derivation(derivation_path, derivation, packages));
    }

    let cyclonedx = CycloneDxBuilder::default()
        .bom_format("CycloneDX")
        .spec_version("1.4")
        .version(1)
        .metadata(metadata)
        .components(components)
        .build()
        .unwrap();

    "".to_string()
}

pub fn dump_derivation(
    derivation_path: &str,
    derivation: &crate::nix::Derivation,
    packages: &crate::nix::Packages,
) -> Component {
    // TODO handle if the package metadata was not found.
    let package = packages.get(derivation_path).unwrap();
    ComponentBuilder::default()
        .bom_ref(derivation_path.to_string())
        .name(package.name.to_string())
        .description("TODO".to_string())
        .cpe("TODO".to_string())
        // TODO application is the generic type, but we should also use file and library
        // also, populate the mime_type in case of a file type.
        .type_("application".to_string())
        // I'm assuming here that if a package has been installed by Nix, it was required.
        .scope("required".to_string())
        .purl("TODO".to_string())
        .publisher("TODO".to_string())
        .version("TODO".to_string())
        .build()
        .unwrap()
}
