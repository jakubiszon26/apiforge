//! A component's inputs edited one at a time (UX-32): the declaration and
//! everything that names it, which is the steps reading it, the board's
//! proxies of it and the component's action.
//!
//! Like the other edits, each borrows the component and returns the next one,
//! so the session keeps the previous one as its undo snapshot.

use crate::format::{Binding, Component, ComponentAction, Port, ProxySource};
use crate::handle::is_valid_handle;
use thiserror::Error;

/// Why a name cannot be an input's. The messages are user-facing.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InputNameError {
    #[error("Not a valid name. Use letters, digits and _, starting with a letter or _.")]
    Invalid,
    #[error("Another input is already named {0}.")]
    Taken(String),
}

/// Whether `name` may name an input of `component`; `own` is the input being
/// renamed, which may keep its name.
pub fn check_input_name(
    component: &Component,
    own: Option<&str>,
    name: &str,
) -> Result<(), InputNameError> {
    if !is_valid_handle(name) {
        return Err(InputNameError::Invalid);
    }
    if own != Some(name) && component.inputs.iter().any(|port| port.name == name) {
        return Err(InputNameError::Taken(name.to_owned()));
    }
    Ok(())
}

/// `port` declared after the component's last input.
pub fn add_input(component: &Component, port: Port) -> Component {
    let mut next = component.clone();
    next.inputs.push(port);
    next
}

/// The input `name` replaced by `port`, which keeps its name; an action fed
/// by it follows a change of its type or contract.
pub fn replace_input(component: &Component, name: &str, port: Port) -> Component {
    let mut next = component.clone();
    let Some(slot) = next.inputs.iter_mut().find(|slot| slot.name == name) else {
        return next;
    };
    let (value_type, contract_id) = (port.value_type, port.contract_id.clone());
    let previous = std::mem::replace(
        slot,
        Port {
            name: name.to_owned(),
            ..port
        },
    );
    if let Some(action) = next.action.as_mut().filter(|action| action.input == name) {
        if action.value_type == previous.value_type {
            action.value_type = value_type;
        }
        if action.contract_id == previous.contract_id {
            action.contract_id = contract_id;
        }
    }
    next
}

/// The input `from` renamed `to`, with every `param.from` the steps read,
/// the board's proxies of it and the action renamed with it.
pub fn rename_input(component: &Component, from: &str, to: &str) -> Component {
    let mut next = component.clone();
    for port in &mut next.inputs {
        if port.name == from {
            port.name = to.to_owned();
        }
    }
    if let Some(action) = next.action.as_mut().filter(|action| action.input == from) {
        action.input = to.to_owned();
    }
    for node in &mut next.graph.nodes {
        for binding in node.inputs.values_mut() {
            if let Some(renamed) = binding.map_leaves(&mut |leaf, _| {
                Some(match leaf {
                    Binding::Parameter { name, path } if name == from => {
                        Binding::parameter_field(to, path.clone())
                    }
                    other => other.clone(),
                })
            }) {
                *binding = renamed;
            }
        }
    }
    for proxy in &mut next.graph.proxies {
        if let ProxySource::Parameter { name } = &mut proxy.source
            && name == from
        {
            *name = to.to_owned();
        }
    }
    next
}

/// The component without the input `name`: each step that read it loses that
/// reading (an operand, field, item or template part, or the whole argument
/// when it read nothing else), and the board's proxies of it and an action
/// fed by it go too.
pub fn delete_input(component: &Component, name: &str) -> Component {
    let mut next = component.clone();
    next.inputs.retain(|port| port.name != name);
    if next
        .action
        .as_ref()
        .is_some_and(|action| action.input == name)
    {
        next.action = None;
    }
    for node in &mut next.graph.nodes {
        let inputs = std::mem::take(&mut node.inputs);
        node.inputs = inputs
            .into_iter()
            .filter_map(|(key, binding)| {
                binding
                    .map_leaves(&mut |leaf, _| match leaf {
                        Binding::Parameter { name: read, .. } if read == name => None,
                        other => Some(other.clone()),
                    })
                    .map(|binding| (key, binding))
            })
            .collect();
    }
    next.graph.proxies.retain(
        |proxy| !matches!(&proxy.source, ProxySource::Parameter { name: read } if read == name),
    );
    next
}

