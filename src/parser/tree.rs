/// Parser for tree-shaped diagrams (`@startmindmap` / `@startwbs`).
///
/// Lines are `*`-prefixed with the count giving the depth:
/// `* root`, `** child`, `*** grandchild`. `*[#color]` sets a node color and
/// `*_` removes the box. OrgMode `+`/`-` prefixes are accepted as aliases.
use crate::ast::tree::*;
use crate::error::PlantUmlError;

pub fn parse(body: &str) -> Result<TreeDiagram, PlantUmlError> {
    let mut diagram = TreeDiagram {
        title: None,
        roots: Vec::new(),
    };
    // We store the chain of nodes as indices; simplest is to build with a
    // recursive-ownership workaround: keep a Vec<TreeNode> chain and fold.
    let mut chain: Vec<(usize, TreeNode)> = Vec::new();

    fn fold_into(chain: &mut Vec<(usize, TreeNode)>, roots: &mut Vec<TreeNode>, min_level: usize) {
        // Pop nodes deeper or equal to min_level, attaching them to parents.
        while let Some(&(level, _)) = chain.last() {
            if level < min_level {
                break;
            }
            let (_, node) = chain.pop().unwrap();
            if let Some((_, parent)) = chain.last_mut() {
                parent.children.push(node);
            } else {
                roots.push(node);
            }
        }
    }

    for (line_no, raw) in body.lines().enumerate() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('\'') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("title ") {
            diagram.title = Some(rest.trim().to_string());
            continue;
        }
        if trimmed == "left side" || trimmed == "right side" {
            // Side switching is not supported yet; nodes stay on one side.
            continue;
        }
        if trimmed.starts_with("skinparam") || trimmed.starts_with("caption") {
            continue;
        }

        let bullet = trimmed
            .chars()
            .take_while(|&c| c == '*' || c == '+' || c == '-')
            .count();
        if bullet == 0 {
            continue;
        }
        let mut rest = &trimmed[bullet..];
        let mut node = TreeNode::new(String::new());
        if let Some(r) = rest.strip_prefix('_') {
            node.boxless = true;
            rest = r;
        }
        if let Some(r) = rest.strip_prefix('[') {
            if let Some(end) = r.find(']') {
                node.color = Some(r[..end].to_string());
                rest = &r[end + 1..];
            }
        }
        node.text = rest.trim().to_string();
        if node.text.is_empty() {
            return Err(PlantUmlError::ParseError {
                line: line_no + 1,
                message: "empty tree node".into(),
            });
        }

        // Attach: pop chain until the top is shallower than this node.
        fold_into(&mut chain, &mut diagram.roots, bullet);
        chain.push((bullet, node));
    }
    fold_into(&mut chain, &mut diagram.roots, 0);

    if diagram.roots.is_empty() {
        return Err(PlantUmlError::ParseError {
            line: 0,
            message: "tree diagram has no nodes".into(),
        });
    }
    Ok(diagram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tree() {
        let d = parse("* Root\n** A\n*** A1\n** B\n").unwrap();
        assert_eq!(d.roots.len(), 1);
        let root = &d.roots[0];
        assert_eq!(root.text, "Root");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].text, "A");
        assert_eq!(root.children[0].children[0].text, "A1");
        assert_eq!(root.children[1].text, "B");
    }

    #[test]
    fn test_color_and_boxless() {
        let d = parse("* Root\n**[#lightblue] Colored\n**_ NoBox\n").unwrap();
        assert_eq!(
            d.roots[0].children[0].color.as_deref(),
            Some("#lightblue")
        );
        assert!(d.roots[0].children[1].boxless);
    }

    #[test]
    fn test_level_skip_back() {
        let d = parse("* R\n** A\n*** A1\n** B\n*** B1\n*** B2\n").unwrap();
        let root = &d.roots[0];
        assert_eq!(root.children[1].children.len(), 2);
    }

    #[test]
    fn test_empty_fails() {
        assert!(parse("").is_err());
    }
}
