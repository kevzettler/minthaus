use std::collections::HashMap;
extern crate psd;

use base64::{engine::general_purpose, Engine as _};
use psd::{Psd, PsdGroup, PsdLayer};

enum PsdNodeType {
    Group(PsdGroup),
    Layer(PsdLayer),
}

struct PsdNode {
    content: Option<PsdNodeType>,
    children: Vec<PsdNode>,
}

// Define a type alias for the closure that will be applied to each node.
// The closure takes a reference to a PsdNode and returns nothing.
type NodeAction<'a> = Box<dyn Fn(&PsdNode, usize) + 'a>; // Now also takes the depth as an argument

fn main() {
    // Example: Encode a token
    let token = "FFFF00000300030003"; // token corresponding to layers
    let encoded_token = general_purpose::URL_SAFE_NO_PAD.encode(token);
    println!("Encoded Token: {}", encoded_token);

    let input_token = encoded_token;
    let decoded_token = match general_purpose::URL_SAFE_NO_PAD.decode(input_token.as_bytes()) {
        Ok(decoded) => String::from_utf8_lossy(&decoded).to_string(), // Convert to owned String
        Err(e) => {
            println!("Failed to decode token: {}", e);
            return;
        }
    };

    let psd_bytes = include_bytes!("../kz-wingrivals-chars-final.psd");
    let psd = Psd::from_bytes(psd_bytes).unwrap();

    // Assuming `psd_tree` is a PsdNode representing the root of your PSD structure
    let psd_tree = build_psd_tree(&psd); // Replace with actual call to build your tree

    // Define a closure for pretty printing each node's name with indentation
    let print_node_name: NodeAction = Box::new(|node, depth| {
        let indentation = " ".repeat(depth * 4); // 4 spaces per depth level
        match &node.content {
            Some(PsdNodeType::Group(group)) => println!("{}Group: {}", indentation, group.name()),
            Some(PsdNodeType::Layer(layer)) => println!("{}Layer: {}", indentation, layer.name()),
            None => println!("{}Root Node", indentation),
        }
    });

    // Traverse the tree and print each node's name
    traverse_psd_tree(&psd_tree, 0, &print_node_name);
}

fn get_top_level_groups(psd: &Psd) -> Vec<(u32, &PsdGroup)> {
    psd.groups()
        .iter()
        .filter_map(|(&id, group)| {
            if group.parent_id().is_none() && !group.name().starts_with("_bg") {
                Some((id, group))
            } else {
                None
            }
        })
        .collect()
}

fn decode_hex_to_visibility(hex_str: &str) -> Vec<u8> {
    (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).unwrap_or(0))
        .collect()
}

fn build_psd_tree(psd: &Psd) -> PsdNode {
    let groups = psd.groups();
    let group_ids_in_order = psd.group_ids_in_order();
    let layers = psd.layers();

    let mut root_node = PsdNode {
        content: None, // None signifies the root node
        children: vec![],
    };

    // Helper function to recursively build a subtree for a group
    fn build_group_subtree(psd: &Psd, group_id: u32) -> PsdNode {
        let group = psd.groups().get(&group_id).unwrap();

        let mut children = Vec::new();
        for (sub_group_id, sub_group) in psd.groups() {
            if sub_group.parent_id() == Some(group_id) {
                children.push(build_group_subtree(psd, *sub_group_id));
            }
        }

        for layer in psd.layers() {
            if layer.parent_id() == Some(group_id) {
                children.push(PsdNode {
                    content: Some(PsdNodeType::Layer(layer.clone())),
                    children: vec![],
                });
            }
        }

        PsdNode {
            content: Some(PsdNodeType::Group(group.clone())),
            children,
        }
    }

    // Add top-level groups in order
    for group_id in group_ids_in_order {
        if groups.get(group_id).unwrap().parent_id().is_none() {
            root_node.children.push(build_group_subtree(psd, *group_id));
        }
    }

    // Keep track of layers that have been added to groups
    let mut added_layers = std::collections::HashSet::new();

    // Add layers to their respective groups first
    for (index, layer) in layers.iter().enumerate() {
        if let Some(parent_id) = layer.parent_id() {
            if groups.contains_key(&parent_id) {
                // This layer is part of a group and will be added in the build_group_subtree function
                added_layers.insert(format!(
                    "{}-{:?}-{}",
                    layer.name(),
                    layer.parent_id(),
                    index
                ));
            }
        }
    }

    // Add remaining top-level layers to the root
    for (index, layer) in layers.iter().enumerate() {
        let layer_identifier = format!("{}-{:?}-{}", layer.name(), layer.parent_id(), index);
        if !added_layers.contains(&layer_identifier) {
            root_node.children.push(PsdNode {
                content: Some(PsdNodeType::Layer(layer.clone())),
                children: vec![],
            });
        }
    }

    root_node
}

fn traverse_psd_tree<'a>(node: &'a PsdNode, depth: usize, action: &NodeAction<'a>) {
    // Execute the action for the current node, passing the current depth
    action(node, depth);

    // Recursively apply the action to each child, increasing the depth
    for child in &node.children {
        traverse_psd_tree(child, depth + 1, action);
    }
}