/// The component with its action fed by the input `name`, or by none. A
/// component has one action, so it moves from any other input.
pub fn set_action(component: &Component, name: Option<&str>) -> Component {
    let mut next = component.clone();
    next.action = name.and_then(|name| {
        let port = next.inputs.iter().find(|port| port.name == name)?;
        Some(ComponentAction {
            input: port.name.clone(),
            value_type: port.value_type,
            contract_id: port.contract_id.clone(),
        })
    });
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demos::shop_project;
    use crate::format::{OptionalJson, Position, Proxy};
    use crate::model::ValueType;
    use serde_json::json;

    fn cart() -> Component {
        shop_project()
            .components
            .into_iter()
            .find(|component| component.name == "Add to cart")
            .expect("the shop's Add to cart")
    }

    fn reads(component: &Component, name: &str) -> usize {
        let mut count = 0;
        for node in &component.graph.nodes {
            for binding in node.inputs.values() {
                binding.for_each_leaf(&mut |leaf, _| {
                    if matches!(leaf, Binding::Parameter { name: read, .. } if read == name) {
                        count += 1;
                    }
                });
            }
        }
        count
    }

    #[test]
    fn a_name_must_be_an_identifier_and_unique() {
        let cart = cart();
        assert_eq!(check_input_name(&cart, None, "currency"), Ok(()));
        assert_eq!(
            check_input_name(&cart, None, "1 bad"),
            Err(InputNameError::Invalid)
        );
        assert_eq!(
            check_input_name(&cart, None, ""),
            Err(InputNameError::Invalid)
        );
        assert_eq!(
            check_input_name(&cart, Some("quantity"), "product"),
            Err(InputNameError::Taken("product".into()))
        );
        assert_eq!(
            check_input_name(&cart, Some("quantity"), "quantity"),
            Ok(())
        );
    }

    #[test]
    fn a_rename_follows_every_reading_the_proxies_and_the_action() {
        let mut cart = cart();
        cart.graph.proxies.push(Proxy {
            id: "proxy-1".into(),
            source: ProxySource::Parameter {
                name: "product".into(),
            },
            position: Position { x: 0.0, y: 0.0 },
        });
        let readings = reads(&cart, "product");
        assert!(readings > 0);
        let renamed = rename_input(&cart, "product", "item");
        assert_eq!(renamed.inputs[0].name, "item");
        assert_eq!(reads(&renamed, "product"), 0);
        assert_eq!(reads(&renamed, "item"), readings);
        assert_eq!(renamed.action.as_ref().unwrap().input, "item");
        assert_eq!(
            renamed.graph.proxies[0].source,
            ProxySource::Parameter {
                name: "item".into()
            }
        );
        // The field read from it is kept: `param.item.id`.
        let id = renamed
            .graph
            .nodes
            .iter()
            .find(|node| node.label.as_deref() == Some("Product ID"))
            .unwrap();
        assert_eq!(
            id.inputs["value"],
            Binding::parameter_field("item", "id"),
            "{:?}",
            id.inputs
        );
    }

    #[test]
    fn a_deleted_input_takes_its_readings_and_its_action() {
        let cart = cart();
        let deleted = delete_input(&cart, "product");
        assert!(deleted.inputs.iter().all(|port| port.name != "product"));
        assert_eq!(reads(&deleted, "product"), 0);
        assert!(deleted.action.is_none());
        // `quantity` is a field of Cart item's object: only that field goes.
        let deleted = delete_input(&cart, "quantity");
        let body = deleted
            .graph
            .nodes
            .iter()
            .find(|node| node.label.as_deref() == Some("Cart item"))
            .unwrap();
        let Binding::Object { fields } = &body.inputs["value"] else {
            panic!("an object: {:?}", body.inputs);
        };
        assert_eq!(fields.keys().collect::<Vec<_>>(), ["productId"]);
        assert_eq!(deleted.action, cart.action, "the action is product's");
    }

    #[test]
    fn the_action_moves_and_follows_its_input_type() {
        let cart = cart();
        let moved = set_action(&cart, Some("quantity"));
        let action = moved.action.as_ref().unwrap();
        assert_eq!(
            (
                action.input.as_str(),
                action.value_type,
                &action.contract_id
            ),
            ("quantity", ValueType::Number, &None)
        );
        assert!(set_action(&cart, None).action.is_none());
        let port = Port {
            value_type: ValueType::String,
            ..moved.inputs[1].clone()
        };
        let retyped = replace_input(&moved, "quantity", port);
        assert_eq!(
            retyped.action.as_ref().unwrap().value_type,
            ValueType::String
        );
    }

    #[test]
    fn an_added_input_comes_last() {
        let cart = cart();
        let port = Port {
            default_value: OptionalJson::Present(json!("EUR")),
            ..Port::new("currency", ValueType::String)
        };
        let added = add_input(&cart, port.clone());
        assert_eq!(added.inputs.last(), Some(&port));
        assert_eq!(added.inputs.len(), cart.inputs.len() + 1);
    }
}
