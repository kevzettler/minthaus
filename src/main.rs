extern crate psd;
use std::cell::RefCell;
use std::rc::Rc;

use base64::{engine::general_purpose, Engine as _};
use image::{ImageBuffer, Rgba};
use psd::{NodeType, Psd, PsdNode};

fn main() {
    // Example: Encode a token
    let token = "0101000300030003"; // token corresponding to layers
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
    println!("decoded hex token {:?}", decoded_token);

    let psd_bytes = include_bytes!("../kz-wingrivals-chars-final.psd");
    let psd = Psd::from_bytes(psd_bytes).unwrap();
    // psd.traverse(|node, depth| {
    //     let indent = " ".repeat(depth * 4); // 4 spaces per depth level
    //     if let Some(content) = &node.content() {
    //         match content {
    //             NodeType::Group(group) => println!("{}Group: {}", indent, group.name()),
    //             NodeType::Layer(layer) => println!("{}Layer: {}", indent, layer.name()),
    //         }
    //     }
    // });

    let top_level_groups = get_top_level_psd_groups(&psd);

    // // actually validates background group
    // let background_group = match root_node.children().last() {
    //     Some(last_top_level_group) => {
    //         let node = last_top_level_group.borrow();
    //         match &node.content() {
    //             Some(NodeType::Group(group)) => {
    //                 if group.name() == "_backgrounds" {
    //                     last_top_level_group
    //                 } else {
    //                     panic!("last root child not _backgrounds group");
    //                 }
    //             }
    //             _ => panic!("No _backgrounds group found"),
    //         }
    //     }
    //     None => panic!("No _backgrounds group found"),
    // };

    // get validation limits from group children lenth
    let token_validation_limits: Vec<u8> = top_level_groups
        .iter()
        .map(|group| {
            let len = group.borrow().children().len();
            // Convert len to u8, handling potential overflow
            len.try_into().unwrap_or(u8::MAX)
        })
        .collect();

    // decode token in to visibility array
    let group_visibility = decode_hex_to_visibility(&decoded_token);

    // validate that the group_visibility values are less than equal token limits
    for (index, visibility) in group_visibility.iter().enumerate() {
        if visibility > &token_validation_limits[index] {
            panic!(
                "token value for {:?} - {:?} exceeds validation of {:?}",
                index, visibility, &token_validation_limits[index]
            );
        }
    }

    output_image_from_visibility_index(&psd, &group_visibility);

    // let token_permutations = generate_permutations(&top_level_groups);
    // println!("****token_permutations{:?}", token_permutations.len());
}

fn get_top_level_psd_groups(psd: &Psd) -> Vec<Rc<RefCell<PsdNode>>> {
    let tree = psd.tree();
    let root_node = tree.borrow();

    let top_level_groups: Vec<Rc<RefCell<PsdNode>>> = root_node
        .children()
        .into_iter()
        .filter(|node_rc| {
            let node = node_rc.borrow();
            match &node.content() {
                Some(NodeType::Group(group)) => group.name() != "_backgrounds",
                _ => true,
            }
        })
        .collect();

    top_level_groups
}

fn get_background_nodes(psd: &Psd) -> Vec<Rc<RefCell<PsdNode>>> {
    let tree = psd.tree();
    let root_node = tree.borrow();

    // Find the last child and verify it's the _backgrounds group
    if let Some(last_top_level_group) = root_node.children().last() {
        let node = last_top_level_group.borrow();

        // Check if the last node is the _backgrounds group and clone its children
        if let Some(NodeType::Group(group)) = node.content() {
            if group.name() == "_backgrounds" {
                return node.children().iter().cloned().collect();
            }
        }
    }

    panic!("No _backgrounds group found or last root child is not _backgrounds group")
}

fn output_image_from_visibility_index(psd: &Psd, visibility_index: &[u8]) {
    let top_level_groups = get_top_level_psd_groups(psd);
    let background_nodes = get_background_nodes(psd);
    let visible_children = collect_visible_children(&top_level_groups, visibility_index);

    let mut cloned_and_extended_visible_children = visible_children.clone();

    for visible_child in &visible_children {
        let visible_child_name = if let Some(content) = visible_child.borrow().content() {
            match content {
                NodeType::Group(group) => group.name().to_string(),
                NodeType::Layer(layer) => layer.name().to_string(),
            }
        } else {
            continue;
        };

        // Add all background nodes with the same name as the visible child
        for background_node in &background_nodes {
            let background_node_name =
                if let Some(background_content) = background_node.borrow().content() {
                    match background_content {
                        NodeType::Group(group) => group.name().to_string(),
                        NodeType::Layer(layer) => layer.name().to_string(),
                    }
                } else {
                    continue;
                };

            if background_node_name == visible_child_name {
                cloned_and_extended_visible_children.push(background_node.clone());
            }
        }
    }

    let total_ids = get_layer_ids(&cloned_and_extended_visible_children);
    flatten_layers_and_output_png(total_ids, psd);
}

fn decode_hex_to_visibility(hex_str: &str) -> Vec<u8> {
    (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).unwrap_or(0))
        .collect()
}

