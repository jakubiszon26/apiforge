//! Preparing projects for export and upgrading stored ones.
use crate::format::Project;

/// A copy of the project without the values of secret variables.
///
/// Secret names are kept, so an imported project still asks for their values.
pub fn public_project(project: &Project) -> Project {
    let mut clone = project.clone();
    let environment = &mut clone.environment;
    for name in &environment.secret_names {
        environment.variables.shift_remove(name);
    }
    clone
}

/// The project with a legacy demo address moved from port 3000 to port 4318.
///
/// Only `http://127.0.0.1:3000/demo/shop` and `.../demo/support` (also on
/// `localhost`, with an optional trailing `/`) are rewritten; any other base URL
/// is left alone.
pub fn migrate_demo_address(project: &Project) -> Project {
    let mut migrated = project.clone();
    if let Some(address) = migrated_demo_address(&project.environment.base_url) {
        migrated.environment.base_url = address;
    }
    migrated
}

fn migrated_demo_address(base_url: &str) -> Option<String> {
    let host = ["http://127.0.0.1", "http://localhost"]
        .into_iter()
        .find(|host| base_url.starts_with(host))?;
    let rest = base_url[host.len()..].strip_prefix(":3000/demo/")?;
    let demo = rest.strip_suffix('/').unwrap_or(rest);
    matches!(demo, "shop" | "support").then(|| format!("{host}:4318/demo/{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::test_support::at;
    use indexmap::IndexMap;

    #[test]
    fn secret_values_are_removed_without_touching_the_source() {
        let mut project = at("shop.json");
        project.environment.secret_names = vec!["token".into()];
        project.environment.variables = IndexMap::from([
            ("token".into(), "private".into()),
            ("region".into(), "pl".into()),
        ]);
        let clean = public_project(&project);
        assert_eq!(
            clean.environment.variables,
            IndexMap::from([("region".to_owned(), "pl".to_owned())])
        );
        assert_eq!(clean.environment.secret_names, ["token"]);
        assert_eq!(project.environment.variables["token"], "private");
    }

    #[test]
    fn demo_projects_survive_a_public_export_and_import() {
        for name in ["shop.json", "tickets.json", "blocks.json", "workflow.json"] {
            let project = at(name);
            crate::validate_project(&project).unwrap();
            let exported = serde_json::to_vec(&public_project(&project)).unwrap();
            let imported = crate::import_document(&exported, &crate::ImportLimits::default())
                .unwrap()
                .into_workspace()
                .projects
                .remove(0);
            assert_eq!(migrate_demo_address(&imported), project, "{name}");
        }
    }

    #[test]
    fn only_legacy_demo_addresses_move_to_the_new_port() {
        let cases = [
            (
                "http://127.0.0.1:3000/demo/shop",
                "http://127.0.0.1:4318/demo/shop",
            ),
            (
                "http://localhost:3000/demo/support/",
                "http://localhost:4318/demo/support/",
            ),
            (
                "http://localhost:3000/demo/other",
                "http://localhost:3000/demo/other",
            ),
            (
                "http://localhost:3000/demo/shop/x",
                "http://localhost:3000/demo/shop/x",
            ),
            (
                "https://localhost:3000/demo/shop",
                "https://localhost:3000/demo/shop",
            ),
            (
                "http://example.com:3000/demo/shop",
                "http://example.com:3000/demo/shop",
            ),
            (
                "http://127.0.0.1:4318/demo/shop",
                "http://127.0.0.1:4318/demo/shop",
            ),
        ];
        let mut project = at("shop.json");
        for (before, after) in cases {
            project.environment.base_url = before.into();
            let migrated = migrate_demo_address(&project);
            assert_eq!(migrated.environment.base_url, after, "{before}");
            assert_eq!(migrated.scenarios, project.scenarios);
        }
    }
}
