//! Strongly connected components.
//!
//! A textbook graph algorithm over plain indices — it knows nothing
//! about files, findings or severity. It lives here rather than beside
//! the finding that uses it because it changes for a different reason:
//! the correctness or cost of the algorithm, not what is worth
//! reporting about a project.

/// Tarjan's algorithm, iterative.
pub fn strongly_connected(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adjacency.len();
    let mut index = vec![usize::MAX; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut next_index = 0usize;
    let mut components = Vec::new();

    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        // (node, how many of its edges have been walked)
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];

        while let Some((v, edge)) = work.pop() {
            if edge == 0 {
                index[v] = next_index;
                lowlink[v] = next_index;
                next_index += 1;
                stack.push(v);
                on_stack[v] = true;
            }

            let mut recursed = false;
            for (offset, &w) in adjacency[v].iter().enumerate().skip(edge) {
                if index[w] == usize::MAX {
                    work.push((v, offset + 1));
                    work.push((w, 0));
                    recursed = true;
                    break;
                } else if on_stack[w] {
                    lowlink[v] = lowlink[v].min(index[w]);
                }
            }
            if recursed {
                continue;
            }

            if lowlink[v] == index[v] {
                let mut component = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    component.push(w);
                    if w == v {
                        break;
                    }
                }
                components.push(component);
            }

            // Propagate this node's lowlink into the parent that
            // suspended itself to visit it.
            if let Some(&(parent, _)) = work.last() {
                lowlink[parent] = lowlink[parent].min(lowlink[v]);
            }
        }
    }

    components
}
