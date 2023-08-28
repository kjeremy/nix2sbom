use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{de::Deserialize, ser::Serialize};

use serde_cyclonedx::cyclonedx::v_1_4::{
    Commit, CommitBuilder, Component, ComponentBuilder, ComponentPedigree, ComponentPedigreeBuilder,
    CycloneDxBuilder, Dependency, DependencyBuilder, ExternalReference, ExternalReferenceBuilder, License,
    LicenseBuilder, LicenseChoice, Metadata, ToolBuilder,
};

const CURRENT_SPEC_VERSION: &str = "1.4";

pub fn dump(
    package_graph: &crate::nix::PackageGraph,
    format: &crate::sbom::SerializationFormat,
) -> Result<String, String> {
    let mut metadata = Metadata::default();
    let now = SystemTime::now();
    let now: DateTime<Utc> = now.into();
    metadata.timestamp = Some(now.to_rfc3339());

    metadata.tools = Some(vec![ToolBuilder::default()
        .vendor("louib".to_string())
        .name(crate::consts::PROJECT_NAME.to_string())
        .version(env!("CARGO_PKG_VERSION"))
        .build()
        .unwrap()]);

    let mut components: Vec<Component> = vec![];
    for (derivation_path, package) in package_graph.iter() {
        if let Some(component) = dump_derivation(derivation_path, package) {
            components.push(component);
        }
    }

    let mut dependencies: Vec<Dependency> = vec![];
    for (derivation_path, package) in package_graph.iter() {
        if package.children.len() == 0 {
            continue;
        }
        let mut dependency_builder = DependencyBuilder::default();
        dependency_builder.ref_(derivation_path);
        let mut depends_on: Vec<String> = vec![];
        for child in package.children.iter() {
            depends_on.push(child.to_string());
        }
        dependency_builder.depends_on(depends_on);
        dependencies.push(dependency_builder.build().unwrap());
    }

    let cyclonedx = CycloneDxBuilder::default()
        .bom_format(crate::sbom::CYCLONE_DX_NAME)
        .spec_version(CURRENT_SPEC_VERSION)
        .version(1)
        .metadata(metadata)
        .components(components)
        .dependencies(dependencies)
        .build()
        .unwrap();

    match format {
        crate::sbom::SerializationFormat::JSON => {
            serde_json::to_string_pretty(&cyclonedx).map_err(|e| e.to_string())
        }
        crate::sbom::SerializationFormat::YAML => serde_yaml::to_string(&cyclonedx).map_err(|e| e.to_string()),
        crate::sbom::SerializationFormat::XML => Err("XML is not supported for CycloneDX".to_string()),
    }
}

pub fn dump_derivation(derivation_path: &str, package_node: &crate::nix::PackageNode) -> Option<Component> {
    log::debug!("Dumping derivation for {}", &derivation_path);
    let mut component_builder = ComponentBuilder::default();

    component_builder.bom_ref(derivation_path.to_string());
    component_builder.name(package_node.package.name.to_string());
    // component_builder.cpe("TODO".to_string())
    // TODO application is the generic type, but we should also use file and library
    // also, populate the mime_type in case of a file type.
    component_builder.type_("application".to_string());
    // I'm assuming here that if a package has been installed by Nix, it was required.
    component_builder.scope("required".to_string());
    component_builder.purl(package_node.package.get_purl());
    component_builder.version(package_node.package.version.to_string());

    if let Some(description) = &package_node.package.meta.description {
        component_builder.description(description.to_string());
    }

    if let Some(author) = get_author(&package_node) {
        component_builder.author(author);
    }

    let mut external_references: Vec<ExternalReference> = get_external_references(&package_node);
    if external_references.len() != 0 {
        component_builder.external_references(external_references);
    }

    let commits = get_commits(&package_node.patches);
    if commits.len() != 0 {
        let mut pedigree_builder = ComponentPedigreeBuilder::default();
        pedigree_builder.commits(commits);
        component_builder.pedigree(pedigree_builder.build().unwrap());
    }

    let licenses = get_licenses(&package_node.package.meta.get_licenses());
    if licenses.len() != 0 {
        component_builder.licenses(licenses);
    }

    Some(component_builder.build().unwrap())
}

fn get_author(package_node: &crate::nix::PackageNode) -> Option<String> {
    let maintainers = package_node.package.meta.get_maintainers();
    if maintainers.len() == 0 {
        return None;
    }
    let author = maintainers
        .iter()
        .map(|m| format!("{} ({})", m.name, m.email))
        .collect::<Vec<String>>()
        .join(" ");
    if author.len() != 0 {
        return Some(author);
    }
    None
}

fn get_commits(patches: &Vec<crate::nix::Derivation>) -> Vec<Commit> {
    let mut response: Vec<Commit> = vec![];
    if patches.len() != 0 {
        let mut commits: Vec<Commit> = vec![];
        for patch in patches {
            let mut commit = CommitBuilder::default();
            commit.url(patch.get_url().unwrap());
            // TODO we could also populate the uid, which is the commit SHA
            commits.push(commit.build().unwrap())
        }
    }
    response
}

fn get_external_references(package_node: &crate::nix::PackageNode) -> Vec<ExternalReference> {
    let mut external_references: Vec<ExternalReference> = vec![];
    for homepage in package_node.package.meta.get_homepages() {
        let mut external_reference_builder = ExternalReferenceBuilder::default();
        // See https://docs.rs/serde-cyclonedx/latest/serde_cyclonedx/cyclonedx/v_1_5/struct.ExternalReference.html#structfield.type_
        // for all the available external reference types
        external_reference_builder.type_("website");
        external_reference_builder.url(homepage.to_string());
        external_references.push(external_reference_builder.build().unwrap());
    }
    for source in &package_node.sources {
        let source_url = match source.get_url() {
            Some(u) => u,
            None => continue,
        };
        if let Some(git_url) = crate::utils::get_git_url_from_generic_url(&source_url) {
            log::debug!("Found git url {} for source URL {}", &git_url, &source_url);
            let mut external_reference_builder = ExternalReferenceBuilder::default();
            external_reference_builder.type_("vcs");
            external_reference_builder.url(git_url);
            external_references.push(external_reference_builder.build().unwrap());
        }
    }
    external_references
}

fn get_licenses(licenses: &Vec<crate::nix::PackageLicense>) -> Vec<LicenseChoice> {
    let mut response: Vec<LicenseChoice> = vec![];
    for license in licenses {
        match license {
            crate::nix::PackageLicense::Name(n) => {
                response.push(LicenseChoice {
                    expression: Some(n.to_string()),
                    license: None,
                });
            }
            crate::nix::PackageLicense::Details(license_details) => {
                let mut license_builder = LicenseBuilder::default();
                match &license_details.spdx_id {
                    None => continue,
                    Some(id) => license_builder.id(id),
                };
                if let Some(full_name) = &license_details.full_name {
                    license_builder.name(full_name);
                }
                response.push(LicenseChoice {
                    expression: None,
                    license: Some(license_builder.build().unwrap()),
                });
            }
        }
    }
    response
}