fn generate_permutations(top_level_groups: &Vec<Rc<RefCell<PsdNode>>>) -> Vec<String> {
    // Gather the limits for each top-level group
    let limits = top_level_groups
        .iter()
        .map(|group| group.borrow().children().len())
        .collect::<Vec<_>>();

    // Generate all permutations within these limits
    let permutations = cartesian_product_with_limits(limits);

    // Convert permutations to hex tokens
    permutations
        .iter()
        .map(|perm| {
            perm.iter()
                .map(|&index| format!("{:02X}", index))
                .collect::<String>()
        })
        .collect()
}

// Helper function to generate Cartesian product with limits
fn cartesian_product_with_limits(limits: Vec<usize>) -> Vec<Vec<usize>> {
    let mut result = vec![vec![]];
    for &limit in &limits {
        let mut temp = Vec::with_capacity(result.len() * limit);
        for current in &result {
            for i in 0..limit {
                let mut next = current.clone();
                next.push(i);
                temp.push(next);
            }
        }
        result = temp;
    }
    result
}

fn collect_visible_children(
    top_level_groups: &[Rc<RefCell<PsdNode>>],
    group_visibility: &[u8],
) -> Vec<Rc<RefCell<PsdNode>>> {
    let mut visible_children = Vec::new();

    // Iterate over each group and its corresponding visibility index
    for (index, &child_visibility_index) in group_visibility.iter().enumerate() {
        // skip visiblity indexs of 0 means nothing for that slot
        if child_visibility_index.eq(&0) {
            continue;
        }
        if let Some(group) = top_level_groups.get(index) {
            let group_ref = group.borrow();
            // the visibility token starts at 1 indexed whereas the child index start at 0
            let visibility_index_clone = child_visibility_index.clone();
            let offset_visibility_index = if visibility_index_clone > 0 {
                visibility_index_clone - 1
            } else {
                visibility_index_clone
            };
            if let Some(child) = group_ref.children().get(offset_visibility_index as usize) {
                visible_children.push(Rc::clone(child));
            }
        }
    }

    visible_children
}

fn get_layer_ids(nodes: &[Rc<RefCell<PsdNode>>]) -> Vec<(Option<String>, String)> {
    let mut layer_ids = Vec::new();

    for node in nodes {
        if let Some(content) = node.borrow().content() {
            let parent_name = match content {
                NodeType::Group(group) => Some(group.name().to_string()),
                NodeType::Layer(_layer) => None, // Top leve layers can never be parent
            };
            collect_layer_ids_recursive(node, &parent_name, &mut layer_ids);
        } else {
            collect_layer_ids_recursive(node, &None, &mut layer_ids);
        }
    }

    layer_ids
}

fn collect_visible_layer_ids(
    top_level_groups: &[Rc<RefCell<PsdNode>>],
    group_visibility: &[u8],
) -> Vec<(Option<String>, String)> {
    let visible_children = collect_visible_children(top_level_groups, group_visibility);
    get_layer_ids(&visible_children)
}

fn collect_layer_ids_recursive(
    node: &Rc<RefCell<PsdNode>>,
    parent_name: &Option<String>,
    layer_ids: &mut Vec<(Option<String>, String)>,
) {
    let node_ref = node.borrow();

    if let Some(content) = &node_ref.content() {
        match content {
            NodeType::Group(group) => {
                let group_name = Some(group.name().to_string());
                for child in node_ref.children() {
                    collect_layer_ids_recursive(&child, &group_name, layer_ids);
                }
            }
            NodeType::Layer(layer) => {
                let layer_name = layer.name().to_string();
                // TODO these layer id's are no garaunteed unique. Its possible a group name and layer name have same id.
                // This might need to be fixed by assigning uuid or some unique id to every node at tree time.
                // or doing it localy deriving a copy of the tree traversing it and making new uinque id's with reference to the nodes or vice versa
                layer_ids.push((parent_name.clone(), layer_name));
            }
        }
    }
}

fn flatten_layers_and_output_png(layer_ids: Vec<(Option<String>, String)>, psd: &Psd) {
    let all_groups = psd.groups();
    // Flattening logic needs to be adjusted to match the new layer ID structure
    let flattened_image = psd
        .flatten_layers_rgba(&|(_idx, layer)| {
            // Check if the layer is in the list of visible layer IDs
            layer_ids.iter().any(|(parent_name_opt, layer_name)| {
                let possible_sub_index = layer.parent_id().unwrap_or(0);
                let parent_group_name = match all_groups.get(&possible_sub_index) {
                    Some(psd_group) => Some(psd_group.name()),
                    None => None,
                };

                // Match both layer name and parent name (if it exists)
                layer.name() == layer_name
                    && match parent_name_opt.as_deref() {
                        Some(parent_name) => {
                            parent_group_name.map_or(false, |name| name == parent_name)
                        }
                        None => parent_group_name.is_none(),
                    }
            })
        })
        .unwrap();

    // Convert the flattened image to an ImageBuffer and save as PNG
    let image_buffer =
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(psd.width(), psd.height(), flattened_image)
            .unwrap();
    image_buffer.save("./test-output.png").unwrap();
}
