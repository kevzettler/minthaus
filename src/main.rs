extern crate psd;
use std::cell::RefCell;
use std::rc::Rc;

use base64::{engine::general_purpose, Engine as _};
use psd::{NodeType, Psd, PsdGroup, PsdNode};

fn main() {
    // Example: Encode a token
    let token = "FFFF000300030003"; // token corresponding to layers
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
    psd.traverse(|node, depth| {
        let indent = " ".repeat(depth * 4); // 4 spaces per depth level
        if let Some(content) = &node.content() {
            match content {
                NodeType::Group(group) => println!("{}Group: {}", indent, group.name()),
                NodeType::Layer(layer) => println!("{}Layer: {}", indent, layer.name()),
            }
        }
    });
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

    let background_group = match root_node.children().last() {
        Some(last_top_level_group) => {
            let node = last_top_level_group.borrow();
            match &node.content() {
                Some(NodeType::Group(group)) => {
                    if group.name() == "_backgrounds" {
                        last_top_level_group
                    } else {
                        panic!("last root child not _backgrounds group");
                    }
                }
                _ => panic!("No _backgrounds group found"),
            }
        }
        None => panic!("No _backgrounds group found"),
    };

    // decode token in to visibility array
    let group_visibility = decode_hex_to_visibility(&decoded_token);

    // get validation limits from group children lenth
    let token_validation_limits: Vec<u8> = top_level_groups
        .iter()
        .map(|group| {
            let len = group.borrow().children().len();
            // Convert len to u8, handling potential overflow
            len.try_into().unwrap_or(u8::MAX)
        })
        .collect();

    // validate that the group_visibility values are less than equal token limits
    for (index, visibility) in group_visibility.iter().enumerate() {
        if visibility != &u8::MAX && visibility > &token_validation_limits[index] {
            panic!(
                "token value for {:?} - {:?} exceeds validation of {:?}",
                index, visibility, &token_validation_limits[index]
            );
        }
    }

    let token_permutations = generate_permutations(&top_level_groups);
    println!("****token_permutations{:?}", token_permutations.len());
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
